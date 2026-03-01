// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! WinWing proprietary USB protocol layer.
//!
//! WinWing devices communicate over USB HID using a mix of standard
//! fixed-length input reports (handled by the per-device parser modules) and
//! a **proprietary variable-length feature-report protocol** used for:
//!
//! - **Display updates** — 7-segment and LCD panels (FCU, EFIS, TOP).
//! - **Backlighting control** — per-button RGB or single-intensity LEDs.
//! - **Detent position queries** — reading throttle detent positions.
//! - **Device identification** — firmware version and capability queries.
//!
//! # Wire format
//!
//! Variable-length feature reports use the following framing:
//!
//! ```text
//! byte 0       : Feature report ID (0xF0)
//! byte 1       : Command category
//! byte 2       : Sub-command
//! bytes 3–4    : Payload length (u16 LE)
//! bytes 5..N   : Payload (variable, up to MAX_PAYLOAD_LEN)
//! byte N+1     : Checksum (XOR of bytes 1..N)
//! ```
//!
//! # Panel display addressing
//!
//! Each display field on a panel is addressed by a `(panel_id, field_index)`
//! pair.  A single feature report can update one or more fields.

use thiserror::Error;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Feature report ID used for all WinWing proprietary commands.
pub const FEATURE_REPORT_ID: u8 = 0xF0;

/// Maximum payload length in a single feature report (bytes).
pub const MAX_PAYLOAD_LEN: usize = 56;

/// Minimum frame size: report-ID(1) + category(1) + sub-cmd(1) + len(2) + checksum(1).
pub const MIN_FRAME_LEN: usize = 6;

/// Maximum total frame size including header and checksum.
pub const MAX_FRAME_LEN: usize = MIN_FRAME_LEN + MAX_PAYLOAD_LEN;

// ── Command categories ────────────────────────────────────────────────────────

/// Command categories for WinWing feature reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandCategory {
    /// Display update commands (7-segment, LCD text).
    Display = 0x01,
    /// Backlighting / LED control.
    Backlight = 0x02,
    /// Detent position read/write.
    Detent = 0x03,
    /// Device identification / firmware query.
    DeviceInfo = 0x04,
}

impl CommandCategory {
    /// Try to convert a raw byte to a `CommandCategory`.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::Display),
            0x02 => Some(Self::Backlight),
            0x03 => Some(Self::Detent),
            0x04 => Some(Self::DeviceInfo),
            _ => None,
        }
    }
}

// ── Display sub-commands ──────────────────────────────────────────────────────

/// Sub-commands for display updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisplaySubCommand {
    /// Write ASCII text to a display field.
    WriteText = 0x01,
    /// Write raw 7-segment bitmask data.
    WriteSegment = 0x02,
    /// Set display brightness (0–255).
    SetBrightness = 0x03,
    /// Clear all display fields on a panel.
    ClearAll = 0x04,
}

impl DisplaySubCommand {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::WriteText),
            0x02 => Some(Self::WriteSegment),
            0x03 => Some(Self::SetBrightness),
            0x04 => Some(Self::ClearAll),
            _ => None,
        }
    }
}

// ── Backlight sub-commands ────────────────────────────────────────────────────

/// Sub-commands for backlighting control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BacklightSubCommand {
    /// Set a single button's backlight intensity (0–255).
    SetSingle = 0x01,
    /// Set a single button's backlight to an RGB colour.
    SetSingleRgb = 0x02,
    /// Set all buttons to the same intensity.
    SetAll = 0x03,
    /// Set all buttons to the same RGB colour.
    SetAllRgb = 0x04,
}

impl BacklightSubCommand {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::SetSingle),
            0x02 => Some(Self::SetSingleRgb),
            0x03 => Some(Self::SetAll),
            0x04 => Some(Self::SetAllRgb),
            _ => None,
        }
    }
}

// ── Detent sub-commands ───────────────────────────────────────────────────────

/// Sub-commands for throttle detent queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DetentSubCommand {
    /// Query current detent positions (response contains positions).
    QueryPositions = 0x01,
    /// Set custom detent position for a lever.
    SetPosition = 0x02,
}

impl DetentSubCommand {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::QueryPositions),
            0x02 => Some(Self::SetPosition),
            _ => None,
        }
    }
}

// ── Detent types ──────────────────────────────────────────────────────────────

/// Named detent positions on a WinWing throttle lever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetentName {
    /// Idle / cut-off position.
    Idle,
    /// Afterburner / full military power.
    Afterburner,
    /// Custom detent at a user-defined position.
    Custom(u8),
}

/// A single detent position reported by the device.
#[derive(Debug, Clone, PartialEq)]
pub struct DetentPosition {
    /// Which lever (0 = left, 1 = right for dual throttle).
    pub lever: u8,
    /// Detent identifier.
    pub name: DetentName,
    /// Raw axis value at the detent (0–65535).
    pub raw_position: u16,
    /// Normalised position \[0.0, 1.0\].
    pub normalised: f32,
}

/// A set of detent positions returned by a detent query response.
#[derive(Debug, Clone, PartialEq)]
pub struct DetentReport {
    pub positions: Vec<DetentPosition>,
}

// ── Frame builder ─────────────────────────────────────────────────────────────

/// A constructed WinWing feature report frame ready to send to the device.
#[derive(Debug, Clone)]
pub struct FeatureReportFrame {
    buf: [u8; MAX_FRAME_LEN],
    len: usize,
}

impl FeatureReportFrame {
    /// Build a frame from components.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::PayloadTooLarge`] if `payload` exceeds
    /// [`MAX_PAYLOAD_LEN`].
    pub fn new(
        category: CommandCategory,
        sub_cmd: u8,
        payload: &[u8],
    ) -> Result<Self, ProtocolError> {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge {
                max: MAX_PAYLOAD_LEN,
                got: payload.len(),
            });
        }

        let total = MIN_FRAME_LEN + payload.len();
        let mut buf = [0u8; MAX_FRAME_LEN];
        buf[0] = FEATURE_REPORT_ID;
        buf[1] = category as u8;
        buf[2] = sub_cmd;
        let plen = payload.len() as u16;
        buf[3..5].copy_from_slice(&plen.to_le_bytes());
        buf[5..5 + payload.len()].copy_from_slice(payload);

        // XOR checksum over bytes 1..total-1
        let mut cksum: u8 = 0;
        for &b in &buf[1..total - 1] {
            cksum ^= b;
        }
        buf[total - 1] = cksum;

        Ok(Self { buf, len: total })
    }

    /// The raw bytes of the frame.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Total frame length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the frame is empty (should never happen for valid frames).
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// ── Frame parser ──────────────────────────────────────────────────────────────

/// Parsed header from a WinWing feature report.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFrame<'a> {
    pub category: CommandCategory,
    pub sub_command: u8,
    pub payload: &'a [u8],
}

/// Parse a raw feature report frame.
///
/// # Errors
///
/// Returns an appropriate [`ProtocolError`] if the frame is malformed.
pub fn parse_feature_report(data: &[u8]) -> Result<ParsedFrame<'_>, ProtocolError> {
    if data.len() < MIN_FRAME_LEN {
        return Err(ProtocolError::FrameTooShort {
            need: MIN_FRAME_LEN,
            got: data.len(),
        });
    }
    if data[0] != FEATURE_REPORT_ID {
        return Err(ProtocolError::InvalidReportId { id: data[0] });
    }

    let category = CommandCategory::from_byte(data[1])
        .ok_or(ProtocolError::UnknownCategory { byte: data[1] })?;
    let sub_command = data[2];
    let payload_len = u16::from_le_bytes([data[3], data[4]]) as usize;

    let expected_total = MIN_FRAME_LEN + payload_len;
    if data.len() < expected_total {
        return Err(ProtocolError::FrameTooShort {
            need: expected_total,
            got: data.len(),
        });
    }
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(ProtocolError::PayloadTooLarge {
            max: MAX_PAYLOAD_LEN,
            got: payload_len,
        });
    }

    // Verify checksum
    let mut cksum: u8 = 0;
    for &b in &data[1..expected_total - 1] {
        cksum ^= b;
    }
    if cksum != data[expected_total - 1] {
        return Err(ProtocolError::ChecksumMismatch {
            expected: cksum,
            got: data[expected_total - 1],
        });
    }

    let payload = &data[5..5 + payload_len];
    Ok(ParsedFrame {
        category,
        sub_command,
        payload,
    })
}

// ── Display command builders ──────────────────────────────────────────────────

/// Build a display-text feature report.
///
/// Writes ASCII `text` to the given `panel_id` and `field_index`.
/// Text is truncated to 16 characters.
///
/// # Errors
///
/// Returns [`ProtocolError::PayloadTooLarge`] on internal overflow (should not
/// happen with the 16-char truncation).
pub fn build_display_text_command(
    panel_id: u8,
    field_index: u8,
    text: &str,
) -> Result<FeatureReportFrame, ProtocolError> {
    let bytes = text.as_bytes();
    let truncated = if bytes.len() > 16 { &bytes[..16] } else { bytes };
    // payload: panel_id(1) + field_index(1) + text bytes
    let mut payload = Vec::with_capacity(2 + truncated.len());
    payload.push(panel_id);
    payload.push(field_index);
    payload.extend_from_slice(truncated);
    FeatureReportFrame::new(
        CommandCategory::Display,
        DisplaySubCommand::WriteText as u8,
        &payload,
    )
}

/// Build a 7-segment raw-write feature report.
///
/// Each byte in `segments` is a raw bitmask for one digit.
///
/// # Errors
///
/// Returns [`ProtocolError::PayloadTooLarge`] if `segments` is too long.
pub fn build_display_segment_command(
    panel_id: u8,
    field_index: u8,
    segments: &[u8],
) -> Result<FeatureReportFrame, ProtocolError> {
    let mut payload = Vec::with_capacity(2 + segments.len());
    payload.push(panel_id);
    payload.push(field_index);
    payload.extend_from_slice(segments);
    FeatureReportFrame::new(
        CommandCategory::Display,
        DisplaySubCommand::WriteSegment as u8,
        &payload,
    )
}

/// Build a display-brightness feature report.
pub fn build_display_brightness_command(
    panel_id: u8,
    brightness: u8,
) -> Result<FeatureReportFrame, ProtocolError> {
    FeatureReportFrame::new(
        CommandCategory::Display,
        DisplaySubCommand::SetBrightness as u8,
        &[panel_id, brightness],
    )
}

/// Build a clear-all-displays feature report.
pub fn build_display_clear_command(panel_id: u8) -> Result<FeatureReportFrame, ProtocolError> {
    FeatureReportFrame::new(
        CommandCategory::Display,
        DisplaySubCommand::ClearAll as u8,
        &[panel_id],
    )
}

// ── Backlight command builders ────────────────────────────────────────────────

/// Build a single-button backlight intensity command.
///
/// `button_index` is 0-based. `intensity` is 0 (off) to 255 (full).
pub fn build_backlight_single_command(
    panel_id: u8,
    button_index: u8,
    intensity: u8,
) -> Result<FeatureReportFrame, ProtocolError> {
    FeatureReportFrame::new(
        CommandCategory::Backlight,
        BacklightSubCommand::SetSingle as u8,
        &[panel_id, button_index, intensity],
    )
}

/// Build a single-button RGB backlight command.
pub fn build_backlight_single_rgb_command(
    panel_id: u8,
    button_index: u8,
    r: u8,
    g: u8,
    b: u8,
) -> Result<FeatureReportFrame, ProtocolError> {
    FeatureReportFrame::new(
        CommandCategory::Backlight,
        BacklightSubCommand::SetSingleRgb as u8,
        &[panel_id, button_index, r, g, b],
    )
}

/// Build an all-buttons backlight intensity command.
pub fn build_backlight_all_command(
    panel_id: u8,
    intensity: u8,
) -> Result<FeatureReportFrame, ProtocolError> {
    FeatureReportFrame::new(
        CommandCategory::Backlight,
        BacklightSubCommand::SetAll as u8,
        &[panel_id, intensity],
    )
}

/// Build an all-buttons RGB backlight command.
pub fn build_backlight_all_rgb_command(
    panel_id: u8,
    r: u8,
    g: u8,
    b: u8,
) -> Result<FeatureReportFrame, ProtocolError> {
    FeatureReportFrame::new(
        CommandCategory::Backlight,
        BacklightSubCommand::SetAllRgb as u8,
        &[panel_id, r, g, b],
    )
}

// ── Detent command builders ───────────────────────────────────────────────────

/// Build a detent query command.
pub fn build_detent_query_command() -> Result<FeatureReportFrame, ProtocolError> {
    FeatureReportFrame::new(
        CommandCategory::Detent,
        DetentSubCommand::QueryPositions as u8,
        &[],
    )
}

/// Build a detent set-position command.
///
/// `lever`: 0 = left, 1 = right.
/// `detent_id`: 0 = idle, 1 = afterburner, 2+ = custom.
/// `position`: raw 16-bit axis value.
pub fn build_detent_set_command(
    lever: u8,
    detent_id: u8,
    position: u16,
) -> Result<FeatureReportFrame, ProtocolError> {
    let pos_bytes = position.to_le_bytes();
    FeatureReportFrame::new(
        CommandCategory::Detent,
        DetentSubCommand::SetPosition as u8,
        &[lever, detent_id, pos_bytes[0], pos_bytes[1]],
    )
}

// ── Detent response parser ────────────────────────────────────────────────────

/// Parse a detent query response payload into a [`DetentReport`].
///
/// The payload format is a sequence of 5-byte entries:
///
/// ```text
/// byte 0     : lever index (0 = left, 1 = right)
/// byte 1     : detent ID (0 = idle, 1 = afterburner, 2+ = custom)
/// bytes 2–3  : raw position (u16 LE)
/// byte 4     : reserved
/// ```
///
/// # Errors
///
/// Returns [`ProtocolError::InvalidDetentPayload`] if the payload length is
/// not a multiple of 5.
pub fn parse_detent_response(payload: &[u8]) -> Result<DetentReport, ProtocolError> {
    const ENTRY_SIZE: usize = 5;
    if !payload.is_empty() && !payload.len().is_multiple_of(ENTRY_SIZE) {
        return Err(ProtocolError::InvalidDetentPayload { len: payload.len() });
    }

    let mut positions = Vec::with_capacity(payload.len() / ENTRY_SIZE);
    for chunk in payload.chunks_exact(ENTRY_SIZE) {
        let lever = chunk[0];
        let detent_id = chunk[1];
        let raw = u16::from_le_bytes([chunk[2], chunk[3]]);
        let normalised = raw as f32 / 65535.0;
        let name = match detent_id {
            0 => DetentName::Idle,
            1 => DetentName::Afterburner,
            n => DetentName::Custom(n),
        };
        positions.push(DetentPosition {
            lever,
            name,
            raw_position: raw,
            normalised,
        });
    }

    Ok(DetentReport { positions })
}

// ── Device identification ─────────────────────────────────────────────────────

/// Known WinWing device types identified by USB Product ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceType {
    /// Orion HOTAS stick (F/A-18 inspired, 3 axes, 20+ buttons, 2 HATs).
    OrionStick,
    /// Orion HOTAS throttle (dual throttle, 6 axes, 40+ buttons).
    OrionThrottle,
    /// Orion 2 stick with improved sensors.
    Orion2Stick,
    /// Orion 2 throttle with improved sensors.
    Orion2Throttle,
    /// Super Taurus / Libra throttle quadrant (6+ axes, 32 buttons).
    SuperTaurus,
    /// F-18 UFC / IFEI panel (button-only, display segments).
    F18Panel,
    /// F-16EX Grip (stick grip, buttons, HATs, 10-byte report).
    F16ExGrip,
    /// TFRP rudder pedals (3 axes).
    TfrpRudder,
    /// Skywalker metal rudder pedals.
    SkywalkerRudder,
}

impl DeviceType {
    /// Detect the device type from a USB Product ID.
    ///
    /// Returns `None` if the PID is not recognised.
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            0xBE60 => Some(Self::OrionStick),
            0xBE61 => Some(Self::OrionThrottle),
            0xBE63 => Some(Self::Orion2Stick),
            0xBE62 => Some(Self::Orion2Throttle),
            0xBD64 => Some(Self::SuperTaurus),
            0xBEDE => Some(Self::F18Panel),
            0xBEA8 => Some(Self::F16ExGrip),
            0xBE64 => Some(Self::TfrpRudder),
            0xBEF0 => Some(Self::SkywalkerRudder),
            _ => None,
        }
    }

    /// Returns the expected HID input report length for this device.
    pub fn report_length(&self) -> usize {
        match self {
            Self::OrionStick | Self::Orion2Stick => 12,
            Self::OrionThrottle | Self::Orion2Throttle => 24,
            Self::SuperTaurus => 13,
            Self::F18Panel => 6,
            Self::F16ExGrip => 10,
            Self::TfrpRudder => 8,
            Self::SkywalkerRudder => 8,
        }
    }

    /// Returns `true` if this device supports LED / backlight control.
    pub fn has_leds(&self) -> bool {
        matches!(
            self,
            Self::OrionThrottle
                | Self::Orion2Throttle
                | Self::SuperTaurus
                | Self::F18Panel
        )
    }

    /// Returns `true` if this device has a display (UFC/IFEI/ICP).
    pub fn has_display(&self) -> bool {
        matches!(self, Self::F18Panel)
    }

    /// Returns `true` if this device supports throttle detent configuration.
    pub fn has_detents(&self) -> bool {
        matches!(
            self,
            Self::OrionThrottle | Self::Orion2Throttle | Self::SuperTaurus
        )
    }
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OrionStick => f.write_str("WinWing Orion Stick"),
            Self::OrionThrottle => f.write_str("WinWing Orion Throttle"),
            Self::Orion2Stick => f.write_str("WinWing Orion 2 Stick"),
            Self::Orion2Throttle => f.write_str("WinWing Orion 2 Throttle"),
            Self::SuperTaurus => f.write_str("WinWing Super Taurus"),
            Self::F18Panel => f.write_str("WinWing F-18 Panel"),
            Self::F16ExGrip => f.write_str("WinWing F-16EX Grip"),
            Self::TfrpRudder => f.write_str("WinWing TFRP Rudder"),
            Self::SkywalkerRudder => f.write_str("WinWing Skywalker Rudder"),
        }
    }
}

/// High-level protocol handler for a detected WinWing device.
///
/// Wraps device identification and provides convenience accessors
/// for the device's capabilities.
#[derive(Debug, Clone)]
pub struct WinWingProtocol {
    device: DeviceType,
    pid: u16,
}

impl WinWingProtocol {
    /// Create a protocol handler by detecting the device from its PID.
    ///
    /// Returns `None` if the PID is not a known WinWing device.
    pub fn from_pid(pid: u16) -> Option<Self> {
        DeviceType::from_pid(pid).map(|device| Self { device, pid })
    }

    /// The detected device type.
    pub fn device_type(&self) -> DeviceType {
        self.device
    }

    /// The USB Product ID.
    pub fn pid(&self) -> u16 {
        self.pid
    }

    /// Expected input report length for this device.
    pub fn report_length(&self) -> usize {
        self.device.report_length()
    }

    /// Whether this device supports LED control.
    pub fn has_leds(&self) -> bool {
        self.device.has_leds()
    }

    /// Whether this device has a display panel.
    pub fn has_display(&self) -> bool {
        self.device.has_display()
    }

    /// Whether this device supports detent configuration.
    pub fn has_detents(&self) -> bool {
        self.device.has_detents()
    }
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors from the WinWing proprietary protocol layer.
#[derive(Debug, Error, PartialEq)]
pub enum ProtocolError {
    #[error("frame too short: need {need} bytes, got {got}")]
    FrameTooShort { need: usize, got: usize },

    #[error("invalid feature report ID: 0x{id:02X}")]
    InvalidReportId { id: u8 },

    #[error("unknown command category: 0x{byte:02X}")]
    UnknownCategory { byte: u8 },

    #[error("payload too large: max {max} bytes, got {got}")]
    PayloadTooLarge { max: usize, got: usize },

    #[error("checksum mismatch: expected 0x{expected:02X}, got 0x{got:02X}")]
    ChecksumMismatch { expected: u8, got: u8 },

    #[error("invalid detent payload length: {len} bytes (not a multiple of 5)")]
    InvalidDetentPayload { len: usize },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Frame round-trip ──────────────────────────────────────────────────

    #[test]
    fn test_build_and_parse_empty_payload() {
        let frame = FeatureReportFrame::new(CommandCategory::DeviceInfo, 0x01, &[]).unwrap();
        let bytes = frame.as_bytes();
        assert_eq!(bytes.len(), MIN_FRAME_LEN);
        assert_eq!(bytes[0], FEATURE_REPORT_ID);
        assert_eq!(bytes[1], CommandCategory::DeviceInfo as u8);
        assert_eq!(bytes[2], 0x01);
        // payload length = 0
        assert_eq!(u16::from_le_bytes([bytes[3], bytes[4]]), 0);

        let parsed = parse_feature_report(bytes).unwrap();
        assert_eq!(parsed.category, CommandCategory::DeviceInfo);
        assert_eq!(parsed.sub_command, 0x01);
        assert!(parsed.payload.is_empty());
    }

    #[test]
    fn test_build_and_parse_with_payload() {
        let payload = [0xAA, 0xBB, 0xCC];
        let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &payload).unwrap();
        let bytes = frame.as_bytes();
        assert_eq!(bytes.len(), MIN_FRAME_LEN + 3);

        let parsed = parse_feature_report(bytes).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, 0x01);
        assert_eq!(parsed.payload, &payload);
    }

    #[test]
    fn test_checksum_verification() {
        let frame =
            FeatureReportFrame::new(CommandCategory::Backlight, 0x02, &[0x10, 0x20]).unwrap();
        let mut bytes = frame.as_bytes().to_vec();
        // Corrupt checksum
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let err = parse_feature_report(&bytes).unwrap_err();
        assert!(matches!(err, ProtocolError::ChecksumMismatch { .. }));
    }

    #[test]
    fn test_payload_too_large() {
        let big = [0u8; MAX_PAYLOAD_LEN + 1];
        let err = FeatureReportFrame::new(CommandCategory::Display, 0x01, &big).unwrap_err();
        assert_eq!(
            err,
            ProtocolError::PayloadTooLarge {
                max: MAX_PAYLOAD_LEN,
                got: MAX_PAYLOAD_LEN + 1
            }
        );
    }

    #[test]
    fn test_max_payload_succeeds() {
        let payload = [0xABu8; MAX_PAYLOAD_LEN];
        let frame = FeatureReportFrame::new(CommandCategory::Display, 0x02, &payload).unwrap();
        assert_eq!(frame.len(), MAX_FRAME_LEN);
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.payload.len(), MAX_PAYLOAD_LEN);
    }

    #[test]
    fn test_frame_too_short() {
        let err = parse_feature_report(&[0xF0, 0x01]).unwrap_err();
        assert!(matches!(err, ProtocolError::FrameTooShort { .. }));
    }

    #[test]
    fn test_invalid_report_id() {
        let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[]).unwrap();
        let mut bytes = frame.as_bytes().to_vec();
        bytes[0] = 0xAA;
        let err = parse_feature_report(&bytes).unwrap_err();
        assert_eq!(err, ProtocolError::InvalidReportId { id: 0xAA });
    }

    #[test]
    fn test_unknown_category() {
        let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[]).unwrap();
        let mut bytes = frame.as_bytes().to_vec();
        bytes[1] = 0xFF; // bad category
        let err = parse_feature_report(&bytes).unwrap_err();
        assert_eq!(err, ProtocolError::UnknownCategory { byte: 0xFF });
    }

    #[test]
    fn test_empty_frame() {
        let err = parse_feature_report(&[]).unwrap_err();
        assert_eq!(
            err,
            ProtocolError::FrameTooShort {
                need: MIN_FRAME_LEN,
                got: 0
            }
        );
    }

    // ── Display commands ──────────────────────────────────────────────────

    #[test]
    fn test_display_text_command() {
        let frame = build_display_text_command(0x01, 0x00, "12345").unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::WriteText as u8);
        assert_eq!(parsed.payload[0], 0x01); // panel_id
        assert_eq!(parsed.payload[1], 0x00); // field_index
        assert_eq!(&parsed.payload[2..], b"12345");
    }

    #[test]
    fn test_display_text_truncation() {
        let long = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let frame = build_display_text_command(0x01, 0x00, long).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        // text payload should be 16 chars max
        assert_eq!(parsed.payload.len(), 2 + 16);
        assert_eq!(&parsed.payload[2..], b"ABCDEFGHIJKLMNOP");
    }

    #[test]
    fn test_display_segment_command() {
        let segs = [0x7F, 0x06, 0x5B, 0x4F, 0x66]; // "01234" in 7-seg
        let frame = build_display_segment_command(0x02, 0x00, &segs).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::WriteSegment as u8);
        assert_eq!(parsed.payload[0], 0x02);
        assert_eq!(parsed.payload[1], 0x00);
        assert_eq!(&parsed.payload[2..], &segs);
    }

    #[test]
    fn test_display_brightness_command() {
        let frame = build_display_brightness_command(0x03, 128).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::SetBrightness as u8);
        assert_eq!(parsed.payload, &[0x03, 128]);
    }

    #[test]
    fn test_display_clear_command() {
        let frame = build_display_clear_command(0x01).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::ClearAll as u8);
        assert_eq!(parsed.payload, &[0x01]);
    }

    // ── Backlight commands ────────────────────────────────────────────────

    #[test]
    fn test_backlight_single_command() {
        let frame = build_backlight_single_command(0x01, 5, 200).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Backlight);
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetSingle as u8);
        assert_eq!(parsed.payload, &[0x01, 5, 200]);
    }

    #[test]
    fn test_backlight_single_rgb_command() {
        let frame = build_backlight_single_rgb_command(0x01, 3, 255, 128, 0).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Backlight);
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetSingleRgb as u8);
        assert_eq!(parsed.payload, &[0x01, 3, 255, 128, 0]);
    }

    #[test]
    fn test_backlight_all_command() {
        let frame = build_backlight_all_command(0x02, 100).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Backlight);
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetAll as u8);
        assert_eq!(parsed.payload, &[0x02, 100]);
    }

    #[test]
    fn test_backlight_all_rgb_command() {
        let frame = build_backlight_all_rgb_command(0x02, 0, 255, 0).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Backlight);
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetAllRgb as u8);
        assert_eq!(parsed.payload, &[0x02, 0, 255, 0]);
    }

    #[test]
    fn test_backlight_off() {
        let frame = build_backlight_single_command(0x01, 0, 0).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.payload[2], 0); // intensity = off
    }

    #[test]
    fn test_backlight_full_intensity() {
        let frame = build_backlight_single_command(0x01, 0, 255).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.payload[2], 255);
    }

    // ── Detent commands ───────────────────────────────────────────────────

    #[test]
    fn test_detent_query_command() {
        let frame = build_detent_query_command().unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Detent);
        assert_eq!(parsed.sub_command, DetentSubCommand::QueryPositions as u8);
        assert!(parsed.payload.is_empty());
    }

    #[test]
    fn test_detent_set_command() {
        let frame = build_detent_set_command(0, 0, 16384).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Detent);
        assert_eq!(parsed.sub_command, DetentSubCommand::SetPosition as u8);
        assert_eq!(parsed.payload[0], 0); // lever
        assert_eq!(parsed.payload[1], 0); // detent_id (idle)
        let pos = u16::from_le_bytes([parsed.payload[2], parsed.payload[3]]);
        assert_eq!(pos, 16384);
    }

    #[test]
    fn test_detent_set_right_lever_afterburner() {
        let frame = build_detent_set_command(1, 1, 60000).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.payload[0], 1); // right lever
        assert_eq!(parsed.payload[1], 1); // afterburner
        let pos = u16::from_le_bytes([parsed.payload[2], parsed.payload[3]]);
        assert_eq!(pos, 60000);
    }

    // ── Detent response parsing ───────────────────────────────────────────

    #[test]
    fn test_parse_detent_response_empty() {
        let report = parse_detent_response(&[]).unwrap();
        assert!(report.positions.is_empty());
    }

    #[test]
    fn test_parse_detent_response_single() {
        // lever=0, detent_id=0(idle), position=16384(LE), reserved=0
        let pos_bytes = 16384u16.to_le_bytes();
        let payload = [0, 0, pos_bytes[0], pos_bytes[1], 0];
        let report = parse_detent_response(&payload).unwrap();
        assert_eq!(report.positions.len(), 1);
        assert_eq!(report.positions[0].lever, 0);
        assert_eq!(report.positions[0].name, DetentName::Idle);
        assert_eq!(report.positions[0].raw_position, 16384);
        assert!((report.positions[0].normalised - 16384.0 / 65535.0).abs() < 1e-5);
    }

    #[test]
    fn test_parse_detent_response_multiple() {
        let idle_pos = 8000u16.to_le_bytes();
        let ab_pos = 58000u16.to_le_bytes();
        let custom_pos = 32000u16.to_le_bytes();
        let payload = [
            0,
            0,
            idle_pos[0],
            idle_pos[1],
            0, // left idle
            0,
            1,
            ab_pos[0],
            ab_pos[1],
            0, // left afterburner
            1,
            0,
            idle_pos[0],
            idle_pos[1],
            0, // right idle
            1,
            2,
            custom_pos[0],
            custom_pos[1],
            0, // right custom #2
        ];
        let report = parse_detent_response(&payload).unwrap();
        assert_eq!(report.positions.len(), 4);
        assert_eq!(report.positions[0].name, DetentName::Idle);
        assert_eq!(report.positions[1].name, DetentName::Afterburner);
        assert_eq!(report.positions[2].lever, 1);
        assert_eq!(report.positions[3].name, DetentName::Custom(2));
    }

    #[test]
    fn test_parse_detent_response_bad_length() {
        let err = parse_detent_response(&[0, 0, 0]).unwrap_err();
        assert_eq!(err, ProtocolError::InvalidDetentPayload { len: 3 });
    }

    #[test]
    fn test_detent_normalised_min() {
        let payload = [0, 0, 0, 0, 0]; // position = 0
        let report = parse_detent_response(&payload).unwrap();
        assert!(report.positions[0].normalised.abs() < 1e-6);
    }

    #[test]
    fn test_detent_normalised_max() {
        let max_pos = 65535u16.to_le_bytes();
        let payload = [0, 1, max_pos[0], max_pos[1], 0];
        let report = parse_detent_response(&payload).unwrap();
        assert!((report.positions[0].normalised - 1.0).abs() < 1e-5);
    }

    // ── Command category conversions ──────────────────────────────────────

    #[test]
    fn test_command_category_roundtrip() {
        for (byte, cat) in [
            (0x01, CommandCategory::Display),
            (0x02, CommandCategory::Backlight),
            (0x03, CommandCategory::Detent),
            (0x04, CommandCategory::DeviceInfo),
        ] {
            assert_eq!(CommandCategory::from_byte(byte), Some(cat));
            assert_eq!(cat as u8, byte);
        }
    }

    #[test]
    fn test_command_category_unknown() {
        assert_eq!(CommandCategory::from_byte(0x00), None);
        assert_eq!(CommandCategory::from_byte(0x05), None);
        assert_eq!(CommandCategory::from_byte(0xFF), None);
    }

    #[test]
    fn test_display_sub_command_roundtrip() {
        for (byte, cmd) in [
            (0x01, DisplaySubCommand::WriteText),
            (0x02, DisplaySubCommand::WriteSegment),
            (0x03, DisplaySubCommand::SetBrightness),
            (0x04, DisplaySubCommand::ClearAll),
        ] {
            assert_eq!(DisplaySubCommand::from_byte(byte), Some(cmd));
        }
        assert_eq!(DisplaySubCommand::from_byte(0x00), None);
    }

    #[test]
    fn test_backlight_sub_command_roundtrip() {
        for (byte, cmd) in [
            (0x01, BacklightSubCommand::SetSingle),
            (0x02, BacklightSubCommand::SetSingleRgb),
            (0x03, BacklightSubCommand::SetAll),
            (0x04, BacklightSubCommand::SetAllRgb),
        ] {
            assert_eq!(BacklightSubCommand::from_byte(byte), Some(cmd));
        }
        assert_eq!(BacklightSubCommand::from_byte(0x00), None);
    }

    #[test]
    fn test_detent_sub_command_roundtrip() {
        assert_eq!(
            DetentSubCommand::from_byte(0x01),
            Some(DetentSubCommand::QueryPositions)
        );
        assert_eq!(
            DetentSubCommand::from_byte(0x02),
            Some(DetentSubCommand::SetPosition)
        );
        assert_eq!(DetentSubCommand::from_byte(0x00), None);
    }

    // ── Variable-length handling ──────────────────────────────────────────

    #[test]
    fn test_variable_length_payloads() {
        // Verify various payload sizes work correctly
        for len in [0, 1, 2, 5, 10, 20, MAX_PAYLOAD_LEN] {
            let payload: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &payload).unwrap();
            assert_eq!(frame.len(), MIN_FRAME_LEN + len);

            let parsed = parse_feature_report(frame.as_bytes()).unwrap();
            assert_eq!(parsed.payload, payload.as_slice());
        }
    }

    #[test]
    fn test_frame_is_empty() {
        let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[]).unwrap();
        assert!(!frame.is_empty());
        assert!(frame.len() >= MIN_FRAME_LEN);
    }

    // ── Truncated frame with declared longer payload ──────────────────────

    #[test]
    fn test_frame_declared_payload_exceeds_data() {
        // Build a valid frame, then truncate it
        let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[1, 2, 3]).unwrap();
        let bytes = frame.as_bytes();
        // Only give first 7 bytes (header + 1 byte of payload, missing rest + checksum)
        let err = parse_feature_report(&bytes[..7]).unwrap_err();
        assert!(matches!(err, ProtocolError::FrameTooShort { .. }));
    }

    // ── Device detection ──────────────────────────────────────────────────

    #[test]
    fn test_device_type_from_pid_orion2_throttle() {
        assert_eq!(
            DeviceType::from_pid(0xBE62),
            Some(DeviceType::Orion2Throttle)
        );
    }

    #[test]
    fn test_device_type_from_pid_orion2_stick() {
        assert_eq!(DeviceType::from_pid(0xBE63), Some(DeviceType::Orion2Stick));
    }

    #[test]
    fn test_device_type_from_pid_super_taurus() {
        assert_eq!(DeviceType::from_pid(0xBD64), Some(DeviceType::SuperTaurus));
    }

    #[test]
    fn test_device_type_from_pid_f18_panel() {
        assert_eq!(DeviceType::from_pid(0xBEDE), Some(DeviceType::F18Panel));
    }

    #[test]
    fn test_device_type_from_pid_f16_grip() {
        assert_eq!(DeviceType::from_pid(0xBEA8), Some(DeviceType::F16ExGrip));
    }

    #[test]
    fn test_device_type_from_pid_tfrp_rudder() {
        assert_eq!(DeviceType::from_pid(0xBE64), Some(DeviceType::TfrpRudder));
    }

    #[test]
    fn test_device_type_from_pid_skywalker() {
        assert_eq!(
            DeviceType::from_pid(0xBEF0),
            Some(DeviceType::SkywalkerRudder)
        );
    }

    #[test]
    fn test_device_type_from_pid_orion_stick() {
        assert_eq!(DeviceType::from_pid(0xBE60), Some(DeviceType::OrionStick));
    }

    #[test]
    fn test_device_type_from_pid_orion_throttle() {
        assert_eq!(
            DeviceType::from_pid(0xBE61),
            Some(DeviceType::OrionThrottle)
        );
    }

    #[test]
    fn test_device_type_from_pid_unknown() {
        assert_eq!(DeviceType::from_pid(0x0000), None);
        assert_eq!(DeviceType::from_pid(0xFFFF), None);
    }

    #[test]
    fn test_device_type_has_leds() {
        assert!(DeviceType::Orion2Throttle.has_leds());
        assert!(DeviceType::SuperTaurus.has_leds());
        assert!(DeviceType::F18Panel.has_leds());
        assert!(!DeviceType::Orion2Stick.has_leds());
        assert!(!DeviceType::TfrpRudder.has_leds());
    }

    #[test]
    fn test_device_type_has_display() {
        assert!(DeviceType::F18Panel.has_display());
        assert!(!DeviceType::F16ExGrip.has_display());
        assert!(!DeviceType::Orion2Throttle.has_display());
        assert!(!DeviceType::SuperTaurus.has_display());
    }

    #[test]
    fn test_device_type_has_detents() {
        assert!(DeviceType::OrionThrottle.has_detents());
        assert!(DeviceType::Orion2Throttle.has_detents());
        assert!(DeviceType::SuperTaurus.has_detents());
        assert!(!DeviceType::Orion2Stick.has_detents());
        assert!(!DeviceType::F18Panel.has_detents());
    }

    #[test]
    fn test_device_type_report_length() {
        assert_eq!(DeviceType::Orion2Stick.report_length(), 12);
        assert_eq!(DeviceType::Orion2Throttle.report_length(), 24);
        assert_eq!(DeviceType::SuperTaurus.report_length(), 13);
        assert_eq!(DeviceType::F18Panel.report_length(), 6);
        assert_eq!(DeviceType::F16ExGrip.report_length(), 10);
    }

    #[test]
    fn test_device_type_display_names() {
        assert_eq!(
            DeviceType::Orion2Throttle.to_string(),
            "WinWing Orion 2 Throttle"
        );
        assert_eq!(DeviceType::F18Panel.to_string(), "WinWing F-18 Panel");
        assert_eq!(DeviceType::SuperTaurus.to_string(), "WinWing Super Taurus");
    }

    #[test]
    fn test_winwing_protocol_from_pid() {
        let proto = WinWingProtocol::from_pid(0xBE62).unwrap();
        assert_eq!(proto.device_type(), DeviceType::Orion2Throttle);
        assert_eq!(proto.pid(), 0xBE62);
        assert!(proto.has_leds());
        assert!(proto.has_detents());
        assert!(!proto.has_display());
    }

    #[test]
    fn test_winwing_protocol_from_pid_unknown() {
        assert!(WinWingProtocol::from_pid(0x0000).is_none());
    }

    #[test]
    fn test_winwing_protocol_f18_panel() {
        let proto = WinWingProtocol::from_pid(0xBEDE).unwrap();
        assert_eq!(proto.device_type(), DeviceType::F18Panel);
        assert!(proto.has_leds());
        assert!(proto.has_display());
        assert!(!proto.has_detents());
        assert_eq!(proto.report_length(), 6);
    }
}
