// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for Saitek HOTAS devices.
//!
//! This module handles axis and button input from standard HID reports.
//! Unlike output protocols, input uses standard HID and is well-understood.

use flight_hid_support::ghost_filter::GhostInputFilter;
use flight_hid_support::ghost_filter::presets;
use flight_hid_support::saitek_hotas::SaitekHotasType;

/// Axis values normalized to -1.0..1.0 range.
#[derive(Debug, Clone, Default)]
pub struct HotasAxes {
    /// Main stick X axis (roll)
    pub stick_x: f32,
    /// Main stick Y axis (pitch)
    pub stick_y: f32,
    /// Stick twist axis (yaw/rudder)
    pub stick_twist: f32,
    /// Main throttle axis
    pub throttle: f32,
    /// Secondary throttle axis (X55/X56)
    pub throttle2: f32,
    /// Rotary 1
    pub rotary1: f32,
    /// Rotary 2
    pub rotary2: f32,
    /// Mini-stick X
    pub mini_x: f32,
    /// Mini-stick Y
    pub mini_y: f32,
    /// Slider
    pub slider: f32,
}

/// Button state as a bitmask.
#[derive(Debug, Clone, Default)]
pub struct HotasButtons {
    /// Primary button mask (buttons 1-32)
    pub primary: u32,
    /// Secondary button mask (buttons 33-64, for devices with many buttons)
    pub secondary: u32,
    /// HAT/POV switch states
    pub hats: u8,
}

/// Parsed input state from a HOTAS device.
#[derive(Debug, Clone, Default)]
pub struct HotasInputState {
    pub axes: HotasAxes,
    pub buttons: HotasButtons,
}

/// Input handler for Saitek HOTAS devices.
#[derive(Debug)]
pub struct HotasInputHandler {
    device_type: SaitekHotasType,
    ghost_filter: GhostInputFilter,
    last_state: HotasInputState,
}

impl HotasInputHandler {
    /// Create a new input handler for the specified device type.
    pub fn new(device_type: SaitekHotasType) -> Self {
        // Use appropriate ghost filter preset based on device
        let ghost_filter = match device_type {
            SaitekHotasType::X55Stick
            | SaitekHotasType::X55Throttle
            | SaitekHotasType::X56Stick
            | SaitekHotasType::X56Throttle => {
                GhostInputFilter::with_config(presets::x55_x56_ministick())
            }
            _ => GhostInputFilter::new(),
        };

        Self {
            device_type,
            ghost_filter,
            last_state: HotasInputState::default(),
        }
    }

    /// Parse a raw HID report into axis and button state.
    ///
    /// # Arguments
    ///
    /// * `report` - Raw HID input report bytes
    ///
    /// # Returns
    ///
    /// Parsed input state with normalized axes and filtered buttons.
    pub fn parse_report(&mut self, report: &[u8]) -> HotasInputState {
        // Parse based on device type
        // NOTE: Actual report format depends on HID descriptor
        // This is a placeholder that should be refined based on captured descriptors
        let state = match self.device_type {
            SaitekHotasType::X52 | SaitekHotasType::X52Pro => self.parse_x52_report(report),
            SaitekHotasType::X55Stick | SaitekHotasType::X56Stick => {
                self.parse_x55_x56_stick_report(report)
            }
            SaitekHotasType::X55Throttle | SaitekHotasType::X56Throttle => {
                self.parse_x55_x56_throttle_report(report)
            }
        };

        self.last_state = state.clone();
        state
    }

    /// Get the current ghost input detection rate.
    pub fn ghost_rate(&self) -> f64 {
        self.ghost_filter.ghost_rate()
    }

    /// Get the device type this handler is configured for.
    pub fn device_type(&self) -> SaitekHotasType {
        self.device_type
    }

    fn parse_x52_report(&mut self, report: &[u8]) -> HotasInputState {
        let mut state = HotasInputState::default();

        if report.len() < 14 {
            tracing::warn!("X52 report too short: {} bytes", report.len());
            return state;
        }

        // X52/X52 Pro report format (hypothesis - needs verification):
        // Bytes 0-1: X axis (11-bit)
        // Bytes 2-3: Y axis (11-bit)
        // Bytes 4-5: Twist (10-bit)
        // Byte 6: Throttle (8-bit)
        // Bytes 7-10: Buttons
        // Remaining: Rotaries, slider, etc.

        // Parse axes with normalization
        // NOTE: Bit depth is UNVERIFIED - may be 10 or 11 bit
        let x_raw = u16::from_le_bytes([report[0], report[1] & 0x07]) & 0x7FF;
        let y_raw = u16::from_le_bytes([report[2], report[3] & 0x07]) & 0x7FF;

        state.axes.stick_x = normalize_axis_11bit(x_raw);
        state.axes.stick_y = normalize_axis_11bit(y_raw);

        // Throttle is typically 8-bit
        state.axes.throttle = normalize_axis_8bit(report[6]);

        // Parse buttons with ghost filtering
        let raw_buttons = u32::from_le_bytes([report[7], report[8], report[9], report[10]]);
        state.buttons.primary = self.ghost_filter.filter(raw_buttons);

        state
    }

    fn parse_x55_x56_stick_report(&mut self, report: &[u8]) -> HotasInputState {
        let mut state = HotasInputState::default();

        if report.len() < 8 {
            tracing::warn!("X55/X56 stick report too short: {} bytes", report.len());
            return state;
        }

        // X55/X56 stick report format (hypothesis - needs verification):
        // 16-bit axes, multiple buttons

        let x_raw = u16::from_le_bytes([report[0], report[1]]);
        let y_raw = u16::from_le_bytes([report[2], report[3]]);

        state.axes.stick_x = normalize_axis_16bit(x_raw);
        state.axes.stick_y = normalize_axis_16bit(y_raw);

        // Parse buttons with ghost filtering
        if report.len() >= 12 {
            let raw_buttons = u32::from_le_bytes([report[8], report[9], report[10], report[11]]);
            state.buttons.primary = self.ghost_filter.filter(raw_buttons);
        }

        state
    }

    fn parse_x55_x56_throttle_report(&mut self, report: &[u8]) -> HotasInputState {
        let mut state = HotasInputState::default();

        if report.len() < 8 {
            tracing::warn!("X55/X56 throttle report too short: {} bytes", report.len());
            return state;
        }

        // X55/X56 throttle report format (hypothesis - needs verification):
        // Dual throttle axes, rotaries, mini-stick, many buttons

        let throttle1 = u16::from_le_bytes([report[0], report[1]]);
        let throttle2 = u16::from_le_bytes([report[2], report[3]]);

        state.axes.throttle = normalize_axis_16bit(throttle1);
        state.axes.throttle2 = normalize_axis_16bit(throttle2);

        // Parse buttons with ghost filtering
        if report.len() >= 12 {
            let raw_buttons = u32::from_le_bytes([report[8], report[9], report[10], report[11]]);
            state.buttons.primary = self.ghost_filter.filter(raw_buttons);
        }

        state
    }
}

/// Normalize an 8-bit axis value to -1.0..1.0 range.
fn normalize_axis_8bit(raw: u8) -> f32 {
    (raw as f32 / 127.5) - 1.0
}

/// Normalize an 11-bit axis value to -1.0..1.0 range.
fn normalize_axis_11bit(raw: u16) -> f32 {
    (raw as f32 / 1023.5) - 1.0
}

/// Normalize a 16-bit axis value to -1.0..1.0 range.
fn normalize_axis_16bit(raw: u16) -> f32 {
    (raw as f32 / 32767.5) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_8bit() {
        assert!((normalize_axis_8bit(0) - (-1.0)).abs() < 0.01);
        assert!((normalize_axis_8bit(127) - 0.0).abs() < 0.01);
        assert!((normalize_axis_8bit(255) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_normalize_16bit() {
        assert!((normalize_axis_16bit(0) - (-1.0)).abs() < 0.001);
        assert!((normalize_axis_16bit(32767) - 0.0).abs() < 0.001);
        assert!((normalize_axis_16bit(65535) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_input_handler_creation() {
        let handler = HotasInputHandler::new(SaitekHotasType::X52Pro);
        assert_eq!(handler.device_type(), SaitekHotasType::X52Pro);
        assert_eq!(handler.ghost_rate(), 0.0);
    }
}
