// SPDX-License-Identifier: MIT OR Apache-2.0

//! Acceptance criteria status generation module.
//!
//! This module generates feature status reports showing the implementation
//! and testing status of all acceptance criteria in the spec ledger.

use anyhow::{Context, Result};
use chrono::Utc;
use flight_bdd_metrics::{
    collect_bdd_traceability_metrics as collect_bdd_traceability_metrics_from_crate,
    extract_crates_from_command as extract_crates_from_command_impl,
    extract_crates_from_reference,
    CoverageStatus,
    BddTraceabilityMetrics,
    BddScenario as BddTraceabilityScenario,
    AcceptanceCriteria as BddAcceptanceCriteria,
    SpecRequirement as BddRequirement,
    RequirementStatus as BddRequirementStatus,
    SpecLedger as BddSpecLedger,
    UNMAPPED_MICROCRATE,
};
use flight_workspace_meta::load_workspace_microcrate_names;
use serde_yaml::{Mapping, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;

use crate::cross_ref::{RequirementStatus, SpecLedger, TestReference};
use crate::gherkin::{GherkinScenario, parse_feature_files};

/// BDD coverage metrics aggregated from the spec ledger and Gherkin scenarios.
pub type BddCoverageMetrics = BddTraceabilityMetrics;

type AcStatus = CoverageStatus;

/// Generate BDD coverage metrics from a ledger and parsed scenarios.
pub fn compute_bdd_metrics(
    ledger: &SpecLedger,
    scenarios: &[GherkinScenario],
) -> BddCoverageMetrics {
    compute_bdd_metrics_with_workspace_crates(ledger, scenarios, false)
}

pub(crate) fn compute_bdd_metrics_with_workspace_crates(
    ledger: &SpecLedger,
    scenarios: &[GherkinScenario],
    include_workspace_crates: bool,
) -> BddCoverageMetrics {
    let mut metrics = collect_bdd_traceability_metrics_from_crate(
        &convert_cross_ref_ledger_for_bdd_metrics(ledger),
        &convert_gherkin_scenarios_for_bdd_metrics(scenarios),
    );

    if include_workspace_crates {
        if let Ok(members) = load_workspace_microcrate_names(".") {
            metrics = metrics.with_workspace_crates(members);
        }
    }

    metrics
}

fn convert_cross_ref_ledger_for_bdd_metrics(ledger: &SpecLedger) -> BddSpecLedger {
    BddSpecLedger {
        requirements: ledger
            .requirements
            .iter()
            .map(|requirement| BddRequirement {
                id: requirement.id.clone(),
                name: requirement.name.clone(),
                status: convert_requirement_status(&requirement.status),
                ac: requirement
                    .ac
                    .iter()
                    .map(|ac| BddAcceptanceCriteria {
                        id: ac.id.clone(),
                        description: ac.description.clone(),
                        tests: ac
                            .tests
                            .iter()
                            .map(convert_test_reference_to_yaml_value)
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn convert_requirement_status(status: &RequirementStatus) -> BddRequirementStatus {
    match status {
        RequirementStatus::Draft => BddRequirementStatus::Draft,
        RequirementStatus::Implemented => BddRequirementStatus::Implemented,
        RequirementStatus::Tested => BddRequirementStatus::Tested,
        RequirementStatus::Deprecated => BddRequirementStatus::Deprecated,
    }
}

fn status_for_requirement_status(
    status: &RequirementStatus,
    has_tests: bool,
    has_gherkin: bool,
) -> AcStatus {
    AcStatus::compute(&convert_requirement_status(status), has_tests, has_gherkin)
}

fn convert_gherkin_scenarios_for_bdd_metrics(
    scenarios: &[GherkinScenario],
) -> Vec<BddTraceabilityScenario> {
    scenarios
        .iter()
        .map(|scenario| BddTraceabilityScenario {
            file_path: scenario.file_path.clone(),
            line_number: scenario.line_number,
            name: scenario.name.clone(),
            tags: scenario.tags.clone(),
        })
        .collect()
}

fn convert_test_reference_to_yaml_value(reference: &TestReference) -> Value {
    match reference {
        TestReference::Simple(value) => Value::String(value.clone()),
        TestReference::Detailed {
            test,
            feature,
            command,
            ..
        } => {
            let mut mapping = Mapping::new();
            if let Some(test) = test {
                mapping.insert(Value::String("test".to_owned()), Value::String(test.clone()));
            }
            if let Some(feature) = feature {
                mapping.insert(
                    Value::String("feature".to_owned()),
                    Value::String(feature.clone()),
                );
            }
            if let Some(command) = command {
                mapping.insert(
                    Value::String("command".to_owned()),
                    Value::String(command.clone()),
                );
            }
            Value::Mapping(mapping)
        }
    }
}

/// Generate feature status report from spec ledger and Gherkin scenarios.
///
/// # Arguments
///
/// * `ledger` - The spec ledger containing requirements and acceptance criteria
/// * `scenarios` - List of parsed Gherkin scenarios
///
/// # Returns
///
/// Returns a markdown string containing the feature status report.
pub fn generate_feature_status(ledger: &SpecLedger, scenarios: &[GherkinScenario]) -> String {
    let metrics = compute_bdd_metrics_with_workspace_crates(ledger, scenarios, true);
    generate_feature_status_with_metrics(ledger, scenarios, &metrics)
}

fn generate_feature_status_with_metrics(
    ledger: &SpecLedger,
    scenarios: &[GherkinScenario],
    metrics: &BddCoverageMetrics,
) -> String {
    let mut output = String::new();

    // Build a map of AC ID -> list of Gherkin scenario locations
    let mut ac_to_scenarios: HashMap<String, Vec<String>> = HashMap::new();
    for scenario in scenarios {
        for ac_tag in scenario.ac_tags() {
            ac_to_scenarios.entry(ac_tag).or_default().push(format!(
                "{}:{}",
                scenario.file_path.display(),
                scenario.line_number
            ));
        }
    }

    // Get git commit hash
    let commit_hash = get_git_commit().unwrap_or_else(|_| "unknown".to_string());

    // Generate header
    output.push_str("# Feature Status Report\n\n");
    output.push_str(&format!(
        "**Generated:** {}\n\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    output.push_str(&format!("**Git Commit:** {}\n\n", commit_hash));
    output.push_str(&metrics.to_markdown());

    // Generate table
    output.push_str(
        "| REQ ID | AC ID | Description | Gherkin (file:line) | Tests (count) | Status |\n",
    );
    output.push_str(
        "|--------|-------|-------------|---------------------|---------------|--------|\n",
    );

    for req in &ledger.requirements {
        for ac in &req.ac {
            // Get Gherkin scenarios for this AC
            let gherkin_locations = ac_to_scenarios.get(&ac.id);
            let gherkin_display = if let Some(locations) = gherkin_locations {
                if locations.is_empty() {
                    "-".to_string()
                } else {
                    locations.join("<br>")
                }
            } else {
                "-".to_string()
            };

            // Count tests
            let test_count = ac.tests.len();

            // Compute status
            let has_tests = test_count > 0;
            let has_gherkin = gherkin_locations.is_some() && !gherkin_locations.unwrap().is_empty();
            let status = status_for_requirement_status(&req.status, has_tests, has_gherkin);

            // Format description (escape pipe characters)
            let description = ac.description.replace('|', "\\|");

            // Add row to table
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} {} |\n",
                req.id,
                ac.id,
                description,
                gherkin_display,
                test_count,
                status.icon(),
                status.text()
            ));
        }
    }

    output
}

fn collect_crate_names_for_tests(tests: &[TestReference]) -> Vec<String> {
    tests.iter().fold(BTreeSet::new(), |mut crates, test_ref| {
        let extracted = crate_names_from_reference(test_ref);
        for crate_name in extracted {
            crates.insert(crate_name);
        }
        crates
    })
    .into_iter()
    .collect()
}

fn crate_names_from_reference(test_ref: &TestReference) -> Vec<String> {
    match test_ref {
        TestReference::Simple(value) => extract_crate_names(value),
        TestReference::Detailed {
            test,
            feature,
            command,
            ..
        } => {
            let mut crate_names = Vec::new();
            if let Some(path) = test {
                crate_names.extend(extract_crate_names(path));
            }
            if feature.is_some() {
                crate_names.push("specs".to_string());
            }
            if let Some(command) = command {
                crate_names.extend(extract_crate_names(command));
                if command.starts_with("cmd:") {
                    crate_names.extend(extract_crates_from_command(
                        command.trim_start_matches("cmd:"),
                    ));
                } else {
                    crate_names.extend(extract_crates_from_command(command));
                }
            }
            crate_names
        }
    }
}

fn extract_crate_names(reference: &str) -> Vec<String> {
    extract_crates_from_reference(reference).into_iter().collect()
}

fn extract_crates_from_command(command: &str) -> Vec<String> {
    extract_crates_from_command_impl(command).into_iter().collect()
}

/// Get the current git commit hash.
///
/// # Returns
///
/// Returns the short commit hash (7 characters) or an error if git is not available.
fn get_git_commit() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .context("Failed to execute git command")?;

    if !output.status.success() {
        anyhow::bail!("git rev-parse failed");
    }

    let commit = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 in git output")?
        .trim()
        .to_string();

    Ok(commit)
}

/// Run the ac-status command to generate feature status report.
///
/// This function:
/// 1. Loads the spec ledger from specs/spec_ledger.yaml
/// 2. Parses Gherkin features from specs/features/
/// 3. Computes BDD and microcrate coverage metrics
/// 4. Generates the feature status report
/// 5. Writes the report to docs/feature_status.md
/// 6. Writes machine-readable metrics to docs/bdd_metrics.json
///
/// # Returns
///
/// Returns Ok(()) on success, or an error if any step fails.
pub fn run_ac_status() -> Result<()> {
    println!("Generating feature status report...");

    // Load spec ledger
    let ledger_path = Path::new("specs/spec_ledger.yaml");
    if !ledger_path.exists() {
        anyhow::bail!(
            "Spec ledger not found at {}. Please create it first.",
            ledger_path.display()
        );
    }

    let ledger_content =
        std::fs::read_to_string(ledger_path).context("Failed to read spec ledger")?;
    let ledger: SpecLedger =
        serde_yaml::from_str(&ledger_content).context("Failed to parse spec ledger YAML")?;

    println!(
        "  ✓ Loaded spec ledger with {} requirements",
        ledger.requirements.len()
    );

    // Parse Gherkin features
    let features_dir = Path::new("specs/features");
    let scenarios =
        parse_feature_files(features_dir).context("Failed to parse Gherkin feature files")?;

    println!("  ✓ Parsed {} Gherkin scenarios", scenarios.len());

    let metrics = compute_bdd_metrics_with_workspace_crates(&ledger, &scenarios, true);

    // Generate feature status report
    let report = generate_feature_status_with_metrics(&ledger, &scenarios, &metrics);

    // Write report to docs/feature_status.md
    let output_path = Path::new("docs/feature_status.md");
    let reference_output_path = Path::new("docs/reference/feature-status.md");
    let metrics_path = Path::new("docs/bdd_metrics.json");

    // Ensure docs directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create docs directory")?;
    }
    if let Some(parent) = reference_output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create docs/reference directory")?;
    }

    // Add auto-generated header
    let mut full_report = String::new();
    full_report.push_str("<!--\n");
    full_report.push_str("  AUTO-GENERATED FILE: DO NOT EDIT BY HAND.\n");
    full_report.push_str("  Generated by: cargo xtask ac-status\n");
    full_report.push_str(&format!(
        "  Generated at: {}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    full_report.push_str(&format!(
        "  Git commit: {}\n",
        get_git_commit().unwrap_or_else(|_| "unknown".to_string())
    ));
    full_report.push_str("  Source of truth: specs/spec_ledger.yaml, specs/features/*.feature\n");
    full_report.push_str("-->\n\n");
    full_report.push_str(&report);

    std::fs::write(output_path, &full_report).context("Failed to write feature status report")?;
    std::fs::write(
        reference_output_path,
        &full_report,
    )
    .context("Failed to write reference feature status report")?;
    std::fs::write(
        metrics_path,
        serde_json::to_string_pretty(&metrics).context("Failed to serialize BDD metrics")?,
    )
    .context("Failed to write BDD metrics JSON")?;

    println!(
        "  ✓ Generated feature status report at {}",
        output_path.display()
    );
    println!(
        "  ✓ Generated reference feature status report at {}",
        reference_output_path.display()
    );
    println!("  ✓ Wrote BDD metrics to {}", metrics_path.display());
    println!("\n✅ Feature status report generated successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cross_ref::{AcceptanceCriteria, Requirement, TestReference};
    use std::path::PathBuf;

    #[test]
    fn test_ac_status_compute_complete() {
        let status = status_for_requirement_status(&RequirementStatus::Tested, true, true);
        assert_eq!(status, AcStatus::Complete);
        assert_eq!(status.icon(), "✅");
        assert_eq!(status.text(), "Complete");
    }

    #[test]
    fn test_ac_status_compute_implemented_complete() {
        let status = status_for_requirement_status(&RequirementStatus::Implemented, true, true);
        assert_eq!(status, AcStatus::Complete);
        assert_eq!(status.icon(), "✅");
        assert_eq!(status.text(), "Complete");
    }

    #[test]
    fn test_ac_status_compute_needs_gherkin() {
        let status = status_for_requirement_status(&RequirementStatus::Implemented, true, false);
        assert_eq!(status, AcStatus::NeedsGherkin);
        assert_eq!(status.icon(), "🟡");
        assert_eq!(status.text(), "Needs Gherkin");
    }

    #[test]
    fn test_ac_status_compute_needs_tests() {
        let status = status_for_requirement_status(&RequirementStatus::Implemented, false, false);
        assert_eq!(status, AcStatus::NeedsTests);
        assert_eq!(status.icon(), "🟡");
        assert_eq!(status.text(), "Needs Tests");
    }

    #[test]
    fn test_ac_status_compute_draft() {
        let status = status_for_requirement_status(&RequirementStatus::Draft, false, false);
        assert_eq!(status, AcStatus::Draft);
        assert_eq!(status.icon(), "⚪");
        assert_eq!(status.text(), "Draft");
    }

    #[test]
    fn test_ac_status_compute_incomplete() {
        // Tested but no tests
        let status = status_for_requirement_status(&RequirementStatus::Tested, false, false);
        assert_eq!(status, AcStatus::Incomplete);
        assert_eq!(status.icon(), "❌");
        assert_eq!(status.text(), "Incomplete");

        // Tested with tests but no Gherkin
        let status = status_for_requirement_status(&RequirementStatus::Tested, true, false);
        assert_eq!(status, AcStatus::Incomplete);
    }

    #[test]
    fn test_generate_feature_status_basic() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Tested,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "First acceptance criteria".to_string(),
                    tests: vec![TestReference::Simple("test::path".to_string())],
                }],
            }],
        };

        let scenarios = vec![GherkinScenario {
            file_path: PathBuf::from("specs/features/test.feature"),
            line_number: 10,
            name: "Test scenario".to_string(),
            tags: vec!["REQ-1".to_string(), "AC-1.1".to_string()],
        }];

        let report = generate_feature_status(&ledger, &scenarios);

        // Check that report contains expected elements
        assert!(report.contains("# Feature Status Report"));
        assert!(report.contains("## BDD Coverage Metrics"));
        assert!(report.contains("REQ-1"));
        assert!(report.contains("AC-1.1"));
        assert!(report.contains("First acceptance criteria"));
        assert!(report.contains("specs/features/test.feature:10"));
        assert!(report.contains("1")); // test count
        assert!(report.contains("✅")); // Complete status
    }

    #[test]
    fn test_generate_feature_status_multiple_scenarios() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Tested,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "Test AC".to_string(),
                    tests: vec![TestReference::Simple("test::path".to_string())],
                }],
            }],
        };

        let scenarios = vec![
            GherkinScenario {
                file_path: PathBuf::from("specs/features/test1.feature"),
                line_number: 10,
                name: "Scenario 1".to_string(),
                tags: vec!["AC-1.1".to_string()],
            },
            GherkinScenario {
                file_path: PathBuf::from("specs/features/test2.feature"),
                line_number: 20,
                name: "Scenario 2".to_string(),
                tags: vec!["AC-1.1".to_string()],
            },
        ];

        let report = generate_feature_status(&ledger, &scenarios);

        // Check that both scenarios are listed
        assert!(report.contains("specs/features/test1.feature:10"));
        assert!(report.contains("specs/features/test2.feature:20"));
        assert!(report.contains("<br>")); // Multiple scenarios separated by <br>
    }

    #[test]
    fn test_generate_feature_status_no_gherkin() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Implemented,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "Test AC".to_string(),
                    tests: vec![TestReference::Simple("test::path".to_string())],
                }],
            }],
        };

        let scenarios = vec![]; // No Gherkin scenarios

        let report = generate_feature_status(&ledger, &scenarios);

        // Check status is "Needs Gherkin"
        assert!(report.contains("🟡"));
        assert!(report.contains("Needs Gherkin"));
        assert!(report.contains("-")); // No Gherkin scenarios
    }

    #[test]
    fn test_generate_feature_status_no_tests() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Implemented,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "Test AC".to_string(),
                    tests: vec![], // No tests
                }],
            }],
        };

        let scenarios = vec![];

        let report = generate_feature_status(&ledger, &scenarios);

        // Check status is "Needs Tests"
        assert!(report.contains("🟡"));
        assert!(report.contains("Needs Tests"));
        assert!(report.contains("0")); // Zero tests
    }

    #[test]
    fn test_generate_feature_status_draft() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Draft,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "Test AC".to_string(),
                    tests: vec![],
                }],
            }],
        };

        let scenarios = vec![];

        let report = generate_feature_status(&ledger, &scenarios);

        // Check status is "Draft"
        assert!(report.contains("⚪"));
        assert!(report.contains("Draft"));
    }

    #[test]
    fn test_generate_feature_status_escapes_pipes() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Draft,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "Test with | pipe character".to_string(),
                    tests: vec![],
                }],
            }],
        };

        let scenarios = vec![];

        let report = generate_feature_status(&ledger, &scenarios);

        // Check that pipe is escaped
        assert!(report.contains("Test with \\| pipe character"));
    }

    #[test]
    fn test_compute_bdd_metrics() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Tested,
                ac: vec![
                    AcceptanceCriteria {
                        id: "AC-1.1".to_string(),
                        description: "First AC".to_string(),
                        tests: vec![TestReference::Simple("test::path".to_string())],
                    },
                    AcceptanceCriteria {
                        id: "AC-1.2".to_string(),
                        description: "Second AC".to_string(),
                        tests: vec![],
                    },
                ],
            }],
        };

        let scenarios = vec![GherkinScenario {
            file_path: PathBuf::from("specs/features/test.feature"),
            line_number: 10,
            name: "Test scenario".to_string(),
            tags: vec!["AC-1.1".to_string()],
        }];

        let metrics = compute_bdd_metrics(&ledger, &scenarios);

        assert_eq!(metrics.total_ac, 2);
        assert_eq!(metrics.ac_with_tests, 1);
        assert_eq!(metrics.ac_with_gherkin, 1);
        assert_eq!(metrics.ac_with_tests_and_gherkin, 1);
        assert_eq!(metrics.complete, 1);
        assert_eq!(metrics.incomplete, 1);
        assert_eq!(metrics.draft, 0);
        assert_eq!(metrics.needs_gherkin, 0);
        assert_eq!(metrics.needs_tests, 0);
        assert_eq!(metrics.microcrate_total, 2);
        assert_eq!(metrics.microcrate_with_tests, 1);
        assert_eq!(metrics.microcrate_with_gherkin, 1);
        assert_eq!(metrics.microcrate_with_tests_and_gherkin, 1);
    }

    #[test]
    fn test_compute_bdd_metrics_includes_workspace_crates_when_requested() {
        let members = crate::cross_ref::load_workspace_crate_members().unwrap_or_default();
        if members.len() < 2 {
            // This test assumes at least one non-mapped workspace member exists.
            return;
        }

        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Tested,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "First AC".to_string(),
                    tests: vec![TestReference::Simple(
                        "flight-core::tests::test_alpha".to_string(),
                    )],
                }],
            }],
        };

        let scenarios = vec![GherkinScenario {
            file_path: PathBuf::from("specs/features/test.feature"),
            line_number: 10,
            name: "Test scenario".to_string(),
            tags: vec!["AC-1.1".to_string()],
        }];

        let metrics = compute_bdd_metrics_with_workspace_crates(&ledger, &scenarios, true);

        let has_flight_core = metrics
            .crate_coverage
            .iter()
            .any(|entry| entry.crate_name == "flight-core");
        assert!(has_flight_core);
        assert_eq!(metrics.microcrate_total, members.len());
    }

    #[test]
    fn test_collect_crate_names_from_test_references() {
        let tests = vec![
            TestReference::Simple("flight_core::tests::test_alpha".to_string()),
            TestReference::Simple("cmd:cargo test -p flight-axis".to_string()),
            TestReference::Simple("cmd:cargo xtask validate".to_string()),
            TestReference::Simple("cmd:cargo test --manifest-path specs/Cargo.toml".to_string()),
            TestReference::Simple("cmd:cargo test --manifest-path=crates/flight-ffb/Cargo.toml".to_string()),
            TestReference::Detailed {
                test: Some("flight-core::tests::integration".to_string()),
                feature: None,
                command: Some("cargo test -p flight-ipc".to_string()),
            },
            TestReference::Simple("feature:specs/features/some.feature:Scenario: X".to_string()),
            TestReference::Detailed {
                test: None,
                feature: Some("specs/features/some.feature".to_string()),
                command: None,
            },
        ];

        let crates = collect_crate_names_for_tests(&tests);

        assert_eq!(crates.len(), 6);
        assert!(crates.contains(&"flight-axis".to_string()));
        assert!(crates.contains(&"flight-core".to_string()));
        assert!(crates.contains(&"flight-ipc".to_string()));
        assert!(crates.contains(&"xtask".to_string()));
        assert!(crates.contains(&"specs".to_string()));
        assert!(crates.contains(&"flight-ffb".to_string()));
    }

    #[test]
    fn test_collect_crate_names_from_feature_reference() {
        let tests = vec![TestReference::Detailed {
            test: None,
            feature: Some("specs/features/some.feature".to_string()),
            command: None,
        }];

        let crates = collect_crate_names_for_tests(&tests);

        assert_eq!(crates, vec!["specs".to_string()]);
    }

    #[test]
    fn test_microcrate_bdd_metrics_includes_crate_breakdown() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Tested,
                ac: vec![
                    AcceptanceCriteria {
                        id: "AC-1.1".to_string(),
                        description: "First AC".to_string(),
                        tests: vec![TestReference::Simple(
                            "flight-core::tests::test_alpha".to_string(),
                        )],
                    },
                    AcceptanceCriteria {
                        id: "AC-1.2".to_string(),
                        description: "Second AC".to_string(),
                        tests: vec![TestReference::Simple(
                            "cmd:cargo test -p flight-core".to_string(),
                        )],
                    },
                ],
            }],
        };

        let scenarios = vec![GherkinScenario {
            file_path: PathBuf::from("specs/features/test.feature"),
            line_number: 10,
            name: "Test scenario".to_string(),
            tags: vec!["AC-1.1".to_string()],
        }];

        let metrics = compute_bdd_metrics(&ledger, &scenarios);

        assert_eq!(metrics.crate_coverage.len(), 1);
        assert_eq!(metrics.crate_coverage[0].crate_name, "flight-core");
        assert_eq!(metrics.crate_coverage[0].total_ac, 2);
        assert_eq!(metrics.crate_coverage[0].ac_with_tests, 2);
        assert_eq!(metrics.crate_coverage[0].ac_with_gherkin, 1);
        assert_eq!(metrics.crate_coverage[0].ac_with_tests_and_gherkin, 1);
        assert_eq!(metrics.microcrate_total, 1);
        assert_eq!(metrics.microcrate_with_tests, 1);
        assert_eq!(metrics.microcrate_with_gherkin, 1);
        assert_eq!(metrics.microcrate_with_tests_and_gherkin, 1);
    }

    #[test]
    fn test_compute_bdd_metrics_includes_unmapped_microcrate() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Implemented,
                ac: vec![
                    AcceptanceCriteria {
                        id: "AC-1.1".to_string(),
                        description: "Mapped AC".to_string(),
                        tests: vec![TestReference::Simple(
                            "flight-core::tests::test_alpha".to_string(),
                        )],
                    },
                    AcceptanceCriteria {
                        id: "AC-1.2".to_string(),
                        description: "Unmapped AC".to_string(),
                        tests: vec![],
                    },
                ],
            }],
        };

        let scenarios = vec![
            GherkinScenario {
                file_path: PathBuf::from("specs/features/test.feature"),
                line_number: 10,
                name: "Mapped".to_string(),
                tags: vec!["AC-1.1".to_string()],
            },
            GherkinScenario {
                file_path: PathBuf::from("specs/features/test.feature"),
                line_number: 20,
                name: "Unmapped".to_string(),
                tags: vec!["AC-1.2".to_string()],
            },
        ];

        let metrics = compute_bdd_metrics(&ledger, &scenarios);

        assert_eq!(metrics.total_ac, 2);
        assert_eq!(metrics.ac_with_tests, 1);
        assert_eq!(metrics.ac_with_gherkin, 2);
        assert_eq!(metrics.ac_with_tests_and_gherkin, 1);

        let unmapped = metrics
            .crate_coverage
            .iter()
            .find(|entry| entry.crate_name == UNMAPPED_MICROCRATE)
            .expect("Expected unmapped microcrate");
        assert_eq!(unmapped.total_ac, 1);
        assert_eq!(unmapped.ac_with_tests, 0);
        assert_eq!(unmapped.ac_with_gherkin, 1);

        let mapped = metrics
            .crate_coverage
            .iter()
            .find(|entry| entry.crate_name == "flight-core")
            .expect("Expected mapped microcrate");
        assert_eq!(mapped.total_ac, 1);
        assert_eq!(mapped.ac_with_tests, 1);
        assert_eq!(mapped.ac_with_gherkin, 1);
        assert_eq!(metrics.microcrate_total, 2);
        assert_eq!(metrics.microcrate_with_tests, 1);
        assert_eq!(metrics.microcrate_with_gherkin, 2);
        assert_eq!(metrics.microcrate_with_tests_and_gherkin, 1);
    }
}
