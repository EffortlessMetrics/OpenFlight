// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shutdown Coordinator
//!
//! Executes a graceful shutdown sequence in ordered phases with
//! per-phase timeouts and dependency-aware component ordering.

use std::time::{Duration, Instant};

/// Describes a single shutdown phase.
#[derive(Debug, Clone)]
pub struct ShutdownPhase {
    pub name: String,
    pub components: Vec<String>,
    pub timeout_ms: u64,
}

/// Result of a full shutdown sequence.
#[derive(Debug, Clone)]
pub struct ShutdownResult {
    pub completed: Vec<String>,
    pub timed_out: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub total_duration_ms: u64,
}

impl ShutdownResult {
    /// Whether the shutdown completed with no timeouts or failures.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.timed_out.is_empty() && self.failed.is_empty()
    }
}

/// Outcome returned by a component's shutdown handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentShutdownOutcome {
    Ok,
    TimedOut,
    Failed(String),
}

/// Callback invoked for each component during shutdown.
/// Implementations should perform cleanup and return the outcome.
pub type ShutdownHandler = Box<dyn Fn(&str, Duration) -> ComponentShutdownOutcome + Send + Sync>;

/// Coordinates a phased, graceful shutdown.
pub struct ShutdownCoordinator {
    phases: Vec<ShutdownPhase>,
    timeout_ms: u64,
    handler: Option<ShutdownHandler>,
}

impl ShutdownCoordinator {
    #[must_use]
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            phases: Vec::new(),
            timeout_ms,
            handler: None,
        }
    }

    /// Register a shutdown handler that will be called for every component.
    pub fn set_handler(&mut self, handler: ShutdownHandler) {
        self.handler = Some(handler);
    }

    /// Add a phase to the shutdown sequence.
    pub fn add_phase(&mut self, name: &str, components: Vec<String>, timeout_ms: u64) {
        self.phases.push(ShutdownPhase {
            name: name.to_string(),
            components,
            timeout_ms,
        });
    }

    /// Number of registered phases.
    #[must_use]
    pub fn phase_count(&self) -> usize {
        self.phases.len()
    }

    /// Global timeout in milliseconds.
    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Execute the full shutdown sequence, invoking the handler for each
    /// component in phase order.
    #[must_use]
    pub fn execute_shutdown(&self) -> ShutdownResult {
        let start = Instant::now();
        let global_deadline = start + Duration::from_millis(self.timeout_ms);

        let mut completed = Vec::new();
        let mut timed_out = Vec::new();
        let mut failed: Vec<(String, String)> = Vec::new();

        for phase in &self.phases {
            let phase_timeout = Duration::from_millis(phase.timeout_ms);

            for component in &phase.components {
                // Respect global deadline
                if Instant::now() >= global_deadline {
                    timed_out.push(component.clone());
                    continue;
                }

                let remaining_global = global_deadline.saturating_duration_since(Instant::now());
                let effective_timeout = phase_timeout.min(remaining_global);

                let outcome = if let Some(ref handler) = self.handler {
                    handler(component, effective_timeout)
                } else {
                    // No handler — assume instant success.
                    ComponentShutdownOutcome::Ok
                };

                match outcome {
                    ComponentShutdownOutcome::Ok => completed.push(component.clone()),
                    ComponentShutdownOutcome::TimedOut => timed_out.push(component.clone()),
                    ComponentShutdownOutcome::Failed(msg) => {
                        failed.push((component.clone(), msg));
                    }
                }
            }
        }

        ShutdownResult {
            completed,
            timed_out,
            failed,
            total_duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_shutdown_is_clean() {
        let coord = ShutdownCoordinator::new(5000);
        let result = coord.execute_shutdown();
        assert!(result.is_clean());
        assert!(result.completed.is_empty());
    }

    #[test]
    fn single_phase_all_succeed() {
        let mut coord = ShutdownCoordinator::new(5000);
        coord.add_phase(
            "services",
            vec!["grpc".to_string(), "http".to_string()],
            1000,
        );
        let result = coord.execute_shutdown();
        assert!(result.is_clean());
        assert_eq!(result.completed.len(), 2);
    }

    #[test]
    fn multiple_phases_ordered() {
        let mut coord = ShutdownCoordinator::new(5000);
        coord.add_phase("network", vec!["grpc".to_string()], 1000);
        coord.add_phase("core", vec!["axis".to_string()], 1000);
        let result = coord.execute_shutdown();
        assert_eq!(result.completed, vec!["grpc", "axis"]);
    }

    #[test]
    fn handler_failure_recorded() {
        let mut coord = ShutdownCoordinator::new(5000);
        coord.add_phase("svc", vec!["broken".to_string(), "ok".to_string()], 1000);
        coord.set_handler(Box::new(|name, _timeout| {
            if name == "broken" {
                ComponentShutdownOutcome::Failed("crash".to_string())
            } else {
                ComponentShutdownOutcome::Ok
            }
        }));

        let result = coord.execute_shutdown();
        assert!(!result.is_clean());
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].0, "broken");
        assert_eq!(result.completed, vec!["ok"]);
    }

    #[test]
    fn handler_timeout_recorded() {
        let mut coord = ShutdownCoordinator::new(5000);
        coord.add_phase("svc", vec!["slow".to_string()], 1000);
        coord.set_handler(Box::new(|_name, _timeout| {
            ComponentShutdownOutcome::TimedOut
        }));

        let result = coord.execute_shutdown();
        assert!(!result.is_clean());
        assert_eq!(result.timed_out, vec!["slow"]);
    }

    #[test]
    fn phase_count() {
        let mut coord = ShutdownCoordinator::new(5000);
        assert_eq!(coord.phase_count(), 0);
        coord.add_phase("a", vec![], 100);
        coord.add_phase("b", vec![], 200);
        assert_eq!(coord.phase_count(), 2);
    }

    #[test]
    fn global_timeout_exposed() {
        let coord = ShutdownCoordinator::new(3000);
        assert_eq!(coord.timeout_ms(), 3000);
    }

    #[test]
    fn mixed_outcomes_across_phases() {
        let mut coord = ShutdownCoordinator::new(10_000);
        coord.add_phase("phase1", vec!["a".to_string(), "b".to_string()], 2000);
        coord.add_phase("phase2", vec!["c".to_string()], 2000);
        coord.set_handler(Box::new(|name, _| match name {
            "a" => ComponentShutdownOutcome::Ok,
            "b" => ComponentShutdownOutcome::Failed("err".to_string()),
            "c" => ComponentShutdownOutcome::TimedOut,
            _ => ComponentShutdownOutcome::Ok,
        }));

        let result = coord.execute_shutdown();
        assert_eq!(result.completed, vec!["a"]);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.timed_out, vec!["c"]);
    }

    #[test]
    fn result_is_clean_checks_both() {
        let clean = ShutdownResult {
            completed: vec!["a".to_string()],
            timed_out: vec![],
            failed: vec![],
            total_duration_ms: 10,
        };
        assert!(clean.is_clean());

        let with_timeout = ShutdownResult {
            completed: vec![],
            timed_out: vec!["x".to_string()],
            failed: vec![],
            total_duration_ms: 10,
        };
        assert!(!with_timeout.is_clean());

        let with_failure = ShutdownResult {
            completed: vec![],
            timed_out: vec![],
            failed: vec![("y".to_string(), "boom".to_string())],
            total_duration_ms: 10,
        };
        assert!(!with_failure.is_clean());
    }

    #[test]
    fn empty_phase_components_are_skipped() {
        let mut coord = ShutdownCoordinator::new(5000);
        coord.add_phase("empty", vec![], 1000);
        coord.add_phase("real", vec!["x".to_string()], 1000);
        let result = coord.execute_shutdown();
        assert_eq!(result.completed, vec!["x"]);
    }

    #[test]
    fn total_duration_is_plausible() {
        let mut coord = ShutdownCoordinator::new(5000);
        coord.add_phase("fast", vec!["a".to_string()], 1000);
        let result = coord.execute_shutdown();
        // Should complete nearly instantly without a real handler
        assert!(result.total_duration_ms < 1000);
    }
}
