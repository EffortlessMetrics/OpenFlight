// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin API surface — host context, plugin trait, and per-tick budget (ADR-003).
//!
//! * [`PluginContext`] — read-only snapshot the host provides each tick.
//! * [`PluginApi`] — trait every plugin must implement.
//! * [`PluginBudget`] — per-tick time budget with enforcement.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Read-only context the host provides to plugins on each tick.
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Current telemetry values keyed by variable name.
    pub telemetry: HashMap<String, f64>,
    /// Plugin-specific configuration key/values.
    pub config: HashMap<String, String>,
    /// Simulation time elapsed since start (seconds).
    pub sim_time_secs: f64,
}

impl Default for PluginContext {
    fn default() -> Self {
        Self {
            telemetry: HashMap::new(),
            config: HashMap::new(),
            sim_time_secs: 0.0,
        }
    }
}

impl PluginContext {
    /// Read a telemetry value by name.
    #[must_use]
    pub fn read_telemetry(&self, name: &str) -> Option<f64> {
        self.telemetry.get(name).copied()
    }

    /// Read a config value by key.
    #[must_use]
    pub fn get_config(&self, key: &str) -> Option<&str> {
        self.config.get(key).map(String::as_str)
    }
}

/// Output a plugin may produce on a given tick.
#[derive(Debug, Clone, Default)]
pub struct PluginOutput {
    /// Key/value pairs the plugin writes back to the host.
    pub values: HashMap<String, f64>,
}

/// The trait every plugin must implement to participate in the tick loop.
pub trait PluginApi: Send + Sync {
    /// Called once when the plugin is first loaded.
    fn on_init(&mut self, ctx: &PluginContext) -> Result<(), String>;

    /// Called every tick with the current context and delta-time in seconds.
    fn on_tick(&mut self, ctx: &PluginContext, dt: f64) -> Result<PluginOutput, String>;

    /// Called once when the plugin is being unloaded.
    fn on_shutdown(&mut self) -> Result<(), String>;
}

/// Per-tick time budget for a plugin.
#[derive(Debug, Clone)]
pub struct PluginBudget {
    /// Maximum time allowed per tick.
    pub limit: Duration,
}

impl PluginBudget {
    /// Create a new budget with the given per-tick limit.
    #[must_use]
    pub fn new(limit: Duration) -> Self {
        Self { limit }
    }

    /// Start a budget measurement, returning a guard that can be checked.
    #[must_use]
    pub fn start(&self) -> BudgetGuard {
        BudgetGuard {
            start: Instant::now(),
            limit: self.limit,
        }
    }
}

/// Tracks elapsed time for a single tick invocation.
#[derive(Debug)]
pub struct BudgetGuard {
    start: Instant,
    limit: Duration,
}

impl BudgetGuard {
    /// Elapsed time since the guard was created.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Check whether the budget has been exceeded. Returns `Err` with a
    /// [`BudgetViolation`] if the elapsed time exceeds the limit.
    pub fn check(&self) -> Result<(), BudgetViolation> {
        let elapsed = self.elapsed();
        if elapsed > self.limit {
            Err(BudgetViolation {
                limit: self.limit,
                actual: elapsed,
            })
        } else {
            Ok(())
        }
    }

    /// Finish the guard and return the elapsed duration, plus an optional
    /// violation if the budget was exceeded.
    #[must_use]
    pub fn finish(self) -> (Duration, Option<BudgetViolation>) {
        let elapsed = self.start.elapsed();
        let violation = if elapsed > self.limit {
            Some(BudgetViolation {
                limit: self.limit,
                actual: elapsed,
            })
        } else {
            None
        };
        (elapsed, violation)
    }
}

/// Produced when a plugin exceeds its per-tick time budget.
#[derive(Debug, Clone)]
pub struct BudgetViolation {
    /// The configured limit.
    pub limit: Duration,
    /// The actual time the plugin used.
    pub actual: Duration,
}

impl std::fmt::Display for BudgetViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "budget violation: used {:?} of {:?} allowed",
            self.actual, self.limit
        )
    }
}

impl std::error::Error for BudgetViolation {}

/// Execute a plugin's `on_tick` under a time budget, returning the output
/// or a budget violation.
pub fn tick_with_budget(
    plugin: &mut dyn PluginApi,
    ctx: &PluginContext,
    dt: f64,
    budget: &PluginBudget,
) -> Result<PluginOutput, BudgetViolation> {
    let guard = budget.start();
    // Run the plugin tick — we measure wall time around it.
    let output = plugin.on_tick(ctx, dt).map_err(|_| BudgetViolation {
        limit: budget.limit,
        actual: guard.elapsed(),
    })?;
    let (_, violation) = guard.finish();
    if let Some(v) = violation {
        Err(v)
    } else {
        Ok(output)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Stub plugin that immediately returns.
    struct FastPlugin;

    impl PluginApi for FastPlugin {
        fn on_init(&mut self, _ctx: &PluginContext) -> Result<(), String> {
            Ok(())
        }
        fn on_tick(&mut self, _ctx: &PluginContext, _dt: f64) -> Result<PluginOutput, String> {
            Ok(PluginOutput::default())
        }
        fn on_shutdown(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    // Stub plugin that writes output values.
    struct OutputPlugin {
        value: f64,
    }

    impl PluginApi for OutputPlugin {
        fn on_init(&mut self, _ctx: &PluginContext) -> Result<(), String> {
            Ok(())
        }
        fn on_tick(&mut self, _ctx: &PluginContext, _dt: f64) -> Result<PluginOutput, String> {
            let mut out = PluginOutput::default();
            out.values.insert("throttle".into(), self.value);
            Ok(out)
        }
        fn on_shutdown(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    // Stub plugin that fails on tick.
    struct FailPlugin;

    impl PluginApi for FailPlugin {
        fn on_init(&mut self, _ctx: &PluginContext) -> Result<(), String> {
            Ok(())
        }
        fn on_tick(&mut self, _ctx: &PluginContext, _dt: f64) -> Result<PluginOutput, String> {
            Err("tick error".into())
        }
        fn on_shutdown(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    // Stub plugin that fails on init.
    struct FailInitPlugin;

    impl PluginApi for FailInitPlugin {
        fn on_init(&mut self, _ctx: &PluginContext) -> Result<(), String> {
            Err("init error".into())
        }
        fn on_tick(&mut self, _ctx: &PluginContext, _dt: f64) -> Result<PluginOutput, String> {
            Ok(PluginOutput::default())
        }
        fn on_shutdown(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    // ── PluginContext ─────────────────────────────────────────────────

    #[test]
    fn context_read_telemetry() {
        let mut ctx = PluginContext::default();
        ctx.telemetry.insert("altitude".into(), 35000.0);
        assert_eq!(ctx.read_telemetry("altitude"), Some(35000.0));
        assert_eq!(ctx.read_telemetry("speed"), None);
    }

    #[test]
    fn context_get_config() {
        let mut ctx = PluginContext::default();
        ctx.config.insert("mode".into(), "combat".into());
        assert_eq!(ctx.get_config("mode"), Some("combat"));
        assert_eq!(ctx.get_config("missing"), None);
    }

    #[test]
    fn context_default_sim_time() {
        let ctx = PluginContext::default();
        assert_eq!(ctx.sim_time_secs, 0.0);
    }

    // ── PluginApi trait ───────────────────────────────────────────────

    #[test]
    fn fast_plugin_lifecycle() {
        let mut plugin = FastPlugin;
        let ctx = PluginContext::default();
        plugin.on_init(&ctx).unwrap();
        let output = plugin.on_tick(&ctx, 0.004).unwrap();
        assert!(output.values.is_empty());
        plugin.on_shutdown().unwrap();
    }

    #[test]
    fn output_plugin_produces_values() {
        let mut plugin = OutputPlugin { value: 0.75 };
        let ctx = PluginContext::default();
        let output = plugin.on_tick(&ctx, 0.004).unwrap();
        assert_eq!(output.values.get("throttle"), Some(&0.75));
    }

    #[test]
    fn fail_plugin_returns_error() {
        let mut plugin = FailPlugin;
        let ctx = PluginContext::default();
        assert!(plugin.on_tick(&ctx, 0.004).is_err());
    }

    #[test]
    fn fail_init_returns_error() {
        let mut plugin = FailInitPlugin;
        let ctx = PluginContext::default();
        assert!(plugin.on_init(&ctx).is_err());
    }

    // ── PluginBudget ──────────────────────────────────────────────────

    #[test]
    fn budget_within_limit() {
        let budget = PluginBudget::new(Duration::from_secs(1));
        let guard = budget.start();
        // Immediate check should be within budget.
        assert!(guard.check().is_ok());
    }

    #[test]
    fn budget_guard_finish_within_limit() {
        let budget = PluginBudget::new(Duration::from_secs(1));
        let guard = budget.start();
        let (elapsed, violation) = guard.finish();
        assert!(violation.is_none());
        assert!(elapsed < Duration::from_secs(1));
    }

    #[test]
    fn budget_violation_display() {
        let v = BudgetViolation {
            limit: Duration::from_millis(4),
            actual: Duration::from_millis(10),
        };
        let msg = v.to_string();
        assert!(msg.contains("budget violation"));
    }

    // ── tick_with_budget ──────────────────────────────────────────────

    #[test]
    fn tick_with_budget_fast_plugin_ok() {
        let mut plugin = FastPlugin;
        let ctx = PluginContext::default();
        let budget = PluginBudget::new(Duration::from_secs(1));
        let result = tick_with_budget(&mut plugin, &ctx, 0.004, &budget);
        assert!(result.is_ok());
    }

    #[test]
    fn tick_with_budget_output_values() {
        let mut plugin = OutputPlugin { value: 0.5 };
        let ctx = PluginContext::default();
        let budget = PluginBudget::new(Duration::from_secs(1));
        let output = tick_with_budget(&mut plugin, &ctx, 0.004, &budget).unwrap();
        assert_eq!(output.values.get("throttle"), Some(&0.5));
    }

    #[test]
    fn tick_with_budget_fail_plugin_returns_violation() {
        let mut plugin = FailPlugin;
        let ctx = PluginContext::default();
        let budget = PluginBudget::new(Duration::from_secs(1));
        let result = tick_with_budget(&mut plugin, &ctx, 0.004, &budget);
        assert!(result.is_err());
    }

    // ── PluginOutput ──────────────────────────────────────────────────

    #[test]
    fn plugin_output_default_empty() {
        let out = PluginOutput::default();
        assert!(out.values.is_empty());
    }

    #[test]
    fn plugin_output_multiple_values() {
        let mut out = PluginOutput::default();
        out.values.insert("x".into(), 1.0);
        out.values.insert("y".into(), 2.0);
        out.values.insert("z".into(), 3.0);
        assert_eq!(out.values.len(), 3);
    }
}
