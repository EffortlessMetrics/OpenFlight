// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for Thrustmaster T.Flight HOTAS devices.
//!
//! This module handles axis and button input from standard HID reports.
//! Supports both Merged and Separate axis modes.

use flight_hid_support::device_support::{AxisMode, TFlightModel};
use flight_hid_support::ghost_filter::{GhostFilterStats, GhostInputFilter, presets};

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
    /// Auxiliary yaw axis (rocker/pedals channel) - Separate mode only
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

/// Yaw source selection policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TFlightYawPolicy {
    /// In Separate mode prefer auxiliary yaw (rocker/pedals) then fall back to twist.
    /// In Merged mode always use combined yaw.
    #[default]
    Auto,
    /// Always prefer twist yaw when available.
    Twist,
    /// Always prefer auxiliary yaw (rocker/pedals) when available.
    Aux,
}

/// Effective yaw source used to resolve logical yaw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TFlightYawSource {
    /// Combined yaw source from merged mode.
    Combined,
    /// Twist yaw source.
    Twist,
    /// Auxiliary yaw source (rocker/pedals channel).
    Aux,
}

/// Resolved logical yaw value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TFlightYawResolution {
    pub value: f32,
    pub source: TFlightYawSource,
}

/// Errors returned by checked report parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TFlightParseError {
    /// Report did not contain enough bytes for the selected/effective mode.
    #[error(
        "T.Flight report too short for {mode:?} mode: expected at least {expected} bytes, got {actual}"
    )]
    ReportTooShort {
        mode: AxisMode,
        expected: usize,
        actual: usize,
    },
}

/// Parsed input state from a T.Flight HOTAS device.
#[derive(Debug, Clone)]
pub struct TFlightInputState {
    pub axes: TFlightAxes,
    pub buttons: TFlightButtons,
    /// Effective mode used to parse this report.
    pub axis_mode: AxisMode,
}

impl Default for TFlightInputState {
    fn default() -> Self {
        Self {
            axes: TFlightAxes::default(),
            buttons: TFlightButtons::default(),
            axis_mode: AxisMode::Unknown,
        }
    }
}

impl TFlightInputState {
    /// Resolve logical yaw from parsed channels using the given policy.
    pub fn resolve_yaw(&self, policy: TFlightYawPolicy) -> TFlightYawResolution {
        if self.axis_mode == AxisMode::Merged {
            return TFlightYawResolution {
                value: self.axes.twist,
                source: TFlightYawSource::Combined,
            };
        }

        match policy {
            TFlightYawPolicy::Twist => TFlightYawResolution {
                value: self.axes.twist,
                source: TFlightYawSource::Twist,
            },
            TFlightYawPolicy::Aux | TFlightYawPolicy::Auto => {
                if let Some(aux) = self.axes.rocker {
                    TFlightYawResolution {
                        value: aux,
                        source: TFlightYawSource::Aux,
                    }
                } else {
                    TFlightYawResolution {
                        value: self.axes.twist,
                        source: TFlightYawSource::Twist,
                    }
                }
            }
        }
    }
}

/// Input handler for Thrustmaster T.Flight HOTAS devices.
#[derive(Debug)]
pub struct TFlightInputHandler {
    device_type: TFlightModel,
    /// Explicit mode selection. `Unknown` means auto-detect every report.
    axis_mode: AxisMode,
    /// Last effective mode observed while parsing.
    detected_axis_mode: AxisMode,
    ghost_filter: GhostInputFilter,
    last_state: TFlightInputState,
    /// Whether throttle should be inverted (some units report 0 = full)
    invert_throttle: bool,
    /// Yaw resolution policy.
    yaw_policy: TFlightYawPolicy,
    /// Strip a leading Report ID byte before parsing.
    ///
    /// Some HID stacks prepend a 1-byte report ID. Set this when the
    /// captured descriptor or receipt confirms the device uses Report IDs.
    has_report_id: bool,
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
            detected_axis_mode: axis_mode,
            ghost_filter,
            last_state: TFlightInputState::default(),
            invert_throttle: false,
            yaw_policy: TFlightYawPolicy::Auto,
            has_report_id: false,
        }
    }

    /// Enable throttle inversion for units where 0 = full throttle.
    pub fn with_throttle_inversion(mut self, invert: bool) -> Self {
        self.invert_throttle = invert;
        self
    }

    /// Configure yaw source policy.
    pub fn with_yaw_policy(mut self, policy: TFlightYawPolicy) -> Self {
        self.yaw_policy = policy;
        self
    }

    /// Enable Report ID stripping.
    ///
    /// Set this when the HID stack prepends a 1-byte report ID to every
    /// input report. The first byte is discarded and the remainder is
    /// parsed as the normal payload.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.has_report_id = enabled;
        self
    }

    /// Update yaw source policy.
    pub fn set_yaw_policy(&mut self, policy: TFlightYawPolicy) {
        self.yaw_policy = policy;
    }

    /// Resolve logical yaw for a parsed state using current policy.
    pub fn resolve_yaw(&self, state: &TFlightInputState) -> TFlightYawResolution {
        state.resolve_yaw(self.yaw_policy)
    }

    /// Get current yaw source policy.
    pub fn yaw_policy(&self) -> TFlightYawPolicy {
        self.yaw_policy
    }

    /// Update the axis mode.
    ///
    /// Use `AxisMode::Unknown` to enable per-report auto-detection.
    pub fn set_axis_mode(&mut self, mode: AxisMode) {
        self.axis_mode = mode;
        if mode != AxisMode::Unknown {
            self.detected_axis_mode = mode;
        }
    }

    /// Parse a raw HID report into axis and button state.
    ///
    /// This compatibility method logs parse errors and returns a default state.
    pub fn parse_report(&mut self, report: &[u8]) -> TFlightInputState {
        match self.try_parse_report(report) {
            Ok(state) => state,
            Err(error) => {
                tracing::warn!("T.Flight report parse failed: {}", error);
                TFlightInputState::default()
            }
        }
    }

    /// Parse a raw HID report into axis and button state.
    ///
    /// Returns a structured error when report bytes are invalid for the
    /// selected/effective axis mode.
    pub fn try_parse_report(
        &mut self,
        report: &[u8],
    ) -> Result<TFlightInputState, TFlightParseError> {
        // Strip leading Report ID byte if the device uses them.
        let payload = if self.has_report_id {
            report.get(1..).unwrap_or(&[])
        } else {
            report
        };
        let effective_mode = self.determine_effective_mode(payload)?;
        let state = match effective_mode {
            AxisMode::Separate => self.parse_separate_report(payload)?,
            AxisMode::Merged | AxisMode::Unknown => self.parse_merged_report(payload)?,
        };

        self.detected_axis_mode = effective_mode;
        self.last_state = state.clone();
        Ok(state)
    }

    /// Get the current ghost input detection rate.
    pub fn ghost_rate(&self) -> f64 {
        self.ghost_filter.ghost_rate()
    }

    /// Get detailed ghost filter stats for diagnostics.
    pub fn ghost_stats(&self) -> GhostFilterStats {
        self.ghost_filter.stats().clone()
    }

    /// Get the device type this handler is configured for.
    pub fn device_type(&self) -> TFlightModel {
        self.device_type
    }

    /// Get configured axis mode.
    ///
    /// Returns `AxisMode::Unknown` when auto-detect mode is enabled.
    pub fn axis_mode(&self) -> AxisMode {
        self.axis_mode
    }

    /// Get currently effective axis mode.
    pub fn current_axis_mode(&self) -> AxisMode {
        match self.axis_mode {
            AxisMode::Unknown => self.detected_axis_mode,
            explicit => explicit,
        }
    }

    fn determine_effective_mode(&self, report: &[u8]) -> Result<AxisMode, TFlightParseError> {
        match self.axis_mode {
            AxisMode::Unknown => {
                if report.len() >= 9 {
                    Ok(AxisMode::Separate)
                } else if report.len() >= 8 {
                    Ok(AxisMode::Merged)
                } else {
                    Err(TFlightParseError::ReportTooShort {
                        mode: AxisMode::Unknown,
                        expected: 8,
                        actual: report.len(),
                    })
                }
            }
            AxisMode::Separate => {
                if report.len() < 9 {
                    Err(TFlightParseError::ReportTooShort {
                        mode: AxisMode::Separate,
                        expected: 9,
                        actual: report.len(),
                    })
                } else {
                    Ok(AxisMode::Separate)
                }
            }
            AxisMode::Merged => {
                if report.len() < 8 {
                    Err(TFlightParseError::ReportTooShort {
                        mode: AxisMode::Merged,
                        expected: 8,
                        actual: report.len(),
                    })
                } else {
                    Ok(AxisMode::Merged)
                }
            }
        }
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
    fn parse_separate_report(
        &mut self,
        report: &[u8],
    ) -> Result<TFlightInputState, TFlightParseError> {
        if report.len() < 9 {
            return Err(TFlightParseError::ReportTooShort {
                mode: AxisMode::Separate,
                expected: 9,
                actual: report.len(),
            });
        }

        let mut state = TFlightInputState {
            axis_mode: AxisMode::Separate,
            ..TFlightInputState::default()
        };

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

        Ok(state)
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
    fn parse_merged_report(
        &mut self,
        report: &[u8],
    ) -> Result<TFlightInputState, TFlightParseError> {
        if report.len() < 8 {
            return Err(TFlightParseError::ReportTooShort {
                mode: AxisMode::Merged,
                expected: 8,
                actual: report.len(),
            });
        }

        let mut state = TFlightInputState {
            axis_mode: AxisMode::Merged,
            ..TFlightInputState::default()
        };

        // Parse 16-bit stick axes
        let x_raw = u16::from_le_bytes([report[0], report[1]]);
        let y_raw = u16::from_le_bytes([report[2], report[3]]);

        state.axes.roll = normalize_axis_16bit(x_raw);
        state.axes.pitch = normalize_axis_16bit(y_raw);

        // Parse 8-bit axes
        state.axes.throttle = normalize_throttle_8bit(report[4], self.invert_throttle);
        state.axes.twist = normalize_axis_8bit_centered(report[5]);
        state.axes.rocker = None; // Not available in merged mode

        // Parse buttons and HAT
        self.parse_buttons_and_hat(&mut state, &report[6..]);

        Ok(state)
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

        // HAT is in the upper nibble of the second byte.
        // Valid range is 0 (center) through 8 (8 directions); clamp anything
        // outside that range back to 0 to avoid phantom hat events.
        let raw_hat = (bytes[1] >> 4) & 0x0F;
        state.buttons.hat = if raw_hat <= 8 { raw_hat } else { 0 };
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
    use std::time::Duration;

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
        assert_eq!(handler.current_axis_mode(), AxisMode::Unknown);
        assert_eq!(handler.yaw_policy(), TFlightYawPolicy::Auto);
        assert_eq!(handler.ghost_rate(), 0.0);
    }

    #[test]
    fn test_axis_mode_detection_and_runtime_switch() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);

        // 8-byte report should be parsed as merged
        let merged_report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
        let state = handler.try_parse_report(&merged_report).unwrap();
        assert_eq!(state.axis_mode, AxisMode::Merged);
        assert!(state.axes.rocker.is_none());
        assert_eq!(handler.current_axis_mode(), AxisMode::Merged);

        // 9-byte report should be parsed as separate
        let separate_report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0xFF, 0x00, 0x00];
        let state = handler.try_parse_report(&separate_report).unwrap();
        assert_eq!(state.axis_mode, AxisMode::Separate);
        assert!(state.axes.rocker.is_some());
        assert_eq!(handler.current_axis_mode(), AxisMode::Separate);
    }

    #[test]
    fn test_manual_axis_mode_enforcement() {
        let mut handler =
            TFlightInputHandler::with_axis_mode(TFlightModel::Hotas4, AxisMode::Separate);
        let merged_report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
        let error = handler.try_parse_report(&merged_report).unwrap_err();
        assert!(matches!(
            error,
            TFlightParseError::ReportTooShort {
                mode: AxisMode::Separate,
                expected: 9,
                actual: 8
            }
        ));
    }

    #[test]
    fn test_try_parse_short_report_error() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let error = handler.try_parse_report(&[0x00, 0x80, 0x00]).unwrap_err();
        assert!(matches!(
            error,
            TFlightParseError::ReportTooShort {
                mode: AxisMode::Unknown,
                expected: 8,
                actual: 3
            }
        ));
    }

    #[test]
    fn test_parse_report_compatibility_returns_default_on_error() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let state = handler.parse_report(&[0x00, 0x80]);
        assert_eq!(state.axis_mode, AxisMode::Unknown);
        assert_eq!(state.axes.roll, 0.0);
        assert_eq!(state.buttons.buttons, 0);
    }

    #[test]
    fn test_button_and_hat_parsing() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x06, 0x71];

        // Prime debounce
        let _ = handler.parse_report(&report);
        std::thread::sleep(Duration::from_millis(35));

        let state = handler.parse_report(&report);
        assert_eq!(state.buttons.buttons, 0x0106);
        assert_eq!(state.buttons.hat, 0x07);
    }

    #[test]
    fn test_throttle_inversion_via_parser() {
        let mut handler =
            TFlightInputHandler::new(TFlightModel::Hotas4).with_throttle_inversion(true);
        let report = vec![0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00];
        let state = handler.parse_report(&report);
        assert!((state.axes.throttle - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_yaw_resolution_policies_separate_mode() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x00, 0xFF, 0x00, 0x00];
        let state = handler.try_parse_report(&report).unwrap();

        handler.set_yaw_policy(TFlightYawPolicy::Auto);
        let yaw = handler.resolve_yaw(&state);
        assert_eq!(yaw.source, TFlightYawSource::Aux);
        assert!((yaw.value - 1.0).abs() < 0.01);

        handler.set_yaw_policy(TFlightYawPolicy::Twist);
        let yaw = handler.resolve_yaw(&state);
        assert_eq!(yaw.source, TFlightYawSource::Twist);
        assert!((yaw.value - (-1.0)).abs() < 0.01);

        handler.set_yaw_policy(TFlightYawPolicy::Aux);
        let yaw = handler.resolve_yaw(&state);
        assert_eq!(yaw.source, TFlightYawSource::Aux);
        assert!((yaw.value - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_yaw_resolution_merged_mode_combined_source() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0xFF, 0x00, 0x00];
        let state = handler.try_parse_report(&report).unwrap();
        let yaw = handler.resolve_yaw(&state);
        assert_eq!(yaw.source, TFlightYawSource::Combined);
        assert!((yaw.value - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_ghost_stats_exposed() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x05, 0x00];
        let _ = handler.parse_report(&report);
        let stats = handler.ghost_stats();
        assert!(stats.total_samples >= 1);
    }

    // -----------------------------------------------------------------------
    // REQ-15 acceptance-criteria tests (canonical names referenced by spec ledger)
    // -----------------------------------------------------------------------

    /// AC-15.2 — merged-mode axes/buttons/hat decode correctly.
    #[test]
    fn test_parse_merged_report() {
        // Centered stick (0x8000 LE = ~0), half throttle (0x80), centered twist
        // (0x80), no buttons, no hat.
        let report: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let state = handler.try_parse_report(report).unwrap();

        assert_eq!(state.axis_mode, AxisMode::Merged);
        assert!(
            state.axes.rocker.is_none(),
            "merged mode should have no rocker"
        );
        assert_eq!(state.buttons.buttons, 0, "no buttons pressed");
        assert_eq!(state.buttons.hat, 0, "hat centered");
        // Roll and pitch should be approximately zero at 0x8000.
        assert!(state.axes.roll.abs() < 0.01);
        assert!(state.axes.pitch.abs() < 0.01);
    }

    /// AC-15.3 — separate-mode rocker is present; axes/buttons/hat decode correctly.
    #[test]
    fn test_parse_separate_report() {
        // Centered stick, half throttle, centered twist, centered rocker, no buttons, no hat.
        let report: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let state = handler.try_parse_report(report).unwrap();

        assert_eq!(state.axis_mode, AxisMode::Separate);
        assert!(
            state.axes.rocker.is_some(),
            "separate mode must expose rocker"
        );
        assert_eq!(state.buttons.buttons, 0);
        assert_eq!(state.buttons.hat, 0);
        assert!(state.axes.roll.abs() < 0.01);
        assert!(state.axes.pitch.abs() < 0.01);
    }

    /// AC-15.4 — axis mode updates without restart when report layout changes mid-session.
    #[test]
    fn test_runtime_mode_switch() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);

        let merged: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
        let separate: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];

        let s1 = handler.try_parse_report(merged).unwrap();
        assert_eq!(s1.axis_mode, AxisMode::Merged);
        assert_eq!(handler.current_axis_mode(), AxisMode::Merged);

        let s2 = handler.try_parse_report(separate).unwrap();
        assert_eq!(s2.axis_mode, AxisMode::Separate);
        assert_eq!(handler.current_axis_mode(), AxisMode::Separate);

        let s3 = handler.try_parse_report(merged).unwrap();
        assert_eq!(s3.axis_mode, AxisMode::Merged);
        assert_eq!(handler.current_axis_mode(), AxisMode::Merged);
    }

    /// AC-15.5 — logical yaw resolves to expected source for each policy.
    ///
    /// Separate-mode report: twist byte = 0x00 (full left), rocker byte = 0xFF (full right).
    #[test]
    fn test_yaw_policy_resolution() {
        let report: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x00, 0xFF, 0x00, 0x00];
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        let state = handler.try_parse_report(report).unwrap();
        assert_eq!(state.axis_mode, AxisMode::Separate);

        // Auto: prefers aux (rocker) when present.
        handler.set_yaw_policy(TFlightYawPolicy::Auto);
        let yaw = handler.resolve_yaw(&state);
        assert_eq!(yaw.source, TFlightYawSource::Aux);
        assert!((yaw.value - 1.0).abs() < 0.01, "aux should be ~+1.0");

        // Twist: forces twist channel.
        handler.set_yaw_policy(TFlightYawPolicy::Twist);
        let yaw = handler.resolve_yaw(&state);
        assert_eq!(yaw.source, TFlightYawSource::Twist);
        assert!((yaw.value - (-1.0)).abs() < 0.01, "twist should be ~-1.0");

        // Aux: forces aux channel.
        handler.set_yaw_policy(TFlightYawPolicy::Aux);
        let yaw = handler.resolve_yaw(&state);
        assert_eq!(yaw.source, TFlightYawSource::Aux);
    }

    // -----------------------------------------------------------------------
    // REQ-16 acceptance-criteria tests — Report ID scaffolding
    // -----------------------------------------------------------------------
    // These tests use a synthetic 0x01 Report ID prefix to validate that the
    // parser correctly strips the ID byte and decodes the payload.
    // Replace the fixture bytes with captured hardware receipts once available.

    /// AC-16.2 — Report ID prefix (0x01) is stripped before parsing a merged report.
    #[test]
    fn test_report_id_stripped_merged() {
        // 0x01 = Report ID, followed by an 8-byte merged-mode payload.
        let report: &[u8] = &[0x01, 0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4).with_report_id(true);
        let state = handler.try_parse_report(report).unwrap();
        assert_eq!(state.axis_mode, AxisMode::Merged);
        assert!(state.axes.rocker.is_none(), "merged mode has no rocker");
        assert!(state.axes.roll.abs() < 0.01, "roll should be near zero");
    }

    /// AC-16.2 — Report ID prefix (0x01) is stripped before parsing a separate report.
    #[test]
    fn test_report_id_stripped_separate() {
        // 0x01 = Report ID, followed by a 9-byte separate-mode payload.
        let report: &[u8] = &[0x01, 0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4).with_report_id(true);
        let state = handler.try_parse_report(report).unwrap();
        assert_eq!(state.axis_mode, AxisMode::Separate);
        assert!(
            state.axes.rocker.is_some(),
            "separate mode must expose rocker"
        );
        assert!(state.axes.roll.abs() < 0.01, "roll should be near zero");
    }

    /// AC-16.3 — HAT values outside 0..=8 are treated as centered (0).
    #[test]
    fn test_hat_out_of_range_clamped_to_center() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        // Upper nibble 0xA = 10 (>8) → should clamp to 0.
        let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0xA0_u8];
        let state = handler.parse_report(&report);
        assert_eq!(state.buttons.hat, 0, "out-of-range HAT must clamp to 0");
    }

    /// AC-16.3 — HAT value 8 (last valid direction) is preserved.
    #[test]
    fn test_hat_max_valid_value_preserved() {
        let mut handler = TFlightInputHandler::new(TFlightModel::Hotas4);
        // Upper nibble 0x8 = 8 (exactly valid).
        let report = vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x80_u8];
        let state = handler.parse_report(&report);
        assert_eq!(state.buttons.hat, 8, "HAT value 8 should be preserved");
    }
}
