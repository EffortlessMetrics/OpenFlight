// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Escalation ladder for the watchdog system.
//!
//! Implements a four-stage escalation chain:
//! **Warn → Degrade → Restart → SafeMode**
//!
//! Each stage has configurable thresholds and the ladder tracks
//! per-component state so that recovery automatically de-escalates.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The four escalation levels, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EscalationLevel {
    /// Component is operating normally.
    Normal,
    /// A warning has been issued; the component is still functional.
    Warn,
    /// Non-essential functionality disabled for this component.
    Degrade,
    /// Component restart has been requested.
    Restart,
    /// System-wide safe mode; only safety-critical paths remain active.
    SafeMode,
}

impl std::fmt::Display for EscalationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscalationLevel::Normal => write!(f, "Normal"),
            EscalationLevel::Warn => write!(f, "Warn"),
            EscalationLevel::Degrade => write!(f, "Degrade"),
            EscalationLevel::Restart => write!(f, "Restart"),
            EscalationLevel::SafeMode => write!(f, "SafeMode"),
        }
    }
}

/// Action the escalation engine requests for a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationAction {
    /// No action needed.
    None,
    /// Emit a warning log/event.
    Warn(String),
    /// Disable non-essential features for this component.
    Degrade(String),
    /// Restart the component.
    Restart(String),
    /// Enter system-wide safe mode.
    EnterSafeMode(String),
}

/// Configurable thresholds for the escalation ladder.
#[derive(Debug, Clone)]
pub struct EscalationConfig {
    /// Consecutive failures before escalating to Warn.
    pub warn_threshold: u32,
    /// Consecutive failures before escalating to Degrade.
    pub degrade_threshold: u32,
    /// Consecutive failures before escalating to Restart.
    pub restart_threshold: u32,
    /// Consecutive failures before escalating to SafeMode.
    pub safe_mode_threshold: u32,
    /// Consecutive successes required to de-escalate one level.
    pub recovery_threshold: u32,
    /// Cooldown between restart attempts.
    pub restart_cooldown: Duration,
    /// Maximum restart attempts before jumping to SafeMode.
    pub max_restart_attempts: u32,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self {
            warn_threshold: 1,
            degrade_threshold: 5,
            restart_threshold: 10,
            safe_mode_threshold: 20,
            recovery_threshold: 5,
            restart_cooldown: Duration::from_secs(10),
            max_restart_attempts: 3,
        }
    }
}

/// Record of an escalation state transition.
#[derive(Debug, Clone)]
pub struct EscalationTransition {
    pub component: String,
    pub from: EscalationLevel,
    pub to: EscalationLevel,
    pub reason: String,
    pub timestamp: Instant,
}

/// Per-component escalation state tracked by the ladder.
#[derive(Debug)]
struct ComponentState {
    level: EscalationLevel,
    consecutive_failures: u32,
    consecutive_successes: u32,
    restart_count: u32,
    last_restart: Option<Instant>,
}

impl ComponentState {
    fn new() -> Self {
        Self {
            level: EscalationLevel::Normal,
            consecutive_failures: 0,
            consecutive_successes: 0,
            restart_count: 0,
            last_restart: None,
        }
    }
}

/// The escalation ladder engine.
///
/// Tracks per-component failure/success counts and determines the appropriate
/// escalation action based on configured thresholds.
pub struct EscalationLadder {
    config: EscalationConfig,
    components: HashMap<String, ComponentState>,
    transitions: Vec<EscalationTransition>,
    max_transitions: usize,
}

impl EscalationLadder {
    /// Create a new escalation ladder with the given configuration.
    pub fn new(config: EscalationConfig) -> Self {
        Self {
            config,
            components: HashMap::new(),
            transitions: Vec::new(),
            max_transitions: 1000,
        }
    }

    /// Register a component for escalation tracking.
    pub fn register(&mut self, component: &str) {
        self.components
            .entry(component.to_string())
            .or_insert_with(ComponentState::new);
    }

    /// Record a failure for a component and return the escalation action.
    pub fn record_failure(&mut self, component: &str, reason: &str) -> EscalationAction {
        let config = self.config.clone();
        let state = self
            .components
            .entry(component.to_string())
            .or_insert_with(ComponentState::new);

        state.consecutive_failures += 1;
        state.consecutive_successes = 0;

        let failures = state.consecutive_failures;
        let old_level = state.level;

        let (new_level, action) = if failures >= config.safe_mode_threshold
            || (state.level == EscalationLevel::Restart
                && state.restart_count >= config.max_restart_attempts)
        {
            (
                EscalationLevel::SafeMode,
                EscalationAction::EnterSafeMode(format!(
                    "{component}: {reason} ({failures} consecutive failures)"
                )),
            )
        } else if failures >= config.restart_threshold {
            let can_restart = state
                .last_restart
                .map(|t| t.elapsed() >= config.restart_cooldown)
                .unwrap_or(true);

            if can_restart {
                state.restart_count += 1;
                state.last_restart = Some(Instant::now());
                (
                    EscalationLevel::Restart,
                    EscalationAction::Restart(format!(
                        "{component}: {reason} (restart #{}, {failures} failures)",
                        state.restart_count
                    )),
                )
            } else {
                // Cooldown active, stay at Degrade
                (
                    EscalationLevel::Degrade,
                    EscalationAction::Degrade(format!(
                        "{component}: {reason} (restart cooldown active)"
                    )),
                )
            }
        } else if failures >= config.degrade_threshold {
            (
                EscalationLevel::Degrade,
                EscalationAction::Degrade(format!("{component}: {reason} ({failures} failures)")),
            )
        } else if failures >= config.warn_threshold {
            (
                EscalationLevel::Warn,
                EscalationAction::Warn(format!("{component}: {reason} ({failures} failures)")),
            )
        } else {
            (EscalationLevel::Normal, EscalationAction::None)
        };

        state.level = new_level;

        if old_level != new_level {
            self.record_transition(component, old_level, new_level, reason);
        }

        action
    }

    /// Record a success for a component, potentially de-escalating.
    pub fn record_success(&mut self, component: &str) -> EscalationLevel {
        let recovery_threshold = self.config.recovery_threshold;
        let state = self
            .components
            .entry(component.to_string())
            .or_insert_with(ComponentState::new);

        state.consecutive_failures = 0;
        state.consecutive_successes += 1;

        let mut transition = None;

        if state.consecutive_successes >= recovery_threshold
            && state.level > EscalationLevel::Normal
        {
            let old_level = state.level;
            state.level = match old_level {
                EscalationLevel::SafeMode => EscalationLevel::Restart,
                EscalationLevel::Restart => EscalationLevel::Degrade,
                EscalationLevel::Degrade => EscalationLevel::Warn,
                EscalationLevel::Warn => EscalationLevel::Normal,
                EscalationLevel::Normal => EscalationLevel::Normal,
            };
            state.consecutive_successes = 0;

            if old_level != state.level {
                if state.level == EscalationLevel::Normal {
                    state.restart_count = 0;
                }
                transition = Some((old_level, state.level));
            }
        }

        let level = self
            .components
            .get(component)
            .map(|s| s.level)
            .unwrap_or(EscalationLevel::Normal);

        if let Some((from, to)) = transition {
            self.record_transition(component, from, to, "recovery");
        }

        level
    }

    /// Get the current escalation level for a component.
    pub fn level(&self, component: &str) -> EscalationLevel {
        self.components
            .get(component)
            .map(|s| s.level)
            .unwrap_or(EscalationLevel::Normal)
    }

    /// Get the consecutive failure count for a component.
    pub fn failure_count(&self, component: &str) -> u32 {
        self.components
            .get(component)
            .map(|s| s.consecutive_failures)
            .unwrap_or(0)
    }

    /// Get the restart count for a component.
    pub fn restart_count(&self, component: &str) -> u32 {
        self.components
            .get(component)
            .map(|s| s.restart_count)
            .unwrap_or(0)
    }

    /// Return the history of escalation transitions.
    pub fn transitions(&self) -> &[EscalationTransition] {
        &self.transitions
    }

    /// Reset a component to Normal state.
    pub fn reset(&mut self, component: &str) {
        if let Some(state) = self.components.get_mut(component) {
            let old = state.level;
            *state = ComponentState::new();
            if old != EscalationLevel::Normal {
                self.record_transition(component, old, EscalationLevel::Normal, "manual reset");
            }
        }
    }

    fn record_transition(
        &mut self,
        component: &str,
        from: EscalationLevel,
        to: EscalationLevel,
        reason: &str,
    ) {
        if self.transitions.len() >= self.max_transitions {
            self.transitions.remove(0);
        }
        self.transitions.push(EscalationTransition {
            component: component.to_string(),
            from,
            to,
            reason: reason.to_string(),
            timestamp: Instant::now(),
        });
    }
}

impl Default for EscalationLadder {
    fn default() -> Self {
        Self::new(EscalationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder_with(warn: u32, degrade: u32, restart: u32, safe: u32) -> EscalationLadder {
        EscalationLadder::new(EscalationConfig {
            warn_threshold: warn,
            degrade_threshold: degrade,
            restart_threshold: restart,
            safe_mode_threshold: safe,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 2,
        })
    }

    #[test]
    fn no_failures_stays_normal() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        ladder.register("axis");
        assert_eq!(ladder.level("axis"), EscalationLevel::Normal);
    }

    #[test]
    fn single_failure_escalates_to_warn() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        let action = ladder.record_failure("axis", "tick missed");
        assert!(matches!(action, EscalationAction::Warn(_)));
        assert_eq!(ladder.level("axis"), EscalationLevel::Warn);
    }

    #[test]
    fn multiple_failures_escalate_to_degrade() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        for _ in 0..3 {
            ladder.record_failure("axis", "tick missed");
        }
        assert_eq!(ladder.level("axis"), EscalationLevel::Degrade);
    }

    #[test]
    fn continued_failures_escalate_to_restart() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        for _ in 0..5 {
            ladder.record_failure("axis", "tick missed");
        }
        assert_eq!(ladder.level("axis"), EscalationLevel::Restart);
        assert_eq!(ladder.restart_count("axis"), 1);
    }

    #[test]
    fn many_failures_escalate_to_safe_mode() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        for _ in 0..10 {
            ladder.record_failure("axis", "tick missed");
        }
        assert_eq!(ladder.level("axis"), EscalationLevel::SafeMode);
    }

    #[test]
    fn max_restarts_triggers_safe_mode() {
        let mut ladder = ladder_with(1, 3, 5, 100);
        // First restart
        for _ in 0..5 {
            ladder.record_failure("hid", "stall");
        }
        assert_eq!(ladder.level("hid"), EscalationLevel::Restart);
        // Recover briefly
        for _ in 0..3 {
            ladder.record_success("hid");
        }
        // Second restart
        for _ in 0..5 {
            ladder.record_failure("hid", "stall");
        }
        assert_eq!(ladder.restart_count("hid"), 2);
        // Third failure cycle should go to SafeMode since max_restart_attempts=2
        for _ in 0..5 {
            ladder.record_failure("hid", "stall");
        }
        assert_eq!(ladder.level("hid"), EscalationLevel::SafeMode);
    }

    #[test]
    fn success_de_escalates() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        // Escalate to Warn
        ladder.record_failure("bus", "slow");
        assert_eq!(ladder.level("bus"), EscalationLevel::Warn);
        // Recover
        for _ in 0..3 {
            ladder.record_success("bus");
        }
        assert_eq!(ladder.level("bus"), EscalationLevel::Normal);
    }

    #[test]
    fn transitions_are_recorded() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        ladder.record_failure("x", "err");
        assert_eq!(ladder.transitions().len(), 1);
        assert_eq!(ladder.transitions()[0].from, EscalationLevel::Normal);
        assert_eq!(ladder.transitions()[0].to, EscalationLevel::Warn);
    }

    #[test]
    fn reset_returns_to_normal() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        for _ in 0..5 {
            ladder.record_failure("x", "err");
        }
        assert_ne!(ladder.level("x"), EscalationLevel::Normal);
        ladder.reset("x");
        assert_eq!(ladder.level("x"), EscalationLevel::Normal);
        assert_eq!(ladder.failure_count("x"), 0);
    }

    #[test]
    fn unregistered_component_starts_normal() {
        let ladder = ladder_with(1, 3, 5, 10);
        assert_eq!(ladder.level("ghost"), EscalationLevel::Normal);
        assert_eq!(ladder.failure_count("ghost"), 0);
    }

    #[test]
    fn escalation_level_ordering() {
        assert!(EscalationLevel::Normal < EscalationLevel::Warn);
        assert!(EscalationLevel::Warn < EscalationLevel::Degrade);
        assert!(EscalationLevel::Degrade < EscalationLevel::Restart);
        assert!(EscalationLevel::Restart < EscalationLevel::SafeMode);
    }

    #[test]
    fn per_component_isolation() {
        let mut ladder = ladder_with(1, 3, 5, 10);
        for _ in 0..5 {
            ladder.record_failure("a", "err");
        }
        ladder.record_failure("b", "err");
        assert_eq!(ladder.level("a"), EscalationLevel::Restart);
        assert_eq!(ladder.level("b"), EscalationLevel::Warn);
    }
}
