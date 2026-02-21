// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Step definitions for BDD rollup coverage scenarios.

use anyhow::{Context, Result};
use cucumber::{given, then, when};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::FlightWorld;

#[given("the implementation is represented by existing unit-test evidence")]
async fn given_unit_test_evidence(world: &mut FlightWorld) {
    world.bdd_traceability = Some(
        collect_bdd_traceability_metrics().expect("failed to compute BDD traceability metrics"),
    );
}

#[when("the acceptance criteria are reviewed end-to-end")]
async fn when_criteria_reviewed(world: &mut FlightWorld) {
    // Recompute to ensure this scenario validates the current repository state.
    world.bdd_traceability = Some(
        collect_bdd_traceability_metrics().expect("failed to recompute BDD traceability metrics"),
    );
}

#[then("the requirement SHALL be traceable through tests and BDD scenarios")]
async fn then_criteria_traceability(world: &mut FlightWorld) {
    let metrics = world
        .bdd_traceability
        .as_ref()
        .expect("BDD traceability metrics were not computed");

    assert!(metrics.total_ac > 0, "No acceptance criteria found in spec ledger");
    assert_eq!(
        metrics.ac_with_gherkin,
        metrics.total_ac,
        "Not all acceptance criteria are covered by Gherkin"
    );

    let missing = metrics
        .crate_coverage
        .iter()
        .filter(|row| row.total_ac > 0 && row.ac_with_gherkin < row.total_ac)
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "Microcrate(s) with incomplete Gherkin traceability: {}",
        describe_microcrate_gaps(&missing)
    );
}

#[derive(Debug, Clone, Serialize)]
pub struct BddTraceabilityMetrics {
    pub total_ac: usize,
    pub ac_with_tests: usize,
    pub ac_with_gherkin: usize,
    pub ac_with_tests_and_gherkin: usize,
    pub complete: usize,
    pub needs_gherkin: usize,
    pub needs_tests: usize,
    pub draft: usize,
    pub incomplete: usize,
    pub crate_coverage: Vec<BddTraceabilityRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BddTraceabilityRow {
    pub crate_name: String,
    pub total_ac: usize,
    pub ac_with_tests: usize,
    pub ac_with_gherkin: usize,
    pub ac_with_tests_and_gherkin: usize,
    pub complete: usize,
    pub needs_gherkin: usize,
    pub needs_tests: usize,
    pub draft: usize,
    pub incomplete: usize,
}

impl Default for BddTraceabilityMetrics {
    fn default() -> Self {
        Self {
            total_ac: 0,
            ac_with_tests: 0,
            ac_with_gherkin: 0,
            ac_with_tests_and_gherkin: 0,
            complete: 0,
            needs_gherkin: 0,
            needs_tests: 0,
            draft: 0,
            incomplete: 0,
            crate_coverage: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct BddScenario {
    tags: Vec<String>,
}

impl BddScenario {
    fn ac_tags(&self) -> Vec<&str> {
        self.tags.iter().filter(|tag| tag.starts_with("AC-")).map(String::as_str).collect()
    }
}

fn describe_microcrate_gaps(rows: &[&BddTraceabilityRow]) -> String {
    let mut details = String::new();

    for row in rows {
        let _ = writeln!(
            &mut details,
            "{}: {}/{} AC covered by Gherkin",
            row.crate_name,
            row.ac_with_gherkin,
            row.total_ac
        );
    }

    details
}

fn collect_bdd_traceability_metrics() -> Result<BddTraceabilityMetrics> {
    let ledger = load_spec_ledger(Path::new("specs/spec_ledger.yaml"))?;
    let scenarios = collect_gherkin_scenarios(Path::new("specs/features"))?;

    let mut metrics = BddTraceabilityMetrics::default();
    let mut ac_with_gherkin: HashSet<String> = HashSet::new();
    for scenario in &scenarios {
        ac_with_gherkin.extend(scenario.ac_tags().into_iter().map(ToString::to_string));
    }

    let mut crate_coverage: BTreeMap<String, BddTraceabilityRow> = BTreeMap::new();

    for requirement in &ledger.requirements {
        for ac in &requirement.ac {
            let has_tests = !ac.tests.is_empty();
            let covered_by_gherkin = ac_with_gherkin.contains(&ac.id);
            let status = compute_ac_status(&requirement.status, has_tests, covered_by_gherkin);

            metrics.total_ac += 1;
            if has_tests {
                metrics.ac_with_tests += 1;
            }
            if covered_by_gherkin {
                metrics.ac_with_gherkin += 1;
            }
            if has_tests && covered_by_gherkin {
                metrics.ac_with_tests_and_gherkin += 1;
            }
            match status {
                CoverageStatus::Complete => metrics.complete += 1,
                CoverageStatus::NeedsGherkin => metrics.needs_gherkin += 1,
                CoverageStatus::NeedsTests => metrics.needs_tests += 1,
                CoverageStatus::Draft => metrics.draft += 1,
                CoverageStatus::Incomplete => metrics.incomplete += 1,
            }

            for crate_name in extract_crates_from_test_references(&ac.tests) {
                let row = crate_coverage
                    .entry(crate_name.clone())
                    .or_insert_with(|| BddTraceabilityRow {
                        crate_name,
                        total_ac: 0,
                        ac_with_tests: 0,
                        ac_with_gherkin: 0,
                        ac_with_tests_and_gherkin: 0,
                        complete: 0,
                        needs_gherkin: 0,
                        needs_tests: 0,
                        draft: 0,
                        incomplete: 0,
                    });

                row.total_ac += 1;
                if has_tests {
                    row.ac_with_tests += 1;
                }
                if covered_by_gherkin {
                    row.ac_with_gherkin += 1;
                }
                if has_tests && covered_by_gherkin {
                    row.ac_with_tests_and_gherkin += 1;
                }
                match status {
                    CoverageStatus::Complete => row.complete += 1,
                    CoverageStatus::NeedsGherkin => row.needs_gherkin += 1,
                    CoverageStatus::NeedsTests => row.needs_tests += 1,
                    CoverageStatus::Draft => row.draft += 1,
                    CoverageStatus::Incomplete => row.incomplete += 1,
                }
            }
        }
    }

    metrics.crate_coverage = crate_coverage.into_values().collect();
    Ok(metrics)
}

fn load_spec_ledger(path: &Path) -> Result<SpecLedger> {
    let content = fs::read_to_string(path).context("failed to read spec ledger")?;
    serde_yaml::from_str(&content).context("failed to parse spec ledger")
}

fn collect_gherkin_scenarios(path: &Path) -> Result<Vec<BddScenario>> {
    let mut scenarios = Vec::new();
    if !path.exists() {
        return Ok(scenarios);
    }

    let mut feature_tags = Vec::new();
    let mut local_tags = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("feature") {
            continue;
        }

        let content = fs::read_to_string(entry.path())
            .with_context(|| format!("failed to read {}", entry.path().display()))?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with('@') {
                local_tags.extend(parse_tags_from_line(trimmed));
                continue;
            }

            if trimmed.starts_with("Feature:") {
                feature_tags = local_tags.clone();
                local_tags.clear();
                continue;
            }

            if is_scenario_header(trimmed) {
                let mut tags = feature_tags.clone();
                tags.extend(local_tags);
                if !tags.is_empty() {
                    scenarios.push(BddScenario { tags });
                }
                local_tags = Vec::new();
            }
        }
    }

    Ok(scenarios)
}

fn is_scenario_header(line: &str) -> bool {
    line.starts_with("Scenario:") || line.starts_with("Scenario Outline:")
}

fn parse_tags_from_line(line: &str) -> Vec<String> {
    line.split_whitespace()
        .filter_map(|token| token.strip_prefix('@'))
        .map(str::to_string)
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct SpecLedger {
    pub requirements: Vec<SpecRequirement>,
}

#[derive(Debug, Deserialize)]
struct SpecRequirement {
    #[allow(dead_code)]
    pub id: String,
    pub status: RequirementStatus,
    pub ac: Vec<AcceptanceCriteria>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RequirementStatus {
    Draft,
    Implemented,
    Tested,
    Deprecated,
}

#[derive(Debug, Deserialize)]
struct AcceptanceCriteria {
    pub id: String,
    #[serde(default)]
    pub tests: Vec<serde_yaml::Value>,
}

enum CoverageStatus {
    Complete,
    NeedsGherkin,
    NeedsTests,
    Draft,
    Incomplete,
}

fn compute_ac_status(
    status: &RequirementStatus,
    has_tests: bool,
    has_gherkin: bool,
) -> CoverageStatus {
    match (status, has_tests, has_gherkin) {
        (RequirementStatus::Tested, true, true) => CoverageStatus::Complete,
        (RequirementStatus::Implemented, true, true) => CoverageStatus::Complete,
        (RequirementStatus::Implemented, true, false) => CoverageStatus::NeedsGherkin,
        (RequirementStatus::Implemented, false, _) => CoverageStatus::NeedsTests,
        (RequirementStatus::Draft, _, _) => CoverageStatus::Draft,
        _ => CoverageStatus::Incomplete,
    }
}

fn extract_crates_from_test_references(tests: &[serde_yaml::Value]) -> BTreeSet<String> {
    let mut crates = BTreeSet::new();

    for test_ref in tests {
        match test_ref {
            serde_yaml::Value::String(text) => {
                crates.extend(extract_crates_from_reference(text));
            }
            serde_yaml::Value::Mapping(mapping) => {
                for (key, value) in mapping {
                    if is_test_reference_key(key.as_str()) {
                        if let Some(reference) = value.as_str() {
                            crates.extend(extract_crates_from_reference(reference));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    crates
}

fn is_test_reference_key(value: Option<&str>) -> bool {
    matches!(value, Some("test") | Some("command") | Some("feature"))
}

fn extract_crates_from_reference(reference: &str) -> BTreeSet<String> {
    let mut crates = BTreeSet::new();

    if let Some(command) = reference.strip_prefix("cmd:") {
        crates.extend(extract_crates_from_command(command));
        return crates;
    }

    if reference.starts_with("feature:") {
        return crates;
    }

    if reference.contains("::") {
        if let Some(root) = reference.split("::").next() {
            let normalized = normalize_crate_name(root);
            if is_crate_name_candidate(&normalized) {
                crates.insert(normalized);
            }
        }
        return crates;
    }

    crates.extend(extract_crates_from_command(reference));
    crates
}

fn extract_crates_from_command(command: &str) -> BTreeSet<String> {
    let mut crates = BTreeSet::new();
    let mut tokens = command.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        if token == "-p" || token == "--package" {
            if let Some(value) = tokens.next() {
                let normalized = normalize_crate_name(value);
                if is_crate_name_candidate(&normalized) {
                    crates.insert(normalized);
                }
            }
            continue;
        }

        if let Some(value) = token.strip_prefix("--package=") {
            let normalized = normalize_crate_name(value);
            if is_crate_name_candidate(&normalized) {
                crates.insert(normalized);
            }
            continue;
        }

        if token.starts_with("-p") && token.len() > 2 && !token.starts_with("-package") {
            let value = token.trim_start_matches("-p");
            let normalized = normalize_crate_name(value);
            if is_crate_name_candidate(&normalized) {
                crates.insert(normalized);
            }
        }
    }

    crates
}

fn normalize_crate_name(crate_name: &str) -> String {
    crate_name
        .trim_matches(&['"', '\'', '`'][..])
        .trim_end_matches("\\")
        .trim()
        .replace('_', "-")
}

fn is_crate_name_candidate(crate_name: &str) -> bool {
    if crate_name.is_empty() {
        return false;
    }

    if !crate_name.chars().next().unwrap_or('_').is_ascii_alphabetic() {
        return false;
    }

    crate_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}
