// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! ViGEm Xbox 360 virtual controller backend (Windows only).
//!
//! Wraps the `vigem-client` crate to create a real XInput-compatible virtual
//! controller via the ViGEmBus driver. If the driver is not installed,
//! [`ViGEmXInputController::new`] returns
//! [`VirtualControllerError::ViGEmNotAvailable`].

use crate::virtual_controller::{VirtualController, VirtualControllerError};
use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};

/// XInput button map: logical index → XButtons bit.
///
/// Indices 0-13 map to the standard face/shoulder/dpad/thumb buttons.
/// Indices 14-31 are no-ops (reserved for future use).
const BUTTON_MAP: [u16; 14] = [
    XButtons::A,      // 0
    XButtons::B,      // 1
    XButtons::X,      // 2
    XButtons::Y,      // 3
    XButtons::LB,     // 4
    XButtons::RB,     // 5
    XButtons::BACK,   // 6
    XButtons::START,  // 7
    XButtons::LTHUMB, // 8
    XButtons::RTHUMB, // 9
    XButtons::UP,     // 10
    XButtons::DOWN,   // 11
    XButtons::LEFT,   // 12
    XButtons::RIGHT,  // 13
];

/// A Windows virtual Xbox 360 controller backed by ViGEmBus.
///
/// Create with [`ViGEmXInputController::new`]. The virtual device is unplugged
/// automatically when this struct is dropped.
pub struct ViGEmXInputController {
    target: Xbox360Wired<Client>,
    gamepad: XGamepad,
}

// SAFETY: vigem_client wraps Windows HANDLEs (raw pointers) which Rust won't
// auto-mark Send. The ViGEmBus driver is designed for cross-thread use and the
// underlying IOCTL calls are synchronised inside the driver, so transferring
// ownership across threads is safe.
unsafe impl Send for ViGEmXInputController {}

impl ViGEmXInputController {
    /// Connect to ViGEmBus and plug in a virtual Xbox 360 controller.
    ///
    /// Returns [`VirtualControllerError::ViGEmNotAvailable`] when the ViGEmBus
    /// driver is not installed or the connection fails.
    pub fn new() -> Result<Self, VirtualControllerError> {
        let client = Client::connect()
            .map_err(|e| VirtualControllerError::ViGEmNotAvailable(e.to_string()))?;
        let mut target = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
        target
            .plugin()
            .map_err(|e| VirtualControllerError::ViGEmNotAvailable(e.to_string()))?;
        target
            .wait_ready()
            .map_err(|e| VirtualControllerError::ViGEmNotAvailable(e.to_string()))?;
        Ok(Self {
            target,
            gamepad: XGamepad::default(),
        })
    }
}

impl VirtualController for ViGEmXInputController {
    /// Map axis `index` onto the XInput gamepad state and push the update.
    ///
    /// | index | field         | range       |
    /// |-------|---------------|-------------|
    /// | 0     | thumb_lx      | −1.0 … 1.0  |
    /// | 1     | thumb_ly      | −1.0 … 1.0  |
    /// | 2     | thumb_rx      | −1.0 … 1.0  |
    /// | 3     | thumb_ry      | −1.0 … 1.0  |
    /// | 4     | left_trigger  | 0.0 … 1.0   |
    /// | 5     | right_trigger | 0.0 … 1.0   |
    /// | 6–7   | (no-op)       |             |
    fn send_axis(&mut self, index: u8, value: f32) -> Result<(), VirtualControllerError> {
        match index {
            0 => self.gamepad.thumb_lx = axis_to_i16(value),
            1 => self.gamepad.thumb_ly = axis_to_i16(value),
            2 => self.gamepad.thumb_rx = axis_to_i16(value),
            3 => self.gamepad.thumb_ry = axis_to_i16(value),
            4 => self.gamepad.left_trigger = trigger_to_u8(value),
            5 => self.gamepad.right_trigger = trigger_to_u8(value),
            6 | 7 => return Ok(()), // reserved axes — no-op, no device update needed
            _ => return Err(VirtualControllerError::AxisOutOfRange(index)),
        }
        self.target
            .update(&self.gamepad)
            .map_err(|e| VirtualControllerError::ViGEmNotAvailable(e.to_string()))
    }

    /// Map button `index` onto an XInput button bit and push the update.
    ///
    /// Indices 0–13 map to A/B/X/Y/LB/RB/Back/Start/LS/RS/Up/Down/Left/Right.
    /// Indices 14–31 are accepted but are no-ops (no device update is sent).
    fn send_button(&mut self, index: u8, pressed: bool) -> Result<(), VirtualControllerError> {
        if index >= 32 {
            return Err(VirtualControllerError::ButtonOutOfRange(index));
        }
        if let Some(&bit) = BUTTON_MAP.get(index as usize) {
            if pressed {
                self.gamepad.buttons.raw |= bit;
            } else {
                self.gamepad.buttons.raw &= !bit;
            }
            self.target
                .update(&self.gamepad)
                .map_err(|e| VirtualControllerError::ViGEmNotAvailable(e.to_string()))
        } else {
            // indices 14-31: reserved, accepted as no-op
            Ok(())
        }
    }
}

/// Convert a normalised axis value (−1.0 … 1.0) to an XInput i16 thumb value.
#[inline]
fn axis_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

/// Convert a normalised trigger value (0.0 … 1.0) to an XInput u8 trigger value.
#[inline]
fn trigger_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * u8::MAX as f32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Conversion math (no hardware required) ──────────────────────────────

    #[test]
    fn axis_to_i16_maps_positive_one() {
        assert_eq!(axis_to_i16(1.0), i16::MAX);
    }

    #[test]
    fn axis_to_i16_maps_negative_one() {
        // -1.0 × 32767 = -32767 (XInput convention; -32768 is unused)
        assert_eq!(axis_to_i16(-1.0), -32767);
    }

    #[test]
    fn axis_to_i16_maps_zero() {
        assert_eq!(axis_to_i16(0.0), 0);
    }

    #[test]
    fn axis_to_i16_clamps_beyond_range() {
        assert_eq!(axis_to_i16(2.0), i16::MAX);
        assert_eq!(axis_to_i16(-2.0), -32767);
    }

    #[test]
    fn trigger_to_u8_maps_zero_and_one() {
        assert_eq!(trigger_to_u8(0.0), 0);
        assert_eq!(trigger_to_u8(1.0), 255);
    }

    #[test]
    fn trigger_to_u8_clamps_below_zero() {
        assert_eq!(trigger_to_u8(-1.0), 0);
    }

    #[test]
    fn trigger_to_u8_clamps_above_one() {
        assert_eq!(trigger_to_u8(2.0), 255);
    }

    #[test]
    fn trigger_to_u8_midpoint() {
        let mid = trigger_to_u8(0.5);
        // 0.5 × 255 = 127.5 → 127 (truncation)
        assert_eq!(mid, 127);
    }

    // ── Error propagation (no ViGEmBus driver required) ─────────────────────

    /// Verifies that `new()` propagates the connection error gracefully
    /// instead of panicking when ViGEmBus is not installed.
    #[test]
    fn new_does_not_panic_without_driver() {
        // We don't care whether it succeeds (ViGEmBus installed) or fails
        // gracefully; we just must not panic.
        let _ = ViGEmXInputController::new();
    }

    // ── Hardware-dependent tests (skipped in CI) ─────────────────────────────

    /// Smoke-test: plug in a virtual controller, send all axes, send all buttons.
    ///
    /// Requires ViGEmBus driver to be installed on the test machine.
    #[test]
    #[ignore = "requires ViGEmBus driver to be installed"]
    fn hardware_round_trip_axes_and_buttons() {
        let mut ctrl = ViGEmXInputController::new().expect("ViGEmBus not available");
        for i in 0u8..8 {
            ctrl.send_axis(i, 0.5).unwrap();
        }
        for i in 0u8..14 {
            ctrl.send_button(i, true).unwrap();
            ctrl.send_button(i, false).unwrap();
        }
    }
}
