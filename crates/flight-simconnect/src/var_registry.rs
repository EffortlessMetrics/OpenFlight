// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SimConnect simulation variable registry
//!
//! Provides a typed catalog of MSFS SimConnect simulation variables (SimVars)
//! with metadata for category, writability, and unit strings. Used by the
//! adapter layer to validate requested subscriptions and build data definitions.

use std::collections::HashMap;

/// Category of a SimConnect simulation variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SimVarCategory {
    FlightControls,
    Engine,
    Navigation,
    Electrical,
    Fuel,
    Landing,
    Environment,
    Instruments,
    Autopilot,
    Communication,
}

/// Metadata for a single MSFS SimConnect simulation variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimVar {
    /// SimConnect variable name (e.g. `"AILERON POSITION"`).
    pub name: &'static str,
    /// Unit string for SimConnect registration (e.g. `"position"`).
    pub unit: &'static str,
    /// Functional category.
    pub category: SimVarCategory,
    /// Whether this variable can be written back to the sim.
    pub writable: bool,
    /// Human-readable description.
    pub description: &'static str,
}

/// Registry of known MSFS SimConnect simulation variables.
pub struct SimVarRegistry {
    vars: HashMap<&'static str, SimVar>,
}

impl SimVarRegistry {
    /// Create a new registry pre-populated with standard MSFS SimVars.
    #[allow(clippy::too_many_lines)]
    pub fn new() -> Self {
        let entries: &[SimVar] = &[
            // ── Flight Controls ──────────────────────────────────────
            SimVar {
                name: "AILERON POSITION",
                unit: "position",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Aileron deflection, -1.0 (left) to 1.0 (right)",
            },
            SimVar {
                name: "ELEVATOR POSITION",
                unit: "position",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Elevator deflection, -1.0 (down) to 1.0 (up)",
            },
            SimVar {
                name: "RUDDER POSITION",
                unit: "position",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Rudder deflection, -1.0 (left) to 1.0 (right)",
            },
            SimVar {
                name: "FLAPS HANDLE INDEX",
                unit: "number",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Current flap handle detent index",
            },
            SimVar {
                name: "FLAPS HANDLE PERCENT",
                unit: "percent over 100",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Flap handle position as 0..1 ratio",
            },
            SimVar {
                name: "TRAILING EDGE FLAPS LEFT ANGLE",
                unit: "degrees",
                category: SimVarCategory::FlightControls,
                writable: false,
                description: "Left trailing-edge flap deflection angle",
            },
            SimVar {
                name: "TRAILING EDGE FLAPS RIGHT ANGLE",
                unit: "degrees",
                category: SimVarCategory::FlightControls,
                writable: false,
                description: "Right trailing-edge flap deflection angle",
            },
            SimVar {
                name: "SPOILERS HANDLE POSITION",
                unit: "percent over 100",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Spoiler/speedbrake handle position 0..1",
            },
            SimVar {
                name: "ELEVATOR TRIM POSITION",
                unit: "degrees",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Elevator trim tab deflection",
            },
            SimVar {
                name: "RUDDER TRIM PCT",
                unit: "percent over 100",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Rudder trim percentage",
            },
            SimVar {
                name: "AILERON TRIM PCT",
                unit: "percent over 100",
                category: SimVarCategory::FlightControls,
                writable: true,
                description: "Aileron trim percentage",
            },
            // ── Engine ───────────────────────────────────────────────
            SimVar {
                name: "GENERAL ENG RPM:1",
                unit: "rpm",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 RPM",
            },
            SimVar {
                name: "GENERAL ENG RPM:2",
                unit: "rpm",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 2 RPM",
            },
            SimVar {
                name: "GENERAL ENG THROTTLE LEVER POSITION:1",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: true,
                description: "Engine 1 throttle lever 0..100%",
            },
            SimVar {
                name: "GENERAL ENG THROTTLE LEVER POSITION:2",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: true,
                description: "Engine 2 throttle lever 0..100%",
            },
            SimVar {
                name: "GENERAL ENG MIXTURE LEVER POSITION:1",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: true,
                description: "Engine 1 mixture lever 0..100%",
            },
            SimVar {
                name: "GENERAL ENG MIXTURE LEVER POSITION:2",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: true,
                description: "Engine 2 mixture lever 0..100%",
            },
            SimVar {
                name: "PROP RPM:1",
                unit: "rpm",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Propeller 1 RPM",
            },
            SimVar {
                name: "PROP RPM:2",
                unit: "rpm",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Propeller 2 RPM",
            },
            SimVar {
                name: "ENG N1 RPM:1",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 N1 percentage",
            },
            SimVar {
                name: "ENG N1 RPM:2",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 2 N1 percentage",
            },
            SimVar {
                name: "ENG N2 RPM:1",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 N2 percentage",
            },
            SimVar {
                name: "ENG N2 RPM:2",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 2 N2 percentage",
            },
            // ── Navigation ───────────────────────────────────────────
            SimVar {
                name: "PLANE ALTITUDE",
                unit: "feet",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "True altitude above MSL",
            },
            SimVar {
                name: "INDICATED ALTITUDE",
                unit: "feet",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Altitude indicated by altimeter",
            },
            SimVar {
                name: "AIRSPEED INDICATED",
                unit: "knots",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Indicated airspeed",
            },
            SimVar {
                name: "AIRSPEED TRUE",
                unit: "knots",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "True airspeed",
            },
            SimVar {
                name: "VERTICAL SPEED",
                unit: "feet per minute",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Rate of climb / descent",
            },
            SimVar {
                name: "HEADING INDICATOR",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Heading indicator reading (gyro-based)",
            },
            SimVar {
                name: "PLANE HEADING DEGREES MAGNETIC",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Magnetic heading",
            },
            SimVar {
                name: "PLANE HEADING DEGREES TRUE",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "True heading",
            },
            SimVar {
                name: "PLANE LATITUDE",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Current latitude",
            },
            SimVar {
                name: "PLANE LONGITUDE",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Current longitude",
            },
            SimVar {
                name: "GPS GROUND SPEED",
                unit: "knots",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Ground speed from GPS",
            },
            SimVar {
                name: "PLANE PITCH DEGREES",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Pitch angle",
            },
            SimVar {
                name: "PLANE BANK DEGREES",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Bank (roll) angle",
            },
            // ── Electrical ───────────────────────────────────────────
            SimVar {
                name: "ELECTRICAL MASTER BATTERY",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: true,
                description: "Master battery switch on/off",
            },
            SimVar {
                name: "GENERAL ENG GENERATOR SWITCH:1",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: true,
                description: "Engine 1 generator / alternator switch",
            },
            SimVar {
                name: "ELECTRICAL AVIONICS BUS VOLTAGE",
                unit: "volts",
                category: SimVarCategory::Electrical,
                writable: false,
                description: "Avionics bus voltage",
            },
            // ── Fuel ─────────────────────────────────────────────────
            SimVar {
                name: "FUEL TOTAL QUANTITY",
                unit: "gallons",
                category: SimVarCategory::Fuel,
                writable: false,
                description: "Total fuel quantity across all tanks",
            },
            SimVar {
                name: "FUEL TOTAL QUANTITY WEIGHT",
                unit: "pounds",
                category: SimVarCategory::Fuel,
                writable: false,
                description: "Total fuel weight",
            },
            SimVar {
                name: "FUEL LEFT QUANTITY",
                unit: "gallons",
                category: SimVarCategory::Fuel,
                writable: false,
                description: "Left tank fuel quantity",
            },
            SimVar {
                name: "FUEL RIGHT QUANTITY",
                unit: "gallons",
                category: SimVarCategory::Fuel,
                writable: false,
                description: "Right tank fuel quantity",
            },
            // ── Landing ──────────────────────────────────────────────
            SimVar {
                name: "GEAR HANDLE POSITION",
                unit: "bool",
                category: SimVarCategory::Landing,
                writable: true,
                description: "Landing gear handle up/down",
            },
            SimVar {
                name: "GEAR POSITION:0",
                unit: "enum",
                category: SimVarCategory::Landing,
                writable: false,
                description: "Nose gear position (0 = retracted, 100 = extended)",
            },
            SimVar {
                name: "GEAR POSITION:1",
                unit: "enum",
                category: SimVarCategory::Landing,
                writable: false,
                description: "Left main gear position",
            },
            SimVar {
                name: "GEAR POSITION:2",
                unit: "enum",
                category: SimVarCategory::Landing,
                writable: false,
                description: "Right main gear position",
            },
            SimVar {
                name: "SIM ON GROUND",
                unit: "bool",
                category: SimVarCategory::Landing,
                writable: false,
                description: "True when aircraft is on the ground",
            },
            // ── Environment ──────────────────────────────────────────
            SimVar {
                name: "AMBIENT TEMPERATURE",
                unit: "celsius",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Outside air temperature",
            },
            SimVar {
                name: "AMBIENT WIND VELOCITY",
                unit: "knots",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Ambient wind speed",
            },
            SimVar {
                name: "AMBIENT WIND DIRECTION",
                unit: "degrees",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Ambient wind direction (from)",
            },
            SimVar {
                name: "BAROMETER PRESSURE",
                unit: "millibars",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Barometric pressure at aircraft",
            },
            // ── Instruments ──────────────────────────────────────────
            SimVar {
                name: "ATTITUDE INDICATOR PITCH DEGREES",
                unit: "degrees",
                category: SimVarCategory::Instruments,
                writable: false,
                description: "Attitude indicator pitch reading",
            },
            SimVar {
                name: "ATTITUDE INDICATOR BANK DEGREES",
                unit: "degrees",
                category: SimVarCategory::Instruments,
                writable: false,
                description: "Attitude indicator bank reading",
            },
            SimVar {
                name: "WISKEY COMPASS INDICATION DEGREES",
                unit: "degrees",
                category: SimVarCategory::Instruments,
                writable: false,
                description: "Whiskey compass heading",
            },
            SimVar {
                name: "TURN INDICATOR RATE",
                unit: "degrees per second",
                category: SimVarCategory::Instruments,
                writable: false,
                description: "Turn-and-slip rate",
            },
            // ── Autopilot ────────────────────────────────────────────
            SimVar {
                name: "AUTOPILOT MASTER",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: true,
                description: "Autopilot master on/off",
            },
            SimVar {
                name: "AUTOPILOT HEADING LOCK DIR",
                unit: "degrees",
                category: SimVarCategory::Autopilot,
                writable: true,
                description: "Autopilot heading bug setting",
            },
            SimVar {
                name: "AUTOPILOT ALTITUDE LOCK VAR",
                unit: "feet",
                category: SimVarCategory::Autopilot,
                writable: true,
                description: "Autopilot target altitude",
            },
            SimVar {
                name: "AUTOPILOT VERTICAL HOLD VAR",
                unit: "feet per minute",
                category: SimVarCategory::Autopilot,
                writable: true,
                description: "Autopilot target vertical speed",
            },
            SimVar {
                name: "AUTOPILOT AIRSPEED HOLD VAR",
                unit: "knots",
                category: SimVarCategory::Autopilot,
                writable: true,
                description: "Autopilot target airspeed",
            },
            // ── Communication ────────────────────────────────────────
            SimVar {
                name: "COM ACTIVE FREQUENCY:1",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "COM1 active frequency",
            },
            SimVar {
                name: "COM STANDBY FREQUENCY:1",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "COM1 standby frequency",
            },
            SimVar {
                name: "NAV ACTIVE FREQUENCY:1",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "NAV1 active frequency",
            },
            SimVar {
                name: "NAV STANDBY FREQUENCY:1",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "NAV1 standby frequency",
            },
            SimVar {
                name: "TRANSPONDER CODE:1",
                unit: "number",
                category: SimVarCategory::Communication,
                writable: true,
                description: "Transponder squawk code",
            },
        ];

        let mut vars = HashMap::with_capacity(entries.len());
        for sv in entries {
            vars.insert(sv.name, sv.clone());
        }

        Self { vars }
    }

    /// Look up a variable by its SimConnect name.
    pub fn get(&self, name: &str) -> Option<&SimVar> {
        self.vars.get(name)
    }

    /// Return all variables belonging to the given category.
    pub fn by_category(&self, cat: SimVarCategory) -> Vec<&SimVar> {
        self.vars.values().filter(|v| v.category == cat).collect()
    }

    /// Return all writable variables.
    pub fn writable_vars(&self) -> Vec<&SimVar> {
        self.vars.values().filter(|v| v.writable).collect()
    }

    /// Return every variable in the registry.
    pub fn all(&self) -> Vec<&SimVar> {
        self.vars.values().collect()
    }

    /// Check whether a variable name exists in the registry.
    pub fn contains(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }

    /// Total number of registered variables.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Returns `true` when the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

impl Default for SimVarRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_populated() {
        let reg = SimVarRegistry::new();
        assert!(
            reg.len() >= 50,
            "registry must contain ≥50 vars, got {}",
            reg.len()
        );
        assert!(!reg.is_empty());
    }

    #[test]
    fn get_known_var() {
        let reg = SimVarRegistry::new();
        let sv = reg
            .get("AILERON POSITION")
            .expect("AILERON POSITION must exist");
        assert_eq!(sv.unit, "position");
        assert_eq!(sv.category, SimVarCategory::FlightControls);
        assert!(sv.writable);
    }

    #[test]
    fn get_unknown_var_returns_none() {
        let reg = SimVarRegistry::new();
        assert!(reg.get("NONEXISTENT VARIABLE").is_none());
    }

    #[test]
    fn contains_checks() {
        let reg = SimVarRegistry::new();
        assert!(reg.contains("ELEVATOR POSITION"));
        assert!(reg.contains("PLANE ALTITUDE"));
        assert!(!reg.contains("DOES NOT EXIST"));
    }

    #[test]
    fn by_category_flight_controls() {
        let reg = SimVarRegistry::new();
        let controls = reg.by_category(SimVarCategory::FlightControls);
        assert!(controls.len() >= 5, "need at least 5 flight-control vars");
        for sv in &controls {
            assert_eq!(sv.category, SimVarCategory::FlightControls);
        }
    }

    #[test]
    fn by_category_engine() {
        let reg = SimVarRegistry::new();
        let engine_vars = reg.by_category(SimVarCategory::Engine);
        assert!(engine_vars.len() >= 4, "need at least 4 engine vars");
    }

    #[test]
    fn by_category_navigation() {
        let reg = SimVarRegistry::new();
        let nav = reg.by_category(SimVarCategory::Navigation);
        assert!(nav.len() >= 5);
        assert!(nav.iter().any(|v| v.name == "AIRSPEED INDICATED"));
    }

    #[test]
    fn writable_vars_are_writable() {
        let reg = SimVarRegistry::new();
        let writable = reg.writable_vars();
        assert!(!writable.is_empty());
        for sv in &writable {
            assert!(sv.writable, "{} should be writable", sv.name);
        }
    }

    #[test]
    fn writable_count_sanity() {
        let reg = SimVarRegistry::new();
        let total = reg.len();
        let writable = reg.writable_vars().len();
        let read_only = total - writable;
        assert!(writable > 0, "must have some writable vars");
        assert!(read_only > 0, "must have some read-only vars");
    }

    #[test]
    fn all_returns_full_set() {
        let reg = SimVarRegistry::new();
        assert_eq!(reg.all().len(), reg.len());
    }

    #[test]
    fn every_category_represented() {
        let reg = SimVarRegistry::new();
        let categories = [
            SimVarCategory::FlightControls,
            SimVarCategory::Engine,
            SimVarCategory::Navigation,
            SimVarCategory::Electrical,
            SimVarCategory::Fuel,
            SimVarCategory::Landing,
            SimVarCategory::Environment,
            SimVarCategory::Instruments,
            SimVarCategory::Autopilot,
            SimVarCategory::Communication,
        ];
        for cat in categories {
            assert!(
                !reg.by_category(cat).is_empty(),
                "category {cat:?} must have at least one variable"
            );
        }
    }

    #[test]
    fn descriptions_non_empty() {
        let reg = SimVarRegistry::new();
        for sv in reg.all() {
            assert!(
                !sv.description.is_empty(),
                "description empty for {}",
                sv.name
            );
        }
    }

    #[test]
    fn default_matches_new() {
        let a = SimVarRegistry::new();
        let b = SimVarRegistry::default();
        assert_eq!(a.len(), b.len());
    }
}
