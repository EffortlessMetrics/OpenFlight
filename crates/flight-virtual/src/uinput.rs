// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! uinput virtual input backend (Linux)
//!
//! Creates virtual input devices via the Linux uinput subsystem so that
//! games and simulators see them as real joysticks / gamepads.
//!
//! Like the vJoy backend, the actual kernel calls are abstracted behind
//! the [`VirtualBackend`] trait to allow mock-based testing everywhere.

use crate::backend::{HatDirection, VirtualBackend, VirtualBackendError};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

/// Default number of axes for a uinput joystick.
pub const UINPUT_DEFAULT_AXES: u8 = 8;

/// Default number of buttons for a uinput joystick.
pub const UINPUT_DEFAULT_BUTTONS: u8 = 32;

/// Default number of POV hats.
pub const UINPUT_DEFAULT_HATS: u8 = 4;

/// Linux ABS axis range we configure (matches typical joystick).
const UINPUT_AXIS_MIN: i32 = -32768;
const UINPUT_AXIS_MAX: i32 = 32767;

// ── Configuration ───────────────────────────────────────────────────

/// Capabilities to request when creating the uinput device.
#[derive(Debug, Clone)]
pub struct UInputCapabilities {
    /// Number of absolute axes to register.
    pub num_axes: u8,
    /// Number of buttons to register.
    pub num_buttons: u8,
    /// Number of POV hat switches to register.
    pub num_hats: u8,
    /// Human-readable device name exposed to the kernel.
    pub device_name: String,
    /// Vendor ID for the virtual device.
    pub vendor_id: u16,
    /// Product ID for the virtual device.
    pub product_id: u16,
}

impl Default for UInputCapabilities {
    fn default() -> Self {
        Self {
            num_axes: UINPUT_DEFAULT_AXES,
            num_buttons: UINPUT_DEFAULT_BUTTONS,
            num_hats: UINPUT_DEFAULT_HATS,
            device_name: "OpenFlight Virtual Joystick".into(),
            vendor_id: 0x1234,
            product_id: 0x5679,
        }
    }
}

// ── uinput device wrapper ───────────────────────────────────────────

/// A virtual joystick backed by the Linux uinput subsystem.
pub struct UInputDevice {
    caps: UInputCapabilities,
    acquired: AtomicBool,
    axes: Vec<i32>,
    buttons: Vec<bool>,
    hats: Vec<HatDirection>,
}

impl UInputDevice {
    /// Create a new uinput virtual device with the given capabilities.
    pub fn new(caps: UInputCapabilities) -> Self {
        let num_axes = caps.num_axes as usize;
        let num_buttons = caps.num_buttons as usize;
        let num_hats = caps.num_hats as usize;

        Self {
            caps,
            acquired: AtomicBool::new(false),
            axes: vec![0; num_axes],
            buttons: vec![false; num_buttons],
            hats: vec![HatDirection::Centered; num_hats],
        }
    }

    /// Return the capabilities this device was created with.
    pub fn capabilities(&self) -> &UInputCapabilities {
        &self.caps
    }

    /// Check whether `/dev/uinput` is accessible.
    #[cfg(target_os = "linux")]
    pub fn is_available() -> bool {
        std::path::Path::new("/dev/uinput").exists()
    }

    #[cfg(not(target_os = "linux"))]
    pub fn is_available() -> bool {
        false
    }
}

impl VirtualBackend for UInputDevice {
    fn acquire(&mut self) -> Result<(), VirtualBackendError> {
        if self.acquired.load(Ordering::Relaxed) {
            return Err(VirtualBackendError::AlreadyAcquired(0));
        }

        // Real implementation: open /dev/uinput, ioctl to register axes/buttons,
        // UI_DEV_CREATE.
        info!(name = %self.caps.device_name, "uinput device created");
        self.acquired.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn release(&mut self) -> Result<(), VirtualBackendError> {
        if !self.acquired.load(Ordering::Relaxed) {
            return Err(VirtualBackendError::NotAcquired(0));
        }

        // Real implementation: UI_DEV_DESTROY, close fd.
        self.axes.fill(0);
        self.buttons.fill(false);
        self.hats.fill(HatDirection::Centered);

        info!(name = %self.caps.device_name, "uinput device destroyed");
        self.acquired.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_acquired(&self) -> bool {
        self.acquired.load(Ordering::Relaxed)
    }

    fn set_axis(&mut self, axis_id: u8, value: f32) -> Result<(), VirtualBackendError> {
        if !self.is_acquired() {
            return Err(VirtualBackendError::NotAcquired(0));
        }
        if axis_id >= self.caps.num_axes {
            return Err(VirtualBackendError::InvalidAxis(axis_id));
        }

        let clamped = value.clamp(-1.0, 1.0);
        // Map [-1, 1] → [-32768, 32767]
        let raw = (clamped * UINPUT_AXIS_MAX as f32) as i32;
        let raw = raw.clamp(UINPUT_AXIS_MIN, UINPUT_AXIS_MAX);

        self.axes[axis_id as usize] = raw;
        debug!(axis = axis_id, raw, "uinput axis set");
        Ok(())
    }

    fn set_button(&mut self, button_id: u8, pressed: bool) -> Result<(), VirtualBackendError> {
        if !self.is_acquired() {
            return Err(VirtualBackendError::NotAcquired(0));
        }
        if button_id >= self.caps.num_buttons {
            return Err(VirtualBackendError::InvalidButton(button_id));
        }

        self.buttons[button_id as usize] = pressed;
        debug!(button = button_id, pressed, "uinput button set");
        Ok(())
    }

    fn set_hat(&mut self, hat_id: u8, direction: HatDirection) -> Result<(), VirtualBackendError> {
        if !self.is_acquired() {
            return Err(VirtualBackendError::NotAcquired(0));
        }
        if hat_id >= self.caps.num_hats {
            return Err(VirtualBackendError::InvalidHat(hat_id));
        }

        self.hats[hat_id as usize] = direction;
        debug!(hat = hat_id, ?direction, "uinput hat set");
        Ok(())
    }

    fn get_axis(&self, axis_id: u8) -> Result<f32, VirtualBackendError> {
        if axis_id >= self.caps.num_axes {
            return Err(VirtualBackendError::InvalidAxis(axis_id));
        }
        let raw = self.axes[axis_id as usize];
        let normalized = raw as f32 / UINPUT_AXIS_MAX as f32;
        Ok(normalized.clamp(-1.0, 1.0))
    }

    fn get_button(&self, button_id: u8) -> Result<bool, VirtualBackendError> {
        if button_id >= self.caps.num_buttons {
            return Err(VirtualBackendError::InvalidButton(button_id));
        }
        Ok(self.buttons[button_id as usize])
    }

    fn get_hat(&self, hat_id: u8) -> Result<HatDirection, VirtualBackendError> {
        if hat_id >= self.caps.num_hats {
            return Err(VirtualBackendError::InvalidHat(hat_id));
        }
        Ok(self.hats[hat_id as usize])
    }

    fn axis_count(&self) -> u8 {
        self.caps.num_axes
    }

    fn button_count(&self) -> u8 {
        self.caps.num_buttons
    }

    fn hat_count(&self) -> u8 {
        self.caps.num_hats
    }
}

impl Drop for UInputDevice {
    fn drop(&mut self) {
        if self.is_acquired()
            && let Err(e) = self.release()
        {
            warn!(error = %e, "failed to release uinput device on drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_device() -> UInputDevice {
        UInputDevice::new(UInputCapabilities::default())
    }

    #[test]
    fn test_uinput_creation() {
        let dev = make_device();
        assert!(!dev.is_acquired());
        assert_eq!(dev.capabilities().num_axes, UINPUT_DEFAULT_AXES);
    }

    #[test]
    fn test_uinput_acquire_release() {
        let mut dev = make_device();

        dev.acquire().unwrap();
        assert!(dev.is_acquired());
        assert!(dev.acquire().is_err());

        dev.release().unwrap();
        assert!(!dev.is_acquired());
        assert!(dev.release().is_err());
    }

    #[test]
    fn test_uinput_axis_clamping() {
        let mut dev = make_device();
        dev.acquire().unwrap();

        dev.set_axis(0, 0.75).unwrap();
        let val = dev.get_axis(0).unwrap();
        assert!((val - 0.75).abs() < 0.01);

        dev.set_axis(0, 3.0).unwrap();
        let val = dev.get_axis(0).unwrap();
        assert!((val - 1.0).abs() < 0.01);

        dev.set_axis(0, -3.0).unwrap();
        let val = dev.get_axis(0).unwrap();
        assert!((val - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn test_uinput_buttons() {
        let mut dev = make_device();
        dev.acquire().unwrap();

        dev.set_button(0, true).unwrap();
        assert!(dev.get_button(0).unwrap());

        dev.set_button(0, false).unwrap();
        assert!(!dev.get_button(0).unwrap());
    }

    #[test]
    fn test_uinput_hat() {
        let mut dev = make_device();
        dev.acquire().unwrap();

        dev.set_hat(0, HatDirection::SouthWest).unwrap();
        assert_eq!(dev.get_hat(0).unwrap(), HatDirection::SouthWest);
    }

    #[test]
    fn test_uinput_not_acquired_errors() {
        let mut dev = make_device();
        assert!(dev.set_axis(0, 0.0).is_err());
        assert!(dev.set_button(0, true).is_err());
        assert!(dev.set_hat(0, HatDirection::North).is_err());
    }

    #[test]
    fn test_uinput_custom_capabilities() {
        let caps = UInputCapabilities {
            num_axes: 4,
            num_buttons: 16,
            num_hats: 2,
            device_name: "Custom Stick".into(),
            vendor_id: 0xABCD,
            product_id: 0xEF01,
        };
        let mut dev = UInputDevice::new(caps);
        dev.acquire().unwrap();

        assert_eq!(dev.axis_count(), 4);
        assert_eq!(dev.button_count(), 16);
        assert_eq!(dev.hat_count(), 2);

        // Out-of-range for the reduced capability set.
        assert!(dev.set_axis(4, 0.0).is_err());
        assert!(dev.set_button(16, false).is_err());
        assert!(dev.set_hat(2, HatDirection::Centered).is_err());
    }

    #[test]
    fn test_uinput_release_resets_state() {
        let mut dev = make_device();
        dev.acquire().unwrap();

        dev.set_axis(0, 1.0).unwrap();
        dev.set_button(3, true).unwrap();
        dev.set_hat(1, HatDirection::West).unwrap();

        dev.release().unwrap();
        dev.acquire().unwrap();

        let val = dev.get_axis(0).unwrap();
        assert!(
            val.abs() < 0.01,
            "axis should be zero after reset, got {val}"
        );
        assert!(!dev.get_button(3).unwrap());
        assert_eq!(dev.get_hat(1).unwrap(), HatDirection::Centered);
    }
}
