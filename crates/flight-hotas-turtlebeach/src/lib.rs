// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Turtle Beach VelocityOne HOTAS, yoke, and rudder drivers for OpenFlight.
//!
//! # Supported devices (VID 0x10F5)
//!
//! | Model                    | PID      | Tier | Category       |
//! |--------------------------|----------|------|----------------|
//! | VelocityOne Flight       | `0x1050` | 2    | Yoke + Panel   |
//! | VelocityOne Rudder       | `0x1051` | 2    | Pedals         |
//! | VelocityOne Flightstick  | `0x1052` | 2    | Joystick       |
//! | VelocityOne Flight Pro   | `0x0210` | 3    | Premium Yoke   |
//! | VelocityOne Flight Univ. | `0x1073` | 3    | All-in-One     |
//! | VelocityOne Flight Yoke  | `0x3085` | 2    | Dedicated Yoke |
//!
//! # Legacy device identifiers (VID 0x1432)
//!
//! | Model                         | VID    | PID    | Tier | Source              |
//! |-------------------------------|--------|--------|------|---------------------|
//! | VelocityOne Flightdeck (yoke) | 0x1432 | 0xB300 | 1    | usb.ids             |
//! | VelocityOne Stick             | 0x1432 | 0xB301 | 3    | PID estimated       |
//! | VelocityOne Rudder            | 0x1432 | 0xB302 | 3    | PID estimated       |
//!
//! # Modules
//!
//! - [`devices`] — Device database and capability descriptors
//! - [`protocol`] — HID report parsing, LED control, display commands
//! - [`profiles`] — Default axis/button configuration profiles
//! - [`velocityone`] — Legacy Flightdeck/Rudder report parsing (VID 0x1432)

pub mod devices;
pub mod profiles;
pub mod protocol;
pub mod velocityone;

// Legacy re-exports (VID 0x1432 parsers)
pub use velocityone::{
    FLIGHTDECK_MIN_REPORT_BYTES, RUDDER_MIN_REPORT_BYTES, TurtleBeachError,
    VelocityOneFlightdeckReport, VelocityOneModel, VelocityOneRudderReport,
    parse_flightdeck_report, parse_rudder_report,
};

// Device database re-exports
pub use devices::{
    TURTLE_BEACH_VID, VelocityOneDevice, capabilities, identify_device, is_turtle_beach_device,
};

// Protocol re-exports
pub use protocol::{
    DisplayCommand, DisplayPage, FLIGHT_MIN_REPORT_BYTES, FLIGHTSTICK_MIN_REPORT_BYTES,
    FlightLedState, GearLedState, ToggleSwitchPosition, TrimWheelTracker, VelocityOneFlightReport,
    VelocityOneFlightstickReport, decode_all_toggles, decode_toggle_switch, parse_flight_report,
    parse_flightstick_report, serialize_display_command, serialize_gear_led_report,
};

// Profile re-exports
pub use profiles::{DeviceProfile, profile_for_device};
