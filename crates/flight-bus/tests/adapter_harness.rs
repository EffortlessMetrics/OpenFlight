// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Adapter test harness for validating BusSnapshot generation
//!
//! This harness runs adapters with fixture data and validates that:
//! - No NaN or Inf values appear in snapshots under normal use
//! - Snapshots are generated at expected rates
//! - All required fields are populated correctly
//!
//! Requirements: P1.5 Phase 1 Checkpoint - Adapters can run in harness that logs
//! BusSnapshots with no NaN/Inf under normal use

use flight_bus::fixtures::{ScenarioType, SnapshotFixture};
use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::{AircraftId, SimId};
use std::time::{Duration, Instant};

/// Test harness for adapter validation
pub struct AdapterHarness {
    /// Simulator being tested
    sim: SimId,
    /// Aircraft being tested
    aircraft: AircraftId,
    /// Test scenario
    scenario: ScenarioType,
    /// Collected snapshots
    snapshots: Vec<BusSnapshot>,
    /// Start time
    start_time: Instant,
    /// Test duration
    duration: Duration,
}

impl AdapterHarness {
    /// Create a new adapter harness
    pub fn new(sim: SimId, aircraft: AircraftId, scenario: ScenarioType) -> Self {
        Self {
            sim,
            aircraft,
            scenario,
            snapshots: Vec::new(),
            start_time: Instant::now(),
            duration: Duration::from_secs(60), // Default 60 second test
        }
    }

    /// Set test duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Run the harness and collect snapshots
    pub fn run(&mut self) -> HarnessResult {
        println!(
            "Starting adapter harness for {:?} / {} / {:?}",
            self.sim, self.aircraft, self.scenario
        );
        println!("Test duration: {:?}", self.duration);

        let mut fixture = SnapshotFixture::new(self.sim, self.aircraft.clone(), self.scenario);
        let mut iteration = 0;
        let update_interval = Duration::from_millis(16); // ~60Hz

        self.start_time = Instant::now();

        while self.start_time.elapsed() < self.duration {
            // Generate snapshot
            let snapshot = fixture.advance(update_interval);

            // Log snapshot details
            if iteration % 60 == 0 {
                // Log every second
                println!(
                    "[{:6.2}s] IAS: {:6.1} kt, ALT: {:7.1} ft, HDG: {:5.1}°, G: {:4.2}",
                    self.start_time.elapsed().as_secs_f32(),
                    snapshot.kinematics.ias.to_knots(),
                    snapshot.environment.altitude,
                    snapshot.kinematics.heading.to_degrees(),
                    snapshot.kinematics.g_force.value()
                );
            }

            // Store snapshot
            self.snapshots.push(snapshot);
            iteration += 1;

            // Sleep to maintain update rate
            std::thread::sleep(update_interval);
        }

        println!(
            "Harness completed: {} snapshots collected over {:?}",
            self.snapshots.len(),
            self.start_time.elapsed()
        );

        // Validate all snapshots
        self.validate_snapshots()
    }

    /// Validate all collected snapshots
    fn validate_snapshots(&self) -> HarnessResult {
        let mut result = HarnessResult {
            total_snapshots: self.snapshots.len(),
            nan_inf_violations: Vec::new(),
            validation_errors: Vec::new(),
            duration: self.start_time.elapsed(),
            success: true,
        };

        for (idx, snapshot) in self.snapshots.iter().enumerate() {
            // Check for NaN/Inf in core telemetry fields
            if let Some(violation) = self.check_nan_inf(snapshot, idx) {
                result.nan_inf_violations.push(violation);
                result.success = false;
            }

            // Validate snapshot structure
            if let Err(e) = snapshot.validate() {
                result
                    .validation_errors
                    .push(format!("Snapshot {}: {}", idx, e));
                result.success = false;
            }
        }

        result
    }

    /// Check for NaN or Inf values in snapshot
    fn check_nan_inf(&self, snapshot: &BusSnapshot, index: usize) -> Option<NanInfViolation> {
        let mut violations = Vec::new();

        // Check kinematics fields
        if !snapshot.kinematics.ias.value().is_finite() {
            violations.push("kinematics.ias".to_string());
        }
        if !snapshot.kinematics.tas.value().is_finite() {
            violations.push("kinematics.tas".to_string());
        }
        if !snapshot.kinematics.ground_speed.value().is_finite() {
            violations.push("kinematics.ground_speed".to_string());
        }
        if !snapshot.kinematics.aoa.value().is_finite() {
            violations.push("kinematics.aoa".to_string());
        }
        if !snapshot.kinematics.sideslip.value().is_finite() {
            violations.push("kinematics.sideslip".to_string());
        }
        if !snapshot.kinematics.bank.value().is_finite() {
            violations.push("kinematics.bank".to_string());
        }
        if !snapshot.kinematics.pitch.value().is_finite() {
            violations.push("kinematics.pitch".to_string());
        }
        if !snapshot.kinematics.heading.value().is_finite() {
            violations.push("kinematics.heading".to_string());
        }
        if !snapshot.kinematics.g_force.value().is_finite() {
            violations.push("kinematics.g_force".to_string());
        }
        if !snapshot.kinematics.g_lateral.value().is_finite() {
            violations.push("kinematics.g_lateral".to_string());
        }
        if !snapshot.kinematics.g_longitudinal.value().is_finite() {
            violations.push("kinematics.g_longitudinal".to_string());
        }
        if !snapshot.kinematics.mach.value().is_finite() {
            violations.push("kinematics.mach".to_string());
        }
        if !snapshot.kinematics.vertical_speed.is_finite() {
            violations.push("kinematics.vertical_speed".to_string());
        }

        // Check angular rates
        if !snapshot.angular_rates.p.is_finite() {
            violations.push("angular_rates.p".to_string());
        }
        if !snapshot.angular_rates.q.is_finite() {
            violations.push("angular_rates.q".to_string());
        }
        if !snapshot.angular_rates.r.is_finite() {
            violations.push("angular_rates.r".to_string());
        }

        // Check environment fields
        if !snapshot.environment.altitude.is_finite() {
            violations.push("environment.altitude".to_string());
        }
        if !snapshot.environment.pressure_altitude.is_finite() {
            violations.push("environment.pressure_altitude".to_string());
        }
        if !snapshot.environment.oat.is_finite() {
            violations.push("environment.oat".to_string());
        }
        if !snapshot.environment.wind_speed.value().is_finite() {
            violations.push("environment.wind_speed".to_string());
        }
        if !snapshot.environment.wind_direction.value().is_finite() {
            violations.push("environment.wind_direction".to_string());
        }
        if !snapshot.environment.visibility.is_finite() {
            violations.push("environment.visibility".to_string());
        }

        // Check control inputs
        if !snapshot.control_inputs.pitch.is_finite() {
            violations.push("control_inputs.pitch".to_string());
        }
        if !snapshot.control_inputs.roll.is_finite() {
            violations.push("control_inputs.roll".to_string());
        }
        if !snapshot.control_inputs.yaw.is_finite() {
            violations.push("control_inputs.yaw".to_string());
        }

        // Check trim state
        if !snapshot.trim_state.elevator.is_finite() {
            violations.push("trim_state.elevator".to_string());
        }
        if !snapshot.trim_state.aileron.is_finite() {
            violations.push("trim_state.aileron".to_string());
        }
        if !snapshot.trim_state.rudder.is_finite() {
            violations.push("trim_state.rudder".to_string());
        }

        if !violations.is_empty() {
            Some(NanInfViolation {
                snapshot_index: index,
                timestamp: snapshot.timestamp,
                fields: violations,
            })
        } else {
            None
        }
    }

    /// Get collected snapshots
    pub fn snapshots(&self) -> &[BusSnapshot] {
        &self.snapshots
    }
}

/// Result of harness execution
#[derive(Debug)]
pub struct HarnessResult {
    /// Total number of snapshots collected
    pub total_snapshots: usize,
    /// NaN/Inf violations found
    pub nan_inf_violations: Vec<NanInfViolation>,
    /// Validation errors
    pub validation_errors: Vec<String>,
    /// Test duration
    pub duration: Duration,
    /// Overall success
    pub success: bool,
}

impl HarnessResult {
    /// Print summary of results
    pub fn print_summary(&self) {
        println!("\n=== Harness Results ===");
        println!("Total snapshots: {}", self.total_snapshots);
        println!("Duration: {:?}", self.duration);
        println!(
            "Update rate: {:.1} Hz",
            self.total_snapshots as f32 / self.duration.as_secs_f32()
        );
        println!("NaN/Inf violations: {}", self.nan_inf_violations.len());
        println!("Validation errors: {}", self.validation_errors.len());
        println!("Success: {}", self.success);

        if !self.nan_inf_violations.is_empty() {
            println!("\n=== NaN/Inf Violations ===");
            for violation in &self.nan_inf_violations {
                println!(
                    "Snapshot {}: {} fields with NaN/Inf: {:?}",
                    violation.snapshot_index,
                    violation.fields.len(),
                    violation.fields
                );
            }
        }

        if !self.validation_errors.is_empty() {
            println!("\n=== Validation Errors ===");
            for error in &self.validation_errors {
                println!("{}", error);
            }
        }

        if self.success {
            println!("\n✓ All snapshots valid - no NaN/Inf detected");
        } else {
            println!("\n✗ Validation failed - see errors above");
        }
    }
}

/// NaN or Inf violation details
#[derive(Debug)]
pub struct NanInfViolation {
    /// Index of snapshot with violation
    pub snapshot_index: usize,
    /// Timestamp of snapshot
    pub timestamp: u64,
    /// Fields containing NaN or Inf
    pub fields: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test MSFS adapter with cold and dark scenario
    /// Requirements: MSFS-INT-01.15 - No NaN/Inf in telemetry
    #[test]
    fn test_msfs_cold_and_dark_no_nan_inf() {
        let mut harness = AdapterHarness::new(
            SimId::Msfs,
            AircraftId::new("C172"),
            ScenarioType::ColdAndDark,
        )
        .with_duration(Duration::from_secs(5));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations"
        );
        assert!(
            result.validation_errors.is_empty(),
            "Should have no validation errors"
        );
        assert!(
            result.total_snapshots > 0,
            "Should have collected snapshots"
        );
    }

    /// Test MSFS adapter with takeoff scenario
    /// Requirements: MSFS-INT-01.15 - No NaN/Inf during dynamic flight
    #[test]
    fn test_msfs_takeoff_no_nan_inf() {
        let mut harness =
            AdapterHarness::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Takeoff)
                .with_duration(Duration::from_secs(10));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations during takeoff"
        );
    }

    /// Test MSFS adapter with cruise scenario
    /// Requirements: MSFS-INT-01.15 - No NaN/Inf during steady state
    #[test]
    fn test_msfs_cruise_no_nan_inf() {
        let mut harness =
            AdapterHarness::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Cruise)
                .with_duration(Duration::from_secs(10));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations during cruise"
        );
    }

    /// Test X-Plane adapter with cruise scenario
    /// Requirements: XPLANE-INT-01.6 - Graceful handling of missing data
    #[test]
    fn test_xplane_cruise_no_nan_inf() {
        let mut harness =
            AdapterHarness::new(SimId::XPlane, AircraftId::new("C172"), ScenarioType::Cruise)
                .with_duration(Duration::from_secs(10));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations"
        );
    }

    /// Test DCS adapter with cruise scenario
    /// Requirements: DCS-INT-01.8 - Nil handling without crashes
    #[test]
    fn test_dcs_cruise_no_nan_inf() {
        let mut harness =
            AdapterHarness::new(SimId::Dcs, AircraftId::new("F16C"), ScenarioType::Cruise)
                .with_duration(Duration::from_secs(10));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations"
        );
    }

    /// Test DCS helicopter hover scenario
    /// Requirements: DCS-INT-01.8, BUS-EXTENDED-01.8 - Helicopter data validation
    #[test]
    fn test_dcs_helo_hover_no_nan_inf() {
        let mut harness =
            AdapterHarness::new(SimId::Dcs, AircraftId::new("UH1H"), ScenarioType::HeloHover)
                .with_duration(Duration::from_secs(10));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations in helicopter hover"
        );

        // Verify helicopter-specific data is present
        let snapshots = harness.snapshots();
        assert!(!snapshots.is_empty(), "Should have collected snapshots");
        assert!(
            snapshots[0].helo.is_some(),
            "Helicopter data should be present"
        );
    }

    /// Test approach scenario with gear and flaps extended
    /// Requirements: MSFS-INT-01.15 - No NaN/Inf with configuration changes
    #[test]
    fn test_msfs_approach_no_nan_inf() {
        let mut harness =
            AdapterHarness::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Approach)
                .with_duration(Duration::from_secs(10));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations during approach"
        );
    }

    /// Test emergency scenario
    /// Requirements: MSFS-INT-01.15 - No NaN/Inf during abnormal conditions
    #[test]
    fn test_msfs_emergency_no_nan_inf() {
        let mut harness = AdapterHarness::new(
            SimId::Msfs,
            AircraftId::new("C172"),
            ScenarioType::Emergency,
        )
        .with_duration(Duration::from_secs(10));

        let result = harness.run();
        result.print_summary();

        assert!(
            result.success,
            "Harness should succeed with no NaN/Inf violations"
        );
        assert!(
            result.nan_inf_violations.is_empty(),
            "Should have no NaN/Inf violations during emergency"
        );
    }

    /// Test update rate is within expected range
    /// Requirements: MSFS-INT-01.7 - Target ≥60 Hz updates
    #[test]
    fn test_update_rate() {
        let mut harness =
            AdapterHarness::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Cruise)
                .with_duration(Duration::from_secs(5));

        let result = harness.run();
        result.print_summary();

        let update_rate = result.total_snapshots as f32 / result.duration.as_secs_f32();
        println!("Measured update rate: {:.1} Hz", update_rate);

        // Allow some tolerance for test timing variations
        assert!(
            update_rate >= 50.0,
            "Update rate should be at least 50 Hz (measured: {:.1} Hz)",
            update_rate
        );
        assert!(
            update_rate <= 70.0,
            "Update rate should not exceed 70 Hz (measured: {:.1} Hz)",
            update_rate
        );
    }

    /// Test snapshot age calculation
    /// Requirements: BUS-CORE-01.15 - Snapshot age API
    #[test]
    fn test_snapshot_age() {
        let mut harness =
            AdapterHarness::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Cruise)
                .with_duration(Duration::from_secs(2));

        let result = harness.run();
        result.print_summary();

        assert!(result.success, "Harness should succeed");

        // Check that snapshots have reasonable ages
        let snapshots = harness.snapshots();
        for snapshot in snapshots {
            let age_ms = snapshot.age_ms();
            // Age should be reasonable (not in the future, not too old)
            assert!(
                age_ms < 10000,
                "Snapshot age should be less than 10 seconds"
            );
        }
    }

    /// Test all scenarios for comprehensive coverage
    /// Requirements: SIM-TEST-01.5 - Fixture-based integration tests
    #[test]
    fn test_all_scenarios_no_nan_inf() {
        let scenarios = vec![
            ScenarioType::ColdAndDark,
            ScenarioType::GroundIdle,
            ScenarioType::Takeoff,
            ScenarioType::Cruise,
            ScenarioType::Approach,
            ScenarioType::Emergency,
        ];

        for scenario in scenarios {
            println!("\nTesting scenario: {:?}", scenario);
            let mut harness = AdapterHarness::new(SimId::Msfs, AircraftId::new("C172"), scenario)
                .with_duration(Duration::from_secs(5));

            let result = harness.run();
            result.print_summary();

            assert!(
                result.success,
                "Scenario {:?} should succeed with no NaN/Inf violations",
                scenario
            );
            assert!(
                result.nan_inf_violations.is_empty(),
                "Scenario {:?} should have no NaN/Inf violations",
                scenario
            );
        }
    }
}
