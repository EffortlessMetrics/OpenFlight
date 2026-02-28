// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Input parsing and virtual-controller aggregation for VKB STECS and Gladiator NXT EVO.

use flight_hid_support::device_support::{VkbGladiatorVariant, VkbStecsVariant};

/// Number of buttons exposed by one STECS virtual controller (VC).
pub const STECS_BUTTONS_PER_VIRTUAL_CONTROLLER: usize = 32;
/// Maximum number of virtual controllers exposed by STECS firmware.
pub const STECS_MAX_VIRTUAL_CONTROLLERS: usize = 3;
/// Maximum merged button capacity (VC0..VC2).
pub const STECS_MAX_BUTTONS: usize =
    STECS_BUTTONS_PER_VIRTUAL_CONTROLLER * STECS_MAX_VIRTUAL_CONTROLLERS;

/// Parsed STECS axes from one interface report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StecsAxes {
    /// RX axis (SpaceBrake on baseline maps).
    pub rx: f32,
    /// RY axis (Laser Power on baseline maps).
    pub ry: f32,
    /// X axis.
    pub x: f32,
    /// Y axis.
    pub y: f32,
    /// Z axis (main throttle in most profiles).
    pub z: f32,
}

/// Parsed state from one STECS virtual-controller interface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StecsInterfaceState {
    /// Byte length of the parsed payload.
    pub report_len: usize,
    /// Optional axes block if present in this interface report.
    pub axes: Option<StecsAxes>,
    /// Button bits local to this VC (`1..=32`).
    pub buttons: u32,
}

/// Merged STECS state across VC0..VC2 for one physical throttle.
#[derive(Debug, Clone, PartialEq)]
pub struct StecsInputState {
    /// Device variant used by this merged state.
    pub variant: VkbStecsVariant,
    /// Selected axes block (typically from VC0 when available).
    pub axes: Option<StecsAxes>,
    /// Global button bitmap (`1..=96`) represented as fixed bool slots.
    pub buttons: [bool; STECS_MAX_BUTTONS],
    /// Virtual controllers that contributed in the current merge cycle.
    pub active_virtual_controllers: [bool; STECS_MAX_VIRTUAL_CONTROLLERS],
}

impl StecsInputState {
    fn new(variant: VkbStecsVariant) -> Self {
        Self {
            variant,
            axes: None,
            buttons: [false; STECS_MAX_BUTTONS],
            active_virtual_controllers: [false; STECS_MAX_VIRTUAL_CONTROLLERS],
        }
    }

    /// Return 1-based pressed button indices (`1..=96`).
    pub fn pressed_buttons(&self) -> Vec<u16> {
        self.buttons
            .iter()
            .enumerate()
            .filter_map(|(index, pressed)| {
                if *pressed {
                    Some((index + 1) as u16)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Errors returned by STECS report parsing/aggregation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StecsParseError {
    /// Report does not have enough payload bytes for the expected decode.
    #[error("STECS report too short: expected at least {expected} bytes, got {actual}")]
    ReportTooShort { expected: usize, actual: usize },
    /// Virtual-controller index is outside supported range.
    #[error("STECS virtual controller index out of range: {index}")]
    VirtualControllerOutOfRange { index: u8 },
}

/// Best-effort parser for one STECS interface report.
#[derive(Debug, Clone, Copy)]
pub struct StecsInputHandler {
    variant: VkbStecsVariant,
    has_report_id: bool,
}

impl StecsInputHandler {
    /// Create a parser for one STECS variant.
    pub fn new(variant: VkbStecsVariant) -> Self {
        Self {
            variant,
            has_report_id: false,
        }
    }

    /// Enable stripping a 1-byte HID report ID prefix.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.has_report_id = enabled;
        self
    }

    /// Return associated variant.
    pub fn variant(&self) -> VkbStecsVariant {
        self.variant
    }

    /// Parse one interface report.
    ///
    /// Layout (best effort):
    /// - If payload is at least 14 bytes: first 10 bytes are five u16 axes
    ///   (`rx, ry, x, y, z`), next 4 bytes are button bits.
    /// - If payload is 4..13 bytes: first 4 bytes are button bits only.
    pub fn parse_interface_report(
        &self,
        report: &[u8],
    ) -> Result<StecsInterfaceState, StecsParseError> {
        let payload = if self.has_report_id {
            report.get(1..).unwrap_or(&[])
        } else {
            report
        };

        if payload.len() < 4 {
            return Err(StecsParseError::ReportTooShort {
                expected: 4,
                actual: payload.len(),
            });
        }

        if payload.len() >= 14 {
            let axes = StecsAxes {
                rx: normalize_axis_16bit(le_u16(payload, 0)),
                ry: normalize_axis_16bit(le_u16(payload, 2)),
                x: normalize_axis_16bit(le_u16(payload, 4)),
                y: normalize_axis_16bit(le_u16(payload, 6)),
                z: normalize_axis_16bit(le_u16(payload, 8)),
            };
            let buttons = le_u32(payload, 10);
            return Ok(StecsInterfaceState {
                report_len: payload.len(),
                axes: Some(axes),
                buttons,
            });
        }

        let buttons = le_u32(payload, 0);
        Ok(StecsInterfaceState {
            report_len: payload.len(),
            axes: None,
            buttons,
        })
    }
}

/// Stateful VC aggregator for one physical STECS unit.
#[derive(Debug, Clone)]
pub struct StecsInputAggregator {
    handler: StecsInputHandler,
    state: StecsInputState,
    axes_source_vc: Option<u8>,
}

impl StecsInputAggregator {
    /// Create an aggregator for the given variant.
    pub fn new(variant: VkbStecsVariant) -> Self {
        Self {
            handler: StecsInputHandler::new(variant),
            state: StecsInputState::new(variant),
            axes_source_vc: None,
        }
    }

    /// Enable Report ID stripping in the underlying parser.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.handler = self.handler.with_report_id(enabled);
        self
    }

    /// Variant associated with this aggregator.
    pub fn variant(&self) -> VkbStecsVariant {
        self.handler.variant()
    }

    /// Reset merge state for a new poll tick.
    pub fn begin_poll(&mut self) {
        self.state.axes = None;
        self.state.buttons = [false; STECS_MAX_BUTTONS];
        self.state.active_virtual_controllers = [false; STECS_MAX_VIRTUAL_CONTROLLERS];
        self.axes_source_vc = None;
    }

    /// Parse and merge one interface report for `VC{virtual_controller_index}`.
    pub fn merge_interface_report(
        &mut self,
        virtual_controller_index: u8,
        report: &[u8],
    ) -> Result<(), StecsParseError> {
        let interface_state = self.handler.parse_interface_report(report)?;
        self.merge_interface_state(virtual_controller_index, interface_state)
    }

    /// Merge one already parsed interface state for `VC{virtual_controller_index}`.
    pub fn merge_interface_state(
        &mut self,
        virtual_controller_index: u8,
        interface_state: StecsInterfaceState,
    ) -> Result<(), StecsParseError> {
        let vc_index = usize::from(virtual_controller_index);
        if vc_index >= STECS_MAX_VIRTUAL_CONTROLLERS {
            return Err(StecsParseError::VirtualControllerOutOfRange {
                index: virtual_controller_index,
            });
        }

        self.state.active_virtual_controllers[vc_index] = true;

        if let Some(axes) = interface_state.axes {
            let replace_axes = match self.axes_source_vc {
                None => true,
                Some(current_source) => virtual_controller_index < current_source,
            };
            if replace_axes {
                self.state.axes = Some(axes);
                self.axes_source_vc = Some(virtual_controller_index);
            }
        }

        let base = vc_index * STECS_BUTTONS_PER_VIRTUAL_CONTROLLER;
        for bit_index in 0..STECS_BUTTONS_PER_VIRTUAL_CONTROLLER {
            if ((interface_state.buttons >> bit_index) & 1) != 0 {
                self.state.buttons[base + bit_index] = true;
            }
        }

        Ok(())
    }

    /// Return the current merged state snapshot.
    pub fn snapshot(&self) -> StecsInputState {
        self.state.clone()
    }

    /// Borrow the current merged state.
    pub fn state(&self) -> &StecsInputState {
        &self.state
    }
}

/// Maximum number of buttons supported by the Gladiator NXT EVO grip.
pub const GLADIATOR_MAX_BUTTONS: usize = 64;
/// Maximum number of POV hats on the Gladiator NXT EVO.
pub const GLADIATOR_MAX_HATS: usize = 2;

/// Parsed axes from one Gladiator NXT EVO report.
///
/// Axes are normalised to `0.0..=1.0` for unidirectional controls
/// (throttle wheel, mini-stick analogue) and `−1.0..=1.0` for
/// bidirectional stick axes (roll, pitch, yaw).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GladiatorAxes {
    /// Main stick roll (X axis), `−1.0..=1.0`.
    pub roll: f32,
    /// Main stick pitch (Y axis), `−1.0..=1.0`.
    pub pitch: f32,
    /// Stick twist / yaw (Z axis), `−1.0..=1.0`.
    pub yaw: f32,
    /// Throttle wheel on base, `0.0..=1.0`.
    pub throttle: f32,
    /// Mini-stick analogue X axis, `−1.0..=1.0`.
    pub mini_x: f32,
    /// Mini-stick analogue Y axis, `−1.0..=1.0`.
    pub mini_y: f32,
}

/// 8-direction POV hat state (matches HID hat-switch encoding 0=N … 7=NW,
/// `None` = centred / released).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HatDirection(pub u8);

/// Parsed state from one Gladiator NXT EVO HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct GladiatorInputState {
    /// Device variant.
    pub variant: VkbGladiatorVariant,
    /// All six analogue axes.
    pub axes: GladiatorAxes,
    /// Up to 64 digital buttons (`true` = pressed).
    pub buttons: [bool; GLADIATOR_MAX_BUTTONS],
    /// POV hat states (`None` = centred).
    pub hats: [Option<HatDirection>; GLADIATOR_MAX_HATS],
}

impl GladiatorInputState {
    fn new(variant: VkbGladiatorVariant) -> Self {
        Self {
            variant,
            axes: GladiatorAxes::default(),
            buttons: [false; GLADIATOR_MAX_BUTTONS],
            hats: [None; GLADIATOR_MAX_HATS],
        }
    }

    /// Return 1-based indices of all currently pressed buttons.
    pub fn pressed_buttons(&self) -> Vec<u16> {
        self.buttons
            .iter()
            .enumerate()
            .filter_map(
                |(i, &pressed)| {
                    if pressed { Some((i + 1) as u16) } else { None }
                },
            )
            .collect()
    }
}

/// Parse errors for Gladiator reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GladiatorParseError {
    /// Report payload is shorter than the minimum required size.
    #[error("Gladiator report too short: expected at least {expected} bytes, got {actual}")]
    ReportTooShort { expected: usize, actual: usize },
}

/// Best-effort parser for VKB Gladiator NXT EVO HID reports.
///
/// ## Expected HID report layout (best effort, descriptor-first preferred)
///
/// | Bytes | Content |
/// |-------|---------|
/// | 0–1   | Roll / X axis (u16 LE, 0 … 65535 → −1..=1) |
/// | 2–3   | Pitch / Y axis (u16 LE) |
/// | 4–5   | Yaw / Z (twist) axis (u16 LE) |
/// | 6–7   | Mini-stick X / Rx axis (u16 LE) |
/// | 8–9   | Mini-stick Y / Ry axis (u16 LE) |
/// | 10–11 | Throttle wheel / Slider (u16 LE, 0 … 65535 → 0..=1) |
/// | 12–15 | Button bitmap (u32 LE, bits 0–31) |
/// | 16–19 | Button bitmap (u32 LE, bits 32–63) |
/// | 20    | Hat 0 nibble (low) + Hat 1 nibble (high), 0xF = centred |
///
/// Shorter reports are parsed best-effort: missing fields default to centre/zero.
#[derive(Debug, Clone, Copy)]
pub struct GladiatorInputHandler {
    variant: VkbGladiatorVariant,
    has_report_id: bool,
}

impl GladiatorInputHandler {
    /// Create a parser for the given Gladiator variant.
    pub fn new(variant: VkbGladiatorVariant) -> Self {
        Self {
            variant,
            has_report_id: false,
        }
    }

    /// Enable stripping a 1-byte HID report ID prefix before parsing.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.has_report_id = enabled;
        self
    }

    /// Return the associated variant.
    pub fn variant(&self) -> VkbGladiatorVariant {
        self.variant
    }

    /// Parse one Gladiator HID report.
    pub fn parse_report(&self, report: &[u8]) -> Result<GladiatorInputState, GladiatorParseError> {
        let payload = if self.has_report_id {
            report.get(1..).unwrap_or(&[])
        } else {
            report
        };

        const MIN_LEN: usize = 12; // need at least axes
        if payload.len() < MIN_LEN {
            return Err(GladiatorParseError::ReportTooShort {
                expected: MIN_LEN,
                actual: payload.len(),
            });
        }

        let mut state = GladiatorInputState::new(self.variant);

        // Axes — signed normalisation for bidirectional, unsigned for unidirectional
        state.axes.roll = normalize_axis_signed(le_u16(payload, 0));
        state.axes.pitch = normalize_axis_signed(le_u16(payload, 2));
        state.axes.yaw = normalize_axis_signed(le_u16(payload, 4));
        state.axes.mini_x = normalize_axis_signed(le_u16(payload, 6));
        state.axes.mini_y = normalize_axis_signed(le_u16(payload, 8));
        state.axes.throttle = normalize_axis_16bit(le_u16(payload, 10));

        // Buttons (optional)
        if payload.len() >= 16 {
            let btn_lo = le_u32(payload, 12);
            for bit in 0..32usize {
                state.buttons[bit] = ((btn_lo >> bit) & 1) != 0;
            }
        }
        if payload.len() >= 20 {
            let btn_hi = le_u32(payload, 16);
            for bit in 0..32usize {
                state.buttons[32 + bit] = ((btn_hi >> bit) & 1) != 0;
            }
        }

        // POV hats (optional)
        if let Some(&hat_byte) = payload.get(20) {
            state.hats[0] = decode_hat_nibble(hat_byte & 0x0F);
            state.hats[1] = decode_hat_nibble((hat_byte >> 4) & 0x0F);
        }

        Ok(state)
    }
}

/// Decode a 4-bit HID hat-switch nibble.
/// Values 0–7 map to N/NE/E/SE/S/SW/W/NW; 0xF (15) means centred.
fn decode_hat_nibble(nibble: u8) -> Option<HatDirection> {
    if nibble <= 7 {
        Some(HatDirection(nibble))
    } else {
        None
    }
}

/// Normalise a raw u16 axis value to `−1.0..=1.0` (bidirectional).
///
/// 0x0000 → −1.0, 0x8000 → 0.0, 0xFFFF → ≈1.0
fn normalize_axis_signed(raw: u16) -> f32 {
    ((raw as f32 / 32767.5) - 1.0).clamp(-1.0, 1.0)
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    let low = bytes.get(offset).copied().unwrap_or(0);
    let high = bytes.get(offset + 1).copied().unwrap_or(0);
    u16::from_le_bytes([low, high])
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    let b0 = bytes.get(offset).copied().unwrap_or(0);
    let b1 = bytes.get(offset + 1).copied().unwrap_or(0);
    let b2 = bytes.get(offset + 2).copied().unwrap_or(0);
    let b3 = bytes.get(offset + 3).copied().unwrap_or(0);
    u32::from_le_bytes([b0, b1, b2, b3])
}

fn normalize_axis_16bit(raw: u16) -> f32 {
    (raw as f32 / u16::MAX as f32).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interface_report_with_axes_and_buttons() {
        let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
        let report = [
            0x00, 0x00, // rx = 0.0
            0xFF, 0xFF, // ry = 1.0
            0x00, 0x80, // x ~= 0.5
            0x00, 0x40, // y ~= 0.25
            0x00, 0xC0, // z ~= 0.75
            0x05, 0x00, 0x00, 0x80, // buttons: 1,3,32
        ];

        let parsed = handler.parse_interface_report(&report).unwrap();
        assert_eq!(parsed.report_len, 14);
        assert_eq!(parsed.buttons, 0x8000_0005);
        let axes = parsed.axes.expect("axes should be present");
        assert!((axes.rx - 0.0).abs() < 0.0001);
        assert!((axes.ry - 1.0).abs() < 0.0001);
        assert!((axes.x - 0.5).abs() < 0.01);
        assert!((axes.y - 0.25).abs() < 0.01);
        assert!((axes.z - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_parse_interface_report_buttons_only() {
        let handler = StecsInputHandler::new(VkbStecsVariant::LeftSpaceThrottleGripMini);
        let report = [0x02, 0x00, 0x00, 0x00];

        let parsed = handler.parse_interface_report(&report).unwrap();
        assert_eq!(parsed.report_len, 4);
        assert!(parsed.axes.is_none());
        assert_eq!(parsed.buttons, 0x0000_0002);
    }

    #[test]
    fn test_parse_interface_report_with_report_id() {
        let handler = StecsInputHandler::new(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus)
            .with_report_id(true);
        let report = [
            0x01, // report id
            0x00, 0x00, // rx
            0x00, 0x00, // ry
            0x00, 0x00, // x
            0x00, 0x00, // y
            0x00, 0x00, // z
            0x01, 0x00, 0x00, 0x00, // button 1
        ];

        let parsed = handler.parse_interface_report(&report).unwrap();
        assert_eq!(parsed.report_len, 14);
        assert_eq!(parsed.buttons, 0x0000_0001);
        assert!(parsed.axes.is_some());
    }

    #[test]
    fn test_merge_interface_report_maps_virtual_button_ranges() {
        let mut aggregator =
            StecsInputAggregator::new(VkbStecsVariant::RightSpaceThrottleGripMiniPlus);
        aggregator.begin_poll();

        let vc0 = [0x01, 0x00, 0x00, 0x80]; // button 1 + 32
        let vc1 = [0x01, 0x00, 0x00, 0x00]; // button 33
        let vc2 = [0x04, 0x00, 0x00, 0x00]; // button 67

        aggregator.merge_interface_report(0, &vc0).unwrap();
        aggregator.merge_interface_report(1, &vc1).unwrap();
        aggregator.merge_interface_report(2, &vc2).unwrap();

        let snapshot = aggregator.snapshot();
        assert!(snapshot.buttons[0]);
        assert!(snapshot.buttons[31]);
        assert!(snapshot.buttons[32]);
        assert!(snapshot.buttons[66]);
        assert_eq!(snapshot.pressed_buttons(), vec![1, 32, 33, 67]);
    }

    #[test]
    fn test_merge_interface_state_prefers_lowest_vc_axes() {
        let mut aggregator = StecsInputAggregator::new(VkbStecsVariant::LeftSpaceThrottleGripMini);
        aggregator.begin_poll();

        aggregator
            .merge_interface_state(
                1,
                StecsInterfaceState {
                    report_len: 14,
                    axes: Some(StecsAxes {
                        rx: 0.1,
                        ry: 0.1,
                        x: 0.1,
                        y: 0.1,
                        z: 0.1,
                    }),
                    buttons: 0,
                },
            )
            .unwrap();

        aggregator
            .merge_interface_state(
                0,
                StecsInterfaceState {
                    report_len: 14,
                    axes: Some(StecsAxes {
                        rx: 0.9,
                        ry: 0.8,
                        x: 0.7,
                        y: 0.6,
                        z: 0.5,
                    }),
                    buttons: 0,
                },
            )
            .unwrap();

        let axes = aggregator.snapshot().axes.expect("axes should be present");
        assert!((axes.rx - 0.9).abs() < 0.0001);
        assert!((axes.ry - 0.8).abs() < 0.0001);
    }

    #[test]
    fn test_virtual_controller_out_of_range() {
        let mut aggregator = StecsInputAggregator::new(VkbStecsVariant::RightSpaceThrottleGripMini);
        aggregator.begin_poll();

        let error = aggregator.merge_interface_report(3, &[0x00, 0x00, 0x00, 0x00]);
        assert!(matches!(
            error,
            Err(StecsParseError::VirtualControllerOutOfRange { index: 3 })
        ));
    }

    #[test]
    fn test_report_too_short() {
        let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
        let error = handler.parse_interface_report(&[0x01, 0x02, 0x03]);
        assert!(matches!(
            error,
            Err(StecsParseError::ReportTooShort {
                expected: 4,
                actual: 3
            })
        ));
    }

    // ----- Gladiator NXT EVO parser tests -----

    fn make_gladiator_report(axes: [u16; 6], btn_lo: u32, btn_hi: u32, hat_byte: u8) -> Vec<u8> {
        let mut report = vec![0u8; 21];
        for (i, &v) in axes.iter().enumerate() {
            let bytes = v.to_le_bytes();
            report[i * 2] = bytes[0];
            report[i * 2 + 1] = bytes[1];
        }
        let lo_bytes = btn_lo.to_le_bytes();
        report[12..16].copy_from_slice(&lo_bytes);
        let hi_bytes = btn_hi.to_le_bytes();
        report[16..20].copy_from_slice(&hi_bytes);
        report[20] = hat_byte;
        report
    }

    #[test]
    fn gladiator_report_too_short_returns_error() {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        let err = handler.parse_report(&[0u8; 10]);
        assert!(matches!(
            err,
            Err(GladiatorParseError::ReportTooShort {
                expected: 12,
                actual: 10
            })
        ));
    }

    #[test]
    fn gladiator_centre_stick_axes_normalise_to_zero() {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        let report =
            make_gladiator_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000], 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        assert!(
            state.axes.roll.abs() < 0.01,
            "roll should be ~0.0 at centre, got {}",
            state.axes.roll
        );
        assert!(
            state.axes.pitch.abs() < 0.01,
            "pitch should be ~0.0 at centre"
        );
        assert!(state.axes.yaw.abs() < 0.01, "yaw should be ~0.0 at centre");
    }

    #[test]
    fn gladiator_full_throttle_wheel_normalises_to_one() {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
        let report =
            make_gladiator_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0xFFFF], 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        assert!(
            (state.axes.throttle - 1.0).abs() < 0.001,
            "expected throttle ≈ 1.0, got {}",
            state.axes.throttle
        );
    }

    #[test]
    fn gladiator_buttons_parsed_correctly() {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        // bit 0 = button 1, bit 31 = button 32, bit 32 = button 33 (in hi word)
        let report = make_gladiator_report(
            [0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0],
            0x8000_0001u32, // buttons 1 and 32
            0x0000_0001u32, // button 33
            0xFF,
        );
        let state = handler.parse_report(&report).unwrap();
        assert!(state.buttons[0], "button 1 should be pressed");
        assert!(state.buttons[31], "button 32 should be pressed");
        assert!(state.buttons[32], "button 33 should be pressed");
        assert_eq!(state.pressed_buttons(), vec![1, 32, 33]);
    }

    #[test]
    fn gladiator_hat_north_decoded() {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        // low nibble = 0 (N), high nibble = 0xF (centred)
        let report = make_gladiator_report(
            [0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0],
            0,
            0,
            0xF0, // hat0=N(0), hat1=centred(F)
        );
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(state.hats[0], Some(HatDirection(0)), "hat 0 should be N");
        assert_eq!(state.hats[1], None, "hat 1 should be centred");
    }

    #[test]
    fn gladiator_with_report_id_strips_prefix() {
        let handler =
            GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight).with_report_id(true);
        let mut report = vec![0x01u8]; // report id
        report.extend_from_slice(&make_gladiator_report(
            [0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0xFFFF],
            0,
            0,
            0xFF,
        ));
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.throttle - 1.0).abs() < 0.001);
    }

    #[test]
    fn stecs_pressed_buttons_empty_when_none_pressed() {
        let state = StecsInputState {
            variant: VkbStecsVariant::RightSpaceThrottleGripMini,
            axes: None,
            buttons: [false; STECS_MAX_BUTTONS],
            active_virtual_controllers: [false; STECS_MAX_VIRTUAL_CONTROLLERS],
        };
        assert!(state.pressed_buttons().is_empty());
    }
}
