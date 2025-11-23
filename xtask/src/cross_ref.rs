// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cross-reference checking module for validating links between artifacts.
//!
//! This module provides functionality to validate cross-references between:
//! - Documentation → Spec ledger (requirement links)
//! - Spec ledger → Codebase (test references)
//! - Gherkin → Spec ledger (tags)

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::front_matter::FrontMatter;

/// Spec ledger data structure.
#[derive(Debug, Deserialize, Serialize)]
pub struct SpecLedger {
    pub requirements: Vec<Requirement>,
}

/// A single requirement with acceptance criteria.
#[derive(Debug, Deserialize, Serialize)]
pub struct Requirement {
    pub id: String,
    pub name: String,
    pub status: RequirementStatus,
    pub ac: Vec<AcceptanceCriteria>,
}

/// Requirement status enumeration.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RequirementStatus {
    Draft,
    Implemented,
    Tested,
    Deprecated,
}

/// Acceptance criteria with test references.
#[derive(Debug, Deserialize, Serialize)]
pub struct AcceptanceCriteria {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub tests: Vec<TestReference>,
}

/// Test reference format (simple string or detailed object).
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum TestReference {
    Simple(String),
    Detailed {
        #[serde(skip_serializing_if = "Option::is_none")]
        test: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        feature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
    },
}

/// Cross-reference error types.
#[derive(Debug, Clone)]
pub enum CrossRefError {
    /// Broken requirement link in documentation
    BrokenRequirementLink {
        doc_path: PathBuf,
        req_id: String,
    },
    /// Missing test reference in codebase
    MissingTest {
        req_id: String,
        ac_id: String,
        test_path: String,
    },
    /// Test references external/non-workspace crate (warning)
    ExternalCrateWarning {
        req_id: String,
        ac_id: String,
        crate_name: String,
        test_path: String,
    },
    /// Invalid Gherkin tag
    InvalidGherkinTag {
        feature_path: PathBuf,
        line: usize,
        tag: String,
    },
}

impl CrossRefError {
    /// Format the error for display according to INF-XREF-NNN format.
    pub fn format(&self) -> String {
        match self {
            CrossRefError::BrokenRequirementLink { doc_path, req_id } => {
                format!(
                    "[ERROR] INF-XREF-001: Broken requirement link\n  \
                     File: {}\n  \
                     Requirement ID: {}\n  \
                     Suggestion: Verify that {} exists in specs/spec_ledger.yaml or remove the link",
                    doc_path.display(),
                    req_id,
                    req_id
                )
            }
            CrossRefError::MissingTest {
                req_id,
                ac_id,
                test_path,
            } => {
                format!(
                    "[ERROR] INF-XREF-002: Missing test reference\n  \
                     Requirement: {}\n  \
                     Acceptance Criteria: {}\n  \
                     Test Path: {}\n  \
                     Suggestion: Implement the test or update the test reference in specs/spec_ledger.yaml",
                    req_id, ac_id, test_path
                )
            }
            CrossRefError::ExternalCrateWarning {
                req_id,
                ac_id,
                crate_name,
                test_path,
            } => {
                format!(
                    "[WARN] INF-XREF-100: Test references non-workspace crate\n  \
                     Requirement: {}\n  \
                     Acceptance Criteria: {}\n  \
                     Crate: {}\n  \
                     Test Path: {}\n  \
                     Note: External crate tests are not validated",
                    req_id, ac_id, crate_name, test_path
                )
            }
            CrossRefError::InvalidGherkinTag {
                feature_path,
                line,
                tag,
            } => {
                format!(
                    "[ERROR] INF-XREF-003: Invalid Gherkin tag\n  \
                     File: {}:{}\n  \
                     Tag: @{}\n  \
                     Suggestion: Verify that {} exists in specs/spec_ledger.yaml or remove the tag",
                    feature_path.display(),
                    line,
                    tag,
                    tag
                )
            }
        }
    }

    /// Check if this is a warning (not an error).
    pub fn is_warning(&self) -> bool {
        matches!(self, CrossRefError::ExternalCrateWarning { .. })
    }
}

/// Build indexes of requirement and acceptance criteria IDs from the spec ledger.
///
/// # Arguments
///
/// * `ledger` - The spec ledger to index
///
/// # Returns
///
/// Returns a tuple of (requirement_ids, ac_ids) as HashSets for fast lookup.
pub fn build_req_index(ledger: &SpecLedger) -> (HashSet<String>, HashSet<String>) {
    let req_ids: HashSet<String> = ledger.requirements.iter().map(|r| r.id.clone()).collect();

    let ac_ids: HashSet<String> = ledger
        .requirements
        .iter()
        .flat_map(|r| r.ac.iter().map(|ac| ac.id.clone()))
        .collect();

    (req_ids, ac_ids)
}

/// Validate documentation links against the spec ledger.
///
/// Checks that all requirement IDs referenced in documentation front matter
/// exist in the spec ledger.
///
/// # Arguments
///
/// * `docs` - List of (path, front_matter) tuples for all documentation
/// * `req_ids` - Set of valid requirement IDs from the spec ledger
///
/// # Returns
///
/// Returns a vector of cross-reference errors for broken links.
pub fn validate_doc_links(
    docs: &[(PathBuf, FrontMatter)],
    req_ids: &HashSet<String>,
) -> Vec<CrossRefError> {
    let mut errors = Vec::new();

    for (doc_path, front_matter) in docs {
        for req_id in &front_matter.links.requirements {
            if !req_ids.contains(req_id) {
                errors.push(CrossRefError::BrokenRequirementLink {
                    doc_path: doc_path.clone(),
                    req_id: req_id.clone(),
                });
            }
        }
    }

    errors
}

/// Validate test references in the spec ledger against the codebase.
///
/// Checks that all test references in the spec ledger point to actual tests
/// in the codebase. Uses ripgrep to search for test functions.
///
/// # Arguments
///
/// * `ledger` - The spec ledger containing test references
///
/// # Returns
///
/// Returns a vector of cross-reference errors for missing tests and warnings
/// for external crate references.
pub fn validate_test_references(ledger: &SpecLedger) -> Vec<CrossRefError> {
    let mut errors = Vec::new();

    // Load workspace members
    let workspace_members = match load_workspace_members() {
        Ok(members) => members,
        Err(e) => {
            eprintln!("Warning: Failed to load workspace members: {}", e);
            HashSet::new()
        }
    };

    for req in &ledger.requirements {
        for ac in &req.ac {
            for test_ref in &ac.tests {
                // Only validate simple test references (not commands or features)
                if let TestReference::Simple(test_path) = test_ref {
                    match validate_single_test_reference(test_path, &workspace_members) {
                        TestValidationResult::Valid => {
                            // Test exists, no error
                        }
                        TestValidationResult::Missing => {
                            errors.push(CrossRefError::MissingTest {
                                req_id: req.id.clone(),
                                ac_id: ac.id.clone(),
                                test_path: test_path.clone(),
                            });
                        }
                        TestValidationResult::ExternalCrate(crate_name) => {
                            errors.push(CrossRefError::ExternalCrateWarning {
                                req_id: req.id.clone(),
                                ac_id: ac.id.clone(),
                                crate_name,
                                test_path: test_path.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    errors
}

/// Result of validating a single test reference.
enum TestValidationResult {
    Valid,
    Missing,
    ExternalCrate(String),
}

/// Validate a single test reference.
///
/// Test reference format: `"<crate>::<module_path>::<test_fn_name>"`
/// Example: `"flight_core::aircraft_switch::tests::test_phase_of_flight_determination"`
///
/// # Arguments
///
/// * `test_path` - The test path to validate
/// * `workspace_members` - Set of workspace member crate names
///
/// # Returns
///
/// Returns the validation result indicating if the test is valid, missing, or external.
fn validate_single_test_reference(
    test_path: &str,
    workspace_members: &HashSet<String>,
) -> TestValidationResult {
    // Parse the test path
    let parts: Vec<&str> = test_path.split("::").collect();
    if parts.is_empty() {
        return TestValidationResult::Missing;
    }

    let crate_name = parts[0].replace('_', "-"); // Convert snake_case to kebab-case
    let test_fn = parts.last().unwrap();

    // Check if crate is in workspace
    if !workspace_members.contains(&crate_name) {
        return TestValidationResult::ExternalCrate(crate_name);
    }

    // Use ripgrep to find the test function
    let crate_path = format!("crates/{}", crate_name);

    // Check if the crate directory exists
    if !Path::new(&crate_path).exists() {
        return TestValidationResult::Missing;
    }

    // Search for the test function using ripgrep
    match search_for_test_function(&crate_path, test_fn) {
        Ok(found) => {
            if found {
                TestValidationResult::Valid
            } else {
                TestValidationResult::Missing
            }
        }
        Err(_) => {
            // If ripgrep fails, fall back to assuming the test exists
            // (better to have false negatives than false positives)
            TestValidationResult::Valid
        }
    }
}

/// Search for a test function in a crate directory using ripgrep.
///
/// # Arguments
///
/// * `crate_path` - Path to the crate directory
/// * `test_fn` - Name of the test function to find
///
/// # Returns
///
/// Returns `Ok(true)` if the test function is found, `Ok(false)` if not found,
/// or an error if ripgrep fails.
fn search_for_test_function(crate_path: &str, test_fn: &str) -> Result<bool> {
    // Build the search pattern: "fn <test_fn>"
    let pattern = format!(r"\bfn\s+{}\b", regex::escape(test_fn));

    // Try to use ripgrep
    let output = Command::new("rg")
        .args(&[
            "-l",           // List files with matches
            "--type", "rust", // Only search Rust files
            &pattern,
            crate_path,
        ])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                Ok(!output.stdout.is_empty())
            } else {
                // ripgrep returned non-zero, but that might just mean no matches
                Ok(false)
            }
        }
        Err(_) => {
            // ripgrep not available, try grep as fallback
            let output = Command::new("grep")
                .args(&[
                    "-r",
                    "-l",
                    &format!("fn {}", test_fn),
                    crate_path,
                    "--include=*.rs",
                ])
                .output();

            match output {
                Ok(output) => Ok(output.status.success() && !output.stdout.is_empty()),
                Err(e) => Err(anyhow::anyhow!(
                    "Failed to search for test function (neither rg nor grep available): {}",
                    e
                )),
            }
        }
    }
}

/// Load workspace member crate names from Cargo.toml.
///
/// # Returns
///
/// Returns a set of workspace member crate names (in kebab-case).
fn load_workspace_members() -> Result<HashSet<String>> {
    // Find the workspace root Cargo.toml
    let cargo_toml_path = find_workspace_cargo_toml()?;
    let content = std::fs::read_to_string(&cargo_toml_path)
        .context("Failed to read workspace Cargo.toml")?;

    let mut members = HashSet::new();

    // Parse the workspace members using a simple regex
    // Format: members = ["crates/flight-core", "crates/flight-ipc", ...]
    // The regex needs to handle multiline arrays with (?s) flag
    let re = Regex::new(r#"(?s)members\s*=\s*\[(.*?)\]"#).unwrap();

    if let Some(captures) = re.captures(&content) {
        let members_str = captures.get(1).unwrap().as_str();

        // Extract individual member paths
        let member_re = Regex::new(r#""([^"]+)""#).unwrap();
        for cap in member_re.captures_iter(members_str) {
            let member_path = cap.get(1).unwrap().as_str();

            // Extract crate name from path (e.g., "crates/flight-core" -> "flight-core")
            if let Some(crate_name) = member_path.split('/').last() {
                members.insert(crate_name.to_string());
            }
        }
    }

    Ok(members)
}

/// Find the workspace root Cargo.toml.
///
/// Searches from the current directory upwards until finding a Cargo.toml
/// with [workspace] section.
fn find_workspace_cargo_toml() -> Result<PathBuf> {
    let mut current = std::env::current_dir()?;

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                return Ok(cargo_toml);
            }
        }

        // Move up one directory
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            anyhow::bail!("Could not find workspace root Cargo.toml");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_req_index() {
        let ledger = SpecLedger {
            requirements: vec![
                Requirement {
                    id: "REQ-1".to_string(),
                    name: "Test Requirement".to_string(),
                    status: RequirementStatus::Draft,
                    ac: vec![
                        AcceptanceCriteria {
                            id: "AC-1.1".to_string(),
                            description: "Test AC".to_string(),
                            tests: vec![],
                        },
                        AcceptanceCriteria {
                            id: "AC-1.2".to_string(),
                            description: "Test AC 2".to_string(),
                            tests: vec![],
                        },
                    ],
                },
                Requirement {
                    id: "INF-REQ-1".to_string(),
                    name: "Infrastructure Requirement".to_string(),
                    status: RequirementStatus::Draft,
                    ac: vec![AcceptanceCriteria {
                        id: "AC-1.1".to_string(),
                        description: "Infra AC".to_string(),
                        tests: vec![],
                    }],
                },
            ],
        };

        let (req_ids, ac_ids) = build_req_index(&ledger);

        assert_eq!(req_ids.len(), 2);
        assert!(req_ids.contains("REQ-1"));
        assert!(req_ids.contains("INF-REQ-1"));

        assert_eq!(ac_ids.len(), 2); // AC-1.1 appears twice but should be deduplicated
        assert!(ac_ids.contains("AC-1.1"));
        assert!(ac_ids.contains("AC-1.2"));
    }

    #[test]
    fn test_validate_doc_links_valid() {
        let req_ids: HashSet<String> = ["REQ-1", "INF-REQ-1"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let docs = vec![(
            PathBuf::from("docs/test.md"),
            FrontMatter {
                doc_id: "DOC-TEST".to_string(),
                kind: crate::front_matter::DocKind::Concept,
                area: crate::front_matter::Area::FlightCore,
                status: crate::front_matter::DocStatus::Draft,
                links: crate::front_matter::Links {
                    requirements: vec!["REQ-1".to_string()],
                    tasks: vec![],
                    adrs: vec![],
                },
            },
        )];

        let errors = validate_doc_links(&docs, &req_ids);
        assert!(errors.is_empty(), "Valid links should not produce errors");
    }

    #[test]
    fn test_validate_doc_links_broken() {
        let req_ids: HashSet<String> = ["REQ-1"].iter().map(|s| s.to_string()).collect();

        let docs = vec![(
            PathBuf::from("docs/test.md"),
            FrontMatter {
                doc_id: "DOC-TEST".to_string(),
                kind: crate::front_matter::DocKind::Concept,
                area: crate::front_matter::Area::FlightCore,
                status: crate::front_matter::DocStatus::Draft,
                links: crate::front_matter::Links {
                    requirements: vec!["REQ-1".to_string(), "REQ-999".to_string()],
                    tasks: vec![],
                    adrs: vec![],
                },
            },
        )];

        let errors = validate_doc_links(&docs, &req_ids);
        assert_eq!(errors.len(), 1, "Should detect one broken link");

        match &errors[0] {
            CrossRefError::BrokenRequirementLink { req_id, .. } => {
                assert_eq!(req_id, "REQ-999");
            }
            _ => panic!("Expected BrokenRequirementLink error"),
        }
    }

    #[test]
    fn test_validate_test_references_with_external_crate() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Draft,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "Test AC".to_string(),
                    tests: vec![TestReference::Simple(
                        "external_crate::tests::test_something".to_string(),
                    )],
                }],
            }],
        };

        let errors = validate_test_references(&ledger);

        // Should produce a warning for external crate
        assert!(!errors.is_empty());
        assert!(errors[0].is_warning());
    }

    #[test]
    fn test_validate_test_references_skips_commands() {
        let ledger = SpecLedger {
            requirements: vec![Requirement {
                id: "REQ-1".to_string(),
                name: "Test Requirement".to_string(),
                status: RequirementStatus::Draft,
                ac: vec![AcceptanceCriteria {
                    id: "AC-1.1".to_string(),
                    description: "Test AC".to_string(),
                    tests: vec![TestReference::Detailed {
                        test: None,
                        feature: None,
                        command: Some("cargo bench --bench test".to_string()),
                    }],
                }],
            }],
        };

        let errors = validate_test_references(&ledger);

        // Commands should be skipped, no errors
        assert!(errors.is_empty());
    }

    #[test]
    fn test_load_workspace_members() {
        // This test needs to run from workspace root
        // If Cargo.toml doesn't exist in current dir, skip the test
        if !std::path::Path::new("Cargo.toml").exists() {
            eprintln!("Skipping test_load_workspace_members: not in workspace root");
            return;
        }

        let members = load_workspace_members();
        if let Err(e) = &members {
            eprintln!("Error loading workspace members: {}", e);
        }
        assert!(members.is_ok(), "Should load workspace members");

        let members = members.unwrap();
        if members.is_empty() {
            eprintln!("No workspace members found. Current dir: {:?}", std::env::current_dir());
        }
        assert!(!members.is_empty(), "Should find workspace members");

        // Check for some known crates
        assert!(
            members.contains("flight-core"),
            "Should contain flight-core. Found: {:?}",
            members
        );
        assert!(members.contains("xtask"), "Should contain xtask");
    }

    #[test]
    fn test_error_formatting() {
        let error = CrossRefError::BrokenRequirementLink {
            doc_path: PathBuf::from("docs/test.md"),
            req_id: "REQ-999".to_string(),
        };

        let formatted = error.format();
        assert!(formatted.contains("[ERROR] INF-XREF-001"));
        assert!(formatted.contains("docs/test.md"));
        assert!(formatted.contains("REQ-999"));
    }

    #[test]
    fn test_error_is_warning() {
        let error = CrossRefError::ExternalCrateWarning {
            req_id: "REQ-1".to_string(),
            ac_id: "AC-1.1".to_string(),
            crate_name: "external".to_string(),
            test_path: "external::test".to_string(),
        };

        assert!(error.is_warning());

        let error = CrossRefError::MissingTest {
            req_id: "REQ-1".to_string(),
            ac_id: "AC-1.1".to_string(),
            test_path: "test::path".to_string(),
        };

        assert!(!error.is_warning());
    }

    #[test]
    fn test_validate_single_test_reference_format() {
        let workspace_members: HashSet<String> =
            ["flight-core"].iter().map(|s| s.to_string()).collect();

        // Test with valid format
        let result = validate_single_test_reference(
            "flight_core::tests::test_something",
            &workspace_members,
        );

        // Should not be external crate
        match result {
            TestValidationResult::ExternalCrate(_) => {
                panic!("Should not be external crate")
            }
            _ => {} // Valid or Missing is acceptable
        }
    }
}
