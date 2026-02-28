// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek Pro Flight Switch Panel — toggle switches, magneto, and gear LEDs.
//!
//! The Switch Panel (VID 0x06A3 / 0x046D, PID 0x0D67) provides:
//! - 5-position magneto/ignition switch (OFF, R, L, BOTH, START)
//! - 7 toggle switches (Master Bat, Master Alt, Avionics, Fuel Pump,
//!   De-ice, Pitot Heat, Cowl Flaps)
//! - Gear lever (UP / DOWN)
//! - 3 gear indicator LEDs (Left, Nose, Right), each with green and red
//!
//! ## HID output report (to device — gear LEDs, community-documented)
//!
//! ```text
//! Byte 0 : Report ID 0x00
//! Byte 1 : Gear LED bits
//!   bit 0 : Left gear  — green
//!   bit 1 : Left gear  — red
//!   bit 2 : Nose gear   — green
//!   bit 3 : Nose gear   — red
//!   bit 4 : Right gear — green
//!   bit 5 : Right gear — red
//! ```
//!
//! ## HID input report (from device — switches, community-documented)
//!
//! ```text
//! Byte 0 : Report ID 0x00
//! Byte 1 : Switch bits (lower)
//!   bit 0 : Master Battery
//!   bit 1 : Master Alternator
//!   bit 2 : Avionics Master
//!   bit 3 : Fuel Pump
//!   bit 4 : De-ice
//!   bit 5 : Pitot Heat
//!   bit 6 : Cowl Flaps (closed)
//!   bit 7 : Panel Light
//! Byte 2 : Switch bits (upper)
//!   bit 0 : Gear lever DOWN
//!   bits 1–4 : Magneto position (encoded, see [`MagnetoPosition`])
//! ```
//!
//! **Note:** Report layouts are derived from MobiFlight, SimVim, and community
//! HID captures. Validate with real hardware before production use.

// ─── Constants ───────────────────────────────────────────────────────────────

/// USB Vendor ID (Saitek).
pub const SWITCH_PANEL_VID: u16 = 0x06A3;
/// USB Product ID.
pub const SWITCH_PANEL_PID: u16 = 0x0D67;

/// Minimum byte count for a Switch Panel HID input report.
pub const SWITCH_PANEL_INPUT_MIN_BYTES: usize = 3;
/// Total byte count for a Switch Panel HID output report.
pub const SWITCH_PANEL_OUTPUT_BYTES: usize = 2;

// ─── Magneto position ────────────────────────────────────────────────────────

/// Magneto / ignition switch positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MagnetoPosition {
    Off = 0,
    Right = 1,
    Left = 2,
    Both = 3,
    Start = 4,
}

impl MagnetoPosition {
    /// Decode magneto position from HID input byte 2 bits 1–4.
    pub fn from_hid_bits(byte2: u8) -> Option<Self> {
        match (byte2 >> 1) & 0x0F {
            0 => Some(Self::Off),
            1 => Some(Self::Right),
            2 => Some(Self::Left),
            3 => Some(Self::Both),
            4 => Some(Self::Start),
            _ => None,
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Right => "R",
            Self::Left => "L",
            Self::Both => "BOTH",
            Self::Start => "START",
        }
    }
}

// ─── Switch state ────────────────────────────────────────────────────────────

/// Parsed switch state from a Switch Panel HID input report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SwitchPanelSwitchState {
    /// Raw byte 1 of the HID input report (toggle switches).
    pub byte1: u8,
    /// Raw byte 2 of the HID input report (gear + magneto).
    pub byte2: u8,
}

impl SwitchPanelSwitchState {
    // ── Byte 1 — toggle switches ────────────────────────────────────────────
    pub fn master_battery(&self) -> bool {
        self.byte1 & (1 << 0) != 0
    }
    pub fn master_alternator(&self) -> bool {
        self.byte1 & (1 << 1) != 0
    }
    pub fn avionics_master(&self) -> bool {
        self.byte1 & (1 << 2) != 0
    }
    pub fn fuel_pump(&self) -> bool {
        self.byte1 & (1 << 3) != 0
    }
    pub fn de_ice(&self) -> bool {
        self.byte1 & (1 << 4) != 0
    }
    pub fn pitot_heat(&self) -> bool {
        self.byte1 & (1 << 5) != 0
    }
    pub fn cowl_flaps_closed(&self) -> bool {
        self.byte1 & (1 << 6) != 0
    }
    pub fn panel_light(&self) -> bool {
        self.byte1 & (1 << 7) != 0
    }

    // ── Byte 2 — gear + magneto ─────────────────────────────────────────────
    pub fn gear_down(&self) -> bool {
        self.byte2 & (1 << 0) != 0
    }
    pub fn magneto(&self) -> Option<MagnetoPosition> {
        MagnetoPosition::from_hid_bits(self.byte2)
    }
}

/// Parse a Switch Panel HID input report.
///
/// Returns `None` when `data` is shorter than [`SWITCH_PANEL_INPUT_MIN_BYTES`].
pub fn parse_switch_panel_input(data: &[u8]) -> Option<SwitchPanelSwitchState> {
    if data.len() < SWITCH_PANEL_INPUT_MIN_BYTES {
        return None;
    }
    Some(SwitchPanelSwitchState {
        byte1: data[1],
        byte2: data[2],
    })
}

// ─── Gear LED control ────────────────────────────────────────────────────────

/// Gear LED bit-position constants for the Switch Panel output report (byte 1).
pub mod gear_led_bits {
    pub const LEFT_GREEN: u8 = 1 << 0;
    pub const LEFT_RED: u8 = 1 << 1;
    pub const NOSE_GREEN: u8 = 1 << 2;
    pub const NOSE_RED: u8 = 1 << 3;
    pub const RIGHT_GREEN: u8 = 1 << 4;
    pub const RIGHT_RED: u8 = 1 << 5;
}

/// Colour state for a single gear indicator LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearLedColor {
    Off,
    Green,
    Red,
}

/// Gear LED bitmask for the Switch Panel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SwitchPanelGearLeds(pub u8);

impl SwitchPanelGearLeds {
    /// All gear LEDs off.
    pub const ALL_OFF: Self = Self(0x00);

    /// All gear LEDs green (gear down and locked).
    pub const ALL_GREEN: Self = Self(
        gear_led_bits::LEFT_GREEN | gear_led_bits::NOSE_GREEN | gear_led_bits::RIGHT_GREEN,
    );

    /// All gear LEDs red (gear in transit).
    pub const ALL_RED: Self = Self(
        gear_led_bits::LEFT_RED | gear_led_bits::NOSE_RED | gear_led_bits::RIGHT_RED,
    );

    /// Set the left gear LED colour.
    pub fn set_left(self, color: GearLedColor) -> Self {
        let cleared = self.0 & !(gear_led_bits::LEFT_GREEN | gear_led_bits::LEFT_RED);
        let bits = match color {
            GearLedColor::Off => 0,
            GearLedColor::Green => gear_led_bits::LEFT_GREEN,
            GearLedColor::Red => gear_led_bits::LEFT_RED,
        };
        Self(cleared | bits)
    }

    /// Set the nose gear LED colour.
    pub fn set_nose(self, color: GearLedColor) -> Self {
        let cleared = self.0 & !(gear_led_bits::NOSE_GREEN | gear_led_bits::NOSE_RED);
        let bits = match color {
            GearLedColor::Off => 0,
            GearLedColor::Green => gear_led_bits::NOSE_GREEN,
            GearLedColor::Red => gear_led_bits::NOSE_RED,
        };
        Self(cleared | bits)
    }

    /// Set the right gear LED colour.
    pub fn set_right(self, color: GearLedColor) -> Self {
        let cleared = self.0 & !(gear_led_bits::RIGHT_GREEN | gear_led_bits::RIGHT_RED);
        let bits = match color {
            GearLedColor::Off => 0,
            GearLedColor::Green => gear_led_bits::RIGHT_GREEN,
            GearLedColor::Red => gear_led_bits::RIGHT_RED,
        };
        Self(cleared | bits)
    }

    /// Set all three gear LEDs to the same colour.
    pub fn set_all(self, color: GearLedColor) -> Self {
        self.set_left(color).set_nose(color).set_right(color)
    }

    /// Raw bitmask byte.
    pub fn raw(self) -> u8 {
        self.0
    }

    /// Build the 2-byte HID output report.
    ///
    /// Layout: `[0x00, gear_led_bits]`
    pub fn to_hid_report(self) -> [u8; SWITCH_PANEL_OUTPUT_BYTES] {
        [0x00, self.0]
    }
}

// ─── Combined state ──────────────────────────────────────────────────────────

/// Combined runtime state for the Switch Panel.
#[derive(Debug, Clone, Default)]
pub struct SwitchPanelState {
    /// Current switch positions.
    pub switches: SwitchPanelSwitchState,
    /// Current gear LED state.
    pub gear_leds: SwitchPanelGearLeds,
}

impl SwitchPanelState {
    /// Build the HID output report from the current gear LED state.
    pub fn to_hid_report(&self) -> [u8; SWITCH_PANEL_OUTPUT_BYTES] {
        self.gear_leds.to_hid_report()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── MagnetoPosition ──────────────────────────────────────────────────────

    #[test]
    fn test_magneto_all_positions() {
        assert_eq!(MagnetoPosition::from_hid_bits(0b0000_0000), Some(MagnetoPosition::Off));
        assert_eq!(MagnetoPosition::from_hid_bits(0b0000_0010), Some(MagnetoPosition::Right));
        assert_eq!(MagnetoPosition::from_hid_bits(0b0000_0100), Some(MagnetoPosition::Left));
        assert_eq!(MagnetoPosition::from_hid_bits(0b0000_0110), Some(MagnetoPosition::Both));
        assert_eq!(MagnetoPosition::from_hid_bits(0b0000_1000), Some(MagnetoPosition::Start));
    }

    #[test]
    fn test_magneto_reserved_values() {
        // Values 5-15 are reserved
        assert_eq!(MagnetoPosition::from_hid_bits(0b0000_1010), None);
        assert_eq!(MagnetoPosition::from_hid_bits(0b0001_1110), None);
    }

    #[test]
    fn test_magneto_labels() {
        assert_eq!(MagnetoPosition::Off.label(), "OFF");
        assert_eq!(MagnetoPosition::Right.label(), "R");
        assert_eq!(MagnetoPosition::Left.label(), "L");
        assert_eq!(MagnetoPosition::Both.label(), "BOTH");
        assert_eq!(MagnetoPosition::Start.label(), "START");
    }

    // ── Parse switch input ───────────────────────────────────────────────────

    #[test]
    fn test_parse_switch_input_too_short() {
        assert!(parse_switch_panel_input(&[0x00, 0x00]).is_none());
        assert!(parse_switch_panel_input(&[]).is_none());
    }

    #[test]
    fn test_parse_switch_input_all_off() {
        let data = [0x00u8, 0x00, 0x00];
        let state = parse_switch_panel_input(&data).unwrap();
        assert!(!state.master_battery());
        assert!(!state.master_alternator());
        assert!(!state.avionics_master());
        assert!(!state.fuel_pump());
        assert!(!state.de_ice());
        assert!(!state.pitot_heat());
        assert!(!state.cowl_flaps_closed());
        assert!(!state.panel_light());
        assert!(!state.gear_down());
        assert_eq!(state.magneto(), Some(MagnetoPosition::Off));
    }

    #[test]
    fn test_parse_switch_input_all_on() {
        let data = [0x00u8, 0xFF, 0xFF];
        let state = parse_switch_panel_input(&data).unwrap();
        assert!(state.master_battery());
        assert!(state.master_alternator());
        assert!(state.avionics_master());
        assert!(state.fuel_pump());
        assert!(state.de_ice());
        assert!(state.pitot_heat());
        assert!(state.cowl_flaps_closed());
        assert!(state.panel_light());
        assert!(state.gear_down());
    }

    #[test]
    fn test_parse_switch_input_individual_switches() {
        // Only Master Battery on
        let data = [0x00u8, 0b0000_0001, 0x00];
        let state = parse_switch_panel_input(&data).unwrap();
        assert!(state.master_battery());
        assert!(!state.master_alternator());
        assert!(!state.avionics_master());

        // Only Avionics Master on
        let data = [0x00u8, 0b0000_0100, 0x00];
        let state = parse_switch_panel_input(&data).unwrap();
        assert!(!state.master_battery());
        assert!(state.avionics_master());
    }

    #[test]
    fn test_parse_switch_input_gear_down() {
        let data = [0x00u8, 0x00, 0b0000_0001];
        let state = parse_switch_panel_input(&data).unwrap();
        assert!(state.gear_down());
    }

    #[test]
    fn test_parse_switch_input_magneto_both() {
        // Magneto BOTH = value 3 in bits 1-4 → byte2 = 0b0000_0110
        let data = [0x00u8, 0x00, 0b0000_0110];
        let state = parse_switch_panel_input(&data).unwrap();
        assert_eq!(state.magneto(), Some(MagnetoPosition::Both));
    }

    // ── Gear LED bits ────────────────────────────────────────────────────────

    #[test]
    fn test_gear_led_bits_are_distinct() {
        let bits = [
            gear_led_bits::LEFT_GREEN,
            gear_led_bits::LEFT_RED,
            gear_led_bits::NOSE_GREEN,
            gear_led_bits::NOSE_RED,
            gear_led_bits::RIGHT_GREEN,
            gear_led_bits::RIGHT_RED,
        ];
        for (i, &b) in bits.iter().enumerate() {
            assert!(b.is_power_of_two(), "gear_led_bits[{i}] = {b:#010b}");
        }
        let combined: u8 = bits.iter().fold(0, |acc, &b| acc | b);
        assert_eq!(combined, 0b0011_1111);
    }

    // ── SwitchPanelGearLeds ──────────────────────────────────────────────────

    #[test]
    fn test_gear_leds_all_off() {
        let leds = SwitchPanelGearLeds::ALL_OFF;
        assert_eq!(leds.raw(), 0x00);
        let report = leds.to_hid_report();
        assert_eq!(report, [0x00, 0x00]);
    }

    #[test]
    fn test_gear_leds_all_green() {
        let leds = SwitchPanelGearLeds::ALL_GREEN;
        assert_ne!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_ne!(leds.raw() & gear_led_bits::NOSE_GREEN, 0);
        assert_ne!(leds.raw() & gear_led_bits::RIGHT_GREEN, 0);
        // Red bits should not be set
        assert_eq!(leds.raw() & gear_led_bits::LEFT_RED, 0);
        assert_eq!(leds.raw() & gear_led_bits::NOSE_RED, 0);
        assert_eq!(leds.raw() & gear_led_bits::RIGHT_RED, 0);
    }

    #[test]
    fn test_gear_leds_all_red() {
        let leds = SwitchPanelGearLeds::ALL_RED;
        assert_ne!(leds.raw() & gear_led_bits::LEFT_RED, 0);
        assert_ne!(leds.raw() & gear_led_bits::NOSE_RED, 0);
        assert_ne!(leds.raw() & gear_led_bits::RIGHT_RED, 0);
        // Green bits should not be set
        assert_eq!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
    }

    #[test]
    fn test_gear_leds_set_individual() {
        let leds = SwitchPanelGearLeds::ALL_OFF
            .set_left(GearLedColor::Green)
            .set_nose(GearLedColor::Red)
            .set_right(GearLedColor::Off);

        assert_ne!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_eq!(leds.raw() & gear_led_bits::LEFT_RED, 0);
        assert_eq!(leds.raw() & gear_led_bits::NOSE_GREEN, 0);
        assert_ne!(leds.raw() & gear_led_bits::NOSE_RED, 0);
        assert_eq!(leds.raw() & gear_led_bits::RIGHT_GREEN, 0);
        assert_eq!(leds.raw() & gear_led_bits::RIGHT_RED, 0);
    }

    #[test]
    fn test_gear_leds_set_all() {
        let leds = SwitchPanelGearLeds::ALL_OFF.set_all(GearLedColor::Green);
        assert_eq!(leds, SwitchPanelGearLeds::ALL_GREEN);

        let leds = SwitchPanelGearLeds::ALL_GREEN.set_all(GearLedColor::Red);
        assert_eq!(leds, SwitchPanelGearLeds::ALL_RED);

        let leds = SwitchPanelGearLeds::ALL_RED.set_all(GearLedColor::Off);
        assert_eq!(leds, SwitchPanelGearLeds::ALL_OFF);
    }

    #[test]
    fn test_gear_leds_color_override() {
        // Setting green should clear red and vice versa
        let leds = SwitchPanelGearLeds::ALL_OFF
            .set_left(GearLedColor::Red)
            .set_left(GearLedColor::Green);
        assert_ne!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_eq!(leds.raw() & gear_led_bits::LEFT_RED, 0);
    }

    #[test]
    fn test_gear_leds_hid_report_format() {
        let leds = SwitchPanelGearLeds::ALL_GREEN;
        let report = leds.to_hid_report();
        assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
        assert_eq!(report[0], 0x00, "byte 0 = report ID");
        assert_eq!(report[1], leds.raw(), "byte 1 = LED bits");
    }

    // ── SwitchPanelState ─────────────────────────────────────────────────────

    #[test]
    fn test_switch_panel_state_default() {
        let state = SwitchPanelState::default();
        let report = state.to_hid_report();
        assert_eq!(report, [0x00, 0x00]);
    }

    #[test]
    fn test_switch_panel_state_gear_down_green() {
        let mut state = SwitchPanelState::default();
        state.gear_leds = SwitchPanelGearLeds::ALL_GREEN;
        let report = state.to_hid_report();
        assert_ne!(report[1], 0x00, "gear LEDs should be set");
    }

    // ── Gear transition simulation ───────────────────────────────────────────

    #[test]
    fn test_gear_transition_up_transit_down() {
        // Gear UP: all off
        let leds_up = SwitchPanelGearLeds::ALL_OFF;
        assert_eq!(leds_up.raw(), 0x00);

        // In transit: all red
        let leds_transit = SwitchPanelGearLeds::ALL_RED;
        assert_ne!(leds_transit.raw() & gear_led_bits::LEFT_RED, 0);
        assert_ne!(leds_transit.raw() & gear_led_bits::NOSE_RED, 0);
        assert_ne!(leds_transit.raw() & gear_led_bits::RIGHT_RED, 0);

        // Gear DOWN and locked: all green
        let leds_down = SwitchPanelGearLeds::ALL_GREEN;
        assert_ne!(leds_down.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_ne!(leds_down.raw() & gear_led_bits::NOSE_GREEN, 0);
        assert_ne!(leds_down.raw() & gear_led_bits::RIGHT_GREEN, 0);
    }
}
