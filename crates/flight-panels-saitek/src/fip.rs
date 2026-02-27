// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek Pro Flight Instrument Panel (FIP) driver.
//!
//! VID: 0x06A3  PID: 0x0A2E
//! 320×240 pixel LCD display with rotary knob and 6 page-select buttons.

// ─── Constants ────────────────────────────────────────────────────────────────

pub const FIP_VID: u16 = 0x06A3;
pub const FIP_PID: u16 = 0x0A2E;

/// Pixel width of the FIP LCD display.
pub const FIP_WIDTH: usize = 320;
/// Pixel height of the FIP LCD display.
pub const FIP_HEIGHT: usize = 240;
/// Pixel format identifier (RGB565, 2 bytes per pixel).
pub const FIP_PIXEL_FORMAT: &str = "RGB565";

// ─── Button enum ─────────────────────────────────────────────────────────────

/// Button / input identifiers for the FIP panel.
///
/// The six `Page*` variants correspond to the page-select buttons arranged
/// vertically on the left side of the device. `RotaryCw` / `RotaryCcw`
/// represent a single clockwise / counter-clockwise tick of the rotary knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FipButton {
    Page1 = 0,
    Page2 = 1,
    Page3 = 2,
    Page4 = 3,
    Page5 = 4,
    Page6 = 5,
    RotaryCw = 6,
    RotaryCcw = 7,
}

// ─── FipFrame ─────────────────────────────────────────────────────────────────

/// A single 320×240 display frame in RGB565 format (2 bytes per pixel,
/// big-endian byte order: high byte first).
///
/// Total buffer size: `FIP_WIDTH * FIP_HEIGHT * 2` = 153 600 bytes.
#[derive(Debug)]
pub struct FipFrame {
    /// Raw pixel data, row-major, big-endian RGB565.
    pub pixels: Vec<u8>,
}

impl FipFrame {
    /// Create a new all-black frame.
    pub fn new() -> Self {
        Self {
            pixels: vec![0u8; FIP_WIDTH * FIP_HEIGHT * 2],
        }
    }

    /// Write a single pixel at `(x, y)` using a pre-encoded RGB565 value.
    ///
    /// Out-of-bounds coordinates are silently ignored.
    #[inline]
    pub fn set_pixel_rgb565(&mut self, x: usize, y: usize, rgb565: u16) {
        if x >= FIP_WIDTH || y >= FIP_HEIGHT {
            return;
        }
        let idx = (y * FIP_WIDTH + x) * 2;
        self.pixels[idx] = (rgb565 >> 8) as u8;
        self.pixels[idx + 1] = (rgb565 & 0xFF) as u8;
    }

    /// Read the RGB565 value of the pixel at `(x, y)`.
    ///
    /// Returns `0` for out-of-bounds coordinates.
    #[inline]
    pub fn get_pixel_rgb565(&self, x: usize, y: usize) -> u16 {
        if x >= FIP_WIDTH || y >= FIP_HEIGHT {
            return 0;
        }
        let idx = (y * FIP_WIDTH + x) * 2;
        ((self.pixels[idx] as u16) << 8) | (self.pixels[idx + 1] as u16)
    }
}

impl Default for FipFrame {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Button state ─────────────────────────────────────────────────────────────

/// Parsed button state from a single FIP HID input report byte.
///
/// Each bit corresponds to one [`FipButton`] variant (bit position = `button as u8`).
pub struct FipButtonState(pub u8);

impl FipButtonState {
    /// Returns `true` when `button` is currently pressed.
    #[inline]
    pub fn is_pressed(&self, button: FipButton) -> bool {
        (self.0 >> (button as u8)) & 1 == 1
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── FipFrame construction ────────────────────────────────────────────────

    #[test]
    fn test_fip_frame_new_correct_size() {
        let frame = FipFrame::new();
        assert_eq!(frame.pixels.len(), FIP_WIDTH * FIP_HEIGHT * 2);
    }

    #[test]
    fn test_fip_frame_new_all_zeros() {
        let frame = FipFrame::new();
        assert!(frame.pixels.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_fip_frame_pixel_count_is_exact() {
        let frame = FipFrame::new();
        // 320 * 240 * 2 = 153 600
        assert_eq!(frame.pixels.len(), 153_600);
    }

    // ── Pixel round-trips ────────────────────────────────────────────────────

    #[test]
    fn test_fip_frame_set_get_top_left() {
        let mut frame = FipFrame::new();
        frame.set_pixel_rgb565(0, 0, 0xF800); // pure red in RGB565
        assert_eq!(frame.get_pixel_rgb565(0, 0), 0xF800);
    }

    #[test]
    fn test_fip_frame_set_get_bottom_right() {
        let mut frame = FipFrame::new();
        frame.set_pixel_rgb565(319, 239, 0x07E0); // pure green in RGB565
        assert_eq!(frame.get_pixel_rgb565(319, 239), 0x07E0);
    }

    #[test]
    fn test_fip_frame_set_get_arbitrary_pixel() {
        let mut frame = FipFrame::new();
        frame.set_pixel_rgb565(100, 120, 0x001F); // pure blue in RGB565
        assert_eq!(frame.get_pixel_rgb565(100, 120), 0x001F);
        // Adjacent pixels must remain untouched
        assert_eq!(frame.get_pixel_rgb565(99, 120), 0x0000);
        assert_eq!(frame.get_pixel_rgb565(101, 120), 0x0000);
    }

    #[test]
    fn test_fip_frame_out_of_bounds_set_does_not_panic() {
        let mut frame = FipFrame::new();
        frame.set_pixel_rgb565(320, 0, 0xFFFF); // x == FIP_WIDTH, out of bounds
        frame.set_pixel_rgb565(0, 240, 0xFFFF); // y == FIP_HEIGHT, out of bounds
        frame.set_pixel_rgb565(9999, 9999, 0xFFFF);
        // Buffer must remain untouched
        assert!(frame.pixels.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_fip_frame_out_of_bounds_get_returns_zero() {
        let frame = FipFrame::new();
        assert_eq!(frame.get_pixel_rgb565(320, 0), 0);
        assert_eq!(frame.get_pixel_rgb565(0, 240), 0);
    }

    #[test]
    fn test_fip_frame_pixel_byte_order_big_endian() {
        let mut frame = FipFrame::new();
        frame.set_pixel_rgb565(0, 0, 0x1234);
        assert_eq!(frame.pixels[0], 0x12, "high byte first");
        assert_eq!(frame.pixels[1], 0x34, "low byte second");
    }

    // ── FipButtonState ───────────────────────────────────────────────────────

    #[test]
    fn test_fip_button_state_no_buttons_pressed() {
        let state = FipButtonState(0x00);
        for btn in [
            FipButton::Page1,
            FipButton::Page2,
            FipButton::Page3,
            FipButton::Page4,
            FipButton::Page5,
            FipButton::Page6,
            FipButton::RotaryCw,
            FipButton::RotaryCcw,
        ] {
            assert!(!state.is_pressed(btn), "{btn:?} should not be pressed");
        }
    }

    #[test]
    fn test_fip_button_state_all_buttons_pressed() {
        let state = FipButtonState(0xFF);
        for btn in [
            FipButton::Page1,
            FipButton::Page2,
            FipButton::Page3,
            FipButton::Page4,
            FipButton::Page5,
            FipButton::Page6,
            FipButton::RotaryCw,
            FipButton::RotaryCcw,
        ] {
            assert!(state.is_pressed(btn), "{btn:?} should be pressed");
        }
    }

    #[test]
    fn test_fip_button_state_individual_page_buttons() {
        for (i, btn) in [
            FipButton::Page1,
            FipButton::Page2,
            FipButton::Page3,
            FipButton::Page4,
            FipButton::Page5,
            FipButton::Page6,
        ]
        .iter()
        .enumerate()
        {
            let state = FipButtonState(1u8 << i);
            assert!(
                state.is_pressed(*btn),
                "{btn:?} should be pressed with bit {i}"
            );
        }
    }

    #[test]
    fn test_fip_button_state_rotary_cw() {
        let state = FipButtonState(1 << 6);
        assert!(state.is_pressed(FipButton::RotaryCw));
        assert!(!state.is_pressed(FipButton::RotaryCcw));
    }

    #[test]
    fn test_fip_button_state_rotary_ccw() {
        let state = FipButtonState(1 << 7);
        assert!(state.is_pressed(FipButton::RotaryCcw));
        assert!(!state.is_pressed(FipButton::RotaryCw));
    }

    #[test]
    fn test_fip_button_state_only_set_bit_is_pressed() {
        // Only Page3 (bit 2) pressed
        let state = FipButtonState(0b0000_0100);
        assert!(!state.is_pressed(FipButton::Page1));
        assert!(!state.is_pressed(FipButton::Page2));
        assert!(state.is_pressed(FipButton::Page3));
        assert!(!state.is_pressed(FipButton::Page4));
        assert!(!state.is_pressed(FipButton::Page5));
        assert!(!state.is_pressed(FipButton::Page6));
        assert!(!state.is_pressed(FipButton::RotaryCw));
        assert!(!state.is_pressed(FipButton::RotaryCcw));
    }
}
