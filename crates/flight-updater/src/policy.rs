// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Update policy engine — decides whether an available update should be
//! applied now, deferred, or skipped entirely.

use crate::channels::Channel;
use crate::manifest::SemVer;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// Governs when and how updates are applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePolicy {
    /// How often the updater checks for new versions.
    #[serde(with = "duration_secs")]
    pub check_interval: Duration,
    /// When `true`, updates are installed without user confirmation.
    pub auto_apply: bool,
    /// Only updates from these channels are considered.
    pub allowed_channels: Vec<Channel>,
    /// When `true`, updates are postponed while a simulator is running.
    pub defer_while_sim_running: bool,
}

impl Default for UpdatePolicy {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(6 * 3600), // 6 hours
            auto_apply: false,
            allowed_channels: vec![Channel::Stable],
            defer_while_sim_running: true,
        }
    }
}

// ---------------------------------------------------------------------------
// CurrentState — snapshot of the system at decision time
// ---------------------------------------------------------------------------

/// A point-in-time snapshot the policy engine uses to make its decision.
#[derive(Debug, Clone)]
pub struct CurrentState {
    /// The installed version.
    pub installed_version: SemVer,
    /// Whether a supported simulator is currently running.
    pub sim_running: bool,
    /// The channel of the candidate update.
    pub update_channel: Channel,
    /// The version offered by the candidate update.
    pub update_version: SemVer,
    /// Minimum version the update requires (`None` = no constraint).
    pub update_min_version: Option<SemVer>,
}

// ---------------------------------------------------------------------------
// UpdateDecision
// ---------------------------------------------------------------------------

/// The outcome of the policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    /// The update should be applied immediately.
    Apply,
    /// The update should be postponed.
    Defer(String),
    /// The update should be skipped entirely.
    Skip(String),
}

impl fmt::Display for UpdateDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Apply => write!(f, "Apply"),
            Self::Defer(r) => write!(f, "Defer: {r}"),
            Self::Skip(r) => write!(f, "Skip: {r}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Decision logic
// ---------------------------------------------------------------------------

/// Evaluate the policy against the current system state.
pub fn should_apply(policy: &UpdatePolicy, state: &CurrentState) -> UpdateDecision {
    // 1. Channel not allowed → Skip
    if !policy.allowed_channels.contains(&state.update_channel) {
        return UpdateDecision::Skip(format!(
            "channel {} is not in the allowed list",
            state.update_channel
        ));
    }

    // 2. Already at or ahead of the offered version → Skip
    if state.installed_version >= state.update_version {
        return UpdateDecision::Skip("installed version is already up to date".into());
    }

    // 3. Installed version below the update's min_version → Skip
    if let Some(ref min) = state.update_min_version {
        if state.installed_version < *min {
            return UpdateDecision::Skip(format!(
                "installed version {} is below the required minimum {}",
                state.installed_version, min
            ));
        }
    }

    // 4. Sim running + policy says defer → Defer
    if state.sim_running && policy.defer_while_sim_running {
        return UpdateDecision::Defer("a simulator is currently running".into());
    }

    // 5. auto_apply is off → Defer (wait for user confirmation)
    if !policy.auto_apply {
        return UpdateDecision::Defer("auto-apply is disabled; awaiting user confirmation".into());
    }

    UpdateDecision::Apply
}

// ---------------------------------------------------------------------------
// Serde helper for Duration as seconds
// ---------------------------------------------------------------------------

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers ----------------------------------------------------------

    fn default_state() -> CurrentState {
        CurrentState {
            installed_version: SemVer::new(1, 0, 0),
            sim_running: false,
            update_channel: Channel::Stable,
            update_version: SemVer::new(2, 0, 0),
            update_min_version: None,
        }
    }

    fn auto_policy() -> UpdatePolicy {
        UpdatePolicy {
            auto_apply: true,
            ..Default::default()
        }
    }

    // -- default policy ---------------------------------------------------

    #[test]
    fn default_policy_values() {
        let p = UpdatePolicy::default();
        assert_eq!(p.check_interval, Duration::from_secs(6 * 3600));
        assert!(!p.auto_apply);
        assert_eq!(p.allowed_channels, vec![Channel::Stable]);
        assert!(p.defer_while_sim_running);
    }

    // -- Apply path -------------------------------------------------------

    #[test]
    fn apply_when_auto_and_channel_allowed() {
        let decision = should_apply(&auto_policy(), &default_state());
        assert_eq!(decision, UpdateDecision::Apply);
    }

    // -- Skip: channel not allowed ----------------------------------------

    #[test]
    fn skip_when_channel_not_allowed() {
        let state = CurrentState {
            update_channel: Channel::Canary,
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert!(matches!(decision, UpdateDecision::Skip(_)));
    }

    // -- Skip: already up to date -----------------------------------------

    #[test]
    fn skip_when_already_at_version() {
        let state = CurrentState {
            installed_version: SemVer::new(2, 0, 0),
            update_version: SemVer::new(2, 0, 0),
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert!(matches!(decision, UpdateDecision::Skip(_)));
    }

    #[test]
    fn skip_when_ahead_of_update() {
        let state = CurrentState {
            installed_version: SemVer::new(3, 0, 0),
            update_version: SemVer::new(2, 0, 0),
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert!(matches!(decision, UpdateDecision::Skip(_)));
    }

    // -- Skip: below min_version ------------------------------------------

    #[test]
    fn skip_when_below_min_version() {
        let state = CurrentState {
            installed_version: SemVer::new(0, 9, 0),
            update_min_version: Some(SemVer::new(1, 0, 0)),
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert!(matches!(decision, UpdateDecision::Skip(_)));
    }

    #[test]
    fn apply_when_at_min_version() {
        let state = CurrentState {
            update_min_version: Some(SemVer::new(1, 0, 0)),
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert_eq!(decision, UpdateDecision::Apply);
    }

    // -- Defer: sim running -----------------------------------------------

    #[test]
    fn defer_when_sim_running_and_policy_defers() {
        let state = CurrentState {
            sim_running: true,
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert!(matches!(decision, UpdateDecision::Defer(_)));
    }

    #[test]
    fn apply_when_sim_running_but_policy_allows() {
        let policy = UpdatePolicy {
            auto_apply: true,
            defer_while_sim_running: false,
            ..Default::default()
        };
        let state = CurrentState {
            sim_running: true,
            ..default_state()
        };
        assert_eq!(should_apply(&policy, &state), UpdateDecision::Apply);
    }

    // -- Defer: auto_apply off --------------------------------------------

    #[test]
    fn defer_when_auto_apply_off() {
        let policy = UpdatePolicy::default(); // auto_apply = false
        let decision = should_apply(&policy, &default_state());
        assert!(matches!(decision, UpdateDecision::Defer(_)));
    }

    // -- Channel variations -----------------------------------------------

    #[test]
    fn apply_beta_when_beta_allowed() {
        let policy = UpdatePolicy {
            auto_apply: true,
            allowed_channels: vec![Channel::Stable, Channel::Beta],
            ..Default::default()
        };
        let state = CurrentState {
            update_channel: Channel::Beta,
            ..default_state()
        };
        assert_eq!(should_apply(&policy, &state), UpdateDecision::Apply);
    }

    #[test]
    fn skip_beta_when_only_stable_allowed() {
        let state = CurrentState {
            update_channel: Channel::Beta,
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert!(matches!(decision, UpdateDecision::Skip(_)));
    }

    // -- Serde round-trip -------------------------------------------------

    #[test]
    fn policy_serde_roundtrip() {
        let policy = UpdatePolicy {
            check_interval: Duration::from_secs(3600),
            auto_apply: true,
            allowed_channels: vec![Channel::Stable, Channel::Canary],
            defer_while_sim_running: false,
        };
        let json = serde_json::to_string(&policy).unwrap();
        let back: UpdatePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, back);
    }

    // -- Display ----------------------------------------------------------

    #[test]
    fn decision_display() {
        assert_eq!(UpdateDecision::Apply.to_string(), "Apply");
        assert!(
            UpdateDecision::Defer("reason".into())
                .to_string()
                .contains("Defer")
        );
        assert!(
            UpdateDecision::Skip("reason".into())
                .to_string()
                .contains("Skip")
        );
    }

    // -- Priority ordering (sim running check before auto_apply) ----------

    #[test]
    fn sim_running_takes_precedence_over_auto_apply_off() {
        // Both conditions would produce Defer, but sim-running should fire first
        let policy = UpdatePolicy {
            auto_apply: false,
            defer_while_sim_running: true,
            ..Default::default()
        };
        let state = CurrentState {
            sim_running: true,
            ..default_state()
        };
        let decision = should_apply(&policy, &state);
        match decision {
            UpdateDecision::Defer(reason) => {
                assert!(
                    reason.contains("simulator"),
                    "reason should mention simulator: {reason}"
                );
            }
            other => panic!("expected Defer, got {other:?}"),
        }
    }

    // -- min_version = None means no constraint ---------------------------

    #[test]
    fn no_min_version_constraint() {
        let state = CurrentState {
            installed_version: SemVer::new(0, 1, 0),
            update_min_version: None,
            ..default_state()
        };
        let decision = should_apply(&auto_policy(), &state);
        assert_eq!(decision, UpdateDecision::Apply);
    }
}
