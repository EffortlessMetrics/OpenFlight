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
            SimVar {
                name: "COM ACTIVE FREQUENCY:2",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "COM2 active frequency",
            },
            SimVar {
                name: "COM STANDBY FREQUENCY:2",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "COM2 standby frequency",
            },
            SimVar {
                name: "NAV ACTIVE FREQUENCY:2",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "NAV2 active frequency",
            },
            SimVar {
                name: "NAV STANDBY FREQUENCY:2",
                unit: "mhz",
                category: SimVarCategory::Communication,
                writable: true,
                description: "NAV2 standby frequency",
            },
            // ── Additional Navigation ────────────────────────────────
            SimVar {
                name: "GROUND ALTITUDE",
                unit: "feet",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Ground elevation below aircraft",
            },
            SimVar {
                name: "PLANE ALT ABOVE GROUND",
                unit: "feet",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Radar altitude (AGL)",
            },
            SimVar {
                name: "AIRSPEED MACH",
                unit: "mach",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Current Mach number",
            },
            SimVar {
                name: "ANGLE OF ATTACK INDICATOR",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Angle of attack",
            },
            SimVar {
                name: "INCIDENCE BETA",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Sideslip angle",
            },
            SimVar {
                name: "G FORCE",
                unit: "gforce",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Current G-force",
            },
            SimVar {
                name: "ACCELERATION BODY X",
                unit: "feet per second squared",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Lateral acceleration (body frame)",
            },
            SimVar {
                name: "ACCELERATION BODY Z",
                unit: "feet per second squared",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Longitudinal acceleration (body frame)",
            },
            SimVar {
                name: "MAGNETIC COMPASS",
                unit: "degrees",
                category: SimVarCategory::Navigation,
                writable: false,
                description: "Magnetic compass heading",
            },
            // ── Additional Engine ────────────────────────────────────
            SimVar {
                name: "ENG FUEL FLOW GPH:1",
                unit: "gallons per hour",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 fuel flow",
            },
            SimVar {
                name: "ENG FUEL FLOW GPH:2",
                unit: "gallons per hour",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 2 fuel flow",
            },
            SimVar {
                name: "ENG OIL TEMPERATURE:1",
                unit: "rankine",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 oil temperature",
            },
            SimVar {
                name: "ENG OIL PRESSURE:1",
                unit: "psf",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 oil pressure",
            },
            SimVar {
                name: "ENG EXHAUST GAS TEMPERATURE:1",
                unit: "rankine",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 EGT",
            },
            SimVar {
                name: "ENG TORQUE PERCENT:1",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 torque percentage",
            },
            SimVar {
                name: "ENG ITT:1",
                unit: "rankine",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 inter-turbine temperature",
            },
            SimVar {
                name: "TURB ENG CORRECTED N1:1",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 1 corrected N1",
            },
            SimVar {
                name: "TURB ENG CORRECTED N1:2",
                unit: "percent",
                category: SimVarCategory::Engine,
                writable: false,
                description: "Engine 2 corrected N1",
            },
            // ── Additional Autopilot ─────────────────────────────────
            SimVar {
                name: "AUTOPILOT HEADING LOCK",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: false,
                description: "Heading hold mode active",
            },
            SimVar {
                name: "AUTOPILOT ALTITUDE LOCK",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: false,
                description: "Altitude hold mode active",
            },
            SimVar {
                name: "AUTOPILOT NAV1 LOCK",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: false,
                description: "NAV1 lock (LNAV) active",
            },
            SimVar {
                name: "AUTOPILOT APPROACH HOLD",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: false,
                description: "Approach mode active",
            },
            SimVar {
                name: "AUTOPILOT FLIGHT LEVEL CHANGE",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: false,
                description: "Flight level change mode active",
            },
            SimVar {
                name: "AUTOPILOT MACH HOLD VAR",
                unit: "number",
                category: SimVarCategory::Autopilot,
                writable: true,
                description: "Autopilot target Mach number",
            },
            SimVar {
                name: "AUTOPILOT FLIGHT DIRECTOR ACTIVE",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: false,
                description: "Flight director active",
            },
            SimVar {
                name: "AUTOPILOT YAW DAMPER",
                unit: "bool",
                category: SimVarCategory::Autopilot,
                writable: false,
                description: "Yaw damper active",
            },
            // ── Additional Electrical / Lighting ─────────────────────
            SimVar {
                name: "LIGHT NAV",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: false,
                description: "Navigation lights on/off",
            },
            SimVar {
                name: "LIGHT BEACON",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: false,
                description: "Beacon light on/off",
            },
            SimVar {
                name: "LIGHT STROBE",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: false,
                description: "Strobe lights on/off",
            },
            SimVar {
                name: "LIGHT LANDING",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: false,
                description: "Landing lights on/off",
            },
            SimVar {
                name: "LIGHT TAXI",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: false,
                description: "Taxi lights on/off",
            },
            SimVar {
                name: "PITOT HEAT",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: true,
                description: "Pitot heat on/off",
            },
            SimVar {
                name: "GENERAL ENG ANTI ICE POSITION:1",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: true,
                description: "Engine 1 anti-ice on/off",
            },
            SimVar {
                name: "STRUCTURAL DEICE SWITCH",
                unit: "bool",
                category: SimVarCategory::Electrical,
                writable: true,
                description: "Structural deice on/off",
            },
            // ── Additional Fuel ──────────────────────────────────────
            SimVar {
                name: "FUEL TANK CENTER QUANTITY",
                unit: "gallons",
                category: SimVarCategory::Fuel,
                writable: false,
                description: "Center tank fuel quantity",
            },
            SimVar {
                name: "ENG FUEL VALVE OPEN:1",
                unit: "bool",
                category: SimVarCategory::Fuel,
                writable: true,
                description: "Engine 1 fuel valve open/closed",
            },
            // ── Additional Landing / Ground ──────────────────────────
            SimVar {
                name: "BRAKE LEFT POSITION",
                unit: "position",
                category: SimVarCategory::Landing,
                writable: true,
                description: "Left brake pedal position 0..1",
            },
            SimVar {
                name: "BRAKE RIGHT POSITION",
                unit: "position",
                category: SimVarCategory::Landing,
                writable: true,
                description: "Right brake pedal position 0..1",
            },
            SimVar {
                name: "BRAKE PARKING POSITION",
                unit: "bool",
                category: SimVarCategory::Landing,
                writable: true,
                description: "Parking brake set",
            },
            SimVar {
                name: "SURFACE TYPE",
                unit: "enum",
                category: SimVarCategory::Landing,
                writable: false,
                description: "Surface type under aircraft (0=concrete, etc.)",
            },
            // ── Additional Instruments ───────────────────────────────
            SimVar {
                name: "VARIOMETER RATE",
                unit: "feet per minute",
                category: SimVarCategory::Instruments,
                writable: false,
                description: "Variometer reading",
            },
            SimVar {
                name: "KOHLSMAN SETTING MB",
                unit: "millibars",
                category: SimVarCategory::Instruments,
                writable: true,
                description: "Altimeter barometric setting",
            },
            SimVar {
                name: "HSI CDI NEEDLE",
                unit: "number",
                category: SimVarCategory::Instruments,
                writable: false,
                description: "HSI course deviation indicator",
            },
            SimVar {
                name: "HSI GSI NEEDLE",
                unit: "number",
                category: SimVarCategory::Instruments,
                writable: false,
                description: "HSI glideslope indicator",
            },
            // ── Additional Environment ───────────────────────────────
            SimVar {
                name: "AMBIENT PRESSURE",
                unit: "inches of mercury",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Ambient atmospheric pressure",
            },
            SimVar {
                name: "AMBIENT DENSITY",
                unit: "slugs per cubic feet",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Ambient air density",
            },
            SimVar {
                name: "TOTAL WEIGHT",
                unit: "pounds",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Total aircraft weight",
            },
            SimVar {
                name: "MAX GROSS WEIGHT",
                unit: "pounds",
                category: SimVarCategory::Environment,
                writable: false,
                description: "Maximum gross weight",
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
            reg.len() >= 80,
            "registry must contain ≥80 vars, got {}",
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
        assert!(engine_vars.len() >= 10, "need at least 10 engine vars");
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
