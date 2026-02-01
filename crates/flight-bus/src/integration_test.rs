// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Adapter integration test framework for validating adapter lifecycle.
//!
//! This module provides the `AdapterIntegrationTest` framework that tests:
//! - Connect → Stream → Disconnect → Reconnect lifecycle
//! - No NaN/Inf values in snapshots
//! - Proper state transitions
//!
//! Requirements: 14.1, 14.2 from release-readiness spec

use crate::fixtures::{ScenarioType, SnapshotFixture};
use crate::snapshot::BusSnapshot;
use crate::types::{AircraftId, SimId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Adapter types supported by the integration test framework
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterType {
    /// MSFS SimConnect adapter
    Msfs,
    /// X-Plane UDP adapter
    XPlane,
    /// DCS Export.lua adapter
    Dcs,
}

impl AdapterType {
    /// Get the corresponding SimId for this adapter type
    pub fn sim_id(&self) -> SimId {
        match self {
            AdapterType::Msfs => SimId::Msfs,
            AdapterType::XPlane => SimId::XPlane,
            AdapterType::Dcs => SimId::Dcs,
        }
    }

    /// Get the default aircraft for this adapter type
    pub fn default_aircraft(&self) -> AircraftId {
        match self {
            AdapterType::Msfs => AircraftId::new("C172"),
            AdapterType::XPlane => AircraftId::new("C172"),
            AdapterType::Dcs => AircraftId::new("F16C"),
        }
    }
}

/// Integration test errors
#[derive(Debug, Error)]
pub enum TestError {
    #[error("Connection failed: {reason}")]
    ConnectionFailed { reason: String },
    #[error("Streaming failed: {reason}")]
    StreamingFailed { reason: String },
    #[error("Disconnect failed: {reason}")]
    DisconnectFailed { reason: String },
    #[error("Reconnect failed: {reason}")]
    ReconnectFailed { reason: String },
    #[error("NaN/Inf detected in snapshot field: {field}")]
    NanInfDetected { field: String },
    #[error("Fixture load failed: {path}")]
    FixtureLoadFailed { path: String },
    #[error("Validation failed: {reason}")]
    ValidationFailed { reason: String },
    #[error("Timeout: {operation}")]
    Timeout { operation: String },
}

/// Result of an integration test run
#[derive(Debug, Default)]
pub struct IntegrationTestResult {
    /// Whether the initial connection succeeded
    pub connect_success: bool,
    /// Number of frames processed during streaming
    pub frames_processed: usize,
    /// Whether NaN or Inf values were detected
    pub nan_inf_detected: bool,
    /// Fields that contained NaN/Inf values
    pub nan_inf_fields: Vec<String>,
    /// Whether disconnect succeeded
    pub disconnect_success: bool,
    /// Whether reconnect succeeded
    pub reconnect_success: bool,
    /// Total test duration
    pub duration: Duration,
    /// Overall test pass/fail
    pub passed: bool,
    /// Detailed phase results
    pub phase_results: Vec<PhaseResult>,
}

/// Result of a single test phase
#[derive(Debug, Clone)]
pub struct PhaseResult {
    /// Phase name
    pub name: String,
    /// Whether the phase succeeded
    pub success: bool,
    /// Phase duration
    pub duration: Duration,
    /// Optional error message
    pub error: Option<String>,
}

/// Mock adapter for testing the integration test framework
///
/// This simulates adapter behavior without requiring actual simulator connections.
/// Used for unit testing the framework itself and for CI environments.
#[derive(Debug)]
pub struct MockAdapter {
    /// Adapter type being simulated
    #[allow(dead_code)]
    adapter_type: AdapterType,
    /// Current connection state
    connected: bool,
    /// Fixture generator for snapshots
    fixture: SnapshotFixture,
    /// Simulated connection delay
    connection_delay: Duration,
    /// Whether to simulate connection failures
    simulate_failure: bool,
    /// Number of frames to generate
    frame_count: usize,
}

impl MockAdapter {
    /// Create a new mock adapter
    pub fn new(adapter_type: AdapterType, aircraft: AircraftId, scenario: ScenarioType) -> Self {
        Self {
            adapter_type,
            connected: false,
            fixture: SnapshotFixture::new(adapter_type.sim_id(), aircraft, scenario),
            connection_delay: Duration::from_millis(50),
            simulate_failure: false,
            frame_count: 100,
        }
    }

    /// Set connection delay for testing
    pub fn with_connection_delay(mut self, delay: Duration) -> Self {
        self.connection_delay = delay;
        self
    }

    /// Set whether to simulate connection failures
    pub fn with_simulate_failure(mut self, simulate: bool) -> Self {
        self.simulate_failure = simulate;
        self
    }

    /// Set number of frames to generate
    pub fn with_frame_count(mut self, count: usize) -> Self {
        self.frame_count = count;
        self
    }

    /// Connect to the simulated adapter
    pub fn connect(&mut self) -> Result<(), TestError> {
        std::thread::sleep(self.connection_delay);

        if self.simulate_failure {
            return Err(TestError::ConnectionFailed {
                reason: "Simulated connection failure".to_string(),
            });
        }

        self.connected = true;
        Ok(())
    }

    /// Disconnect from the simulated adapter
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.fixture.reset();
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Process a frame and return a snapshot
    pub fn process_frame(&mut self) -> Result<BusSnapshot, TestError> {
        if !self.connected {
            return Err(TestError::StreamingFailed {
                reason: "Not connected".to_string(),
            });
        }

        let snapshot = self.fixture.advance(Duration::from_millis(16));
        Ok(snapshot)
    }

    /// Get the number of frames to generate
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
}

/// Integration test for sim adapter lifecycle
///
/// Tests the complete adapter lifecycle: connect → stream → disconnect → reconnect
///
/// Requirements: 14.1, 14.2
pub struct AdapterIntegrationTest {
    /// Adapter type being tested
    adapter_type: AdapterType,
    /// Optional fixture path for file-based fixtures
    fixture_path: Option<PathBuf>,
    /// Aircraft to use for testing
    aircraft: AircraftId,
    /// Scenario to use for testing
    scenario: ScenarioType,
    /// Number of frames to stream
    frame_count: usize,
    /// Connection timeout
    connection_timeout: Duration,
}

impl AdapterIntegrationTest {
    /// Create a new integration test for the specified adapter type
    pub fn new(adapter_type: AdapterType) -> Self {
        Self {
            adapter_type,
            fixture_path: None,
            aircraft: adapter_type.default_aircraft(),
            scenario: ScenarioType::Cruise,
            frame_count: 100,
            connection_timeout: Duration::from_secs(5),
        }
    }

    /// Set the fixture path for file-based fixtures
    pub fn with_fixture_path(mut self, path: PathBuf) -> Self {
        self.fixture_path = Some(path);
        self
    }

    /// Set the aircraft to use for testing
    pub fn with_aircraft(mut self, aircraft: AircraftId) -> Self {
        self.aircraft = aircraft;
        self
    }

    /// Set the scenario to use for testing
    pub fn with_scenario(mut self, scenario: ScenarioType) -> Self {
        self.scenario = scenario;
        self
    }

    /// Set the number of frames to stream
    pub fn with_frame_count(mut self, count: usize) -> Self {
        self.frame_count = count;
        self
    }

    /// Set the connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Run adapter lifecycle test: connect → stream → disconnect → reconnect
    ///
    /// Requirements: 14.1
    pub fn run(&self) -> Result<IntegrationTestResult, TestError> {
        let start_time = Instant::now();
        let mut results = IntegrationTestResult::default();

        // Create mock adapter for testing
        let mut adapter = MockAdapter::new(self.adapter_type, self.aircraft.clone(), self.scenario)
            .with_frame_count(self.frame_count);

        // Phase 1: Connect
        let phase1_start = Instant::now();
        match adapter.connect() {
            Ok(()) => {
                results.connect_success = true;
                results.phase_results.push(PhaseResult {
                    name: "Connect".to_string(),
                    success: true,
                    duration: phase1_start.elapsed(),
                    error: None,
                });
            }
            Err(e) => {
                results.phase_results.push(PhaseResult {
                    name: "Connect".to_string(),
                    success: false,
                    duration: phase1_start.elapsed(),
                    error: Some(e.to_string()),
                });
                results.duration = start_time.elapsed();
                return Ok(results);
            }
        }

        // Phase 2: Stream telemetry
        let phase2_start = Instant::now();
        for _ in 0..adapter.frame_count() {
            match adapter.process_frame() {
                Ok(snapshot) => {
                    // Validate snapshot for NaN/Inf
                    if let Some(field) = self.check_nan_inf(&snapshot) {
                        results.nan_inf_detected = true;
                        results.nan_inf_fields.push(field);
                    }
                    results.frames_processed += 1;
                }
                Err(e) => {
                    results.phase_results.push(PhaseResult {
                        name: "Stream".to_string(),
                        success: false,
                        duration: phase2_start.elapsed(),
                        error: Some(e.to_string()),
                    });
                    results.duration = start_time.elapsed();
                    return Ok(results);
                }
            }
        }
        results.phase_results.push(PhaseResult {
            name: "Stream".to_string(),
            success: !results.nan_inf_detected,
            duration: phase2_start.elapsed(),
            error: if results.nan_inf_detected {
                Some(format!(
                    "NaN/Inf detected in fields: {:?}",
                    results.nan_inf_fields
                ))
            } else {
                None
            },
        });

        // Phase 3: Disconnect
        let phase3_start = Instant::now();
        adapter.disconnect();
        results.disconnect_success = !adapter.is_connected();
        results.phase_results.push(PhaseResult {
            name: "Disconnect".to_string(),
            success: results.disconnect_success,
            duration: phase3_start.elapsed(),
            error: if results.disconnect_success {
                None
            } else {
                Some("Failed to disconnect".to_string())
            },
        });

        // Phase 4: Reconnect
        let phase4_start = Instant::now();
        match adapter.connect() {
            Ok(()) => {
                results.reconnect_success = adapter.is_connected();
                results.phase_results.push(PhaseResult {
                    name: "Reconnect".to_string(),
                    success: results.reconnect_success,
                    duration: phase4_start.elapsed(),
                    error: None,
                });
            }
            Err(e) => {
                results.phase_results.push(PhaseResult {
                    name: "Reconnect".to_string(),
                    success: false,
                    duration: phase4_start.elapsed(),
                    error: Some(e.to_string()),
                });
            }
        }

        // Calculate overall result
        results.duration = start_time.elapsed();
        results.passed = results.connect_success
            && !results.nan_inf_detected
            && results.disconnect_success
            && results.reconnect_success;

        Ok(results)
    }

    /// Check for NaN or Inf values in snapshot
    ///
    /// Returns the first field name containing NaN/Inf, or None if all values are finite.
    fn check_nan_inf(&self, snapshot: &BusSnapshot) -> Option<String> {
        // Check kinematics fields
        if !snapshot.kinematics.ias.value().is_finite() {
            return Some("kinematics.ias".to_string());
        }
        if !snapshot.kinematics.tas.value().is_finite() {
            return Some("kinematics.tas".to_string());
        }
        if !snapshot.kinematics.ground_speed.value().is_finite() {
            return Some("kinematics.ground_speed".to_string());
        }
        if !snapshot.kinematics.aoa.value().is_finite() {
            return Some("kinematics.aoa".to_string());
        }
        if !snapshot.kinematics.sideslip.value().is_finite() {
            return Some("kinematics.sideslip".to_string());
        }
        if !snapshot.kinematics.bank.value().is_finite() {
            return Some("kinematics.bank".to_string());
        }
        if !snapshot.kinematics.pitch.value().is_finite() {
            return Some("kinematics.pitch".to_string());
        }
        if !snapshot.kinematics.heading.value().is_finite() {
            return Some("kinematics.heading".to_string());
        }
        if !snapshot.kinematics.g_force.value().is_finite() {
            return Some("kinematics.g_force".to_string());
        }
        if !snapshot.kinematics.g_lateral.value().is_finite() {
            return Some("kinematics.g_lateral".to_string());
        }
        if !snapshot.kinematics.g_longitudinal.value().is_finite() {
            return Some("kinematics.g_longitudinal".to_string());
        }
        if !snapshot.kinematics.mach.value().is_finite() {
            return Some("kinematics.mach".to_string());
        }
        if !snapshot.kinematics.vertical_speed.is_finite() {
            return Some("kinematics.vertical_speed".to_string());
        }

        // Check angular rates
        if !snapshot.angular_rates.p.is_finite() {
            return Some("angular_rates.p".to_string());
        }
        if !snapshot.angular_rates.q.is_finite() {
            return Some("angular_rates.q".to_string());
        }
        if !snapshot.angular_rates.r.is_finite() {
            return Some("angular_rates.r".to_string());
        }

        // Check environment fields
        if !snapshot.environment.altitude.is_finite() {
            return Some("environment.altitude".to_string());
        }
        if !snapshot.environment.pressure_altitude.is_finite() {
            return Some("environment.pressure_altitude".to_string());
        }
        if !snapshot.environment.oat.is_finite() {
            return Some("environment.oat".to_string());
        }
        if !snapshot.environment.wind_speed.value().is_finite() {
            return Some("environment.wind_speed".to_string());
        }
        if !snapshot.environment.wind_direction.value().is_finite() {
            return Some("environment.wind_direction".to_string());
        }
        if !snapshot.environment.visibility.is_finite() {
            return Some("environment.visibility".to_string());
        }

        // Check control inputs
        if !snapshot.control_inputs.pitch.is_finite() {
            return Some("control_inputs.pitch".to_string());
        }
        if !snapshot.control_inputs.roll.is_finite() {
            return Some("control_inputs.roll".to_string());
        }
        if !snapshot.control_inputs.yaw.is_finite() {
            return Some("control_inputs.yaw".to_string());
        }

        // Check trim state
        if !snapshot.trim_state.elevator.is_finite() {
            return Some("trim_state.elevator".to_string());
        }
        if !snapshot.trim_state.aileron.is_finite() {
            return Some("trim_state.aileron".to_string());
        }
        if !snapshot.trim_state.rudder.is_finite() {
            return Some("trim_state.rudder".to_string());
        }

        // Check navigation
        if !snapshot.navigation.latitude.is_finite() {
            return Some("navigation.latitude".to_string());
        }
        if !snapshot.navigation.longitude.is_finite() {
            return Some("navigation.longitude".to_string());
        }

        None
    }
}

impl IntegrationTestResult {
    /// Print a summary of the test results
    pub fn print_summary(&self) {
        println!("\n=== Integration Test Results ===");
        println!("Overall: {}", if self.passed { "PASSED" } else { "FAILED" });
        println!("Duration: {:?}", self.duration);
        println!("Frames processed: {}", self.frames_processed);
        println!();

        println!("Phase Results:");
        for phase in &self.phase_results {
            let status = if phase.success { "✓" } else { "✗" };
            println!(
                "  {} {} ({:?}){}",
                status,
                phase.name,
                phase.duration,
                phase
                    .error
                    .as_ref()
                    .map(|e| format!(" - {}", e))
                    .unwrap_or_default()
            );
        }

        if !self.nan_inf_fields.is_empty() {
            println!();
            println!("NaN/Inf detected in fields:");
            for field in &self.nan_inf_fields {
                println!("  - {}", field);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_type_sim_id() {
        assert_eq!(AdapterType::Msfs.sim_id(), SimId::Msfs);
        assert_eq!(AdapterType::XPlane.sim_id(), SimId::XPlane);
        assert_eq!(AdapterType::Dcs.sim_id(), SimId::Dcs);
    }

    #[test]
    fn test_mock_adapter_lifecycle() {
        let mut adapter = MockAdapter::new(
            AdapterType::Msfs,
            AircraftId::new("C172"),
            ScenarioType::Cruise,
        );

        // Initially disconnected
        assert!(!adapter.is_connected());

        // Connect
        adapter.connect().unwrap();
        assert!(adapter.is_connected());

        // Process frame
        let snapshot = adapter.process_frame().unwrap();
        assert_eq!(snapshot.sim, SimId::Msfs);

        // Disconnect
        adapter.disconnect();
        assert!(!adapter.is_connected());

        // Reconnect
        adapter.connect().unwrap();
        assert!(adapter.is_connected());
    }

    #[test]
    fn test_mock_adapter_failure_simulation() {
        let mut adapter = MockAdapter::new(
            AdapterType::Msfs,
            AircraftId::new("C172"),
            ScenarioType::Cruise,
        )
        .with_simulate_failure(true);

        let result = adapter.connect();
        assert!(result.is_err());
    }

    /// Test MSFS adapter lifecycle
    /// Requirements: 14.1
    #[test]
    fn test_msfs_adapter_lifecycle() {
        let test = AdapterIntegrationTest::new(AdapterType::Msfs)
            .with_aircraft(AircraftId::new("C172"))
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(50);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "MSFS adapter lifecycle test should pass");
        assert!(result.connect_success, "Connect should succeed");
        assert!(!result.nan_inf_detected, "No NaN/Inf should be detected");
        assert!(result.disconnect_success, "Disconnect should succeed");
        assert!(result.reconnect_success, "Reconnect should succeed");
        assert!(result.frames_processed > 0, "Should process frames");
    }

    /// Test X-Plane adapter lifecycle
    /// Requirements: 14.1
    #[test]
    fn test_xplane_adapter_lifecycle() {
        let test = AdapterIntegrationTest::new(AdapterType::XPlane)
            .with_aircraft(AircraftId::new("C172"))
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(50);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "X-Plane adapter lifecycle test should pass");
        assert!(result.connect_success, "Connect should succeed");
        assert!(!result.nan_inf_detected, "No NaN/Inf should be detected");
        assert!(result.disconnect_success, "Disconnect should succeed");
        assert!(result.reconnect_success, "Reconnect should succeed");
    }

    /// Test DCS adapter lifecycle
    /// Requirements: 14.1
    #[test]
    fn test_dcs_adapter_lifecycle() {
        let test = AdapterIntegrationTest::new(AdapterType::Dcs)
            .with_aircraft(AircraftId::new("F16C"))
            .with_scenario(ScenarioType::Cruise)
            .with_frame_count(50);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "DCS adapter lifecycle test should pass");
        assert!(result.connect_success, "Connect should succeed");
        assert!(!result.nan_inf_detected, "No NaN/Inf should be detected");
        assert!(result.disconnect_success, "Disconnect should succeed");
        assert!(result.reconnect_success, "Reconnect should succeed");
    }

    /// Test all scenarios for each adapter
    /// Requirements: 14.1
    #[test]
    fn test_all_scenarios() {
        let scenarios = vec![
            ScenarioType::ColdAndDark,
            ScenarioType::GroundIdle,
            ScenarioType::Takeoff,
            ScenarioType::Cruise,
            ScenarioType::Approach,
            ScenarioType::Emergency,
        ];

        for adapter_type in [AdapterType::Msfs, AdapterType::XPlane, AdapterType::Dcs] {
            for scenario in &scenarios {
                let test = AdapterIntegrationTest::new(adapter_type)
                    .with_scenario(*scenario)
                    .with_frame_count(20);

                let result = test.run().unwrap();

                assert!(
                    result.passed,
                    "{:?} adapter with {:?} scenario should pass",
                    adapter_type, scenario
                );
            }
        }
    }

    /// Test helicopter scenario for DCS
    /// Requirements: 14.1
    #[test]
    fn test_dcs_helicopter_lifecycle() {
        let test = AdapterIntegrationTest::new(AdapterType::Dcs)
            .with_aircraft(AircraftId::new("UH1H"))
            .with_scenario(ScenarioType::HeloHover)
            .with_frame_count(50);

        let result = test.run().unwrap();
        result.print_summary();

        assert!(result.passed, "DCS helicopter lifecycle test should pass");
    }
}
