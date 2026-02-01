// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! MSFS Sanity Gate state machine for telemetry validation
//!
//! The Sanity Gate validates telemetry plausibility and controls the safe_for_ffb flag
//! through a state machine that ensures FFB is only enabled during stable, active flight.
//!
//! Requirements: MSFS-INT-01.9 through MSFS-INT-01.16

use flight_bus::snapshot::BusSnapshot;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Sanity Gate state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SanityState {
    /// Disconnected from MSFS
    Disconnected,
    /// SimConnect connected, waiting for valid telemetry
    Booting,
    /// Valid telemetry received, waiting for stable flight state
    Loading,
    /// In active flight with stable telemetry
    ActiveFlight,
    /// Sim is paused
    Paused,
    /// Sanity violations exceeded threshold
    Faulted,
}

/// Configuration for the Sanity Gate
#[derive(Debug, Clone)]
pub struct SanityGateConfig {
    /// Number of consecutive valid frames required before transitioning to ActiveFlight
    pub stable_frames_required: u32,
    /// Maximum sanity violations before transitioning to Faulted
    pub violation_threshold: u32,
    /// Time window for violation counting (seconds)
    pub violation_window_secs: f64,
    /// Minimum interval between violation log messages (seconds)
    pub log_rate_limit_secs: f64,
    /// Maximum allowed attitude change per frame (radians)
    pub max_attitude_change_rad: f32,
    /// Maximum allowed velocity change per frame (m/s)
    pub max_velocity_change_mps: f32,
}

impl Default for SanityGateConfig {
    fn default() -> Self {
        Self {
            stable_frames_required: 10,
            violation_threshold: 10,
            violation_window_secs: 5.0,
            log_rate_limit_secs: 5.0,
            max_attitude_change_rad: std::f32::consts::PI, // 180 deg/s at 60Hz = 3 rad/frame
            max_velocity_change_mps: 100.0,                // ~200 knots/s at 60Hz
        }
    }
}

/// Minimal previous state for sanity checks
#[derive(Debug, Clone)]
struct PreviousState {
    pitch: f32,
    bank: f32,
    heading: f32,
    ias: f32,
    tas: f32,
    timestamp: u64,
}

/// Sanity Gate implementation
pub struct SanityGate {
    /// Current state
    state: SanityState,
    /// Configuration
    config: SanityGateConfig,
    /// Previous telemetry state for jump detection
    previous_state: Option<PreviousState>,
    /// Count of consecutive stable frames
    stable_frame_count: u32,
    /// Sanity violation counter
    violation_count: u32,
    /// Timestamps of recent violations for windowed counting
    violation_timestamps: Vec<Instant>,
    /// Last time a violation was logged
    last_violation_log: Instant,
    /// Whether sim is currently paused
    sim_paused: bool,
}

impl SanityGate {
    /// Create a new Sanity Gate with default configuration
    pub fn new() -> Self {
        Self::with_config(SanityGateConfig::default())
    }

    /// Create a new Sanity Gate with custom configuration
    pub fn with_config(config: SanityGateConfig) -> Self {
        Self {
            state: SanityState::Disconnected,
            config,
            previous_state: None,
            stable_frame_count: 0,
            violation_count: 0,
            violation_timestamps: Vec::new(),
            last_violation_log: Instant::now(),
            sim_paused: false,
        }
    }

    /// Get current state
    pub fn state(&self) -> SanityState {
        self.state
    }

    /// Get violation count
    pub fn violation_count(&self) -> u32 {
        self.violation_count
    }

    /// Reset the sanity gate (e.g., on reconnection)
    pub fn reset(&mut self) {
        self.state = SanityState::Disconnected;
        self.previous_state = None;
        self.stable_frame_count = 0;
        self.violation_count = 0;
        self.violation_timestamps.clear();
        self.sim_paused = false;
    }

    /// Transition to Booting state (SimConnect connected)
    pub fn transition_to_booting(&mut self) {
        debug!("Sanity Gate: Disconnected -> Booting");
        self.state = SanityState::Booting;
        self.previous_state = None;
        self.stable_frame_count = 0;
    }

    /// Transition to Disconnected state
    pub fn transition_to_disconnected(&mut self) {
        debug!(
            "Sanity Gate: {} -> Disconnected",
            format!("{:?}", self.state)
        );
        self.reset();
    }

    /// Set sim paused state
    pub fn set_sim_paused(&mut self, paused: bool) {
        if paused != self.sim_paused {
            self.sim_paused = paused;

            if paused && self.state == SanityState::ActiveFlight {
                debug!("Sanity Gate: ActiveFlight -> Paused");
                self.state = SanityState::Paused;
            } else if !paused && self.state == SanityState::Paused {
                debug!("Sanity Gate: Paused -> ActiveFlight");
                self.state = SanityState::ActiveFlight;
            }
        }
    }

    /// Check and update snapshot with sanity validation
    ///
    /// This is the main entry point for sanity checking. It:
    /// 1. Validates telemetry for NaN/Inf
    /// 2. Checks for physically implausible jumps
    /// 3. Updates state machine
    /// 4. Sets safe_for_ffb flag
    ///
    /// Requirements: MSFS-INT-01.9 through MSFS-INT-01.16
    pub fn check(&mut self, snapshot: &mut BusSnapshot) {
        // Check for NaN/Inf
        if self.has_nan_or_inf(snapshot) {
            self.record_violation("NaN or Inf detected in telemetry");
            self.mark_invalid(snapshot);
            return;
        }

        // Check for physically implausible jumps
        if let Some(ref prev) = self.previous_state
            && self.has_implausible_jump(snapshot, prev)
        {
            self.record_violation("Physically implausible telemetry jump detected");
            self.mark_invalid(snapshot);
            return;
        }

        // Update state machine
        self.update_state_machine(snapshot);

        // Set safe_for_ffb flag based on current state
        snapshot.validity.safe_for_ffb = self.state == SanityState::ActiveFlight;

        // Store current state for next check
        self.store_current_state(snapshot);
    }

    /// Check if snapshot contains NaN or Inf values
    fn has_nan_or_inf(&self, snapshot: &BusSnapshot) -> bool {
        // Check kinematics
        if !snapshot.kinematics.pitch.to_radians().is_finite()
            || !snapshot.kinematics.bank.to_radians().is_finite()
            || !snapshot.kinematics.heading.to_radians().is_finite()
        {
            return true;
        }

        if !snapshot.kinematics.ias.to_mps().is_finite()
            || !snapshot.kinematics.tas.to_mps().is_finite()
            || !snapshot.kinematics.ground_speed.to_mps().is_finite()
        {
            return true;
        }

        if !snapshot.kinematics.g_force.value().is_finite()
            || !snapshot.kinematics.g_lateral.value().is_finite()
            || !snapshot.kinematics.g_longitudinal.value().is_finite()
        {
            return true;
        }

        // Check angular rates
        if !snapshot.angular_rates.p.is_finite()
            || !snapshot.angular_rates.q.is_finite()
            || !snapshot.angular_rates.r.is_finite()
        {
            return true;
        }

        // Check environment
        if !snapshot.environment.altitude.is_finite() || !snapshot.environment.oat.is_finite() {
            return true;
        }

        false
    }

    /// Check for physically implausible jumps in telemetry
    fn has_implausible_jump(&self, snapshot: &BusSnapshot, prev: &PreviousState) -> bool {
        let dt = if snapshot.timestamp > prev.timestamp {
            (snapshot.timestamp - prev.timestamp) as f64 / 1e9
        } else {
            return false; // Can't check if time went backwards
        };

        if dt <= 0.0 || dt > 1.0 {
            // Skip check if dt is invalid or too large (missed frames)
            return false;
        }

        // Check attitude changes
        let d_pitch = (snapshot.kinematics.pitch.to_radians() - prev.pitch).abs();
        let d_bank = (snapshot.kinematics.bank.to_radians() - prev.bank).abs();
        let d_heading = angle_diff(snapshot.kinematics.heading.to_radians(), prev.heading);

        if d_pitch / dt as f32 > self.config.max_attitude_change_rad
            || d_bank / dt as f32 > self.config.max_attitude_change_rad
            || d_heading / dt as f32 > self.config.max_attitude_change_rad
        {
            return true;
        }

        // Check velocity changes
        let d_ias = (snapshot.kinematics.ias.to_mps() - prev.ias).abs();
        let d_tas = (snapshot.kinematics.tas.to_mps() - prev.tas).abs();

        if d_ias / dt as f32 > self.config.max_velocity_change_mps
            || d_tas / dt as f32 > self.config.max_velocity_change_mps
        {
            return true;
        }

        false
    }

    /// Update state machine based on telemetry
    fn update_state_machine(&mut self, snapshot: &BusSnapshot) {
        match self.state {
            SanityState::Disconnected => {
                // Should be transitioned externally via transition_to_booting()
            }
            SanityState::Booting => {
                // Transition to Loading once we have valid core telemetry
                if self.has_valid_core_telemetry(snapshot) {
                    debug!("Sanity Gate: Booting -> Loading");
                    self.state = SanityState::Loading;
                    self.stable_frame_count = 0;
                }
            }
            SanityState::Loading => {
                // Check if telemetry is stable
                if self.is_telemetry_stable(snapshot) {
                    self.stable_frame_count += 1;

                    if self.stable_frame_count >= self.config.stable_frames_required {
                        debug!(
                            "Sanity Gate: Loading -> ActiveFlight (after {} stable frames)",
                            self.stable_frame_count
                        );
                        self.state = SanityState::ActiveFlight;
                    }
                } else {
                    // Reset counter if telemetry becomes unstable
                    self.stable_frame_count = 0;
                }
            }
            SanityState::ActiveFlight => {
                // Check for pause
                if self.sim_paused {
                    debug!("Sanity Gate: ActiveFlight -> Paused");
                    self.state = SanityState::Paused;
                }
            }
            SanityState::Paused => {
                // Check for resume
                if !self.sim_paused {
                    debug!("Sanity Gate: Paused -> ActiveFlight");
                    self.state = SanityState::ActiveFlight;
                }
            }
            SanityState::Faulted => {
                // Remain in Faulted state until explicit reset
            }
        }
    }

    /// Check if snapshot has valid core telemetry
    fn has_valid_core_telemetry(&self, snapshot: &BusSnapshot) -> bool {
        // Check that essential fields are marked valid
        snapshot.validity.attitude_valid
            && snapshot.validity.velocities_valid
            && snapshot.validity.kinematics_valid
    }

    /// Check if telemetry is stable (no large changes)
    fn is_telemetry_stable(&self, snapshot: &BusSnapshot) -> bool {
        // For now, consider telemetry stable if it has valid core fields
        // More sophisticated stability checks could be added here
        self.has_valid_core_telemetry(snapshot)
            && snapshot.kinematics.ias.to_mps() > 0.0 // Some forward speed
            && snapshot.environment.altitude > -1000.0 // Reasonable altitude
    }

    /// Record a sanity violation
    fn record_violation(&mut self, reason: &str) {
        let now = Instant::now();

        // Add to violation timestamps
        self.violation_timestamps.push(now);

        // Remove old violations outside the window
        let window = Duration::from_secs_f64(self.config.violation_window_secs);
        self.violation_timestamps
            .retain(|&ts| now.duration_since(ts) < window);

        // Update violation count
        self.violation_count = self.violation_timestamps.len() as u32;

        // Rate-limited logging
        if now.duration_since(self.last_violation_log)
            > Duration::from_secs_f64(self.config.log_rate_limit_secs)
        {
            warn!(
                "Sanity violation: {} (count: {} in last {:.1}s)",
                reason, self.violation_count, self.config.violation_window_secs
            );
            self.last_violation_log = now;
        }

        // Check if we should transition to Faulted
        if self.violation_count >= self.config.violation_threshold {
            warn!(
                "Sanity Gate: {} -> Faulted (violation threshold exceeded)",
                format!("{:?}", self.state)
            );
            self.state = SanityState::Faulted;
        }
    }

    /// Mark snapshot as invalid
    fn mark_invalid(&mut self, snapshot: &mut BusSnapshot) {
        snapshot.validity.safe_for_ffb = false;
        snapshot.validity.attitude_valid = false;
        snapshot.validity.angular_rates_valid = false;
        snapshot.validity.velocities_valid = false;
        snapshot.validity.kinematics_valid = false;
        snapshot.validity.aero_valid = false;
    }

    /// Store current state for next check
    fn store_current_state(&mut self, snapshot: &BusSnapshot) {
        self.previous_state = Some(PreviousState {
            pitch: snapshot.kinematics.pitch.to_radians(),
            bank: snapshot.kinematics.bank.to_radians(),
            heading: snapshot.kinematics.heading.to_radians(),
            ias: snapshot.kinematics.ias.to_mps(),
            tas: snapshot.kinematics.tas.to_mps(),
            timestamp: snapshot.timestamp,
        });
    }
}

impl Default for SanityGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate the smallest angle difference between two angles (handling wraparound)
fn angle_diff(a: f32, b: f32) -> f32 {
    let diff = (a - b).abs();
    if diff > std::f32::consts::PI {
        2.0 * std::f32::consts::PI - diff
    } else {
        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::types::{
        AircraftId, GForce, Mach, Percentage, SimId, ValidatedAngle, ValidatedSpeed,
    };

    fn create_test_snapshot() -> BusSnapshot {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Set valid core telemetry
        snapshot.kinematics.pitch = ValidatedAngle::new_degrees(5.0).unwrap();
        snapshot.kinematics.bank = ValidatedAngle::new_degrees(0.0).unwrap();
        snapshot.kinematics.heading = ValidatedAngle::new_degrees(90.0).unwrap();
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(120.0).unwrap();
        snapshot.kinematics.tas = ValidatedSpeed::new_knots(125.0).unwrap();
        snapshot.kinematics.ground_speed = ValidatedSpeed::new_knots(120.0).unwrap();
        snapshot.kinematics.g_force = GForce::new(1.0).unwrap();
        snapshot.kinematics.g_lateral = GForce::new(0.0).unwrap();
        snapshot.kinematics.g_longitudinal = GForce::new(0.0).unwrap();
        snapshot.kinematics.mach = Mach::new(0.18).unwrap();

        snapshot.angular_rates.p = 0.0;
        snapshot.angular_rates.q = 0.0;
        snapshot.angular_rates.r = 0.0;

        snapshot.environment.altitude = 5000.0;
        snapshot.environment.oat = 15.0;

        snapshot.validity.attitude_valid = true;
        snapshot.validity.velocities_valid = true;
        snapshot.validity.kinematics_valid = true;

        snapshot
    }

    #[test]
    fn test_sanity_gate_creation() {
        let gate = SanityGate::new();
        assert_eq!(gate.state(), SanityState::Disconnected);
        assert_eq!(gate.violation_count(), 0);
    }

    #[test]
    fn test_state_transition_booting() {
        let mut gate = SanityGate::new();
        assert_eq!(gate.state(), SanityState::Disconnected);

        gate.transition_to_booting();
        assert_eq!(gate.state(), SanityState::Booting);
    }

    #[test]
    fn test_state_transition_disconnected() {
        let mut gate = SanityGate::new();
        gate.transition_to_booting();

        gate.transition_to_disconnected();
        assert_eq!(gate.state(), SanityState::Disconnected);
        assert_eq!(gate.violation_count(), 0);
    }

    #[test]
    fn test_nan_detection() {
        let mut gate = SanityGate::new();
        gate.transition_to_booting();

        let mut snapshot = create_test_snapshot();
        snapshot.angular_rates.p = f32::NAN;

        gate.check(&mut snapshot);

        assert!(!snapshot.validity.safe_for_ffb);
        assert!(gate.violation_count() > 0);
    }

    #[test]
    fn test_inf_detection() {
        let mut gate = SanityGate::new();
        gate.transition_to_booting();

        let mut snapshot = create_test_snapshot();
        snapshot.angular_rates.q = f32::INFINITY;

        gate.check(&mut snapshot);

        assert!(!snapshot.validity.safe_for_ffb);
        assert!(gate.violation_count() > 0);
    }

    #[test]
    fn test_angle_diff() {
        // Test normal case
        assert!((angle_diff(0.5, 0.3) - 0.2).abs() < 0.001);

        // Test wraparound
        let diff = angle_diff(0.1, 6.2); // Near 0 and near 2π
        assert!(diff < 0.2); // Should be small, not ~6.1
    }
}
