// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek Pro Flight Multi Panel — LCD display abstraction and button state.
//!
//! The Multi Panel (VID 0x06A3 / 0x046D, PID 0x0D06) provides:
//! - A 5-character 7-segment LCD display for autopilot value readout
//! - A mode selector knob (ALT / VS / IAS / HDG / CRS)
//! - Autopilot control buttons (AP, HDG, NAV, IAS, ALT, VS, APR, REV, AT)
//! - Rotary encoder for value increment / decrement
//! - Flap (UP / DN) and pitch-trim (UP / DN) buttons
//! - LED indicators for each autopilot mode button
//!
//! ## HID output report (to device — LCD + LEDs, community-documented)
//!
//! ```text
//! Byte  0   : Report ID 0x00
//! Bytes 1–5 : Display characters 1–5 (7-segment encoded, left to right)
//! Bytes 6–10: Lower row / reserved (set to 0x00 for single-row display)
//! Byte 11   : LED bitmask — bits 0-7 = ALT, VS, IAS, HDG, CRS, AT, FLAPS, PITCHTRIM
//! ```
//!
//! ## HID input report (from device — buttons, community-documented)
//!
//! ```text
//! Byte 0 : Report ID 0x00
//! Byte 1 : Mode selector + encoder (see [`MultiPanelButtonState`])
//! Byte 2 : AP function buttons (see [`MultiPanelButtonState`])
//! ```
//!
//! **Note:** Report layouts above are derived from MobiFlight, SimVim, and
//! community HID captures. Validate with real hardware before production use.

// ─── Constants ───────────────────────────────────────────────────────────────

/// Minimum byte count for a Multi Panel HID input report (report-ID + 2 data bytes).
pub const MULTI_PANEL_INPUT_MIN_BYTES: usize = 3;

/// Total byte count for a Multi Panel HID output report.
pub const MULTI_PANEL_OUTPUT_BYTES: usize = 12;

// ─── LED mask ────────────────────────────────────────────────────────────────

/// LED bit-position constants for the Multi Panel output report (byte 11).
///
/// Bit order matches [`crate::saitek::PanelType::MultiPanel::led_mapping()`]:
/// `["ALT", "VS", "IAS", "HDG", "CRS", "AUTOTHROTTLE", "FLAPS", "PITCHTRIM"]`.
pub mod led_bits {
    pub const ALT: u8 = 1 << 0;
    pub const VS: u8 = 1 << 1;
    pub const IAS: u8 = 1 << 2;
    pub const HDG: u8 = 1 << 3;
    pub const CRS: u8 = 1 << 4;
    pub const AUTO_THROTTLE: u8 = 1 << 5;
    pub const FLAPS: u8 = 1 << 6;
    pub const PITCH_TRIM: u8 = 1 << 7;
}

/// LED bitmask for the Multi Panel.
///
/// Combine constants from [`led_bits`] or use the [`set`][Self::set] builder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MultiPanelLedMask(pub u8);

impl MultiPanelLedMask {
    /// All LEDs off.
    pub const NONE: Self = Self(0x00);
    /// All LEDs on.
    pub const ALL: Self = Self(0xFF);

    /// Returns `true` if the given bit-pattern (from [`led_bits`]) is set.
    #[inline]
    pub fn is_set(self, bit: u8) -> bool {
        self.0 & bit != 0
    }

    /// Set or clear specific bit(s).
    #[inline]
    pub fn set(self, bit: u8, on: bool) -> Self {
        if on {
            Self(self.0 | bit)
        } else {
            Self(self.0 & !bit)
        }
    }

    /// Return the raw bitmask byte.
    #[inline]
    pub fn raw(self) -> u8 {
        self.0
    }
}

impl From<u8> for MultiPanelLedMask {
    fn from(v: u8) -> Self {
        Self(v)
    }
}

// ─── 7-segment LCD ───────────────────────────────────────────────────────────

/// Encode a single character as a 7-segment byte.
///
/// ## Segment bit assignment
///
/// | Bit | Segment | Panel position   |
/// |-----|---------|------------------|
/// |  0  |    a    | top horizontal   |
/// |  1  |    b    | upper-right vert |
/// |  2  |    c    | lower-right vert |
/// |  3  |    d    | bottom horizontal|
/// |  4  |    e    | lower-left vert  |
/// |  5  |    f    | upper-left vert  |
/// |  6  |    g    | middle horizontal|
///
/// Unrepresentable characters return `0x00` (blank).
pub fn encode_segment(c: char) -> u8 {
    match c {
        '0' => 0x3F,
        '1' => 0x06,
        '2' => 0x5B,
        '3' => 0x4F,
        '4' => 0x66,
        '5' => 0x6D,
        '6' => 0x7D,
        '7' => 0x07,
        '8' => 0x7F,
        '9' => 0x6F,
        '-' => 0x40,
        ' ' => 0x00,
        _ => 0x00,
    }
}

/// 5-character 7-segment LCD display, as found on the Saitek Multi Panel.
///
/// Each position stores one raw 7-segment byte. Use [`encode_segment`] to
/// convert individual characters or the convenience constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcdDisplay {
    chars: [u8; 5],
}

impl Default for LcdDisplay {
    fn default() -> Self {
        Self::blank()
    }
}

impl LcdDisplay {
    /// Blank (all-segments-off) display.
    pub const fn blank() -> Self {
        Self { chars: [0u8; 5] }
    }

    /// Encode up to 5 characters from `s` left-to-right; remaining positions
    /// are filled with blanks. Extra characters are silently truncated.
    pub fn encode_str(s: &str) -> Self {
        let mut chars = [0u8; 5];
        for (i, c) in s.chars().take(5).enumerate() {
            chars[i] = encode_segment(c);
        }
        Self { chars }
    }

    /// Display an integer right-justified in 5 columns.
    ///
    /// - Non-negative values 0..=99 999 are shown as-is, right-padded with spaces.
    /// - Negative values down to −9 999 are shown with a leading `−`.
    /// - Out-of-range values are clamped to the representable extremes.
    pub fn from_integer(value: i32) -> Self {
        let s = if value < 0 {
            let abs = value.unsigned_abs().min(9999);
            format!("-{abs:>4}")
        } else {
            format!("{:>5}", value.min(99999))
        };
        Self::encode_str(&s)
    }

    /// Set one character position (0 = leftmost) from a `char`.
    /// Out-of-bounds positions are silently ignored.
    pub fn set_char(&mut self, position: usize, c: char) {
        if let Some(slot) = self.chars.get_mut(position) {
            *slot = encode_segment(c);
        }
    }

    /// Set one position to an arbitrary raw 7-segment byte.
    /// Out-of-bounds positions are silently ignored.
    pub fn set_raw(&mut self, position: usize, segments: u8) {
        if let Some(slot) = self.chars.get_mut(position) {
            *slot = segments;
        }
    }

    /// Return the raw 7-segment byte at `position` (0 = leftmost).
    pub fn raw(&self, position: usize) -> u8 {
        self.chars.get(position).copied().unwrap_or(0)
    }

    /// Return all five raw 7-segment bytes (leftmost first).
    pub fn as_bytes(&self) -> &[u8; 5] {
        &self.chars
    }

    /// Build the 12-byte HID output report for this display combined with
    /// `led_mask`.
    ///
    /// Layout: `[0x00, d0, d1, d2, d3, d4, 0, 0, 0, 0, 0, led_mask_byte]`
    pub fn to_hid_report(&self, led_mask: MultiPanelLedMask) -> [u8; MULTI_PANEL_OUTPUT_BYTES] {
        let mut report = [0u8; MULTI_PANEL_OUTPUT_BYTES];
        // byte 0 = report ID
        report[1..6].copy_from_slice(&self.chars);
        // bytes 6–10 = lower row (unused)
        report[11] = led_mask.raw();
        report
    }
}

// ─── Button state ─────────────────────────────────────────────────────────────

/// Parsed button / switch state from a Multi Panel HID input report.
///
/// ## Byte 1 — mode selector + encoder
///
/// | Bit | Label   | Description                       |
/// |-----|---------|-----------------------------------|
/// |  0  | SEL_ALT | Mode knob in ALT position         |
/// |  1  | SEL_VS  | Mode knob in VS position          |
/// |  2  | SEL_IAS | Mode knob in IAS position         |
/// |  3  | SEL_HDG | Mode knob in HDG position         |
/// |  4  | SEL_CRS | Mode knob in CRS position         |
/// |  5  | ENC_CCW | Rotary encoder counter-clockwise  |
/// |  6  | ENC_CW  | Rotary encoder clockwise          |
///
/// ## Byte 2 — AP function buttons
///
/// | Bit | Label   | Description              |
/// |-----|---------|--------------------------|
/// |  0  | AP      | Autopilot master         |
/// |  1  | HDG     | HDG mode button          |
/// |  2  | NAV     | NAV mode button          |
/// |  3  | IAS     | IAS / Mach mode button   |
/// |  4  | ALT     | ALT hold button          |
/// |  5  | VS      | VS mode button           |
/// |  6  | APR     | Approach mode button     |
/// |  7  | REV     | Reverse / back-course    |
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultiPanelButtonState {
    /// Byte 1 of the HID input report (mode selector + encoder bits).
    pub byte1: u8,
    /// Byte 2 of the HID input report (AP function button bits).
    pub byte2: u8,
}

impl MultiPanelButtonState {
    // ── Byte 1 ──────────────────────────────────────────────────────────────
    pub fn sel_alt(&self) -> bool {
        self.byte1 & (1 << 0) != 0
    }
    pub fn sel_vs(&self) -> bool {
        self.byte1 & (1 << 1) != 0
    }
    pub fn sel_ias(&self) -> bool {
        self.byte1 & (1 << 2) != 0
    }
    pub fn sel_hdg(&self) -> bool {
        self.byte1 & (1 << 3) != 0
    }
    pub fn sel_crs(&self) -> bool {
        self.byte1 & (1 << 4) != 0
    }
    pub fn enc_ccw(&self) -> bool {
        self.byte1 & (1 << 5) != 0
    }
    pub fn enc_cw(&self) -> bool {
        self.byte1 & (1 << 6) != 0
    }

    // ── Byte 2 ──────────────────────────────────────────────────────────────
    pub fn btn_ap(&self) -> bool {
        self.byte2 & (1 << 0) != 0
    }
    pub fn btn_hdg(&self) -> bool {
        self.byte2 & (1 << 1) != 0
    }
    pub fn btn_nav(&self) -> bool {
        self.byte2 & (1 << 2) != 0
    }
    pub fn btn_ias(&self) -> bool {
        self.byte2 & (1 << 3) != 0
    }
    pub fn btn_alt(&self) -> bool {
        self.byte2 & (1 << 4) != 0
    }
    pub fn btn_vs(&self) -> bool {
        self.byte2 & (1 << 5) != 0
    }
    pub fn btn_apr(&self) -> bool {
        self.byte2 & (1 << 6) != 0
    }
    pub fn btn_rev(&self) -> bool {
        self.byte2 & (1 << 7) != 0
    }
}

/// Parse a Multi Panel HID input report into [`MultiPanelButtonState`].
///
/// Returns `None` when `data` is shorter than [`MULTI_PANEL_INPUT_MIN_BYTES`].
/// Byte 0 (report ID) is consumed but ignored.
pub fn parse_multi_panel_input(data: &[u8]) -> Option<MultiPanelButtonState> {
    if data.len() < MULTI_PANEL_INPUT_MIN_BYTES {
        return None;
    }
    Some(MultiPanelButtonState {
        byte1: data[1],
        byte2: data[2],
    })
}

// ─── Combined state ───────────────────────────────────────────────────────────

/// Combined runtime state for the Multi Panel.
#[derive(Debug, Clone, Default)]
pub struct MultiPanelState {
    /// Current LCD display contents.
    pub display: LcdDisplay,
    /// Current LED bitmask.
    pub leds: MultiPanelLedMask,
    /// Most-recently parsed button state.
    pub buttons: MultiPanelButtonState,
}

impl MultiPanelState {
    /// Build the 12-byte HID output report from the current display + LED state.
    pub fn to_hid_report(&self) -> [u8; MULTI_PANEL_OUTPUT_BYTES] {
        self.display.to_hid_report(self.leds)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── LED mask tests ───────────────────────────────────────────────────────

    #[test]
    fn test_led_mask_none_is_zero() {
        assert_eq!(MultiPanelLedMask::NONE.raw(), 0x00);
    }

    #[test]
    fn test_led_mask_all_is_ff() {
        assert_eq!(MultiPanelLedMask::ALL.raw(), 0xFF);
    }

    #[test]
    fn test_led_bit_constants_are_distinct_powers_of_two() {
        let bits = [
            led_bits::ALT,
            led_bits::VS,
            led_bits::IAS,
            led_bits::HDG,
            led_bits::CRS,
            led_bits::AUTO_THROTTLE,
            led_bits::FLAPS,
            led_bits::PITCH_TRIM,
        ];
        // Each must be a distinct power-of-two
        for (i, &b) in bits.iter().enumerate() {
            assert!(
                b.is_power_of_two(),
                "led_bits[{i}] = {b:#010b} is not a power-of-two"
            );
        }
        let combined: u8 = bits.iter().fold(0, |acc, &b| acc | b);
        assert_eq!(combined, 0xFF, "8 bits should cover all of 0xFF");
    }

    #[test]
    fn test_led_mask_individual_bits() {
        assert_eq!(led_bits::ALT, 0x01);
        assert_eq!(led_bits::VS, 0x02);
        assert_eq!(led_bits::IAS, 0x04);
        assert_eq!(led_bits::HDG, 0x08);
        assert_eq!(led_bits::CRS, 0x10);
        assert_eq!(led_bits::AUTO_THROTTLE, 0x20);
        assert_eq!(led_bits::FLAPS, 0x40);
        assert_eq!(led_bits::PITCH_TRIM, 0x80);
    }

    #[test]
    fn test_led_mask_set_method() {
        let mask = MultiPanelLedMask::NONE
            .set(led_bits::ALT, true)
            .set(led_bits::HDG, true);
        assert!(mask.is_set(led_bits::ALT));
        assert!(mask.is_set(led_bits::HDG));
        assert!(!mask.is_set(led_bits::VS));

        let cleared = mask.set(led_bits::ALT, false);
        assert!(!cleared.is_set(led_bits::ALT));
        assert!(cleared.is_set(led_bits::HDG));
    }

    #[test]
    fn test_led_mask_is_set() {
        let mask = MultiPanelLedMask(led_bits::CRS | led_bits::AUTO_THROTTLE);
        assert!(mask.is_set(led_bits::CRS));
        assert!(mask.is_set(led_bits::AUTO_THROTTLE));
        assert!(!mask.is_set(led_bits::ALT));
        assert!(!mask.is_set(led_bits::FLAPS));
    }

    #[test]
    fn test_led_mask_from_u8() {
        let mask = MultiPanelLedMask::from(0b0000_1010);
        assert!(mask.is_set(led_bits::VS));
        assert!(mask.is_set(led_bits::HDG));
        assert!(!mask.is_set(led_bits::ALT));
    }

    #[test]
    fn test_led_mask_all_off_then_all_on() {
        let mut mask = MultiPanelLedMask::NONE;
        for &bit in &[
            led_bits::ALT,
            led_bits::VS,
            led_bits::IAS,
            led_bits::HDG,
            led_bits::CRS,
            led_bits::AUTO_THROTTLE,
            led_bits::FLAPS,
            led_bits::PITCH_TRIM,
        ] {
            mask = mask.set(bit, true);
        }
        assert_eq!(mask, MultiPanelLedMask::ALL);
    }

    // ── 7-segment encoding tests ─────────────────────────────────────────────

    #[test]
    fn test_encode_digits_zero_through_nine() {
        let expected = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];
        for (digit, &enc) in expected.iter().enumerate() {
            let c = char::from_digit(digit as u32, 10).unwrap();
            assert_eq!(
                encode_segment(c),
                enc,
                "digit '{digit}' should encode to {enc:#04x}"
            );
        }
    }

    #[test]
    fn test_encode_space_is_zero() {
        assert_eq!(encode_segment(' '), 0x00);
    }

    #[test]
    fn test_encode_dash_is_middle_segment() {
        // Only segment g (middle) = bit 6
        assert_eq!(encode_segment('-'), 0x40);
    }

    #[test]
    fn test_encode_unknown_char_is_blank() {
        assert_eq!(encode_segment('Z'), 0x00);
        assert_eq!(encode_segment('@'), 0x00);
    }

    // ── LcdDisplay tests ─────────────────────────────────────────────────────

    #[test]
    fn test_lcd_blank_is_all_zeros() {
        let lcd = LcdDisplay::blank();
        assert_eq!(lcd.as_bytes(), &[0u8; 5]);
    }

    #[test]
    fn test_lcd_default_is_blank() {
        assert_eq!(LcdDisplay::default(), LcdDisplay::blank());
    }

    #[test]
    fn test_lcd_from_str_five_digits() {
        let lcd = LcdDisplay::encode_str("12345");
        assert_eq!(lcd.raw(0), encode_segment('1'));
        assert_eq!(lcd.raw(1), encode_segment('2'));
        assert_eq!(lcd.raw(2), encode_segment('3'));
        assert_eq!(lcd.raw(3), encode_segment('4'));
        assert_eq!(lcd.raw(4), encode_segment('5'));
    }

    #[test]
    fn test_lcd_from_str_shorter_than_five_pads_right() {
        let lcd = LcdDisplay::encode_str("42");
        assert_eq!(lcd.raw(0), encode_segment('4'));
        assert_eq!(lcd.raw(1), encode_segment('2'));
        assert_eq!(lcd.raw(2), 0x00, "position 2 should be blank");
        assert_eq!(lcd.raw(3), 0x00, "position 3 should be blank");
        assert_eq!(lcd.raw(4), 0x00, "position 4 should be blank");
    }

    #[test]
    fn test_lcd_from_str_longer_than_five_truncated() {
        let lcd = LcdDisplay::encode_str("123456789");
        // Only first 5 chars used
        for i in 0..5 {
            let c = char::from_digit(i as u32 + 1, 10).unwrap();
            assert_eq!(lcd.raw(i), encode_segment(c), "position {i}");
        }
    }

    #[test]
    fn test_lcd_from_integer_zero() {
        let lcd = LcdDisplay::from_integer(0);
        // Right-justified "    0" → 4 spaces then '0'
        assert_eq!(lcd.raw(4), encode_segment('0'));
        assert_eq!(lcd.raw(3), encode_segment(' '));
    }

    #[test]
    fn test_lcd_from_integer_positive() {
        let lcd = LcdDisplay::from_integer(100);
        // "  100" — positions 0,1 are spaces
        assert_eq!(lcd.raw(0), encode_segment(' '));
        assert_eq!(lcd.raw(1), encode_segment(' '));
        assert_eq!(lcd.raw(2), encode_segment('1'));
        assert_eq!(lcd.raw(3), encode_segment('0'));
        assert_eq!(lcd.raw(4), encode_segment('0'));
    }

    #[test]
    fn test_lcd_from_integer_max_value() {
        let lcd = LcdDisplay::from_integer(99999);
        assert_eq!(lcd.raw(0), encode_segment('9'));
        assert_eq!(lcd.raw(4), encode_segment('9'));
    }

    #[test]
    fn test_lcd_from_integer_negative() {
        let lcd = LcdDisplay::from_integer(-99);
        // "-  99" → '-', ' ', ' ', '9', '9'
        assert_eq!(lcd.raw(0), encode_segment('-'));
        assert_eq!(lcd.raw(1), encode_segment(' '));
        assert_eq!(lcd.raw(2), encode_segment(' '));
        assert_eq!(lcd.raw(3), encode_segment('9'));
        assert_eq!(lcd.raw(4), encode_segment('9'));
    }

    #[test]
    fn test_lcd_set_char_in_bounds() {
        let mut lcd = LcdDisplay::blank();
        lcd.set_char(2, '7');
        assert_eq!(lcd.raw(2), encode_segment('7'));
        assert_eq!(lcd.raw(0), 0x00);
    }

    #[test]
    fn test_lcd_set_char_out_of_bounds_noop() {
        let mut lcd = LcdDisplay::blank();
        lcd.set_char(10, '5'); // should not panic
        assert_eq!(lcd.as_bytes(), &[0u8; 5]);
    }

    #[test]
    fn test_lcd_set_raw() {
        let mut lcd = LcdDisplay::blank();
        lcd.set_raw(3, 0xAB);
        assert_eq!(lcd.raw(3), 0xAB);
    }

    #[test]
    fn test_lcd_to_hid_report_format() {
        let lcd = LcdDisplay::encode_str("12345");
        let leds = MultiPanelLedMask(led_bits::ALT | led_bits::VS);
        let report = lcd.to_hid_report(leds);

        assert_eq!(report.len(), MULTI_PANEL_OUTPUT_BYTES);
        assert_eq!(report[0], 0x00, "byte 0 = report ID");
        assert_eq!(report[1], encode_segment('1'), "byte 1 = char 0");
        assert_eq!(report[2], encode_segment('2'), "byte 2 = char 1");
        assert_eq!(report[3], encode_segment('3'), "byte 3 = char 2");
        assert_eq!(report[4], encode_segment('4'), "byte 4 = char 3");
        assert_eq!(report[5], encode_segment('5'), "byte 5 = char 4");
        // bytes 6–10 = lower row (zero)
        for i in 6..11 {
            assert_eq!(report[i], 0x00, "byte {i} should be 0");
        }
        assert_eq!(
            report[11],
            led_bits::ALT | led_bits::VS,
            "byte 11 = LED mask"
        );
    }

    #[test]
    fn test_lcd_blank_hid_report_is_zeroes_except_led() {
        let lcd = LcdDisplay::blank();
        let report = lcd.to_hid_report(MultiPanelLedMask::NONE);
        assert!(
            report.iter().all(|&b| b == 0),
            "blank display + NONE LEDs: all bytes must be 0"
        );
    }

    // ── Button state tests ───────────────────────────────────────────────────

    #[test]
    fn test_parse_input_too_short_returns_none() {
        assert!(parse_multi_panel_input(&[0x00, 0x00]).is_none());
        assert!(parse_multi_panel_input(&[]).is_none());
    }

    #[test]
    fn test_parse_input_mode_selector_bits() {
        // byte1 = 0b0001_0101 → SEL_ALT, SEL_IAS, SEL_CRS set
        let data = [0x00u8, 0b0001_0101, 0x00];
        let state = parse_multi_panel_input(&data).unwrap();
        assert!(state.sel_alt(), "SEL_ALT");
        assert!(!state.sel_vs(), "SEL_VS");
        assert!(state.sel_ias(), "SEL_IAS");
        assert!(!state.sel_hdg(), "SEL_HDG");
        assert!(state.sel_crs(), "SEL_CRS");
        assert!(!state.enc_ccw(), "ENC_CCW");
        assert!(!state.enc_cw(), "ENC_CW");
    }

    #[test]
    fn test_parse_input_encoder_bits() {
        // byte1 = 0b0100_0000 → ENC_CW (bit 6)
        let data = [0x00u8, 0b0100_0000, 0x00];
        let state = parse_multi_panel_input(&data).unwrap();
        assert!(!state.enc_ccw());
        assert!(state.enc_cw());

        // byte1 = 0b0010_0000 → ENC_CCW (bit 5)
        let data2 = [0x00u8, 0b0010_0000, 0x00];
        let state2 = parse_multi_panel_input(&data2).unwrap();
        assert!(state2.enc_ccw());
        assert!(!state2.enc_cw());
    }

    #[test]
    fn test_parse_input_ap_buttons() {
        // byte2 = 0xFF → all AP buttons pressed
        let data = [0x00u8, 0x00, 0xFF];
        let state = parse_multi_panel_input(&data).unwrap();
        assert!(state.btn_ap(), "AP");
        assert!(state.btn_hdg(), "HDG");
        assert!(state.btn_nav(), "NAV");
        assert!(state.btn_ias(), "IAS");
        assert!(state.btn_alt(), "ALT");
        assert!(state.btn_vs(), "VS");
        assert!(state.btn_apr(), "APR");
        assert!(state.btn_rev(), "REV");
    }

    #[test]
    fn test_parse_input_no_buttons_pressed() {
        let data = [0x00u8, 0x00, 0x00];
        let state = parse_multi_panel_input(&data).unwrap();
        assert!(!state.sel_alt());
        assert!(!state.enc_cw());
        assert!(!state.btn_ap());
        assert!(!state.btn_rev());
    }

    // ── MultiPanelState ───────────────────────────────────────────────────────

    #[test]
    fn test_multi_panel_state_default_is_all_zero() {
        let state = MultiPanelState::default();
        let report = state.to_hid_report();
        assert!(
            report.iter().all(|&b| b == 0),
            "default state: all report bytes should be 0"
        );
    }

    #[test]
    fn test_multi_panel_state_to_hid_report_combines_display_and_leds() {
        let mut state = MultiPanelState::default();
        state.display = LcdDisplay::encode_str("88888");
        state.leds = MultiPanelLedMask::ALL;
        let report = state.to_hid_report();
        // All 5 display positions should be 0x7F ('8')
        for i in 1..=5 {
            assert_eq!(report[i], 0x7F, "byte {i} should be 0x7F ('8')");
        }
        assert_eq!(report[11], 0xFF, "byte 11 should be 0xFF (all LEDs)");
    }
}
