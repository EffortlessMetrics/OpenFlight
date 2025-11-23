// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for Gherkin parsing and validation.

use std::collections::HashSet;
use std::path::Path;

// Import the gherkin module from xtask
// Since xtask is a binary crate, we need to include the module directly
#[path = "../src/gherkin.rs"]
mod gherkin;

#[path = "../src/cross_ref.rs"]
mod cross_ref;

#[path = "../src/front_matter.rs"]
mod front_matter;

#[test]
fn test_parse_minimal_fixture_features() {
    let features_dir = Path::new("tests/fixtures/minimal/specs/features");

    let scenarios = gherkin::parse_feature_files(features_dir)
        .expect("Failed to parse feature files");

    assert!(!scenarios.is_empty(), "Should find at least one scenario");

    // Check that we found the expected scenarios
    let scenario_names: Vec<String> = scenarios.iter().map(|s| s.name.clone()).collect();
    assert!(
        scenario_names.contains(&"First acceptance criteria test".to_string()),
        "Should find 'First acceptance criteria test' scenario"
    );
    assert!(
        scenario_names.contains(&"Second acceptance criteria test".to_string()),
        "Should find 'Second acceptance criteria test' scenario"
    );
}

#[test]
fn test_parse_feature_extracts_tags() {
    let features_dir = Path::new("tests/fixtures/minimal/specs/features");

    let scenarios = gherkin::parse_feature_files(features_dir)
        .expect("Failed to parse feature files");

    // Find the first scenario
    let first_scenario = scenarios
        .iter()
        .find(|s| s.name == "First acceptance criteria test")
        .expect("Should find first scenario");

    // Check that feature-level and scenario-level tags are merged
    assert!(
        first_scenario.tags.contains(&"REQ-1".to_string()),
        "Should have feature-level tag REQ-1"
    );
    assert!(
        first_scenario.tags.contains(&"AC-1.1".to_string()),
        "Should have scenario-level tag AC-1.1"
    );

    // Check req_tags and ac_tags methods
    assert_eq!(first_scenario.req_tags(), vec!["REQ-1"]);
    assert_eq!(first_scenario.ac_tags(), vec!["AC-1.1"]);
}

#[test]
fn test_parse_feature_multiple_tags() {
    let features_dir = Path::new("tests/fixtures/minimal/specs/features");

    let scenarios = gherkin::parse_feature_files(features_dir)
        .expect("Failed to parse feature files");

    // Find the second scenario which has multiple tags
    let second_scenario = scenarios
        .iter()
        .find(|s| s.name == "Second acceptance criteria test")
        .expect("Should find second scenario");

    // Check that it has both AC and smoke tags
    assert!(second_scenario.tags.contains(&"AC-1.2".to_string()));
    assert!(second_scenario.tags.contains(&"smoke".to_string()));
}

#[test]
fn test_validate_gherkin_tags_with_valid_tags() {
    let features_dir = Path::new("tests/fixtures/minimal/specs/features");

    let scenarios = gherkin::parse_feature_files(features_dir)
        .expect("Failed to parse feature files");

    // Create valid requirement and AC IDs
    let req_ids: HashSet<String> = ["REQ-1"].iter().map(|s| s.to_string()).collect();
    let ac_ids: HashSet<String> = ["AC-1.1", "AC-1.2"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let errors = gherkin::validate_gherkin_tags(&scenarios, &req_ids, &ac_ids);

    assert!(
        errors.is_empty(),
        "Valid tags should not produce errors. Errors: {:?}",
        errors
    );
}

#[test]
fn test_validate_gherkin_tags_with_invalid_tags() {
    let features_dir = Path::new("tests/fixtures/invalid/gherkin_invalid_tags");

    let scenarios = gherkin::parse_feature_files(features_dir)
        .expect("Failed to parse feature files");

    // Create valid requirement and AC IDs (but not the ones in the invalid fixture)
    let req_ids: HashSet<String> = ["REQ-1"].iter().map(|s| s.to_string()).collect();
    let ac_ids: HashSet<String> = ["AC-1.1"].iter().map(|s| s.to_string()).collect();

    let errors = gherkin::validate_gherkin_tags(&scenarios, &req_ids, &ac_ids);

    assert!(
        !errors.is_empty(),
        "Invalid tags should produce errors"
    );

    // Check that we get errors for the invalid tags
    let error_messages: Vec<String> = errors.iter().map(|e| e.format()).collect();
    let all_errors = error_messages.join("\n");

    assert!(
        all_errors.contains("REQ-999") || all_errors.contains("REQ-888"),
        "Should report invalid REQ tags"
    );
    assert!(
        all_errors.contains("AC-999.1") || all_errors.contains("AC-888.1"),
        "Should report invalid AC tags"
    );
}

#[test]
fn test_extract_req_tags() {
    let tags = vec![
        "REQ-1".to_string(),
        "INF-REQ-2".to_string(),
        "AC-1.1".to_string(),
        "smoke".to_string(),
    ];

    let req_tags = gherkin::extract_req_tags(&tags);

    assert_eq!(req_tags.len(), 2);
    assert!(req_tags.contains(&"REQ-1".to_string()));
    assert!(req_tags.contains(&"INF-REQ-2".to_string()));
}

#[test]
fn test_extract_ac_tags() {
    let tags = vec![
        "REQ-1".to_string(),
        "AC-1.1".to_string(),
        "AC-2.3".to_string(),
        "smoke".to_string(),
    ];

    let ac_tags = gherkin::extract_ac_tags(&tags);

    assert_eq!(ac_tags.len(), 2);
    assert!(ac_tags.contains(&"AC-1.1".to_string()));
    assert!(ac_tags.contains(&"AC-2.3".to_string()));
}

#[test]
fn test_error_formatting() {
    use std::path::PathBuf;

    let error = cross_ref::CrossRefError::InvalidGherkinTag {
        feature_path: PathBuf::from("specs/features/test.feature"),
        line: 10,
        tag: "REQ-999".to_string(),
    };

    let formatted = error.format();

    assert!(formatted.contains("[ERROR] INF-XREF-003"));
    assert!(formatted.contains("specs/features/test.feature:10"));
    assert!(formatted.contains("REQ-999"));
    assert!(formatted.contains("Suggestion"));
}
