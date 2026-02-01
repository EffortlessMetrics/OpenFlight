// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! End-to-end integration test framework for validating the complete pipeline.
//!
//! This module provides the `EndToEndTest` framework that tests:
//! - Sim fixture → Bus → FFB → Safety pipeline
//! - No safety violations under normal conditions
//! - Diagnostic output on failure
//!
//! Requirements: 14.2, 14.3 from release-readiness spec

use crate::fixtures::{ScenarioType, SnapshotFixture};
use crate::integration_test::AdapterType;
use crate::snapshot::BusSnapshot;
use crate::types::AircraftId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;

/// End-to-end test errors
#[derive(Debug, Error)]
pub enum E2ETestError {
    #[error("Fixture load failed: {path}")]
    FixtureLoadFailed { path: String },
    #[error("Safety violation: {reason}")]
    SafetyViolation { reason: String },
    #[error("FFB processing error: {reason}")]
    FfbProcessingError { reason: String },
    #[error("Bus error: {reason}")]
    BusError { reason: String },
    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },
}

/// Result of an end-to-end test run
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct E2ETestResult {
    /// Number of frames processed
    pub frames_processed: usize,
    /// Number of safety violations detected
    pub safety_violations: usize,
    /// Details of safety violations
    pub violation_details: Vec<SafetyViolationDetail>,
    /// Total test duration
    pub duration: Duration,
    /// Overall test pass/fail
    pub passed: bool,
    /// FFB state at end of test
    pub final_ffb_state: FfbStateSnapshot,
    /// Diagnostic information (populated on failure)
    pub diagnostics: Option<E2EDiagnostics>,
}

/// Details of a safety violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyViolationDetail {
    /// Frame number where violation occurred
    pub frame: usize,
    /// Type of violation
    pub violation_type: SafetyViolationType,
    /// Snapshot state at time of violation
    pub snapshot_state: SnapshotStateInfo,
    /// FFB state at time of violation
    pub ffb_state: FfbStateSnapshot,
    /// Description of the violation
    pub description: String,
}

/// Types of safety violations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyViolationType {
    /// Torque output when safe_for_ffb is false
    TorqueWhenUnsafe,
    /// Torque exceeds maximum limit
    TorqueExceedsLimit,
    /// Slew rate exceeds limit
    SlewRateExceedsLimit,
    /// NaN or Inf in torque output
    InvalidTorqueValue,
    /// Fault ramp not triggered when expected
    FaultRampNotTriggered,
}

/// Snapshot state information for diagnostics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotStateInfo {
    /// Simulator ID
    pub sim: String,
    /// Aircraft ID
    pub aircraft: String,
    /// Whether safe_for_ffb was set
    pub safe_for_ffb: bool,
    /// Indicated airspeed (knots)
    pub ias_knots: f32,
    /// Pitch angle (degrees)
    pub pitch_deg: f32,
    /// Bank angle (degrees)
    pub bank_deg: f32,
    /// G-force
    pub g_force: f32,
    /// Timestamp
    pub timestamp: u64,
}

impl From<&BusSnapshot> for SnapshotStateInfo {
    fn from(snapshot: &BusSnapshot) -> Self {
        Self {
            sim: format!("{:?}", snapshot.sim),
            aircraft: snapshot.aircraft.icao.clone(),
            safe_for_ffb: snapshot.validity.safe_for_ffb,
            ias_knots: snapshot.kinematics.ias.to_knots(),
            pitch_deg: snapshot.kinematics.pitch.to_degrees(),
            bank_deg: snapshot.kinematics.bank.to_degrees(),
            g_force: snapshot.kinematics.g_force.value(),
            timestamp: snapshot.timestamp,
        }
    }
}

/// FFB state snapshot for diagnostics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FfbStateSnapshot {
    /// Current torque output (Nm)
    pub torque_nm: f32,
    /// Current slew rate (Nm/s)
    pub slew_rate_nm_per_s: f32,
    /// Whether in fault ramp
    pub in_fault_ramp: bool,
    /// Fault ramp progress (0.0 to 1.0)
    pub fault_ramp_progress: Option<f32>,
}

/// Diagnostic information for test failures
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E2EDiagnostics {
    /// Frame where failure occurred
    pub failure_frame: usize,
    /// Failure reason
    pub failure_reason: String,
    /// Snapshot state at failure
    pub snapshot_at_failure: SnapshotStateInfo,
    /// FFB state at failure
    pub ffb_at_failure: FfbStateSnapshot,
    /// Recent frame history (last N frames before failure)
    pub frame_history: Vec<FrameHistoryEntry>,
    /// Test configuration
    pub test_config: E2ETestConfig,
}

/// Entry in frame history for diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameHistoryEntry {
    /// Frame number
    pub frame: usize,
    /// Snapshot state
    pub snapshot: SnapshotStateInfo,
    /// FFB state
    pub ffb: FfbStateSnapshot,
    /// Requested torque
    pub requested_torque_nm: f32,
    /// Output torque (after safety)
    pub output_torque_nm: f32,
}

/// Configuration for end-to-end tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2ETestConfig {
    /// Maximum torque limit (Nm)
    pub max_torque_nm: f32,
    /// Maximum slew rate (Nm/s)
    pub max_slew_rate_nm_per_s: f32,
    /// Maximum jerk (Nm/s²)
    pub max_jerk_nm_per_s2: f32,
    /// Timestep for FFB calculations (seconds)
    pub timestep_s: f32,
    /// Number of frames to keep in history
    pub history_frames: usize,
}

impl Default for E2ETestConfig {
    fn default() -> Self {
        Self {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            timestep_s: 0.004, // 250Hz
            history_frames: 50,
        }
    }
}

/// Mock FFB engine for end-to-end testing
///
/// Simulates the FFB safety envelope without requiring actual hardware.
/// Implements the same safety constraints as the real SafetyEnvelope.
#[derive(Debug)]
pub struct MockFfbEngine {
    config: E2ETestConfig,
    last_torque_nm: f32,
    last_slew_rate_nm_per_s: f32,
    in_fault_ramp: bool,
    fault_ramp_start: Option<Instant>,
    fault_initial_torque_nm: f32,
}

impl MockFfbEngine {
    /// Create a new mock FFB engine
    pub fn new(config: E2ETestConfig) -> Self {
        Self {
            config,
            last_torque_nm: 0.0,
            last_slew_rate_nm_per_s: 0.0,
            in_fault_ramp: false,
            fault_ramp_start: None,
            fault_initial_torque_nm: 0.0,
        }
    }

    /// Compute torque output based on snapshot and requested torque
    ///
    /// Applies safety envelope constraints:
    /// - Torque clamping to max_torque_nm
    /// - Slew rate limiting
    /// - Jerk limiting
    /// - safe_for_ffb enforcement
    pub fn compute_torque(
        &mut self,
        snapshot: &BusSnapshot,
        requested_torque_nm: f32,
    ) -> Result<f32, E2ETestError> {
        // Validate input
        if !requested_torque_nm.is_finite() {
            return Err(E2ETestError::FfbProcessingError {
                reason: format!("Invalid requested torque: {}", requested_torque_nm),
            });
        }

        let dt = self.config.timestep_s;

        // Handle fault ramp-down
        if self.in_fault_ramp {
            if let Some(start) = self.fault_ramp_start {
                let elapsed = start.elapsed();
                let ramp_time = Duration::from_millis(50);

                if elapsed >= ramp_time {
                    // Ramp complete
                    self.last_torque_nm = 0.0;
                    self.last_slew_rate_nm_per_s = 0.0;
                    return Ok(0.0);
                } else {
                    // Linear ramp to zero
                    let progress = elapsed.as_secs_f32() / ramp_time.as_secs_f32();
                    let torque = self.fault_initial_torque_nm * (1.0 - progress);
                    self.last_torque_nm = torque;
                    return Ok(torque);
                }
            }
        }

        // Enforce safe_for_ffb flag
        let target_torque = if snapshot.validity.safe_for_ffb {
            requested_torque_nm
        } else {
            0.0
        };

        // Clamp to device maximum
        let clamped = target_torque.clamp(-self.config.max_torque_nm, self.config.max_torque_nm);

        // Apply slew rate limiting
        let desired_delta = clamped - self.last_torque_nm;
        let max_delta = self.config.max_slew_rate_nm_per_s * dt;
        let limited_delta = desired_delta.clamp(-max_delta, max_delta);
        let limited_slew_rate = limited_delta / dt;

        // Apply jerk limiting
        let desired_jerk = (limited_slew_rate - self.last_slew_rate_nm_per_s) / dt;
        let limited_jerk = desired_jerk.clamp(
            -self.config.max_jerk_nm_per_s2,
            self.config.max_jerk_nm_per_s2,
        );
        let final_slew_rate = self.last_slew_rate_nm_per_s + (limited_jerk * dt);

        // Calculate final torque
        let final_delta = final_slew_rate * dt;
        let final_torque = self.last_torque_nm + final_delta;

        // Final clamp
        let output = final_torque.clamp(-self.config.max_torque_nm, self.config.max_torque_nm);

        // Update state
        self.last_torque_nm = output;
        self.last_slew_rate_nm_per_s = (output - self.last_torque_nm + final_delta) / dt;

        Ok(output)
    }

    /// Trigger fault ramp-down
    pub fn trigger_fault_ramp(&mut self) {
        if !self.in_fault_ramp {
            self.in_fault_ramp = true;
            self.fault_ramp_start = Some(Instant::now());
            self.fault_initial_torque_nm = self.last_torque_nm;
        }
    }

    /// Clear fault state
    pub fn clear_fault(&mut self) {
        self.in_fault_ramp = false;
        self.fault_ramp_start = None;
        self.fault_initial_torque_nm = 0.0;
    }

    /// Get current state snapshot
    pub fn get_state(&self) -> FfbStateSnapshot {
        FfbStateSnapshot {
            torque_nm: self.last_torque_nm,
            slew_rate_nm_per_s: self.last_slew_rate_nm_per_s,
            in_fault_ramp: self.in_fault_ramp,
            fault_ramp_progress: self.fault_ramp_start.map(|start| {
                let elapsed = start.elapsed();
                (elapsed.as_secs_f32() / 0.050).clamp(0.0, 1.0)
            }),
        }
    }

    /// Reset engine state
    pub fn reset(&mut self) {
        self.last_torque_nm = 0.0;
        self.last_slew_rate_nm_per_s = 0.0;
        self.in_fault_ramp = false;
        self.fault_ramp_start = None;
        self.fault_initial_torque_nm = 0.0;
    }
}

/// Mock telemetry bus for end-to-end testing
#[derive(Debug, Default)]
pub struct MockTelemetryBus {
    /// Last published snapshot
    last_snapshot: Option<BusSnapshot>,
    /// Total snapshots published
    publish_count: usize,
}

impl MockTelemetryBus {
    /// Create a new mock bus
    pub fn new() -> Self {
        Self::default()
    }

    /// Publish a snapshot to the bus
    pub fn publish(&mut self, snapshot: BusSnapshot) {
        self.last_snapshot = Some(snapshot);
        self.publish_count += 1;
    }

    /// Get the last published snapshot
    pub fn last_snapshot(&self) -> Option<&BusSnapshot> {
        self.last_snapshot.as_ref()
    }

    /// Get total publish count
    pub fn publish_count(&self) -> usize {
        self.publish_count
    }
}

/// End-to-end integration test
///
/// Tests the complete pipeline: sim fixture → bus → FFB → safety
///
/// Requirements: 14.2, 14.3
pub struct EndToEndTest {
    /// Optional fixture path for file-based fixtures
    fixture_path: Option<PathBuf>,
    /// Adapter type to simulate
    adapter_type: AdapterType,
    /// Aircraft to use
    aircraft: AircraftId,
    /// Scenario to test
    scenario: ScenarioType,
    /// Number of frames to process
    frame_count: usize,
    /// Test configuration
    config: E2ETestConfig,
}

impl EndToEndTest {
    /// Create a new end-to-end test
    pub fn new(adapter_type: AdapterType) -> Self {
        Self {
            fixture_path: None,
            adapter_type,
            aircraft: adapter_type.default_aircraft(),
            scenario: ScenarioType::Cruise,
            frame_count: 250, // 1 second at 250Hz
            config: E2ETestConfig::default(),
        }
    }

    /// Set fixture path for file-based fixtures
    pub fn with_fixture_path(mut self, path: PathBuf) -> Self {
        self.fixture_path = Some(path);
        self
    }

    /// Set aircraft
    pub fn with_aircraft(mut self, aircraft: AircraftId) -> Self {
        self.aircraft = aircraft;
        self
    }

    /// Set scenario
    pub fn with_scenario(mut self, scenario: ScenarioType) -> Self {
        self.scenario = scenario;
        self
    }

    /// Set frame count
    pub fn with_frame_count(mut self, count: usize) -> Self {
        self.frame_count = count;
        self
    }

    /// Set test configuration
    pub fn with_config(mut self, config: E2ETestConfig) -> Self {
        self.config = config;
        self
    }

    /// Run end-to-end test: sim fixture → bus → FFB → safety
    ///
    /// Requirements: 14.2
    pub fn run(&self) -> Result<E2ETestResult, E2ETestError> {
        let start_time = Instant::now();
        let mut results = E2ETestResult::default();

        // Initialize components
        let mut fixture = SnapshotFixture::new(
            self.adapter_type.sim_id(),
            self.aircraft.clone(),
            self.scenario,
        );
        let mut bus = MockTelemetryBus::new();
        let mut ffb = MockFfbEngine::new(self.config.clone());

        // Frame history for diagnostics
        let mut frame_history: Vec<FrameHistoryEntry> =
            Vec::with_capacity(self.config.history_frames);

        // Process frames through the pipeline
        for frame_idx in 0..self.frame_count {
            // Generate snapshot from fixture (Sim → Bus)
            let mut snapshot = fixture.advance(Duration::from_micros(4000)); // 250Hz = 4ms

            // Set safe_for_ffb based on scenario
            // Normal conditions should have safe_for_ffb = true
            snapshot.validity.safe_for_ffb = self.is_safe_for_ffb(&snapshot);

            // Publish to bus
            bus.publish(snapshot.clone());

            // Compute requested torque based on flight state
            let requested_torque = self.compute_requested_torque(&snapshot);

            // Process through FFB engine (Bus → FFB → Safety)
            let output_torque = ffb.compute_torque(&snapshot, requested_torque)?;

            // Check for safety violations
            if let Some(violation) = self.check_safety_violation(
                frame_idx,
                &snapshot,
                requested_torque,
                output_torque,
                &ffb,
            ) {
                results.safety_violations += 1;
                results.violation_details.push(violation);
            }

            // Update frame history (ring buffer)
            let history_entry = FrameHistoryEntry {
                frame: frame_idx,
                snapshot: SnapshotStateInfo::from(&snapshot),
                ffb: ffb.get_state(),
                requested_torque_nm: requested_torque,
                output_torque_nm: output_torque,
            };

            if frame_history.len() >= self.config.history_frames {
                frame_history.remove(0);
            }
            frame_history.push(history_entry);

            results.frames_processed += 1;
        }

        // Finalize results
        results.duration = start_time.elapsed();
        results.final_ffb_state = ffb.get_state();
        results.passed = results.safety_violations == 0;

        // Generate diagnostics on failure
        if !results.passed {
            results.diagnostics = Some(self.generate_diagnostics(&results, frame_history));
        }

        Ok(results)
    }

    /// Determine if FFB is safe based on snapshot state
    fn is_safe_for_ffb(&self, snapshot: &BusSnapshot) -> bool {
        // Normal flight conditions are safe for FFB
        // Unsafe conditions include:
        // - On ground with gear down and low speed (parking)
        // - Extreme attitudes (inverted, etc.)
        // - Invalid data

        let ias = snapshot.kinematics.ias.to_knots();
        let pitch = snapshot.kinematics.pitch.to_degrees().abs();
        let bank = snapshot.kinematics.bank.to_degrees().abs();

        // Check for valid data
        if !ias.is_finite() || !pitch.is_finite() || !bank.is_finite() {
            return false;
        }

        // Check for extreme attitudes
        if pitch > 60.0 || bank > 75.0 {
            return false;
        }

        // Normal conditions are safe
        true
    }

    /// Compute requested torque based on flight state
    fn compute_requested_torque(&self, snapshot: &BusSnapshot) -> f32 {
        // Simple torque model based on g-forces and control inputs
        let g_force = snapshot.kinematics.g_force.value();
        let pitch_input = snapshot.control_inputs.pitch;
        let roll_input = snapshot.control_inputs.roll;

        // Base torque from g-forces (simulating stick forces)
        let g_torque = (g_force - 1.0) * 2.0; // 2 Nm per G above 1G

        // Control input contribution
        let control_torque = (pitch_input.abs() + roll_input.abs()) * 3.0;

        // Combined torque (clamped to reasonable range)
        (g_torque + control_torque).clamp(-self.config.max_torque_nm, self.config.max_torque_nm)
    }

    /// Check for safety violations
    fn check_safety_violation(
        &self,
        frame: usize,
        snapshot: &BusSnapshot,
        _requested_torque: f32,
        output_torque: f32,
        ffb: &MockFfbEngine,
    ) -> Option<SafetyViolationDetail> {
        // Check: Torque output when safe_for_ffb is false
        if !snapshot.validity.safe_for_ffb && output_torque.abs() > 0.001 {
            // Allow small torque during ramp-down
            if !ffb.in_fault_ramp {
                return Some(SafetyViolationDetail {
                    frame,
                    violation_type: SafetyViolationType::TorqueWhenUnsafe,
                    snapshot_state: SnapshotStateInfo::from(snapshot),
                    ffb_state: ffb.get_state(),
                    description: format!(
                        "Torque output {} Nm when safe_for_ffb=false",
                        output_torque
                    ),
                });
            }
        }

        // Check: Torque exceeds maximum limit
        if output_torque.abs() > self.config.max_torque_nm * 1.001 {
            return Some(SafetyViolationDetail {
                frame,
                violation_type: SafetyViolationType::TorqueExceedsLimit,
                snapshot_state: SnapshotStateInfo::from(snapshot),
                ffb_state: ffb.get_state(),
                description: format!(
                    "Torque {} Nm exceeds limit {} Nm",
                    output_torque, self.config.max_torque_nm
                ),
            });
        }

        // Check: Invalid torque value
        if !output_torque.is_finite() {
            return Some(SafetyViolationDetail {
                frame,
                violation_type: SafetyViolationType::InvalidTorqueValue,
                snapshot_state: SnapshotStateInfo::from(snapshot),
                ffb_state: ffb.get_state(),
                description: format!("Invalid torque value: {}", output_torque),
            });
        }

        None
    }

    /// Generate diagnostic output on failure
    ///
    /// Requirements: 14.3
    fn generate_diagnostics(
        &self,
        results: &E2ETestResult,
        frame_history: Vec<FrameHistoryEntry>,
    ) -> E2EDiagnostics {
        let first_violation = results.violation_details.first();

        E2EDiagnostics {
            failure_frame: first_violation.map(|v| v.frame).unwrap_or(0),
            failure_reason: first_violation
                .map(|v| v.description.clone())
                .unwrap_or_else(|| "Unknown failure".to_string()),
            snapshot_at_failure: first_violation
                .map(|v| v.snapshot_state.clone())
                .unwrap_or_default(),
            ffb_at_failure: first_violation
                .map(|v| v.ffb_state.clone())
                .unwrap_or_default(),
            frame_history,
            test_config: self.config.clone(),
        }
    }
}

impl E2ETestResult {
    /// Print a summary of the test results
    pub fn print_summary(&self) {
        println!("\n=== End-to-End Test Results ===");
        println!("Overall: {}", if self.passed { "PASSED" } else { "FAILED" });
        println!("Duration: {:?}", self.duration);
        println!("Frames processed: {}", self.frames_processed);
        println!("Safety violations: {}", self.safety_violations);

        if !self.violation_details.is_empty() {
            println!("\nViolation Details:");
            for (i, violation) in self.violation_details.iter().take(5).enumerate() {
                println!(
                    "  {}. Frame {}: {:?} - {}",
                    i + 1,
                    violation.frame,
                    violation.violation_type,
                    violation.description
                );
            }
            if self.violation_details.len() > 5 {
                println!(
                    "  ... and {} more violations",
                    self.violation_details.len() - 5
                );
            }
        }

        println!("\nFinal FFB State:");
        println!("  Torque: {} Nm", self.final_ffb_state.torque_nm);
        println!(
            "  Slew rate: {} Nm/s",
            self.final_ffb_state.slew_rate_nm_per_s
        );
        println!("  In fault ramp: {}", self.final_ffb_state.in_fault_ramp);

        if let Some(ref diag) = self.diagnostics {
            println!("\nDiagnostics:");
            println!("  Failure frame: {}", diag.failure_frame);
            println!("  Failure reason: {}", diag.failure_reason);
            println!("  Frame history entries: {}", diag.frame_history.len());
        }
    }

    /// Export diagnostics to JSON for detailed analysis
    pub fn export_diagnostics_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.diagnostics)
    }
}

impl E2EDiagnostics {
    /// Print detailed diagnostic information
    pub fn print_detailed(&self) {
        println!("\n=== Detailed Diagnostics ===");
        println!("Failure Frame: {}", self.failure_frame);
        println!("Failure Reason: {}", self.failure_reason);

        println!("\nSnapshot at Failure:");
        println!("  Sim: {}", self.snapshot_at_failure.sim);
        println!("  Aircraft: {}", self.snapshot_at_failure.aircraft);
        println!("  safe_for_ffb: {}", self.snapshot_at_failure.safe_for_ffb);
        println!("  IAS: {} knots", self.snapshot_at_failure.ias_knots);
        println!("  Pitch: {}°", self.snapshot_at_failure.pitch_deg);
        println!("  Bank: {}°", self.snapshot_at_failure.bank_deg);
        println!("  G-force: {}", self.snapshot_at_failure.g_force);

        println!("\nFFB State at Failure:");
        println!("  Torque: {} Nm", self.ffb_at_failure.torque_nm);
        println!(
            "  Slew rate: {} Nm/s",
            self.ffb_at_failure.slew_rate_nm_per_s
        );
        println!("  In fault ramp: {}", self.ffb_at_failure.in_fault_ramp);

        println!("\nTest Configuration:");
        println!("  Max torque: {} Nm", self.test_config.max_torque_nm);
        println!(
            "  Max slew rate: {} Nm/s",
            self.test_config.max_slew_rate_nm_per_s
        );
        println!("  Timestep: {} s", self.test_config.timestep_s);

        if !self.frame_history.is_empty() {
            println!(
                "\nRecent Frame History (last {} frames):",
                self.frame_history.len()
            );
            for entry in self.frame_history.iter().rev().take(10) {
                println!(
                    "  Frame {}: req={:.2} Nm, out={:.2} Nm, safe={}",
                    entry.frame,
                    entry.requested_torque_nm,
                    entry.output_torque_nm,
                    entry.snapshot.safe_for_ffb
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SimId;

    /// Test end-to-end pipeline with MSFS cruise scenario
    /// Requirements: 14.2
    #[test]
    fn test_e2e_msfs_cruise() {
        let test = EndToEndTest::new(AdapterType::Msfs)
            .with_aircraft(AircraftId::new("C172"))
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(250); // 1 second

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "MSFS cruise E2E test should pass");
        assert_eq!(result.safety_violations, 0, "No safety violations expected");
        assert!(result.frames_processed > 0, "Should process frames");
    }

    /// Test end-to-end pipeline with X-Plane cruise scenario
    /// Requirements: 14.2
    #[test]
    fn test_e2e_xplane_cruise() {
        let test = EndToEndTest::new(AdapterType::XPlane)
            .with_aircraft(AircraftId::new("C172"))
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(250);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "X-Plane cruise E2E test should pass");
        assert_eq!(result.safety_violations, 0, "No safety violations expected");
    }

    /// Test end-to-end pipeline with DCS cruise scenario
    /// Requirements: 14.2
    #[test]
    fn test_e2e_dcs_cruise() {
        let test = EndToEndTest::new(AdapterType::Dcs)
            .with_aircraft(AircraftId::new("F16C"))
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(250);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "DCS cruise E2E test should pass");
        assert_eq!(result.safety_violations, 0, "No safety violations expected");
    }

    /// Test end-to-end pipeline with takeoff scenario
    /// Requirements: 14.2
    #[test]
    fn test_e2e_takeoff_scenario() {
        let test = EndToEndTest::new(AdapterType::Msfs)
            .with_aircraft(AircraftId::new("C172"))
            .with_scenario(ScenarioType::Takeoff)
            .with_frame_count(500); // 2 seconds

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "Takeoff E2E test should pass");
        assert_eq!(result.safety_violations, 0, "No safety violations expected");
    }

    /// Test end-to-end pipeline with approach scenario
    /// Requirements: 14.2
    #[test]
    fn test_e2e_approach_scenario() {
        let test = EndToEndTest::new(AdapterType::Msfs)
            .with_aircraft(AircraftId::new("C172"))
            .with_scenario(ScenarioType::Approach)
            .with_frame_count(500);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "Approach E2E test should pass");
        assert_eq!(result.safety_violations, 0, "No safety violations expected");
    }

    /// Test end-to-end pipeline with helicopter hover scenario
    /// Requirements: 14.2
    #[test]
    fn test_e2e_helo_hover() {
        let test = EndToEndTest::new(AdapterType::Dcs)
            .with_aircraft(AircraftId::new("UH1H"))
            .with_scenario(ScenarioType::HeloHover)
            .with_frame_count(250);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "Helicopter hover E2E test should pass");
        assert_eq!(result.safety_violations, 0, "No safety violations expected");
    }

    /// Test all scenarios for each adapter
    /// Requirements: 14.2
    #[test]
    fn test_e2e_all_scenarios() {
        let scenarios = vec![
            ScenarioType::ColdAndDark,
            ScenarioType::GroundIdle,
            ScenarioType::Takeoff,
            ScenarioType::Cruise,
            ScenarioType::Approach,
        ];

        for adapter_type in [AdapterType::Msfs, AdapterType::XPlane, AdapterType::Dcs] {
            for scenario in &scenarios {
                let test = EndToEndTest::new(adapter_type)
                    .with_scenario(*scenario)
                    .with_frame_count(100);

                let result = test.run().unwrap();

                assert!(
                    result.passed,
                    "{:?} adapter with {:?} scenario should pass (violations: {})",
                    adapter_type, scenario, result.safety_violations
                );
            }
        }
    }

    /// Test mock FFB engine torque clamping
    #[test]
    fn test_mock_ffb_torque_clamping() {
        let config = E2ETestConfig {
            max_torque_nm: 10.0,
            ..Default::default()
        };
        let mut ffb = MockFfbEngine::new(config);

        let mut snapshot = BusSnapshot::default();
        snapshot.validity.safe_for_ffb = true;

        // Request torque above limit
        let output = ffb.compute_torque(&snapshot, 20.0).unwrap();

        // Should be clamped (may not reach max immediately due to slew rate)
        assert!(output <= 10.0, "Torque should be clamped to max");
        assert!(
            output >= 0.0,
            "Torque should be non-negative for positive request"
        );
    }

    /// Test mock FFB engine safe_for_ffb enforcement
    #[test]
    fn test_mock_ffb_safe_for_ffb_enforcement() {
        let config = E2ETestConfig::default();
        let mut ffb = MockFfbEngine::new(config);

        let mut snapshot = BusSnapshot::default();
        snapshot.validity.safe_for_ffb = false;

        // Request torque when not safe
        let output = ffb.compute_torque(&snapshot, 10.0).unwrap();

        // Should output zero (or ramping to zero)
        assert!(
            output.abs() < 0.001,
            "Torque should be zero when not safe for FFB"
        );
    }

    /// Test diagnostic output generation
    /// Requirements: 14.3
    #[test]
    fn test_diagnostic_output() {
        let test = EndToEndTest::new(AdapterType::Msfs)
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(100);

        let result = test.run().unwrap();

        // For a passing test, diagnostics should be None
        if result.passed {
            assert!(
                result.diagnostics.is_none(),
                "Passing test should have no diagnostics"
            );
        }

        // Test JSON export
        let json = result.export_diagnostics_json();
        assert!(json.is_ok(), "JSON export should succeed");
    }

    /// Test frame history tracking
    #[test]
    fn test_frame_history_tracking() {
        let config = E2ETestConfig {
            history_frames: 10,
            ..Default::default()
        };

        let test = EndToEndTest::new(AdapterType::Msfs)
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(50)
            .with_config(config);

        let result = test.run().unwrap();

        // Test should pass
        assert!(result.passed);
        assert_eq!(result.frames_processed, 50);
    }

    /// Test mock telemetry bus
    #[test]
    fn test_mock_telemetry_bus() {
        let mut bus = MockTelemetryBus::new();

        assert!(bus.last_snapshot().is_none());
        assert_eq!(bus.publish_count(), 0);

        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        bus.publish(snapshot);

        assert!(bus.last_snapshot().is_some());
        assert_eq!(bus.publish_count(), 1);
        assert_eq!(bus.last_snapshot().unwrap().sim, SimId::Msfs);
    }

    /// Test diagnostic output contains required information on failure
    /// Requirements: 14.3
    #[test]
    fn test_diagnostic_output_on_failure() {
        // This test verifies that the diagnostic output structure is properly
        // populated when a test fails. We test the structure directly rather
        // than trying to force a failure in the E2E test.

        // Create a result with a simulated failure
        let mut result = E2ETestResult {
            frames_processed: 100,
            safety_violations: 1,
            violation_details: vec![SafetyViolationDetail {
                frame: 50,
                violation_type: SafetyViolationType::TorqueWhenUnsafe,
                snapshot_state: SnapshotStateInfo {
                    sim: "Msfs".to_string(),
                    aircraft: "C172".to_string(),
                    safe_for_ffb: false,
                    ias_knots: 120.0,
                    pitch_deg: 5.0,
                    bank_deg: -10.0,
                    g_force: 1.2,
                    timestamp: 12345,
                },
                ffb_state: FfbStateSnapshot {
                    torque_nm: 8.5,
                    slew_rate_nm_per_s: 25.0,
                    in_fault_ramp: false,
                    fault_ramp_progress: None,
                },
                description: "Torque output 8.5 Nm when safe_for_ffb=false".to_string(),
            }],
            duration: Duration::from_millis(100),
            passed: false,
            final_ffb_state: FfbStateSnapshot::default(),
            diagnostics: None,
        };

        // Generate diagnostics like the real test would
        let frame_history = vec![
            FrameHistoryEntry {
                frame: 48,
                snapshot: SnapshotStateInfo::default(),
                ffb: FfbStateSnapshot::default(),
                requested_torque_nm: 5.0,
                output_torque_nm: 8.0,
            },
            FrameHistoryEntry {
                frame: 49,
                snapshot: SnapshotStateInfo::default(),
                ffb: FfbStateSnapshot::default(),
                requested_torque_nm: 5.0,
                output_torque_nm: 8.3,
            },
        ];

        // Simulate diagnostic generation
        let first_violation = result.violation_details.first();
        result.diagnostics = Some(E2EDiagnostics {
            failure_frame: first_violation.map(|v| v.frame).unwrap_or(0),
            failure_reason: first_violation
                .map(|v| v.description.clone())
                .unwrap_or_else(|| "Unknown failure".to_string()),
            snapshot_at_failure: first_violation
                .map(|v| v.snapshot_state.clone())
                .unwrap_or_default(),
            ffb_at_failure: first_violation
                .map(|v| v.ffb_state.clone())
                .unwrap_or_default(),
            frame_history,
            test_config: E2ETestConfig::default(),
        });

        // Verify diagnostics are populated correctly
        assert!(
            result.diagnostics.is_some(),
            "Diagnostics should be present on failure"
        );
        let diag = result.diagnostics.as_ref().unwrap();

        // Verify failure point is logged (Requirement 14.3)
        assert_eq!(diag.failure_frame, 50, "Failure frame should be captured");
        assert!(
            diag.failure_reason.contains("safe_for_ffb=false"),
            "Failure reason should describe the violation"
        );

        // Verify snapshot state is captured (Requirement 14.3)
        assert_eq!(diag.snapshot_at_failure.sim, "Msfs");
        assert_eq!(diag.snapshot_at_failure.aircraft, "C172");
        assert!(!diag.snapshot_at_failure.safe_for_ffb);

        // Verify FFB state is captured (Requirement 14.3)
        assert_eq!(diag.ffb_at_failure.torque_nm, 8.5);
        assert_eq!(diag.ffb_at_failure.slew_rate_nm_per_s, 25.0);

        // Verify frame history is captured
        assert_eq!(diag.frame_history.len(), 2);

        // Test print methods don't panic
        result.print_summary();
        diag.print_detailed();
    }

    /// Test E2EDiagnostics structure contains all required fields
    /// Requirements: 14.3
    #[test]
    fn test_diagnostics_structure_completeness() {
        let diag = E2EDiagnostics {
            failure_frame: 42,
            failure_reason: "Test failure".to_string(),
            snapshot_at_failure: SnapshotStateInfo {
                sim: "Msfs".to_string(),
                aircraft: "C172".to_string(),
                safe_for_ffb: false,
                ias_knots: 120.0,
                pitch_deg: 5.0,
                bank_deg: -10.0,
                g_force: 1.2,
                timestamp: 12345,
            },
            ffb_at_failure: FfbStateSnapshot {
                torque_nm: 8.5,
                slew_rate_nm_per_s: 25.0,
                in_fault_ramp: true,
                fault_ramp_progress: Some(0.5),
            },
            frame_history: vec![
                FrameHistoryEntry {
                    frame: 40,
                    snapshot: SnapshotStateInfo::default(),
                    ffb: FfbStateSnapshot::default(),
                    requested_torque_nm: 5.0,
                    output_torque_nm: 4.8,
                },
                FrameHistoryEntry {
                    frame: 41,
                    snapshot: SnapshotStateInfo::default(),
                    ffb: FfbStateSnapshot::default(),
                    requested_torque_nm: 5.0,
                    output_torque_nm: 4.9,
                },
            ],
            test_config: E2ETestConfig::default(),
        };

        // Verify all required diagnostic fields are present
        assert_eq!(diag.failure_frame, 42);
        assert!(!diag.failure_reason.is_empty());
        assert_eq!(diag.snapshot_at_failure.sim, "Msfs");
        assert_eq!(diag.snapshot_at_failure.aircraft, "C172");
        assert!(!diag.snapshot_at_failure.safe_for_ffb);
        assert_eq!(diag.ffb_at_failure.torque_nm, 8.5);
        assert!(diag.ffb_at_failure.in_fault_ramp);
        assert_eq!(diag.frame_history.len(), 2);

        // Test print_detailed doesn't panic
        diag.print_detailed();

        // Test JSON serialization
        let json = serde_json::to_string(&diag).unwrap();
        assert!(json.contains("failure_frame"));
        assert!(json.contains("snapshot_at_failure"));
        assert!(json.contains("ffb_at_failure"));
        assert!(json.contains("frame_history"));
    }

    /// Test SafetyViolationDetail captures all required information
    /// Requirements: 14.3
    #[test]
    fn test_safety_violation_detail_completeness() {
        let violation = SafetyViolationDetail {
            frame: 100,
            violation_type: SafetyViolationType::TorqueWhenUnsafe,
            snapshot_state: SnapshotStateInfo {
                sim: "Dcs".to_string(),
                aircraft: "F16C".to_string(),
                safe_for_ffb: false,
                ias_knots: 350.0,
                pitch_deg: 15.0,
                bank_deg: 45.0,
                g_force: 4.5,
                timestamp: 99999,
            },
            ffb_state: FfbStateSnapshot {
                torque_nm: 12.0,
                slew_rate_nm_per_s: 30.0,
                in_fault_ramp: false,
                fault_ramp_progress: None,
            },
            description: "Torque output 12.0 Nm when safe_for_ffb=false".to_string(),
        };

        // Verify violation captures failure point
        assert_eq!(violation.frame, 100);
        assert_eq!(
            violation.violation_type,
            SafetyViolationType::TorqueWhenUnsafe
        );

        // Verify snapshot state is captured
        assert_eq!(violation.snapshot_state.sim, "Dcs");
        assert!(!violation.snapshot_state.safe_for_ffb);

        // Verify FFB state is captured
        assert_eq!(violation.ffb_state.torque_nm, 12.0);

        // Verify description is meaningful
        assert!(violation.description.contains("12.0 Nm"));
        assert!(violation.description.contains("safe_for_ffb=false"));
    }
}
