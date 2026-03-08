// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! LED zone map and backlighting control for WinWing panels.
//!
//! WinWing panels that support backlight control expose individual LEDs
//! (or groups) that can be addressed through the feature-report protocol
//! (see [`crate::protocol`]).  This module provides per-device LED zone
//! maps so callers can address LEDs by logical name rather than raw index,
//! along with a high-level [`LedController`] to manage panel state.
//!
//! # Protocol overview
//!
//! LED control uses the **Backlight** command category (`0x02`) in the
//! WinWing feature-report protocol.  Each command targets a `panel_id`
//! (assigned per-device) and a 0-based `button_index` within that panel.
//!
//! Supported sub-commands:
//! - `SetSingle` (`0x01`): intensity 0–255 for one LED
//! - `SetSingleRgb` (`0x02`): R/G/B colour for one LED (RGB-capable panels)
//! - `SetAll` (`0x03`): uniform intensity for all LEDs
//! - `SetAllRgb` (`0x04`): uniform RGB colour for all LEDs
//!
//! # RGB support
//!
//! Some newer WinWing panels (e.g. Combat Ready Panel MkII) support per-LED
//! RGB colour.  Older panels only support single-channel intensity.  The
//! [`LedZone::rgb`] field indicates whether RGB addressing is available for
//! that zone.

use crate::protocol::{
    FeatureReportFrame, build_backlight_all_command, build_backlight_all_rgb_command,
    build_backlight_single_command, build_backlight_single_rgb_command,
};

/// A single addressable LED zone on a WinWing panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedZone {
    /// Human-readable label (e.g. "MASTER ARM", "APU START").
    pub name: &'static str,
    /// 0-based LED index used in backlight feature-report commands.
    pub index: u8,
    /// `true` if this zone supports RGB colour commands.
    pub rgb: bool,
}

/// Complete LED zone map for a specific WinWing device.
#[derive(Debug, Clone)]
pub struct LedZoneMap {
    /// Device name for display purposes.
    pub device: &'static str,
    /// Panel ID used in feature-report addressing.
    pub panel_id: u8,
    /// Ordered list of LED zones.
    pub zones: &'static [LedZone],
}

// ── Combat Ready Panel LED map ────────────────────────────────────────────────

/// LED zones for the WinWing F/A-18 Combat Ready Panel (PID 0xBE05).
///
/// The panel has 30 individually backlit push-buttons arranged in 3 rows of 10.
/// ASSUMED zone ordering — verify with hardware capture.
pub static COMBAT_READY_PANEL_LEDS: LedZoneMap = LedZoneMap {
    device: "WinWing Combat Ready Panel",
    panel_id: 0x10, // ASSUMED panel ID
    zones: &[
        LedZone {
            name: "MASTER ARM",
            index: 0,
            rgb: false,
        },
        LedZone {
            name: "STORES JETT",
            index: 1,
            rgb: false,
        },
        LedZone {
            name: "EMCON",
            index: 2,
            rgb: false,
        },
        LedZone {
            name: "A/A",
            index: 3,
            rgb: false,
        },
        LedZone {
            name: "A/G",
            index: 4,
            rgb: false,
        },
        LedZone {
            name: "NAV",
            index: 5,
            rgb: false,
        },
        LedZone {
            name: "GUN",
            index: 6,
            rgb: false,
        },
        LedZone {
            name: "MSL",
            index: 7,
            rgb: false,
        },
        LedZone {
            name: "CMBT",
            index: 8,
            rgb: false,
        },
        LedZone {
            name: "FUEL DUMP",
            index: 9,
            rgb: false,
        },
        // Row 2 (indices 10–19) and Row 3 (indices 20–29) follow the same
        // pattern but without confirmed label assignment.
        LedZone {
            name: "ROW2_01",
            index: 10,
            rgb: false,
        },
        LedZone {
            name: "ROW2_02",
            index: 11,
            rgb: false,
        },
        LedZone {
            name: "ROW2_03",
            index: 12,
            rgb: false,
        },
        LedZone {
            name: "ROW2_04",
            index: 13,
            rgb: false,
        },
        LedZone {
            name: "ROW2_05",
            index: 14,
            rgb: false,
        },
        LedZone {
            name: "ROW2_06",
            index: 15,
            rgb: false,
        },
        LedZone {
            name: "ROW2_07",
            index: 16,
            rgb: false,
        },
        LedZone {
            name: "ROW2_08",
            index: 17,
            rgb: false,
        },
        LedZone {
            name: "ROW2_09",
            index: 18,
            rgb: false,
        },
        LedZone {
            name: "ROW2_10",
            index: 19,
            rgb: false,
        },
        LedZone {
            name: "ROW3_01",
            index: 20,
            rgb: false,
        },
        LedZone {
            name: "ROW3_02",
            index: 21,
            rgb: false,
        },
        LedZone {
            name: "ROW3_03",
            index: 22,
            rgb: false,
        },
        LedZone {
            name: "ROW3_04",
            index: 23,
            rgb: false,
        },
        LedZone {
            name: "ROW3_05",
            index: 24,
            rgb: false,
        },
        LedZone {
            name: "ROW3_06",
            index: 25,
            rgb: false,
        },
        LedZone {
            name: "ROW3_07",
            index: 26,
            rgb: false,
        },
        LedZone {
            name: "ROW3_08",
            index: 27,
            rgb: false,
        },
        LedZone {
            name: "ROW3_09",
            index: 28,
            rgb: false,
        },
        LedZone {
            name: "ROW3_10",
            index: 29,
            rgb: false,
        },
    ],
};

/// Look up a LED zone by name within a zone map (case-insensitive).
pub fn find_zone_by_name<'a>(map: &'a LedZoneMap, name: &str) -> Option<&'a LedZone> {
    let upper = name.to_uppercase();
    map.zones.iter().find(|z| z.name.to_uppercase() == upper)
}

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
    fn test_combat_ready_panel_led_count() {
        assert_eq!(COMBAT_READY_PANEL_LEDS.zones.len(), 30);
    }

    #[test]
    fn test_combat_ready_panel_indices_unique() {
        let indices: Vec<u8> = COMBAT_READY_PANEL_LEDS
            .zones
            .iter()
            .map(|z| z.index)
            .collect();
        let mut sorted = indices.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(indices.len(), sorted.len(), "LED indices must be unique");
    }

    #[test]
    fn test_combat_ready_panel_indices_sequential() {
        for (i, zone) in COMBAT_READY_PANEL_LEDS.zones.iter().enumerate() {
            assert_eq!(
                zone.index, i as u8,
                "zone {} has non-sequential index",
                zone.name
            );
        }
    }

    #[test]
    fn test_find_zone_by_name_found() {
        let zone = find_zone_by_name(&COMBAT_READY_PANEL_LEDS, "MASTER ARM");
        assert!(zone.is_some());
        assert_eq!(zone.unwrap().index, 0);
    }

    #[test]
    fn test_find_zone_by_name_not_found() {
        let zone = find_zone_by_name(&COMBAT_READY_PANEL_LEDS, "NONEXISTENT");
        assert!(zone.is_none());
    }

    #[test]
    fn test_led_zone_map_panel_id() {
        assert_eq!(COMBAT_READY_PANEL_LEDS.panel_id, 0x10);
    }

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
