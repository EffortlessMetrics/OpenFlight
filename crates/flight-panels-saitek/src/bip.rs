// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek Pro Flight Backlit Information Panel (BIP) driver.
//!
//! VID: 0x06A3  PID: 0x0B4E
//! Two LED strips of 25 LEDs each. Every LED can independently display
//! one of four states: off, green, amber, or red.

// ─── Constants ────────────────────────────────────────────────────────────────

pub const BIP_VID: u16 = 0x06A3;
pub const BIP_PID: u16 = 0x0B4E;

/// Number of LED strips on the BIP.
pub const BIP_STRIP_COUNT: usize = 2;
/// Number of LEDs per strip.
pub const BIP_LEDS_PER_STRIP: usize = 25;

// ─── LED colour ──────────────────────────────────────────────────────────────

/// Colour states supported by each BIP LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BipLedColor {
    Off = 0,
    Green = 1,
    Amber = 2,
    Red = 3,
}

// ─── BipState ────────────────────────────────────────────────────────────────

/// Full LED state for the BIP: two strips of 25 LEDs.
pub struct BipState {
    leds: [[BipLedColor; BIP_LEDS_PER_STRIP]; BIP_STRIP_COUNT],
}

impl BipState {
    /// Create a new state with all LEDs set to [`BipLedColor::Off`].
    pub fn new() -> Self {
        Self {
            leds: [[BipLedColor::Off; BIP_LEDS_PER_STRIP]; BIP_STRIP_COUNT],
        }
    }

    /// Set the colour of a single LED.
    ///
    /// Out-of-bounds `strip` or `position` values are silently ignored.
    pub fn set_led(&mut self, strip: usize, position: usize, color: BipLedColor) {
        if strip < BIP_STRIP_COUNT && position < BIP_LEDS_PER_STRIP {
            self.leds[strip][position] = color;
        }
    }

    /// Return the colour of a single LED, or `None` for out-of-bounds indices.
    pub fn get_led(&self, strip: usize, position: usize) -> Option<BipLedColor> {
        if strip < BIP_STRIP_COUNT && position < BIP_LEDS_PER_STRIP {
            Some(self.leds[strip][position])
        } else {
            None
        }
    }

    /// Encode one strip's state as a 25-byte HID report payload.
    ///
    /// Each byte holds the `u8` value of the corresponding [`BipLedColor`].
    /// Returns an all-zero array for an out-of-bounds `strip` index.
    pub fn encode_strip(&self, strip: usize) -> [u8; BIP_LEDS_PER_STRIP] {
        let mut report = [0u8; BIP_LEDS_PER_STRIP];
        if strip < BIP_STRIP_COUNT {
            for (i, &color) in self.leds[strip].iter().enumerate() {
                report[i] = color as u8;
            }
        }
        report
    }

    /// Count the number of LEDs showing `color` in the given strip.
    ///
    /// Returns `0` for an out-of-bounds `strip` index.
    pub fn count_color(&self, strip: usize, color: BipLedColor) -> usize {
        if strip >= BIP_STRIP_COUNT {
            return 0;
        }
        self.leds[strip].iter().filter(|&&c| c == color).count()
    }
}

impl Default for BipState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bip_state_new_all_off() {
        let state = BipState::new();
        for strip in 0..BIP_STRIP_COUNT {
            for pos in 0..BIP_LEDS_PER_STRIP {
                assert_eq!(
                    state.get_led(strip, pos),
                    Some(BipLedColor::Off),
                    "strip {strip} pos {pos} should be Off"
                );
            }
        }
    }

    #[test]
    fn test_bip_state_set_get_roundtrip() {
        let mut state = BipState::new();
        state.set_led(0, 0, BipLedColor::Green);
        state.set_led(1, 24, BipLedColor::Red);
        assert_eq!(state.get_led(0, 0), Some(BipLedColor::Green));
        assert_eq!(state.get_led(1, 24), Some(BipLedColor::Red));
    }

    #[test]
    fn test_bip_state_set_all_colors() {
        let mut state = BipState::new();
        state.set_led(0, 0, BipLedColor::Off);
        state.set_led(0, 1, BipLedColor::Green);
        state.set_led(0, 2, BipLedColor::Amber);
        state.set_led(0, 3, BipLedColor::Red);
        assert_eq!(state.get_led(0, 0), Some(BipLedColor::Off));
        assert_eq!(state.get_led(0, 1), Some(BipLedColor::Green));
        assert_eq!(state.get_led(0, 2), Some(BipLedColor::Amber));
        assert_eq!(state.get_led(0, 3), Some(BipLedColor::Red));
    }

    #[test]
    fn test_bip_set_led_out_of_bounds_strip_ignored() {
        let mut state = BipState::new();
        state.set_led(BIP_STRIP_COUNT, 0, BipLedColor::Green); // should not panic
        // All LEDs must remain Off
        for strip in 0..BIP_STRIP_COUNT {
            for pos in 0..BIP_LEDS_PER_STRIP {
                assert_eq!(state.get_led(strip, pos), Some(BipLedColor::Off));
            }
        }
    }

    #[test]
    fn test_bip_set_led_out_of_bounds_position_ignored() {
        let mut state = BipState::new();
        state.set_led(0, BIP_LEDS_PER_STRIP, BipLedColor::Red); // should not panic
        for pos in 0..BIP_LEDS_PER_STRIP {
            assert_eq!(state.get_led(0, pos), Some(BipLedColor::Off));
        }
    }

    #[test]
    fn test_bip_get_led_out_of_bounds_returns_none() {
        let state = BipState::new();
        assert_eq!(state.get_led(BIP_STRIP_COUNT, 0), None);
        assert_eq!(state.get_led(0, BIP_LEDS_PER_STRIP), None);
        assert_eq!(state.get_led(99, 99), None);
    }

    #[test]
    fn test_bip_encode_strip_length() {
        let state = BipState::new();
        let report = state.encode_strip(0);
        assert_eq!(report.len(), BIP_LEDS_PER_STRIP);
    }

    #[test]
    fn test_bip_encode_strip_all_off_is_zeros() {
        let state = BipState::new();
        assert_eq!(state.encode_strip(0), [0u8; BIP_LEDS_PER_STRIP]);
        assert_eq!(state.encode_strip(1), [0u8; BIP_LEDS_PER_STRIP]);
    }

    #[test]
    fn test_bip_encode_strip_all_green() {
        let mut state = BipState::new();
        for pos in 0..BIP_LEDS_PER_STRIP {
            state.set_led(0, pos, BipLedColor::Green);
        }
        assert_eq!(
            state.encode_strip(0),
            [BipLedColor::Green as u8; BIP_LEDS_PER_STRIP]
        );
    }

    #[test]
    fn test_bip_encode_strip_mixed_colors() {
        let mut state = BipState::new();
        state.set_led(1, 0, BipLedColor::Amber);
        state.set_led(1, 24, BipLedColor::Red);
        let report = state.encode_strip(1);
        assert_eq!(report[0], BipLedColor::Amber as u8);
        assert_eq!(report[24], BipLedColor::Red as u8);
        assert_eq!(report[1], BipLedColor::Off as u8);
    }

    #[test]
    fn test_bip_encode_strip_out_of_bounds_is_zeros() {
        let state = BipState::new();
        assert_eq!(
            state.encode_strip(BIP_STRIP_COUNT),
            [0u8; BIP_LEDS_PER_STRIP]
        );
    }

    #[test]
    fn test_bip_count_color_all_off() {
        let state = BipState::new();
        assert_eq!(state.count_color(0, BipLedColor::Off), BIP_LEDS_PER_STRIP);
        assert_eq!(state.count_color(0, BipLedColor::Green), 0);
        assert_eq!(state.count_color(0, BipLedColor::Amber), 0);
        assert_eq!(state.count_color(0, BipLedColor::Red), 0);
    }

    #[test]
    fn test_bip_count_color_mixed() {
        let mut state = BipState::new();
        state.set_led(0, 0, BipLedColor::Green);
        state.set_led(0, 1, BipLedColor::Green);
        state.set_led(0, 2, BipLedColor::Red);
        assert_eq!(state.count_color(0, BipLedColor::Green), 2);
        assert_eq!(state.count_color(0, BipLedColor::Red), 1);
        assert_eq!(state.count_color(0, BipLedColor::Amber), 0);
        assert_eq!(
            state.count_color(0, BipLedColor::Off),
            BIP_LEDS_PER_STRIP - 3
        );
    }

    #[test]
    fn test_bip_count_color_out_of_bounds_strip_returns_zero() {
        let state = BipState::new();
        assert_eq!(state.count_color(BIP_STRIP_COUNT, BipLedColor::Off), 0);
    }
}
