// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! USB HID protocol details for Turtle Beach VelocityOne devices.
//!
//! # VelocityOne Flight (Flightdeck) input report
//!
//! The Flight is a combined yoke + throttle quadrant with integrated panel.
//! It exposes a single HID joystick interface with the following estimated
//! report layout:
//!
//! ```text
//! Bytes 0–1:   Roll (u16 LE, 12-bit, centre ≈ 2048)
//! Bytes 2–3:   Pitch (u16 LE, 12-bit, centre ≈ 2048)
//! Bytes 4–5:   Rudder twist (u16 LE, 12-bit, centre ≈ 2048)
//! Byte  6:     Throttle left (u8, 0 = idle, 255 = full)
//! Byte  7:     Throttle right (u8, 0 = idle, 255 = full)
//! Bytes 8–9:   Trim wheel (u16 LE, 12-bit, centre ≈ 2048)
//! Bytes 10–17: Button bitmask (u64 LE)
//! Byte  18:    Hat switch (lower nibble, 0–8)
//! Byte  19:    Toggle switches bitmask (bits 0–6 = switches 1–7)
//! ```
//!
//! # VelocityOne Flightstick input report
//!
//! ```text
//! Bytes 0–1:   X axis (u16 LE, 12-bit, centre ≈ 2048)
//! Bytes 2–3:   Y axis (u16 LE, 12-bit, centre ≈ 2048)
//! Bytes 4–5:   Twist / Z axis (u16 LE, 12-bit, centre ≈ 2048)
//! Byte  6:     Throttle slider (u8, 0 = idle, 255 = full)
//! Bytes 7–8:   Button bitmask (u16 LE)
//! Byte  9:     Hat switch (lower nibble, 0–8)
//! Bytes 10–11: Reserved
//! ```
//!
//! # LED output
//!
//! The VelocityOne Flight gear lever has three pairs of green/red LEDs
//! (nose, left, right). LED state is sent via a 4-byte HID feature report.
//!
//! # Display commands
//!
//! The VelocityOne Flight's multi-function display supports page selection
//! and brightness control via vendor-specific output reports.
//!
//! **All report layouts are estimated and require hardware validation.**

use crate::velocityone::TurtleBeachError;

// ── VelocityOne Flight input report ──────────────────────────────────────────

/// Minimum report length for a VelocityOne Flight HID input report.
pub const FLIGHT_MIN_REPORT_BYTES: usize = 20;

/// Parsed input state from a VelocityOne Flight HID report.
#[derive(Debug, Clone)]
pub struct VelocityOneFlightReport {
    /// Roll axis. −1.0 = full left, 1.0 = full right.
    pub roll: f32,
    /// Pitch axis. −1.0 = full forward, 1.0 = full back.
    pub pitch: f32,
    /// Rudder twist axis. −1.0 = full left, 1.0 = full right.
    pub rudder_twist: f32,
    /// Left throttle lever. 0.0 = idle, 1.0 = full.
    pub throttle_left: f32,
    /// Right throttle lever. 0.0 = idle, 1.0 = full.
    pub throttle_right: f32,
    /// Trim wheel position. −1.0 = full nose-down, 1.0 = full nose-up.
    pub trim_wheel: f32,
    /// Button bitmask (bit 0 = button 1, little-endian).
    pub buttons: u64,
    /// Hat switch position: 0 = centred, 1–8 = N/NE/E/SE/S/SW/W/NW.
    pub hat: u8,
    /// Toggle switch bitmask (bit 0 = switch 1, up to 7 switches).
    pub toggle_switches: u8,
}

/// Parse a VelocityOne Flight HID input report.
///
/// # Errors
///
/// Returns [`TurtleBeachError::TooShort`] if `data` has fewer than
/// [`FLIGHT_MIN_REPORT_BYTES`] bytes.
pub fn parse_flight_report(data: &[u8]) -> Result<VelocityOneFlightReport, TurtleBeachError> {
    if data.len() < FLIGHT_MIN_REPORT_BYTES {
        return Err(TurtleBeachError::TooShort {
            expected: FLIGHT_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let roll_raw = u16::from_le_bytes([data[0], data[1]]);
    let pitch_raw = u16::from_le_bytes([data[2], data[3]]);
    let rudder_raw = u16::from_le_bytes([data[4], data[5]]);
    let throttle_left_raw = data[6];
    let throttle_right_raw = data[7];
    let trim_raw = u16::from_le_bytes([data[8], data[9]]);

    let buttons = u64::from_le_bytes([
        data[10], data[11], data[12], data[13], data[14], data[15], data[16], data[17],
    ]);

    let hat = hat_raw_to_8way(data[18] & 0x0F);
    let toggle_switches = data[19] & 0x7F;

    Ok(VelocityOneFlightReport {
        roll: normalize_12bit_bipolar(roll_raw),
        pitch: normalize_12bit_bipolar(pitch_raw),
        rudder_twist: normalize_12bit_bipolar(rudder_raw),
        throttle_left: normalize_u8_unipolar(throttle_left_raw),
        throttle_right: normalize_u8_unipolar(throttle_right_raw),
        trim_wheel: normalize_12bit_bipolar(trim_raw),
        buttons,
        hat,
        toggle_switches,
    })
}

// ── VelocityOne Flightstick input report ─────────────────────────────────────

/// Minimum report length for a VelocityOne Flightstick HID input report.
pub const FLIGHTSTICK_MIN_REPORT_BYTES: usize = 12;

/// Parsed input state from a VelocityOne Flightstick HID report.
#[derive(Debug, Clone)]
pub struct VelocityOneFlightstickReport {
    /// X axis (roll). −1.0 = full left, 1.0 = full right.
    pub x: f32,
    /// Y axis (pitch). −1.0 = full forward, 1.0 = full back.
    pub y: f32,
    /// Twist axis (rudder). −1.0 = full left, 1.0 = full right.
    pub twist: f32,
    /// Throttle slider. 0.0 = idle, 1.0 = full.
    pub throttle: f32,
    /// Button bitmask (bit 0 = button 1).
    pub buttons: u16,
    /// Hat switch position: 0 = centred, 1–8 = N/NE/E/SE/S/SW/W/NW.
    pub hat: u8,
}

/// Parse a VelocityOne Flightstick HID input report.
///
/// # Errors
///
/// Returns [`TurtleBeachError::TooShort`] if `data` has fewer than
/// [`FLIGHTSTICK_MIN_REPORT_BYTES`] bytes.
pub fn parse_flightstick_report(
    data: &[u8],
) -> Result<VelocityOneFlightstickReport, TurtleBeachError> {
    if data.len() < FLIGHTSTICK_MIN_REPORT_BYTES {
        return Err(TurtleBeachError::TooShort {
            expected: FLIGHTSTICK_MIN_REPORT_BYTES,
            actual: data.len(),
        });
    }

    let x_raw = u16::from_le_bytes([data[0], data[1]]);
    let y_raw = u16::from_le_bytes([data[2], data[3]]);
    let twist_raw = u16::from_le_bytes([data[4], data[5]]);
    let throttle_raw = data[6];
    let buttons = u16::from_le_bytes([data[7], data[8]]);
    let hat = hat_raw_to_8way(data[9] & 0x0F);

    Ok(VelocityOneFlightstickReport {
        x: normalize_12bit_bipolar(x_raw),
        y: normalize_12bit_bipolar(y_raw),
        twist: normalize_12bit_bipolar(twist_raw),
        throttle: normalize_u8_unipolar(throttle_raw),
        buttons,
        hat,
    })
}

// ── Gear indicator LED control ───────────────────────────────────────────────

/// Landing gear indicator state for the VelocityOne Flight gear lever.
///
/// The gear lever reports discrete button positions. Gear LED state is derived
/// from the button mask and driven back to the hardware via
/// [`serialize_gear_led_report`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GearLedState {
    /// Gear retracted — LEDs off.
    Up,
    /// Gear extended and locked — green LEDs on.
    Down,
    /// Gear in transit — red LEDs on (unsafe).
    Transit,
}

impl GearLedState {
    /// Derive gear state from the VelocityOne Flight button mask.
    ///
    /// Bit 30 = gear-up, bit 31 = gear-down. If both set, gear-down wins.
    pub fn from_button_mask(mask: u64) -> Self {
        let gear_up = (mask >> 30) & 1 != 0;
        let gear_down = (mask >> 31) & 1 != 0;

        if gear_down {
            GearLedState::Down
        } else if gear_up {
            GearLedState::Up
        } else {
            GearLedState::Transit
        }
    }

    /// Returns `(green, red)` LED states for this gear position.
    pub fn led_colors(self) -> (bool, bool) {
        match self {
            GearLedState::Up => (false, false),
            GearLedState::Down => (true, false),
            GearLedState::Transit => (false, true),
        }
    }
}

impl std::fmt::Display for GearLedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GearLedState::Up => f.write_str("UP"),
            GearLedState::Down => f.write_str("DOWN"),
            GearLedState::Transit => f.write_str("TRANSIT"),
        }
    }
}

/// LED state for the VelocityOne Flight gear indicator panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlightLedState {
    /// Nose gear green (down-and-locked).
    pub gear_nose_green: bool,
    /// Nose gear red (in transit / unsafe).
    pub gear_nose_red: bool,
    /// Left main gear green.
    pub gear_left_green: bool,
    /// Left main gear red.
    pub gear_left_red: bool,
    /// Right main gear green.
    pub gear_right_green: bool,
    /// Right main gear red.
    pub gear_right_red: bool,
}

impl FlightLedState {
    /// All LEDs off.
    pub fn all_off() -> Self {
        Self::default()
    }

    /// All LEDs on (lamp test).
    pub fn all_on() -> Self {
        Self {
            gear_nose_green: true,
            gear_nose_red: true,
            gear_left_green: true,
            gear_left_red: true,
            gear_right_green: true,
            gear_right_red: true,
        }
    }

    /// Set all three gear indicators from a [`GearLedState`].
    pub fn set_from_gear_state(&mut self, state: GearLedState) {
        let (green, red) = state.led_colors();
        self.gear_nose_green = green;
        self.gear_nose_red = red;
        self.gear_left_green = green;
        self.gear_left_red = red;
        self.gear_right_green = green;
        self.gear_right_red = red;
    }
}

/// Serialise a [`FlightLedState`] into a 4-byte HID feature report.
///
/// # Layout
///
/// | Byte | Content |
/// |------|---------|
/// | 0    | Report ID = 0x00 |
/// | 1    | Gear LEDs (bits 0–5: nose_g, nose_r, left_g, left_r, right_g, right_r) |
/// | 2    | Reserved |
/// | 3    | Reserved |
pub fn serialize_gear_led_report(leds: &FlightLedState) -> [u8; 4] {
    let mut data = [0u8; 4];
    data[1] |= leds.gear_nose_green as u8;
    data[1] |= (leds.gear_nose_red as u8) << 1;
    data[1] |= (leds.gear_left_green as u8) << 2;
    data[1] |= (leds.gear_left_red as u8) << 3;
    data[1] |= (leds.gear_right_green as u8) << 4;
    data[1] |= (leds.gear_right_red as u8) << 5;
    data
}

// ── Display commands ─────────────────────────────────────────────────────────

/// Display pages on the VelocityOne Flight multi-function display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayPage {
    /// Navigation instruments (heading, course, etc.).
    Nav = 0,
    /// Engine instruments (RPM, EGT, fuel flow).
    Engine = 1,
    /// Systems status (electrical, hydraulic, fuel).
    Systems = 2,
    /// User-customisable page.
    Custom = 3,
}

/// A command to update the VelocityOne Flight display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayCommand {
    /// Target page.
    pub page: DisplayPage,
    /// Brightness level (0 = off, 255 = maximum).
    pub brightness: u8,
}

/// Serialise a [`DisplayCommand`] into an 8-byte vendor-specific output report.
///
/// # Layout
///
/// | Byte | Content |
/// |------|---------|
/// | 0    | Report ID = 0x02 (vendor-specific) |
/// | 1    | Command: 0x01 = page + brightness |
/// | 2    | Page number (0–3) |
/// | 3    | Brightness (0–255) |
/// | 4–7  | Reserved (zero) |
pub fn serialize_display_command(cmd: &DisplayCommand) -> [u8; 8] {
    let mut data = [0u8; 8];
    data[0] = 0x02;
    data[1] = 0x01;
    data[2] = cmd.page as u8;
    data[3] = cmd.brightness;
    data
}

// ── Toggle switch decoding ───────────────────────────────────────────────────

/// Toggle switch state (simple on/off).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToggleSwitchPosition {
    /// Switch is off.
    Off,
    /// Switch is on.
    On,
}

/// Decode a single toggle switch from the toggle switch bitmask.
///
/// `switch_num` is 1-based (1–7). Returns [`ToggleSwitchPosition::Off`] for
/// out-of-range switch numbers.
pub fn decode_toggle_switch(toggle_mask: u8, switch_num: u8) -> ToggleSwitchPosition {
    if !(1..=7).contains(&switch_num) {
        return ToggleSwitchPosition::Off;
    }
    if (toggle_mask >> (switch_num - 1)) & 1 != 0 {
        ToggleSwitchPosition::On
    } else {
        ToggleSwitchPosition::Off
    }
}

/// Decode all 7 toggle switches at once.
pub fn decode_all_toggles(toggle_mask: u8) -> [ToggleSwitchPosition; 7] {
    [
        decode_toggle_switch(toggle_mask, 1),
        decode_toggle_switch(toggle_mask, 2),
        decode_toggle_switch(toggle_mask, 3),
        decode_toggle_switch(toggle_mask, 4),
        decode_toggle_switch(toggle_mask, 5),
        decode_toggle_switch(toggle_mask, 6),
        decode_toggle_switch(toggle_mask, 7),
    ]
}

// ── Trim wheel tracking ─────────────────────────────────────────────────────

/// Stateful trim wheel delta tracker.
///
/// The VelocityOne Flight trim wheel is reported as a 12-bit position value.
/// This tracker converts position changes into signed deltas for the profile
/// pipeline.
#[derive(Debug, Clone)]
pub struct TrimWheelTracker {
    prev: Option<u16>,
}

impl Default for TrimWheelTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TrimWheelTracker {
    /// Create a new trim wheel tracker.
    pub fn new() -> Self {
        Self { prev: None }
    }

    /// Update with the current raw trim wheel value and return the signed delta.
    ///
    /// First call returns 0 (establishing baseline). Subsequent calls return
    /// the difference from the previous value.
    pub fn update(&mut self, raw: u16) -> i32 {
        let clamped = raw.min(4095);
        let delta = match self.prev {
            Some(prev) => clamped as i32 - prev as i32,
            None => 0,
        };
        self.prev = Some(clamped);
        delta
    }

    /// Reset the tracker (e.g., on reconnect).
    pub fn reset(&mut self) {
        self.prev = None;
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Normalise a 12-bit unsigned axis value to −1.0..=1.0 (centred at 2048).
#[inline]
fn normalize_12bit_bipolar(raw: u16) -> f32 {
    let raw = raw.min(4095);
    ((raw as f32 - 2048.0) / 2048.0).clamp(-1.0, 1.0)
}

/// Normalise an 8-bit unsigned value to 0.0..=1.0.
#[inline]
fn normalize_u8_unipolar(raw: u8) -> f32 {
    (raw as f32 / 255.0).clamp(0.0, 1.0)
}

/// Convert a 4-bit hat raw value (0–15) to an 8-way direction (0 = centred).
fn hat_raw_to_8way(raw: u8) -> u8 {
    match raw {
        0 => 1, // N
        1 => 2, // NE
        2 => 3, // E
        3 => 4, // SE
        4 => 5, // S
        5 => 6, // SW
        6 => 7, // W
        7 => 8, // NW
        _ => 0, // centred (includes 8–15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── Flight report helpers ────────────────────────────────────────────

    fn make_flight_report(
        roll: u16,
        pitch: u16,
        rudder: u16,
        tl: u8,
        tr: u8,
        trim: u16,
        buttons: u64,
        hat: u8,
        toggles: u8,
    ) -> [u8; 20] {
        let mut b = [0u8; 20];
        b[0..2].copy_from_slice(&roll.to_le_bytes());
        b[2..4].copy_from_slice(&pitch.to_le_bytes());
        b[4..6].copy_from_slice(&rudder.to_le_bytes());
        b[6] = tl;
        b[7] = tr;
        b[8..10].copy_from_slice(&trim.to_le_bytes());
        b[10..18].copy_from_slice(&buttons.to_le_bytes());
        b[18] = hat;
        b[19] = toggles;
        b
    }

    fn make_flightstick_report(
        x: u16,
        y: u16,
        twist: u16,
        throttle: u8,
        buttons: u16,
        hat: u8,
    ) -> [u8; 12] {
        let mut b = [0u8; 12];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&twist.to_le_bytes());
        b[6] = throttle;
        b[7..9].copy_from_slice(&buttons.to_le_bytes());
        b[9] = hat;
        b
    }

    // ── Flight report tests ──────────────────────────────────────────────

    #[test]
    fn test_flight_report_center() {
        let data = make_flight_report(2048, 2048, 2048, 0, 0, 2048, 0, 15, 0);
        let r = parse_flight_report(&data).unwrap();
        assert!(r.roll.abs() < 0.001);
        assert!(r.pitch.abs() < 0.001);
        assert!(r.rudder_twist.abs() < 0.001);
        assert!(r.throttle_left < 0.001);
        assert!(r.throttle_right < 0.001);
        assert!(r.trim_wheel.abs() < 0.001);
        assert_eq!(r.buttons, 0);
        assert_eq!(r.hat, 0); // centred
        assert_eq!(r.toggle_switches, 0);
    }

    #[test]
    fn test_flight_report_full_roll_right() {
        let data = make_flight_report(4095, 2048, 2048, 0, 0, 2048, 0, 15, 0);
        let r = parse_flight_report(&data).unwrap();
        assert!(
            r.roll > 0.99,
            "full right roll should be ~1.0, got {}",
            r.roll
        );
    }

    #[test]
    fn test_flight_report_full_roll_left() {
        let data = make_flight_report(0, 2048, 2048, 0, 0, 2048, 0, 15, 0);
        let r = parse_flight_report(&data).unwrap();
        assert!(
            r.roll < -0.99,
            "full left roll should be ~-1.0, got {}",
            r.roll
        );
    }

    #[test]
    fn test_flight_report_throttle_full() {
        let data = make_flight_report(2048, 2048, 2048, 255, 255, 2048, 0, 15, 0);
        let r = parse_flight_report(&data).unwrap();
        assert!(r.throttle_left > 0.999);
        assert!(r.throttle_right > 0.999);
    }

    #[test]
    fn test_flight_report_rudder_twist() {
        let data = make_flight_report(2048, 2048, 4095, 0, 0, 2048, 0, 15, 0);
        let r = parse_flight_report(&data).unwrap();
        assert!(
            r.rudder_twist > 0.99,
            "full right rudder twist should be ~1.0"
        );
    }

    #[test]
    fn test_flight_report_toggles() {
        let data = make_flight_report(2048, 2048, 2048, 0, 0, 2048, 0, 15, 0b0101_0101);
        let r = parse_flight_report(&data).unwrap();
        assert_eq!(r.toggle_switches, 0b0101_0101);
    }

    #[test]
    fn test_flight_report_hat_north() {
        let data = make_flight_report(2048, 2048, 2048, 0, 0, 2048, 0, 0, 0);
        let r = parse_flight_report(&data).unwrap();
        assert_eq!(r.hat, 1, "hat 0 (raw) should map to 1 (N)");
    }

    #[test]
    fn test_flight_report_too_short() {
        assert!(parse_flight_report(&[0u8; 19]).is_err());
        assert!(parse_flight_report(&[]).is_err());
    }

    // ── Flightstick report tests ─────────────────────────────────────────

    #[test]
    fn test_flightstick_center() {
        let data = make_flightstick_report(2048, 2048, 2048, 0, 0, 15);
        let r = parse_flightstick_report(&data).unwrap();
        assert!(r.x.abs() < 0.001);
        assert!(r.y.abs() < 0.001);
        assert!(r.twist.abs() < 0.001);
        assert!(r.throttle < 0.001);
        assert_eq!(r.buttons, 0);
        assert_eq!(r.hat, 0);
    }

    #[test]
    fn test_flightstick_full_deflection() {
        let data = make_flightstick_report(4095, 0, 4095, 255, 0, 15);
        let r = parse_flightstick_report(&data).unwrap();
        assert!(r.x > 0.99, "full right X should be ~1.0");
        assert!(r.y < -0.99, "full forward Y should be ~-1.0");
        assert!(r.twist > 0.99, "full right twist should be ~1.0");
        assert!(r.throttle > 0.999, "full throttle should be ~1.0");
    }

    #[test]
    fn test_flightstick_buttons() {
        let data = make_flightstick_report(2048, 2048, 2048, 0, 0xA5A5, 15);
        let r = parse_flightstick_report(&data).unwrap();
        assert_eq!(r.buttons, 0xA5A5);
    }

    #[test]
    fn test_flightstick_too_short() {
        assert!(parse_flightstick_report(&[0u8; 11]).is_err());
        assert!(parse_flightstick_report(&[]).is_err());
    }

    // ── Gear LED tests ───────────────────────────────────────────────────

    #[test]
    fn test_gear_state_from_button_mask_down() {
        let mask: u64 = 1 << 31;
        assert_eq!(GearLedState::from_button_mask(mask), GearLedState::Down);
    }

    #[test]
    fn test_gear_state_from_button_mask_up() {
        let mask: u64 = 1 << 30;
        assert_eq!(GearLedState::from_button_mask(mask), GearLedState::Up);
    }

    #[test]
    fn test_gear_state_from_button_mask_transit() {
        assert_eq!(GearLedState::from_button_mask(0), GearLedState::Transit);
    }

    #[test]
    fn test_gear_down_priority_when_both_set() {
        let mask: u64 = (1 << 30) | (1 << 31);
        assert_eq!(GearLedState::from_button_mask(mask), GearLedState::Down);
    }

    #[test]
    fn test_gear_led_colors() {
        assert_eq!(GearLedState::Up.led_colors(), (false, false));
        assert_eq!(GearLedState::Down.led_colors(), (true, false));
        assert_eq!(GearLedState::Transit.led_colors(), (false, true));
    }

    #[test]
    fn test_gear_led_display() {
        assert_eq!(format!("{}", GearLedState::Up), "UP");
        assert_eq!(format!("{}", GearLedState::Down), "DOWN");
        assert_eq!(format!("{}", GearLedState::Transit), "TRANSIT");
    }

    #[test]
    fn test_gear_led_report_all_off() {
        let report = serialize_gear_led_report(&FlightLedState::all_off());
        assert_eq!(report, [0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_gear_led_report_all_green() {
        let mut leds = FlightLedState::all_off();
        leds.set_from_gear_state(GearLedState::Down);
        let report = serialize_gear_led_report(&leds);
        // Green bits: 0, 2, 4
        assert_eq!(report[1], 0b0001_0101);
    }

    #[test]
    fn test_gear_led_report_all_red() {
        let mut leds = FlightLedState::all_off();
        leds.set_from_gear_state(GearLedState::Transit);
        let report = serialize_gear_led_report(&leds);
        // Red bits: 1, 3, 5
        assert_eq!(report[1], 0b0010_1010);
    }

    #[test]
    fn test_gear_led_report_all_on() {
        let report = serialize_gear_led_report(&FlightLedState::all_on());
        assert_eq!(report[0], 0x00); // report ID
        assert_eq!(report[1], 0b0011_1111); // all 6 LED bits
    }

    // ── Display command tests ────────────────────────────────────────────

    #[test]
    fn test_display_command_nav_page() {
        let cmd = DisplayCommand {
            page: DisplayPage::Nav,
            brightness: 200,
        };
        let report = serialize_display_command(&cmd);
        assert_eq!(report[0], 0x02); // report ID
        assert_eq!(report[1], 0x01); // command type
        assert_eq!(report[2], 0); // nav page
        assert_eq!(report[3], 200); // brightness
    }

    #[test]
    fn test_display_command_custom_page() {
        let cmd = DisplayCommand {
            page: DisplayPage::Custom,
            brightness: 0,
        };
        let report = serialize_display_command(&cmd);
        assert_eq!(report[2], 3); // custom page
        assert_eq!(report[3], 0); // brightness off
    }

    // ── Toggle switch tests ──────────────────────────────────────────────

    #[test]
    fn test_toggle_all_off() {
        let toggles = decode_all_toggles(0);
        for t in &toggles {
            assert_eq!(*t, ToggleSwitchPosition::Off);
        }
    }

    #[test]
    fn test_toggle_all_on() {
        let toggles = decode_all_toggles(0x7F);
        for t in &toggles {
            assert_eq!(*t, ToggleSwitchPosition::On);
        }
    }

    #[test]
    fn test_toggle_individual() {
        assert_eq!(
            decode_toggle_switch(0b0000_0001, 1),
            ToggleSwitchPosition::On
        );
        assert_eq!(
            decode_toggle_switch(0b0000_0001, 2),
            ToggleSwitchPosition::Off
        );
        assert_eq!(
            decode_toggle_switch(0b0100_0000, 7),
            ToggleSwitchPosition::On
        );
    }

    #[test]
    fn test_toggle_out_of_range() {
        assert_eq!(decode_toggle_switch(0xFF, 0), ToggleSwitchPosition::Off);
        assert_eq!(decode_toggle_switch(0xFF, 8), ToggleSwitchPosition::Off);
    }

    // ── Trim wheel tracker tests ─────────────────────────────────────────

    #[test]
    fn test_trim_wheel_first_update_zero_delta() {
        let mut tracker = TrimWheelTracker::new();
        assert_eq!(tracker.update(2048), 0);
    }

    #[test]
    fn test_trim_wheel_positive_delta() {
        let mut tracker = TrimWheelTracker::new();
        tracker.update(2048);
        assert_eq!(tracker.update(2058), 10);
    }

    #[test]
    fn test_trim_wheel_negative_delta() {
        let mut tracker = TrimWheelTracker::new();
        tracker.update(2048);
        assert_eq!(tracker.update(2038), -10);
    }

    #[test]
    fn test_trim_wheel_reset() {
        let mut tracker = TrimWheelTracker::new();
        tracker.update(2048);
        tracker.reset();
        assert_eq!(tracker.update(1000), 0); // first after reset
    }

    #[test]
    fn test_trim_wheel_clamps_at_4095() {
        let mut tracker = TrimWheelTracker::new();
        tracker.update(4000);
        // Value above 4095 is clamped
        assert_eq!(tracker.update(5000), 4095 - 4000);
    }

    // ── Proptest ─────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn test_flight_report_axes_bounded(
            bytes in proptest::collection::vec(proptest::num::u8::ANY, 20..=32)
        ) {
            let r = parse_flight_report(&bytes).unwrap();
            prop_assert!((-1.0..=1.0).contains(&r.roll));
            prop_assert!((-1.0..=1.0).contains(&r.pitch));
            prop_assert!((-1.0..=1.0).contains(&r.rudder_twist));
            prop_assert!((0.0..=1.0).contains(&r.throttle_left));
            prop_assert!((0.0..=1.0).contains(&r.throttle_right));
            prop_assert!((-1.0..=1.0).contains(&r.trim_wheel));
        }

        #[test]
        fn test_flightstick_report_axes_bounded(
            bytes in proptest::collection::vec(proptest::num::u8::ANY, 12..=20)
        ) {
            let r = parse_flightstick_report(&bytes).unwrap();
            prop_assert!((-1.0..=1.0).contains(&r.x));
            prop_assert!((-1.0..=1.0).contains(&r.y));
            prop_assert!((-1.0..=1.0).contains(&r.twist));
            prop_assert!((0.0..=1.0).contains(&r.throttle));
        }
    }
}
