// SPDX-License-Identifier: MIT OR Apache-2.0

//! Acceptance criteria status generation module.
//!
//! This module generates feature status reports showing the implementation
//! and testing status of all acceptance criteria in the spec ledger.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::cross_ref::{RequirementStatus, SpecLedger};
use crate::gherkin::{GherkinScenario, parse_feature_files};

/// Status of an acceptance criteria based on Property 8 logic.
#[derive(Debug, PartialEq)]
enum AcStatus {
    /// ✅ Complete: status=tested, has tests, has Gherkin
    Complete,
    /// 🟡 Needs Gherkin: status=implemented, has tests, no Gherkin
    NeedsGherkin,
    /// 🟡 Needs Tests: status=implemented, no tests
    NeedsTests,
    /// ⚪ Draft: status=draft
    Draft,
    /// ❌ Incomplete: other cases
    Incomplete,
}

impl AcStatus {
    /// Get the status icon for display.
    fn icon(&self) -> &'static str {
        match self {
            AcStatus::Complete => "✅",
            AcStatus::NeedsGherkin => "🟡",
            AcStatus::NeedsTests => "🟡",
            AcStatus::Draft => "⚪",
            AcStatus::Incomplete => "❌",
        }
    }

    /// Get the status text for display.
    fn text(&self) -> &'static str {
        match self {
            AcStatus::Complete => "Complete",
            AcStatus::NeedsGherkin => "Needs Gherkin",
            AcStatus::NeedsTests => "Needs Tests",
            AcStatus::Draft => "Draft",
            AcStatus::Incomplete => "Incomplete",
        }
    }

    /// Compute the status based on Property 8 logic.
    ///
    /// Property 8 logic:
    /// - ✅ Complete: status=tested, has tests, has Gherkin
    /// - 🟡 Needs Gherkin: status=implemented, has tests, no Gherkin
    /// - 🟡 Needs Tests: status=implemented, no tests
    /// - ⚪ Draft: status=draft
    /// - ❌ Incomplete: other cases
    fn compute(req_status: &RequirementStatus, has_tests: bool, has_gherkin: bool) -> Self {
        match (req_status, has_tests, has_gherkin) {
            (RequirementStatus::Tested, true, true) => AcStatus::Complete,
            (RequirementStatus::Implemented, true, false) => AcStatus::NeedsGherkin,
            (RequirementStatus::Implemented, false, _) => AcStatus::NeedsTests,
            (RequirementStatus::Draft, _, _) => AcStatus::Draft,
            _ => AcStatus::Incomplete,
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
            let status = AcStatus::compute(&req.status, has_tests, has_gherkin);

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

/// Get the current git commit hash.
///
/// # Returns
///
/// Returns the short commit hash (7 characters) or an error if git is not available.
fn get_git_commit() -> Result<String> {
    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
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
/// 3. Generates the feature status report
/// 4. Writes the report to docs/feature_status.md
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

    // Generate feature status report
    let report = generate_feature_status(&ledger, &scenarios);

    // Write report to docs/feature_status.md
    let output_path = Path::new("docs/feature_status.md");

    // Ensure docs directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create docs directory")?;
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

    std::fs::write(output_path, full_report).context("Failed to write feature status report")?;

    println!(
        "  ✓ Generated feature status report at {}",
        output_path.display()
    );
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
        let status = AcStatus::compute(&RequirementStatus::Tested, true, true);
        assert_eq!(status, AcStatus::Complete);
        assert_eq!(status.icon(), "✅");
        assert_eq!(status.text(), "Complete");
    }

    #[test]
    fn test_ac_status_compute_needs_gherkin() {
        let status = AcStatus::compute(&RequirementStatus::Implemented, true, false);
        assert_eq!(status, AcStatus::NeedsGherkin);
        assert_eq!(status.icon(), "🟡");
        assert_eq!(status.text(), "Needs Gherkin");
    }

    #[test]
    fn test_ac_status_compute_needs_tests() {
        let status = AcStatus::compute(&RequirementStatus::Implemented, false, false);
        assert_eq!(status, AcStatus::NeedsTests);
        assert_eq!(status.icon(), "🟡");
        assert_eq!(status.text(), "Needs Tests");
    }

    #[test]
    fn test_ac_status_compute_draft() {
        let status = AcStatus::compute(&RequirementStatus::Draft, false, false);
        assert_eq!(status, AcStatus::Draft);
        assert_eq!(status.icon(), "⚪");
        assert_eq!(status.text(), "Draft");
    }

    #[test]
    fn test_ac_status_compute_incomplete() {
        // Tested but no tests
        let status = AcStatus::compute(&RequirementStatus::Tested, false, false);
        assert_eq!(status, AcStatus::Incomplete);
        assert_eq!(status.icon(), "❌");
        assert_eq!(status.text(), "Incomplete");

        // Tested with tests but no Gherkin
        let status = AcStatus::compute(&RequirementStatus::Tested, true, false);
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
}
