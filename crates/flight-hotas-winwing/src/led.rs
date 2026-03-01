// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! LED backlighting control for WinWing panels.
//!
//! WinWing panels with individually-addressable LEDs use the proprietary
//! feature-report protocol (see [`crate::protocol`]) to control per-button
//! backlight intensity and RGB colour.
//!
//! This module provides a higher-level API that manages a local state buffer
//! and generates the minimal set of feature reports needed to synchronise
//! the physical LEDs with the desired state.

use crate::protocol::{
    FeatureReportFrame, build_backlight_all_command, build_backlight_all_rgb_command,
    build_backlight_single_command, build_backlight_single_rgb_command,
};

/// Maximum number of LEDs supported per panel.
pub const MAX_LEDS: usize = 64;

/// An RGB colour value for an LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const OFF: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };
    pub const RED: Self = Self { r: 255, g: 0, b: 0 };
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0 };
    pub const AMBER: Self = Self {
        r: 255,
        g: 191,
        b: 0,
    };
}

/// Per-LED state: intensity-only or full RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedState {
    /// Single-channel intensity (0–255).
    Intensity(u8),
    /// Full RGB colour.
    Rgb(RgbColor),
}

impl Default for LedState {
    fn default() -> Self {
        Self::Intensity(0)
    }
}

/// LED state buffer for a WinWing panel.
///
/// Tracks desired LED states and generates protocol commands.
#[derive(Debug, Clone)]
pub struct LedController {
    panel_id: u8,
    led_count: u8,
    state: [LedState; MAX_LEDS],
    dirty: [bool; MAX_LEDS],
}

impl LedController {
    /// Create a new controller for `led_count` LEDs on `panel_id`, all initially off.
    pub fn new(panel_id: u8, led_count: u8) -> Self {
        let led_count = led_count.min(MAX_LEDS as u8);
        Self {
            panel_id,
            led_count,
            state: [LedState::default(); MAX_LEDS],
            dirty: [false; MAX_LEDS],
        }
    }

    /// Number of LEDs managed by this controller.
    pub fn led_count(&self) -> u8 {
        self.led_count
    }

    /// Panel ID for this controller.
    pub fn panel_id(&self) -> u8 {
        self.panel_id
    }

    /// Set a single LED to an intensity value (0–255).
    ///
    /// Returns `false` if `index` is out of range.
    pub fn set_intensity(&mut self, index: u8, intensity: u8) -> bool {
        if index >= self.led_count {
            return false;
        }
        let new_state = LedState::Intensity(intensity);
        if self.state[index as usize] != new_state {
            self.state[index as usize] = new_state;
            self.dirty[index as usize] = true;
        }
        true
    }

    /// Set a single LED to an RGB colour.
    ///
    /// Returns `false` if `index` is out of range.
    pub fn set_rgb(&mut self, index: u8, color: RgbColor) -> bool {
        if index >= self.led_count {
            return false;
        }
        let new_state = LedState::Rgb(color);
        if self.state[index as usize] != new_state {
            self.state[index as usize] = new_state;
            self.dirty[index as usize] = true;
        }
        true
    }

    /// Set all LEDs to the same intensity.
    pub fn set_all_intensity(&mut self, intensity: u8) {
        for i in 0..self.led_count as usize {
            let new_state = LedState::Intensity(intensity);
            if self.state[i] != new_state {
                self.state[i] = new_state;
                self.dirty[i] = true;
            }
        }
    }

    /// Set all LEDs to the same RGB colour.
    pub fn set_all_rgb(&mut self, color: RgbColor) {
        for i in 0..self.led_count as usize {
            let new_state = LedState::Rgb(color);
            if self.state[i] != new_state {
                self.state[i] = new_state;
                self.dirty[i] = true;
            }
        }
    }

    /// Get the current state of LED `index`.
    pub fn get(&self, index: u8) -> Option<LedState> {
        if index >= self.led_count {
            return None;
        }
        Some(self.state[index as usize])
    }

    /// Returns `true` if any LED has been changed since the last flush.
    pub fn has_dirty(&self) -> bool {
        self.dirty[..self.led_count as usize].iter().any(|&d| d)
    }

    /// Count of dirty (changed) LEDs.
    pub fn dirty_count(&self) -> usize {
        self.dirty[..self.led_count as usize]
            .iter()
            .filter(|&&d| d)
            .count()
    }

    /// Generate protocol frames for all dirty LEDs, then clear dirty flags.
    ///
    /// If all LEDs are dirty and share the same state, a single "set all"
    /// command is emitted.  Otherwise, individual per-LED commands are used.
    ///
    /// Frames that fail to build (e.g. payload too large) are silently skipped.
    pub fn flush(&mut self) -> Vec<FeatureReportFrame> {
        if !self.has_dirty() {
            return vec![];
        }

        let count = self.led_count as usize;
        let all_dirty = self.dirty[..count].iter().all(|&d| d);

        // Optimisation: if all LEDs are dirty and identical, emit one command.
        if all_dirty && count > 0 {
            let first = self.state[0];
            let all_same = self.state[..count].iter().all(|&s| s == first);
            if all_same {
                let frame = match first {
                    LedState::Intensity(v) => build_backlight_all_command(self.panel_id, v),
                    LedState::Rgb(c) => {
                        build_backlight_all_rgb_command(self.panel_id, c.r, c.g, c.b)
                    }
                };
                if let Ok(f) = frame {
                    self.dirty[..count].fill(false);
                    return vec![f];
                }
                // Fall through to per-LED path without clearing dirty
            }
        }

        let mut commands = Vec::new();
        for i in 0..count {
            if self.dirty[i] {
                let cmd = match self.state[i] {
                    LedState::Intensity(v) => {
                        build_backlight_single_command(self.panel_id, i as u8, v)
                    }
                    LedState::Rgb(c) => {
                        build_backlight_single_rgb_command(self.panel_id, i as u8, c.r, c.g, c.b)
                    }
                };
                if let Ok(frame) = cmd {
                    commands.push(frame);
                }
                self.dirty[i] = false;
            }
        }
        commands
    }

    /// Reset all LEDs to off and mark them dirty.
    pub fn reset(&mut self) {
        self.set_all_intensity(0);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_controller_all_off() {
        let ctrl = LedController::new(0, 20);
        assert_eq!(ctrl.led_count(), 20);
        assert_eq!(ctrl.panel_id(), 0);
        for i in 0..20 {
            assert_eq!(ctrl.get(i), Some(LedState::Intensity(0)));
        }
    }

    #[test]
    fn test_get_out_of_range() {
        let ctrl = LedController::new(0, 5);
        assert_eq!(ctrl.get(5), None);
        assert_eq!(ctrl.get(255), None);
    }

    #[test]
    fn test_set_intensity() {
        let mut ctrl = LedController::new(0, 10);
        assert!(ctrl.set_intensity(0, 128));
        assert_eq!(ctrl.get(0), Some(LedState::Intensity(128)));
    }

    #[test]
    fn test_set_intensity_out_of_range() {
        let mut ctrl = LedController::new(0, 5);
        assert!(!ctrl.set_intensity(5, 128));
    }

    #[test]
    fn test_set_rgb() {
        let mut ctrl = LedController::new(0, 10);
        let color = RgbColor::new(255, 128, 0);
        assert!(ctrl.set_rgb(3, color));
        assert_eq!(ctrl.get(3), Some(LedState::Rgb(color)));
    }

    #[test]
    fn test_set_rgb_out_of_range() {
        let mut ctrl = LedController::new(0, 5);
        assert!(!ctrl.set_rgb(5, RgbColor::RED));
    }

    #[test]
    fn test_set_all_intensity() {
        let mut ctrl = LedController::new(0, 8);
        ctrl.set_all_intensity(200);
        for i in 0..8 {
            assert_eq!(ctrl.get(i), Some(LedState::Intensity(200)));
        }
    }

    #[test]
    fn test_set_all_rgb() {
        let mut ctrl = LedController::new(0, 8);
        ctrl.set_all_rgb(RgbColor::GREEN);
        for i in 0..8 {
            assert_eq!(ctrl.get(i), Some(LedState::Rgb(RgbColor::GREEN)));
        }
    }

    #[test]
    fn test_dirty_tracking() {
        let mut ctrl = LedController::new(0, 4);
        assert!(!ctrl.has_dirty());
        ctrl.set_intensity(0, 100);
        assert!(ctrl.has_dirty());
        assert_eq!(ctrl.dirty_count(), 1);
    }

    #[test]
    fn test_no_dirty_on_same_value() {
        let mut ctrl = LedController::new(0, 4);
        ctrl.set_intensity(0, 100);
        let _ = ctrl.flush();
        assert!(!ctrl.has_dirty());
        // Set same value again — should not dirty
        ctrl.set_intensity(0, 100);
        assert!(!ctrl.has_dirty());
    }

    #[test]
    fn test_flush_all_same_intensity_single_command() {
        let mut ctrl = LedController::new(0, 4);
        ctrl.set_all_intensity(128);
        let cmds = ctrl.flush();
        assert_eq!(cmds.len(), 1, "all-same-intensity should emit 1 command");
        assert!(!ctrl.has_dirty());
    }

    #[test]
    fn test_flush_all_same_rgb_single_command() {
        let mut ctrl = LedController::new(0, 4);
        ctrl.set_all_rgb(RgbColor::AMBER);
        let cmds = ctrl.flush();
        assert_eq!(cmds.len(), 1, "all-same-RGB should emit 1 command");
    }

    #[test]
    fn test_flush_mixed_multiple_commands() {
        let mut ctrl = LedController::new(0, 4);
        ctrl.set_intensity(0, 100);
        ctrl.set_intensity(1, 200);
        let cmds = ctrl.flush();
        assert_eq!(cmds.len(), 2, "two different LEDs → 2 commands");
    }

    #[test]
    fn test_flush_clears_dirty() {
        let mut ctrl = LedController::new(0, 4);
        ctrl.set_intensity(0, 100);
        let _ = ctrl.flush();
        assert!(!ctrl.has_dirty());
        let cmds = ctrl.flush();
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_flush_partial_dirty() {
        let mut ctrl = LedController::new(0, 4);
        ctrl.set_all_intensity(50);
        let _ = ctrl.flush();
        ctrl.set_intensity(2, 200);
        let cmds = ctrl.flush();
        assert_eq!(cmds.len(), 1);
    }

    #[test]
    fn test_reset() {
        let mut ctrl = LedController::new(0, 4);
        ctrl.set_all_intensity(255);
        let _ = ctrl.flush();
        ctrl.reset();
        for i in 0..4 {
            assert_eq!(ctrl.get(i), Some(LedState::Intensity(0)));
        }
        assert!(ctrl.has_dirty());
    }

    #[test]
    fn test_rgb_color_constants() {
        assert_eq!(RgbColor::OFF, RgbColor::new(0, 0, 0));
        assert_eq!(RgbColor::WHITE, RgbColor::new(255, 255, 255));
        assert_eq!(RgbColor::RED, RgbColor::new(255, 0, 0));
        assert_eq!(RgbColor::GREEN, RgbColor::new(0, 255, 0));
        assert_eq!(RgbColor::AMBER, RgbColor::new(255, 191, 0));
    }

    #[test]
    fn test_zero_led_controller() {
        let mut ctrl = LedController::new(0, 0);
        assert!(!ctrl.has_dirty());
        assert!(!ctrl.set_intensity(0, 100));
        assert!(ctrl.flush().is_empty());
    }
}
