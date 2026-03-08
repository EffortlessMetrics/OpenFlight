// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Depth tests for flight-bdd-metrics: parsing, metrics, reports, edge cases.

use flight_bdd_metrics::*;
use std::collections::BTreeSet;
use std::fs;
use tempfile::TempDir;

// ── Helpers ────────────────────────────────────────────────────────

fn write_feature(dir: &TempDir, name: &str, content: &str) {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
}

fn make_ledger(requirements: Vec<SpecRequirement>) -> SpecLedger {
    SpecLedger { requirements }
}

fn make_req(id: &str, status: RequirementStatus, acs: Vec<AcceptanceCriteria>) -> SpecRequirement {
    SpecRequirement {
        id: id.to_string(),
        name: format!("Requirement {id}"),
        status,
        ac: acs,
    }
}

fn make_ac(id: &str, tests: Vec<&str>) -> AcceptanceCriteria {
    AcceptanceCriteria {
        id: id.to_string(),
        description: format!("Acceptance criteria {id}"),
        tests: tests
            .into_iter()
            .map(|t| serde_yaml::Value::String(t.to_string()))
            .collect(),
    }
}

fn make_ac_no_tests(id: &str) -> AcceptanceCriteria {
    AcceptanceCriteria {
        id: id.to_string(),
        description: format!("Acceptance criteria {id}"),
        tests: vec![],
    }
}

// ── Gherkin Parsing ────────────────────────────────────────────────

#[test]
fn parse_simple_scenario() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "basic.feature",
        "\
Feature: Basic feature

  Scenario: First scenario
    Given a precondition
    When an action
    Then a result
",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 1);
    assert_eq!(scenarios[0].name, "First scenario");
    assert!(scenarios[0].tags.is_empty());
    assert_eq!(scenarios[0].line_number, 3);
}

#[test]
fn parse_scenario_outline() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "outline.feature",
        "\
Feature: Outline feature

  Scenario Outline: Parameterized test
    Given input <val>
    When processed
    Then result <expected>
",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 1);
    assert_eq!(scenarios[0].name, "Parameterized test");
}

#[test]
fn parse_feature_level_tags_inherited() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "tagged.feature",
        "\
@AC-100 @smoke
Feature: Tagged feature

  Scenario: Inherits feature tags
    Given something
",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 1);
    assert_eq!(scenarios[0].tags, vec!["AC-100", "smoke"]);
}

#[test]
fn parse_scenario_tags_merged_with_feature_tags() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "merged.feature",
        "\
@AC-001
Feature: Merged tags

  @AC-002 @regression
  Scenario: Has both
    Given something
",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 1);
    assert_eq!(scenarios[0].tags, vec!["AC-001", "AC-002", "regression"]);
}

#[test]
fn parse_multiple_scenarios_in_one_file() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "multi.feature",
        "\
Feature: Multiple scenarios

  Scenario: First
    Given step one

  Scenario: Second
    Given step two

  Scenario Outline: Third
    Given step <n>
",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 3);
    assert_eq!(scenarios[0].name, "First");
    assert_eq!(scenarios[1].name, "Second");
    assert_eq!(scenarios[2].name, "Third");
}

#[test]
fn parse_feature_files_across_subdirectories() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "sub/a.feature",
        "Feature: A\n  Scenario: Sub A\n    Given x\n",
    );
    write_feature(
        &dir,
        "sub/deep/b.feature",
        "Feature: B\n  Scenario: Sub B\n    Given y\n",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 2);
    let names: BTreeSet<_> = scenarios.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains("Sub A"));
    assert!(names.contains("Sub B"));
}

#[test]
fn parse_ignores_non_feature_files() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "readme.md",
        "# Not a feature\nScenario: Fake\n  Given fake\n",
    );
    write_feature(
        &dir,
        "actual.feature",
        "Feature: Real\n  Scenario: Real one\n    Given real\n",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 1);
    assert_eq!(scenarios[0].name, "Real one");
}

#[test]
fn parse_empty_feature_file() {
    let dir = TempDir::new().unwrap();
    write_feature(&dir, "empty.feature", "");

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert!(scenarios.is_empty());
}

#[test]
fn parse_feature_with_no_scenarios() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "no_scenarios.feature",
        "Feature: Empty feature\n  # No scenarios here\n",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert!(scenarios.is_empty());
}

#[test]
fn parse_nonexistent_directory_returns_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("does_not_exist");
    let scenarios = collect_gherkin_scenarios(path).unwrap();
    assert!(scenarios.is_empty());
}

#[test]
fn parse_scenario_tags_reset_between_scenarios() {
    let dir = TempDir::new().unwrap();
    write_feature(
        &dir,
        "reset.feature",
        "\
Feature: Tag reset

  @AC-010
  Scenario: Tagged one
    Given tagged

  Scenario: Untagged one
    Given untagged
",
    );

    let scenarios = collect_gherkin_scenarios(dir.path()).unwrap();
    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].tags, vec!["AC-010"]);
    assert!(scenarios[1].tags.is_empty());
}

// ── Tag Filtering ──────────────────────────────────────────────────

#[test]
fn ac_tags_filters_only_ac_prefixed() {
    let scenario = BddScenario {
        file_path: "test.feature".into(),
        line_number: 1,
        name: "tagged".into(),
        tags: vec![
            "AC-001".into(),
            "smoke".into(),
            "AC-999".into(),
            "regression".into(),
        ],
    };
    let ac_tags = scenario.ac_tags();
    assert_eq!(ac_tags, vec!["AC-001", "AC-999"]);
}

#[test]
fn ac_tags_empty_when_no_ac_tags() {
    let scenario = BddScenario {
        file_path: "test.feature".into(),
        line_number: 1,
        name: "no ac".into(),
        tags: vec!["smoke".into(), "wip".into()],
    };
    assert!(scenario.ac_tags().is_empty());
}

#[test]
fn parse_tags_from_line_multiple_tags() {
    let tags = parse_tags_from_line("@AC-001 @AC-002 @smoke @regression");
    assert_eq!(tags, vec!["AC-001", "AC-002", "smoke", "regression"]);
}

#[test]
fn parse_tags_from_line_empty_input() {
    assert!(parse_tags_from_line("").is_empty());
}

#[test]
fn parse_tags_from_line_no_at_sign() {
    assert!(parse_tags_from_line("just some text").is_empty());
}

// ── Metrics Collection & Aggregation ───────────────────────────────

#[test]
fn metrics_with_no_requirements_produces_zeros() {
    let ledger = make_ledger(vec![]);
    let metrics = collect_bdd_traceability_metrics(&ledger, &[]);

    assert_eq!(metrics.total_ac, 0);
    assert_eq!(metrics.ac_with_tests, 0);
    assert_eq!(metrics.ac_with_gherkin, 0);
    assert!(metrics.crate_coverage.is_empty());
}

#[test]
fn metrics_counts_ac_with_tests() {
    let ledger = make_ledger(vec![make_req(
        "REQ-1",
        RequirementStatus::Implemented,
        vec![
            make_ac("AC-001", vec!["cargo test -p flight-core"]),
            make_ac_no_tests("AC-002"),
        ],
    )]);

    let metrics = collect_bdd_traceability_metrics(&ledger, &[]);
    assert_eq!(metrics.total_ac, 2);
    assert_eq!(metrics.ac_with_tests, 1);
    assert_eq!(metrics.ac_with_gherkin, 0);
}

#[test]
fn metrics_counts_ac_with_gherkin() {
    let ledger = make_ledger(vec![make_req(
        "REQ-1",
        RequirementStatus::Implemented,
        vec![make_ac_no_tests("AC-010")],
    )]);

    let scenarios = vec![BddScenario {
        file_path: "test.feature".into(),
        line_number: 1,
        name: "covers AC-010".into(),
        tags: vec!["AC-010".into()],
    }];

    let metrics = collect_bdd_traceability_metrics(&ledger, &scenarios);
    assert_eq!(metrics.ac_with_gherkin, 1);
}

#[test]
fn metrics_counts_ac_with_both_tests_and_gherkin() {
    let ledger = make_ledger(vec![make_req(
        "REQ-1",
        RequirementStatus::Tested,
        vec![make_ac("AC-020", vec!["cargo test -p flight-axis"])],
    )]);

    let scenarios = vec![BddScenario {
        file_path: "test.feature".into(),
        line_number: 1,
        name: "covers AC-020".into(),
        tags: vec!["AC-020".into()],
    }];

    let metrics = collect_bdd_traceability_metrics(&ledger, &scenarios);
    assert_eq!(metrics.ac_with_tests_and_gherkin, 1);
    assert_eq!(metrics.complete, 1);
}

#[test]
fn metrics_categorizes_coverage_statuses() {
    let ledger = make_ledger(vec![
        make_req(
            "REQ-1",
            RequirementStatus::Draft,
            vec![make_ac_no_tests("AC-D1")],
        ),
        make_req(
            "REQ-2",
            RequirementStatus::Implemented,
            vec![
                make_ac("AC-NG1", vec!["flight_core::SomeTest"]),
                make_ac_no_tests("AC-NT1"),
            ],
        ),
        make_req(
            "REQ-3",
            RequirementStatus::Deprecated,
            vec![make_ac_no_tests("AC-INC1")],
        ),
    ]);

    let metrics = collect_bdd_traceability_metrics(&ledger, &[]);
    assert_eq!(metrics.draft, 1);
    assert_eq!(metrics.needs_gherkin, 1);
    assert_eq!(metrics.needs_tests, 1);
    assert_eq!(metrics.incomplete, 1);
}

#[test]
fn metrics_unmapped_crate_created_for_acs_without_test_crate() {
    let ledger = make_ledger(vec![make_req(
        "REQ-1",
        RequirementStatus::Implemented,
        vec![make_ac_no_tests("AC-U1")],
    )]);

    let metrics = collect_bdd_traceability_metrics(&ledger, &[]);
    assert!(metrics.has_unmapped_microcrate());
    let unmapped = metrics.crate_coverage_for(UNMAPPED_MICROCRATE).unwrap();
    assert_eq!(unmapped.total_ac, 1);
}

#[test]
fn metrics_per_crate_breakdown_correct() {
    let ledger = make_ledger(vec![make_req(
        "REQ-1",
        RequirementStatus::Implemented,
        vec![
            make_ac("AC-A1", vec!["cargo test -p flight-core"]),
            make_ac("AC-A2", vec!["cargo test -p flight-axis"]),
            make_ac("AC-A3", vec!["cargo test -p flight-core"]),
        ],
    )]);

    let metrics = collect_bdd_traceability_metrics(&ledger, &[]);
    let core = metrics.crate_coverage_for("flight-core").unwrap();
    assert_eq!(core.total_ac, 2);
    assert_eq!(core.ac_with_tests, 2);

    let axis = metrics.crate_coverage_for("flight-axis").unwrap();
    assert_eq!(axis.total_ac, 1);
}

#[test]
fn metrics_microcrate_totals_recomputed() {
    let ledger = make_ledger(vec![make_req(
        "REQ-1",
        RequirementStatus::Tested,
        vec![make_ac("AC-T1", vec!["cargo test -p flight-bus"])],
    )]);

    let scenarios = vec![BddScenario {
        file_path: "t.feature".into(),
        line_number: 1,
        name: "cover".into(),
        tags: vec!["AC-T1".into()],
    }];

    let metrics = collect_bdd_traceability_metrics(&ledger, &scenarios);
    assert_eq!(metrics.microcrate_total, 1);
    assert_eq!(metrics.microcrate_with_tests, 1);
    assert_eq!(metrics.microcrate_with_gherkin, 1);
    assert_eq!(metrics.microcrate_with_tests_and_gherkin, 1);
}

// ── BddTraceabilityRow ────────────────────────────────────────────

#[test]
fn row_new_initializes_zeroed() {
    let row = BddTraceabilityRow::new("flight-test");
    assert_eq!(row.crate_name, "flight-test");
    assert_eq!(row.total_ac, 0);
    assert_eq!(row.ac_with_tests, 0);
    assert!(!row.is_fully_tested());
}

#[test]
fn row_fully_tested_requires_nonzero_total() {
    let mut row = BddTraceabilityRow::new("empty");
    assert!(!row.is_fully_tested());
    row.total_ac = 5;
    row.ac_with_tests = 5;
    assert!(row.is_fully_tested());
}

#[test]
fn row_fully_gherkin_requires_nonzero_total() {
    let mut row = BddTraceabilityRow::new("empty");
    assert!(!row.is_fully_gherkin_covered());
    row.total_ac = 3;
    row.ac_with_gherkin = 3;
    assert!(row.is_fully_gherkin_covered());
}

#[test]
fn row_fully_both_covered() {
    let mut row = BddTraceabilityRow::new("full");
    row.total_ac = 2;
    row.ac_with_tests_and_gherkin = 2;
    assert!(row.is_fully_both_covered());
}

#[test]
fn row_is_unmapped_detects_sentinel() {
    assert!(BddTraceabilityRow::new(UNMAPPED_MICROCRATE).is_unmapped());
    assert!(!BddTraceabilityRow::new("flight-core").is_unmapped());
}

// ── Coverage Percent ───────────────────────────────────────────────

#[test]
fn coverage_percent_partial() {
    let pct = coverage_percent(3, 10);
    assert!((pct - 30.0).abs() < 0.01);
}

#[test]
fn coverage_percent_over_100_possible() {
    let pct = coverage_percent(15, 10);
    assert!((pct - 150.0).abs() < 0.01);
}

// ── CoverageStatus ────────────────────────────────────────────────

#[test]
fn coverage_status_icons_are_nonempty() {
    let statuses = [
        CoverageStatus::Complete,
        CoverageStatus::NeedsGherkin,
        CoverageStatus::NeedsTests,
        CoverageStatus::Draft,
        CoverageStatus::Incomplete,
    ];
    for status in &statuses {
        assert!(!status.icon().is_empty());
        assert!(!status.text().is_empty());
    }
}

#[test]
fn coverage_status_implemented_with_both_is_complete() {
    assert_eq!(
        CoverageStatus::compute(&RequirementStatus::Implemented, true, true),
        CoverageStatus::Complete
    );
}

#[test]
fn coverage_status_implemented_no_gherkin() {
    assert_eq!(
        CoverageStatus::compute(&RequirementStatus::Implemented, true, false),
        CoverageStatus::NeedsGherkin
    );
}

#[test]
fn coverage_status_implemented_no_tests() {
    assert_eq!(
        CoverageStatus::compute(&RequirementStatus::Implemented, false, true),
        CoverageStatus::NeedsTests
    );
}

// ── Report Generation: Markdown ────────────────────────────────────

#[test]
fn markdown_report_contains_header() {
    let metrics = BddTraceabilityMetrics::default();
    let md = metrics.to_markdown();
    assert!(md.contains("## BDD Coverage Metrics"));
}

#[test]
fn markdown_report_empty_matrix_message() {
    let metrics = BddTraceabilityMetrics::default();
    let md = metrics.to_markdown();
    assert!(md.contains("No microcrate test mappings discovered yet."));
}

#[test]
fn markdown_report_with_rows_contains_table() {
    let ledger = make_ledger(vec![make_req(
        "REQ-1",
        RequirementStatus::Tested,
        vec![make_ac("AC-M1", vec!["cargo test -p flight-core"])],
    )]);
    let scenarios = vec![BddScenario {
        file_path: "t.feature".into(),
        line_number: 1,
        name: "test".into(),
        tags: vec!["AC-M1".into()],
    }];

    let metrics = collect_bdd_traceability_metrics(&ledger, &scenarios);
    let md = metrics.to_markdown();
    assert!(md.contains("| flight-core |"));
    assert!(md.contains("| Microcrate |"));
    assert!(md.contains("100.0%"));
}

#[test]
fn markdown_row_formatting() {
    let mut row = BddTraceabilityRow::new("flight-test");
    row.total_ac = 10;
    row.ac_with_tests = 8;
    row.ac_with_gherkin = 6;
    row.ac_with_tests_and_gherkin = 5;
    row.complete = 5;

    let md = row.to_markdown_row();
    assert!(md.starts_with("| flight-test |"));
    assert!(md.contains("80.0%"));
    assert!(md.contains("60.0%"));
    assert!(md.contains("50.0%"));
}

#[test]
fn markdown_report_shows_global_percentages() {
    let metrics = BddTraceabilityMetrics {
        total_ac: 10,
        ac_with_tests: 5,
        ..Default::default()
    };
    let md = metrics.to_markdown();
    assert!(md.contains("50.0%"));
}

// ── Report Generation: JSON round-trip ─────────────────────────────

#[test]
fn json_roundtrip_metrics() {
    let metrics = BddTraceabilityMetrics {
        total_ac: 42,
        ac_with_tests: 20,
        crate_coverage: vec![{
            let mut r = BddTraceabilityRow::new("flight-core");
            r.total_ac = 10;
            r
        }],
        ..Default::default()
    };

    let json = serde_json::to_string(&metrics).unwrap();
    let parsed: BddTraceabilityMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total_ac, 42);
    assert_eq!(parsed.ac_with_tests, 20);
    assert_eq!(parsed.crate_coverage.len(), 1);
    assert_eq!(parsed.crate_coverage[0].crate_name, "flight-core");
}

#[test]
fn json_roundtrip_row() {
    let mut row = BddTraceabilityRow::new("flight-axis");
    row.total_ac = 5;
    row.ac_with_gherkin = 3;

    let json = serde_json::to_string(&row).unwrap();
    let parsed: BddTraceabilityRow = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.crate_name, "flight-axis");
    assert_eq!(parsed.total_ac, 5);
    assert_eq!(parsed.ac_with_gherkin, 3);
}

#[test]
fn json_deserialize_empty_object_uses_defaults() {
    let parsed: BddTraceabilityMetrics = serde_json::from_str("{}").unwrap();
    assert_eq!(parsed.total_ac, 0);
    assert!(parsed.crate_coverage.is_empty());
}

// ── Crate Name Extraction ──────────────────────────────────────────

#[test]
fn extract_crates_from_command_package_eq_syntax() {
    let crates = extract_crates_from_command("cargo test --package=flight-hid");
    assert!(crates.contains("flight-hid"));
}

#[test]
fn extract_crates_from_command_manifest_path() {
    let crates =
        extract_crates_from_command("cargo test --manifest-path crates/flight-bus/Cargo.toml");
    assert!(crates.contains("flight-bus"));
}

#[test]
fn extract_crates_from_command_short_p_flag() {
    let crates = extract_crates_from_command("cargo test -pflight-ipc");
    assert!(crates.contains("flight-ipc"));
}

#[test]
fn extract_crates_from_reference_feature_prefix() {
    let crates = extract_crates_from_reference("feature:axis_processing.feature");
    assert!(crates.contains("specs"));
}

#[test]
fn extract_crates_from_command_xtask() {
    let crates = extract_crates_from_command("cargo xtask validate");
    assert!(crates.contains("xtask"));
}

#[test]
fn normalize_crate_name_handles_backslash_suffix() {
    assert_eq!(normalize_crate_name("flight_core\\"), "flight-core");
}

#[test]
fn is_crate_name_candidate_rejects_special_chars() {
    assert!(!is_crate_name_candidate("foo/bar"));
    assert!(!is_crate_name_candidate("hello world"));
    assert!(!is_crate_name_candidate("@scope/pkg"));
}

// ── with_workspace_crates ──────────────────────────────────────────

#[test]
fn with_workspace_crates_adds_missing_crates_with_zeros() {
    let metrics = BddTraceabilityMetrics::default();
    let updated = metrics.with_workspace_crates(["flight-core", "flight-axis"]);
    assert_eq!(updated.crate_coverage.len(), 2);
    assert!(updated.crate_coverage_for("flight-core").is_some());
    assert!(updated.crate_coverage_for("flight-axis").is_some());
    assert_eq!(
        updated.crate_coverage_for("flight-core").unwrap().total_ac,
        0
    );
}

#[test]
fn describe_microcrate_gaps_produces_expected_lines() {
    let row1 = BddTraceabilityRow {
        crate_name: "flight-core".into(),
        total_ac: 10,
        ac_with_gherkin: 3,
        ..Default::default()
    };
    let row2 = BddTraceabilityRow {
        crate_name: "flight-axis".into(),
        total_ac: 5,
        ac_with_gherkin: 5,
        ..Default::default()
    };

    let output = describe_microcrate_gaps(&[&row1, &row2]);
    assert!(output.contains("flight-core: 3/10 AC covered by Gherkin"));
    assert!(output.contains("flight-axis: 5/5 AC covered by Gherkin"));
}

// ── Metrics global coverage percent ────────────────────────────────

#[test]
fn metrics_global_coverage_percents() {
    let metrics = BddTraceabilityMetrics {
        total_ac: 20,
        ac_with_tests: 10,
        ac_with_gherkin: 15,
        ac_with_tests_and_gherkin: 5,
        ..Default::default()
    };

    assert!((metrics.test_coverage_percent() - 50.0).abs() < 0.01);
    assert!((metrics.gherkin_coverage_percent() - 75.0).abs() < 0.01);
    assert!((metrics.both_coverage_percent() - 25.0).abs() < 0.01);
}

#[test]
fn metrics_microcrate_coverage_percents() {
    let metrics = BddTraceabilityMetrics {
        microcrate_total: 4,
        microcrate_with_tests: 2,
        microcrate_with_gherkin: 3,
        microcrate_with_tests_and_gherkin: 1,
        ..Default::default()
    };

    assert!((metrics.microcrate_test_coverage_percent() - 50.0).abs() < 0.01);
    assert!((metrics.microcrate_gherkin_coverage_percent() - 75.0).abs() < 0.01);
    assert!((metrics.microcrate_full_coverage_percent() - 25.0).abs() < 0.01);
}

// ── is_scenario_header ─────────────────────────────────────────────

#[test]
fn is_scenario_header_rejects_steps() {
    assert!(!is_scenario_header("Given a setup"));
    assert!(!is_scenario_header("When something happens"));
    assert!(!is_scenario_header("Then it should work"));
    assert!(!is_scenario_header("And another thing"));
    assert!(!is_scenario_header("Background:"));
}

// ── Property-based tests ───────────────────────────────────────────

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn coverage_percent_never_negative(num in 0usize..1000, denom in 0usize..1000) {
            let pct = coverage_percent(num, denom);
            prop_assert!(pct >= 0.0, "coverage_percent({num}, {denom}) = {pct}");
        }

        #[test]
        fn coverage_percent_zero_denom_always_zero(num in 0usize..1000) {
            prop_assert_eq!(coverage_percent(num, 0), 0.0);
        }

        #[test]
        fn normalize_crate_name_idempotent_on_clean_names(
            name in "[a-z][a-z0-9-]{0,20}"
        ) {
            let once = normalize_crate_name(&name);
            let twice = normalize_crate_name(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn is_crate_name_candidate_accepts_normalized_valid_names(
            name in "[a-z][a-z0-9-]{0,20}"
        ) {
            prop_assert!(is_crate_name_candidate(&name));
        }

        #[test]
        fn parse_tags_from_line_always_strips_at(
            tags in prop::collection::vec("[A-Z][A-Z0-9-]{1,10}", 1..5)
        ) {
            let line = tags.iter().map(|t| format!("@{t}")).collect::<Vec<_>>().join(" ");
            let parsed = parse_tags_from_line(&line);
            for tag in &parsed {
                prop_assert!(!tag.starts_with('@'), "Tag still has @: {tag}");
            }
            prop_assert_eq!(parsed.len(), tags.len());
        }

        #[test]
        fn row_test_coverage_in_range(total in 1usize..100, tested in 0usize..100) {
            let tested = tested.min(total);
            let mut row = BddTraceabilityRow::new("test");
            row.total_ac = total;
            row.ac_with_tests = tested;
            let pct = row.test_coverage_percent();
            prop_assert!((0.0..=100.0).contains(&pct));
        }

        #[test]
        fn row_gherkin_coverage_in_range(total in 1usize..100, gherkin in 0usize..100) {
            let gherkin = gherkin.min(total);
            let mut row = BddTraceabilityRow::new("test");
            row.total_ac = total;
            row.ac_with_gherkin = gherkin;
            let pct = row.gherkin_coverage_percent();
            prop_assert!((0.0..=100.0).contains(&pct));
        }
    }
}
