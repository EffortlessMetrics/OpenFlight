// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Adapter helpers and converter functions for different simulators

use crate::snapshot::{BusSnapshot, EngineData};
use crate::types::{
    BusTypeError, GForce, Mach, Percentage, SimId, ValidatedAngle, ValidatedSpeed,
};

/// Adapter helper trait for converting simulator-specific data to bus format
pub trait SimAdapter {
    type RawData;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Convert raw simulator data to normalized bus snapshot
    fn convert_to_snapshot(&self, raw: Self::RawData) -> Result<BusSnapshot, Self::Error>;

    /// Get simulator identifier
    fn sim_id(&self) -> SimId;

    /// Validate raw data before conversion
    fn validate_raw_data(&self, raw: &Self::RawData) -> Result<(), Self::Error>;
}

/// MSFS adapter helper functions
pub mod msfs {
    use super::*;

    /// Convert MSFS SimConnect units to normalized values
    pub struct MsfsConverter;

    impl MsfsConverter {
        /// Convert MSFS indicated airspeed (knots) to ValidatedSpeed
        pub fn convert_ias(value: f64) -> Result<ValidatedSpeed, BusTypeError> {
            ValidatedSpeed::new_knots(value as f32)
        }

        /// Convert MSFS true airspeed (knots) to ValidatedSpeed
        pub fn convert_tas(value: f64) -> Result<ValidatedSpeed, BusTypeError> {
            ValidatedSpeed::new_knots(value as f32)
        }

        /// Convert MSFS ground speed (knots) to ValidatedSpeed
        pub fn convert_ground_speed(value: f64) -> Result<ValidatedSpeed, BusTypeError> {
            ValidatedSpeed::new_knots(value as f32)
        }

        /// Convert MSFS angle (degrees) to ValidatedAngle
        pub fn convert_angle_degrees(value: f64) -> Result<ValidatedAngle, BusTypeError> {
            // Normalize to -180 to 180 range
            let normalized = ((value % 360.0) + 540.0) % 360.0 - 180.0;
            ValidatedAngle::new_degrees(normalized as f32)
        }

        /// Convert MSFS angle (radians) to ValidatedAngle
        pub fn convert_angle_radians(value: f64) -> Result<ValidatedAngle, BusTypeError> {
            ValidatedAngle::new_radians(value as f32)
        }

        /// Convert MSFS G-force to GForce
        pub fn convert_g_force(value: f64) -> Result<GForce, BusTypeError> {
            GForce::new(value as f32)
        }

        /// Convert MSFS Mach number to Mach
        pub fn convert_mach(value: f64) -> Result<Mach, BusTypeError> {
            Mach::new(value as f32)
        }

        /// Convert MSFS percentage (0-100) to Percentage
        pub fn convert_percentage(value: f64) -> Result<Percentage, BusTypeError> {
            Percentage::new(value as f32)
        }

        /// Convert MSFS normalized value (0-1) to Percentage
        pub fn convert_normalized_to_percentage(value: f64) -> Result<Percentage, BusTypeError> {
            Percentage::from_normalized(value as f32)
        }

        /// Convert MSFS RPM to percentage of redline
        pub fn convert_rpm_to_percentage(rpm: f64, redline_rpm: f64) -> Result<Percentage, BusTypeError> {
            if redline_rpm <= 0.0 {
                return Err(BusTypeError::InvalidValue {
                    field: "redline_rpm".to_string(),
                    reason: "Redline RPM must be positive".to_string(),
                });
            }
            let percentage = (rpm / redline_rpm * 100.0).clamp(0.0, 100.0);
            Percentage::new(percentage as f32)
        }

        /// Convert MSFS fuel quantity (gallons) to percentage of capacity
        pub fn convert_fuel_to_percentage(
            current_gallons: f64,
            capacity_gallons: f64,
        ) -> Result<Percentage, BusTypeError> {
            if capacity_gallons <= 0.0 {
                return Err(BusTypeError::InvalidValue {
                    field: "fuel_capacity".to_string(),
                    reason: "Fuel capacity must be positive".to_string(),
                });
            }
            let percentage = (current_gallons / capacity_gallons * 100.0).clamp(0.0, 100.0);
            Percentage::new(percentage as f32)
        }
    }
}

/// X-Plane adapter helper functions
pub mod xplane {
    use super::*;

    /// Convert X-Plane DataRef units to normalized values
    pub struct XPlaneConverter;

    impl XPlaneConverter {
        /// Convert X-Plane airspeed (m/s) to ValidatedSpeed
        pub fn convert_airspeed_mps(value: f32) -> Result<ValidatedSpeed, BusTypeError> {
            ValidatedSpeed::new_mps(value)
        }

        /// Convert X-Plane airspeed (knots) to ValidatedSpeed
        pub fn convert_airspeed_knots(value: f32) -> Result<ValidatedSpeed, BusTypeError> {
            ValidatedSpeed::new_knots(value)
        }

        /// Convert X-Plane angle (degrees) to ValidatedAngle
        pub fn convert_angle_degrees(value: f32) -> Result<ValidatedAngle, BusTypeError> {
            // X-Plane uses different conventions for some angles
            let normalized = ((value % 360.0) + 540.0) % 360.0 - 180.0;
            ValidatedAngle::new_degrees(normalized)
        }

        /// Convert X-Plane ratio (0-1) to Percentage
        pub fn convert_ratio_to_percentage(value: f32) -> Result<Percentage, BusTypeError> {
            Percentage::from_normalized(value)
        }

        /// Convert X-Plane G-force (G units) to GForce
        pub fn convert_g_force(value: f32) -> Result<GForce, BusTypeError> {
            GForce::new(value)
        }

        /// Convert X-Plane altitude (meters) to feet
        pub fn convert_altitude_m_to_ft(meters: f32) -> f32 {
            meters * 3.28084
        }

        /// Convert X-Plane temperature (Celsius) - pass through
        pub fn convert_temperature_celsius(value: f32) -> f32 {
            value
        }

        /// Convert X-Plane engine N1 percentage to normalized percentage
        pub fn convert_n1_percentage(value: f32) -> Result<Percentage, BusTypeError> {
            Percentage::new(value.clamp(0.0, 100.0))
        }
    }
}

/// DCS adapter helper functions
pub mod dcs {
    use super::*;

    /// Convert DCS Export.lua units to normalized values
    pub struct DcsConverter;

    impl DcsConverter {
        /// Convert DCS airspeed (m/s) to ValidatedSpeed
        pub fn convert_airspeed_mps(value: f64) -> Result<ValidatedSpeed, BusTypeError> {
            ValidatedSpeed::new_mps(value as f32)
        }

        /// Convert DCS angle (radians) to ValidatedAngle
        pub fn convert_angle_radians(value: f64) -> Result<ValidatedAngle, BusTypeError> {
            ValidatedAngle::new_radians(value as f32)
        }

        /// Convert DCS normalized value (0-1) to Percentage
        pub fn convert_normalized_to_percentage(value: f64) -> Result<Percentage, BusTypeError> {
            Percentage::from_normalized(value as f32)
        }

        /// Convert DCS altitude (meters) to feet
        pub fn convert_altitude_m_to_ft(meters: f64) -> f32 {
            (meters * 3.28084) as f32
        }

        /// Convert DCS G-force to GForce
        pub fn convert_g_force(value: f64) -> Result<GForce, BusTypeError> {
            GForce::new(value as f32)
        }

        /// Convert DCS Mach number to Mach
        pub fn convert_mach(value: f64) -> Result<Mach, BusTypeError> {
            Mach::new(value as f32)
        }

        /// Convert DCS engine RPM percentage to Percentage
        pub fn convert_engine_rpm_percentage(value: f64) -> Result<Percentage, BusTypeError> {
            Percentage::new((value * 100.0) as f32)
        }

        /// Convert DCS fuel quantity (0-1) to Percentage
        pub fn convert_fuel_percentage(value: f64) -> Result<Percentage, BusTypeError> {
            Percentage::from_normalized(value as f32)
        }

        /// Convert DCS helicopter rotor RPM (0-1) to Percentage
        pub fn convert_rotor_rpm_percentage(value: f64) -> Result<Percentage, BusTypeError> {
            Percentage::from_normalized(value as f32)
        }
    }
}

/// Generic validation helpers
pub mod validation {
    use super::*;

    /// Validate that a value is within expected range for the field type
    pub fn validate_speed_range(speed: f32, field_name: &str) -> Result<(), BusTypeError> {
        if !(0.0..=1000.0).contains(&speed) {
            Err(BusTypeError::OutOfRange {
                field: field_name.to_string(),
                value: speed,
                min: 0.0,
                max: 1000.0,
            })
        } else {
            Ok(())
        }
    }

    /// Validate that an angle is within valid range
    pub fn validate_angle_range(angle: f32, field_name: &str) -> Result<(), BusTypeError> {
        if !(-180.0..=180.0).contains(&angle) {
            Err(BusTypeError::OutOfRange {
                field: field_name.to_string(),
                value: angle,
                min: -180.0,
                max: 180.0,
            })
        } else {
            Ok(())
        }
    }

    /// Validate that altitude is reasonable
    pub fn validate_altitude_range(altitude: f32, field_name: &str) -> Result<(), BusTypeError> {
        if !(-1000.0..=100000.0).contains(&altitude) {
            Err(BusTypeError::OutOfRange {
                field: field_name.to_string(),
                value: altitude,
                min: -1000.0,
                max: 100000.0,
            })
        } else {
            Ok(())
        }
    }

    /// Validate engine data consistency
    pub fn validate_engine_data(engine: &EngineData) -> Result<(), BusTypeError> {
        if engine.running {
            // Running engine should have reasonable values
            if engine.rpm.value() < 10.0 {
                return Err(BusTypeError::InvalidValue {
                    field: "engine.rpm".to_string(),
                    reason: "Running engine should have RPM > 10%".to_string(),
                });
            }
        } else {
            // Stopped engine should have low/zero values
            if engine.rpm.value() > 5.0 {
                return Err(BusTypeError::InvalidValue {
                    field: "engine.rpm".to_string(),
                    reason: "Stopped engine should have RPM < 5%".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Validate snapshot cross-field consistency
    pub fn validate_snapshot_consistency(snapshot: &BusSnapshot) -> Result<(), BusTypeError> {
        // Validate that ground speed is reasonable compared to IAS
        let ias = snapshot.kinematics.ias.to_knots();
        let gs = snapshot.kinematics.ground_speed.to_knots();
        
        // Ground speed should be within reasonable range of IAS (accounting for wind)
        if (ias - gs).abs() > 100.0 {
            return Err(BusTypeError::InvalidValue {
                field: "ground_speed_vs_ias".to_string(),
                reason: format!("Ground speed {} too different from IAS {}", gs, ias),
            });
        }

        // Validate engine data
        for engine in &snapshot.engines {
            validate_engine_data(engine)?;
        }

        // Validate helicopter data if present
        if let Some(helo) = &snapshot.helo
            && (helo.pedals < -100.0 || helo.pedals > 100.0) {
            return Err(BusTypeError::OutOfRange {
                field: "helo.pedals".to_string(),
                value: helo.pedals,
                min: -100.0,
                max: 100.0,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod msfs_tests {
        use super::*;
        use crate::adapters::msfs::MsfsConverter;

        #[test]
        fn test_msfs_speed_conversion() {
            let speed = MsfsConverter::convert_ias(150.0).unwrap();
            assert_eq!(speed.value(), 150.0);
            assert_eq!(speed.to_knots(), 150.0);

            // Test out of range
            assert!(MsfsConverter::convert_ias(-10.0).is_err());
            assert!(MsfsConverter::convert_ias(1500.0).is_err());
        }

        #[test]
        fn test_msfs_angle_conversion() {
            // Test normal angle
            let angle = MsfsConverter::convert_angle_degrees(45.0).unwrap();
            assert_eq!(angle.to_degrees(), 45.0);

            // Test angle normalization
            let angle = MsfsConverter::convert_angle_degrees(270.0).unwrap();
            assert_eq!(angle.to_degrees(), -90.0);

            let angle = MsfsConverter::convert_angle_degrees(-270.0).unwrap();
            assert_eq!(angle.to_degrees(), 90.0);
        }

        #[test]
        fn test_msfs_percentage_conversion() {
            let pct = MsfsConverter::convert_percentage(75.0).unwrap();
            assert_eq!(pct.value(), 75.0);

            let pct = MsfsConverter::convert_normalized_to_percentage(0.75).unwrap();
            assert_eq!(pct.value(), 75.0);

            // Test out of range
            assert!(MsfsConverter::convert_percentage(-5.0).is_err());
            assert!(MsfsConverter::convert_percentage(105.0).is_err());
        }

        #[test]
        fn test_msfs_rpm_conversion() {
            let rpm_pct = MsfsConverter::convert_rpm_to_percentage(2400.0, 2700.0).unwrap();
            assert!((rpm_pct.value() - 88.89).abs() < 0.1);

            // Test invalid redline
            assert!(MsfsConverter::convert_rpm_to_percentage(2400.0, 0.0).is_err());
        }

        #[test]
        fn test_msfs_fuel_conversion() {
            let fuel_pct = MsfsConverter::convert_fuel_to_percentage(30.0, 40.0).unwrap();
            assert_eq!(fuel_pct.value(), 75.0);

            // Test invalid capacity
            assert!(MsfsConverter::convert_fuel_to_percentage(30.0, 0.0).is_err());
        }
    }

    mod xplane_tests {
        use super::*;
        use crate::adapters::xplane::XPlaneConverter;

        #[test]
        fn test_xplane_speed_conversion() {
            let speed = XPlaneConverter::convert_airspeed_mps(77.17).unwrap(); // ~150 knots
            assert!((speed.value() - 77.17).abs() < 0.01);

            let speed_kts = XPlaneConverter::convert_airspeed_knots(150.0).unwrap();
            assert_eq!(speed_kts.value(), 150.0);
        }

        #[test]
        fn test_xplane_ratio_conversion() {
            let pct = XPlaneConverter::convert_ratio_to_percentage(0.75).unwrap();
            assert_eq!(pct.value(), 75.0);
        }

        #[test]
        fn test_xplane_altitude_conversion() {
            let alt_ft = XPlaneConverter::convert_altitude_m_to_ft(1000.0);
            assert!((alt_ft - 3280.84).abs() < 0.1);
        }

        #[test]
        fn test_xplane_n1_conversion() {
            let n1 = XPlaneConverter::convert_n1_percentage(85.5).unwrap();
            assert_eq!(n1.value(), 85.5);

            // Test clamping
            let n1_clamped = XPlaneConverter::convert_n1_percentage(105.0).unwrap();
            assert_eq!(n1_clamped.value(), 100.0);
        }
    }

    mod dcs_tests {
        use super::*;
        use crate::adapters::dcs::DcsConverter;

        #[test]
        fn test_dcs_speed_conversion() {
            let speed = DcsConverter::convert_airspeed_mps(77.17).unwrap();
            assert!((speed.value() - 77.17).abs() < 0.01);
        }

        #[test]
        fn test_dcs_angle_conversion() {
            let angle = DcsConverter::convert_angle_radians(std::f64::consts::PI / 4.0).unwrap();
            assert!((angle.to_degrees() - 45.0).abs() < 0.01);
        }

        #[test]
        fn test_dcs_normalized_conversion() {
            let pct = DcsConverter::convert_normalized_to_percentage(0.65).unwrap();
            assert_eq!(pct.value(), 65.0);
        }

        #[test]
        fn test_dcs_altitude_conversion() {
            let alt_ft = DcsConverter::convert_altitude_m_to_ft(1500.0);
            assert!((alt_ft - 4921.26).abs() < 0.1);
        }

        #[test]
        fn test_dcs_engine_rpm_conversion() {
            let rpm_pct = DcsConverter::convert_engine_rpm_percentage(0.85).unwrap();
            assert_eq!(rpm_pct.value(), 85.0);
        }

        #[test]
        fn test_dcs_rotor_rpm_conversion() {
            let rotor_pct = DcsConverter::convert_rotor_rpm_percentage(1.0).unwrap();
            assert_eq!(rotor_pct.value(), 100.0);
        }
    }

    mod validation_tests {
        use super::*;
        use crate::adapters::validation::*;

        #[test]
        fn test_speed_validation() {
            assert!(validate_speed_range(150.0, "test_speed").is_ok());
            assert!(validate_speed_range(-10.0, "test_speed").is_err());
            assert!(validate_speed_range(1500.0, "test_speed").is_err());
        }

        #[test]
        fn test_angle_validation() {
            assert!(validate_angle_range(45.0, "test_angle").is_ok());
            assert!(validate_angle_range(-179.0, "test_angle").is_ok());
            assert!(validate_angle_range(180.0, "test_angle").is_ok());
            assert!(validate_angle_range(-181.0, "test_angle").is_err());
            assert!(validate_angle_range(181.0, "test_angle").is_err());
        }

        #[test]
        fn test_altitude_validation() {
            assert!(validate_altitude_range(5000.0, "test_altitude").is_ok());
            assert!(validate_altitude_range(-500.0, "test_altitude").is_ok());
            assert!(validate_altitude_range(-1500.0, "test_altitude").is_err());
            assert!(validate_altitude_range(150000.0, "test_altitude").is_err());
        }

        #[test]
        fn test_engine_validation() {
            let running_engine = EngineData {
                index: 0,
                running: true,
                rpm: Percentage::new(75.0).unwrap(),
                manifold_pressure: None,
                egt: None,
                cht: None,
                fuel_flow: None,
                oil_pressure: None,
                oil_temperature: None,
            };
            assert!(validate_engine_data(&running_engine).is_ok());

            let stopped_engine = EngineData {
                index: 0,
                running: false,
                rpm: Percentage::new(0.0).unwrap(),
                manifold_pressure: None,
                egt: None,
                cht: None,
                fuel_flow: None,
                oil_pressure: None,
                oil_temperature: None,
            };
            assert!(validate_engine_data(&stopped_engine).is_ok());

            // Invalid: running engine with low RPM
            let invalid_engine = EngineData {
                index: 0,
                running: true,
                rpm: Percentage::new(5.0).unwrap(),
                manifold_pressure: None,
                egt: None,
                cht: None,
                fuel_flow: None,
                oil_pressure: None,
                oil_temperature: None,
            };
            assert!(validate_engine_data(&invalid_engine).is_err());
        }
    }
}