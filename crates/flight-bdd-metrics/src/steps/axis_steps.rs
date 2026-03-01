// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Axis processing step definitions.
//!
//! Connects Gherkin steps to `flight_axis` types:
//! - Deadzone creation and application
//! - Response curve setup
//! - Full axis-engine processing
//! - Output assertion with tolerance

use crate::step_registry::{StepOutcome, StepRegistry};
use flight_axis::deadzone::{DeadzoneConfig, DeadzoneProcessor};
use flight_axis::{AxisEngine, AxisFrame};

/// Register all axis-related step definitions.
pub fn register(registry: &mut StepRegistry) {
    // -- Given ----------------------------------------------------------

    registry.given(
        r"^an axis with deadzone (-?\d+\.?\d*)$",
        |ctx, caps| {
            let dz: f32 = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad float: {e}")),
            };
            match DeadzoneConfig::center_only(dz) {
                Ok(cfg) => {
                    let proc = DeadzoneProcessor::new(cfg);
                    ctx.set("deadzone_proc", proc);
                    ctx.set("deadzone_value", dz);
                    StepOutcome::Passed
                }
                Err(e) => StepOutcome::Failed(format!("invalid deadzone: {e}")),
            }
        },
    );

    registry.given(
        r"^an axis with S-curve exponent (-?\d+\.?\d*)$",
        |ctx, caps| {
            let expo: f32 = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad float: {e}")),
            };
            ctx.set("curve_expo", expo);
            StepOutcome::Passed
        },
    );

    registry.given(r"^the axis engine is ready$", |ctx, _caps| {
        let engine = AxisEngine::new_for_axis("test_axis".to_string());
        ctx.set("axis_engine", engine);
        StepOutcome::Passed
    });

    // -- When -----------------------------------------------------------

    registry.when(
        r"^input (-?\d+\.?\d*) is processed$",
        |ctx, caps| {
            let input: f32 = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad float: {e}")),
            };

            // If a deadzone processor exists, use it
            if let Some(proc) = ctx.get::<DeadzoneProcessor>("deadzone_proc") {
                let output = proc.apply(input);
                ctx.set("axis_output", output);
                return StepOutcome::Passed;
            }

            // Otherwise use the axis engine
            if let Some(engine) = ctx.get::<AxisEngine>("axis_engine") {
                let mut frame = AxisFrame::new(input, 1000);
                match engine.process(&mut frame) {
                    Ok(()) => {
                        ctx.set("axis_output", frame.out);
                        StepOutcome::Passed
                    }
                    Err(e) => StepOutcome::Failed(format!("engine error: {e}")),
                }
            } else {
                // Bare input – just store the raw value
                ctx.set("axis_output", input);
                StepOutcome::Passed
            }
        },
    );

    // -- Then -----------------------------------------------------------

    registry.then(
        r"^output should be (-?\d+\.?\d*) ±(\d+\.?\d*)$",
        |ctx, caps| {
            let expected: f32 = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad expected: {e}")),
            };
            let tolerance: f32 = match caps[2].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad tolerance: {e}")),
            };
            match ctx.get::<f32>("axis_output") {
                Some(actual) => {
                    if (*actual - expected).abs() <= tolerance {
                        StepOutcome::Passed
                    } else {
                        StepOutcome::Failed(format!(
                            "expected {expected} ±{tolerance}, got {actual}"
                        ))
                    }
                }
                None => StepOutcome::Failed("no axis output in context".to_string()),
            }
        },
    );

    registry.then(r"^output should be zero$", |ctx, _caps| {
        match ctx.get::<f32>("axis_output") {
            Some(actual) => {
                if actual.abs() < 1e-6 {
                    StepOutcome::Passed
                } else {
                    StepOutcome::Failed(format!("expected 0.0, got {actual}"))
                }
            }
            None => StepOutcome::Failed("no axis output in context".to_string()),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{parse_scenario, run_scenario};

    fn registry() -> StepRegistry {
        let mut r = StepRegistry::new();
        register(&mut r);
        r
    }

    #[test]
    fn deadzone_zeroes_small_input() {
        let reg = registry();
        let s = parse_scenario(
            "dz_small",
            "Given an axis with deadzone 0.05\nWhen input 0.02 is processed\nThen output should be zero",
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "step results: {:?}", result.step_results);
    }

    #[test]
    fn deadzone_passes_large_input() {
        let reg = registry();
        let s = parse_scenario(
            "dz_large",
            "Given an axis with deadzone 0.05\nWhen input 1.0 is processed\nThen output should be 1.0 ±0.001",
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "step results: {:?}", result.step_results);
    }

    #[test]
    fn deadzone_negative_input_zeroed() {
        let reg = registry();
        let s = parse_scenario(
            "dz_neg",
            "Given an axis with deadzone 0.1\nWhen input -0.05 is processed\nThen output should be zero",
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "step results: {:?}", result.step_results);
    }

    #[test]
    fn engine_passthrough() {
        let reg = registry();
        let s = parse_scenario(
            "engine_pass",
            "Given the axis engine is ready\nWhen input 0.75 is processed\nThen output should be 0.75 ±0.01",
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "step results: {:?}", result.step_results);
    }
}
