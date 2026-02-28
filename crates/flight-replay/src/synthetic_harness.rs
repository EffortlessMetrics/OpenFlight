// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Synthetic telemetry harness for sim-disabled FFB testing
//!
//! This module provides a harness that generates synthetic BusSnapshot data
//! and feeds it into the FFB engine for testing without requiring a real simulator.

use anyhow::{Context, Result};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

use flight_bus::{
    AircraftConfig, AircraftId, BusSnapshot, Environment, GForce, Kinematics, LightsConfig, Mach,
    Navigation, SimId, ValidatedAngle, ValidatedSpeed,
    snapshot::{AngularRates, ControlInputs, TrimState, ValidityFlags},
};
use flight_ffb::{FfbConfig, FfbEngine, FfbMode, SafetyState};

/// Configuration for synthetic telemetry generation
#[derive(Debug, Clone)]
pub struct SyntheticHarnessConfig {
    /// Update rate in Hz
    pub update_rate_hz: u32,
    /// Duration to run the harness
    pub duration: Duration,
    /// FFB configuration
    pub ffb_config: FfbConfig,
    /// Telemetry pattern to generate
    pub pattern: TelemetryPattern,
}

/// Telemetry pattern for synthetic data generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryPattern {
    /// Steady level flight
    SteadyFlight,
    /// Gentle banking maneuver
    GentleBank,
    /// Pitch oscillation
    PitchOscillation,
    /// Combined roll and pitch
    CombinedManeuver,
    /// High-G turn
    HighGTurn,
}

/// Synthetic telemetry harness for FFB testing
pub struct SyntheticHarness {
    config: SyntheticHarnessConfig,
    ffb_engine: FfbEngine,
    start_time: Instant,
    frame_count: u64,
    last_snapshot: Option<BusSnapshot>,
}

impl SyntheticHarness {
    /// Create a new synthetic harness
    pub fn new(config: SyntheticHarnessConfig) -> Result<Self> {
        let ffb_engine =
            FfbEngine::new(config.ffb_config.clone()).context("Failed to create FFB engine")?;

        Ok(Self {
            config,
            ffb_engine,
            start_time: Instant::now(),
            frame_count: 0,
            last_snapshot: None,
        })
    }

    /// Run the harness for the configured duration
    pub fn run(&mut self) -> Result<HarnessResults> {
        info!(
            "Starting synthetic harness: pattern={:?}, rate={} Hz, duration={:?}",
            self.config.pattern, self.config.update_rate_hz, self.config.duration
        );

        let frame_interval = Duration::from_secs_f64(1.0 / self.config.update_rate_hz as f64);
        let mut next_frame_time = self.start_time;
        let end_time = self.start_time + self.config.duration;

        let mut results = HarnessResults::default();

        while Instant::now() < end_time {
            // Generate synthetic snapshot
            let snapshot = self.generate_snapshot()?;

            // Feed into FFB engine
            if let Err(e) = self.process_snapshot(&snapshot) {
                results.error_count += 1;
                debug!("FFB processing error: {}", e);
            } else {
                results.success_count += 1;
            }

            self.last_snapshot = Some(snapshot);
            self.frame_count += 1;

            // Wait for next frame
            next_frame_time += frame_interval;
            let now = Instant::now();
            if next_frame_time > now {
                std::thread::sleep(next_frame_time - now);
            } else {
                results.missed_frames += 1;
            }
        }

        results.total_frames = self.frame_count;
        results.duration = self.start_time.elapsed();
        results.final_safety_state = self.ffb_engine.safety_state();

        info!(
            "Synthetic harness completed: {} frames, {} errors, {} missed",
            results.total_frames, results.error_count, results.missed_frames
        );

        Ok(results)
    }

    /// Generate a synthetic snapshot based on the configured pattern
    fn generate_snapshot(&self) -> Result<BusSnapshot> {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let mut snapshot = BusSnapshot {
            sim: SimId::Unknown,
            aircraft: AircraftId::new("SYNTHETIC"),
            timestamp,
            kinematics: self.generate_kinematics(elapsed)?,
            angular_rates: self.generate_angular_rates(elapsed),
            config: AircraftConfig::default(),
            control_inputs: self.generate_control_inputs(elapsed),
            trim_state: TrimState::default(),
            helo: None,
            engines: Vec::new(),
            environment: Environment::default(),
            navigation: Navigation::default(),
            validity: ValidityFlags {
                safe_for_ffb: true,
                attitude_valid: true,
                angular_rates_valid: true,
                velocities_valid: true,
                kinematics_valid: true,
                aero_valid: true,
                position_valid: true,
            },
        };

        // Set lights to default
        snapshot.config.lights = LightsConfig::default();

        Ok(snapshot)
    }

    /// Generate kinematics based on the pattern
    fn generate_kinematics(&self, elapsed: f64) -> Result<Kinematics> {
        let (pitch, bank, heading) = match self.config.pattern {
            TelemetryPattern::SteadyFlight => (5.0, 0.0, 90.0),
            TelemetryPattern::GentleBank => {
                let bank_angle = 15.0 * (elapsed * 0.5).sin();
                (5.0, bank_angle, 90.0)
            }
            TelemetryPattern::PitchOscillation => {
                let pitch_angle = 5.0 + 10.0 * (elapsed * 0.3).sin();
                (pitch_angle, 0.0, 90.0)
            }
            TelemetryPattern::CombinedManeuver => {
                let pitch_angle = 5.0 + 8.0 * (elapsed * 0.4).sin();
                let bank_angle = 12.0 * (elapsed * 0.3).cos();
                (pitch_angle, bank_angle, 90.0)
            }
            TelemetryPattern::HighGTurn => {
                let bank_angle = 30.0 * (elapsed * 0.2).sin();
                let pitch_angle = 10.0 + 5.0 * (elapsed * 0.2).sin();
                (pitch_angle, bank_angle, 90.0)
            }
        };

        let g_force = match self.config.pattern {
            TelemetryPattern::HighGTurn => 1.0 + 1.5 * (elapsed * 0.2).sin().abs(),
            _ => 1.0 + 0.2 * (elapsed * 0.5).sin(),
        };

        Ok(Kinematics {
            ias: ValidatedSpeed::new_knots(120.0)?,
            tas: ValidatedSpeed::new_knots(125.0)?,
            ground_speed: ValidatedSpeed::new_knots(120.0)?,
            aoa: ValidatedAngle::new_degrees((5.0 + 2.0 * (elapsed * 0.3).sin()) as f32)?,
            sideslip: ValidatedAngle::new_degrees((0.5 * (elapsed * 0.4).cos()) as f32)?,
            bank: ValidatedAngle::new_degrees(bank as f32)?,
            pitch: ValidatedAngle::new_degrees(pitch as f32)?,
            heading: ValidatedAngle::new_degrees(heading)?,
            g_force: GForce::new(g_force as f32)?,
            g_lateral: GForce::new((0.1 * (elapsed * 0.3).sin()) as f32)?,
            g_longitudinal: GForce::new((0.05 * (elapsed * 0.2).cos()) as f32)?,
            mach: Mach::new(0.18)?,
            vertical_speed: 100.0 * (elapsed * 0.2).sin() as f32,
        })
    }

    /// Generate angular rates based on the pattern
    fn generate_angular_rates(&self, elapsed: f64) -> AngularRates {
        match self.config.pattern {
            TelemetryPattern::SteadyFlight => AngularRates {
                p: 0.01 * (elapsed * 0.5).sin() as f32,
                q: 0.01 * (elapsed * 0.3).cos() as f32,
                r: 0.005 * (elapsed * 0.2).sin() as f32,
            },
            TelemetryPattern::GentleBank => AngularRates {
                p: 0.1 * (elapsed * 0.5).cos() as f32,
                q: 0.02 * (elapsed * 0.3).sin() as f32,
                r: 0.01 * (elapsed * 0.2).cos() as f32,
            },
            TelemetryPattern::PitchOscillation => AngularRates {
                p: 0.01 * (elapsed * 0.2).sin() as f32,
                q: 0.15 * (elapsed * 0.3).cos() as f32,
                r: 0.005 * (elapsed * 0.1).sin() as f32,
            },
            TelemetryPattern::CombinedManeuver => AngularRates {
                p: 0.12 * (elapsed * 0.3).sin() as f32,
                q: 0.1 * (elapsed * 0.4).cos() as f32,
                r: 0.03 * (elapsed * 0.2).sin() as f32,
            },
            TelemetryPattern::HighGTurn => AngularRates {
                p: 0.2 * (elapsed * 0.2).sin() as f32,
                q: 0.15 * (elapsed * 0.2).cos() as f32,
                r: 0.05 * (elapsed * 0.2).sin() as f32,
            },
        }
    }

    /// Generate control inputs based on the pattern
    fn generate_control_inputs(&self, elapsed: f64) -> ControlInputs {
        match self.config.pattern {
            TelemetryPattern::SteadyFlight => ControlInputs {
                pitch: 0.05,
                roll: 0.0,
                yaw: 0.0,
                throttle: vec![0.75],
            },
            TelemetryPattern::GentleBank => ControlInputs {
                pitch: 0.05,
                roll: 0.3 * (elapsed * 0.5).sin() as f32,
                yaw: 0.05 * (elapsed * 0.5).sin() as f32,
                throttle: vec![0.75],
            },
            TelemetryPattern::PitchOscillation => ControlInputs {
                pitch: 0.2 * (elapsed * 0.3).sin() as f32,
                roll: 0.0,
                yaw: 0.0,
                throttle: vec![0.75],
            },
            TelemetryPattern::CombinedManeuver => ControlInputs {
                pitch: 0.15 * (elapsed * 0.4).sin() as f32,
                roll: 0.25 * (elapsed * 0.3).cos() as f32,
                yaw: 0.08 * (elapsed * 0.2).sin() as f32,
                throttle: vec![0.75],
            },
            TelemetryPattern::HighGTurn => ControlInputs {
                pitch: 0.3 * (elapsed * 0.2).sin() as f32,
                roll: 0.6 * (elapsed * 0.2).sin() as f32,
                yaw: 0.1 * (elapsed * 0.2).sin() as f32,
                throttle: vec![0.85],
            },
        }
    }

    /// Process a snapshot through the FFB engine
    fn process_snapshot(&mut self, snapshot: &BusSnapshot) -> Result<()> {
        // Update FFB engine with telemetry
        if let Some(_effect_output) = self.ffb_engine.update_telemetry_synthesis(snapshot)? {
            // Effect output received, engine is processing telemetry
        }

        // Update engine state
        self.ffb_engine.update()?;

        Ok(())
    }

    /// Get the FFB engine for inspection
    pub fn get_ffb_engine(&self) -> &FfbEngine {
        &self.ffb_engine
    }

    /// Get mutable FFB engine for configuration
    pub fn get_ffb_engine_mut(&mut self) -> &mut FfbEngine {
        &mut self.ffb_engine
    }

    /// Get the last generated snapshot
    pub fn get_last_snapshot(&self) -> Option<&BusSnapshot> {
        self.last_snapshot.as_ref()
    }

    /// Get frame count
    pub fn get_frame_count(&self) -> u64 {
        self.frame_count
    }
}

/// Results from running the synthetic harness
#[derive(Debug, Clone)]
pub struct HarnessResults {
    /// Total frames processed
    pub total_frames: u64,
    /// Number of successful frame processing
    pub success_count: u64,
    /// Number of errors encountered
    pub error_count: u64,
    /// Number of missed frame deadlines
    pub missed_frames: u64,
    /// Total duration of the run
    pub duration: Duration,
    /// Final safety state of the FFB engine
    pub final_safety_state: SafetyState,
}

impl Default for HarnessResults {
    fn default() -> Self {
        Self {
            total_frames: 0,
            success_count: 0,
            error_count: 0,
            missed_frames: 0,
            duration: Duration::from_secs(0),
            final_safety_state: SafetyState::SafeTorque,
        }
    }
}

impl HarnessResults {
    /// Check if the harness run was successful
    pub fn is_successful(&self) -> bool {
        self.error_count == 0 && self.success_count > 0
    }

    /// Get the success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.success_count as f64 / self.total_frames as f64
        }
    }

    /// Get the actual frame rate achieved
    pub fn actual_frame_rate(&self) -> f64 {
        if self.duration.as_secs_f64() == 0.0 {
            0.0
        } else {
            self.total_frames as f64 / self.duration.as_secs_f64()
        }
    }
}

impl Default for SyntheticHarnessConfig {
    fn default() -> Self {
        Self {
            update_rate_hz: 60,
            duration: Duration::from_secs(5),
            ffb_config: FfbConfig {
                max_torque_nm: 15.0,
                fault_timeout_ms: 50,
                interlock_required: false,
                mode: FfbMode::TelemetrySynth,
                device_path: None,
            },
            pattern: TelemetryPattern::SteadyFlight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_ffb::TelemetrySynthConfig;

    #[test]
    fn test_synthetic_harness_creation() {
        let config = SyntheticHarnessConfig::default();
        let harness = SyntheticHarness::new(config);
        assert!(harness.is_ok());
    }

    #[test]
    fn test_steady_flight_pattern() {
        let mut config = SyntheticHarnessConfig::default();
        config.pattern = TelemetryPattern::SteadyFlight;
        config.duration = Duration::from_millis(100);

        let mut harness = SyntheticHarness::new(config).unwrap();

        // Enable telemetry synthesis
        let synth_config = TelemetrySynthConfig::default();
        harness
            .get_ffb_engine_mut()
            .enable_telemetry_synthesis(synth_config)
            .unwrap();

        let results = harness.run().unwrap();

        assert!(results.total_frames > 0);
        assert_eq!(results.error_count, 0);
    }

    #[test]
    fn test_gentle_bank_pattern() {
        let mut config = SyntheticHarnessConfig::default();
        config.pattern = TelemetryPattern::GentleBank;
        config.duration = Duration::from_millis(100);

        let mut harness = SyntheticHarness::new(config).unwrap();

        // Enable telemetry synthesis
        let synth_config = TelemetrySynthConfig::default();
        harness
            .get_ffb_engine_mut()
            .enable_telemetry_synthesis(synth_config)
            .unwrap();

        let results = harness.run().unwrap();

        assert!(results.total_frames > 0);
        assert_eq!(results.error_count, 0);
    }

    #[test]
    fn test_snapshot_generation() {
        let config = SyntheticHarnessConfig::default();
        let harness = SyntheticHarness::new(config).unwrap();

        let snapshot = harness.generate_snapshot().unwrap();

        // Verify snapshot is valid
        assert!(snapshot.validate().is_ok());
        assert!(snapshot.validity.safe_for_ffb);
        assert!(snapshot.validity.attitude_valid);
        assert!(snapshot.validity.velocities_valid);
    }

    #[test]
    fn test_all_patterns() {
        let patterns = [
            TelemetryPattern::SteadyFlight,
            TelemetryPattern::GentleBank,
            TelemetryPattern::PitchOscillation,
            TelemetryPattern::CombinedManeuver,
            TelemetryPattern::HighGTurn,
        ];

        for pattern in patterns {
            let mut config = SyntheticHarnessConfig::default();
            config.pattern = pattern;
            config.duration = Duration::from_millis(50);

            let mut harness = SyntheticHarness::new(config).unwrap();

            // Enable telemetry synthesis
            let synth_config = TelemetrySynthConfig::default();
            harness
                .get_ffb_engine_mut()
                .enable_telemetry_synthesis(synth_config)
                .unwrap();

            let results = harness.run().unwrap();

            assert!(
                results.total_frames > 0,
                "Pattern {:?} produced no frames",
                pattern
            );
            assert_eq!(results.error_count, 0, "Pattern {:?} had errors", pattern);
        }
    }

    #[test]
    fn test_harness_results() {
        let mut results = HarnessResults::default();
        results.total_frames = 100;
        results.success_count = 100;
        results.error_count = 0;
        results.duration = Duration::from_secs(1);

        assert!(results.is_successful());
        assert_eq!(results.success_rate(), 1.0);
        assert!((results.actual_frame_rate() - 100.0).abs() < 1.0);
    }
}
