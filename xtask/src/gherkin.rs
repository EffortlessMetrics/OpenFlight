// SPDX-License-Identifier: MIT OR Apache-2.0

//! Gherkin feature file parsing and validation module.
//!
//! This module provides functionality to parse Gherkin .feature files and
//! validate their tags against the spec ledger.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::cross_ref::CrossRefError;

/// A parsed Gherkin scenario with metadata.
#[derive(Debug, Clone)]
pub struct GherkinScenario {
    /// Path to the .feature file
    pub file_path: PathBuf,
    /// Line number where the scenario starts
    pub line_number: usize,
    /// Name of the scenario
    #[allow(dead_code)]
    pub name: String,
    /// Tags from both Feature and Scenario lines (merged)
    pub tags: Vec<String>,
}

impl GherkinScenario {
    /// Extract requirement tags (@REQ-* and @INF-REQ-*) from the scenario.
    pub fn req_tags(&self) -> Vec<String> {
        extract_req_tags(&self.tags)
    }

    /// Extract acceptance criteria tags (@AC-*) from the scenario.
    pub fn ac_tags(&self) -> Vec<String> {
        extract_ac_tags(&self.tags)
    }
}

/// Parse all .feature files in the given directory.
///
/// # Arguments
///
/// * `features_dir` - Path to the directory containing .feature files
///
/// # Returns
///
/// Returns a vector of parsed Gherkin scenarios.
pub fn parse_feature_files(features_dir: &Path) -> Result<Vec<GherkinScenario>> {
    let mut scenarios = Vec::new();

    // Check if the directory exists
    if !features_dir.exists() {
        // Return empty vector if directory doesn't exist (not an error)
        return Ok(scenarios);
    }

    // Walk the directory tree looking for .feature files
    for entry in WalkDir::new(features_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("feature") {
            let file_scenarios = parse_single_feature_file(path)
                .with_context(|| format!("Failed to parse feature file: {}", path.display()))?;
            scenarios.extend(file_scenarios);
        }
    }

    Ok(scenarios)
}

/// Parse a single .feature file.
///
/// # Arguments
///
/// * `path` - Path to the .feature file
///
/// # Returns
///
/// Returns a vector of parsed scenarios from the file.
fn parse_single_feature_file(path: &Path) -> Result<Vec<GherkinScenario>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read feature file: {}", path.display()))?;

    let mut scenarios = Vec::new();
    let mut feature_tags = Vec::new();
    let mut current_tags = Vec::new();
    let mut in_scenario = false;

    for (line_idx, line) in content.lines().enumerate() {
        let line_number = line_idx + 1;
        let trimmed = line.trim();

        // Parse tags (lines starting with @)
        if trimmed.starts_with('@') {
            let tags_on_line = parse_tags_from_line(trimmed);
            current_tags.extend(tags_on_line);
        }
        // Parse Feature line
        else if trimmed.starts_with("Feature:") {
            // Tags before Feature line are feature-level tags
            feature_tags = current_tags.clone();
            current_tags.clear();
            in_scenario = false;
        }
        // Parse Scenario line
        else if trimmed.starts_with("Scenario:") || trimmed.starts_with("Scenario Outline:") {
            let scenario_name = if trimmed.starts_with("Scenario:") {
                trimmed.trim_start_matches("Scenario:").trim().to_string()
            } else {
                trimmed
                    .trim_start_matches("Scenario Outline:")
                    .trim()
                    .to_string()
            };

            // Merge feature-level tags with scenario-level tags
            let mut all_tags = feature_tags.clone();
            all_tags.extend(current_tags.clone());

            scenarios.push(GherkinScenario {
                file_path: path.to_path_buf(),
                line_number,
                name: scenario_name,
                tags: all_tags,
            });

            current_tags.clear();
            in_scenario = true;
        }
        // Clear tags if we hit a non-tag, non-keyword line while not in a scenario
        else if !in_scenario && !trimmed.is_empty() && !trimmed.starts_with('#') {
            // Reset tags if we're not in a scenario and hit content
            if !trimmed.starts_with("Given")
                && !trimmed.starts_with("When")
                && !trimmed.starts_with("Then")
                && !trimmed.starts_with("And")
                && !trimmed.starts_with("But")
                && !trimmed.starts_with("Examples:")
                && !trimmed.starts_with('|')
            {
                current_tags.clear();
            }
        }
    }

    Ok(scenarios)
}

/// Parse tags from a line (tags are space-separated and start with @).
///
/// # Arguments
///
/// * `line` - The line to parse tags from
///
/// # Returns
///
/// Returns a vector of tag strings (without the @ prefix).
fn parse_tags_from_line(line: &str) -> Vec<String> {
    line.split_whitespace()
        .filter(|s| s.starts_with('@'))
        .map(|s| s.trim_start_matches('@').to_string())
        .collect()
}

/// Extract requirement tags (@REQ-* and @INF-REQ-*) from a list of tags.
///
/// # Arguments
///
/// * `tags` - List of tags to filter
///
/// # Returns
///
/// Returns a vector of requirement tag strings (without @ prefix).
pub fn extract_req_tags(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter(|tag| tag.starts_with("REQ-") || tag.starts_with("INF-REQ-"))
        .cloned()
        .collect()
}

/// Extract acceptance criteria tags (@AC-*) from a list of tags.
///
/// # Arguments
///
/// * `tags` - List of tags to filter
///
/// # Returns
///
/// Returns a vector of AC tag strings (without @ prefix).
pub fn extract_ac_tags(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter(|tag| tag.starts_with("AC-"))
        .cloned()
        .collect()
}

/// Validate Gherkin tags against the spec ledger.
///
/// Checks that all @REQ-*, @INF-REQ-*, and @AC-* tags in Gherkin scenarios
/// reference valid IDs from the spec ledger.
///
/// # Arguments
///
/// * `scenarios` - List of parsed Gherkin scenarios
/// * `req_ids` - Set of valid requirement IDs from the spec ledger
/// * `ac_ids` - Set of valid acceptance criteria IDs from the spec ledger
///
/// # Returns
///
/// Returns a vector of cross-reference errors for invalid tags.
pub fn validate_gherkin_tags(
    scenarios: &[GherkinScenario],
    req_ids: &HashSet<String>,
    ac_ids: &HashSet<String>,
) -> Vec<CrossRefError> {
    let mut errors = Vec::new();

    for scenario in scenarios {
        // Validate requirement tags
        for req_tag in scenario.req_tags() {
            if !req_ids.contains(&req_tag) {
                errors.push(CrossRefError::InvalidGherkinTag {
                    feature_path: scenario.file_path.clone(),
                    line: scenario.line_number,
                    tag: req_tag,
                });
            }
        }

        // Validate acceptance criteria tags
        for ac_tag in scenario.ac_tags() {
            if !ac_ids.contains(&ac_tag) {
                errors.push(CrossRefError::InvalidGherkinTag {
                    feature_path: scenario.file_path.clone(),
                    line: scenario.line_number,
                    tag: ac_tag,
                });
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_tags_from_line() {
        let tags = parse_tags_from_line("@REQ-1 @AC-1.1 @smoke");
        assert_eq!(tags, vec!["REQ-1", "AC-1.1", "smoke"]);

        let tags = parse_tags_from_line("@INF-REQ-3");
        assert_eq!(tags, vec!["INF-REQ-3"]);

        let tags = parse_tags_from_line("  @tag1   @tag2  ");
        assert_eq!(tags, vec!["tag1", "tag2"]);

        let tags = parse_tags_from_line("no tags here");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_req_tags() {
        let tags = vec![
            "REQ-1".to_string(),
            "AC-1.1".to_string(),
            "INF-REQ-2".to_string(),
            "smoke".to_string(),
        ];

        let req_tags = extract_req_tags(&tags);
        assert_eq!(req_tags, vec!["REQ-1", "INF-REQ-2"]);
    }

    #[test]
    fn test_extract_ac_tags() {
        let tags = vec![
            "REQ-1".to_string(),
            "AC-1.1".to_string(),
            "AC-2.3".to_string(),
            "smoke".to_string(),
        ];

        let ac_tags = extract_ac_tags(&tags);
        assert_eq!(ac_tags, vec!["AC-1.1", "AC-2.3"]);
    }

    #[test]
    fn test_parse_single_feature_file_basic() {
        let temp_dir = TempDir::new().unwrap();
        let feature_path = temp_dir.path().join("test.feature");

        let content = r#"
@REQ-1
Feature: Test Feature

  @AC-1.1
  Scenario: Test scenario
    Given a precondition
    When an action occurs
    Then a result is expected
"#;

        fs::write(&feature_path, content).unwrap();

        let scenarios = parse_single_feature_file(&feature_path).unwrap();
        assert_eq!(scenarios.len(), 1);

        let scenario = &scenarios[0];
        assert_eq!(scenario.name, "Test scenario");
        assert_eq!(scenario.line_number, 6);
        assert_eq!(scenario.tags, vec!["REQ-1", "AC-1.1"]);
        assert_eq!(scenario.req_tags(), vec!["REQ-1"]);
        assert_eq!(scenario.ac_tags(), vec!["AC-1.1"]);
    }

    #[test]
    fn test_parse_single_feature_file_multiple_scenarios() {
        let temp_dir = TempDir::new().unwrap();
        let feature_path = temp_dir.path().join("test.feature");

        let content = r#"
@REQ-1
Feature: Test Feature

  @AC-1.1
  Scenario: First scenario
    Given a precondition
    When an action occurs
    Then a result is expected

  @AC-1.2 @smoke
  Scenario: Second scenario
    Given another precondition
    When another action occurs
    Then another result is expected
"#;

        fs::write(&feature_path, content).unwrap();

        let scenarios = parse_single_feature_file(&feature_path).unwrap();
        assert_eq!(scenarios.len(), 2);

        // First scenario
        assert_eq!(scenarios[0].name, "First scenario");
        assert_eq!(scenarios[0].tags, vec!["REQ-1", "AC-1.1"]);

        // Second scenario
        assert_eq!(scenarios[1].name, "Second scenario");
        assert_eq!(scenarios[1].tags, vec!["REQ-1", "AC-1.2", "smoke"]);
    }

    #[test]
    fn test_parse_single_feature_file_scenario_outline() {
        let temp_dir = TempDir::new().unwrap();
        let feature_path = temp_dir.path().join("test.feature");

        let content = r#"
@REQ-1
Feature: Test Feature

  @AC-1.1
  Scenario Outline: Parameterized scenario
    Given a value <value>
    When processing occurs
    Then result is <result>

    Examples:
      | value | result |
      | 1     | 2      |
      | 2     | 4      |
"#;

        fs::write(&feature_path, content).unwrap();

        let scenarios = parse_single_feature_file(&feature_path).unwrap();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].name, "Parameterized scenario");
        assert_eq!(scenarios[0].tags, vec!["REQ-1", "AC-1.1"]);
    }

    #[test]
    fn test_parse_single_feature_file_tags_on_same_line() {
        let temp_dir = TempDir::new().unwrap();
        let feature_path = temp_dir.path().join("test.feature");

        let content = r#"
@REQ-1 @INF-REQ-2
Feature: Test Feature

  @AC-1.1 @AC-2.1
  Scenario: Test scenario
    Given a precondition
"#;

        fs::write(&feature_path, content).unwrap();

        let scenarios = parse_single_feature_file(&feature_path).unwrap();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(
            scenarios[0].tags,
            vec!["REQ-1", "INF-REQ-2", "AC-1.1", "AC-2.1"]
        );
        assert_eq!(scenarios[0].req_tags(), vec!["REQ-1", "INF-REQ-2"]);
        assert_eq!(scenarios[0].ac_tags(), vec!["AC-1.1", "AC-2.1"]);
    }

    #[test]
    fn test_parse_single_feature_file_tags_on_multiple_lines() {
        let temp_dir = TempDir::new().unwrap();
        let feature_path = temp_dir.path().join("test.feature");

        let content = r#"
@REQ-1
@INF-REQ-2
Feature: Test Feature

  @AC-1.1
  @AC-2.1
  @smoke
  Scenario: Test scenario
    Given a precondition
"#;

        fs::write(&feature_path, content).unwrap();

        let scenarios = parse_single_feature_file(&feature_path).unwrap();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(
            scenarios[0].tags,
            vec!["REQ-1", "INF-REQ-2", "AC-1.1", "AC-2.1", "smoke"]
        );
    }

    #[test]
    fn test_parse_feature_files_directory() {
        let temp_dir = TempDir::new().unwrap();
        let features_dir = temp_dir.path().join("features");
        fs::create_dir(&features_dir).unwrap();

        // Create first feature file
        let feature1_path = features_dir.join("feature1.feature");
        let content1 = r#"
@REQ-1
Feature: Feature 1

  @AC-1.1
  Scenario: Scenario 1
    Given a precondition
"#;
        fs::write(&feature1_path, content1).unwrap();

        // Create second feature file
        let feature2_path = features_dir.join("feature2.feature");
        let content2 = r#"
@REQ-2
Feature: Feature 2

  @AC-2.1
  Scenario: Scenario 2
    Given another precondition
"#;
        fs::write(&feature2_path, content2).unwrap();

        let scenarios = parse_feature_files(&features_dir).unwrap();
        assert_eq!(scenarios.len(), 2);
    }

    #[test]
    fn test_parse_feature_files_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent");

        let scenarios = parse_feature_files(&nonexistent).unwrap();
        assert!(scenarios.is_empty());
    }

    #[test]
    fn test_validate_gherkin_tags_valid() {
        let scenarios = vec![GherkinScenario {
            file_path: PathBuf::from("test.feature"),
            line_number: 5,
            name: "Test".to_string(),
            tags: vec!["REQ-1".to_string(), "AC-1.1".to_string()],
        }];

        let req_ids: HashSet<String> = ["REQ-1"].iter().map(|s| s.to_string()).collect();
        let ac_ids: HashSet<String> = ["AC-1.1"].iter().map(|s| s.to_string()).collect();

        let errors = validate_gherkin_tags(&scenarios, &req_ids, &ac_ids);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_gherkin_tags_invalid_req() {
        let scenarios = vec![GherkinScenario {
            file_path: PathBuf::from("test.feature"),
            line_number: 5,
            name: "Test".to_string(),
            tags: vec!["REQ-999".to_string()],
        }];

        let req_ids: HashSet<String> = ["REQ-1"].iter().map(|s| s.to_string()).collect();
        let ac_ids: HashSet<String> = HashSet::new();

        let errors = validate_gherkin_tags(&scenarios, &req_ids, &ac_ids);
        assert_eq!(errors.len(), 1);

        match &errors[0] {
            CrossRefError::InvalidGherkinTag { tag, .. } => {
                assert_eq!(tag, "REQ-999");
            }
            _ => panic!("Expected InvalidGherkinTag error"),
        }
    }

    #[test]
    fn test_validate_gherkin_tags_invalid_ac() {
        let scenarios = vec![GherkinScenario {
            file_path: PathBuf::from("test.feature"),
            line_number: 5,
            name: "Test".to_string(),
            tags: vec!["AC-999.1".to_string()],
        }];

        let req_ids: HashSet<String> = HashSet::new();
        let ac_ids: HashSet<String> = ["AC-1.1"].iter().map(|s| s.to_string()).collect();

        let errors = validate_gherkin_tags(&scenarios, &req_ids, &ac_ids);
        assert_eq!(errors.len(), 1);

        match &errors[0] {
            CrossRefError::InvalidGherkinTag { tag, .. } => {
                assert_eq!(tag, "AC-999.1");
            }
            _ => panic!("Expected InvalidGherkinTag error"),
        }
    }

    #[test]
    fn test_validate_gherkin_tags_mixed_valid_invalid() {
        let scenarios = vec![
            GherkinScenario {
                file_path: PathBuf::from("test.feature"),
                line_number: 5,
                name: "Valid".to_string(),
                tags: vec!["REQ-1".to_string(), "AC-1.1".to_string()],
            },
            GherkinScenario {
                file_path: PathBuf::from("test.feature"),
                line_number: 10,
                name: "Invalid".to_string(),
                tags: vec!["REQ-999".to_string(), "AC-999.1".to_string()],
            },
        ];

        let req_ids: HashSet<String> = ["REQ-1"].iter().map(|s| s.to_string()).collect();
        let ac_ids: HashSet<String> = ["AC-1.1"].iter().map(|s| s.to_string()).collect();

        let errors = validate_gherkin_tags(&scenarios, &req_ids, &ac_ids);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_gherkin_scenario_methods() {
        let scenario = GherkinScenario {
            file_path: PathBuf::from("test.feature"),
            line_number: 5,
            name: "Test".to_string(),
            tags: vec![
                "REQ-1".to_string(),
                "INF-REQ-2".to_string(),
                "AC-1.1".to_string(),
                "AC-2.1".to_string(),
                "smoke".to_string(),
            ],
        };

        assert_eq!(scenario.req_tags(), vec!["REQ-1", "INF-REQ-2"]);
        assert_eq!(scenario.ac_tags(), vec!["AC-1.1", "AC-2.1"]);
    }
}
