// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Turtle Beach VelocityOne HOTAS, yoke, and rudder drivers for OpenFlight.
//!
//! # Supported devices
//!
//! | Model                         | VID    | PID    | Tier | Source              |
//! |-------------------------------|--------|--------|------|---------------------|
//! | VelocityOne Flightdeck (yoke) | 0x1432 | 0xB300 | 1    | usb.ids             |
//! | VelocityOne Stick             | 0x1432 | 0xB301 | 3    | PID estimated       |
//! | VelocityOne Rudder            | 0x1432 | 0xB302 | 3    | PID estimated       |
//!
//! Tier 3 entries have estimated PIDs and require USB capture for verification.

pub mod velocityone;

pub use velocityone::{
    FLIGHTDECK_MIN_REPORT_BYTES, RUDDER_MIN_REPORT_BYTES, TurtleBeachError,
    VelocityOneFlightdeckReport, VelocityOneModel, VelocityOneRudderReport,
    parse_flightdeck_report, parse_rudder_report,
};
