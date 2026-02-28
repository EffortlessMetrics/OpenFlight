// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Logitech racing wheel parsers for OpenFlight.
//!
//! Supports:
//! - G29 (VID 0x046D, PID 0xC24F) — 900° wheel with 3 pedals and FFB
//! - G920 (VID 0x046D, PID 0xC262) — Xbox/PC version of the G29
//! - G923 PS (VID 0x046D, PID 0xC266) — G923 with TrueForce (PS)
//! - G923 Xbox (VID 0x046D, PID 0xC267) — G923 with TrueForce (Xbox/PC)
//! - G27 (VID 0x046D, PID 0xC29B) — older wheel with H-pattern shifter
//! - G25 (VID 0x046D, PID 0xC299) — classic GT wheel (paddle shifters only)
//!
//! # Architecture
//!
//! Each parser accepts a raw HID report byte slice and returns a state struct
//! with raw axis values (u16). Use [`normalize_wheel`] and [`normalize_pedal`]
//! to convert raw values to the −1.0..=1.0 / 0.0..=1.0 ranges expected by
//! the OpenFlight axis pipeline.

pub mod g27;
pub mod g29;

pub use g27::{G25_PID, G27_PID, G27State, parse_g27};
pub use g29::{G29_PID, G29State, G920_PID, G923_PS_PID, G923_XBOX_PID, LOGITECH_VID, parse_g29};

use thiserror::Error;

/// Errors returned by wheel HID report parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WheelError {
    #[error("Report too short: need {need}, got {got}")]
    TooShort { need: usize, got: usize },
    #[error("Invalid report ID: {0:#04x}")]
    InvalidReportId(u8),
}

/// Normalize a 16-bit wheel axis (0–65535) to −1.0..=1.0.
///
/// `32768` maps to approximately `0.0` (center).
#[inline]
pub fn normalize_wheel(raw: u16) -> f32 {
    ((raw as f32 - 32767.5) / 32767.5).clamp(-1.0, 1.0)
}

/// Normalize a 16-bit pedal axis (0–65535) to 0.0..=1.0.
///
/// `0` = fully released, `65535` = fully depressed.
#[inline]
pub fn normalize_pedal(raw: u16) -> f32 {
    (raw as f32 / 65535.0).clamp(0.0, 1.0)
}
