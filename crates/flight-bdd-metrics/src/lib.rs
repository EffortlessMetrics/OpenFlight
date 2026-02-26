// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const UNMAPPED_MICROCRATE: &str = "unmapped";

/// BDD coverage matrix for all acceptance criteria in the spec ledger.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BddTraceabilityMetrics {
    /// Total acceptance criteria discovered in the ledger.
    pub total_ac: usize,
    /// ACs that have at least one linked test reference.
    pub ac_with_tests: usize,
    /// ACs linked by at least one Gherkin scenario.
    pub ac_with_gherkin: usize,
    /// ACs with both test and Gherkin traceability.
    pub ac_with_tests_and_gherkin: usize,
    /// ACs marked as fully complete.
    pub complete: usize,
    /// ACs marked as needing Gherkin.
    pub needs_gherkin: usize,
    /// ACs marked as needing tests.
    pub needs_tests: usize,
    /// ACs still marked as draft.
    pub draft: usize,
    /// ACs marked incomplete.
    pub incomplete: usize,
    /// Number of microcrates discovered in traces.
    pub microcrate_total: usize,
    /// Microcrates with full test coverage.
    pub microcrate_with_tests: usize,
    /// Microcrates with full Gherkin coverage.
    pub microcrate_with_gherkin: usize,
    /// Microcrates with full test+Gherkin coverage.
    pub microcrate_with_tests_and_gherkin: usize,
    /// Per-microcrate breakdown.
    pub crate_coverage: Vec<BddTraceabilityRow>,
}

/// BDD traceability metrics for one microcrate.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
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

impl BddTraceabilityRow {
    /// Construct a coverage row for a single microcrate.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            crate_name: name.into(),
            total_ac: 0,
            ac_with_tests: 0,
            ac_with_gherkin: 0,
            ac_with_tests_and_gherkin: 0,
            complete: 0,
            needs_gherkin: 0,
            needs_tests: 0,
            draft: 0,
            incomplete: 0,
        }
    }

    /// Percentage of mapped acceptance criteria covered by tests.
    pub fn test_coverage_percent(&self) -> f64 {
        coverage_percent(self.ac_with_tests, self.total_ac)
    }

    /// Percentage of mapped acceptance criteria covered by Gherkin.
    pub fn gherkin_coverage_percent(&self) -> f64 {
        coverage_percent(self.ac_with_gherkin, self.total_ac)
    }

    /// Percentage of mapped acceptance criteria with both test and Gherkin traceability.
    pub fn combined_coverage_percent(&self) -> f64 {
        coverage_percent(self.ac_with_tests_and_gherkin, self.total_ac)
    }

    /// Whether this microcrate is fully covered by tests.
    pub fn is_fully_tested(&self) -> bool {
        self.total_ac > 0 && self.ac_with_tests == self.total_ac
    }

    /// Whether this microcrate is fully covered by Gherkin scenarios.
    pub fn is_fully_gherkin_covered(&self) -> bool {
        self.total_ac > 0 && self.ac_with_gherkin == self.total_ac
    }

    /// Whether this microcrate is fully covered by both tests and Gherkin.
    pub fn is_fully_both_covered(&self) -> bool {
        self.total_ac > 0 && self.ac_with_tests_and_gherkin == self.total_ac
    }

    /// Whether this microcrate is the synthetic unmapped row.
    pub fn is_unmapped(&self) -> bool {
        self.crate_name == UNMAPPED_MICROCRATE
    }

    /// Row entry formatted for Markdown matrix output.
    pub fn to_markdown_row(&self) -> String {
        format!(
            "| {} | {} | {} | {} | {} | {} | {:.1}% | {:.1}% | {:.1}% |\n",
            self.crate_name,
            self.total_ac,
            self.ac_with_tests,
            self.ac_with_gherkin,
            self.ac_with_tests_and_gherkin,
            self.complete,
            self.test_coverage_percent(),
            self.gherkin_coverage_percent(),
            self.combined_coverage_percent()
        )
    }
}

impl BddTraceabilityMetrics {
    fn with_totals() -> Self {
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
            microcrate_total: 0,
            microcrate_with_tests: 0,
            microcrate_with_gherkin: 0,
            microcrate_with_tests_and_gherkin: 0,
            crate_coverage: Vec::new(),
        }
    }

    /// Add explicit rows for all workspace microcrates and recompute microcrate totals.
    ///
    /// This is used by callers that want to guarantee every workspace crate is represented in
    /// the matrix, even when it currently has zero mapped acceptance criteria.
    pub fn with_workspace_crates<I, S>(mut self, workspace_crates: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let workspace_crates: BTreeSet<String> =
            workspace_crates.into_iter().map(Into::into).collect();

        let mut rows: BTreeMap<String, BddTraceabilityRow> = self
            .crate_coverage
            .into_iter()
            .filter(|row| row.is_unmapped() || workspace_crates.contains(&row.crate_name))
            .map(|row| (row.crate_name.clone(), row))
            .collect();

        for crate_name in workspace_crates {
            rows.entry(crate_name.clone())
                .or_insert_with(|| BddTraceabilityRow::new(crate_name));
        }

        self.crate_coverage = rows.into_values().collect();
        self.recompute_microcrate_totals();
        self
    }

    fn recompute_microcrate_totals(&mut self) {
        self.microcrate_total = self.crate_coverage.len();
        self.microcrate_with_tests = self
            .crate_coverage
            .iter()
            .filter(|row| row.is_fully_tested())
            .count();
        self.microcrate_with_gherkin = self
            .crate_coverage
            .iter()
            .filter(|row| row.is_fully_gherkin_covered())
            .count();
        self.microcrate_with_tests_and_gherkin = self
            .crate_coverage
            .iter()
            .filter(|row| row.is_fully_both_covered())
            .count();
    }

    /// Whether the synthetic unmapped microcrate exists in the matrix.
    pub fn has_unmapped_microcrate(&self) -> bool {
        self.crate_coverage
            .iter()
            .any(BddTraceabilityRow::is_unmapped)
    }

    /// Find a single microcrate coverage row by name.
    pub fn crate_coverage_for(&self, crate_name: &str) -> Option<&BddTraceabilityRow> {
        self.crate_coverage
            .iter()
            .find(|row| row.crate_name == crate_name)
    }

    /// Microcrate coverage percentage for fully test-covered microcrates.
    pub fn microcrate_test_coverage_percent(&self) -> f64 {
        coverage_percent(self.microcrate_with_tests, self.microcrate_total)
    }

    /// Microcrate coverage percentage for fully Gherkin-covered microcrates.
    pub fn microcrate_gherkin_coverage_percent(&self) -> f64 {
        coverage_percent(self.microcrate_with_gherkin, self.microcrate_total)
    }

    /// Microcrate coverage percentage for microcrates fully covered by both.
    pub fn microcrate_full_coverage_percent(&self) -> f64 {
        coverage_percent(
            self.microcrate_with_tests_and_gherkin,
            self.microcrate_total,
        )
    }

    /// Render the metrics into a markdown block for report output.
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        output.push_str("## BDD Coverage Metrics\n\n");
        output.push_str("| Metric | Value |\n");
        output.push_str("|--------|-------|\n");
        output.push_str(&format!("| Total AC | {} |\n", self.total_ac));
        output.push_str(&format!("| ACs with tests | {} |\n", self.ac_with_tests));
        output.push_str(&format!(
            "| ACs with Gherkin | {} |\n",
            self.ac_with_gherkin
        ));
        output.push_str(&format!(
            "| ACs with both tests + Gherkin | {} |\n",
            self.ac_with_tests_and_gherkin
        ));
        output.push_str(&format!("| Complete | {} |\n", self.complete));
        output.push_str(&format!("| Needs Gherkin | {} |\n", self.needs_gherkin));
        output.push_str(&format!("| Needs Tests | {} |\n", self.needs_tests));
        output.push_str(&format!("| Draft | {} |\n", self.draft));
        output.push_str(&format!("| Incomplete | {} |\n", self.incomplete));
        output.push_str(&format!("| Microcrates | {} |\n", self.microcrate_total));
        output.push_str(&format!(
            "| Microcrates with tests | {} ({:.1}%) |\n",
            self.microcrate_with_tests,
            coverage_percent(self.microcrate_with_tests, self.microcrate_total),
        ));
        output.push_str(&format!(
            "| Microcrates with Gherkin | {} ({:.1}%) |\n",
            self.microcrate_with_gherkin,
            coverage_percent(self.microcrate_with_gherkin, self.microcrate_total),
        ));
        output.push_str(&format!(
            "| Microcrates fully covered | {} ({:.1}%) |\n",
            self.microcrate_with_tests_and_gherkin,
            coverage_percent(
                self.microcrate_with_tests_and_gherkin,
                self.microcrate_total
            ),
        ));
        output.push_str(&format!(
            "| Test coverage | {:.1}% |\n",
            coverage_percent(self.ac_with_tests, self.total_ac)
        ));
        output.push_str(&format!(
            "| Gherkin coverage | {:.1}% |\n",
            coverage_percent(self.ac_with_gherkin, self.total_ac)
        ));
        output.push_str(&format!(
            "| Test + Gherkin coverage | {:.1}% |\n\n",
            coverage_percent(self.ac_with_tests_and_gherkin, self.total_ac)
        ));

        output.push_str("## BDD Microcrate Matrix\n\n");
        if self.crate_coverage.is_empty() {
            output.push_str("No microcrate test mappings discovered yet.\n\n");
            return output;
        }

        output.push_str("| Microcrate | Total AC | ACs with tests | ACs with Gherkin | ACs with both | Complete | Test coverage | Gherkin coverage | Test+Gherkin coverage |\n");
        output.push_str("|-----------|----------|----------------|------------------|---------------|----------|--------------|------------------|------------------------|\n");

        for row in &self.crate_coverage {
            output.push_str(&row.to_markdown_row());
        }

        output.push('\n');

        output
    }

    /// Percentage of ACs that have at least one test reference.
    pub fn test_coverage_percent(&self) -> f64 {
        coverage_percent(self.ac_with_tests, self.total_ac)
    }

    /// Percentage of ACs that have a Gherkin scenario reference.
    pub fn gherkin_coverage_percent(&self) -> f64 {
        coverage_percent(self.ac_with_gherkin, self.total_ac)
    }

    /// Percentage of ACs that have both test and Gherkin references.
    pub fn both_coverage_percent(&self) -> f64 {
        coverage_percent(self.ac_with_tests_and_gherkin, self.total_ac)
    }
}

/// Spec ledger model for acceptance criteria and tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecLedger {
    pub requirements: Vec<SpecRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecRequirement {
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
    pub status: RequirementStatus,
    pub ac: Vec<AcceptanceCriteria>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequirementStatus {
    Draft,
    Implemented,
    Tested,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriteria {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub tests: Vec<serde_yaml::Value>,
}

/// Parsed Gherkin scenario metadata.
#[derive(Debug, Clone)]
pub struct BddScenario {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub name: String,
    pub tags: Vec<String>,
}

impl BddScenario {
    pub fn ac_tags(&self) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|tag| tag.starts_with("AC-"))
            .map(String::as_str)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageStatus {
    Complete,
    NeedsGherkin,
    NeedsTests,
    Draft,
    Incomplete,
}

impl CoverageStatus {
    /// Compute AC coverage status from source status and traceability flags.
    pub fn compute(status: &RequirementStatus, has_tests: bool, has_gherkin: bool) -> Self {
        match (status, has_tests, has_gherkin) {
            (RequirementStatus::Tested, true, true) => Self::Complete,
            (RequirementStatus::Implemented, true, true) => Self::Complete,
            (RequirementStatus::Implemented, true, false) => Self::NeedsGherkin,
            (RequirementStatus::Implemented, false, _) => Self::NeedsTests,
            (RequirementStatus::Draft, _, _) => Self::Draft,
            _ => Self::Incomplete,
        }
    }

    /// Status icon for report rendering.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Complete => "✅",
            Self::NeedsGherkin => "🟡",
            Self::NeedsTests => "🟡",
            Self::Draft => "⚪",
            Self::Incomplete => "❌",
        }
    }

    /// Human-readable status label for report rendering.
    pub fn text(&self) -> &'static str {
        match self {
            Self::Complete => "Complete",
            Self::NeedsGherkin => "Needs Gherkin",
            Self::NeedsTests => "Needs Tests",
            Self::Draft => "Draft",
            Self::Incomplete => "Incomplete",
        }
    }
}

/// Compute percentage with safe integer division for empty denominators.
pub fn coverage_percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }

    (numerator as f64 / denominator as f64) * 100.0
}

/// Load the spec ledger file.
pub fn load_spec_ledger(path: impl AsRef<Path>) -> Result<SpecLedger> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read spec ledger at {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse spec ledger at {}", path.display()))
}

/// Parse feature files and return scenarios with merged feature/scenario tags.
pub fn collect_gherkin_scenarios(path: impl AsRef<Path>) -> Result<Vec<BddScenario>> {
    let feature_dir = path.as_ref();
    let mut scenarios = Vec::new();

    if !feature_dir.exists() {
        return Ok(scenarios);
    }

    for entry in WalkDir::new(feature_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("feature") {
            continue;
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let mut feature_tags = Vec::new();
        let mut local_tags = Vec::new();

        for (idx, line) in content.lines().enumerate() {
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
                let name = if trimmed.starts_with("Scenario:") {
                    trimmed.trim_start_matches("Scenario:").trim().to_string()
                } else {
                    trimmed
                        .trim_start_matches("Scenario Outline:")
                        .trim()
                        .to_string()
                };

                let mut tags = feature_tags.clone();
                tags.extend(local_tags.clone());

                scenarios.push(BddScenario {
                    file_path: path.to_path_buf(),
                    line_number: idx + 1,
                    name,
                    tags,
                });

                local_tags.clear();
            }
        }
    }

    Ok(scenarios)
}

/// Compute acceptance criteria traceability metrics.
pub fn collect_bdd_traceability_metrics(
    ledger: &SpecLedger,
    scenarios: &[BddScenario],
) -> BddTraceabilityMetrics {
    let mut ac_to_gherkin: HashSet<String> = HashSet::new();
    for scenario in scenarios {
        ac_to_gherkin.extend(scenario.ac_tags().into_iter().map(ToString::to_string));
    }

    let mut metrics = BddTraceabilityMetrics::with_totals();
    let mut matrix: BTreeMap<String, BddTraceabilityRow> = BTreeMap::new();

    for requirement in &ledger.requirements {
        for ac in &requirement.ac {
            let has_tests = !ac.tests.is_empty();
            let has_gherkin = ac_to_gherkin.contains(&ac.id);

            metrics.total_ac += 1;
            if has_tests {
                metrics.ac_with_tests += 1;
            }
            if has_gherkin {
                metrics.ac_with_gherkin += 1;
            }
            if has_tests && has_gherkin {
                metrics.ac_with_tests_and_gherkin += 1;
            }

            match CoverageStatus::compute(&requirement.status, has_tests, has_gherkin) {
                CoverageStatus::Complete => metrics.complete += 1,
                CoverageStatus::NeedsGherkin => metrics.needs_gherkin += 1,
                CoverageStatus::NeedsTests => metrics.needs_tests += 1,
                CoverageStatus::Draft => metrics.draft += 1,
                CoverageStatus::Incomplete => metrics.incomplete += 1,
            }

            let crate_names = {
                let extracted = collect_crate_names_for_tests(&ac.tests);
                if extracted.is_empty() {
                    vec![UNMAPPED_MICROCRATE.to_string()]
                } else {
                    extracted
                }
            };

            for crate_name in crate_names {
                let row = matrix
                    .entry(crate_name.clone())
                    .or_insert_with(|| BddTraceabilityRow::new(crate_name.clone()));

                row.total_ac += 1;
                if has_tests {
                    row.ac_with_tests += 1;
                }
                if has_gherkin {
                    row.ac_with_gherkin += 1;
                }
                if has_tests && has_gherkin {
                    row.ac_with_tests_and_gherkin += 1;
                }

                match CoverageStatus::compute(&requirement.status, has_tests, has_gherkin) {
                    CoverageStatus::Complete => row.complete += 1,
                    CoverageStatus::NeedsGherkin => row.needs_gherkin += 1,
                    CoverageStatus::NeedsTests => row.needs_tests += 1,
                    CoverageStatus::Draft => row.draft += 1,
                    CoverageStatus::Incomplete => row.incomplete += 1,
                }
            }
        }
    }

    metrics.crate_coverage = matrix.into_values().collect();
    metrics.recompute_microcrate_totals();

    metrics
}

pub fn describe_microcrate_gaps(rows: &[&BddTraceabilityRow]) -> String {
    let mut details = String::new();
    for row in rows {
        let _ = std::fmt::Write::write_fmt(
            &mut details,
            format_args!(
                "{}: {}/{} AC covered by Gherkin\n",
                row.crate_name, row.ac_with_gherkin, row.total_ac
            ),
        );
    }

    details
}

pub fn parse_tags_from_line(line: &str) -> Vec<String> {
    line.split_whitespace()
        .filter(|token| token.starts_with('@'))
        .map(|token| token.trim_start_matches('@').to_string())
        .collect()
}

pub fn is_scenario_header(line: &str) -> bool {
    line.starts_with("Scenario:") || line.starts_with("Scenario Outline:")
}

pub fn extract_crates_from_reference(reference: &str) -> BTreeSet<String> {
    let mut crates = BTreeSet::new();

    if let Some(command) = reference.strip_prefix("cmd:") {
        crates.extend(extract_crates_from_command(command));
        return crates;
    }

    if reference.starts_with("feature:") {
        crates.insert("specs".to_string());
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

pub fn extract_crates_from_command(command: &str) -> BTreeSet<String> {
    let mut crates = BTreeSet::new();
    let mut tokens = command.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        if token == "cargo" {
            continue;
        }

        if token == "xtask" {
            crates.insert("xtask".to_string());
            continue;
        }

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

        if token == "--manifest-path" {
            if let Some(path) = tokens.next()
                && let Some(crate_name) = extract_crate_from_manifest_path(path)
            {
                crates.insert(crate_name);
            }
            continue;
        }

        if let Some(path) = token.strip_prefix("--manifest-path=") {
            if let Some(crate_name) = extract_crate_from_manifest_path(path) {
                crates.insert(crate_name);
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

pub fn collect_crate_names_for_tests(test_references: &[serde_yaml::Value]) -> Vec<String> {
    test_references
        .iter()
        .fold(BTreeSet::new(), |mut crates, test_ref| {
            match test_ref {
                serde_yaml::Value::String(text) => {
                    crates.extend(extract_crates_from_reference(text))
                }
                serde_yaml::Value::Mapping(mapping) => {
                    for (key, value) in mapping {
                        if is_test_reference_key(key.as_str())
                            && let Some(reference) = value.as_str()
                        {
                            crates.extend(extract_crates_from_reference(reference));
                        }
                    }
                }
                _ => {}
            }
            crates
        })
        .into_iter()
        .collect()
}

fn is_test_reference_key(value: Option<&str>) -> bool {
    matches!(value, Some("test") | Some("command") | Some("feature"))
}

fn extract_crate_from_manifest_path(path: &str) -> Option<String> {
    let manifest_path = Path::new(
        path.trim_matches(&['\'', '"', '`'][..])
            .trim_end_matches("\\"),
    );
    let manifest_dir =
        if manifest_path.file_name().and_then(|value| value.to_str()) == Some("Cargo.toml") {
            manifest_path.parent().unwrap_or(manifest_path)
        } else {
            manifest_path
        };

    let crate_name = manifest_dir.file_name().and_then(|value| value.to_str())?;
    let normalized = normalize_crate_name(crate_name);

    if is_crate_name_candidate(&normalized) {
        Some(normalized)
    } else {
        None
    }
}

pub fn normalize_crate_name(crate_name: &str) -> String {
    crate_name
        .trim_matches(&['\'', '"', '`'][..])
        .trim_end_matches("\\")
        .trim()
        .replace('_', "-")
}

pub fn is_crate_name_candidate(crate_name: &str) -> bool {
    if crate_name.is_empty() {
        return false;
    }

    if !crate_name
        .chars()
        .next()
        .unwrap_or('_')
        .is_ascii_alphabetic()
    {
        return false;
    }

    crate_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(
        crate_name: &str,
        total_ac: usize,
        ac_with_tests: usize,
        ac_with_gherkin: usize,
        ac_with_tests_and_gherkin: usize,
    ) -> BddTraceabilityRow {
        BddTraceabilityRow {
            crate_name: crate_name.to_string(),
            total_ac,
            ac_with_tests,
            ac_with_gherkin,
            ac_with_tests_and_gherkin,
            complete: 0,
            needs_gherkin: 0,
            needs_tests: 0,
            draft: 0,
            incomplete: 0,
        }
    }

    #[test]
    fn with_workspace_crates_filters_non_workspace_rows() {
        let metrics = BddTraceabilityMetrics {
            crate_coverage: vec![
                sample_row("flight-core", 2, 2, 1, 1),
                sample_row("xtask", 5, 5, 5, 5),
                sample_row("specs", 1, 1, 1, 1),
            ],
            ..Default::default()
        };

        let metrics = metrics.with_workspace_crates(["flight-core", "flight-ipc"]);

        let names: Vec<&str> = metrics
            .crate_coverage
            .iter()
            .map(|row| row.crate_name.as_str())
            .collect();
        assert_eq!(names, vec!["flight-core", "flight-ipc"]);
        assert_eq!(metrics.microcrate_total, 2);
        assert_eq!(metrics.microcrate_with_tests, 1);
        assert_eq!(metrics.microcrate_with_gherkin, 0);
        assert_eq!(metrics.microcrate_with_tests_and_gherkin, 0);
    }

    #[test]
    fn with_workspace_crates_preserves_unmapped_row() {
        let metrics = BddTraceabilityMetrics {
            crate_coverage: vec![
                sample_row(UNMAPPED_MICROCRATE, 3, 1, 1, 1),
                sample_row("xtask", 2, 2, 2, 2),
            ],
            ..Default::default()
        };

        let metrics = metrics.with_workspace_crates(["flight-core"]);

        let names: Vec<&str> = metrics
            .crate_coverage
            .iter()
            .map(|row| row.crate_name.as_str())
            .collect();
        assert_eq!(names, vec!["flight-core", UNMAPPED_MICROCRATE]);
        assert!(metrics.has_unmapped_microcrate());
        let unmapped = metrics
            .crate_coverage_for(UNMAPPED_MICROCRATE)
            .expect("Expected unmapped row");
        assert_eq!(unmapped.total_ac, 3);
        assert_eq!(metrics.microcrate_total, 2);
    }

    #[test]
    fn coverage_percent_zero_denominator_returns_zero() {
        assert_eq!(coverage_percent(5, 0), 0.0);
        assert_eq!(coverage_percent(0, 0), 0.0);
    }

    #[test]
    fn coverage_percent_full_coverage() {
        assert!((coverage_percent(10, 10) - 100.0).abs() < 0.01);
        assert!((coverage_percent(1, 4) - 25.0).abs() < 0.01);
    }

    #[test]
    fn is_crate_name_candidate_valid_and_invalid() {
        assert!(is_crate_name_candidate("flight-core"));
        assert!(is_crate_name_candidate("xtask"));
        assert!(!is_crate_name_candidate(""));
        assert!(!is_crate_name_candidate("1invalid"));
        assert!(!is_crate_name_candidate("-starts-with-dash"));
    }

    #[test]
    fn normalize_crate_name_strips_quotes_and_underscores() {
        assert_eq!(normalize_crate_name("\"flight_core\""), "flight-core");
        assert_eq!(normalize_crate_name("flight_axis"), "flight-axis");
        assert_eq!(normalize_crate_name("`xtask`"), "xtask");
    }

    #[test]
    fn parse_tags_from_line_extracts_tags() {
        let tags = parse_tags_from_line("@AC-001 @AC-002 @smoke");
        assert_eq!(tags, vec!["AC-001", "AC-002", "smoke"]);

        let no_tags = parse_tags_from_line("Scenario: something");
        assert!(no_tags.is_empty());
    }

    #[test]
    fn is_scenario_header_recognises_scenario_types() {
        assert!(is_scenario_header("Scenario: do something"));
        assert!(is_scenario_header("Scenario Outline: parameterized test"));
        assert!(!is_scenario_header("Feature: some feature"));
        assert!(!is_scenario_header("Given something happens"));
    }

    #[test]
    fn extract_crates_from_command_parses_p_flag() {
        let crates = extract_crates_from_command("cargo test -p flight-axis --lib");
        assert!(crates.contains("flight-axis"));

        let crates2 = extract_crates_from_command("cargo test --package flight-bus");
        assert!(crates2.contains("flight-bus"));
    }

    #[test]
    fn extract_crates_from_reference_cmd_prefix() {
        let crates = extract_crates_from_reference("cmd:cargo test -p flight-rules --lib");
        assert!(crates.contains("flight-rules"), "{crates:?}");
    }

    #[test]
    fn extract_crates_from_reference_double_colon_notation() {
        let crates = extract_crates_from_reference("flight_core::ProfileManager");
        assert!(crates.contains("flight-core"), "{crates:?}");
    }

    #[test]
    fn coverage_status_compute_all_branches() {
        use RequirementStatus::*;
        assert_eq!(
            CoverageStatus::compute(&Tested, true, true),
            CoverageStatus::Complete
        );
        assert_eq!(
            CoverageStatus::compute(&Implemented, true, false),
            CoverageStatus::NeedsGherkin
        );
        assert_eq!(
            CoverageStatus::compute(&Implemented, false, false),
            CoverageStatus::NeedsTests
        );
        assert_eq!(
            CoverageStatus::compute(&Draft, false, false),
            CoverageStatus::Draft
        );
        // Deprecated with tests but no Gherkin → Incomplete
        assert_eq!(
            CoverageStatus::compute(&Deprecated, false, false),
            CoverageStatus::Incomplete
        );
    }

    #[test]
    fn bdd_traceability_row_coverage_methods() {
        let row = sample_row("flight-axis", 4, 4, 2, 2);
        assert!(row.is_fully_tested());
        assert!(!row.is_fully_gherkin_covered());
        assert!(!row.is_fully_both_covered());
        assert!((row.test_coverage_percent() - 100.0).abs() < 0.01);
        assert!((row.gherkin_coverage_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn bdd_scenario_ac_tags_filters_non_ac_tags() {
        let scenario = BddScenario {
            file_path: std::path::PathBuf::from("test.feature"),
            line_number: 1,
            name: "test scenario".to_string(),
            tags: vec![
                "AC-001".to_string(),
                "smoke".to_string(),
                "AC-003".to_string(),
            ],
        };
        let ac_tags = scenario.ac_tags();
        assert_eq!(ac_tags, vec!["AC-001", "AC-003"]);
    }
}
