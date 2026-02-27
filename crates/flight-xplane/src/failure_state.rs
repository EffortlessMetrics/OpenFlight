// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane failure state tracking (REQ-783)
//!
//! Maps X-Plane dataref paths to structured failure state information.
//! Each failure field corresponds to a well-known X-Plane failure dataref
//! that reports `0` for normal and a non-zero value for active failure.

/// Dataref paths for failure state detection.
pub const FAILURE_DATAREFS: &[(&str, &str)] = &[
    ("sim/operation/failures/rel_engfai0", "engine_failure"),
    ("sim/operation/failures/rel_elefai", "electrical_failure"),
    ("sim/operation/failures/rel_hydpmp", "hydraulic_failure"),
    ("sim/operation/failures/rel_vacfai", "vacuum_failure"),
    ("sim/operation/failures/rel_pitfai", "pitot_failure"),
    ("sim/operation/failures/rel_ss_ahz", "attitude_indicator_failure"),
    ("sim/operation/failures/rel_g_fuel", "fuel_system_failure"),
    ("sim/operation/failures/rel_lbrake", "landing_gear_failure"),
];

/// Tracked failure states for an X-Plane session.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FailureState {
    /// Engine failure (any engine).
    pub engine_failure: bool,
    /// Electrical system failure.
    pub electrical_failure: bool,
    /// Hydraulic system failure.
    pub hydraulic_failure: bool,
    /// Vacuum system failure.
    pub vacuum_failure: bool,
    /// Pitot-static system failure.
    pub pitot_failure: bool,
    /// Attitude indicator failure.
    pub attitude_indicator_failure: bool,
    /// Fuel system failure.
    pub fuel_system_failure: bool,
    /// Landing gear failure.
    pub landing_gear_failure: bool,
}

impl FailureState {
    /// Returns `true` if any failure is currently active.
    pub fn is_any_failure_active(&self) -> bool {
        self.engine_failure
            || self.electrical_failure
            || self.hydraulic_failure
            || self.vacuum_failure
            || self.pitot_failure
            || self.attitude_indicator_failure
            || self.fuel_system_failure
            || self.landing_gear_failure
    }

    /// Count of currently active failures.
    pub fn active_failure_count(&self) -> usize {
        [
            self.engine_failure,
            self.electrical_failure,
            self.hydraulic_failure,
            self.vacuum_failure,
            self.pitot_failure,
            self.attitude_indicator_failure,
            self.fuel_system_failure,
            self.landing_gear_failure,
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }

    /// Update failure state from a dataref path and its value.
    ///
    /// A non-zero value indicates the failure is active.  Unknown dataref
    /// paths are silently ignored.
    pub fn update_from_dataref(&mut self, dataref_path: &str, value: f32) {
        let active = value != 0.0;
        match dataref_path {
            "sim/operation/failures/rel_engfai0" => self.engine_failure = active,
            "sim/operation/failures/rel_elefai" => self.electrical_failure = active,
            "sim/operation/failures/rel_hydpmp" => self.hydraulic_failure = active,
            "sim/operation/failures/rel_vacfai" => self.vacuum_failure = active,
            "sim/operation/failures/rel_pitfai" => self.pitot_failure = active,
            "sim/operation/failures/rel_ss_ahz" => self.attitude_indicator_failure = active,
            "sim/operation/failures/rel_g_fuel" => self.fuel_system_failure = active,
            "sim/operation/failures/rel_lbrake" => self.landing_gear_failure = active,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_no_failures() {
        let state = FailureState::default();
        assert!(!state.is_any_failure_active());
        assert_eq!(state.active_failure_count(), 0);
    }

    #[test]
    fn test_engine_failure_detection() {
        let mut state = FailureState::default();
        state.update_from_dataref("sim/operation/failures/rel_engfai0", 6.0);
        assert!(state.engine_failure);
        assert!(state.is_any_failure_active());
        assert_eq!(state.active_failure_count(), 1);
    }

    #[test]
    fn test_electrical_failure_detection() {
        let mut state = FailureState::default();
        state.update_from_dataref("sim/operation/failures/rel_elefai", 1.0);
        assert!(state.electrical_failure);
        assert!(state.is_any_failure_active());
    }

    #[test]
    fn test_hydraulic_failure_detection() {
        let mut state = FailureState::default();
        state.update_from_dataref("sim/operation/failures/rel_hydpmp", 1.0);
        assert!(state.hydraulic_failure);
        assert!(state.is_any_failure_active());
    }

    #[test]
    fn test_multiple_failures() {
        let mut state = FailureState::default();
        state.update_from_dataref("sim/operation/failures/rel_engfai0", 6.0);
        state.update_from_dataref("sim/operation/failures/rel_pitfai", 1.0);
        state.update_from_dataref("sim/operation/failures/rel_lbrake", 1.0);
        assert_eq!(state.active_failure_count(), 3);
        assert!(state.engine_failure);
        assert!(state.pitot_failure);
        assert!(state.landing_gear_failure);
    }

    #[test]
    fn test_failure_cleared_on_zero() {
        let mut state = FailureState::default();
        state.update_from_dataref("sim/operation/failures/rel_engfai0", 6.0);
        assert!(state.engine_failure);
        state.update_from_dataref("sim/operation/failures/rel_engfai0", 0.0);
        assert!(!state.engine_failure);
        assert!(!state.is_any_failure_active());
    }

    #[test]
    fn test_unknown_dataref_ignored() {
        let mut state = FailureState::default();
        state.update_from_dataref("sim/unknown/dataref", 1.0);
        assert!(!state.is_any_failure_active());
    }

    #[test]
    fn test_all_failure_types() {
        let mut state = FailureState::default();
        for (path, _) in FAILURE_DATAREFS {
            state.update_from_dataref(path, 1.0);
        }
        assert_eq!(state.active_failure_count(), FAILURE_DATAREFS.len());
        assert!(state.is_any_failure_active());
    }

    #[test]
    fn test_failure_datarefs_count() {
        assert_eq!(FAILURE_DATAREFS.len(), 8);
    }
}
