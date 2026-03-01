// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Logitech/Saitek HID protocol definitions for flight peripherals.
//!
//! This module provides:
//! - Device identification table for all Logitech/Saitek flight peripherals
//! - X52/X52 Pro MFD display command builders (text, clear, line select)
//! - X52/X52 Pro LED control command builders (color, blink, brightness)
//! - X56 RGB LED control command builders
//! - Mode selector handling (3 modes × button matrix)
//!
//! All functions produce raw byte buffers suitable for HID output reports.
//! No hardware I/O is performed here — callers are responsible for sending
//! the buffers to the device via `hidapi` or equivalent.
//!
//! # Protocol Status
//!
//! **UNVERIFIED** — Based on community reverse-engineering (libx52, x52pro-linux,
//! SDL2 controller DB). See `docs/reference/hotas-claims.md` for per-claim
//! verification status.

use serde::{Deserialize, Serialize};

// ── Device identification ──────────────────────────────────────────────────────

/// Saitek vendor ID (pre-Logitech acquisition).
pub const SAITEK_VID: u16 = 0x06A3;

/// Mad Catz vendor ID (used for early X56 production runs).
pub const MAD_CATZ_VID: u16 = 0x0738;

/// Logitech vendor ID.
pub const LOGITECH_VID: u16 = 0x046D;

/// Known Logitech/Saitek flight peripheral identifiers.
///
/// Each entry carries a USB VID/PID pair and a human-readable description.
/// Use [`identify_device`] to look up a device by VID/PID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceId {
    /// Saitek X52 HOTAS (unified USB).
    X52,
    /// Saitek X52 Pro HOTAS (unified USB).
    X52Pro,
    /// Saitek X55 Rhino — stick unit.
    X55Stick,
    /// Saitek X55 Rhino — throttle unit.
    X55Throttle,
    /// Mad Catz X56 Rhino — stick unit.
    X56MadCatzStick,
    /// Mad Catz X56 Rhino — throttle unit.
    X56MadCatzThrottle,
    /// Saitek Pro Flight Yoke System.
    ProFlightYoke,
    /// Saitek Pro Flight Quadrant.
    ProFlightQuadrant,
    /// Saitek Pro Flight Rudder Pedals.
    ProFlightRudderPedals,
    /// Saitek Pro Flight Combat Rudder.
    ProFlightCombatRudder,
    /// Logitech G Flight Yoke System.
    GFlightYoke,
    /// Logitech G Flight Throttle Quadrant.
    GFlightThrottle,
    /// Logitech Flight Rudder Pedals.
    FlightRudderPedals,
}

/// A device identification table entry.
#[derive(Debug, Clone, Copy)]
pub struct DeviceInfo {
    /// USB Vendor ID.
    pub vid: u16,
    /// USB Product ID.
    pub pid: u16,
    /// Device type identifier.
    pub id: DeviceId,
    /// Human-readable device name.
    pub name: &'static str,
}

/// Complete device identification table for known Logitech/Saitek flight peripherals.
pub const DEVICE_TABLE: &[DeviceInfo] = &[
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x0255,
        id: DeviceId::X52,
        name: "Saitek X52 Flight Control System (v1)",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x075C,
        id: DeviceId::X52,
        name: "Saitek X52 Flight Control System",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x0762,
        id: DeviceId::X52Pro,
        name: "Saitek X52 Pro Flight Control System",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x2215,
        id: DeviceId::X55Stick,
        name: "Saitek X55 Rhino Stick",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0xA215,
        id: DeviceId::X55Throttle,
        name: "Saitek X55 Rhino Throttle",
    },
    DeviceInfo {
        vid: MAD_CATZ_VID,
        pid: 0x2221,
        id: DeviceId::X56MadCatzStick,
        name: "Mad Catz / Saitek X56 Rhino Stick",
    },
    DeviceInfo {
        vid: MAD_CATZ_VID,
        pid: 0xA221,
        id: DeviceId::X56MadCatzThrottle,
        name: "Mad Catz / Saitek X56 Rhino Throttle",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x0BAC,
        id: DeviceId::ProFlightYoke,
        name: "Saitek Pro Flight Yoke System",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x0C2D,
        id: DeviceId::ProFlightQuadrant,
        name: "Saitek Pro Flight Quadrant",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x0763,
        id: DeviceId::ProFlightRudderPedals,
        name: "Saitek Pro Flight Rudder Pedals",
    },
    DeviceInfo {
        vid: SAITEK_VID,
        pid: 0x0764,
        id: DeviceId::ProFlightCombatRudder,
        name: "Saitek Pro Flight Combat Rudder",
    },
    DeviceInfo {
        vid: LOGITECH_VID,
        pid: 0xC259,
        id: DeviceId::GFlightYoke,
        name: "Logitech G Flight Yoke System",
    },
    DeviceInfo {
        vid: LOGITECH_VID,
        pid: 0xC25A,
        id: DeviceId::GFlightThrottle,
        name: "Logitech G Flight Throttle Quadrant",
    },
    DeviceInfo {
        vid: LOGITECH_VID,
        pid: 0xC264,
        id: DeviceId::FlightRudderPedals,
        name: "Logitech Flight Rudder Pedals",
    },
];

/// Look up a device by USB VID/PID pair.
///
/// Returns `None` if the VID/PID combination is not in the device table.
pub fn identify_device(vid: u16, pid: u16) -> Option<&'static DeviceInfo> {
    DEVICE_TABLE.iter().find(|d| d.vid == vid && d.pid == pid)
}

// ── X52 / X52 Pro MFD display protocol ────────────────────────────────────────
//
// Source: nirenjan/libx52 (x52_commands.h, x52_control.c)
//
// The X52 Pro MFD is controlled via USB vendor control transfers, NOT HID
// output reports. All commands use:
//   bmRequestType = 0x40 (vendor, device, host-to-device)
//   bRequest      = 0x91 (X52_VENDOR_REQUEST)
//   wIndex        = command index
//   wValue        = command value
//   data          = none (zero-length data phase)

/// USB vendor bRequest value for all X52 MFD/LED commands.
///
/// Source: libx52 `X52_VENDOR_REQUEST = 0x91`.
pub const X52_VENDOR_REQUEST: u8 = 0x91;

/// bmRequestType for X52 vendor commands (vendor, device, host-to-device).
pub const X52_VENDOR_REQUEST_TYPE: u8 = 0x40;

/// MFD line 1 index (clear = `| 0x08`, write = `| 0x00`).
pub const X52_MFD_LINE1: u16 = 0x00D1;

/// MFD line 2 index.
pub const X52_MFD_LINE2: u16 = 0x00D2;

/// MFD line 3 index.
pub const X52_MFD_LINE3: u16 = 0x00D4;

/// OR-mask applied to line index to select "clear line" sub-command.
pub const X52_MFD_CLEAR_LINE: u16 = 0x0008;

/// OR-mask applied to line index to select "write line" sub-command.
pub const X52_MFD_WRITE_LINE: u16 = 0x0000;

/// MFD brightness control index (wValue = 0..128).
pub const X52_MFD_BRIGHTNESS: u16 = 0x00B1;

/// LED brightness control index (wValue = 0..128).
pub const X52_LED_BRIGHTNESS: u16 = 0x00B2;

/// LED set command index (wValue = `state | (led_bit << 8)`).
pub const X52_LED_CMD: u16 = 0x00B8;

/// Shift indicator command index (wValue = 0x51 on, 0x50 off).
pub const X52_SHIFT_INDICATOR: u16 = 0x00FD;

/// Blink indicator command index (wValue = 0x51 on, 0x50 off).
pub const X52_BLINK_INDICATOR: u16 = 0x00B4;

/// Date DD-MM command index.
pub const X52_DATE_DDMM: u16 = 0x00C4;

/// Date year command index.
pub const X52_DATE_YEAR: u16 = 0x00C8;

/// Clock 1 (primary time) command index.
pub const X52_TIME_CLOCK1: u16 = 0x00C0;

/// Clock 2 offset command index.
pub const X52_OFFS_CLOCK2: u16 = 0x00C1;

/// Clock 3 offset command index.
pub const X52_OFFS_CLOCK3: u16 = 0x00C2;

// ── Legacy HID report constants (kept for backward compatibility) ──────────

/// HID output report command byte for MFD text-line write.
pub const MFD_CMD_LINE: u8 = 0xB4;

/// HID output report command byte for MFD brightness control.
pub const MFD_CMD_BRIGHTNESS: u8 = 0xB1;

/// HID output report command byte for MFD clear-line.
pub const MFD_CMD_CLEAR: u8 = 0xB4;

/// Maximum characters per MFD display line.
pub const MFD_LINE_LENGTH: usize = 16;

/// Number of text lines on the X52 Pro MFD.
pub const MFD_LINE_COUNT: u8 = 3;

/// Total size of an MFD text-line HID output report.
pub const MFD_LINE_REPORT_SIZE: usize = 3 + MFD_LINE_LENGTH; // report_id + cmd + line + 16 chars

/// Encode text for the X52 Pro MFD.
///
/// Non-printable or non-ASCII characters are replaced with `'?'`.
/// The result is truncated to [`MFD_LINE_LENGTH`] and space-padded.
pub fn mfd_encode_text(text: &str) -> [u8; MFD_LINE_LENGTH] {
    let mut buf = [b' '; MFD_LINE_LENGTH];
    for (i, ch) in text.chars().take(MFD_LINE_LENGTH).enumerate() {
        buf[i] = if ch.is_ascii() && ch >= ' ' {
            ch as u8
        } else {
            b'?'
        };
    }
    buf
}

/// Build an MFD text-line write command.
///
/// # Arguments
/// * `line` — Display line index (0, 1, or 2).
/// * `text` — Text to display (truncated/padded to 16 characters).
///
/// # Returns
/// A 19-byte HID output report buffer, or `None` if `line >= 3`.
///
/// # Protocol
/// ```text
/// Byte  0:     0x00       (HID report ID — unnumbered)
/// Byte  1:     0xB4       (MFD text command)
/// Byte  2:     line       (0, 1, or 2)
/// Bytes 3–18:  ASCII text (space-padded to 16 chars)
/// ```
pub fn mfd_write_line(line: u8, text: &str) -> Option<[u8; MFD_LINE_REPORT_SIZE]> {
    if line >= MFD_LINE_COUNT {
        return None;
    }
    let mut buf = [b' '; MFD_LINE_REPORT_SIZE];
    buf[0] = 0x00;
    buf[1] = MFD_CMD_LINE;
    buf[2] = line;
    let encoded = mfd_encode_text(text);
    buf[3..3 + MFD_LINE_LENGTH].copy_from_slice(&encoded);
    Some(buf)
}

/// Build an MFD clear-line command (writes 16 spaces to the given line).
///
/// Returns `None` if `line >= 3`.
pub fn mfd_clear_line(line: u8) -> Option<[u8; MFD_LINE_REPORT_SIZE]> {
    mfd_write_line(line, "")
}

/// Build a complete MFD clear command (clears all 3 lines).
///
/// Returns an array of 3 HID output report buffers.
pub fn mfd_clear_all() -> [[u8; MFD_LINE_REPORT_SIZE]; 3] {
    [
        mfd_clear_line(0).expect("line 0 valid"),
        mfd_clear_line(1).expect("line 1 valid"),
        mfd_clear_line(2).expect("line 2 valid"),
    ]
}

/// Build an MFD brightness command.
///
/// # Arguments
/// * `level` — Brightness level, clamped to 0–127.
///
/// # Protocol
/// ```text
/// Byte 0: 0x00  (HID report ID)
/// Byte 1: 0xB1  (brightness command)
/// Byte 2: level (0–127)
/// ```
pub fn mfd_set_brightness(level: u8) -> [u8; 3] {
    [0x00, MFD_CMD_BRIGHTNESS, level.min(127)]
}

// ── X52 / X52 Pro vendor-command MFD protocol ──────────────────────────────────
//
// These functions build USB vendor control transfer descriptors that match
// the libx52 protocol (nirenjan/libx52, x52_control.c).  Each returns
// `(request_type, request, wValue, wIndex)` suitable for
// `libusb_control_transfer`.

/// Resolve the line index constant for a given MFD line number (0, 1, 2).
const fn x52_mfd_line_index(line: u8) -> Option<u16> {
    match line {
        0 => Some(X52_MFD_LINE1),
        1 => Some(X52_MFD_LINE2),
        2 => Some(X52_MFD_LINE3),
        _ => None,
    }
}

/// Build a vendor command to clear a single MFD line.
///
/// Returns `None` if `line >= 3`.
pub fn x52_mfd_clear_line(line: u8) -> Option<(u8, u8, u16, u16)> {
    let idx = x52_mfd_line_index(line)?;
    Some((
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        0x0000,
        idx | X52_MFD_CLEAR_LINE,
    ))
}

/// Build a sequence of vendor commands to write a 16-character text line to
/// the MFD.
///
/// Returns `None` if `line >= 3`.  The returned vec contains:
///   1. One clear-line command.
///   2. Up to 8 write commands (2 characters per command).
pub fn x52_mfd_write_line(line: u8, text: &str) -> Option<Vec<(u8, u8, u16, u16)>> {
    let idx = x52_mfd_line_index(line)?;
    let encoded = mfd_encode_text(text);
    let mut cmds = Vec::with_capacity(9);
    // Clear the line first.
    cmds.push((
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        0x0000,
        idx | X52_MFD_CLEAR_LINE,
    ));
    // Write 2 characters at a time (LSB = char 0, MSB = char 1).
    for pair in encoded.chunks(2) {
        let lo = pair[0] as u16;
        let hi = if pair.len() > 1 { pair[1] as u16 } else { b' ' as u16 };
        let w_value = lo | (hi << 8);
        cmds.push((
            X52_VENDOR_REQUEST_TYPE,
            X52_VENDOR_REQUEST,
            w_value,
            idx | X52_MFD_WRITE_LINE,
        ));
    }
    Some(cmds)
}

/// Build a vendor command to set MFD display brightness.
///
/// `level` is clamped to 0–128 (libx52 maximum).
pub fn x52_mfd_set_brightness(level: u8) -> (u8, u8, u16, u16) {
    (
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        level.min(128) as u16,
        X52_MFD_BRIGHTNESS,
    )
}

/// Build a vendor command to set LED panel brightness.
///
/// `level` is clamped to 0–128.
pub fn x52_led_set_brightness(level: u8) -> (u8, u8, u16, u16) {
    (
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        level.min(128) as u16,
        X52_LED_BRIGHTNESS,
    )
}

/// Build a vendor command to set the X52 primary clock (Clock 1).
///
/// `hour` (0–23), `minute` (0–59).  `h24` selects 24-hour format.
pub fn x52_set_clock(hour: u8, minute: u8, h24: bool) -> (u8, u8, u16, u16) {
    let h = (hour as u16).min(23);
    let m = (minute as u16).min(59);
    let w_value = m | (h << 8) | if h24 { 1 << 15 } else { 0 };
    (
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        w_value,
        X52_TIME_CLOCK1,
    )
}

/// Build a vendor command to set the X52 date (DD-MM).
///
/// `day` (1–31), `month` (1–12).
pub fn x52_set_date(day: u8, month: u8) -> (u8, u8, u16, u16) {
    let d = (day as u16).clamp(1, 31);
    let m = (month as u16).clamp(1, 12);
    let w_value = d | (m << 8);
    (
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        w_value,
        X52_DATE_DDMM,
    )
}

/// Build a vendor command to set the date year.
pub fn x52_set_year(year: u8) -> (u8, u8, u16, u16) {
    (
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        year as u16,
        X52_DATE_YEAR,
    )
}

/// Build a vendor command to control the shift indicator.
pub fn x52_set_shift(on: bool) -> (u8, u8, u16, u16) {
    (
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        if on { 0x51 } else { 0x50 },
        X52_SHIFT_INDICATOR,
    )
}

/// Build a vendor command to control the blink indicator.
pub fn x52_set_blink(on: bool) -> (u8, u8, u16, u16) {
    (
        X52_VENDOR_REQUEST_TYPE,
        X52_VENDOR_REQUEST,
        if on { 0x51 } else { 0x50 },
        X52_BLINK_INDICATOR,
    )
}

// ── X52 / X52 Pro LED control protocol ─────────────────────────────────────────

/// LED identifiers for X52/X52 Pro devices.
///
/// The X52 Pro has 11 controllable bi-color (green/amber/red) LEDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum X52LedId {
    Fire,
    ButtonA,
    ButtonB,
    ButtonD,
    ButtonE,
    Toggle1,
    Toggle2,
    Toggle3,
    Pov2,
    Clutch,
    Throttle,
}

/// LED color states for X52/X52 Pro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum X52LedColor {
    Off,
    Green,
    Amber,
    Red,
}

/// X52 LED command: bRequest is `X52_VENDOR_REQUEST` (0x91), wIndex is
/// `X52_LED_CMD` (0xB8).  wValue encodes the LED bit and on/off state:
/// `wValue = state | (led_bit << 8)`.
///
/// Source: libx52 `x52_control.c` — `libx52_vendor_command(x52, X52_LED, value | (bit << 8))`.
pub const LED_REQUEST: u8 = X52_VENDOR_REQUEST;

/// bmRequestType for LED USB control transfer (vendor, device, host-to-device).
pub const LED_REQUEST_TYPE: u8 = X52_VENDOR_REQUEST_TYPE;

/// Map an LED identifier to its libx52 hardware bit index.
///
/// Source: libx52 `x52_common.h` LED bit definitions:
///   Fire=1, A_RED=2, A_GREEN=3, B_RED=4, B_GREEN=5, D_RED=6, D_GREEN=7,
///   E_RED=8, E_GREEN=9, T1_RED=10, T1_GREEN=11, T2_RED=12, T2_GREEN=13,
///   T3_RED=14, T3_GREEN=15, POV_RED=16, POV_GREEN=17, I_RED=18, I_GREEN=19,
///   THROTTLE=20.
///
/// For bicolor LEDs, Red is the lower (even) bit and Green is the next (odd)
/// bit.  Fire and Throttle are single-color LEDs with only one bit each.
pub fn x52_led_index(led: X52LedId) -> u8 {
    match led {
        X52LedId::Fire => 1,
        X52LedId::ButtonA => 2,
        X52LedId::ButtonB => 4,
        X52LedId::ButtonD => 6,
        X52LedId::ButtonE => 8,
        X52LedId::Toggle1 => 10,
        X52LedId::Toggle2 => 12,
        X52LedId::Toggle3 => 14,
        X52LedId::Pov2 => 16,
        X52LedId::Clutch => 18,
        X52LedId::Throttle => 20,
    }
}

/// Map an LED color to its protocol state.
///
/// For bicolor LEDs (all except Fire/Throttle) the caller must issue two
/// vendor commands — one for the red sub-LED and one for the green sub-LED.
/// The state value is 0 (off) or 1 (on).
///
/// | Desired color | Red sub-LED | Green sub-LED |
/// |:---:|:---:|:---:|
/// | Off   | 0 | 0 |
/// | Red   | 1 | 0 |
/// | Green | 0 | 1 |
/// | Amber | 1 | 1 |
///
/// For single-color LEDs (Fire, Throttle), only one command is needed with
/// state = 0 (off) or 1 (on).
pub fn x52_led_color_code(color: X52LedColor) -> u8 {
    match color {
        X52LedColor::Off => 0,
        X52LedColor::Green => 1,
        X52LedColor::Amber => 2,
        X52LedColor::Red => 3,
    }
}

/// Build a USB vendor control transfer descriptor for setting an X52 LED.
///
/// For bicolor LEDs, this builds the red sub-LED command.  Call
/// `x52_led_command_green` for the green sub-LED.  For single-color LEDs
/// (Fire, Throttle), only one command is needed.
///
/// Returns `(request_type, request, wValue, wIndex)` suitable for a
/// USB control transfer via `libusb_control_transfer`.
///
/// Source: libx52 — `libx52_vendor_command(x52, X52_LED, value | (bit << 8))`
/// where X52_LED = 0xB8 is the wIndex, and wValue encodes state + LED bit.
pub fn x52_led_command(led: X52LedId, color: X52LedColor) -> (u8, u8, u16, u16) {
    let led_bit = x52_led_index(led);
    let color_code = x52_led_color_code(color);
    // For bicolor LEDs, the red sub-LED is at `led_bit` and green at `led_bit + 1`.
    // The state for the red sub-LED: on if color is Red or Amber, off otherwise.
    let red_state: u16 = if color_code & 0x02 != 0 { 1 } else { 0 };
    let w_value = red_state | ((led_bit as u16) << 8);
    (
        LED_REQUEST_TYPE,
        LED_REQUEST,
        w_value,
        X52_LED_CMD,
    )
}

/// Build the green sub-LED vendor command for a bicolor X52 LED.
///
/// Not meaningful for single-color LEDs (Fire, Throttle).
pub fn x52_led_command_green(led: X52LedId, color: X52LedColor) -> (u8, u8, u16, u16) {
    let led_bit = x52_led_index(led) + 1;
    let color_code = x52_led_color_code(color);
    let green_state: u16 = if color_code & 0x01 != 0 { 1 } else { 0 };
    let w_value = green_state | ((led_bit as u16) << 8);
    (
        LED_REQUEST_TYPE,
        LED_REQUEST,
        w_value,
        X52_LED_CMD,
    )
}

/// Blink pattern for X52 LEDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct X52BlinkPattern {
    /// LED to blink.
    pub led: X52LedId,
    /// Primary color (shown during "on" phase).
    pub on_color: X52LedColor,
    /// Secondary color (shown during "off" phase, usually Off).
    pub off_color: X52LedColor,
    /// On-phase duration in milliseconds.
    pub on_ms: u16,
    /// Off-phase duration in milliseconds.
    pub off_ms: u16,
}

impl X52BlinkPattern {
    /// Create a simple on/off blink pattern.
    pub const fn new(led: X52LedId, color: X52LedColor, interval_ms: u16) -> Self {
        Self {
            led,
            on_color: color,
            off_color: X52LedColor::Off,
            on_ms: interval_ms,
            off_ms: interval_ms,
        }
    }

    /// Return the command for the current phase.
    ///
    /// `on == true` → on_color, `on == false` → off_color.
    pub fn command_for_phase(&self, on: bool) -> (u8, u8, u16, u16) {
        let color = if on { self.on_color } else { self.off_color };
        x52_led_command(self.led, color)
    }
}

// ── X56 RGB LED control protocol ───────────────────────────────────────────────

/// RGB color value for X56 lighting zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const OFF: Self = Self::new(0, 0, 0);
    pub const RED: Self = Self::new(255, 0, 0);
    pub const GREEN: Self = Self::new(0, 255, 0);
    pub const BLUE: Self = Self::new(0, 0, 255);
    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const AMBER: Self = Self::new(255, 191, 0);
}

/// X56 RGB lighting zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum X56RgbZone {
    /// Stick base ring.
    StickBase,
    /// Stick grip accent.
    StickGrip,
    /// Throttle base ring.
    ThrottleBase,
    /// Throttle grip accent.
    ThrottleGrip,
}

/// Hypothesized HID report ID for X56 RGB control.
pub const X56_RGB_REPORT_ID: u8 = 0x00;

/// Hypothesized command byte for X56 RGB zone set.
pub const X56_RGB_CMD: u8 = 0x09;

/// X56 RGB report size (report_id + cmd + zone + r + g + b).
pub const X56_RGB_REPORT_SIZE: usize = 6;

/// Map an X56 RGB zone to its hypothesized hardware index.
pub fn x56_rgb_zone_index(zone: X56RgbZone) -> u8 {
    match zone {
        X56RgbZone::StickBase => 0,
        X56RgbZone::StickGrip => 1,
        X56RgbZone::ThrottleBase => 2,
        X56RgbZone::ThrottleGrip => 3,
    }
}

/// Build an X56 RGB zone-set command.
///
/// # Protocol (hypothesized)
/// ```text
/// Byte 0: 0x00  (HID report ID)
/// Byte 1: 0x09  (RGB set command)
/// Byte 2: zone  (0–3)
/// Byte 3: red   (0–255)
/// Byte 4: green (0–255)
/// Byte 5: blue  (0–255)
/// ```
pub fn x56_rgb_set_zone(zone: X56RgbZone, color: RgbColor) -> [u8; X56_RGB_REPORT_SIZE] {
    [
        X56_RGB_REPORT_ID,
        X56_RGB_CMD,
        x56_rgb_zone_index(zone),
        color.r,
        color.g,
        color.b,
    ]
}

/// Build commands to set all X56 RGB zones to the same color.
pub fn x56_rgb_set_all(color: RgbColor) -> [[u8; X56_RGB_REPORT_SIZE]; 4] {
    [
        x56_rgb_set_zone(X56RgbZone::StickBase, color),
        x56_rgb_set_zone(X56RgbZone::StickGrip, color),
        x56_rgb_set_zone(X56RgbZone::ThrottleBase, color),
        x56_rgb_set_zone(X56RgbZone::ThrottleGrip, color),
    ]
}

// ── X52 Mode Selector ──────────────────────────────────────────────────────────

/// X52 mode selector positions.
///
/// The X52/X52 Pro throttle has a 3-position rotary mode selector that
/// remaps the button matrix. Mode is reported in the HID input report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum X52Mode {
    /// Mode 1 (green LED on throttle).
    #[default]
    Mode1,
    /// Mode 2 (amber LED on throttle).
    Mode2,
    /// Mode 3 (red LED on throttle).
    Mode3,
}

impl X52Mode {
    /// Decode mode from the raw mode-selector bits in the HID report.
    ///
    /// The mode selector is hypothesized to occupy 2 bits in the button field:
    /// - `0b00` or `0b01` → Mode 1
    /// - `0b10` → Mode 2
    /// - `0b11` → Mode 3
    pub fn from_raw(raw: u8) -> Self {
        match raw & 0x03 {
            0 | 1 => Self::Mode1,
            2 => Self::Mode2,
            _ => Self::Mode3,
        }
    }

    /// Return the 0-based mode index (0, 1, or 2).
    pub const fn index(self) -> usize {
        match self {
            Self::Mode1 => 0,
            Self::Mode2 => 1,
            Self::Mode3 => 2,
        }
    }
}

/// Resolve a physical button ID to a logical button ID based on the current mode.
///
/// The X52 has 3 modes, and each physical button can be mapped to a different
/// logical function per mode. `button_map` is indexed as `[mode_index][physical_button]`.
///
/// Returns `None` if the physical button is out of range.
pub fn resolve_mode_button(
    mode: X52Mode,
    physical_button: u8,
    button_map: &[[u8; 32]; 3],
) -> Option<u8> {
    let idx = physical_button as usize;
    if idx >= 32 {
        return None;
    }
    Some(button_map[mode.index()][idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Device identification tests ────────────────────────────────────────

    #[test]
    fn identify_x52_pro() {
        let info = identify_device(SAITEK_VID, 0x0762).unwrap();
        assert_eq!(info.id, DeviceId::X52Pro);
        assert!(info.name.contains("X52 Pro"));
    }

    #[test]
    fn identify_x56_madcatz_stick() {
        let info = identify_device(MAD_CATZ_VID, 0x2221).unwrap();
        assert_eq!(info.id, DeviceId::X56MadCatzStick);
    }

    #[test]
    fn identify_g_flight_yoke() {
        let info = identify_device(LOGITECH_VID, 0xC259).unwrap();
        assert_eq!(info.id, DeviceId::GFlightYoke);
    }

    #[test]
    fn identify_unknown_device_returns_none() {
        assert!(identify_device(0xDEAD, 0xBEEF).is_none());
    }

    #[test]
    fn device_table_has_no_duplicate_vid_pid() {
        for (i, a) in DEVICE_TABLE.iter().enumerate() {
            for b in &DEVICE_TABLE[i + 1..] {
                assert!(
                    !(a.vid == b.vid && a.pid == b.pid),
                    "duplicate VID/PID: {:04X}:{:04X} ({} vs {})",
                    a.vid,
                    a.pid,
                    a.name,
                    b.name,
                );
            }
        }
    }

    #[test]
    fn device_table_vid_pid_constants() {
        assert_eq!(SAITEK_VID, 0x06A3);
        assert_eq!(MAD_CATZ_VID, 0x0738);
        assert_eq!(LOGITECH_VID, 0x046D);
    }

    // ── MFD display command tests ──────────────────────────────────────────

    #[test]
    fn mfd_write_line_basic() {
        let buf = mfd_write_line(0, "HELLO").unwrap();
        assert_eq!(buf[0], 0x00, "report ID");
        assert_eq!(buf[1], MFD_CMD_LINE, "command byte");
        assert_eq!(buf[2], 0, "line index");
        assert_eq!(&buf[3..8], b"HELLO");
        assert_eq!(buf[8], b' ', "space-padded");
        assert_eq!(buf.len(), MFD_LINE_REPORT_SIZE);
    }

    #[test]
    fn mfd_write_line_all_lines() {
        for line in 0..MFD_LINE_COUNT {
            let buf = mfd_write_line(line, "TEST").unwrap();
            assert_eq!(buf[2], line);
        }
    }

    #[test]
    fn mfd_write_line_invalid_line_returns_none() {
        assert!(mfd_write_line(3, "FAIL").is_none());
        assert!(mfd_write_line(255, "FAIL").is_none());
    }

    #[test]
    fn mfd_write_line_truncates_long_text() {
        let long = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let buf = mfd_write_line(0, long).unwrap();
        assert_eq!(&buf[3..19], b"ABCDEFGHIJKLMNOP");
    }

    #[test]
    fn mfd_write_line_pads_short_text() {
        let buf = mfd_write_line(0, "HI").unwrap();
        assert_eq!(&buf[3..5], b"HI");
        for &b in &buf[5..19] {
            assert_eq!(b, b' ', "should be space-padded");
        }
    }

    #[test]
    fn mfd_encode_text_replaces_non_ascii() {
        let encoded = mfd_encode_text("H\u{00C9}LLO");
        assert_eq!(&encoded[..5], b"H?LLO");
    }

    #[test]
    fn mfd_encode_text_control_chars_replaced() {
        let encoded = mfd_encode_text("\x01\x02\x03");
        assert_eq!(&encoded[..3], b"???");
    }

    #[test]
    fn mfd_clear_line_produces_spaces() {
        let buf = mfd_clear_line(1).unwrap();
        assert_eq!(buf[1], MFD_CMD_LINE);
        assert_eq!(buf[2], 1);
        for &b in &buf[3..19] {
            assert_eq!(b, b' ');
        }
    }

    #[test]
    fn mfd_clear_all_produces_three_reports() {
        let reports = mfd_clear_all();
        assert_eq!(reports.len(), 3);
        for (i, report) in reports.iter().enumerate() {
            assert_eq!(report[2], i as u8, "line index mismatch");
        }
    }

    #[test]
    fn mfd_brightness_clamps_to_127() {
        let buf = mfd_set_brightness(200);
        assert_eq!(buf[0], 0x00);
        assert_eq!(buf[1], MFD_CMD_BRIGHTNESS);
        assert_eq!(buf[2], 127);
    }

    #[test]
    fn mfd_brightness_preserves_valid_value() {
        let buf = mfd_set_brightness(64);
        assert_eq!(buf[2], 64);
    }

    #[test]
    fn mfd_brightness_zero() {
        let buf = mfd_set_brightness(0);
        assert_eq!(buf[2], 0);
    }

    // ── LED control command tests ──────────────────────────────────────────

    #[test]
    fn led_index_mapping() {
        // libx52 bit indices
        assert_eq!(x52_led_index(X52LedId::Fire), 1);
        assert_eq!(x52_led_index(X52LedId::ButtonA), 2);
        assert_eq!(x52_led_index(X52LedId::Throttle), 20);
    }

    #[test]
    fn led_color_code_mapping() {
        assert_eq!(x52_led_color_code(X52LedColor::Off), 0);
        assert_eq!(x52_led_color_code(X52LedColor::Green), 1);
        assert_eq!(x52_led_color_code(X52LedColor::Amber), 2);
        assert_eq!(x52_led_color_code(X52LedColor::Red), 3);
    }

    #[test]
    fn led_command_structure() {
        // ButtonA Green: red sub-LED off (bit 2, state 0), green sub-LED on (bit 3, state 1)
        let (req_type, req, wvalue, windex) =
            x52_led_command(X52LedId::ButtonA, X52LedColor::Green);
        assert_eq!(req_type, LED_REQUEST_TYPE);
        assert_eq!(req, LED_REQUEST);
        // Red sub-LED: state=0 (Green has no red), bit=2 → wValue = 0 | (2 << 8) = 0x0200
        assert_eq!(wvalue, 0x0200);
        assert_eq!(windex, X52_LED_CMD); // 0xB8
    }

    #[test]
    fn led_command_green_sub() {
        // ButtonA Green: green sub-LED on (bit 3, state 1)
        let (_, _, wvalue, windex) =
            x52_led_command_green(X52LedId::ButtonA, X52LedColor::Green);
        // Green sub-LED: state=1, bit=3 → wValue = 1 | (3 << 8) = 0x0301
        assert_eq!(wvalue, 0x0301);
        assert_eq!(windex, X52_LED_CMD);
    }

    #[test]
    fn led_command_fire_red() {
        // Fire is single-color (bit 1). Red state=1 (color code 3 has bit 1 set).
        let (_, _, wvalue, windex) = x52_led_command(X52LedId::Fire, X52LedColor::Red);
        // state = 1 (red bit set), bit = 1 → wValue = 1 | (1 << 8) = 0x0101
        assert_eq!(wvalue, 0x0101);
        assert_eq!(windex, X52_LED_CMD);
    }

    #[test]
    fn led_command_off() {
        let (_, _, wvalue, windex) = x52_led_command(X52LedId::Toggle1, X52LedColor::Off);
        // Off: state=0, bit=10 → wValue = 0 | (10 << 8) = 0x0A00
        assert_eq!(wvalue, 0x0A00);
        assert_eq!(windex, X52_LED_CMD);
    }

    #[test]
    fn blink_pattern_creation() {
        let pattern = X52BlinkPattern::new(X52LedId::Fire, X52LedColor::Red, 500);
        assert_eq!(pattern.led, X52LedId::Fire);
        assert_eq!(pattern.on_color, X52LedColor::Red);
        assert_eq!(pattern.off_color, X52LedColor::Off);
        assert_eq!(pattern.on_ms, 500);
        assert_eq!(pattern.off_ms, 500);
    }

    #[test]
    fn blink_pattern_on_phase() {
        let pattern = X52BlinkPattern::new(X52LedId::ButtonA, X52LedColor::Green, 250);
        let (_, _, _, windex) = pattern.command_for_phase(true);
        // Now wIndex is always X52_LED_CMD (0xB8); color is encoded in wValue
        assert_eq!(windex, X52_LED_CMD);
    }

    #[test]
    fn blink_pattern_off_phase() {
        let pattern = X52BlinkPattern::new(X52LedId::ButtonA, X52LedColor::Green, 250);
        let (_, _, _, windex) = pattern.command_for_phase(false);
        assert_eq!(windex, X52_LED_CMD);
    }

    // ── X56 RGB tests ──────────────────────────────────────────────────────

    #[test]
    fn x56_rgb_zone_indices() {
        assert_eq!(x56_rgb_zone_index(X56RgbZone::StickBase), 0);
        assert_eq!(x56_rgb_zone_index(X56RgbZone::StickGrip), 1);
        assert_eq!(x56_rgb_zone_index(X56RgbZone::ThrottleBase), 2);
        assert_eq!(x56_rgb_zone_index(X56RgbZone::ThrottleGrip), 3);
    }

    #[test]
    fn x56_rgb_set_zone_report_layout() {
        let color = RgbColor::new(128, 64, 32);
        let buf = x56_rgb_set_zone(X56RgbZone::StickBase, color);
        assert_eq!(buf[0], X56_RGB_REPORT_ID);
        assert_eq!(buf[1], X56_RGB_CMD);
        assert_eq!(buf[2], 0); // StickBase zone
        assert_eq!(buf[3], 128); // R
        assert_eq!(buf[4], 64); // G
        assert_eq!(buf[5], 32); // B
        assert_eq!(buf.len(), X56_RGB_REPORT_SIZE);
    }

    #[test]
    fn x56_rgb_set_zone_throttle_grip() {
        let buf = x56_rgb_set_zone(X56RgbZone::ThrottleGrip, RgbColor::RED);
        assert_eq!(buf[2], 3); // ThrottleGrip
        assert_eq!(buf[3], 255);
        assert_eq!(buf[4], 0);
        assert_eq!(buf[5], 0);
    }

    #[test]
    fn x56_rgb_set_all_four_zones() {
        let reports = x56_rgb_set_all(RgbColor::BLUE);
        assert_eq!(reports.len(), 4);
        for (i, report) in reports.iter().enumerate() {
            assert_eq!(report[2], i as u8, "zone index");
            assert_eq!(report[3], 0); // R
            assert_eq!(report[4], 0); // G
            assert_eq!(report[5], 255); // B
        }
    }

    #[test]
    fn x56_rgb_off_is_all_zeros() {
        let buf = x56_rgb_set_zone(X56RgbZone::StickBase, RgbColor::OFF);
        assert_eq!(buf[3], 0);
        assert_eq!(buf[4], 0);
        assert_eq!(buf[5], 0);
    }

    #[test]
    fn rgb_color_presets() {
        assert_eq!(RgbColor::RED, RgbColor::new(255, 0, 0));
        assert_eq!(RgbColor::GREEN, RgbColor::new(0, 255, 0));
        assert_eq!(RgbColor::BLUE, RgbColor::new(0, 0, 255));
        assert_eq!(RgbColor::WHITE, RgbColor::new(255, 255, 255));
        assert_eq!(RgbColor::OFF, RgbColor::new(0, 0, 0));
        assert_eq!(RgbColor::AMBER, RgbColor::new(255, 191, 0));
    }

    // ── Mode selector tests ────────────────────────────────────────────────

    #[test]
    fn mode_from_raw_mode1() {
        assert_eq!(X52Mode::from_raw(0), X52Mode::Mode1);
        assert_eq!(X52Mode::from_raw(1), X52Mode::Mode1);
    }

    #[test]
    fn mode_from_raw_mode2() {
        assert_eq!(X52Mode::from_raw(2), X52Mode::Mode2);
    }

    #[test]
    fn mode_from_raw_mode3() {
        assert_eq!(X52Mode::from_raw(3), X52Mode::Mode3);
    }

    #[test]
    fn mode_from_raw_masks_upper_bits() {
        // Upper bits should be masked off
        assert_eq!(X52Mode::from_raw(0xFC), X52Mode::Mode1); // 0xFC & 0x03 = 0
        assert_eq!(X52Mode::from_raw(0xFE), X52Mode::Mode2); // 0xFE & 0x03 = 2
        assert_eq!(X52Mode::from_raw(0xFF), X52Mode::Mode3); // 0xFF & 0x03 = 3
    }

    #[test]
    fn mode_index() {
        assert_eq!(X52Mode::Mode1.index(), 0);
        assert_eq!(X52Mode::Mode2.index(), 1);
        assert_eq!(X52Mode::Mode3.index(), 2);
    }

    #[test]
    fn mode_default_is_mode1() {
        assert_eq!(X52Mode::default(), X52Mode::Mode1);
    }

    #[test]
    fn resolve_mode_button_basic() {
        // Identity mapping: physical == logical in all modes
        let mut map = [[0u8; 32]; 3];
        for mode in 0..3 {
            for btn in 0..32 {
                map[mode][btn] = btn as u8;
            }
        }
        // Mode 1, button 5
        assert_eq!(resolve_mode_button(X52Mode::Mode1, 5, &map), Some(5));
    }

    #[test]
    fn resolve_mode_button_remapped() {
        let mut map = [[0u8; 32]; 3];
        // In Mode 2, physical button 3 maps to logical 10
        for btn in 0..32 {
            map[0][btn] = btn as u8;
            map[1][btn] = btn as u8;
            map[2][btn] = btn as u8;
        }
        map[1][3] = 10;

        assert_eq!(resolve_mode_button(X52Mode::Mode1, 3, &map), Some(3));
        assert_eq!(resolve_mode_button(X52Mode::Mode2, 3, &map), Some(10));
        assert_eq!(resolve_mode_button(X52Mode::Mode3, 3, &map), Some(3));
    }

    #[test]
    fn resolve_mode_button_out_of_range() {
        let map = [[0u8; 32]; 3];
        assert!(resolve_mode_button(X52Mode::Mode1, 32, &map).is_none());
        assert!(resolve_mode_button(X52Mode::Mode1, 255, &map).is_none());
    }

    #[test]
    fn resolve_mode_button_in_range() {
        let map = [[0u8; 32]; 3];
        assert!(resolve_mode_button(X52Mode::Mode1, 0, &map).is_some());
        assert!(resolve_mode_button(X52Mode::Mode1, 31, &map).is_some());
    }

    #[test]
    fn mode_switching_changes_button_mapping() {
        // Simulate a button map where each mode shifts buttons by mode index
        let mut map = [[0u8; 32]; 3];
        for mode_idx in 0..3 {
            for btn in 0..32 {
                map[mode_idx][btn] = ((btn + mode_idx) % 32) as u8;
            }
        }

        let phys = 5;
        assert_eq!(resolve_mode_button(X52Mode::Mode1, phys, &map), Some(5));
        assert_eq!(resolve_mode_button(X52Mode::Mode2, phys, &map), Some(6));
        assert_eq!(resolve_mode_button(X52Mode::Mode3, phys, &map), Some(7));
    }

    // ── Vendor command MFD tests ───────────────────────────────────────────

    #[test]
    fn x52_mfd_clear_line_valid() {
        let (rt, req, wval, widx) = x52_mfd_clear_line(0).unwrap();
        assert_eq!(rt, X52_VENDOR_REQUEST_TYPE);
        assert_eq!(req, X52_VENDOR_REQUEST);
        assert_eq!(wval, 0x0000);
        assert_eq!(widx, X52_MFD_LINE1 | X52_MFD_CLEAR_LINE);
    }

    #[test]
    fn x52_mfd_clear_line_invalid() {
        assert!(x52_mfd_clear_line(3).is_none());
    }

    #[test]
    fn x52_mfd_write_line_command_count() {
        let cmds = x52_mfd_write_line(0, "HELLO").unwrap();
        // 1 clear + 8 write commands (16 chars / 2 per command)
        assert_eq!(cmds.len(), 9);
    }

    #[test]
    fn x52_mfd_write_line_first_is_clear() {
        let cmds = x52_mfd_write_line(1, "TEST").unwrap();
        let (_, _, _, widx) = cmds[0];
        assert_eq!(widx, X52_MFD_LINE2 | X52_MFD_CLEAR_LINE);
    }

    #[test]
    fn x52_mfd_write_line_char_encoding() {
        let cmds = x52_mfd_write_line(0, "AB").unwrap();
        // First write command (index 1) should encode 'A' (0x41) and 'B' (0x42)
        let (_, _, wval, _) = cmds[1];
        assert_eq!(wval, 0x41 | (0x42 << 8));
    }

    #[test]
    fn x52_mfd_write_line_invalid() {
        assert!(x52_mfd_write_line(3, "FAIL").is_none());
    }

    #[test]
    fn x52_mfd_brightness_clamps() {
        let (_, _, wval, widx) = x52_mfd_set_brightness(200);
        assert_eq!(wval, 128);
        assert_eq!(widx, X52_MFD_BRIGHTNESS);
    }

    #[test]
    fn x52_led_brightness_cmd() {
        let (_, _, wval, widx) = x52_led_set_brightness(64);
        assert_eq!(wval, 64);
        assert_eq!(widx, X52_LED_BRIGHTNESS);
    }

    #[test]
    fn x52_clock_24h() {
        let (_, _, wval, widx) = x52_set_clock(14, 30, true);
        // wValue = 30 | (14 << 8) | (1 << 15)
        assert_eq!(wval, 30 | (14 << 8) | (1 << 15));
        assert_eq!(widx, X52_TIME_CLOCK1);
    }

    #[test]
    fn x52_date_cmd() {
        let (_, _, wval, widx) = x52_set_date(25, 12);
        assert_eq!(wval, 25 | (12 << 8));
        assert_eq!(widx, X52_DATE_DDMM);
    }

    #[test]
    fn x52_shift_on() {
        let (_, _, wval, widx) = x52_set_shift(true);
        assert_eq!(wval, 0x51);
        assert_eq!(widx, X52_SHIFT_INDICATOR);
    }

    #[test]
    fn x52_blink_off() {
        let (_, _, wval, widx) = x52_set_blink(false);
        assert_eq!(wval, 0x50);
        assert_eq!(widx, X52_BLINK_INDICATOR);
    }

    // ── Device table X52 v1 test ───────────────────────────────────────────

    #[test]
    fn device_table_has_x52_v1() {
        let found = DEVICE_TABLE
            .iter()
            .any(|d| d.vid == SAITEK_VID && d.pid == 0x0255 && d.id == DeviceId::X52);
        assert!(found, "X52 v1 (PID 0x0255) should be in DEVICE_TABLE");
    }
}
