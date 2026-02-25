// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Virtual controller abstraction for Project Wingman input forwarding.
//!
//! [`VirtualController`] is the trait that all virtual-controller backends
//! must implement. The [`StubVirtualController`] provided here logs axis and
//! button changes but does **not** create a real device.
//!
//! ## Adding ViGEm support (Windows)
//!
//! 1. Install **ViGEm Bus** from <https://github.com/nefarius/ViGEmBus>.
//! 2. Add the `vigem-client` crate as a dependency.
//! 3. Implement `VirtualController` on a `ViGEmXInputController` struct that
//!    maps `send_axis` / `send_button` calls onto `vigem_client::XGamepad`.
//! 4. Pass the new backend to `WingmanAdapter::start_with_controller`.

use thiserror::Error;

/// Errors that can be returned by a virtual controller backend.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VirtualControllerError {
    #[error("controller not initialized")]
    NotInitialized,
    #[error("axis index {0} is out of range (max 7)")]
    AxisOutOfRange(u8),
    #[error("button index {0} is out of range (max 31)")]
    ButtonOutOfRange(u8),
}

/// Trait for virtual controller backends.
///
/// Implementations must be [`Send`] so they can be owned by the adapter
/// running on any thread.
pub trait VirtualController: Send {
    /// Set a continuous axis value.
    ///
    /// `index` is 0–7; `value` should be in `[-1.0, 1.0]`.
    fn send_axis(&mut self, index: u8, value: f32) -> Result<(), VirtualControllerError>;

    /// Set a digital button state.
    ///
    /// `index` is 0–31.
    fn send_button(&mut self, index: u8, pressed: bool) -> Result<(), VirtualControllerError>;
}

/// Stub virtual controller — records state internally and traces changes.
///
/// No real virtual device is created. This is the default backend used until
/// a driver-backed implementation is configured.
pub struct StubVirtualController {
    axes: [f32; 8],
    buttons: u32,
}

impl StubVirtualController {
    /// Create a stub controller with all axes centred and all buttons released.
    pub fn new() -> Self {
        Self {
            axes: [0.0; 8],
            buttons: 0,
        }
    }

    /// Read back the last axis value sent to `index`, or `None` if out of range.
    pub fn axis(&self, index: u8) -> Option<f32> {
        self.axes.get(index as usize).copied()
    }

    /// Read back the last button state sent to `index`, or `None` if out of range.
    pub fn button_state(&self, index: u8) -> Option<bool> {
        if index >= 32 {
            return None;
        }
        Some((self.buttons >> index) & 1 != 0)
    }
}

impl Default for StubVirtualController {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualController for StubVirtualController {
    fn send_axis(&mut self, index: u8, value: f32) -> Result<(), VirtualControllerError> {
        if index >= 8 {
            return Err(VirtualControllerError::AxisOutOfRange(index));
        }
        self.axes[index as usize] = value.clamp(-1.0, 1.0);
        tracing::trace!("Wingman stub: axis[{}] = {:.3}", index, value);
        Ok(())
    }

    fn send_button(&mut self, index: u8, pressed: bool) -> Result<(), VirtualControllerError> {
        if index >= 32 {
            return Err(VirtualControllerError::ButtonOutOfRange(index));
        }
        if pressed {
            self.buttons |= 1 << index;
        } else {
            self.buttons &= !(1 << index);
        }
        tracing::trace!("Wingman stub: button[{}] = {}", index, pressed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_axis_clamps_and_stores_value() {
        let mut ctrl = StubVirtualController::new();
        ctrl.send_axis(0, 0.5).unwrap();
        assert!((ctrl.axis(0).unwrap() - 0.5).abs() < f32::EPSILON);
        ctrl.send_axis(0, 2.0).unwrap();
        assert_eq!(ctrl.axis(0).unwrap(), 1.0, "should be clamped to 1.0");
    }

    #[test]
    fn stub_axis_out_of_range_errors() {
        let mut ctrl = StubVirtualController::new();
        assert_eq!(
            ctrl.send_axis(8, 0.0),
            Err(VirtualControllerError::AxisOutOfRange(8))
        );
    }

    #[test]
    fn stub_button_stores_state() {
        let mut ctrl = StubVirtualController::new();
        ctrl.send_button(3, true).unwrap();
        assert_eq!(ctrl.button_state(3), Some(true));
        ctrl.send_button(3, false).unwrap();
        assert_eq!(ctrl.button_state(3), Some(false));
    }

    #[test]
    fn stub_button_out_of_range_errors() {
        let mut ctrl = StubVirtualController::new();
        assert_eq!(
            ctrl.send_button(32, true),
            Err(VirtualControllerError::ButtonOutOfRange(32))
        );
    }
}
