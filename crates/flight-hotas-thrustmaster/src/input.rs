// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for Thrustmaster T.Flight HOTAS devices.
//!
//! This module handles axis and button input from standard HID reports.
//! Supports both Merged and Separate axis modes.

use flight_hid_support::device_support::{AxisMode, TFlightModel};
use flight_hid_support::ghost_filter::GhostInputFilter;
use flight_hid_support::ghost_filter::presets;

/// Axis values normalized to -1.0..1.0 range.
#[derive(Debug, Clone, Default)]
pub struct TFlightAxes {
    /// Roll axis (X) - stick left/right
    pub roll: f32,
    /// Pitch axis (Y) - stick forward/back
    pub pitch: f32,
    /// Throttle axis - 0.0 (idle) to 1.0 (full)
    pub throttle: f32,
    /// Twist axis (Rz) - stick twist for yaw
    pub twist: f32,
    /// Rocker axis - only present in Separate mode
    pub rocker: Option<f32>,
}

/// Button state as a bitmask.
#[derive(Debug, Clone, Default)]
pub struct TFlightButtons {
    /// Button bitmask (buttons 1-12)
    pub buttons: u16,
    /// HAT switch position (0-8, 0 = centered, 1-8 = directions)
    pub hat: u8,
}

/// Parsed input state from a T.Flight HOTAS device.
#[derive(Debug, Clone, Default)]
pub struct TFlightInputState {
    pub axes: TFlightAxes,
    pub buttons: TFlightButtons,
}

/// Input handler for Thrustmaster T.Flight HOTAS devices.
#[derive(Debug)]
pub struct TFlightInputHandler {
    device_type: TFlightModel,
    axis_mode: AxisMode,
    ghost_filter: GhostInputFilter,
    last_state: TFlightInputState,
    /// Whether throttle should be inverted (some units report 0 = full)
    invert_throttle: bool,
}

impl TFlightInputHandler {
    /// Create a new input handler for the specified device type.
    pub fn new(device_type: TFlightModel) -> Self {
        Self::with_axis_mode(device_type, AxisMode::Unknown)
    }

    /// Create a new input handler with a known axis mode.
    pub fn with_axis_mode(device_type: TFlightModel, axis_mode: AxisMode) -> Self {
        // Use T.Flight specific ghost filter preset
        let ghost_filter = GhostInputFilter::with_config(presets::tflight_hotas4());

        Self {
            device_type,
            axis_mode,
            ghost_filter,
            last_state: TFlightInputState::default(),
            invert_throttle: false,
        }
    }

    /// Enable throttle inversion for units where 0 = full throttle.
    pub fn with_throttle_inversion(mut self, invert: bool) -> Self {
        self.invert_throttle = invert;
        self
    }

    /// Update the axis mode (can change at runtime via PS/Guide button).
    pub fn set_axis_mode(&mut self, mode: AxisMode) {
        self.axis_mode = mode;
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
    pub fn parse_report(&mut self, report: &[u8]) -> TFlightInputState {
        // Auto-detect axis mode from report size if unknown
        let effective_mode = match self.axis_mode {
            AxisMode::Unknown => {
                if report.len() >= 9 {
                    AxisMode::Separate
                } else {
                    AxisMode::Merged
                }
            }
            mode => mode,
        };

        let state = match effective_mode {
            AxisMode::Separate => self.parse_separate_report(report),
            AxisMode::Merged | AxisMode::Unknown => self.parse_merged_report(report),
        };

        self.last_state = state.clone();
        state
    }

    /// Get the current ghost input detection rate.
    pub fn ghost_rate(&self) -> f64 {
        self.ghost_filter.ghost_rate()
    }

    /// Get the device type this handler is configured for.
    pub fn device_type(&self) -> TFlightModel {
        self.device_type
    }

    /// Get the current axis mode.
    pub fn axis_mode(&self) -> AxisMode {
        self.axis_mode
    }

    /// Parse report in Separate axis mode (9+ bytes).
    ///
    /// Report structure (Separate mode):
    /// | Byte(s) | Content          | Notes                    |
    /// |---------|------------------|--------------------------|
    /// | 0-1     | X axis (16-bit)  | Roll (little-endian)     |
    /// | 2-3     | Y axis (16-bit)  | Pitch (little-endian)    |
    /// | 4       | Throttle (8-bit) | May be inverted          |
    /// | 5       | Twist Rz (8-bit) | Yaw                      |
    /// | 6       | Rocker (8-bit)   | Separate mode only       |
    /// | 7-8     | Buttons + HAT    | Button mask and HAT      |
    fn parse_separate_report(&mut self, report: &[u8]) -> TFlightInputState {
        let mut state = TFlightInputState::default();

        if report.len() < 9 {
            tracing::warn!(
                "T.Flight Separate mode report too short: {} bytes",
                report.len()
            );
            return state;
        }

        // Parse 16-bit stick axes
        let x_raw = u16::from_le_bytes([report[0], report[1]]);
        let y_raw = u16::from_le_bytes([report[2], report[3]]);

        state.axes.roll = normalize_axis_16bit(x_raw);
        state.axes.pitch = normalize_axis_16bit(y_raw);

        // Parse 8-bit axes
        state.axes.throttle = normalize_throttle_8bit(report[4], self.invert_throttle);
        state.axes.twist = normalize_axis_8bit_centered(report[5]);
        state.axes.rocker = Some(normalize_axis_8bit_centered(report[6]));

        // Parse buttons and HAT
        self.parse_buttons_and_hat(&mut state, &report[7..]);

        state
    }

    /// Parse report in Merged axis mode (8 bytes).
    ///
    /// Report structure (Merged mode):
    /// | Byte(s) | Content          | Notes                    |
    /// |---------|------------------|--------------------------|
    /// | 0-1     | X axis (16-bit)  | Roll (little-endian)     |
    /// | 2-3     | Y axis (16-bit)  | Pitch (little-endian)    |
    /// | 4       | Throttle (8-bit) | May be inverted          |
    /// | 5       | Rz combined      | Twist+Rocker combined    |
    /// | 6-7     | Buttons + HAT    | Button mask and HAT      |
    fn parse_merged_report(&mut self, report: &[u8]) -> TFlightInputState {
        let mut state = TFlightInputState::default();

        if report.len() < 8 {
            tracing::warn!(
                "T.Flight Merged mode report too short: {} bytes",
                report.len()
            );
            return state;
        }

        // Parse 16-bit stick axes
        let x_raw = u16::from_le_bytes([report[0], report[1]]);
        let y_raw = u16::from_le_bytes([report[2], report[3]]);

        state.axes.roll = normalize_axis_16bit(x_raw);
        state.axes.pitch = normalize_axis_16bit(y_raw);

        // Parse 8-bit axes
        state.axes.throttle = normalize_throttle_8bit(report[4], self.invert_throttle);
        state.axes.twist = normalize_axis_8bit_centered(report[5]);
        state.axes.rocker = None; // Not available in Merged mode

        // Parse buttons and HAT
        self.parse_buttons_and_hat(&mut state, &report[6..]);

        state
    }

    /// Parse button mask and HAT switch from report bytes.
    fn parse_buttons_and_hat(&mut self, state: &mut TFlightInputState, bytes: &[u8]) {
        if bytes.len() < 2 {
            return;
        }

        // Buttons are in the lower bits
        let raw_buttons = u16::from_le_bytes([bytes[0], bytes[1] & 0x0F]);

        // Apply ghost filtering (cast to u32 for filter, then back)
        let filtered = self.ghost_filter.filter(raw_buttons as u32);
        state.buttons.buttons = filtered as u16;

        // HAT is typically in the upper nibble of the second byte
        state.buttons.hat = (bytes[1] >> 4) & 0x0F;
    }
}

/// Normalize a 16-bit axis value to -1.0..1.0 range.
fn normalize_axis_16bit(raw: u16) -> f32 {
    (raw as f32 / 32767.5) - 1.0
}

/// Normalize an 8-bit axis value (centered at 128) to -1.0..1.0 range.
fn normalize_axis_8bit_centered(raw: u8) -> f32 {
    (raw as f32 / 127.5) - 1.0
}

/// Normalize an 8-bit throttle value to 0.0..1.0 range.
fn normalize_throttle_8bit(raw: u8, invert: bool) -> f32 {
    let normalized = raw as f32 / 255.0;
    if invert { 1.0 - normalized } else { normalized }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_16bit() {
        assert!((normalize_axis_16bit(0) - (-1.0)).abs() < 0.001);
        assert!((normalize_axis_16bit(32767) - 0.0).abs() < 0.001);
        assert!((normalize_axis_16bit(65535) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_normalize_8bit_centered() {
        assert!((normalize_axis_8bit_centered(0) - (-1.0)).abs() < 0.01);
        assert!((normalize_axis_8bit_centered(127) - 0.0).abs() < 0.01);
        assert!((normalize_axis_8bit_centered(255) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_normalize_throttle() {
        // Normal
        assert!((normalize_throttle_8bit(0, false) - 0.0).abs() < 0.01);
        assert!((normalize_throttle_8bit(255, false) - 1.0).abs() < 0.01);

        // Inverted
        assert!((normalize_throttle_8bit(0, true) - 1.0).abs() < 0.01);
        assert!((normalize_throttle_8bit(255, true) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_input_handler_creation() {
        let handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        assert_eq!(handler.device_type(), TFlightModel::Hotas4);
        assert_eq!(handler.axis_mode(), AxisMode::Unknown);
        assert_eq!(handler.ghost_rate(), 0.0);
    }

    #[test]
    fn test_axis_mode_detection() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);

        // 8-byte report should be parsed as Merged
        let merged_report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
        let state = handler.parse_report(&merged_report);
        assert!(state.axes.rocker.is_none());

        // 9-byte report should be parsed as Separate
        let separate_report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];
        let state = handler.parse_report(&separate_report);
        assert!(state.axes.rocker.is_some());
    }
}
