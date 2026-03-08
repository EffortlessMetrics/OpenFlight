// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Pixel-level button image rendering for StreamDeck keys
//!
//! Produces simple RGB byte-buffer images that can be sent directly to
//! StreamDeck hardware. Supports solid colour fills and centred label text
//! using a minimal built-in bitmap font (no external dependencies).

use crate::button_manager::{ButtonConfig, ButtonDisplayState};

// ── ButtonImage ──────────────────────────────────────────────────────────────

/// A rendered button image as a raw RGB byte buffer.
#[derive(Debug, Clone)]
pub struct ButtonImage {
    pub width: u32,
    pub height: u32,
    /// RGB pixel data, length = width × height × 3.
    pub pixels: Vec<u8>,
}

impl ButtonImage {
    /// Create a blank (black) image.
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize) * (height as usize) * 3;
        Self {
            width,
            height,
            pixels: vec![0u8; len],
        }
    }

    /// Fill the entire image with a solid RGB colour.
    pub fn fill(&mut self, color: [u8; 3]) {
        for pixel in self.pixels.chunks_exact_mut(3) {
            pixel[0] = color[0];
            pixel[1] = color[1];
            pixel[2] = color[2];
        }
    }

    /// Set a single pixel. Does nothing if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 3]) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 3) as usize;
            self.pixels[idx] = color[0];
            self.pixels[idx + 1] = color[1];
            self.pixels[idx + 2] = color[2];
        }
    }

    /// Read a single pixel. Returns `None` if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 3) as usize;
            Some([self.pixels[idx], self.pixels[idx + 1], self.pixels[idx + 2]])
        } else {
            None
        }
    }

    /// Total number of bytes in the pixel buffer.
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }
}

// ── Minimal bitmap font ──────────────────────────────────────────────────────

/// Character cell size for the built-in 5×7 bitmap font.
const CHAR_WIDTH: u32 = 5;
const CHAR_HEIGHT: u32 = 7;
/// Spacing between characters.
const CHAR_SPACING: u32 = 1;

/// Very small 5×7 bitmaps for printable ASCII (space through '~').
/// Each character is 7 rows of 5 bits packed into a `u8` (MSB = leftmost).
fn glyph_data(ch: char) -> [u8; 7] {
    match ch {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ' ' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        _ => [
            0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
        ], // box
    }
}

/// Measure the pixel width of a text string using the bitmap font.
fn text_width(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    if chars == 0 {
        return 0;
    }
    chars * CHAR_WIDTH + (chars - 1) * CHAR_SPACING
}

/// Draw a string onto an image at the given top-left position.
fn draw_text(img: &mut ButtonImage, text: &str, x: u32, y: u32, color: [u8; 3]) {
    let mut cx = x;
    for ch in text.chars() {
        let glyph = glyph_data(ch.to_ascii_uppercase());
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..CHAR_WIDTH {
                if bits & (1 << (CHAR_WIDTH - 1 - col)) != 0 {
                    img.set_pixel(cx + col, y + row as u32, color);
                }
            }
        }
        cx += CHAR_WIDTH + CHAR_SPACING;
    }
}

// ── ButtonRenderer ───────────────────────────────────────────────────────────

/// Renders [`ButtonImage`] instances from button configuration and state.
pub struct ButtonRenderer {
    /// Square image dimension in pixels (e.g. 72 or 96).
    pub size: u32,
}

impl ButtonRenderer {
    /// Create a renderer for the given icon size.
    pub fn new(size: u32) -> Self {
        Self { size }
    }

    /// Render a complete button image.
    pub fn render_button(&self, config: &ButtonConfig, state: &ButtonDisplayState) -> ButtonImage {
        let mut img = ButtonImage::new(self.size, self.size);

        // 1. Fill background.
        img.fill(state.background_color);

        // 2. Draw centred label text (first line only if multi-line).
        let label = &config.label;
        let lines: Vec<&str> = label.split('\n').collect();
        let total_text_height =
            lines.len() as u32 * CHAR_HEIGHT + lines.len().saturating_sub(1) as u32 * 2;
        let start_y = self.size.saturating_sub(total_text_height) / 2;

        for (i, line) in lines.iter().enumerate() {
            let w = text_width(line);
            let x = self.size.saturating_sub(w) / 2;
            let y = start_y + i as u32 * (CHAR_HEIGHT + 2);
            draw_text(&mut img, line, x, y, state.text_color);
        }

        img
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::button_manager::ButtonAction;

    fn default_config(label: &str) -> ButtonConfig {
        ButtonConfig {
            label: label.to_string(),
            icon_path: None,
            action: ButtonAction::SimCommand("X".to_string()),
        }
    }

    fn default_display(label: &str) -> ButtonDisplayState {
        ButtonDisplayState {
            label: label.to_string(),
            background_color: [0x1A, 0x1A, 0x2E],
            text_color: [0xFF, 0xFF, 0xFF],
            icon_path: None,
            active: false,
        }
    }

    // ── ButtonImage basics ─────────────────────────────────────────

    #[test]
    fn test_image_dimensions_72() {
        let img = ButtonImage::new(72, 72);
        assert_eq!(img.width, 72);
        assert_eq!(img.height, 72);
        assert_eq!(img.byte_len(), 72 * 72 * 3);
    }

    #[test]
    fn test_image_dimensions_96() {
        let img = ButtonImage::new(96, 96);
        assert_eq!(img.width, 96);
        assert_eq!(img.height, 96);
        assert_eq!(img.byte_len(), 96 * 96 * 3);
    }

    #[test]
    fn test_image_fill() {
        let mut img = ButtonImage::new(4, 4);
        img.fill([0xFF, 0x00, 0x80]);
        for pixel in img.pixels.chunks_exact(3) {
            assert_eq!(pixel, [0xFF, 0x00, 0x80]);
        }
    }

    #[test]
    fn test_set_get_pixel() {
        let mut img = ButtonImage::new(8, 8);
        img.set_pixel(3, 5, [10, 20, 30]);
        assert_eq!(img.get_pixel(3, 5), Some([10, 20, 30]));
        assert_eq!(img.get_pixel(0, 0), Some([0, 0, 0]));
    }

    #[test]
    fn test_out_of_bounds_pixel() {
        let mut img = ButtonImage::new(4, 4);
        img.set_pixel(10, 10, [0xFF, 0xFF, 0xFF]); // should not panic
        assert_eq!(img.get_pixel(10, 10), None);
    }

    // ── Renderer output ────────────────────────────────────────────

    #[test]
    fn test_render_produces_correct_size_72() {
        let renderer = ButtonRenderer::new(72);
        let cfg = default_config("AP");
        let state = default_display("AP");
        let img = renderer.render_button(&cfg, &state);
        assert_eq!(img.width, 72);
        assert_eq!(img.height, 72);
    }

    #[test]
    fn test_render_produces_correct_size_96() {
        let renderer = ButtonRenderer::new(96);
        let cfg = default_config("HDG");
        let state = default_display("HDG");
        let img = renderer.render_button(&cfg, &state);
        assert_eq!(img.width, 96);
        assert_eq!(img.height, 96);
    }

    #[test]
    fn test_render_background_color() {
        let renderer = ButtonRenderer::new(72);
        let cfg = default_config("X");
        let state = ButtonDisplayState {
            label: "X".into(),
            background_color: [0xAA, 0xBB, 0xCC],
            text_color: [0xAA, 0xBB, 0xCC], // same as bg so all pixels are bg
            icon_path: None,
            active: false,
        };
        let img = renderer.render_button(&cfg, &state);
        // Corner pixel should be the background colour.
        assert_eq!(img.get_pixel(0, 0), Some([0xAA, 0xBB, 0xCC]));
    }

    #[test]
    fn test_render_text_pixels_present() {
        let renderer = ButtonRenderer::new(72);
        let cfg = default_config("A");
        let state = ButtonDisplayState {
            label: "A".into(),
            background_color: [0, 0, 0],
            text_color: [0xFF, 0xFF, 0xFF],
            icon_path: None,
            active: false,
        };
        let img = renderer.render_button(&cfg, &state);
        // At least some non-black pixels should exist (the rendered "A").
        let non_black = img.pixels.chunks_exact(3).any(|p| p != [0, 0, 0]);
        assert!(non_black, "rendered text should produce non-black pixels");
    }

    #[test]
    fn test_render_multiline_label() {
        let renderer = ButtonRenderer::new(96);
        let cfg = default_config("COM\nSWAP");
        let state = default_display("COM\nSWAP");
        let img = renderer.render_button(&cfg, &state);
        // Just verify it completes without panic and has correct size.
        assert_eq!(img.byte_len(), 96 * 96 * 3);
    }

    #[test]
    fn test_render_empty_label() {
        let renderer = ButtonRenderer::new(72);
        let cfg = default_config("");
        let state = default_display("");
        let img = renderer.render_button(&cfg, &state);
        // All pixels should be the background colour.
        for pixel in img.pixels.chunks_exact(3) {
            assert_eq!(pixel, [0x1A, 0x1A, 0x2E]);
        }
    }

    // ── Active / inactive state colours ────────────────────────────

    #[test]
    fn test_render_active_state_different_color() {
        let renderer = ButtonRenderer::new(72);
        let cfg = default_config("ON");

        let inactive = ButtonDisplayState {
            label: "ON".into(),
            background_color: [0, 0, 0],
            text_color: [0xFF, 0xFF, 0xFF],
            icon_path: None,
            active: false,
        };
        let active = ButtonDisplayState {
            label: "ON".into(),
            background_color: [0, 0x44, 0x22],
            text_color: [0x00, 0xFF, 0x88],
            icon_path: None,
            active: true,
        };

        let img_off = renderer.render_button(&cfg, &inactive);
        let img_on = renderer.render_button(&cfg, &active);

        // Corner pixels should differ (different backgrounds).
        assert_ne!(
            img_off.get_pixel(0, 0),
            img_on.get_pixel(0, 0),
            "active and inactive should have different backgrounds"
        );
    }

    // ── Text measurement ───────────────────────────────────────────

    #[test]
    fn test_text_width_empty() {
        assert_eq!(text_width(""), 0);
    }

    #[test]
    fn test_text_width_single_char() {
        assert_eq!(text_width("A"), CHAR_WIDTH);
    }

    #[test]
    fn test_text_width_multiple_chars() {
        // "AB" = 5 + 1 + 5 = 11
        assert_eq!(text_width("AB"), CHAR_WIDTH * 2 + CHAR_SPACING);
    }

    // ── Glyph coverage ─────────────────────────────────────────────

    #[test]
    fn test_glyph_uppercase_letters() {
        for ch in 'A'..='Z' {
            let g = glyph_data(ch);
            // At least one row should have bits set.
            assert!(g.iter().any(|&r| r != 0), "glyph for {ch} is empty");
        }
    }

    #[test]
    fn test_glyph_digits() {
        for ch in '0'..='9' {
            let g = glyph_data(ch);
            assert!(g.iter().any(|&r| r != 0), "glyph for {ch} is empty");
        }
    }

    #[test]
    fn test_unknown_char_renders_box() {
        let g = glyph_data('€');
        // The fallback is a box — all rows should have bits.
        assert!(g.iter().all(|&r| r != 0));
    }
}
