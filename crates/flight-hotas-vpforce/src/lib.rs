// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! VPforce Rhino FFB joystick HID input support for OpenFlight.
//!
//! This crate handles **HID input only** (axes, buttons, hat switch).
//! Force-feedback output is implemented in `flight-ffb-vpforce`.
//!
//! # USB Identifiers
//!
//! | Product       | VID    | PID    |
//! |---------------|--------|--------|
//! | Rhino v2      | 0x0483 | 0xA1C0 |
//! | Rhino v3 (Mk II) | 0x0483 | 0xA1C1 |
//!
//! VID 0x0483 belongs to STMicroelectronics (the MCU vendor). This is expected
//! for VPforce Rhino hardware and does not indicate a generic STM device.
//!
//! # Report format
//!
//! The Rhino exposes a standard USB HID joystick with a 20-byte input report:
//!
//! ```text
//! byte  0         : report_id (0x01)
//! bytes  1– 2     : X  (roll),  i16 LE, range −32768..32767
//! bytes  3– 4     : Y  (pitch), i16 LE
//! bytes  5– 6     : Z  (throttle slider), i16 LE
//! bytes  7– 8     : Rx (rocker), i16 LE
//! bytes  9–10     : Ry (unused by default firmware), i16 LE
//! bytes 11–12     : Rz (twist), i16 LE
//! bytes 13–16     : button mask, u32 LE (bit 0 = button 1, …, bit 31 = button 32)
//! byte  17        : POV hat (0=N, 1=NE, 2=E, …, 7=NW; 0xFF = centred)
//! bytes 18–19     : reserved / padding
//! ```
//!
//! Format validated by the property tests in `flight-ffb-vpforce::input` and
//! confirmed against `compat/devices/vpforce/rhino.yaml`.

pub mod rhino;

pub use flight_hid_support::device_support::{
    VPFORCE_RHINO_PID_V2, VPFORCE_RHINO_PID_V3, VPFORCE_VENDOR_ID, VpforceModel, is_vpforce_device,
    vpforce_model,
};

pub use rhino::{
    RHINO_MIN_REPORT_BYTES, RhinoAxes, RhinoButtons, RhinoInputState, RhinoParseError,
    parse_rhino_report,
};
