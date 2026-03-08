// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! GoFlight USB HID panel module drivers for OpenFlight.
//!
//! GoFlight modules communicate over USB HID and share a common 8-byte report
//! format for encoder deltas, button states, and LED output.

pub mod modules;

pub use modules::{
    GOFLIGHT_MIN_REPORT_BYTES, GoFlightError, GoFlightModule, GoFlightReport, build_led_command,
    parse_report,
};
