// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS mission phase state machine (REQ-824)
//!
//! Determines the current mission phase from DCS export telemetry (mission
//! time, altitude AGL, ground speed) and tracks transitions through a
//! simple state machine.

/// Mission phases in a typical DCS sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissionPhase {
    /// Pre-flight briefing or mission not yet started.
    Briefing,
    /// Taxiing on the ground.
    Taxi,
    /// Takeoff roll / initial climb.
    Takeoff,
    /// Enroute / cruise.
    Enroute,
    /// Combat engagement.
    Combat,
    /// Approach and landing.
    Landing,
    /// Parked / engines off.
    Parked,
}

impl std::fmt::Display for MissionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MissionPhase::Briefing => write!(f, "Briefing"),
            MissionPhase::Taxi => write!(f, "Taxi"),
            MissionPhase::Takeoff => write!(f, "Takeoff"),
            MissionPhase::Enroute => write!(f, "Enroute"),
            MissionPhase::Combat => write!(f, "Combat"),
            MissionPhase::Landing => write!(f, "Landing"),
            MissionPhase::Parked => write!(f, "Parked"),
        }
    }
}

/// Telemetry snapshot used to derive mission phase.
#[derive(Debug, Clone, PartialEq)]
pub struct MissionTelemetry {
    /// Mission elapsed time in seconds.
    pub mission_time: f64,
    /// Altitude above ground level in metres.
    pub altitude_agl: f64,
    /// Ground speed in metres per second.
    pub ground_speed: f64,
    /// Whether weapons have been released recently.
    pub weapons_active: bool,
}

/// Thresholds for phase detection.
const GROUND_ALT_THRESHOLD: f64 = 10.0; // metres AGL
const TAXI_SPEED_THRESHOLD: f64 = 5.0; // m/s (~10 kt)
const TAKEOFF_SPEED_THRESHOLD: f64 = 30.0; // m/s (~58 kt)
const LANDING_ALT_THRESHOLD: f64 = 150.0; // metres AGL
const PARKED_SPEED_THRESHOLD: f64 = 0.5; // m/s

/// State machine for tracking mission phase transitions.
#[derive(Debug, Clone)]
pub struct MissionStateMachine {
    current_phase: MissionPhase,
}

impl MissionStateMachine {
    /// Create a new state machine starting in the `Briefing` phase.
    pub fn new() -> Self {
        Self {
            current_phase: MissionPhase::Briefing,
        }
    }

    /// Current mission phase.
    pub fn phase(&self) -> MissionPhase {
        self.current_phase
    }

    /// Update the state machine with fresh telemetry and return the new phase.
    pub fn update(&mut self, telemetry: &MissionTelemetry) -> MissionPhase {
        let new_phase = self.detect_phase(telemetry);
        if self.is_valid_transition(new_phase) {
            self.current_phase = new_phase;
        }
        self.current_phase
    }

    /// Derive phase purely from telemetry values.
    fn detect_phase(&self, t: &MissionTelemetry) -> MissionPhase {
        // Combat takes priority when weapons are active and airborne
        if t.weapons_active && t.altitude_agl > GROUND_ALT_THRESHOLD {
            return MissionPhase::Combat;
        }

        let on_ground = t.altitude_agl <= GROUND_ALT_THRESHOLD;

        if on_ground {
            if t.ground_speed < PARKED_SPEED_THRESHOLD {
                if t.mission_time < 1.0 {
                    return MissionPhase::Briefing;
                }
                return MissionPhase::Parked;
            }
            if t.ground_speed < TAXI_SPEED_THRESHOLD {
                return MissionPhase::Taxi;
            }
            if t.ground_speed < TAKEOFF_SPEED_THRESHOLD {
                return MissionPhase::Taxi;
            }
            return MissionPhase::Takeoff;
        }

        // Airborne
        if t.altitude_agl < LANDING_ALT_THRESHOLD
            && matches!(
                self.current_phase,
                MissionPhase::Enroute | MissionPhase::Combat
            )
        {
            return MissionPhase::Landing;
        }

        MissionPhase::Enroute
    }

    /// Check whether transitioning from the current phase to `target` is valid.
    fn is_valid_transition(&self, target: MissionPhase) -> bool {
        use MissionPhase::*;
        matches!(
            (self.current_phase, target),
            (Briefing, Briefing)
                | (Briefing, Taxi)
                | (Briefing, Parked)
                | (Taxi, Taxi)
                | (Taxi, Takeoff)
                | (Taxi, Parked)
                | (Takeoff, Takeoff)
                | (Takeoff, Enroute)
                | (Enroute, Enroute)
                | (Enroute, Combat)
                | (Enroute, Landing)
                | (Combat, Combat)
                | (Combat, Enroute)
                | (Combat, Landing)
                | (Landing, Landing)
                | (Landing, Taxi)
                | (Landing, Parked)
                | (Parked, Parked)
                | (Parked, Taxi)
        )
    }

    /// Force-set the phase, bypassing transition validation.
    pub fn force_phase(&mut self, phase: MissionPhase) {
        self.current_phase = phase;
    }
}

impl Default for MissionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telem(mission_time: f64, alt: f64, speed: f64, weapons: bool) -> MissionTelemetry {
        MissionTelemetry {
            mission_time,
            altitude_agl: alt,
            ground_speed: speed,
            weapons_active: weapons,
        }
    }

    #[test]
    fn test_initial_phase_is_briefing() {
        let sm = MissionStateMachine::new();
        assert_eq!(sm.phase(), MissionPhase::Briefing);
    }

    #[test]
    fn test_briefing_to_taxi() {
        let mut sm = MissionStateMachine::new();
        let phase = sm.update(&telem(5.0, 0.0, 3.0, false));
        assert_eq!(phase, MissionPhase::Taxi);
    }

    #[test]
    fn test_taxi_to_takeoff() {
        let mut sm = MissionStateMachine::new();
        sm.force_phase(MissionPhase::Taxi);
        let phase = sm.update(&telem(30.0, 5.0, 35.0, false));
        assert_eq!(phase, MissionPhase::Takeoff);
    }

    #[test]
    fn test_takeoff_to_enroute() {
        let mut sm = MissionStateMachine::new();
        sm.force_phase(MissionPhase::Takeoff);
        let phase = sm.update(&telem(60.0, 500.0, 80.0, false));
        assert_eq!(phase, MissionPhase::Enroute);
    }

    #[test]
    fn test_enroute_to_combat() {
        let mut sm = MissionStateMachine::new();
        sm.force_phase(MissionPhase::Enroute);
        let phase = sm.update(&telem(120.0, 3000.0, 200.0, true));
        assert_eq!(phase, MissionPhase::Combat);
    }

    #[test]
    fn test_enroute_to_landing() {
        let mut sm = MissionStateMachine::new();
        sm.force_phase(MissionPhase::Enroute);
        let phase = sm.update(&telem(300.0, 100.0, 60.0, false));
        assert_eq!(phase, MissionPhase::Landing);
    }

    #[test]
    fn test_landing_to_parked() {
        let mut sm = MissionStateMachine::new();
        sm.force_phase(MissionPhase::Landing);
        let phase = sm.update(&telem(400.0, 0.0, 0.2, false));
        assert_eq!(phase, MissionPhase::Parked);
    }

    #[test]
    fn test_invalid_transition_blocked() {
        let mut sm = MissionStateMachine::new();
        // Briefing → Enroute is not a valid direct transition
        let phase = sm.update(&telem(0.5, 5000.0, 200.0, false));
        assert_eq!(phase, MissionPhase::Briefing);
    }

    #[test]
    fn test_combat_to_enroute() {
        let mut sm = MissionStateMachine::new();
        sm.force_phase(MissionPhase::Combat);
        let phase = sm.update(&telem(200.0, 3000.0, 180.0, false));
        assert_eq!(phase, MissionPhase::Enroute);
    }

    #[test]
    fn test_display_all_phases() {
        assert_eq!(MissionPhase::Briefing.to_string(), "Briefing");
        assert_eq!(MissionPhase::Taxi.to_string(), "Taxi");
        assert_eq!(MissionPhase::Takeoff.to_string(), "Takeoff");
        assert_eq!(MissionPhase::Enroute.to_string(), "Enroute");
        assert_eq!(MissionPhase::Combat.to_string(), "Combat");
        assert_eq!(MissionPhase::Landing.to_string(), "Landing");
        assert_eq!(MissionPhase::Parked.to_string(), "Parked");
    }

    #[test]
    fn test_force_phase() {
        let mut sm = MissionStateMachine::new();
        sm.force_phase(MissionPhase::Combat);
        assert_eq!(sm.phase(), MissionPhase::Combat);
    }
}
