// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Scenario runner that executes parsed BDD scenarios against a [`StepRegistry`].

use crate::step_registry::{StepContext, StepOutcome, StepRegistry};

/// Keyword classifying a Gherkin step line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKeyword {
    Given,
    When,
    Then,
}

/// A single step in a scenario.
#[derive(Debug, Clone)]
pub struct Step {
    pub keyword: StepKeyword,
    pub text: String,
}

/// A BDD scenario consisting of ordered steps.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: String,
    pub steps: Vec<Step>,
}

/// Outcome of running one step within a scenario.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub keyword: StepKeyword,
    pub text: String,
    pub outcome: StepOutcome,
}

/// Aggregate result of running a full scenario.
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    pub name: String,
    pub status: ScenarioStatus,
    pub step_results: Vec<StepResult>,
}

/// High-level scenario status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioStatus {
    Passed,
    Failed,
    Skipped,
}

impl ScenarioResult {
    pub fn is_passed(&self) -> bool {
        self.status == ScenarioStatus::Passed
    }

    /// Return all failed steps.
    pub fn failures(&self) -> Vec<&StepResult> {
        self.step_results
            .iter()
            .filter(|r| r.outcome.is_failed())
            .collect()
    }
}

/// Execute a scenario against the given step registry.
///
/// Steps are executed in order. If any step fails, subsequent steps in the same
/// scenario are skipped. A fresh [`StepContext`] is created per scenario so that
/// state does not leak between scenarios.
pub fn run_scenario(scenario: &Scenario, registry: &StepRegistry) -> ScenarioResult {
    let ctx = StepContext::new();
    let mut step_results = Vec::with_capacity(scenario.steps.len());
    let mut failed = false;

    for step in &scenario.steps {
        if failed {
            step_results.push(StepResult {
                keyword: step.keyword,
                text: step.text.clone(),
                outcome: StepOutcome::Skipped("previous step failed".to_string()),
            });
            continue;
        }

        let outcome = execute_step(step, registry, &ctx);
        if outcome.is_failed() {
            failed = true;
        }
        step_results.push(StepResult {
            keyword: step.keyword,
            text: step.text.clone(),
            outcome,
        });
    }

    let status = if step_results.is_empty() {
        ScenarioStatus::Failed
    } else if failed {
        ScenarioStatus::Failed
    } else if step_results.iter().all(|r| r.outcome.is_passed()) {
        ScenarioStatus::Passed
    } else {
        ScenarioStatus::Skipped
    };

    ScenarioResult {
        name: scenario.name.clone(),
        status,
        step_results,
    }
}

/// Execute all scenarios, returning results for each.
pub fn run_scenarios(scenarios: &[Scenario], registry: &StepRegistry) -> Vec<ScenarioResult> {
    scenarios
        .iter()
        .map(|s| run_scenario(s, registry))
        .collect()
}

fn execute_step(step: &Step, registry: &StepRegistry, ctx: &StepContext) -> StepOutcome {
    let matcher = match step.keyword {
        StepKeyword::Given => registry.match_given(&step.text),
        StepKeyword::When => registry.match_when(&step.text),
        StepKeyword::Then => registry.match_then(&step.text),
    };

    match matcher {
        Some((handler, caps)) => handler(ctx, &caps),
        None => StepOutcome::Failed(format!(
            "no matching {:?} step definition for: {}",
            step.keyword, step.text
        )),
    }
}

/// Parse a minimal Gherkin-like text block into a [`Scenario`].
///
/// Recognises lines starting with `Given`, `When`, `Then`, `And`, `But`.
/// `And`/`But` inherit the keyword of the preceding step.
///
/// Returns an error if any non-blank, non-comment line is not a recognised
/// Gherkin keyword.
pub fn parse_scenario(name: &str, text: &str) -> Result<Scenario, String> {
    let mut steps = Vec::new();
    let mut last_keyword: Option<StepKeyword> = None;

    for (line_no, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("Scenario") {
            continue;
        }

        let (keyword, rest) = if let Some(rest) = trimmed.strip_prefix("Given ") {
            (StepKeyword::Given, rest)
        } else if let Some(rest) = trimmed.strip_prefix("When ") {
            (StepKeyword::When, rest)
        } else if let Some(rest) = trimmed.strip_prefix("Then ") {
            (StepKeyword::Then, rest)
        } else if let Some(rest) = trimmed.strip_prefix("And ") {
            (last_keyword.unwrap_or(StepKeyword::Given), rest)
        } else if let Some(rest) = trimmed.strip_prefix("But ") {
            (last_keyword.unwrap_or(StepKeyword::Given), rest)
        } else {
            return Err(format!(
                "unrecognised line {} in scenario '{}': {trimmed}",
                line_no + 1,
                name,
            ));
        };

        last_keyword = Some(keyword);
        steps.push(Step {
            keyword,
            text: rest.to_string(),
        });
    }

    Ok(Scenario {
        name: name.to_string(),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_scenario() {
        let text = r#"
            Given a value of 1
            When it is doubled
            Then the result is 2
        "#;
        let scenario = parse_scenario("double", text).unwrap();
        assert_eq!(scenario.steps.len(), 3);
        assert_eq!(scenario.steps[0].keyword, StepKeyword::Given);
        assert_eq!(scenario.steps[0].text, "a value of 1");
        assert_eq!(scenario.steps[1].keyword, StepKeyword::When);
        assert_eq!(scenario.steps[2].keyword, StepKeyword::Then);
    }

    #[test]
    fn parse_and_inherits_keyword() {
        let text = r#"
            Given a
            And b
            When c
            And d
            Then e
            And f
        "#;
        let scenario = parse_scenario("and_test", text).unwrap();
        assert_eq!(scenario.steps.len(), 6);
        assert_eq!(scenario.steps[1].keyword, StepKeyword::Given);
        assert_eq!(scenario.steps[3].keyword, StepKeyword::When);
        assert_eq!(scenario.steps[5].keyword, StepKeyword::Then);
    }

    #[test]
    fn run_passing_scenario() {
        let mut reg = StepRegistry::new();
        reg.given(r"^a value of (\d+)$", |ctx, caps| {
            ctx.set("val", caps[1].parse::<i32>().unwrap());
            StepOutcome::Passed
        });
        reg.when(r"^it is doubled$", |ctx, _| {
            let v: i32 = *ctx.get::<i32>("val").unwrap();
            ctx.set("val", v * 2);
            StepOutcome::Passed
        });
        reg.then(r"^the result is (\d+)$", |ctx, caps| {
            let expected: i32 = caps[1].parse().unwrap();
            let actual: i32 = *ctx.get::<i32>("val").unwrap();
            if actual == expected {
                StepOutcome::Passed
            } else {
                StepOutcome::Failed(format!("expected {expected}, got {actual}"))
            }
        });

        let scenario = parse_scenario(
            "double",
            "Given a value of 3\nWhen it is doubled\nThen the result is 6",
        ).unwrap();
        let result = run_scenario(&scenario, &reg);
        assert!(result.is_passed());
    }

    #[test]
    fn run_failing_scenario_skips_remaining() {
        let mut reg = StepRegistry::new();
        reg.given(r"^setup$", |_, _| StepOutcome::Passed);
        reg.when(r"^fail$", |_, _| StepOutcome::Failed("boom".to_string()));
        reg.then(r"^check$", |_, _| StepOutcome::Passed);

        let scenario = parse_scenario(
            "fail_test",
            "Given setup\nWhen fail\nThen check",
        ).unwrap();
        let result = run_scenario(&scenario, &reg);
        assert_eq!(result.status, ScenarioStatus::Failed);
        assert!(matches!(
            result.step_results[2].outcome,
            StepOutcome::Skipped(_)
        ));
    }

    #[test]
    fn undefined_step_fails() {
        let reg = StepRegistry::new();
        let scenario = parse_scenario("undef", "Given something undefined").unwrap();
        let result = run_scenario(&scenario, &reg);
        assert_eq!(result.status, ScenarioStatus::Failed);
        assert!(result.failures()[0]
            .outcome
            .is_failed());
    }
}
