// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Step definitions for BDD rollup coverage scenarios.

use anyhow::{Context, Result};
use cucumber::{given, then, when};
use flight_bdd_metrics::{
    BddTraceabilityMetrics, collect_bdd_traceability_metrics as compute_bdd_traceability_metrics,
    collect_gherkin_scenarios, describe_microcrate_gaps,
    extract_crates_from_command as extract_crates_from_command_impl, load_spec_ledger,
};
use flight_workspace_meta::load_workspace_microcrate_names;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::FlightWorld;

/// Absolute path to the `specs/` crate root (resolved at compile time).
fn specs_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Absolute path to the workspace root (parent of `specs/`).
fn workspace_root() -> PathBuf {
    specs_root()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

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

    assert!(
        metrics.total_ac > 0,
        "No acceptance criteria found in spec ledger"
    );
    assert_eq!(
        metrics.ac_with_gherkin, metrics.total_ac,
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

fn collect_bdd_traceability_metrics() -> Result<BddTraceabilityMetrics> {
    let ledger = load_spec_ledger(specs_root().join("spec_ledger.yaml"))
        .context("failed to read spec ledger")?;
    let scenarios = collect_gherkin_scenarios(specs_root().join("features"))
        .context("failed to parse feature scenarios")?;
    let metrics = compute_bdd_traceability_metrics(&ledger, &scenarios);

    match load_workspace_microcrate_names(workspace_root()) {
        Ok(workspace_crates) => Ok(metrics.with_workspace_crates(workspace_crates)),
        Err(_) => Ok(metrics),
    }
}

pub(crate) fn extract_crates_from_command(command: &str) -> BTreeSet<String> {
    extract_crates_from_command_impl(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    const PROJECT_INFRA_REQUIREMENTS_PATH: &str =
        ".kiro/specs/project-infrastructure/requirements.md";
    const PROJECT_INFRA_TASKS_PATH: &str = ".kiro/specs/project-infrastructure/tasks.md";

    #[test]
    fn test_extract_crates_from_command_includes_xtask() {
        let crates = extract_crates_from_command("cargo xtask validate");
        assert!(crates.contains("xtask"));
        assert_eq!(crates.len(), 1);
    }

    fn inf_req_7_task_section_lines() -> Vec<String> {
        let mut lines = Vec::new();
        let mut in_section = false;

        let content = fs::read_to_string(PROJECT_INFRA_TASKS_PATH)
            .expect("Failed to read project infrastructure tasks");
        for line in content.lines() {
            if line.contains("For INF-REQ-7 (Task-Driven Maintenance)") {
                in_section = true;
                continue;
            }

            if in_section {
                let trimmed = line.trim_start();
                if trimmed.starts_with("- For INF-REQ-") && !trimmed.contains("INF-REQ-7") {
                    break;
                }
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
        }

        assert!(
            !lines.is_empty(),
            "INF-REQ-7 task block should be present in {PROJECT_INFRA_TASKS_PATH}"
        );

        lines
    }

    fn inf_req_7_ac_numbers(lines: &[String]) -> Vec<u8> {
        let ac_re = Regex::new(r"AC-7\.([1-7])").expect("Invalid AC regex");
        let mut numbers = Vec::new();

        for line in lines {
            for cap in ac_re.captures_iter(line) {
                let number = cap[1].parse::<u8>().expect("AC number should parse");
                numbers.push(number);
            }
        }

        numbers
    }

    fn task_section_code_tokens(lines: &[String]) -> Vec<String> {
        let token_re = Regex::new(r#"`([^`]+)`"#).expect("Invalid token regex");
        let mut tokens = Vec::new();

        for line in lines {
            for capture in token_re.captures_iter(line) {
                tokens.push(capture[1].to_string());
            }
        }

        tokens
    }

    fn is_potential_path_token(token: &str) -> bool {
        token.contains(".md")
            || token.contains(".yml")
            || token.contains(".yaml")
            || token.contains(".toml")
            || token.contains(".rs")
            || token.contains("/")
            || token.contains("\\")
    }

    fn is_absolute_path_token(token: &str) -> bool {
        if token.starts_with('/') || token.starts_with('\\') {
            return true;
        }

        token.len() >= 2 && token.as_bytes()[1] == b':' && token.as_bytes()[0].is_ascii_alphabetic()
    }

    #[test]
    fn test_inf_req_7_1_task_templates_include_title_motivation_steps_and_acceptance() {
        let section_lines = inf_req_7_task_section_lines();
        let section_text = section_lines.join(" ").to_lowercase();

        assert!(
            section_text.contains("title"),
            "INF-REQ-7 task templates should include task titles"
        );
        assert!(
            section_text.contains("motivation"),
            "INF-REQ-7 task templates should include motivation"
        );
        assert!(
            section_text.contains("step"),
            "INF-REQ-7 task templates should include step bullets"
        );
        assert!(
            section_text.contains("acceptance"),
            "INF-REQ-7 AC-7.1 should be defined"
        );

        let requirements = fs::read_to_string(PROJECT_INFRA_REQUIREMENTS_PATH)
            .expect("Failed to read project infrastructure requirements");
        assert!(
            requirements.contains("Requirement 7: Task-Driven Maintenance Workflow"),
            "Project infrastructure requirements should include INF-REQ-7"
        );
        assert!(
            requirements.contains("WHEN defining maintenance tasks THEN they SHALL include title"),
            "INF-REQ-7 AC-7.1 should be defined"
        );
    }

    #[test]
    fn test_inf_req_7_2_task_file_references_are_workspace_relative() {
        let section_lines = inf_req_7_task_section_lines();
        let token_references = task_section_code_tokens(&section_lines)
            .into_iter()
            .filter(|token| is_potential_path_token(token))
            .collect::<Vec<_>>();

        for token in token_references {
            assert!(
                !is_absolute_path_token(&token),
                "INF-REQ-7 file reference should be workspace-relative, got absolute: {token}"
            );
        }
    }

    #[test]
    fn test_inf_req_7_3_validation_steps_are_explicit_commands() {
        let section_lines = inf_req_7_task_section_lines();
        let line_73 = section_lines
            .iter()
            .find(|line| line.contains("AC-7.3"))
            .expect("Missing AC-7.3 line in tasks section");

        let has_command = task_section_code_tokens(&[line_73.clone()])
            .into_iter()
            .any(|token| {
                token.starts_with("cargo ")
                    || token.starts_with("rg ")
                    || token.starts_with("rustc ")
            });

        assert!(
            has_command,
            "INF-REQ-7 AC-7.3 should specify exact validation commands"
        );
    }

    #[test]
    fn test_inf_req_7_4_task_execution_should_be_sequential() {
        let section_lines = inf_req_7_task_section_lines();
        let mut ac_numbers = inf_req_7_ac_numbers(&section_lines);
        ac_numbers.sort_unstable();
        ac_numbers.dedup();

        assert_eq!(
            ac_numbers,
            vec![1, 2, 3, 4, 5, 6, 7],
            "INF-REQ-7 should enumerate AC-7.1 through AC-7.7"
        );

        let section_text = section_lines.join(" ").to_lowercase();
        assert!(
            section_text.contains("1."),
            "INF-REQ-7 task execution should include ordered/sequential steps"
        );
    }

    #[test]
    fn test_inf_req_7_5_failure_handling_includes_diagnostics() {
        let section_lines = inf_req_7_task_section_lines();
        let section_text = section_lines.join(" ").to_lowercase();

        assert!(
            section_text.contains("diagnostic"),
            "INF-REQ-7 should include diagnostics guidance when tasks fail"
        );
        assert!(
            section_text.contains("suggest") || section_text.contains("corrective action"),
            "INF-REQ-7 should include suggested remediation actions"
        );
    }

    #[test]
    fn test_inf_req_7_6_new_needs_are_covered_as_tasks() {
        let section_lines = inf_req_7_task_section_lines();
        let section_text = section_lines.join(" ").to_lowercase();

        assert!(
            section_text.contains("new maintenance needs"),
            "INF-REQ-7 should encode new maintenance needs as tasks"
        );
        assert!(
            section_text.contains("tasks.md"),
            "INF-REQ-7 should reference where task entries are encoded"
        );
    }

    #[test]
    fn test_inf_req_7_7_acceptance_is_re_runnable() {
        let section_lines = inf_req_7_task_section_lines();
        let section_text = section_lines.join(" ").to_lowercase();

        assert!(
            section_text.contains("re-run"),
            "INF-REQ-7 should require acceptance commands to be re-run"
        );
        assert!(
            section_text.contains("acceptance"),
            "INF-REQ-7 should include acceptance command re-run coverage"
        );
    }
}
