// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Recovery-policy engine for the watchdog system.

/// Action the watchdog should take in response to component failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Restart the named component.
    RestartComponent(String),
    /// Put the entire system into a safe/fallback mode.
    EnterSafeMode,
    /// Surface a user-visible alert.
    AlertUser(String),
    /// Emit a warning log entry.
    LogWarning(String),
    /// No action required.
    NoAction,
}

/// A single rule mapping a failure threshold to an action.
pub struct RecoveryRule {
    /// Component this rule applies to.
    pub component: String,
    /// Minimum consecutive failure count that activates this rule.
    pub on_failure_count: u32,
    /// Action to take when the threshold is met.
    pub action: RecoveryAction,
}

/// Determines recovery actions based on component health state.
///
/// Rules are evaluated in insertion order; the **last** rule whose
/// `on_failure_count` is ≤ the actual failure count wins, giving
/// natural escalation from mild to severe actions.
pub struct RecoveryPolicy {
    policies: Vec<RecoveryRule>,
}

impl RecoveryPolicy {
    /// Create an empty policy set.
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// Add a recovery rule for a component.
    pub fn add_rule(&mut self, component: &str, failure_count: u32, action: RecoveryAction) {
        self.policies.push(RecoveryRule {
            component: component.to_string(),
            on_failure_count: failure_count,
            action,
        });
    }

    /// Evaluate policies for `component` given its current `failure_count`.
    ///
    /// Returns the action from the highest-threshold matching rule, or
    /// [`RecoveryAction::NoAction`] if no rules match.
    pub fn evaluate(&self, component: &str, failure_count: u32) -> RecoveryAction {
        self.policies
            .iter()
            .rfind(|r| r.component == component && failure_count >= r.on_failure_count)
            .map(|r| r.action.clone())
            .unwrap_or(RecoveryAction::NoAction)
    }

    /// Return all rules that apply to `component`, in insertion order.
    pub fn rules_for(&self, component: &str) -> Vec<&RecoveryRule> {
        self.policies
            .iter()
            .filter(|r| r.component == component)
            .collect()
    }
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_rule_returns_no_action() {
        let policy = RecoveryPolicy::new();
        assert_eq!(policy.evaluate("axis", 5), RecoveryAction::NoAction);
    }

    #[test]
    fn first_failure_triggers_log_warning() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("hid", 1, RecoveryAction::LogWarning("HID trouble".into()));
        assert_eq!(
            policy.evaluate("hid", 1),
            RecoveryAction::LogWarning("HID trouble".into())
        );
    }

    #[test]
    fn multiple_failures_trigger_restart() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("hid", 1, RecoveryAction::LogWarning("warn".into()));
        policy.add_rule("hid", 3, RecoveryAction::RestartComponent("hid".into()));
        assert_eq!(
            policy.evaluate("hid", 3),
            RecoveryAction::RestartComponent("hid".into())
        );
    }

    #[test]
    fn many_failures_trigger_safe_mode() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("ffb", 1, RecoveryAction::LogWarning("warn".into()));
        policy.add_rule("ffb", 3, RecoveryAction::RestartComponent("ffb".into()));
        policy.add_rule("ffb", 10, RecoveryAction::EnterSafeMode);
        assert_eq!(policy.evaluate("ffb", 10), RecoveryAction::EnterSafeMode);
    }

    #[test]
    fn per_component_policies() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("a", 1, RecoveryAction::LogWarning("a warn".into()));
        policy.add_rule("b", 1, RecoveryAction::AlertUser("b alert".into()));
        assert_eq!(
            policy.evaluate("a", 1),
            RecoveryAction::LogWarning("a warn".into())
        );
        assert_eq!(
            policy.evaluate("b", 1),
            RecoveryAction::AlertUser("b alert".into())
        );
    }

    #[test]
    fn rules_for_returns_correct_subset() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("x", 1, RecoveryAction::LogWarning("w".into()));
        policy.add_rule("y", 2, RecoveryAction::EnterSafeMode);
        policy.add_rule("x", 5, RecoveryAction::RestartComponent("x".into()));
        let rules = policy.rules_for("x");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].on_failure_count, 1);
        assert_eq!(rules[1].on_failure_count, 5);
    }

    #[test]
    fn below_threshold_returns_no_action() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("z", 5, RecoveryAction::EnterSafeMode);
        assert_eq!(policy.evaluate("z", 4), RecoveryAction::NoAction);
    }
}
