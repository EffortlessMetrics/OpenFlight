// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Common trait and types shared by all virtual-device backends.

use std::fmt;

// ── Hat direction ───────────────────────────────────────────────────

/// 8-direction POV hat switch position (plus centered).
///
/// Matches the standard HID hat-switch encoding:
/// 0 = North, 1 = NE, … 7 = NW, centered = no contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HatDirection {
    Centered = 0xFF,
    North = 0,
    NorthEast = 1,
    East = 2,
    SouthEast = 3,
    South = 4,
    SouthWest = 5,
    West = 6,
    NorthWest = 7,
}

impl HatDirection {
    /// Convert a raw HID hat byte into a `HatDirection`.
    pub fn from_hid(value: u8) -> Self {
        match value {
            0 => Self::North,
            1 => Self::NorthEast,
            2 => Self::East,
            3 => Self::SouthEast,
            4 => Self::South,
            5 => Self::SouthWest,
            6 => Self::West,
            7 => Self::NorthWest,
            _ => Self::Centered,
        }
    }

    /// Return the HID hat-switch byte (0–7, or 0xFF for centered).
    pub fn to_hid(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for HatDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Centered => write!(f, "Centered"),
            Self::North => write!(f, "N"),
            Self::NorthEast => write!(f, "NE"),
            Self::East => write!(f, "E"),
            Self::SouthEast => write!(f, "SE"),
            Self::South => write!(f, "S"),
            Self::SouthWest => write!(f, "SW"),
            Self::West => write!(f, "W"),
            Self::NorthWest => write!(f, "NW"),
        }
    }
}

// ── Error type ──────────────────────────────────────────────────────

/// Errors that a virtual-device backend can return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualBackendError {
    /// Tried to use a device that hasn't been acquired.
    NotAcquired(u8),
    /// Device is already acquired.
    AlreadyAcquired(u8),
    /// Axis ID is out of range.
    InvalidAxis(u8),
    /// Button ID is out of range.
    InvalidButton(u8),
    /// Hat ID is out of range.
    InvalidHat(u8),
    /// Platform driver is not available.
    DriverNotAvailable,
    /// An OS / driver-level error.
    PlatformError(String),
}

impl fmt::Display for VirtualBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAcquired(id) => write!(f, "device {id} not acquired"),
            Self::AlreadyAcquired(id) => write!(f, "device {id} already acquired"),
            Self::InvalidAxis(id) => write!(f, "invalid axis id {id}"),
            Self::InvalidButton(id) => write!(f, "invalid button id {id}"),
            Self::InvalidHat(id) => write!(f, "invalid hat id {id}"),
            Self::DriverNotAvailable => write!(f, "virtual device driver not available"),
            Self::PlatformError(msg) => write!(f, "platform error: {msg}"),
        }
    }
}

impl std::error::Error for VirtualBackendError {}

// ── Backend trait ───────────────────────────────────────────────────

/// Unified interface for platform-specific virtual device drivers.
///
/// Both [`VJoyDevice`](crate::vjoy::VJoyDevice) (Windows / vJoy) and
/// [`UInputDevice`](crate::uinput::UInputDevice) (Linux / uinput) implement
/// this trait, as does [`MockBackend`](crate::mock::MockBackend) for testing.
pub trait VirtualBackend {
    /// Acquire exclusive control of the virtual device.
    fn acquire(&mut self) -> Result<(), VirtualBackendError>;

    /// Release the device (resets state to defaults).
    fn release(&mut self) -> Result<(), VirtualBackendError>;

    /// Whether this backend currently holds the device.
    fn is_acquired(&self) -> bool;

    /// Set an axis to a normalized value in `[-1.0, 1.0]`.
    fn set_axis(&mut self, axis_id: u8, value: f32) -> Result<(), VirtualBackendError>;

    /// Set a button's pressed / released state.
    fn set_button(&mut self, button_id: u8, pressed: bool) -> Result<(), VirtualBackendError>;

    /// Set a POV hat direction.
    fn set_hat(&mut self, hat_id: u8, direction: HatDirection) -> Result<(), VirtualBackendError>;

    /// Read the current axis value.
    fn get_axis(&self, axis_id: u8) -> Result<f32, VirtualBackendError>;

    /// Read the current button state.
    fn get_button(&self, button_id: u8) -> Result<bool, VirtualBackendError>;

    /// Read the current hat direction.
    fn get_hat(&self, hat_id: u8) -> Result<HatDirection, VirtualBackendError>;

    /// Number of axes the device supports.
    fn axis_count(&self) -> u8;

    /// Number of buttons the device supports.
    fn button_count(&self) -> u8;

    /// Number of hats the device supports.
    fn hat_count(&self) -> u8;
}

// ── Mock backend (always compiled — used in tests) ──────────────────

/// A platform-independent mock backend for cross-platform testing.
///
/// Stores all state in-memory without touching any OS driver.
pub struct MockBackend {
    acquired: bool,
    num_axes: u8,
    num_buttons: u8,
    num_hats: u8,
    axes: Vec<f32>,
    buttons: Vec<bool>,
    hats: Vec<HatDirection>,
}

impl MockBackend {
    /// Create a mock device with the given capability counts.
    pub fn new(num_axes: u8, num_buttons: u8, num_hats: u8) -> Self {
        Self {
            acquired: false,
            num_axes,
            num_buttons,
            num_hats,
            axes: vec![0.0; num_axes as usize],
            buttons: vec![false; num_buttons as usize],
            hats: vec![HatDirection::Centered; num_hats as usize],
        }
    }

    /// Convenience: create a mock that mirrors a typical joystick.
    pub fn joystick() -> Self {
        Self::new(8, 32, 4)
    }
}

impl VirtualBackend for MockBackend {
    fn acquire(&mut self) -> Result<(), VirtualBackendError> {
        if self.acquired {
            return Err(VirtualBackendError::AlreadyAcquired(0));
        }
        self.acquired = true;
        Ok(())
    }

    fn release(&mut self) -> Result<(), VirtualBackendError> {
        if !self.acquired {
            return Err(VirtualBackendError::NotAcquired(0));
        }
        self.axes.fill(0.0);
        self.buttons.fill(false);
        self.hats.fill(HatDirection::Centered);
        self.acquired = false;
        Ok(())
    }

    fn is_acquired(&self) -> bool {
        self.acquired
    }

    fn set_axis(&mut self, axis_id: u8, value: f32) -> Result<(), VirtualBackendError> {
        if !self.acquired {
            return Err(VirtualBackendError::NotAcquired(0));
        }
        if axis_id >= self.num_axes {
            return Err(VirtualBackendError::InvalidAxis(axis_id));
        }
        self.axes[axis_id as usize] = value.clamp(-1.0, 1.0);
        Ok(())
    }

    fn set_button(&mut self, button_id: u8, pressed: bool) -> Result<(), VirtualBackendError> {
        if !self.acquired {
            return Err(VirtualBackendError::NotAcquired(0));
        }
        if button_id >= self.num_buttons {
            return Err(VirtualBackendError::InvalidButton(button_id));
        }
        self.buttons[button_id as usize] = pressed;
        Ok(())
    }

    fn set_hat(&mut self, hat_id: u8, direction: HatDirection) -> Result<(), VirtualBackendError> {
        if !self.acquired {
            return Err(VirtualBackendError::NotAcquired(0));
        }
        if hat_id >= self.num_hats {
            return Err(VirtualBackendError::InvalidHat(hat_id));
        }
        self.hats[hat_id as usize] = direction;
        Ok(())
    }

    fn get_axis(&self, axis_id: u8) -> Result<f32, VirtualBackendError> {
        if axis_id >= self.num_axes {
            return Err(VirtualBackendError::InvalidAxis(axis_id));
        }
        Ok(self.axes[axis_id as usize])
    }

    fn get_button(&self, button_id: u8) -> Result<bool, VirtualBackendError> {
        if button_id >= self.num_buttons {
            return Err(VirtualBackendError::InvalidButton(button_id));
        }
        Ok(self.buttons[button_id as usize])
    }

    fn get_hat(&self, hat_id: u8) -> Result<HatDirection, VirtualBackendError> {
        if hat_id >= self.num_hats {
            return Err(VirtualBackendError::InvalidHat(hat_id));
        }
        Ok(self.hats[hat_id as usize])
    }

    fn axis_count(&self) -> u8 {
        self.num_axes
    }

    fn button_count(&self) -> u8 {
        self.num_buttons
    }

    fn hat_count(&self) -> u8 {
        self.num_hats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hat_direction_hid_round_trip() {
        for raw in 0..=7u8 {
            let dir = HatDirection::from_hid(raw);
            assert_eq!(dir.to_hid(), raw);
        }
        // Out of range → Centered
        assert_eq!(HatDirection::from_hid(0x10), HatDirection::Centered);
    }

    #[test]
    fn test_mock_backend_lifecycle() {
        let mut mock = MockBackend::joystick();
        assert!(!mock.is_acquired());

        mock.acquire().unwrap();
        assert!(mock.is_acquired());

        mock.set_axis(0, 0.5).unwrap();
        assert!((mock.get_axis(0).unwrap() - 0.5).abs() < f32::EPSILON);

        mock.set_button(2, true).unwrap();
        assert!(mock.get_button(2).unwrap());

        mock.set_hat(1, HatDirection::South).unwrap();
        assert_eq!(mock.get_hat(1).unwrap(), HatDirection::South);

        mock.release().unwrap();
        assert!(!mock.is_acquired());
    }

    #[test]
    fn test_mock_backend_clamping() {
        let mut mock = MockBackend::joystick();
        mock.acquire().unwrap();

        mock.set_axis(0, 5.0).unwrap();
        assert!((mock.get_axis(0).unwrap() - 1.0).abs() < f32::EPSILON);

        mock.set_axis(0, -5.0).unwrap();
        assert!((mock.get_axis(0).unwrap() - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mock_backend_error_cases() {
        let mut mock = MockBackend::new(2, 4, 1);

        // Not acquired.
        assert!(mock.set_axis(0, 0.0).is_err());

        mock.acquire().unwrap();

        // Out of range.
        assert!(mock.set_axis(2, 0.0).is_err());
        assert!(mock.set_button(4, false).is_err());
        assert!(mock.set_hat(1, HatDirection::Centered).is_err());
    }

    #[test]
    fn test_virtual_backend_error_display() {
        let e = VirtualBackendError::NotAcquired(1);
        assert_eq!(e.to_string(), "device 1 not acquired");

        let e = VirtualBackendError::InvalidAxis(99);
        assert_eq!(e.to_string(), "invalid axis id 99");
    }
}
