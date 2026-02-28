// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek Pro Flight Radio Panel — dual frequency display and mode selection.
//!
//! The Radio Panel (VID 0x06A3 / 0x046D, PID 0x0D05) provides:
//! - Dual 5-character 7-segment displays (active / standby frequency)
//! - Mode selector rotary (COM1, COM2, NAV1, NAV2, ADF, DME, XPDR)
//! - Dual concentric rotary encoders (outer = MHz/coarse, inner = kHz/fine)
//! - ACT/STBY swap button
//!
//! ## HID output report (to device — displays, community-documented)
//!
//! ```text
//! Byte  0    : Report ID 0x00
//! Bytes 1–5  : Upper display (active frequency), 7-segment encoded
//! Bytes 6–10 : Lower display (standby frequency), 7-segment encoded
//! Bytes 11–22: Reserved (set to 0x00)
//! ```
//!
//! ## HID input report (from device — buttons/encoders, community-documented)
//!
//! ```text
//! Byte 0 : Report ID 0x00
//! Byte 1 : Mode selector bits (see [`RadioMode`])
//! Byte 2 : Buttons + encoder bits (see [`RadioPanelButtonState`])
//! ```
//!
//! **Note:** Report layouts are derived from MobiFlight, SimVim, and community
//! HID captures. Validate with real hardware before production use.

use crate::multi_panel::LcdDisplay;

// ─── Constants ───────────────────────────────────────────────────────────────

/// USB Vendor ID (Saitek).
pub const RADIO_PANEL_VID: u16 = 0x06A3;
/// USB Product ID.
pub const RADIO_PANEL_PID: u16 = 0x0D05;

/// Minimum byte count for a Radio Panel HID input report.
pub const RADIO_PANEL_INPUT_MIN_BYTES: usize = 3;
/// Total byte count for a Radio Panel HID output report.
pub const RADIO_PANEL_OUTPUT_BYTES: usize = 23;

// ─── Radio mode ──────────────────────────────────────────────────────────────

/// Radio mode selector positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RadioMode {
    Com1 = 0,
    Com2 = 1,
    Nav1 = 2,
    Nav2 = 3,
    Adf = 4,
    Dme = 5,
    Xpdr = 6,
}

impl RadioMode {
    /// Decode the mode selector from HID input byte 1.
    ///
    /// The mode is encoded in the lower 3 bits of the byte.
    /// Returns `None` for reserved / unknown values.
    pub fn from_hid_byte(byte: u8) -> Option<Self> {
        match byte & 0x07 {
            0 => Some(Self::Com1),
            1 => Some(Self::Com2),
            2 => Some(Self::Nav1),
            3 => Some(Self::Nav2),
            4 => Some(Self::Adf),
            5 => Some(Self::Dme),
            6 => Some(Self::Xpdr),
            _ => None,
        }
    }

    /// Human-readable label for this mode.
    pub fn label(self) -> &'static str {
        match self {
            Self::Com1 => "COM1",
            Self::Com2 => "COM2",
            Self::Nav1 => "NAV1",
            Self::Nav2 => "NAV2",
            Self::Adf => "ADF",
            Self::Dme => "DME",
            Self::Xpdr => "XPDR",
        }
    }
}

// ─── Button / encoder state ──────────────────────────────────────────────────

/// Parsed button and encoder state from a Radio Panel HID input report.
///
/// ## Byte 2 layout
///
/// | Bit | Label          | Description                      |
/// |-----|----------------|----------------------------------|
/// |  0  | ACT_STBY       | Active/Standby swap button       |
/// |  1  | OUTER_ENC_CW   | Outer encoder clockwise tick     |
/// |  2  | OUTER_ENC_CCW  | Outer encoder counter-clockwise  |
/// |  3  | INNER_ENC_CW   | Inner encoder clockwise tick     |
/// |  4  | INNER_ENC_CCW  | Inner encoder counter-clockwise  |
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RadioPanelButtonState {
    /// Current radio mode selector position.
    pub mode: Option<RadioMode>,
    /// Raw byte 2 of the HID input report.
    pub buttons: u8,
}

impl RadioPanelButtonState {
    /// ACT/STBY swap button is pressed.
    pub fn act_stby(&self) -> bool {
        self.buttons & (1 << 0) != 0
    }
    /// Outer encoder clockwise tick.
    pub fn outer_enc_cw(&self) -> bool {
        self.buttons & (1 << 1) != 0
    }
    /// Outer encoder counter-clockwise tick.
    pub fn outer_enc_ccw(&self) -> bool {
        self.buttons & (1 << 2) != 0
    }
    /// Inner encoder clockwise tick.
    pub fn inner_enc_cw(&self) -> bool {
        self.buttons & (1 << 3) != 0
    }
    /// Inner encoder counter-clockwise tick.
    pub fn inner_enc_ccw(&self) -> bool {
        self.buttons & (1 << 4) != 0
    }
}

/// Parse a Radio Panel HID input report.
///
/// Returns `None` when `data` is shorter than [`RADIO_PANEL_INPUT_MIN_BYTES`].
pub fn parse_radio_panel_input(data: &[u8]) -> Option<RadioPanelButtonState> {
    if data.len() < RADIO_PANEL_INPUT_MIN_BYTES {
        return None;
    }
    Some(RadioPanelButtonState {
        mode: RadioMode::from_hid_byte(data[1]),
        buttons: data[2],
    })
}

// ─── Dual frequency display ──────────────────────────────────────────────────

/// Dual-row frequency display for the Radio Panel.
///
/// The upper row shows the *active* frequency; the lower row shows the
/// *standby* frequency. Each row is a 5-character 7-segment display
/// (reuses [`LcdDisplay`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RadioDisplay {
    /// Active frequency display (upper row).
    pub active: LcdDisplay,
    /// Standby frequency display (lower row).
    pub standby: LcdDisplay,
}

impl RadioDisplay {
    /// Build the 23-byte HID output report.
    ///
    /// Layout: `[0x00, active[0..5], standby[0..5], 0×12 reserved]`
    pub fn to_hid_report(&self) -> [u8; RADIO_PANEL_OUTPUT_BYTES] {
        let mut report = [0u8; RADIO_PANEL_OUTPUT_BYTES];
        report[1..6].copy_from_slice(self.active.as_bytes());
        report[6..11].copy_from_slice(self.standby.as_bytes());
        report
    }
}

// ─── Combined state ──────────────────────────────────────────────────────────

/// Combined runtime state for the Radio Panel.
#[derive(Debug, Clone, Default)]
pub struct RadioPanelState {
    /// Current display contents.
    pub display: RadioDisplay,
    /// Most-recently parsed button/encoder state.
    pub buttons: RadioPanelButtonState,
}

impl RadioPanelState {
    /// Build the HID output report from the current display state.
    pub fn to_hid_report(&self) -> [u8; RADIO_PANEL_OUTPUT_BYTES] {
        self.display.to_hid_report()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multi_panel::encode_segment;
    use flight_panels_core::display;

    // ── RadioMode ────────────────────────────────────────────────────────────

    #[test]
    fn test_radio_mode_from_hid_byte_all_valid() {
        assert_eq!(RadioMode::from_hid_byte(0), Some(RadioMode::Com1));
        assert_eq!(RadioMode::from_hid_byte(1), Some(RadioMode::Com2));
        assert_eq!(RadioMode::from_hid_byte(2), Some(RadioMode::Nav1));
        assert_eq!(RadioMode::from_hid_byte(3), Some(RadioMode::Nav2));
        assert_eq!(RadioMode::from_hid_byte(4), Some(RadioMode::Adf));
        assert_eq!(RadioMode::from_hid_byte(5), Some(RadioMode::Dme));
        assert_eq!(RadioMode::from_hid_byte(6), Some(RadioMode::Xpdr));
    }

    #[test]
    fn test_radio_mode_from_hid_byte_reserved() {
        assert_eq!(RadioMode::from_hid_byte(7), None);
    }

    #[test]
    fn test_radio_mode_masks_upper_bits() {
        // Upper bits should be ignored
        assert_eq!(RadioMode::from_hid_byte(0b1111_1000), Some(RadioMode::Com1));
        assert_eq!(RadioMode::from_hid_byte(0b1111_1011), Some(RadioMode::Nav2));
    }

    #[test]
    fn test_radio_mode_labels() {
        assert_eq!(RadioMode::Com1.label(), "COM1");
        assert_eq!(RadioMode::Com2.label(), "COM2");
        assert_eq!(RadioMode::Nav1.label(), "NAV1");
        assert_eq!(RadioMode::Nav2.label(), "NAV2");
        assert_eq!(RadioMode::Adf.label(), "ADF");
        assert_eq!(RadioMode::Dme.label(), "DME");
        assert_eq!(RadioMode::Xpdr.label(), "XPDR");
    }

    // ── Button state ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_radio_input_too_short() {
        assert!(parse_radio_panel_input(&[0x00, 0x00]).is_none());
        assert!(parse_radio_panel_input(&[]).is_none());
    }

    #[test]
    fn test_parse_radio_input_no_buttons() {
        let data = [0x00u8, 0x00, 0x00];
        let state = parse_radio_panel_input(&data).unwrap();
        assert_eq!(state.mode, Some(RadioMode::Com1));
        assert!(!state.act_stby());
        assert!(!state.outer_enc_cw());
        assert!(!state.outer_enc_ccw());
        assert!(!state.inner_enc_cw());
        assert!(!state.inner_enc_ccw());
    }

    #[test]
    fn test_parse_radio_input_act_stby_button() {
        let data = [0x00u8, 0x00, 0b0000_0001];
        let state = parse_radio_panel_input(&data).unwrap();
        assert!(state.act_stby());
        assert!(!state.outer_enc_cw());
    }

    #[test]
    fn test_parse_radio_input_outer_encoder() {
        // Clockwise
        let data_cw = [0x00u8, 0x00, 0b0000_0010];
        let state_cw = parse_radio_panel_input(&data_cw).unwrap();
        assert!(state_cw.outer_enc_cw());
        assert!(!state_cw.outer_enc_ccw());

        // Counter-clockwise
        let data_ccw = [0x00u8, 0x00, 0b0000_0100];
        let state_ccw = parse_radio_panel_input(&data_ccw).unwrap();
        assert!(!state_ccw.outer_enc_cw());
        assert!(state_ccw.outer_enc_ccw());
    }

    #[test]
    fn test_parse_radio_input_inner_encoder() {
        // Clockwise
        let data_cw = [0x00u8, 0x00, 0b0000_1000];
        let state_cw = parse_radio_panel_input(&data_cw).unwrap();
        assert!(state_cw.inner_enc_cw());
        assert!(!state_cw.inner_enc_ccw());

        // Counter-clockwise
        let data_ccw = [0x00u8, 0x00, 0b0001_0000];
        let state_ccw = parse_radio_panel_input(&data_ccw).unwrap();
        assert!(!state_ccw.inner_enc_cw());
        assert!(state_ccw.inner_enc_ccw());
    }

    #[test]
    fn test_parse_radio_input_mode_selector() {
        for mode_val in 0u8..7 {
            let data = [0x00u8, mode_val, 0x00];
            let state = parse_radio_panel_input(&data).unwrap();
            assert!(state.mode.is_some(), "mode {mode_val} should be valid");
        }
    }

    // ── RadioDisplay ─────────────────────────────────────────────────────────

    #[test]
    fn test_radio_display_default_is_blank() {
        let display = RadioDisplay::default();
        assert_eq!(display.active, LcdDisplay::blank());
        assert_eq!(display.standby, LcdDisplay::blank());
    }

    #[test]
    fn test_radio_display_hid_report_size() {
        let display = RadioDisplay::default();
        let report = display.to_hid_report();
        assert_eq!(report.len(), RADIO_PANEL_OUTPUT_BYTES);
    }

    #[test]
    fn test_radio_display_hid_report_content() {
        let mut display = RadioDisplay::default();
        display.active = LcdDisplay::encode_str("12150");
        display.standby = LcdDisplay::encode_str("12350");

        let report = display.to_hid_report();
        assert_eq!(report[0], 0x00, "report ID");
        // Active frequency in bytes 1–5
        assert_eq!(report[1], encode_segment('1'));
        assert_eq!(report[2], encode_segment('2'));
        assert_eq!(report[3], encode_segment('1'));
        assert_eq!(report[4], encode_segment('5'));
        assert_eq!(report[5], encode_segment('0'));
        // Standby frequency in bytes 6–10
        assert_eq!(report[6], encode_segment('1'));
        assert_eq!(report[7], encode_segment('2'));
        assert_eq!(report[8], encode_segment('3'));
        assert_eq!(report[9], encode_segment('5'));
        assert_eq!(report[10], encode_segment('0'));
        // Reserved bytes should be zero
        for i in 11..RADIO_PANEL_OUTPUT_BYTES {
            assert_eq!(report[i], 0x00, "byte {i} should be reserved/zero");
        }
    }

    #[test]
    fn test_radio_display_blank_is_all_zeros() {
        let display = RadioDisplay::default();
        let report = display.to_hid_report();
        assert!(report.iter().all(|&b| b == 0));
    }

    // ── Frequency display with formatting ────────────────────────────────────

    #[test]
    fn test_com_frequency_display() {
        let freq_str = display::format_com_freq(121_500);
        let lcd = LcdDisplay::encode_str(&freq_str);
        assert_eq!(lcd.raw(0), encode_segment('1'));
        assert_eq!(lcd.raw(1), encode_segment('2'));
        assert_eq!(lcd.raw(2), encode_segment('1'));
        assert_eq!(lcd.raw(3), encode_segment('5'));
        assert_eq!(lcd.raw(4), encode_segment('0'));
    }

    #[test]
    fn test_nav_frequency_display() {
        let freq_str = display::format_nav_freq(110_300);
        let lcd = LcdDisplay::encode_str(&freq_str);
        assert_eq!(lcd.raw(0), encode_segment('1'));
        assert_eq!(lcd.raw(1), encode_segment('1'));
        assert_eq!(lcd.raw(2), encode_segment('0'));
        assert_eq!(lcd.raw(3), encode_segment('3'));
        assert_eq!(lcd.raw(4), encode_segment('0'));
    }

    #[test]
    fn test_xpdr_display() {
        let code_str = display::format_xpdr(1200);
        let lcd = LcdDisplay::encode_str(&code_str);
        // " 1200" — position 0 is space (blank)
        assert_eq!(lcd.raw(0), encode_segment(' '));
        assert_eq!(lcd.raw(1), encode_segment('1'));
        assert_eq!(lcd.raw(2), encode_segment('2'));
        assert_eq!(lcd.raw(3), encode_segment('0'));
        assert_eq!(lcd.raw(4), encode_segment('0'));
    }

    #[test]
    fn test_adf_display() {
        let freq_str = display::format_adf(340);
        let lcd = LcdDisplay::encode_str(&freq_str);
        // "  340"
        assert_eq!(lcd.raw(0), encode_segment(' '));
        assert_eq!(lcd.raw(1), encode_segment(' '));
        assert_eq!(lcd.raw(2), encode_segment('3'));
        assert_eq!(lcd.raw(3), encode_segment('4'));
        assert_eq!(lcd.raw(4), encode_segment('0'));
    }

    // ── RadioPanelState ──────────────────────────────────────────────────────

    #[test]
    fn test_radio_panel_state_default() {
        let state = RadioPanelState::default();
        let report = state.to_hid_report();
        assert!(report.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_radio_panel_state_with_frequencies() {
        let mut state = RadioPanelState::default();
        state.display.active = LcdDisplay::encode_str(&display::format_com_freq(118_000));
        state.display.standby = LcdDisplay::encode_str(&display::format_com_freq(136_975));
        let report = state.to_hid_report();
        // Active: "11800"
        assert_eq!(report[1], encode_segment('1'));
        // Standby: "13697"
        assert_eq!(report[6], encode_segment('1'));
        assert_eq!(report[7], encode_segment('3'));
    }
}
