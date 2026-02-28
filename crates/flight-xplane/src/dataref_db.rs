// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane dataref database
//!
//! A registry of known X-Plane datarefs with metadata including type information,
//! writability, array sizes, and human-readable descriptions. Covers position,
//! orientation, flight controls, engine data, navigation, autopilot, and more.

use std::collections::HashMap;

/// Type of data a dataref carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatarefType {
    Int,
    Float,
    Double,
    IntArray,
    FloatArray,
    Data,
}

/// Metadata for a single X-Plane dataref.
#[derive(Debug, Clone, PartialEq)]
pub struct DatarefInfo {
    pub path: &'static str,
    pub data_type: DatarefType,
    pub writable: bool,
    pub description: &'static str,
    pub array_size: Option<u32>,
}

/// In-memory database of known X-Plane datarefs.
#[derive(Debug, Clone)]
pub struct DatarefDatabase {
    refs: HashMap<&'static str, DatarefInfo>,
}

impl DatarefDatabase {
    /// Build a new database pre-populated with 50+ real X-Plane datarefs.
    pub fn new() -> Self {
        let mut refs = HashMap::new();

        // Helper closures
        let mut add = |path: &'static str,
                       dt: DatarefType,
                       writable: bool,
                       desc: &'static str,
                       arr: Option<u32>| {
            refs.insert(
                path,
                DatarefInfo {
                    path,
                    data_type: dt,
                    writable,
                    description: desc,
                    array_size: arr,
                },
            );
        };

        // ── Position ────────────────────────────────────────────────
        add(
            "sim/flightmodel/position/local_x",
            DatarefType::Double,
            false,
            "Aircraft local X position (OpenGL coords, meters)",
            None,
        );
        add(
            "sim/flightmodel/position/local_y",
            DatarefType::Double,
            false,
            "Aircraft local Y position (OpenGL coords, meters)",
            None,
        );
        add(
            "sim/flightmodel/position/local_z",
            DatarefType::Double,
            false,
            "Aircraft local Z position (OpenGL coords, meters)",
            None,
        );
        add(
            "sim/flightmodel/position/latitude",
            DatarefType::Double,
            false,
            "Aircraft latitude in degrees",
            None,
        );
        add(
            "sim/flightmodel/position/longitude",
            DatarefType::Double,
            false,
            "Aircraft longitude in degrees",
            None,
        );
        add(
            "sim/flightmodel/position/elevation",
            DatarefType::Float,
            false,
            "Aircraft elevation MSL in meters",
            None,
        );

        // ── Orientation ─────────────────────────────────────────────
        add(
            "sim/flightmodel/position/phi",
            DatarefType::Float,
            false,
            "Roll angle in degrees",
            None,
        );
        add(
            "sim/flightmodel/position/theta",
            DatarefType::Float,
            false,
            "Pitch angle in degrees",
            None,
        );
        add(
            "sim/flightmodel/position/psi",
            DatarefType::Float,
            false,
            "True heading in degrees",
            None,
        );

        // ── Speeds ──────────────────────────────────────────────────
        add(
            "sim/flightmodel/position/indicated_airspeed",
            DatarefType::Float,
            false,
            "Indicated airspeed in kias",
            None,
        );
        add(
            "sim/flightmodel/position/true_airspeed",
            DatarefType::Float,
            false,
            "True airspeed in m/s",
            None,
        );
        add(
            "sim/flightmodel/position/groundspeed",
            DatarefType::Float,
            false,
            "Ground speed in m/s",
            None,
        );
        add(
            "sim/flightmodel/position/vh_ind",
            DatarefType::Float,
            false,
            "Vertical speed (indicated) in m/s",
            None,
        );
        add(
            "sim/flightmodel/misc/machno",
            DatarefType::Float,
            false,
            "Mach number",
            None,
        );

        // ── Angles of attack / sideslip ─────────────────────────────
        add(
            "sim/flightmodel/position/alpha",
            DatarefType::Float,
            false,
            "Angle of attack in degrees",
            None,
        );
        add(
            "sim/flightmodel/position/beta",
            DatarefType::Float,
            false,
            "Sideslip angle in degrees",
            None,
        );

        // ── Angular rates ───────────────────────────────────────────
        add(
            "sim/flightmodel/position/P",
            DatarefType::Float,
            false,
            "Roll rate in deg/s",
            None,
        );
        add(
            "sim/flightmodel/position/Q",
            DatarefType::Float,
            false,
            "Pitch rate in deg/s",
            None,
        );
        add(
            "sim/flightmodel/position/R",
            DatarefType::Float,
            false,
            "Yaw rate in deg/s",
            None,
        );

        // ── Ground track ────────────────────────────────────────────
        add(
            "sim/flightmodel/position/hpath",
            DatarefType::Float,
            false,
            "Ground track heading in degrees",
            None,
        );

        // ── G-forces ────────────────────────────────────────────────
        add(
            "sim/flightmodel/forces/g_nrml",
            DatarefType::Float,
            false,
            "Normal (vertical) G-force",
            None,
        );
        add(
            "sim/flightmodel/forces/g_side",
            DatarefType::Float,
            false,
            "Lateral G-force",
            None,
        );
        add(
            "sim/flightmodel/forces/g_axil",
            DatarefType::Float,
            false,
            "Longitudinal G-force",
            None,
        );

        // ── Flight controls (cockpit2) ──────────────────────────────
        add(
            "sim/cockpit2/controls/yoke_pitch_ratio",
            DatarefType::Float,
            true,
            "Yoke pitch ratio (-1..1)",
            None,
        );
        add(
            "sim/cockpit2/controls/yoke_roll_ratio",
            DatarefType::Float,
            true,
            "Yoke roll ratio (-1..1)",
            None,
        );
        add(
            "sim/cockpit2/controls/yoke_heading_ratio",
            DatarefType::Float,
            true,
            "Rudder pedal ratio (-1..1)",
            None,
        );
        add(
            "sim/cockpit2/controls/parking_brake_ratio",
            DatarefType::Float,
            true,
            "Parking brake ratio (0..1)",
            None,
        );

        // ── Flight controls (flightmodel) ───────────────────────────
        add(
            "sim/flightmodel/controls/elv_trim",
            DatarefType::Float,
            true,
            "Elevator trim ratio (-1..1)",
            None,
        );
        add(
            "sim/flightmodel/controls/ail_trim",
            DatarefType::Float,
            true,
            "Aileron trim ratio (-1..1)",
            None,
        );
        add(
            "sim/flightmodel/controls/rud_trim",
            DatarefType::Float,
            true,
            "Rudder trim ratio (-1..1)",
            None,
        );

        // ── Throttle / mixture / prop ───────────────────────────────
        add(
            "sim/cockpit2/engine/actuators/throttle_ratio_all",
            DatarefType::Float,
            true,
            "Throttle ratio for all engines (0..1)",
            None,
        );
        add(
            "sim/cockpit2/engine/actuators/mixture_ratio_all",
            DatarefType::Float,
            true,
            "Mixture ratio for all engines (0..1)",
            None,
        );
        add(
            "sim/cockpit2/engine/actuators/prop_ratio_all",
            DatarefType::Float,
            true,
            "Prop RPM ratio for all engines (0..1)",
            None,
        );

        // ── Engine data ─────────────────────────────────────────────
        add(
            "sim/flightmodel/engine/ENGN_N1_",
            DatarefType::FloatArray,
            false,
            "Engine N1 percentage per engine",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_N2_",
            DatarefType::FloatArray,
            false,
            "Engine N2 percentage per engine",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_EGT",
            DatarefType::FloatArray,
            false,
            "Exhaust gas temperature per engine (deg C)",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_FF_",
            DatarefType::FloatArray,
            false,
            "Fuel flow per engine (kg/s)",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_ITT",
            DatarefType::FloatArray,
            false,
            "Interstage turbine temperature per engine (deg C)",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_CHT",
            DatarefType::FloatArray,
            false,
            "Cylinder head temperature per engine (deg C)",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_MPR",
            DatarefType::FloatArray,
            false,
            "Manifold pressure per engine (inHg)",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_oilp",
            DatarefType::FloatArray,
            false,
            "Oil pressure per engine (PSI)",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_oilt",
            DatarefType::FloatArray,
            false,
            "Oil temperature per engine (deg C)",
            Some(8),
        );
        add(
            "sim/flightmodel/engine/ENGN_running",
            DatarefType::IntArray,
            false,
            "Engine running state per engine (0/1)",
            Some(8),
        );

        // ── Fuel ────────────────────────────────────────────────────
        add(
            "sim/flightmodel/weight/m_fuel_total",
            DatarefType::Float,
            false,
            "Total fuel weight in kg",
            None,
        );
        add(
            "sim/flightmodel/weight/m_fuel",
            DatarefType::FloatArray,
            false,
            "Fuel weight per tank in kg",
            Some(9),
        );

        // ── Gear / flaps / speedbrake ───────────────────────────────
        add(
            "sim/aircraft/parts/acf_gear_deploy",
            DatarefType::FloatArray,
            false,
            "Gear deployment ratio per gear leg",
            Some(10),
        );
        add(
            "sim/cockpit2/controls/flap_ratio",
            DatarefType::Float,
            true,
            "Flap handle ratio (0..1)",
            None,
        );
        add(
            "sim/cockpit2/controls/speedbrake_ratio",
            DatarefType::Float,
            true,
            "Speedbrake handle ratio (-0.5..1.0)",
            None,
        );

        // ── Autopilot ───────────────────────────────────────────────
        add(
            "sim/cockpit/autopilot/autopilot_mode",
            DatarefType::Int,
            false,
            "Autopilot master mode (0=off,1=FD,2=on)",
            None,
        );
        add(
            "sim/cockpit/autopilot/altitude",
            DatarefType::Float,
            true,
            "Autopilot altitude target in feet",
            None,
        );
        add(
            "sim/cockpit/autopilot/heading",
            DatarefType::Float,
            true,
            "Autopilot heading target in degrees",
            None,
        );
        add(
            "sim/cockpit/autopilot/airspeed",
            DatarefType::Float,
            true,
            "Autopilot airspeed target in knots",
            None,
        );
        add(
            "sim/cockpit/autopilot/vertical_velocity",
            DatarefType::Float,
            true,
            "Autopilot vertical speed target in fpm",
            None,
        );
        add(
            "sim/cockpit2/autopilot/autothrottle_on",
            DatarefType::Int,
            true,
            "Autothrottle engaged (0/1)",
            None,
        );

        // ── Navigation ──────────────────────────────────────────────
        add(
            "sim/cockpit/radios/nav1_freq_hz",
            DatarefType::Int,
            true,
            "NAV1 active frequency in 10 kHz units",
            None,
        );
        add(
            "sim/cockpit/radios/nav2_freq_hz",
            DatarefType::Int,
            true,
            "NAV2 active frequency in 10 kHz units",
            None,
        );
        add(
            "sim/cockpit/radios/com1_freq_hz",
            DatarefType::Int,
            true,
            "COM1 active frequency in 10 kHz units",
            None,
        );
        add(
            "sim/cockpit/radios/com2_freq_hz",
            DatarefType::Int,
            true,
            "COM2 active frequency in 10 kHz units",
            None,
        );
        add(
            "sim/cockpit/radios/adf1_freq_hz",
            DatarefType::Int,
            true,
            "ADF1 active frequency in Hz",
            None,
        );
        add(
            "sim/cockpit/radios/transponder_code",
            DatarefType::Int,
            true,
            "Transponder squawk code",
            None,
        );

        // ── Lights ──────────────────────────────────────────────────
        add(
            "sim/cockpit/electrical/nav_lights_on",
            DatarefType::Int,
            true,
            "Navigation lights on (0/1)",
            None,
        );
        add(
            "sim/cockpit/electrical/beacon_lights_on",
            DatarefType::Int,
            true,
            "Beacon light on (0/1)",
            None,
        );
        add(
            "sim/cockpit/electrical/strobe_lights_on",
            DatarefType::Int,
            true,
            "Strobe lights on (0/1)",
            None,
        );
        add(
            "sim/cockpit/electrical/landing_lights_on",
            DatarefType::Int,
            true,
            "Landing lights on (0/1)",
            None,
        );
        add(
            "sim/cockpit/electrical/taxi_light_on",
            DatarefType::Int,
            true,
            "Taxi light on (0/1)",
            None,
        );

        // ── Weather ─────────────────────────────────────────────────
        add(
            "sim/weather/temperature_ambient_c",
            DatarefType::Float,
            false,
            "Outside air temperature in degrees C",
            None,
        );
        add(
            "sim/weather/barometer_sealevel_inhg",
            DatarefType::Float,
            false,
            "Sea-level barometric pressure in inHg",
            None,
        );
        add(
            "sim/weather/wind_speed_kt",
            DatarefType::FloatArray,
            false,
            "Wind speed layers in knots",
            Some(13),
        );
        add(
            "sim/weather/wind_direction_degt",
            DatarefType::FloatArray,
            false,
            "Wind direction layers in degrees true",
            Some(13),
        );

        // ── Simulator state ─────────────────────────────────────────
        add(
            "sim/time/total_running_time_sec",
            DatarefType::Float,
            false,
            "Total running time since X-Plane launch (seconds)",
            None,
        );
        add(
            "sim/time/zulu_time_sec",
            DatarefType::Float,
            false,
            "Zulu time of day in seconds since midnight",
            None,
        );
        add(
            "sim/operation/prefs/replay_mode",
            DatarefType::Int,
            false,
            "Replay mode active (0/1)",
            None,
        );
        add(
            "sim/version/xplane_internal_version",
            DatarefType::Int,
            false,
            "X-Plane internal version number",
            None,
        );

        // ── Gauges / indicators ─────────────────────────────────────
        add(
            "sim/cockpit2/gauges/indicators/altitude_ft_pilot",
            DatarefType::Float,
            false,
            "Pressure altitude on pilot-side altimeter (feet)",
            None,
        );
        add(
            "sim/cockpit2/gauges/indicators/airspeed_kts_pilot",
            DatarefType::Float,
            false,
            "Indicated airspeed on pilot-side ASI (ktas)",
            None,
        );
        add(
            "sim/cockpit2/gauges/indicators/heading_AHARS_deg_mag_pilot",
            DatarefType::Float,
            false,
            "Magnetic heading from AHARS (degrees)",
            None,
        );

        // ── Aircraft identification ─────────────────────────────────
        add(
            "sim/aircraft/view/acf_ICAO",
            DatarefType::Data,
            false,
            "Aircraft ICAO type code",
            Some(40),
        );
        add(
            "sim/aircraft/view/acf_descrip",
            DatarefType::Data,
            false,
            "Aircraft description string",
            Some(260),
        );
        add(
            "sim/aircraft/view/acf_author",
            DatarefType::Data,
            false,
            "Aircraft author string",
            Some(500),
        );

        Self { refs }
    }

    /// Look up a dataref by its full path. Returns `None` if not known.
    pub fn get(&self, path: &str) -> Option<&DatarefInfo> {
        self.refs.get(path)
    }

    /// Return all datarefs whose path starts with `prefix`.
    pub fn by_prefix(&self, prefix: &str) -> Vec<&DatarefInfo> {
        self.refs
            .values()
            .filter(|info| info.path.starts_with(prefix))
            .collect()
    }

    /// Return all datarefs that are writable.
    pub fn writable_refs(&self) -> Vec<&DatarefInfo> {
        self.refs.values().filter(|info| info.writable).collect()
    }

    /// Return every dataref in the database.
    pub fn all(&self) -> Vec<&DatarefInfo> {
        self.refs.values().collect()
    }

    /// Datarefs relevant to flight controls (yoke, rudder, trim, throttle, etc.).
    pub fn flight_controls(&self) -> Vec<&DatarefInfo> {
        let prefixes = [
            "sim/cockpit2/controls/yoke_",
            "sim/cockpit2/controls/parking_brake",
            "sim/cockpit2/controls/flap_",
            "sim/cockpit2/controls/speedbrake_",
            "sim/flightmodel/controls/",
            "sim/cockpit2/engine/actuators/",
        ];
        self.refs
            .values()
            .filter(|info| prefixes.iter().any(|p| info.path.starts_with(p)))
            .collect()
    }

    /// Datarefs relevant to engine instrumentation.
    pub fn engine_data(&self) -> Vec<&DatarefInfo> {
        self.refs
            .values()
            .filter(|info| info.path.starts_with("sim/flightmodel/engine/"))
            .collect()
    }

    /// Datarefs relevant to navigation radios and transponder.
    pub fn navigation(&self) -> Vec<&DatarefInfo> {
        self.refs
            .values()
            .filter(|info| info.path.starts_with("sim/cockpit/radios/"))
            .collect()
    }
}

impl Default for DatarefDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_has_at_least_50_datarefs() {
        let db = DatarefDatabase::new();
        assert!(
            db.all().len() >= 50,
            "expected >=50 datarefs, got {}",
            db.all().len()
        );
    }

    #[test]
    fn test_get_existing_dataref() {
        let db = DatarefDatabase::new();
        let info = db.get("sim/flightmodel/position/local_x").unwrap();
        assert_eq!(info.data_type, DatarefType::Double);
        assert!(!info.writable);
    }

    #[test]
    fn test_get_missing_returns_none() {
        let db = DatarefDatabase::new();
        assert!(db.get("sim/nonexistent/dataref").is_none());
    }

    #[test]
    fn test_by_prefix_position() {
        let db = DatarefDatabase::new();
        let position = db.by_prefix("sim/flightmodel/position/");
        assert!(position.len() >= 10, "expected >=10 position datarefs");
    }

    #[test]
    fn test_writable_refs_non_empty() {
        let db = DatarefDatabase::new();
        let writable = db.writable_refs();
        assert!(!writable.is_empty());
        for info in &writable {
            assert!(info.writable);
        }
    }

    #[test]
    fn test_flight_controls_includes_yoke() {
        let db = DatarefDatabase::new();
        let controls = db.flight_controls();
        let paths: Vec<&str> = controls.iter().map(|i| i.path).collect();
        assert!(paths.contains(&"sim/cockpit2/controls/yoke_pitch_ratio"));
        assert!(paths.contains(&"sim/cockpit2/controls/yoke_roll_ratio"));
        assert!(paths.contains(&"sim/cockpit2/controls/yoke_heading_ratio"));
    }

    #[test]
    fn test_engine_data_entries() {
        let db = DatarefDatabase::new();
        let engines = db.engine_data();
        assert!(engines.len() >= 8, "expected >=8 engine datarefs");
        let has_n1 = engines.iter().any(|i| i.path.contains("ENGN_N1_"));
        assert!(has_n1);
    }

    #[test]
    fn test_navigation_includes_radios() {
        let db = DatarefDatabase::new();
        let nav = db.navigation();
        let paths: Vec<&str> = nav.iter().map(|i| i.path).collect();
        assert!(paths.contains(&"sim/cockpit/radios/nav1_freq_hz"));
        assert!(paths.contains(&"sim/cockpit/radios/transponder_code"));
    }

    #[test]
    fn test_array_datarefs_have_sizes() {
        let db = DatarefDatabase::new();
        for info in db.all() {
            match info.data_type {
                DatarefType::FloatArray | DatarefType::IntArray => {
                    assert!(
                        info.array_size.is_some(),
                        "array dataref {} should have array_size",
                        info.path,
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_non_array_datarefs_have_no_size() {
        let db = DatarefDatabase::new();
        for info in db.all() {
            match info.data_type {
                DatarefType::Int | DatarefType::Float | DatarefType::Double => {
                    assert!(
                        info.array_size.is_none(),
                        "scalar dataref {} should not have array_size",
                        info.path,
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_default_matches_new() {
        let a = DatarefDatabase::new();
        let b = DatarefDatabase::default();
        assert_eq!(a.all().len(), b.all().len());
    }

    #[test]
    fn test_by_prefix_empty_result() {
        let db = DatarefDatabase::new();
        let empty = db.by_prefix("totally/bogus/prefix/");
        assert!(empty.is_empty());
    }
}
