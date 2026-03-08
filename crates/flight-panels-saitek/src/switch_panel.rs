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

use flight_panels_core::protocol::{PanelEvent, PanelProtocol};
use std::time::{Duration, Instant};

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
    pub const ALL_GREEN: Self =
        Self(gear_led_bits::LEFT_GREEN | gear_led_bits::NOSE_GREEN | gear_led_bits::RIGHT_GREEN);

    /// All gear LEDs red (gear in transit).
    pub const ALL_RED: Self =
        Self(gear_led_bits::LEFT_RED | gear_led_bits::NOSE_RED | gear_led_bits::RIGHT_RED);

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

// ─── PanelProtocol implementation ────────────────────────────────────────────

/// Switch Panel protocol driver.
///
/// Tracks the previous switch state to emit change events and supports
/// configurable debounce to suppress mechanical switch bounce.
pub struct SwitchPanelProtocol {
    prev_state: SwitchPanelSwitchState,
    debounce: SwitchDebounce,
}

/// Per-switch debounce tracker for mechanical switch bounce suppression.
///
/// Each switch has a last-change timestamp; transitions that arrive before
/// `debounce_period` has elapsed since the last accepted change are rejected.
pub struct SwitchDebounce {
    /// Debounce period.
    period: Duration,
    /// Last accepted transition time per switch index (0–12).
    last_change: [Option<Instant>; 13],
}

impl SwitchDebounce {
    /// Create a new debounce tracker with the given period.
    pub fn new(period: Duration) -> Self {
        Self {
            period,
            last_change: [None; 13],
        }
    }

    /// Returns `true` if the switch transition should be accepted (not bouncing).
    pub fn accept(&mut self, switch_index: usize, now: Instant) -> bool {
        if switch_index >= self.last_change.len() {
            return false;
        }
        if let Some(last) = self.last_change[switch_index]
            && now.duration_since(last) < self.period
        {
            return false;
        }
        self.last_change[switch_index] = Some(now);
        true
    }

    /// Current debounce period.
    pub fn period(&self) -> Duration {
        self.period
    }
}

/// Switch names in bit order (byte 1 bits 0–7, then byte 2 bit 0 = gear,
/// byte 2 bits 1–4 = magneto encoded separately).
const SWITCH_NAMES: [&str; 9] = [
    "MASTER_BAT",
    "MASTER_ALT",
    "AVIONICS",
    "FUEL_PUMP",
    "DE_ICE",
    "PITOT_HEAT",
    "COWL_FLAPS",
    "PANEL_LIGHT",
    "GEAR",
];

impl SwitchPanelProtocol {
    /// Create a new Switch Panel protocol driver with the given debounce period.
    pub fn new(debounce_period: Duration) -> Self {
        Self {
            prev_state: SwitchPanelSwitchState::default(),
            debounce: SwitchDebounce::new(debounce_period),
        }
    }

    /// Compare current vs previous state and emit change events, applying debounce.
    pub fn diff_with_debounce(
        &mut self,
        current: &SwitchPanelSwitchState,
        now: Instant,
    ) -> Vec<PanelEvent> {
        let mut events = Vec::new();
        let prev = &self.prev_state;

        // Toggle switches (byte1 bits 0–7)
        for (i, &name) in SWITCH_NAMES[..8].iter().enumerate() {
            let prev_on = prev.byte1 & (1 << i) != 0;
            let curr_on = current.byte1 & (1 << i) != 0;
            if prev_on != curr_on && self.debounce.accept(i, now) {
                events.push(PanelEvent::SwitchChange { name, on: curr_on });
            }
        }

        // Gear lever (byte2 bit 0)
        let prev_gear = prev.byte2 & 1 != 0;
        let curr_gear = current.byte2 & 1 != 0;
        if prev_gear != curr_gear && self.debounce.accept(8, now) {
            events.push(PanelEvent::SwitchChange {
                name: "GEAR",
                on: curr_gear,
            });
        }

        // Magneto selector
        let prev_mag = MagnetoPosition::from_hid_bits(prev.byte2);
        let curr_mag = MagnetoPosition::from_hid_bits(current.byte2);
        if prev_mag != curr_mag
            && let Some(pos) = curr_mag
        {
            events.push(PanelEvent::SelectorChange {
                name: "MAGNETO",
                position: pos as u8,
            });
        }

        self.prev_state = current.clone();
        events
    }
}

impl PanelProtocol for SwitchPanelProtocol {
    fn name(&self) -> &str {
        "Saitek Switch Panel"
    }

    fn vendor_id(&self) -> u16 {
        SWITCH_PANEL_VID
    }

    fn product_id(&self) -> u16 {
        SWITCH_PANEL_PID
    }

    fn led_names(&self) -> &[&'static str] {
        &[
            "GEAR_LEFT_GREEN",
            "GEAR_LEFT_RED",
            "GEAR_NOSE_GREEN",
            "GEAR_NOSE_RED",
            "GEAR_RIGHT_GREEN",
            "GEAR_RIGHT_RED",
        ]
    }

    fn output_report_size(&self) -> usize {
        SWITCH_PANEL_OUTPUT_BYTES
    }

    fn parse_input(&self, data: &[u8]) -> Option<Vec<PanelEvent>> {
        let state = parse_switch_panel_input(data)?;
        let mut events = Vec::new();

        // Emit switch state as events
        for (i, &name) in SWITCH_NAMES[..8].iter().enumerate() {
            if state.byte1 & (1 << i) != 0 {
                events.push(PanelEvent::SwitchChange { name, on: true });
            }
        }
        if state.gear_down() {
            events.push(PanelEvent::SwitchChange {
                name: "GEAR",
                on: true,
            });
        }
        if let Some(pos) = state.magneto() {
            events.push(PanelEvent::SelectorChange {
                name: "MAGNETO",
                position: pos as u8,
            });
        }

        Some(events)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── MagnetoPosition ──────────────────────────────────────────────────────

    #[test]
    fn test_magneto_all_positions() {
        assert_eq!(
            MagnetoPosition::from_hid_bits(0b0000_0000),
            Some(MagnetoPosition::Off)
        );
        assert_eq!(
            MagnetoPosition::from_hid_bits(0b0000_0010),
            Some(MagnetoPosition::Right)
        );
        assert_eq!(
            MagnetoPosition::from_hid_bits(0b0000_0100),
            Some(MagnetoPosition::Left)
        );
        assert_eq!(
            MagnetoPosition::from_hid_bits(0b0000_0110),
            Some(MagnetoPosition::Both)
        );
        assert_eq!(
            MagnetoPosition::from_hid_bits(0b0000_1000),
            Some(MagnetoPosition::Start)
        );
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
        let state = SwitchPanelState {
            gear_leds: SwitchPanelGearLeds::ALL_GREEN,
            ..Default::default()
        };
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

    // ── SwitchPanelProtocol ──────────────────────────────────────────────────

    #[test]
    fn test_switch_panel_protocol_metadata() {
        let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
        assert_eq!(proto.name(), "Saitek Switch Panel");
        assert_eq!(proto.vendor_id(), SWITCH_PANEL_VID);
        assert_eq!(proto.product_id(), SWITCH_PANEL_PID);
        assert_eq!(proto.led_names().len(), 6);
        assert_eq!(proto.output_report_size(), SWITCH_PANEL_OUTPUT_BYTES);
    }

    #[test]
    fn test_switch_panel_protocol_parse_input() {
        let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
        // Master Battery + Gear Down + Magneto BOTH
        let data = [0x00u8, 0b0000_0001, 0b0000_0111]; // gear bit + magneto BOTH
        let events = proto.parse_input(&data).unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "MASTER_BAT",
                on: true
            }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "GEAR",
                on: true
            }
        )));
    }

    #[test]
    fn test_switch_panel_protocol_parse_too_short() {
        let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
        assert!(proto.parse_input(&[0x00]).is_none());
    }

    // ── Debounce ─────────────────────────────────────────────────────────────

    #[test]
    fn test_debounce_accepts_first_change() {
        let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
        let now = Instant::now();
        assert!(debounce.accept(0, now));
    }

    #[test]
    fn test_debounce_rejects_rapid_change() {
        let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
        let now = Instant::now();
        assert!(debounce.accept(0, now));
        // Immediate second change should be rejected
        assert!(!debounce.accept(0, now + Duration::from_millis(10)));
    }

    #[test]
    fn test_debounce_accepts_after_period() {
        let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
        let now = Instant::now();
        assert!(debounce.accept(0, now));
        // After debounce period, should be accepted
        assert!(debounce.accept(0, now + Duration::from_millis(60)));
    }

    #[test]
    fn test_debounce_independent_per_switch() {
        let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
        let now = Instant::now();
        assert!(debounce.accept(0, now));
        // Different switch should still accept
        assert!(debounce.accept(1, now));
        // Same switch should reject
        assert!(!debounce.accept(0, now + Duration::from_millis(10)));
    }

    #[test]
    fn test_debounce_out_of_bounds_rejected() {
        let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
        assert!(!debounce.accept(99, Instant::now()));
    }

    // ── Diff with debounce ───────────────────────────────────────────────────

    #[test]
    fn test_diff_detects_switch_changes() {
        let mut proto = SwitchPanelProtocol::new(Duration::ZERO);
        let now = Instant::now();

        // All off initially
        let state1 = SwitchPanelSwitchState {
            byte1: 0b0000_0001, // Master Battery on
            byte2: 0x00,
        };
        let events = proto.diff_with_debounce(&state1, now);
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "MASTER_BAT",
                on: true
            }
        )));

        // Turn off Master Battery
        let state2 = SwitchPanelSwitchState {
            byte1: 0x00,
            byte2: 0x00,
        };
        let events2 = proto.diff_with_debounce(&state2, now + Duration::from_millis(10));
        assert!(events2.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "MASTER_BAT",
                on: false
            }
        )));
    }

    #[test]
    fn test_diff_no_events_when_unchanged() {
        let mut proto = SwitchPanelProtocol::new(Duration::ZERO);
        let now = Instant::now();
        let state = SwitchPanelSwitchState {
            byte1: 0x00,
            byte2: 0x00,
        };
        let _ = proto.diff_with_debounce(&state, now);
        // Same state again → no events
        let events = proto.diff_with_debounce(&state, now + Duration::from_millis(10));
        assert!(events.is_empty());
    }

    #[test]
    fn test_diff_magneto_change_emits_selector_event() {
        let mut proto = SwitchPanelProtocol::new(Duration::ZERO);
        let now = Instant::now();
        // Magneto OFF (default)
        let state1 = SwitchPanelSwitchState {
            byte1: 0,
            byte2: 0b0000_0000,
        };
        let _ = proto.diff_with_debounce(&state1, now);

        // Magneto BOTH = value 3 → bits 1-4 = 0b0110
        let state2 = SwitchPanelSwitchState {
            byte1: 0,
            byte2: 0b0000_0110,
        };
        let events = proto.diff_with_debounce(&state2, now + Duration::from_millis(10));
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::SelectorChange {
                name: "MAGNETO",
                position: 3
            }
        )));
    }

    // ── Gear LED round-trip / depth ──────────────────────────────────────────

    #[test]
    fn test_gear_led_all_color_combinations() {
        let colors = [GearLedColor::Off, GearLedColor::Green, GearLedColor::Red];
        for &left in &colors {
            for &nose in &colors {
                for &right in &colors {
                    let leds = SwitchPanelGearLeds::ALL_OFF
                        .set_left(left)
                        .set_nose(nose)
                        .set_right(right);

                    // Verify each gear position independently
                    let has_left_green = leds.raw() & gear_led_bits::LEFT_GREEN != 0;
                    let has_left_red = leds.raw() & gear_led_bits::LEFT_RED != 0;
                    match left {
                        GearLedColor::Off => {
                            assert!(!has_left_green && !has_left_red);
                        }
                        GearLedColor::Green => {
                            assert!(has_left_green && !has_left_red);
                        }
                        GearLedColor::Red => {
                            assert!(!has_left_green && has_left_red);
                        }
                    }

                    // No color should set both green and red simultaneously
                    let left_both = has_left_green && has_left_red;
                    let nose_both = (leds.raw() & gear_led_bits::NOSE_GREEN != 0)
                        && (leds.raw() & gear_led_bits::NOSE_RED != 0);
                    let right_both = (leds.raw() & gear_led_bits::RIGHT_GREEN != 0)
                        && (leds.raw() & gear_led_bits::RIGHT_RED != 0);
                    assert!(!left_both && !nose_both && !right_both,
                        "green+red should never be set simultaneously");
                }
            }
        }
    }

    #[test]
    fn test_gear_led_hid_report_roundtrip() {
        // Build LED state, convert to HID report, verify byte layout
        let leds = SwitchPanelGearLeds::ALL_OFF
            .set_left(GearLedColor::Green)
            .set_nose(GearLedColor::Red)
            .set_right(GearLedColor::Green);
        let report = leds.to_hid_report();

        assert_eq!(report[0], 0x00, "report ID");
        assert_eq!(report[1], leds.raw(), "LED byte");
        assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
    }

    #[test]
    fn test_debounce_exact_boundary_timing() {
        let period = Duration::from_millis(50);
        let mut debounce = SwitchDebounce::new(period);
        let t0 = Instant::now();

        assert!(debounce.accept(0, t0));
        // Exactly at the boundary: should still be rejected (< not <=)
        assert!(!debounce.accept(0, t0 + period - Duration::from_nanos(1)));
        // At exactly the period: should be accepted
        assert!(debounce.accept(0, t0 + period));
    }

    #[test]
    fn test_diff_with_debounce_rapid_toggle_suppressed() {
        let debounce_period = Duration::from_millis(50);
        let mut proto = SwitchPanelProtocol::new(debounce_period);
        let t0 = Instant::now();

        // First change: Master Battery on
        let state_on = SwitchPanelSwitchState {
            byte1: 0b0000_0001,
            byte2: 0x00,
        };
        let events = proto.diff_with_debounce(&state_on, t0);
        assert_eq!(events.len(), 1); // accepted

        // Rapid toggle off within debounce period — debounced, and prev_state updated
        let state_off = SwitchPanelSwitchState {
            byte1: 0x00,
            byte2: 0x00,
        };
        let events = proto.diff_with_debounce(&state_off, t0 + Duration::from_millis(10));
        assert!(events.is_empty(), "should be suppressed by debounce");

        // After debounce period, toggling back on should be accepted
        let events = proto.diff_with_debounce(&state_on, t0 + Duration::from_millis(60));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_switch_state_all_individual_switches_toggle() {
        // Verify each toggle switch independently responds to its bit
        let switches: &[(&str, u8, fn(&SwitchPanelSwitchState) -> bool)] = &[
            ("MASTER_BAT", 0, |s| s.master_battery()),
            ("MASTER_ALT", 1, |s| s.master_alternator()),
            ("AVIONICS", 2, |s| s.avionics_master()),
            ("FUEL_PUMP", 3, |s| s.fuel_pump()),
            ("DE_ICE", 4, |s| s.de_ice()),
            ("PITOT_HEAT", 5, |s| s.pitot_heat()),
            ("COWL_FLAPS", 6, |s| s.cowl_flaps_closed()),
            ("PANEL_LIGHT", 7, |s| s.panel_light()),
        ];

        for (name, bit, accessor) in switches {
            let data = [0x00u8, 1 << bit, 0x00];
            let state = parse_switch_panel_input(&data).unwrap();
            assert!(
                accessor(&state),
                "switch {name} (bit {bit}) should be on"
            );

            // All other switches should be off
            let data_off = [0x00u8, 0x00, 0x00];
            let state_off = parse_switch_panel_input(&data_off).unwrap();
            assert!(
                !accessor(&state_off),
                "switch {name} should be off when bit is clear"
            );
        }
    }

    #[test]
    fn test_switch_panel_malformed_report_extra_bytes_ok() {
        // Reports longer than minimum should still parse (extra bytes ignored)
        let data = [0x00u8, 0b0000_0001, 0b0000_0001, 0xFF, 0xFF];
        let state = parse_switch_panel_input(&data).unwrap();
        assert!(state.master_battery());
        assert!(state.gear_down());
    }
}
