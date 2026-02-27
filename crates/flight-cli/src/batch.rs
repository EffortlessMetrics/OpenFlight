// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Batch operations for the CLI (REQ-881).
//!
//! Allows multiple axis configuration operations to be submitted as a single
//! batch, executing each in order and collecting per-operation results.

use std::fmt;

/// A single operation that can appear inside a batch.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchOp {
    /// Set deadzone for an axis (axis name, value 0.0–1.0).
    SetDeadzone { axis: String, value: f64 },
    /// Set expo/sensitivity curve for an axis (axis name, value -1.0–1.0).
    SetExpo { axis: String, value: f64 },
    /// Set a named curve preset on an axis.
    SetCurve { axis: String, curve: String },
    /// Enable an axis by name.
    EnableAxis { axis: String },
    /// Disable an axis by name.
    DisableAxis { axis: String },
}

impl fmt::Display for BatchOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SetDeadzone { axis, value } => write!(f, "set-deadzone {axis} {value}"),
            Self::SetExpo { axis, value } => write!(f, "set-expo {axis} {value}"),
            Self::SetCurve { axis, curve } => write!(f, "set-curve {axis} {curve}"),
            Self::EnableAxis { axis } => write!(f, "enable-axis {axis}"),
            Self::DisableAxis { axis } => write!(f, "disable-axis {axis}"),
        }
    }
}

/// Outcome of a single operation inside a batch.
#[derive(Debug, Clone)]
pub struct OpResult {
    /// The operation that was attempted.
    pub op: BatchOp,
    /// `Ok(())` on success, `Err(message)` on failure.
    pub outcome: Result<(), String>,
}

/// Aggregated result of executing a full batch.
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// Per-operation results, in execution order.
    pub results: Vec<OpResult>,
}

impl BatchResult {
    /// Number of operations that succeeded.
    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.outcome.is_ok()).count()
    }

    /// Number of operations that failed.
    pub fn failure_count(&self) -> usize {
        self.results.iter().filter(|r| r.outcome.is_err()).count()
    }

    /// `true` when every operation succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(|r| r.outcome.is_ok())
    }
}

/// A named collection of operations to execute as a batch.
#[derive(Debug, Clone)]
pub struct BatchCommand {
    pub ops: Vec<BatchOp>,
}

/// Execute a slice of [`BatchOp`]s sequentially, collecting results.
///
/// A failure in one operation does **not** prevent subsequent operations from
/// running.
pub fn execute_batch(ops: &[BatchOp]) -> BatchResult {
    let results = ops.iter().map(|op| execute_single(op)).collect();
    BatchResult { results }
}

/// Execute one operation, returning its [`OpResult`].
fn execute_single(op: &BatchOp) -> OpResult {
    let outcome = match op {
        BatchOp::SetDeadzone { axis, value } => validate_deadzone(axis, *value),
        BatchOp::SetExpo { axis, value } => validate_expo(axis, *value),
        BatchOp::SetCurve { axis, curve } => validate_curve(axis, curve),
        BatchOp::EnableAxis { axis } | BatchOp::DisableAxis { axis } => validate_axis_name(axis),
    };
    OpResult {
        op: op.clone(),
        outcome,
    }
}

fn validate_deadzone(axis: &str, value: f64) -> Result<(), String> {
    if axis.is_empty() {
        return Err("axis name must not be empty".into());
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(format!("deadzone value {value} out of range 0.0–1.0"));
    }
    Ok(())
}

fn validate_expo(axis: &str, value: f64) -> Result<(), String> {
    if axis.is_empty() {
        return Err("axis name must not be empty".into());
    }
    if !(-1.0..=1.0).contains(&value) {
        return Err(format!("expo value {value} out of range -1.0–1.0"));
    }
    Ok(())
}

fn validate_curve(axis: &str, curve: &str) -> Result<(), String> {
    if axis.is_empty() {
        return Err("axis name must not be empty".into());
    }
    if curve.is_empty() {
        return Err("curve name must not be empty".into());
    }
    Ok(())
}

fn validate_axis_name(axis: &str) -> Result<(), String> {
    if axis.is_empty() {
        return Err("axis name must not be empty".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_operation_succeeds() {
        let ops = [BatchOp::SetDeadzone {
            axis: "roll".into(),
            value: 0.05,
        }];
        let result = execute_batch(&ops);
        assert_eq!(result.results.len(), 1);
        assert!(result.results[0].outcome.is_ok());
        assert!(result.all_succeeded());
    }

    #[test]
    fn multiple_operations_execute_in_order() {
        let ops = [
            BatchOp::SetDeadzone {
                axis: "roll".into(),
                value: 0.05,
            },
            BatchOp::SetExpo {
                axis: "pitch".into(),
                value: 0.3,
            },
            BatchOp::EnableAxis {
                axis: "yaw".into(),
            },
        ];
        let result = execute_batch(&ops);
        assert_eq!(result.results.len(), 3);
        assert_eq!(result.success_count(), 3);
        // Verify order is preserved.
        assert_eq!(result.results[0].op, ops[0]);
        assert_eq!(result.results[1].op, ops[1]);
        assert_eq!(result.results[2].op, ops[2]);
    }

    #[test]
    fn failure_in_one_does_not_stop_others() {
        let ops = [
            BatchOp::SetDeadzone {
                axis: "roll".into(),
                value: 0.05,
            },
            // Out-of-range value → failure
            BatchOp::SetDeadzone {
                axis: "pitch".into(),
                value: 2.0,
            },
            BatchOp::EnableAxis {
                axis: "yaw".into(),
            },
        ];
        let result = execute_batch(&ops);
        assert_eq!(result.results.len(), 3);
        assert!(result.results[0].outcome.is_ok());
        assert!(result.results[1].outcome.is_err());
        assert!(result.results[2].outcome.is_ok());
    }

    #[test]
    fn batch_result_tracks_individual_results() {
        let ops = [
            BatchOp::EnableAxis {
                axis: "roll".into(),
            },
            BatchOp::SetExpo {
                axis: "".into(),
                value: 0.0,
            },
            BatchOp::DisableAxis {
                axis: "yaw".into(),
            },
        ];
        let result = execute_batch(&ops);
        assert_eq!(result.success_count(), 2);
        assert_eq!(result.failure_count(), 1);
        assert!(!result.all_succeeded());
    }
}
