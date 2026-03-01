// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Escalation policy that maps consecutive heartbeat misses to recovery actions.
//!
//! Default policy:
//!  - 1 miss   → Alert (warn)
//!  - 3 misses → RestartComponent
//!  - 5 misses → DegradeMode
//!  - 10 misses → Shutdown

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Recovery action the watchdog should take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Restart a specific subsystem.
    RestartComponent { component: String },
    /// Switch the system to safe / degraded mode.
    DegradeMode,
    /// Log or notify but don't take corrective action.
    Alert { message: String },
    /// Initiate graceful shutdown — irrecoverable failure.
    Shutdown { reason: String },
}

/// Configurable thresholds for the escalation policy.
#[derive(Debug, Clone)]
pub struct EscalationPolicyConfig {
    /// Consecutive misses before emitting a warning alert.
    pub alert_threshold: u32,
    /// Consecutive misses before restarting the component.
    pub restart_threshold: u32,
    /// Consecutive misses before entering degraded mode.
    pub degrade_threshold: u32,
    /// Consecutive misses before initiating shutdown.
    pub shutdown_threshold: u32,
}

impl Default for EscalationPolicyConfig {
    fn default() -> Self {
        Self {
            alert_threshold: 1,
            restart_threshold: 3,
            degrade_threshold: 5,
            shutdown_threshold: 10,
        }
    }
}

/// Per-component escalation state.
#[derive(Debug)]
struct ComponentEscalation {
    consecutive_misses: u32,
    last_action: Option<RecoveryStrategy>,
}

/// The escalation policy engine.
///
/// Tracks per-component miss counts and determines the appropriate
/// [`RecoveryStrategy`] based on configured thresholds.
pub struct EscalationPolicy {
    config: EscalationPolicyConfig,
    components: HashMap<String, ComponentEscalation>,
}

impl EscalationPolicy {
    /// Create a new policy with the given configuration.
    pub fn new(config: EscalationPolicyConfig) -> Self {
        Self {
            config,
            components: HashMap::new(),
        }
    }

    /// Register a component for tracking.
    pub fn register(&mut self, component: &str) {
        self.components.insert(
            component.to_string(),
            ComponentEscalation {
                consecutive_misses: 0,
                last_action: None,
            },
        );
    }

    /// Record a miss for a component and return the escalation action.
    pub fn record_miss(&mut self, component: &str) -> RecoveryStrategy {
        let state = self
            .components
            .entry(component.to_string())
            .or_insert(ComponentEscalation {
                consecutive_misses: 0,
                last_action: None,
            });

        state.consecutive_misses += 1;
        let misses = state.consecutive_misses;

        let action = if misses >= self.config.shutdown_threshold {
            RecoveryStrategy::Shutdown {
                reason: format!("{component}: {misses} consecutive misses — irrecoverable"),
            }
        } else if misses >= self.config.degrade_threshold {
            RecoveryStrategy::DegradeMode
        } else if misses >= self.config.restart_threshold {
            RecoveryStrategy::RestartComponent {
                component: component.to_string(),
            }
        } else if misses >= self.config.alert_threshold {
            RecoveryStrategy::Alert {
                message: format!("{component}: {misses} consecutive miss(es)"),
            }
        } else {
            RecoveryStrategy::Alert {
                message: format!("{component}: miss below threshold"),
            }
        };

        state.last_action = Some(action.clone());
        action
    }

    /// Record a recovery (successful heartbeat) for a component, resetting its count.
    pub fn record_recovery(&mut self, component: &str) {
        if let Some(state) = self.components.get_mut(component) {
            state.consecutive_misses = 0;
            state.last_action = None;
        }
    }

    /// Get the consecutive miss count for a component.
    pub fn miss_count(&self, component: &str) -> u32 {
        self.components
            .get(component)
            .map(|s| s.consecutive_misses)
            .unwrap_or(0)
    }

    /// Get the last action for a component.
    pub fn last_action(&self, component: &str) -> Option<&RecoveryStrategy> {
        self.components
            .get(component)
            .and_then(|s| s.last_action.as_ref())
    }
}

impl Default for EscalationPolicy {
    fn default() -> Self {
        Self::new(EscalationPolicyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> EscalationPolicy {
        EscalationPolicy::new(EscalationPolicyConfig::default())
    }

    #[test]
    fn first_miss_triggers_alert() {
        let mut policy = default_policy();
        let action = policy.record_miss("axis");
        assert!(matches!(action, RecoveryStrategy::Alert { .. }));
    }

    #[test]
    fn three_misses_trigger_restart() {
        let mut policy = default_policy();
        for _ in 0..3 {
            policy.record_miss("axis");
        }
        assert!(
            matches!(
                policy.last_action("axis"),
                Some(RecoveryStrategy::RestartComponent { .. })
            ),
            "3 misses should trigger RestartComponent"
        );
    }

    #[test]
    fn five_misses_trigger_degrade() {
        let mut policy = default_policy();
        for _ in 0..5 {
            policy.record_miss("ffb");
        }
        assert_eq!(
            policy.last_action("ffb"),
            Some(&RecoveryStrategy::DegradeMode)
        );
    }

    #[test]
    fn ten_misses_trigger_shutdown() {
        let mut policy = default_policy();
        for _ in 0..10 {
            policy.record_miss("adapter");
        }
        assert!(
            matches!(
                policy.last_action("adapter"),
                Some(RecoveryStrategy::Shutdown { .. })
            ),
            "10 misses should trigger Shutdown"
        );
    }

    #[test]
    fn recovery_resets_count() {
        let mut policy = default_policy();
        for _ in 0..4 {
            policy.record_miss("axis");
        }
        assert!(policy.miss_count("axis") > 0);
        policy.record_recovery("axis");
        assert_eq!(policy.miss_count("axis"), 0);
        assert!(policy.last_action("axis").is_none());
    }

    #[test]
    fn escalation_progression() {
        let mut policy = default_policy();

        // 1 → Alert
        let a = policy.record_miss("x");
        assert!(matches!(a, RecoveryStrategy::Alert { .. }));

        // 2 → Alert
        let a = policy.record_miss("x");
        assert!(matches!(a, RecoveryStrategy::Alert { .. }));

        // 3 → Restart
        let a = policy.record_miss("x");
        assert!(matches!(a, RecoveryStrategy::RestartComponent { .. }));

        // 4 → Restart
        let a = policy.record_miss("x");
        assert!(matches!(a, RecoveryStrategy::RestartComponent { .. }));

        // 5 → Degrade
        let a = policy.record_miss("x");
        assert_eq!(a, RecoveryStrategy::DegradeMode);

        // 6..9 → Degrade
        for _ in 6..10 {
            let a = policy.record_miss("x");
            assert_eq!(a, RecoveryStrategy::DegradeMode);
        }

        // 10 → Shutdown
        let a = policy.record_miss("x");
        assert!(matches!(a, RecoveryStrategy::Shutdown { .. }));
    }

    #[test]
    fn recovery_then_new_misses_restart_from_alert() {
        let mut policy = default_policy();
        for _ in 0..5 {
            policy.record_miss("y");
        }
        assert_eq!(
            policy.last_action("y"),
            Some(&RecoveryStrategy::DegradeMode)
        );

        policy.record_recovery("y");
        let a = policy.record_miss("y");
        assert!(
            matches!(a, RecoveryStrategy::Alert { .. }),
            "after recovery, first miss should be Alert again"
        );
    }

    #[test]
    fn per_component_isolation() {
        let mut policy = default_policy();
        for _ in 0..5 {
            policy.record_miss("a");
        }
        policy.record_miss("b");

        assert_eq!(
            policy.last_action("a"),
            Some(&RecoveryStrategy::DegradeMode)
        );
        assert!(matches!(
            policy.last_action("b"),
            Some(RecoveryStrategy::Alert { .. })
        ));
    }

    #[test]
    fn custom_thresholds() {
        let mut policy = EscalationPolicy::new(EscalationPolicyConfig {
            alert_threshold: 2,
            restart_threshold: 4,
            degrade_threshold: 6,
            shutdown_threshold: 8,
        });

        // 1 miss — below alert_threshold=2
        let a = policy.record_miss("c");
        assert!(matches!(a, RecoveryStrategy::Alert { .. }));

        // 2 misses — at alert_threshold
        let a = policy.record_miss("c");
        assert!(matches!(a, RecoveryStrategy::Alert { .. }));

        // 4 misses
        policy.record_miss("c");
        let a = policy.record_miss("c");
        assert!(matches!(a, RecoveryStrategy::RestartComponent { .. }));

        // 6 misses
        policy.record_miss("c");
        let a = policy.record_miss("c");
        assert_eq!(a, RecoveryStrategy::DegradeMode);

        // 8 misses
        policy.record_miss("c");
        let a = policy.record_miss("c");
        assert!(matches!(a, RecoveryStrategy::Shutdown { .. }));
    }

    #[test]
    fn unregistered_component_auto_creates() {
        let mut policy = default_policy();
        let a = policy.record_miss("new_component");
        assert!(matches!(a, RecoveryStrategy::Alert { .. }));
        assert_eq!(policy.miss_count("new_component"), 1);
    }
}
