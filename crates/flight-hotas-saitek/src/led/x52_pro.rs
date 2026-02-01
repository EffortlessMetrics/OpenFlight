// SPDX-License-Identifier: MIT OR Apache-2.0

//! X52 Pro LED implementation.
//!
//! **UNVERIFIED PROTOCOL** - See `docs/reference/hotas-claims.md`

use crate::policy::allow_device_io;
use crate::traits::{HotasError, HotasResult, LedId, LedProtocol, LedState};

/// Hypothesized bRequest value for LED control.
const LED_REQUEST: u8 = 0xB8;

/// Hypothesized request type for LED control.
const LED_REQUEST_TYPE: u8 = 0x40;

/// X52/X52 Pro LED controller.
///
/// # Protocol Status
///
/// **UNVERIFIED** - Based on community documentation.
pub struct X52ProLed {
    device_path: String,
    verified: bool,
}

impl X52ProLed {
    /// Create a new LED controller.
    pub fn new(device_path: String) -> Self {
        tracing::warn!(
            target: "hotas::led",
            device = %device_path,
            "Creating X52 Pro LED controller with UNVERIFIED protocol"
        );

        Self {
            device_path,
            verified: false,
        }
    }

    /// Map LED ID to hypothesized hardware index.
    fn led_index(led: LedId) -> u8 {
        // These mappings are UNVERIFIED
        match led {
            LedId::Fire => 0,
            LedId::ButtonA => 1,
            LedId::ButtonB => 2,
            LedId::ButtonD => 3,
            LedId::ButtonE => 4,
            LedId::Toggle1 => 5,
            LedId::Toggle2 => 6,
            LedId::Toggle3 => 7,
            LedId::Pov2 => 8,
            LedId::Clutch => 9,
            LedId::Throttle => 10,
        }
    }

    /// Map LED state to hypothesized color code.
    fn state_code(state: LedState) -> u8 {
        // These mappings are UNVERIFIED
        match state {
            LedState::Off => 0,
            LedState::Green => 1,
            LedState::Amber => 2,
            LedState::Red => 3,
        }
    }

    fn send_control_transfer(
        &self,
        _request_type: u8,
        request: u8,
        value: u16,
        index: u16,
    ) -> HotasResult<()> {
        // Policy gate: block all output I/O unless explicitly enabled
        if !allow_device_io() {
            return Err(HotasError::UnverifiedProtocol("x52_pro_led"));
        }

        tracing::debug!(
            target: "hotas::led",
            device = %self.device_path,
            request = %format!("0x{:02X}", request),
            value = %format!("0x{:04X}", value),
            index = %format!("0x{:04X}", index),
            "Attempting LED control transfer (UNVERIFIED)"
        );

        if !self.verified {
            Err(HotasError::UnverifiedProtocol("x52_pro_led"))
        } else {
            Ok(())
        }
    }
}

impl LedProtocol for X52ProLed {
    fn set_led(&mut self, led: LedId, state: LedState) -> HotasResult<()> {
        tracing::info!(
            target: "hotas::led",
            led = ?led,
            state = ?state,
            "Setting LED state (UNVERIFIED protocol)"
        );

        // Hypothesis: wValue = LED index, wIndex = color code
        self.send_control_transfer(
            LED_REQUEST_TYPE,
            LED_REQUEST,
            Self::led_index(led) as u16,
            Self::state_code(state) as u16,
        )
    }

    fn set_global_brightness(&mut self, level: u8) -> HotasResult<()> {
        tracing::info!(
            target: "hotas::led",
            level = level,
            "Setting global LED brightness (UNVERIFIED protocol)"
        );

        // Protocol for global brightness is unknown
        Err(HotasError::UnverifiedProtocol("x52_pro_led_brightness"))
    }
}
