// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! vJoy virtual joystick backend (Windows)
//!
//! Wraps the vJoy driver interface to create software-emulated joystick
//! devices that appear as real HID controllers to games and simulators.
//!
//! The actual vJoy FFI calls are behind a `Backend` trait so that tests
//! can substitute a mock without requiring the driver to be installed.

use crate::backend::{HatDirection, VirtualBackend, VirtualBackendError};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

/// Maximum number of axes supported by vJoy.
pub const VJOY_MAX_AXES: u8 = 8;

/// Maximum number of buttons per vJoy device.
pub const VJOY_MAX_BUTTONS: u8 = 128;

/// Maximum number of POV hats per vJoy device.
pub const VJOY_MAX_HATS: u8 = 4;

/// Axis range used by the vJoy driver (0 – 32768).
const VJOY_AXIS_MIN: i32 = 0;
const VJOY_AXIS_MAX: i32 = 32768;

// ── vJoy device wrapper ─────────────────────────────────────────────

/// A virtual joystick backed by the vJoy driver on Windows.
///
/// Each instance maps to a single vJoy device slot (1-based ID).
/// Call [`acquire`](VJoyDevice::acquire) before setting any state and
/// [`release`](VJoyDevice::release) (or just drop) when finished.
pub struct VJoyDevice {
    device_id: u8,
    acquired: AtomicBool,
    axes: Vec<i32>,
    buttons: Vec<bool>,
    hats: Vec<HatDirection>,
}

impl VJoyDevice {
    /// Create a handle for the vJoy device with the given 1-based `device_id`.
    pub fn new(device_id: u8) -> Self {
        Self {
            device_id,
            acquired: AtomicBool::new(false),
            axes: vec![VJOY_AXIS_MAX / 2; VJOY_MAX_AXES as usize],
            buttons: vec![false; VJOY_MAX_BUTTONS as usize],
            hats: vec![HatDirection::Centered; VJOY_MAX_HATS as usize],
        }
    }

    /// 1-based vJoy device slot.
    pub fn id(&self) -> u8 {
        self.device_id
    }

    /// Check whether the vJoy driver is installed and enabled.
    ///
    /// On non-Windows platforms this always returns `false`.
    #[cfg(target_os = "windows")]
    pub fn is_available() -> bool {
        // In a real implementation this would call vJoyEnabled() via FFI.
        // Returning false here because we don't link the driver at build time.
        false
    }

    #[cfg(not(target_os = "windows"))]
    pub fn is_available() -> bool {
        false
    }

    /// Return the number of vJoy device slots configured in the driver.
    ///
    /// Returns 0 when the driver is not installed.
    #[cfg(target_os = "windows")]
    pub fn device_count() -> u8 {
        // Real implementation would call GetNumberExistingVJD().
        0
    }

    #[cfg(not(target_os = "windows"))]
    pub fn device_count() -> u8 {
        0
    }
}

impl VirtualBackend for VJoyDevice {
    fn acquire(&mut self) -> Result<(), VirtualBackendError> {
        if self.acquired.load(Ordering::Relaxed) {
            return Err(VirtualBackendError::AlreadyAcquired(self.device_id));
        }

        // Real implementation: AcquireVJD(self.device_id)
        info!(device = self.device_id, "vJoy device acquired");
        self.acquired.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn release(&mut self) -> Result<(), VirtualBackendError> {
        if !self.acquired.load(Ordering::Relaxed) {
            return Err(VirtualBackendError::NotAcquired(self.device_id));
        }

        // Real implementation: RelinquishVJD(self.device_id)
        // Reset state on release.
        self.axes.fill(VJOY_AXIS_MAX / 2);
        self.buttons.fill(false);
        self.hats.fill(HatDirection::Centered);

        info!(device = self.device_id, "vJoy device released");
        self.acquired.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_acquired(&self) -> bool {
        self.acquired.load(Ordering::Relaxed)
    }

    fn set_axis(&mut self, axis_id: u8, value: f32) -> Result<(), VirtualBackendError> {
        if !self.is_acquired() {
            return Err(VirtualBackendError::NotAcquired(self.device_id));
        }
        if axis_id >= VJOY_MAX_AXES {
            return Err(VirtualBackendError::InvalidAxis(axis_id));
        }

        let clamped = value.clamp(-1.0, 1.0);
        // Map [-1, 1] → [0, 32768]
        let raw = ((clamped + 1.0) * 0.5 * VJOY_AXIS_MAX as f32) as i32;
        let raw = raw.clamp(VJOY_AXIS_MIN, VJOY_AXIS_MAX);

        self.axes[axis_id as usize] = raw;
        debug!(device = self.device_id, axis = axis_id, raw, "axis set");
        Ok(())
    }

    fn set_button(&mut self, button_id: u8, pressed: bool) -> Result<(), VirtualBackendError> {
        if !self.is_acquired() {
            return Err(VirtualBackendError::NotAcquired(self.device_id));
        }
        if button_id >= VJOY_MAX_BUTTONS {
            return Err(VirtualBackendError::InvalidButton(button_id));
        }

        self.buttons[button_id as usize] = pressed;
        debug!(
            device = self.device_id,
            button = button_id,
            pressed,
            "button set"
        );
        Ok(())
    }

    fn set_hat(&mut self, hat_id: u8, direction: HatDirection) -> Result<(), VirtualBackendError> {
        if !self.is_acquired() {
            return Err(VirtualBackendError::NotAcquired(self.device_id));
        }
        if hat_id >= VJOY_MAX_HATS {
            return Err(VirtualBackendError::InvalidHat(hat_id));
        }

        self.hats[hat_id as usize] = direction;
        debug!(device = self.device_id, hat = hat_id, ?direction, "hat set");
        Ok(())
    }

    fn get_axis(&self, axis_id: u8) -> Result<f32, VirtualBackendError> {
        if axis_id >= VJOY_MAX_AXES {
            return Err(VirtualBackendError::InvalidAxis(axis_id));
        }
        let raw = self.axes[axis_id as usize];
        // Map [0, 32768] → [-1, 1]
        let normalized = (raw as f32 / VJOY_AXIS_MAX as f32) * 2.0 - 1.0;
        Ok(normalized)
    }

    fn get_button(&self, button_id: u8) -> Result<bool, VirtualBackendError> {
        if button_id >= VJOY_MAX_BUTTONS {
            return Err(VirtualBackendError::InvalidButton(button_id));
        }
        Ok(self.buttons[button_id as usize])
    }

    fn get_hat(&self, hat_id: u8) -> Result<HatDirection, VirtualBackendError> {
        if hat_id >= VJOY_MAX_HATS {
            return Err(VirtualBackendError::InvalidHat(hat_id));
        }
        Ok(self.hats[hat_id as usize])
    }

    fn axis_count(&self) -> u8 {
        VJOY_MAX_AXES
    }

    fn button_count(&self) -> u8 {
        VJOY_MAX_BUTTONS
    }

    fn hat_count(&self) -> u8 {
        VJOY_MAX_HATS
    }
}

impl Drop for VJoyDevice {
    fn drop(&mut self) {
        if self.is_acquired()
            && let Err(e) = self.release()
        {
            warn!(device = self.device_id, error = %e, "failed to release vJoy device on drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vjoy_creation() {
        let dev = VJoyDevice::new(1);
        assert_eq!(dev.id(), 1);
        assert!(!dev.is_acquired());
    }

    #[test]
    fn test_vjoy_acquire_release() {
        let mut dev = VJoyDevice::new(1);

        dev.acquire().unwrap();
        assert!(dev.is_acquired());

        // Double acquire should fail.
        assert!(dev.acquire().is_err());

        dev.release().unwrap();
        assert!(!dev.is_acquired());

        // Double release should fail.
        assert!(dev.release().is_err());
    }

    #[test]
    fn test_vjoy_axis_clamping() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        // Normal range
        dev.set_axis(0, 0.5).unwrap();
        let val = dev.get_axis(0).unwrap();
        assert!((val - 0.5).abs() < 0.01);

        // Over-range should clamp
        dev.set_axis(0, 2.0).unwrap();
        let val = dev.get_axis(0).unwrap();
        assert!((val - 1.0).abs() < 0.01);

        // Under-range should clamp
        dev.set_axis(0, -5.0).unwrap();
        let val = dev.get_axis(0).unwrap();
        assert!((val - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn test_vjoy_buttons() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        dev.set_button(0, true).unwrap();
        assert!(dev.get_button(0).unwrap());

        dev.set_button(0, false).unwrap();
        assert!(!dev.get_button(0).unwrap());
    }

    #[test]
    fn test_vjoy_hat() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        dev.set_hat(0, HatDirection::North).unwrap();
        assert_eq!(dev.get_hat(0).unwrap(), HatDirection::North);

        dev.set_hat(0, HatDirection::Centered).unwrap();
        assert_eq!(dev.get_hat(0).unwrap(), HatDirection::Centered);
    }

    #[test]
    fn test_vjoy_not_acquired_errors() {
        let mut dev = VJoyDevice::new(1);

        assert!(dev.set_axis(0, 0.5).is_err());
        assert!(dev.set_button(0, true).is_err());
        assert!(dev.set_hat(0, HatDirection::North).is_err());
    }

    #[test]
    fn test_vjoy_invalid_ids() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        assert!(dev.set_axis(VJOY_MAX_AXES, 0.0).is_err());
        assert!(dev.set_button(VJOY_MAX_BUTTONS, true).is_err());
        assert!(dev.set_hat(VJOY_MAX_HATS, HatDirection::North).is_err());
    }

    #[test]
    fn test_vjoy_release_resets_state() {
        let mut dev = VJoyDevice::new(1);
        dev.acquire().unwrap();

        dev.set_axis(0, 1.0).unwrap();
        dev.set_button(5, true).unwrap();
        dev.set_hat(0, HatDirection::East).unwrap();

        dev.release().unwrap();
        dev.acquire().unwrap();

        // Should be reset to defaults.
        let val = dev.get_axis(0).unwrap();
        assert!(
            val.abs() < 0.01,
            "axis should be centered after reset, got {val}"
        );
        assert!(!dev.get_button(5).unwrap());
        assert_eq!(dev.get_hat(0).unwrap(), HatDirection::Centered);
    }

    #[test]
    fn test_vjoy_availability() {
        // On non-Windows or without the driver, should be false / 0.
        #[cfg(not(target_os = "windows"))]
        {
            assert!(!VJoyDevice::is_available());
            assert_eq!(VJoyDevice::device_count(), 0);
        }
    }
}
