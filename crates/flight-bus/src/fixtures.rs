// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Test fixtures for consistent snapshot publishing and validation

use crate::snapshot::{
    AircraftConfig, BusSnapshot, EngineData, Environment, HeloData, Kinematics, LightsConfig,
    Navigation,
};
use crate::types::{
    AircraftId, AutopilotState, GForce, GearPosition, GearState, Mach, Percentage, SimId,
    ValidatedAngle, ValidatedSpeed,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Fixture generator for creating consistent test snapshots
pub struct SnapshotFixture {
    sim: SimId,
    aircraft: AircraftId,
    scenario: ScenarioType,
    time_offset: Duration,
}

/// Different flight scenarios for testing
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScenarioType {
    /// Aircraft on ground, engines off
    ColdAndDark,
    /// Aircraft on ground, engines running
    GroundIdle,
    /// Normal takeoff sequence
    Takeoff,
    /// Cruise flight
    Cruise,
    /// Approach and landing
    Approach,
    /// Emergency scenarios
    Emergency,
    /// Helicopter-specific scenarios
    HeloHover,
    /// Helicopter hover
    Hover,
}

impl SnapshotFixture {
    /// Create a new fixture generator
    pub fn new(sim: SimId, aircraft: AircraftId, scenario: ScenarioType) -> Self {
        Self {
            sim,
            aircraft,
            scenario,
            time_offset: Duration::ZERO,
        }
    }

    /// Generate a snapshot for the current time offset
    pub fn generate(&self) -> BusSnapshot {
        let mut snapshot = BusSnapshot::new(self.sim, self.aircraft.clone());

        match self.scenario {
            ScenarioType::ColdAndDark => self.apply_cold_and_dark(&mut snapshot),
            ScenarioType::GroundIdle => self.apply_ground_idle(&mut snapshot),
            ScenarioType::Takeoff => self.apply_takeoff(&mut snapshot),
            ScenarioType::Cruise => self.apply_cruise(&mut snapshot),
            ScenarioType::Approach => self.apply_approach(&mut snapshot),
            ScenarioType::Emergency => self.apply_emergency(&mut snapshot),
            ScenarioType::HeloHover | ScenarioType::Hover => self.apply_helo_hover(&mut snapshot),
        }

        snapshot
    }

    /// Advance time and generate next snapshot
    pub fn advance(&mut self, delta: Duration) -> BusSnapshot {
        self.time_offset += delta;
        self.generate()
    }

    /// Reset time offset
    pub fn reset(&mut self) {
        self.time_offset = Duration::ZERO;
    }

    /// Get current time offset in seconds
    pub fn time_seconds(&self) -> f32 {
        self.time_offset.as_secs_f32()
    }

    fn apply_cold_and_dark(&self, snapshot: &mut BusSnapshot) {
        // Aircraft on ground, everything off
        snapshot.kinematics = Kinematics {
            ias: ValidatedSpeed::new_knots(0.0).unwrap(),
            tas: ValidatedSpeed::new_knots(0.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(0.0).unwrap(),
            aoa: ValidatedAngle::new_degrees(0.0).unwrap(),
            sideslip: ValidatedAngle::new_degrees(0.0).unwrap(),
            bank: ValidatedAngle::new_degrees(0.0).unwrap(),
            pitch: ValidatedAngle::new_degrees(0.0).unwrap(),
            heading: ValidatedAngle::new_degrees(90.0).unwrap(), // Runway heading
            g_force: GForce::new(1.0).unwrap(),
            g_lateral: GForce::new(0.0).unwrap(),
            g_longitudinal: GForce::new(0.0).unwrap(),
            mach: Mach::new(0.0).unwrap(),
            vertical_speed: 0.0,
        };

        snapshot.config = AircraftConfig {
            gear: GearState {
                nose: GearPosition::Down,
                left: GearPosition::Down,
                right: GearPosition::Down,
            },
            flaps: Percentage::new(0.0).unwrap(),
            spoilers: Percentage::new(0.0).unwrap(),
            ap_state: AutopilotState::Off,
            ap_altitude: None,
            ap_heading: None,
            ap_speed: None,
            lights: LightsConfig::default(),
            fuel: {
                let mut fuel = HashMap::new();
                fuel.insert("left".to_string(), Percentage::new(100.0).unwrap());
                fuel.insert("right".to_string(), Percentage::new(100.0).unwrap());
                fuel
            },
        };

        // No engines running
        snapshot.engines = vec![EngineData {
            index: 0,
            running: false,
            rpm: Percentage::new(0.0).unwrap(),
            manifold_pressure: Some(29.92),
            egt: Some(15.0),
            cht: Some(15.0),
            fuel_flow: Some(0.0),
            oil_pressure: Some(0.0),
            oil_temperature: Some(15.0),
        }];

        snapshot.environment = Environment {
            altitude: 1000.0, // Airport elevation
            pressure_altitude: 1000.0,
            oat: 15.0,
            wind_speed: ValidatedSpeed::new_knots(5.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(120.0).unwrap(),
            visibility: 10.0,
            cloud_coverage: Percentage::new(25.0).unwrap(),
        };
    }

    fn apply_ground_idle(&self, snapshot: &mut BusSnapshot) {
        self.apply_cold_and_dark(snapshot);

        // Engine running at idle
        snapshot.engines[0] = EngineData {
            index: 0,
            running: true,
            rpm: Percentage::new(25.0).unwrap(), // Idle RPM
            manifold_pressure: Some(12.0),
            egt: Some(650.0),
            cht: Some(180.0),
            fuel_flow: Some(8.5),
            oil_pressure: Some(45.0),
            oil_temperature: Some(75.0),
        };

        // Some lights on
        snapshot.config.lights = LightsConfig {
            nav: true,
            beacon: true,
            strobe: false,
            landing: false,
            taxi: true,
            logo: false,
            wing: false,
        };
    }

    fn apply_takeoff(&self, snapshot: &mut BusSnapshot) {
        let t = self.time_seconds();

        // Simulate takeoff roll and rotation
        let ground_speed = (t * 5.0).min(80.0); // Accelerate to 80 knots
        let ias = ground_speed + 2.0; // Slight difference due to wind

        snapshot.kinematics = Kinematics {
            ias: ValidatedSpeed::new_knots(ias).unwrap(),
            tas: ValidatedSpeed::new_knots(ias + 3.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(ground_speed).unwrap(),
            aoa: ValidatedAngle::new_degrees(if t > 15.0 { 8.0 } else { 2.0 }).unwrap(),
            sideslip: ValidatedAngle::new_degrees(0.5).unwrap(),
            bank: ValidatedAngle::new_degrees(0.0).unwrap(),
            pitch: ValidatedAngle::new_degrees(if t > 15.0 { 10.0 } else { 0.0 }).unwrap(),
            heading: ValidatedAngle::new_degrees(90.0).unwrap(),
            g_force: GForce::new(if t > 15.0 { 1.2 } else { 1.0 }).unwrap(),
            g_lateral: GForce::new(0.1).unwrap(),
            g_longitudinal: GForce::new(0.3).unwrap(),
            mach: Mach::new((ias / 661.0).min(0.3)).unwrap(),
            vertical_speed: if t > 15.0 { 500.0 } else { 0.0 },
        };

        // Gear up after rotation
        let gear_pos = if t > 20.0 {
            GearPosition::Up
        } else if t > 17.0 {
            GearPosition::Transitioning
        } else {
            GearPosition::Down
        };

        snapshot.config = AircraftConfig {
            gear: GearState {
                nose: gear_pos,
                left: gear_pos,
                right: gear_pos,
            },
            flaps: Percentage::new(10.0).unwrap(), // Takeoff flaps
            spoilers: Percentage::new(0.0).unwrap(),
            ap_state: AutopilotState::Off,
            ap_altitude: None,
            ap_heading: None,
            ap_speed: None,
            lights: LightsConfig {
                nav: true,
                beacon: true,
                strobe: true,
                landing: true,
                taxi: false,
                logo: true,
                wing: true,
            },
            fuel: {
                let mut fuel = HashMap::new();
                let consumption = (t * 0.1).min(5.0); // Fuel consumption
                fuel.insert(
                    "left".to_string(),
                    Percentage::new(100.0 - consumption).unwrap(),
                );
                fuel.insert(
                    "right".to_string(),
                    Percentage::new(100.0 - consumption).unwrap(),
                );
                fuel
            },
        };

        // Full power engine
        snapshot.engines = vec![EngineData {
            index: 0,
            running: true,
            rpm: Percentage::new(100.0).unwrap(),
            manifold_pressure: Some(29.0),
            egt: Some(1200.0),
            cht: Some(380.0),
            fuel_flow: Some(18.5),
            oil_pressure: Some(85.0),
            oil_temperature: Some(195.0),
        }];

        // Altitude increases after rotation
        snapshot.environment.altitude = 1000.0 + if t > 15.0 { (t - 15.0) * 100.0 } else { 0.0 };
        snapshot.environment.pressure_altitude = snapshot.environment.altitude;
    }

    fn apply_cruise(&self, snapshot: &mut BusSnapshot) {
        let t = self.time_seconds();

        snapshot.kinematics = Kinematics {
            ias: ValidatedSpeed::new_knots(120.0).unwrap(),
            tas: ValidatedSpeed::new_knots(135.0).unwrap(), // Higher TAS at altitude
            ground_speed: ValidatedSpeed::new_knots(140.0).unwrap(), // Tailwind
            aoa: ValidatedAngle::new_degrees(3.0).unwrap(),
            sideslip: ValidatedAngle::new_degrees(0.0).unwrap(),
            bank: ValidatedAngle::new_degrees((t * 0.1).sin() * 2.0).unwrap(), // Gentle turns
            pitch: ValidatedAngle::new_degrees(2.0).unwrap(),
            heading: ValidatedAngle::new_degrees(90.0 + t * 0.5).unwrap(), // Slow turn
            g_force: GForce::new(1.0).unwrap(),
            g_lateral: GForce::new(0.0).unwrap(),
            g_longitudinal: GForce::new(0.0).unwrap(),
            mach: Mach::new(0.2).unwrap(),
            vertical_speed: 0.0,
        };

        snapshot.config = AircraftConfig {
            gear: GearState {
                nose: GearPosition::Up,
                left: GearPosition::Up,
                right: GearPosition::Up,
            },
            flaps: Percentage::new(0.0).unwrap(),
            spoilers: Percentage::new(0.0).unwrap(),
            ap_state: AutopilotState::Engaged,
            ap_altitude: Some(5500.0),
            ap_heading: Some(ValidatedAngle::new_degrees(90.0).unwrap()),
            ap_speed: Some(ValidatedSpeed::new_knots(120.0).unwrap()),
            lights: LightsConfig {
                nav: true,
                beacon: true,
                strobe: true,
                landing: false,
                taxi: false,
                logo: false,
                wing: false,
            },
            fuel: {
                let mut fuel = HashMap::new();
                let consumption = 20.0 + t * 0.05; // Gradual fuel consumption
                fuel.insert(
                    "left".to_string(),
                    Percentage::new((100.0 - consumption).max(0.0)).unwrap(),
                );
                fuel.insert(
                    "right".to_string(),
                    Percentage::new((100.0 - consumption).max(0.0)).unwrap(),
                );
                fuel
            },
        };

        // Cruise power engine
        snapshot.engines = vec![EngineData {
            index: 0,
            running: true,
            rpm: Percentage::new(75.0).unwrap(),
            manifold_pressure: Some(23.0),
            egt: Some(1050.0),
            cht: Some(350.0),
            fuel_flow: Some(12.5),
            oil_pressure: Some(75.0),
            oil_temperature: Some(185.0),
        }];

        snapshot.environment = Environment {
            altitude: 5500.0,
            pressure_altitude: 5500.0,
            oat: 5.0, // Cooler at altitude
            wind_speed: ValidatedSpeed::new_knots(15.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(45.0).unwrap(),
            visibility: 15.0,
            cloud_coverage: Percentage::new(10.0).unwrap(),
        };

        snapshot.navigation = Navigation {
            latitude: 40.7128 + (t * 0.0001) as f64, // Moving north slowly
            longitude: -74.0060 + (t * 0.0001) as f64, // Moving east slowly
            ground_track: ValidatedAngle::new_degrees(45.0).unwrap(),
            distance_to_dest: Some(50.0 - t * 0.1),
            time_to_dest: Some(25.0 - t * 0.05),
            active_waypoint: Some("KNYC".to_string()),
        };
    }

    fn apply_approach(&self, snapshot: &mut BusSnapshot) {
        let t = self.time_seconds();

        // Descending approach
        let altitude = (3000.0 - t * 50.0).max(1000.0);
        let ias = 85.0 + (t * 0.5).min(10.0); // Gradually slow down

        snapshot.kinematics = Kinematics {
            ias: ValidatedSpeed::new_knots(ias).unwrap(),
            tas: ValidatedSpeed::new_knots(ias + 2.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(ias - 5.0).unwrap(), // Headwind
            aoa: ValidatedAngle::new_degrees(5.0).unwrap(),
            sideslip: ValidatedAngle::new_degrees(1.0).unwrap(),
            bank: ValidatedAngle::new_degrees(0.0).unwrap(),
            pitch: ValidatedAngle::new_degrees(-3.0).unwrap(), // Descent attitude
            heading: ValidatedAngle::new_degrees(90.0).unwrap(),
            g_force: GForce::new(1.0).unwrap(),
            g_lateral: GForce::new(0.0).unwrap(),
            g_longitudinal: GForce::new(0.0).unwrap(),
            mach: Mach::new(ias / 661.0).unwrap(),
            vertical_speed: -500.0,
        };

        // Gear and flaps extended
        snapshot.config = AircraftConfig {
            gear: GearState {
                nose: GearPosition::Down,
                left: GearPosition::Down,
                right: GearPosition::Down,
            },
            flaps: Percentage::new(30.0).unwrap(), // Approach flaps
            spoilers: Percentage::new(0.0).unwrap(),
            ap_state: AutopilotState::Engaged,
            ap_altitude: Some(altitude),
            ap_heading: Some(ValidatedAngle::new_degrees(90.0).unwrap()),
            ap_speed: Some(ValidatedSpeed::new_knots(ias).unwrap()),
            lights: LightsConfig {
                nav: true,
                beacon: true,
                strobe: true,
                landing: true,
                taxi: true,
                logo: true,
                wing: true,
            },
            fuel: {
                let mut fuel = HashMap::new();
                fuel.insert("left".to_string(), Percentage::new(35.0).unwrap());
                fuel.insert("right".to_string(), Percentage::new(35.0).unwrap());
                fuel
            },
        };

        snapshot.engines = vec![EngineData {
            index: 0,
            running: true,
            rpm: Percentage::new(60.0).unwrap(),
            manifold_pressure: Some(18.0),
            egt: Some(950.0),
            cht: Some(320.0),
            fuel_flow: Some(10.0),
            oil_pressure: Some(70.0),
            oil_temperature: Some(175.0),
        }];

        snapshot.environment.altitude = altitude;
        snapshot.environment.pressure_altitude = altitude;
    }

    fn apply_emergency(&self, snapshot: &mut BusSnapshot) {
        // Engine failure scenario
        self.apply_cruise(snapshot);

        // Failed engine
        snapshot.engines[0] = EngineData {
            index: 0,
            running: false,
            rpm: Percentage::new(0.0).unwrap(),
            manifold_pressure: Some(10.0),
            egt: Some(200.0),
            cht: Some(150.0),
            fuel_flow: Some(0.0),
            oil_pressure: Some(0.0),
            oil_temperature: Some(100.0),
        };

        // Emergency descent
        snapshot.kinematics.vertical_speed = -1000.0;
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(90.0).unwrap(); // Best glide speed
        snapshot.config.ap_state = AutopilotState::Off;
    }

    fn apply_helo_hover(&self, snapshot: &mut BusSnapshot) {
        let t = self.time_seconds();

        // Hovering helicopter with small oscillations
        snapshot.kinematics = Kinematics {
            ias: ValidatedSpeed::new_knots(0.0).unwrap(),
            tas: ValidatedSpeed::new_knots(0.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(2.0).unwrap(), // Slight drift
            aoa: ValidatedAngle::new_degrees(0.0).unwrap(),
            sideslip: ValidatedAngle::new_degrees((t * 2.0).sin() * 1.0).unwrap(),
            bank: ValidatedAngle::new_degrees((t * 1.5).sin() * 2.0).unwrap(),
            pitch: ValidatedAngle::new_degrees((t * 1.8).sin() * 1.5).unwrap(),
            heading: ValidatedAngle::new_degrees(180.0).unwrap(),
            g_force: GForce::new(1.0).unwrap(),
            g_lateral: GForce::new((t * 2.0).sin() * 0.1).unwrap(),
            g_longitudinal: GForce::new((t * 1.8).sin() * 0.1).unwrap(),
            mach: Mach::new(0.0).unwrap(),
            vertical_speed: (t * 3.0).sin() * 50.0, // Small vertical oscillations
        };

        // Helicopter-specific data
        snapshot.helo = Some(HeloData {
            nr: Percentage::new(100.0).unwrap(), // Main rotor at 100%
            np: Percentage::new(100.0).unwrap(), // Power turbine at 100%
            torque: Percentage::new(65.0 + (t * 0.5).sin() * 5.0).unwrap(), // Varying torque
            collective: Percentage::new(45.0 + (t * 0.3).sin() * 3.0).unwrap(), // Small collective inputs
            pedals: (t * 1.2).sin() * 10.0, // Anti-torque pedal inputs
        });

        snapshot.environment = Environment {
            altitude: 500.0 + (t * 3.0).sin() * 10.0, // Hovering with small altitude changes
            pressure_altitude: 500.0,
            oat: 25.0,
            wind_speed: ValidatedSpeed::new_knots(8.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(-90.0).unwrap(), // 270° = -90°
            visibility: 10.0,
            cloud_coverage: Percentage::new(0.0).unwrap(),
        };
    }
}

/// Fixture validator for ensuring snapshot consistency
pub struct SnapshotValidator {
    tolerance: ValidationTolerance,
}

/// Validation tolerances for different fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTolerance {
    pub speed_knots: f32,
    pub angle_degrees: f32,
    pub altitude_feet: f32,
    pub percentage: f32,
    pub g_force: f32,
}

impl Default for ValidationTolerance {
    fn default() -> Self {
        Self {
            speed_knots: 0.1,
            angle_degrees: 0.1,
            altitude_feet: 1.0,
            percentage: 0.1,
            g_force: 0.01,
        }
    }
}

impl SnapshotValidator {
    pub fn new(tolerance: ValidationTolerance) -> Self {
        Self { tolerance }
    }

    /// Validate that two snapshots are within tolerance
    pub fn validate_consistency(&self, a: &BusSnapshot, b: &BusSnapshot) -> Result<(), String> {
        // Check basic fields match
        if a.sim != b.sim {
            return Err(format!("Sim mismatch: {:?} != {:?}", a.sim, b.sim));
        }

        if a.aircraft != b.aircraft {
            return Err(format!(
                "Aircraft mismatch: {:?} != {:?}",
                a.aircraft, b.aircraft
            ));
        }

        // Check kinematics within tolerance
        self.validate_speed_tolerance(
            a.kinematics.ias.to_knots(),
            b.kinematics.ias.to_knots(),
            "IAS",
        )?;
        self.validate_angle_tolerance(
            a.kinematics.heading.to_degrees(),
            b.kinematics.heading.to_degrees(),
            "heading",
        )?;
        self.validate_g_force_tolerance(
            a.kinematics.g_force.value(),
            b.kinematics.g_force.value(),
            "g_force",
        )?;

        Ok(())
    }

    fn validate_speed_tolerance(&self, a: f32, b: f32, field: &str) -> Result<(), String> {
        if (a - b).abs() > self.tolerance.speed_knots {
            Err(format!(
                "{} tolerance exceeded: {} vs {} (tolerance: {})",
                field, a, b, self.tolerance.speed_knots
            ))
        } else {
            Ok(())
        }
    }

    fn validate_angle_tolerance(&self, a: f32, b: f32, field: &str) -> Result<(), String> {
        if (a - b).abs() > self.tolerance.angle_degrees {
            Err(format!(
                "{} tolerance exceeded: {} vs {} (tolerance: {})",
                field, a, b, self.tolerance.angle_degrees
            ))
        } else {
            Ok(())
        }
    }

    fn validate_g_force_tolerance(&self, a: f32, b: f32, field: &str) -> Result<(), String> {
        if (a - b).abs() > self.tolerance.g_force {
            Err(format!(
                "{} tolerance exceeded: {} vs {} (tolerance: {})",
                field, a, b, self.tolerance.g_force
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_cold_and_dark() {
        let fixture = SnapshotFixture::new(
            SimId::Msfs,
            AircraftId::new("C172"),
            ScenarioType::ColdAndDark,
        );
        let snapshot = fixture.generate();

        assert_eq!(snapshot.sim, SimId::Msfs);
        assert_eq!(snapshot.aircraft.icao, "C172");
        assert_eq!(snapshot.kinematics.ias.value(), 0.0);
        assert!(!snapshot.engines[0].running);
        assert!(snapshot.config.gear.all_down());
    }

    #[test]
    fn test_fixture_takeoff_progression() {
        let mut fixture =
            SnapshotFixture::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Takeoff);

        // Initial state
        let snapshot1 = fixture.generate();
        assert_eq!(snapshot1.kinematics.ias.value(), 2.0); // 0 * 5.0 + 2.0 = 2.0 (wind effect)
        assert_eq!(snapshot1.kinematics.aoa.to_degrees(), 2.0); // Ground roll AoA
        assert!(snapshot1.config.gear.all_down());

        // After 10 seconds
        let snapshot2 = fixture.advance(Duration::from_secs(10));
        assert!(snapshot2.kinematics.ias.value() > 0.0);
        assert!(snapshot2.config.gear.all_down());

        // After 20 seconds (gear should be up)
        let snapshot3 = fixture.advance(Duration::from_secs(10));
        assert!(snapshot3.kinematics.ias.value() > snapshot2.kinematics.ias.value());
        // At 20 seconds, gear should be up (transition happens at 17-20s)
        assert!(snapshot3.config.gear.all_up() || snapshot3.config.gear.transitioning());
    }

    #[test]
    fn test_helo_hover_fixture() {
        let fixture =
            SnapshotFixture::new(SimId::Dcs, AircraftId::new("UH1H"), ScenarioType::HeloHover);
        let snapshot = fixture.generate();

        assert_eq!(snapshot.sim, SimId::Dcs);
        assert!(snapshot.helo.is_some());

        let helo = snapshot.helo.unwrap();
        assert_eq!(helo.nr.value(), 100.0);
        assert_eq!(helo.np.value(), 100.0);
    }

    #[test]
    fn test_snapshot_validator() {
        let validator = SnapshotValidator::new(ValidationTolerance::default());

        let snapshot1 =
            SnapshotFixture::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Cruise)
                .generate();
        let snapshot2 =
            SnapshotFixture::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Cruise)
                .generate();

        // Same scenario should validate
        assert!(
            validator
                .validate_consistency(&snapshot1, &snapshot2)
                .is_ok()
        );

        // Different aircraft should fail
        let snapshot3 =
            SnapshotFixture::new(SimId::Msfs, AircraftId::new("A320"), ScenarioType::Cruise)
                .generate();
        assert!(
            validator
                .validate_consistency(&snapshot1, &snapshot3)
                .is_err()
        );
    }

    #[test]
    fn test_fixture_time_advancement() {
        let mut fixture =
            SnapshotFixture::new(SimId::Msfs, AircraftId::new("C172"), ScenarioType::Takeoff);

        assert_eq!(fixture.time_seconds(), 0.0);

        fixture.advance(Duration::from_secs(5));
        assert_eq!(fixture.time_seconds(), 5.0);

        fixture.reset();
        assert_eq!(fixture.time_seconds(), 0.0);
    }
}
