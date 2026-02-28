// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek Pro Flight Instrument Panel (FIP) driver.
//!
//! VID: 0x06A3  PID: 0x0A2E
//! 320×240 pixel LCD display with rotary knob and 6 page-select buttons.

use flight_panels_core::protocol::{PanelEvent, PanelProtocol};

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

// ─── Soft key labels ─────────────────────────────────────────────────────────

/// Manages the six configurable soft-key labels shown alongside the FIP display.
#[derive(Debug, Clone)]
pub struct FipSoftKeys {
    labels: [String; 6],
}

impl FipSoftKeys {
    /// Create soft keys with all labels blank.
    pub fn new() -> Self {
        Self {
            labels: std::array::from_fn(|_| String::new()),
        }
    }

    /// Set the label for soft key `index` (0–5).
    pub fn set_label(&mut self, index: usize, label: &str) {
        if index < 6 {
            self.labels[index] = label.to_string();
        }
    }

    /// Get the label for soft key `index`, or `""` for out-of-bounds.
    pub fn label(&self, index: usize) -> &str {
        self.labels.get(index).map_or("", String::as_str)
    }

    /// Return all 6 labels as a slice.
    pub fn labels(&self) -> &[String; 6] {
        &self.labels
    }
}

impl Default for FipSoftKeys {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Scroll wheel tracker ────────────────────────────────────────────────────

/// Accumulates scroll wheel ticks for the FIP rotary knob.
#[derive(Debug, Clone, Default)]
pub struct FipScrollWheel {
    /// Accumulated ticks (positive = CW).
    pub accumulated: i32,
}

impl FipScrollWheel {
    /// Update from a button state byte.
    pub fn update(&mut self, state: &FipButtonState) {
        if state.is_pressed(FipButton::RotaryCw) {
            self.accumulated += 1;
        }
        if state.is_pressed(FipButton::RotaryCcw) {
            self.accumulated -= 1;
        }
    }

    /// Drain and return accumulated ticks.
    pub fn drain(&mut self) -> i32 {
        let result = self.accumulated;
        self.accumulated = 0;
        result
    }
}

// ─── Page manager ────────────────────────────────────────────────────────────

/// Manages the current display page on the FIP.
///
/// The FIP supports up to 6 pages (one per page-select button).
#[derive(Debug, Clone)]
pub struct FipPageManager {
    current_page: u8,
    page_count: u8,
}

impl FipPageManager {
    /// Create a page manager with `count` pages (clamped to 1–6).
    pub fn new(count: u8) -> Self {
        Self {
            current_page: 0,
            page_count: count.clamp(1, 6),
        }
    }

    /// Select a page directly (clamped to valid range).
    pub fn select(&mut self, page: u8) {
        if page < self.page_count {
            self.current_page = page;
        }
    }

    /// Process a button press and update the current page if it's a page button.
    /// Returns `true` if the page changed.
    pub fn handle_button(&mut self, button: FipButton) -> bool {
        let page = match button {
            FipButton::Page1 => 0,
            FipButton::Page2 => 1,
            FipButton::Page3 => 2,
            FipButton::Page4 => 3,
            FipButton::Page5 => 4,
            FipButton::Page6 => 5,
            _ => return false,
        };
        if page < self.page_count && page != self.current_page {
            self.current_page = page;
            true
        } else {
            false
        }
    }

    /// Current page index (0-based).
    pub fn current(&self) -> u8 {
        self.current_page
    }

    /// Total number of pages.
    pub fn page_count(&self) -> u8 {
        self.page_count
    }
}

impl Default for FipPageManager {
    fn default() -> Self {
        Self::new(6)
    }
}

// ─── PanelProtocol implementation ────────────────────────────────────────────

/// FIP protocol driver.
pub struct FipProtocol;

impl PanelProtocol for FipProtocol {
    fn name(&self) -> &str {
        "Saitek Flight Instrument Panel"
    }

    fn vendor_id(&self) -> u16 {
        FIP_VID
    }

    fn product_id(&self) -> u16 {
        FIP_PID
    }

    fn led_names(&self) -> &[&'static str] {
        // FIP has no discrete LEDs — the display is the output
        &[]
    }

    fn output_report_size(&self) -> usize {
        // Frame buffer output is variable; report the base size
        FIP_WIDTH * FIP_HEIGHT * 2
    }

    fn parse_input(&self, data: &[u8]) -> Option<Vec<PanelEvent>> {
        if data.is_empty() {
            return None;
        }
        let state = FipButtonState(data[0]);
        let mut events = Vec::new();

        let buttons = [
            (FipButton::Page1, "PAGE1"),
            (FipButton::Page2, "PAGE2"),
            (FipButton::Page3, "PAGE3"),
            (FipButton::Page4, "PAGE4"),
            (FipButton::Page5, "PAGE5"),
            (FipButton::Page6, "PAGE6"),
        ];
        for (btn, name) in buttons {
            if state.is_pressed(btn) {
                events.push(PanelEvent::ButtonPress { name });
            }
        }
        if state.is_pressed(FipButton::RotaryCw) {
            events.push(PanelEvent::EncoderTick {
                name: "SCROLL",
                delta: 1,
            });
        }
        if state.is_pressed(FipButton::RotaryCcw) {
            events.push(PanelEvent::EncoderTick {
                name: "SCROLL",
                delta: -1,
            });
        }

        Some(events)
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

    // ── FipSoftKeys ──────────────────────────────────────────────────────────

    #[test]
    fn test_soft_keys_default_blank() {
        let keys = FipSoftKeys::new();
        for i in 0..6 {
            assert_eq!(keys.label(i), "", "key {i} should be blank");
        }
    }

    #[test]
    fn test_soft_keys_set_and_get() {
        let mut keys = FipSoftKeys::new();
        keys.set_label(0, "NAV");
        keys.set_label(5, "EXIT");
        assert_eq!(keys.label(0), "NAV");
        assert_eq!(keys.label(5), "EXIT");
        assert_eq!(keys.label(1), "");
    }

    #[test]
    fn test_soft_keys_out_of_bounds_ignored() {
        let mut keys = FipSoftKeys::new();
        keys.set_label(10, "NOPE"); // should not panic
        assert_eq!(keys.label(10), "");
    }

    #[test]
    fn test_soft_keys_labels_slice() {
        let mut keys = FipSoftKeys::new();
        keys.set_label(2, "MAP");
        let labels = keys.labels();
        assert_eq!(labels.len(), 6);
        assert_eq!(labels[2], "MAP");
    }

    // ── FipScrollWheel ───────────────────────────────────────────────────────

    #[test]
    fn test_scroll_wheel_default_zero() {
        let sw = FipScrollWheel::default();
        assert_eq!(sw.accumulated, 0);
    }

    #[test]
    fn test_scroll_wheel_accumulates_cw() {
        let mut sw = FipScrollWheel::default();
        let state = FipButtonState(1 << 6); // RotaryCw
        sw.update(&state);
        sw.update(&state);
        assert_eq!(sw.accumulated, 2);
    }

    #[test]
    fn test_scroll_wheel_accumulates_ccw() {
        let mut sw = FipScrollWheel::default();
        let state = FipButtonState(1 << 7); // RotaryCcw
        sw.update(&state);
        assert_eq!(sw.accumulated, -1);
    }

    #[test]
    fn test_scroll_wheel_drain_resets() {
        let mut sw = FipScrollWheel::default();
        sw.accumulated = 5;
        let val = sw.drain();
        assert_eq!(val, 5);
        assert_eq!(sw.accumulated, 0);
    }

    // ── FipPageManager ───────────────────────────────────────────────────────

    #[test]
    fn test_page_manager_default() {
        let pm = FipPageManager::default();
        assert_eq!(pm.current(), 0);
        assert_eq!(pm.page_count(), 6);
    }

    #[test]
    fn test_page_manager_select() {
        let mut pm = FipPageManager::new(4);
        pm.select(3);
        assert_eq!(pm.current(), 3);
        pm.select(10); // out of range, ignored
        assert_eq!(pm.current(), 3);
    }

    #[test]
    fn test_page_manager_handle_button() {
        let mut pm = FipPageManager::new(6);
        assert!(pm.handle_button(FipButton::Page3)); // switches to page 2
        assert_eq!(pm.current(), 2);
        assert!(!pm.handle_button(FipButton::Page3)); // same page, no change
        assert!(pm.handle_button(FipButton::Page1)); // back to page 0
        assert_eq!(pm.current(), 0);
    }

    #[test]
    fn test_page_manager_rotary_not_a_page() {
        let mut pm = FipPageManager::new(6);
        assert!(!pm.handle_button(FipButton::RotaryCw));
        assert_eq!(pm.current(), 0);
    }

    #[test]
    fn test_page_manager_clamps_count() {
        let pm = FipPageManager::new(0);
        assert_eq!(pm.page_count(), 1);
        let pm = FipPageManager::new(100);
        assert_eq!(pm.page_count(), 6);
    }

    // ── FipProtocol ──────────────────────────────────────────────────────────

    #[test]
    fn test_fip_protocol_metadata() {
        let proto = FipProtocol;
        assert_eq!(proto.name(), "Saitek Flight Instrument Panel");
        assert_eq!(proto.vendor_id(), FIP_VID);
        assert_eq!(proto.product_id(), FIP_PID);
        assert!(proto.led_names().is_empty());
    }

    #[test]
    fn test_fip_protocol_parse_page_button() {
        let proto = FipProtocol;
        let events = proto.parse_input(&[0b0000_0100]).unwrap(); // Page3
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PanelEvent::ButtonPress { name: "PAGE3" }))
        );
    }

    #[test]
    fn test_fip_protocol_parse_rotary() {
        let proto = FipProtocol;
        let events = proto.parse_input(&[1 << 6]).unwrap(); // RotaryCw
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::EncoderTick {
                name: "SCROLL",
                delta: 1
            }
        )));
    }

    #[test]
    fn test_fip_protocol_parse_empty_returns_none() {
        let proto = FipProtocol;
        assert!(proto.parse_input(&[]).is_none());
    }
}
