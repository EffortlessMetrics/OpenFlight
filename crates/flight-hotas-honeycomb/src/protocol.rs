// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Honeycomb HID protocol details: magneto switch decoding, encoder delta
//! calculation, and gear indicator state management.
//!
//! # Magneto switch (Alpha Yoke)
//!
//! The Alpha Yoke's magneto/ignition switch is a 5-position rotary with
//! discrete button assignments in the HID report. Each position maps to a
//! unique combination of two consecutive button bits (magneto button A and B).
//!
//! # Encoder delta (Bravo Throttle)
//!
//! The Bravo reports rotary encoder CW/CCW as momentary button presses
//! (bits 12 and 13). This module provides stateful delta tracking that
//! converts pulse pairs into signed deltas, with wrapping support for
//! autopilot heading/altitude/VS/course knobs.
//!
//! # Gear indicator state
//!
//! The Bravo's landing gear lever has three logical states (up, down, transit)
//! that drive the three pairs of red/green LEDs via the feature report.

/// Magneto switch positions for the Alpha Yoke ignition switch.
///
/// The magneto switch is a 5-position rotary reported as combinations of
/// two button bits in the Alpha HID input report. Button assignments are
/// based on community documentation for GA trainer yokes.
///
/// Typical button encoding (buttons are 1-indexed):
/// - Off:   neither magneto button pressed
/// - Right: button 25 only
/// - Left:  button 26 only
/// - Both:  buttons 25 + 26
/// - Start: button 27 (momentary spring-return)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MagnetoPosition {
    /// Magneto off — no ignition.
    Off,
    /// Right magneto only.
    Right,
    /// Left magneto only.
    Left,
    /// Both magnetos active — normal operation.
    Both,
    /// Starter engaged (spring-return momentary).
    Start,
}

impl MagnetoPosition {
    /// Returns all positions in switch order (Off → Start).
    pub fn all() -> &'static [MagnetoPosition; 5] {
        &[
            MagnetoPosition::Off,
            MagnetoPosition::Right,
            MagnetoPosition::Left,
            MagnetoPosition::Both,
            MagnetoPosition::Start,
        ]
    }

    /// Human-readable label for the position.
    pub fn label(self) -> &'static str {
        match self {
            MagnetoPosition::Off => "OFF",
            MagnetoPosition::Right => "R",
            MagnetoPosition::Left => "L",
            MagnetoPosition::Both => "BOTH",
            MagnetoPosition::Start => "START",
        }
    }
}

impl std::fmt::Display for MagnetoPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Button numbers (1-indexed) used for magneto switch decoding.
pub const MAGNETO_BUTTON_A: u8 = 25;
/// Second magneto button (1-indexed).
pub const MAGNETO_BUTTON_B: u8 = 26;
/// Starter button (1-indexed, momentary spring-return).
pub const MAGNETO_BUTTON_START: u8 = 27;

/// Decode the magneto switch position from the Alpha Yoke button mask.
///
/// The button mask uses 0-indexed bits (bit 0 = button 1). Magneto buttons
/// 25, 26, and 27 correspond to bits 24, 25, and 26 respectively.
///
/// # Encoding
///
/// | Position | Btn 25 | Btn 26 | Btn 27 |
/// |----------|--------|--------|--------|
/// | Off      | 0      | 0      | 0      |
/// | Right    | 1      | 0      | 0      |
/// | Left     | 0      | 1      | 0      |
/// | Both     | 1      | 1      | 0      |
/// | Start    | ×      | ×      | 1      |
pub fn decode_magneto(button_mask: u64) -> MagnetoPosition {
    let btn_a = (button_mask >> (MAGNETO_BUTTON_A - 1)) & 1 != 0;
    let btn_b = (button_mask >> (MAGNETO_BUTTON_B - 1)) & 1 != 0;
    let btn_start = (button_mask >> (MAGNETO_BUTTON_START - 1)) & 1 != 0;

    if btn_start {
        MagnetoPosition::Start
    } else {
        match (btn_a, btn_b) {
            (false, false) => MagnetoPosition::Off,
            (true, false) => MagnetoPosition::Right,
            (false, true) => MagnetoPosition::Left,
            (true, true) => MagnetoPosition::Both,
        }
    }
}

// ── Encoder delta tracking ───────────────────────────────────────────────────

/// Button bit indices (0-indexed) for the Bravo encoder increment/decrement.
pub const ENCODER_CW_BIT: u8 = 12;
/// Encoder counter-clockwise button bit (0-indexed).
pub const ENCODER_CCW_BIT: u8 = 13;

/// Stateful encoder delta tracker for the Bravo Throttle rotary encoders.
///
/// The Bravo reports encoder rotation as momentary button presses: bit 12 for
/// CW (increment) and bit 13 for CCW (decrement). This tracker converts
/// edge-detected pulses into signed deltas.
///
/// # Usage
///
/// Call [`EncoderTracker::update`] with each new button mask from the Bravo
/// input report. The returned delta is +1 for CW, −1 for CCW, or 0 for no
/// change. Only rising edges (0→1 transitions) are counted.
#[derive(Debug, Clone, Default)]
pub struct EncoderTracker {
    prev_cw: bool,
    prev_ccw: bool,
}

impl EncoderTracker {
    /// Create a new encoder tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with the current button mask and return the encoder delta.
    ///
    /// Returns `+1` for a clockwise click, `−1` for counter-clockwise, or `0`
    /// if neither edge was detected.
    pub fn update(&mut self, button_mask: u64) -> i32 {
        let cw = (button_mask >> ENCODER_CW_BIT) & 1 != 0;
        let ccw = (button_mask >> ENCODER_CCW_BIT) & 1 != 0;

        let mut delta = 0i32;

        // Detect rising edges only
        if cw && !self.prev_cw {
            delta += 1;
        }
        if ccw && !self.prev_ccw {
            delta -= 1;
        }

        self.prev_cw = cw;
        self.prev_ccw = ccw;

        delta
    }

    /// Reset the tracker state (e.g., on reconnect).
    pub fn reset(&mut self) {
        self.prev_cw = false;
        self.prev_ccw = false;
    }
}

/// Accumulating encoder tracker with configurable wrapping for autopilot knobs.
///
/// Wraps accumulated value within `[min, max]` (inclusive). Useful for heading
/// (0–359), altitude (0–50000), vertical speed (−9999–+9999), and course
/// (0–359) knobs.
#[derive(Debug, Clone)]
pub struct WrappingEncoder {
    tracker: EncoderTracker,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    wrap: bool,
}

impl WrappingEncoder {
    /// Create a new wrapping encoder with the given range and step size.
    ///
    /// If `wrap` is `true`, the value wraps around (e.g., heading: 359 + 1 = 0).
    /// If `wrap` is `false`, the value clamps at the bounds.
    pub fn new(initial: i32, min: i32, max: i32, step: i32, wrap: bool) -> Self {
        Self {
            tracker: EncoderTracker::new(),
            value: initial.clamp(min, max),
            min,
            max,
            step,
            wrap,
        }
    }

    /// Create a heading encoder (0–359, step 1, wrapping).
    pub fn heading() -> Self {
        Self::new(0, 0, 359, 1, true)
    }

    /// Create an altitude encoder (100–50000, step 100, clamping).
    pub fn altitude() -> Self {
        Self::new(3000, 100, 50000, 100, false)
    }

    /// Create a vertical speed encoder (−9999–+9999, step 100, clamping).
    pub fn vertical_speed() -> Self {
        Self::new(0, -9999, 9999, 100, false)
    }

    /// Create a course encoder (0–359, step 1, wrapping).
    pub fn course() -> Self {
        Self::new(0, 0, 359, 1, true)
    }

    /// Update with the current button mask and return the new accumulated value.
    pub fn update(&mut self, button_mask: u64) -> i32 {
        let delta = self.tracker.update(button_mask);
        if delta != 0 {
            let new_val = self.value + delta * self.step;
            self.value = if self.wrap {
                let range = self.max - self.min + 1;
                ((new_val - self.min).rem_euclid(range)) + self.min
            } else {
                new_val.clamp(self.min, self.max)
            };
        }
        self.value
    }

    /// Returns the current accumulated value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Set the current value directly (e.g., to sync with sim state).
    pub fn set_value(&mut self, value: i32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// Reset the encoder tracker and set value to the given initial.
    pub fn reset(&mut self, initial: i32) {
        self.tracker.reset();
        self.value = initial.clamp(self.min, self.max);
    }
}

// ── Gear indicator state ─────────────────────────────────────────────────────

/// Landing gear indicator state derived from the Bravo gear lever buttons.
///
/// The Bravo reports gear lever position as two buttons: gear-up (bit 30) and
/// gear-down (bit 31). The transit state occurs when neither button is active
/// (lever in motion between detents).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GearIndicatorState {
    /// Gear fully retracted — gear-up button active, green LEDs off, red LEDs off.
    Up,
    /// Gear fully extended — gear-down button active, green LEDs on.
    Down,
    /// Gear in transit — neither gear button active, red LEDs on (unsafe).
    Transit,
}

impl GearIndicatorState {
    /// Derive gear state from the Bravo button mask.
    ///
    /// Bit 30 = gear up, bit 31 = gear down. If both are set simultaneously
    /// (should not occur in normal operation), gear-down takes priority.
    pub fn from_button_mask(mask: u64) -> Self {
        let gear_up = (mask >> 30) & 1 != 0;
        let gear_down = (mask >> 31) & 1 != 0;

        if gear_down {
            GearIndicatorState::Down
        } else if gear_up {
            GearIndicatorState::Up
        } else {
            GearIndicatorState::Transit
        }
    }

    /// Returns the LED configuration for this gear state.
    ///
    /// Returns `(green, red)` — set on the `BravoLedState` gear fields.
    pub fn led_colors(self) -> (bool, bool) {
        match self {
            GearIndicatorState::Up => (false, false),
            GearIndicatorState::Down => (true, false),
            GearIndicatorState::Transit => (false, true),
        }
    }
}

impl std::fmt::Display for GearIndicatorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GearIndicatorState::Up => f.write_str("UP"),
            GearIndicatorState::Down => f.write_str("DOWN"),
            GearIndicatorState::Transit => f.write_str("TRANSIT"),
        }
    }
}

/// Bravo toggle switch state for the 7 two-position toggle switches.
///
/// Each toggle switch reports as two buttons (UP and DOWN positions).
/// Button bit indices (0-indexed): switches start at bit 33.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToggleSwitchState {
    /// Switch is in the UP position.
    Up,
    /// Switch is in the DOWN position.
    Down,
    /// Switch is in the centre/neutral position (neither UP nor DOWN active).
    Center,
}

/// Decode a single toggle switch state from the Bravo button mask.
///
/// `switch_num` is 1-based (1–7). Each switch uses two consecutive bits
/// starting at bit 33 (switch 1 UP = bit 33, switch 1 DOWN = bit 34, etc.).
pub fn decode_toggle_switch(button_mask: u64, switch_num: u8) -> ToggleSwitchState {
    if !(1..=7).contains(&switch_num) {
        return ToggleSwitchState::Center;
    }
    let base_bit = 33 + (switch_num - 1) as u32 * 2;
    let up = (button_mask >> base_bit) & 1 != 0;
    let down = (button_mask >> (base_bit + 1)) & 1 != 0;

    match (up, down) {
        (true, false) => ToggleSwitchState::Up,
        (false, true) => ToggleSwitchState::Down,
        _ => ToggleSwitchState::Center,
    }
}

/// Decode all 7 toggle switch states from the Bravo button mask.
pub fn decode_all_toggle_switches(button_mask: u64) -> [ToggleSwitchState; 7] {
    [
        decode_toggle_switch(button_mask, 1),
        decode_toggle_switch(button_mask, 2),
        decode_toggle_switch(button_mask, 3),
        decode_toggle_switch(button_mask, 4),
        decode_toggle_switch(button_mask, 5),
        decode_toggle_switch(button_mask, 6),
        decode_toggle_switch(button_mask, 7),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Magneto tests ────────────────────────────────────────────────────

    #[test]
    fn test_magneto_off() {
        assert_eq!(decode_magneto(0), MagnetoPosition::Off);
    }

    #[test]
    fn test_magneto_right() {
        // Button 25 = bit 24
        let mask: u64 = 1 << 24;
        assert_eq!(decode_magneto(mask), MagnetoPosition::Right);
    }

    #[test]
    fn test_magneto_left() {
        // Button 26 = bit 25
        let mask: u64 = 1 << 25;
        assert_eq!(decode_magneto(mask), MagnetoPosition::Left);
    }

    #[test]
    fn test_magneto_both() {
        let mask: u64 = (1 << 24) | (1 << 25);
        assert_eq!(decode_magneto(mask), MagnetoPosition::Both);
    }

    #[test]
    fn test_magneto_start() {
        // Button 27 = bit 26; Start overrides A/B
        let mask: u64 = 1 << 26;
        assert_eq!(decode_magneto(mask), MagnetoPosition::Start);
    }

    #[test]
    fn test_magneto_start_overrides_both() {
        let mask: u64 = (1 << 24) | (1 << 25) | (1 << 26);
        assert_eq!(decode_magneto(mask), MagnetoPosition::Start);
    }

    #[test]
    fn test_magneto_all_positions() {
        let positions = MagnetoPosition::all();
        assert_eq!(positions.len(), 5);
        assert_eq!(positions[0], MagnetoPosition::Off);
        assert_eq!(positions[4], MagnetoPosition::Start);
    }

    #[test]
    fn test_magneto_labels() {
        assert_eq!(MagnetoPosition::Off.label(), "OFF");
        assert_eq!(MagnetoPosition::Right.label(), "R");
        assert_eq!(MagnetoPosition::Left.label(), "L");
        assert_eq!(MagnetoPosition::Both.label(), "BOTH");
        assert_eq!(MagnetoPosition::Start.label(), "START");
    }

    #[test]
    fn test_magneto_display() {
        assert_eq!(format!("{}", MagnetoPosition::Both), "BOTH");
    }

    #[test]
    fn test_magneto_unrelated_buttons_ignored() {
        // Only bits 24-26 matter; other buttons should not affect magneto
        let mask: u64 = 0xFF_FFFF; // bits 0–23 all set, 24–26 clear
        assert_eq!(decode_magneto(mask), MagnetoPosition::Off);
    }

    // ── Encoder tracker tests ────────────────────────────────────────────

    #[test]
    fn test_encoder_no_change() {
        let mut tracker = EncoderTracker::new();
        assert_eq!(tracker.update(0), 0);
        assert_eq!(tracker.update(0), 0);
    }

    #[test]
    fn test_encoder_cw_rising_edge() {
        let mut tracker = EncoderTracker::new();
        let cw_mask: u64 = 1 << ENCODER_CW_BIT;
        assert_eq!(tracker.update(cw_mask), 1); // rising edge
        assert_eq!(tracker.update(cw_mask), 0); // held, no new edge
        assert_eq!(tracker.update(0), 0); // released
        assert_eq!(tracker.update(cw_mask), 1); // rising edge again
    }

    #[test]
    fn test_encoder_ccw_rising_edge() {
        let mut tracker = EncoderTracker::new();
        let ccw_mask: u64 = 1 << ENCODER_CCW_BIT;
        assert_eq!(tracker.update(ccw_mask), -1);
        assert_eq!(tracker.update(ccw_mask), 0);
        assert_eq!(tracker.update(0), 0);
        assert_eq!(tracker.update(ccw_mask), -1);
    }

    #[test]
    fn test_encoder_both_simultaneously() {
        let mut tracker = EncoderTracker::new();
        let both: u64 = (1 << ENCODER_CW_BIT) | (1 << ENCODER_CCW_BIT);
        // Both edges at once cancel out
        assert_eq!(tracker.update(both), 0);
    }

    #[test]
    fn test_encoder_reset() {
        let mut tracker = EncoderTracker::new();
        let cw_mask: u64 = 1 << ENCODER_CW_BIT;
        tracker.update(cw_mask); // set prev_cw = true
        tracker.reset();
        assert_eq!(tracker.update(cw_mask), 1); // after reset, rising edge detected
    }

    #[test]
    fn test_encoder_multiple_pulses() {
        let mut tracker = EncoderTracker::new();
        let cw: u64 = 1 << ENCODER_CW_BIT;
        // Simulate 3 CW clicks
        assert_eq!(tracker.update(cw), 1);
        assert_eq!(tracker.update(0), 0);
        assert_eq!(tracker.update(cw), 1);
        assert_eq!(tracker.update(0), 0);
        assert_eq!(tracker.update(cw), 1);
    }

    #[test]
    fn test_encoder_alternating_direction() {
        let mut tracker = EncoderTracker::new();
        let cw: u64 = 1 << ENCODER_CW_BIT;
        let ccw: u64 = 1 << ENCODER_CCW_BIT;
        assert_eq!(tracker.update(cw), 1);
        assert_eq!(tracker.update(0), 0);
        assert_eq!(tracker.update(ccw), -1);
        assert_eq!(tracker.update(0), 0);
        assert_eq!(tracker.update(cw), 1);
    }

    // ── Wrapping encoder tests ───────────────────────────────────────────

    #[test]
    fn test_heading_encoder_wraps_forward() {
        let mut enc = WrappingEncoder::heading();
        enc.set_value(359);
        let cw: u64 = 1 << ENCODER_CW_BIT;
        let val = enc.update(cw);
        assert_eq!(val, 0, "heading should wrap 359 → 0");
    }

    #[test]
    fn test_heading_encoder_wraps_backward() {
        let mut enc = WrappingEncoder::heading();
        enc.set_value(0);
        let ccw: u64 = 1 << ENCODER_CCW_BIT;
        let val = enc.update(ccw);
        assert_eq!(val, 359, "heading should wrap 0 → 359");
    }

    #[test]
    fn test_altitude_encoder_clamps_at_min() {
        let mut enc = WrappingEncoder::altitude();
        enc.set_value(100);
        let ccw: u64 = 1 << ENCODER_CCW_BIT;
        let val = enc.update(ccw);
        assert_eq!(val, 100, "altitude should clamp at 100");
    }

    #[test]
    fn test_altitude_encoder_clamps_at_max() {
        let mut enc = WrappingEncoder::altitude();
        enc.set_value(50000);
        let cw: u64 = 1 << ENCODER_CW_BIT;
        let val = enc.update(cw);
        assert_eq!(val, 50000, "altitude should clamp at 50000");
    }

    #[test]
    fn test_vs_encoder_crosses_zero() {
        let mut enc = WrappingEncoder::vertical_speed();
        enc.set_value(0);
        let ccw: u64 = 1 << ENCODER_CCW_BIT;
        let val = enc.update(ccw);
        assert_eq!(val, -100, "VS should go negative from 0");
    }

    #[test]
    fn test_course_encoder_wraps() {
        let mut enc = WrappingEncoder::course();
        enc.set_value(0);
        let ccw: u64 = 1 << ENCODER_CCW_BIT;
        let val = enc.update(ccw);
        assert_eq!(val, 359, "course should wrap 0 → 359");
    }

    #[test]
    fn test_wrapping_encoder_set_value_clamps() {
        let mut enc = WrappingEncoder::heading();
        enc.set_value(999);
        assert_eq!(enc.value(), 359);
        enc.set_value(-5);
        assert_eq!(enc.value(), 0);
    }

    #[test]
    fn test_wrapping_encoder_reset() {
        let mut enc = WrappingEncoder::heading();
        enc.set_value(180);
        enc.reset(90);
        assert_eq!(enc.value(), 90);
    }

    #[test]
    fn test_wrapping_encoder_no_change_without_edge() {
        let mut enc = WrappingEncoder::heading();
        enc.set_value(180);
        assert_eq!(enc.update(0), 180);
        assert_eq!(enc.update(0), 180);
    }

    // ── Gear indicator state tests ───────────────────────────────────────

    #[test]
    fn test_gear_up_state() {
        let mask: u64 = 1 << 30;
        assert_eq!(GearIndicatorState::from_button_mask(mask), GearIndicatorState::Up);
    }

    #[test]
    fn test_gear_down_state() {
        let mask: u64 = 1 << 31;
        assert_eq!(
            GearIndicatorState::from_button_mask(mask),
            GearIndicatorState::Down
        );
    }

    #[test]
    fn test_gear_transit_state() {
        assert_eq!(
            GearIndicatorState::from_button_mask(0),
            GearIndicatorState::Transit
        );
    }

    #[test]
    fn test_gear_down_priority_when_both_set() {
        let mask: u64 = (1 << 30) | (1 << 31);
        assert_eq!(
            GearIndicatorState::from_button_mask(mask),
            GearIndicatorState::Down
        );
    }

    #[test]
    fn test_gear_led_colors() {
        assert_eq!(GearIndicatorState::Up.led_colors(), (false, false));
        assert_eq!(GearIndicatorState::Down.led_colors(), (true, false));
        assert_eq!(GearIndicatorState::Transit.led_colors(), (false, true));
    }

    #[test]
    fn test_gear_display() {
        assert_eq!(format!("{}", GearIndicatorState::Up), "UP");
        assert_eq!(format!("{}", GearIndicatorState::Down), "DOWN");
        assert_eq!(format!("{}", GearIndicatorState::Transit), "TRANSIT");
    }

    #[test]
    fn test_gear_unrelated_bits_ignored() {
        // Bits other than 30/31 should not affect gear state
        let mask: u64 = 0x3FFF_FFFF; // bits 0–29 set, 30–31 clear
        assert_eq!(
            GearIndicatorState::from_button_mask(mask),
            GearIndicatorState::Transit
        );
    }

    // ── Toggle switch tests ──────────────────────────────────────────────

    #[test]
    fn test_toggle_switch_center() {
        assert_eq!(decode_toggle_switch(0, 1), ToggleSwitchState::Center);
    }

    #[test]
    fn test_toggle_switch_1_up() {
        // Switch 1 UP = bit 33
        let mask: u64 = 1 << 33;
        assert_eq!(decode_toggle_switch(mask, 1), ToggleSwitchState::Up);
    }

    #[test]
    fn test_toggle_switch_1_down() {
        // Switch 1 DOWN = bit 34
        let mask: u64 = 1 << 34;
        assert_eq!(decode_toggle_switch(mask, 1), ToggleSwitchState::Down);
    }

    #[test]
    fn test_toggle_switch_7_up() {
        // Switch 7 UP = bit 33 + (7-1)*2 = bit 45
        let mask: u64 = 1 << 45;
        assert_eq!(decode_toggle_switch(mask, 7), ToggleSwitchState::Up);
    }

    #[test]
    fn test_toggle_switch_7_down() {
        // Switch 7 DOWN = bit 46
        let mask: u64 = 1 << 46;
        assert_eq!(decode_toggle_switch(mask, 7), ToggleSwitchState::Down);
    }

    #[test]
    fn test_toggle_switch_invalid_number() {
        assert_eq!(decode_toggle_switch(u64::MAX, 0), ToggleSwitchState::Center);
        assert_eq!(decode_toggle_switch(u64::MAX, 8), ToggleSwitchState::Center);
    }

    #[test]
    fn test_decode_all_toggle_switches_all_center() {
        let switches = decode_all_toggle_switches(0);
        for s in &switches {
            assert_eq!(*s, ToggleSwitchState::Center);
        }
    }

    #[test]
    fn test_decode_all_toggle_switches_mixed() {
        // Switch 1 UP (bit 33) + Switch 3 DOWN (bit 38)
        let mask: u64 = (1 << 33) | (1 << 38);
        let switches = decode_all_toggle_switches(mask);
        assert_eq!(switches[0], ToggleSwitchState::Up);
        assert_eq!(switches[1], ToggleSwitchState::Center);
        assert_eq!(switches[2], ToggleSwitchState::Down);
        for i in 3..7 {
            assert_eq!(switches[i], ToggleSwitchState::Center);
        }
    }

    #[test]
    fn test_toggle_switch_both_up_down_returns_center() {
        // Both UP and DOWN set simultaneously → Center (invalid state)
        let mask: u64 = (1 << 33) | (1 << 34);
        assert_eq!(decode_toggle_switch(mask, 1), ToggleSwitchState::Center);
    }
}
