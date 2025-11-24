// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! XInput rumble integration for limited FFB support
//!
//! This module provides XInput rumble support as a fallback FFB mechanism for
//! Xbox-compatible controllers. XInput rumble is **severely limited** compared
//! to full force feedback:
//!
//! ## Limitations
//!
//! - **No directional torque**: XInput only provides two independent vibration motors
//!   (low-frequency and high-frequency), not directional force feedback
//! - **No spring/damper effects**: Cannot model stick centering or resistance
//! - **No constant force**: Cannot apply sustained directional loads
//! - **Vibration only**: Suitable only for coarse effects like buffeting, stall warning,
//!   and engine vibration
//!
//! ## Motor Mapping
//!
//! XInput provides two rumble motors:
//!
//! - **Low-frequency motor** (left motor): Used for buffeting, stall vibration, and
//!   low-frequency aerodynamic effects
//! - **High-frequency motor** (right motor): Used for engine vibration, fine texture
//!   effects, and high-frequency feedback
//!
//! ## Integration with FFB Pipeline
//!
//! XInput rumble is mapped into the FFB synthesis pipeline as **coarse vibration only**.
//! The telemetry synthesis engine generates vibration intensities based on flight
//! conditions, which are then mapped to the two rumble channels.
//!
//! **Important**: Full stick torque modeling (directional forces, spring centering,
//! damping) is **NOT** attempted through XInput. For realistic control loading, a
//! DirectInput-compatible force feedback device is required.
//!
//! ## Requirements
//!
//! Validates: Requirements FFB-HID-01.5

use std::time::Instant;

/// XInput rumble device for limited FFB support
///
/// Provides vibration-only feedback through XInput's two rumble motors.
/// This is a fallback mechanism for controllers without full force feedback support.
#[derive(Debug)]
pub struct XInputRumbleDevice {
    /// XInput user index (0-3)
    user_index: u32,
    /// Last low-frequency motor value (0.0-1.0)
    last_low_freq: f32,
    /// Last high-frequency motor value (0.0-1.0)
    last_high_freq: f32,
    /// Last update timestamp
    last_update: Instant,
    /// Whether device is connected
    connected: bool,
}

/// XInput rumble channel mapping
#[derive(Debug, Clone, Copy)]
pub struct RumbleChannels {
    /// Low-frequency motor intensity (0.0-1.0)
    /// Used for: buffeting, stall vibration, low-frequency aerodynamic effects
    pub low_freq: f32,
    /// High-frequency motor intensity (0.0-1.0)
    /// Used for: engine vibration, fine texture effects, high-frequency feedback
    pub high_freq: f32,
}

impl Default for RumbleChannels {
    fn default() -> Self {
        Self {
            low_freq: 0.0,
            high_freq: 0.0,
        }
    }
}

/// XInput rumble errors
#[derive(Debug, thiserror::Error)]
pub enum XInputRumbleError {
    #[error("XInput device not connected")]
    NotConnected,
    #[error("Invalid user index: {0} (must be 0-3)")]
    InvalidUserIndex(u32),
    #[error("XInput API error: {0}")]
    ApiError(String),
}

pub type Result<T> = std::result::Result<T, XInputRumbleError>;

impl XInputRumbleDevice {
    /// Create a new XInput rumble device
    ///
    /// # Arguments
    ///
    /// * `user_index` - XInput user index (0-3)
    ///
    /// # Errors
    ///
    /// Returns error if user_index is invalid (>3)
    pub fn new(user_index: u32) -> Result<Self> {
        if user_index > 3 {
            return Err(XInputRumbleError::InvalidUserIndex(user_index));
        }

        Ok(Self {
            user_index,
            last_low_freq: 0.0,
            last_high_freq: 0.0,
            last_update: Instant::now(),
            connected: false,
        })
    }

    /// Check if device is connected
    ///
    /// This should be called periodically to detect connection/disconnection.
    /// On Windows, this would use XInputGetState to check device presence.
    pub fn check_connection(&mut self) -> bool {
        // Platform-specific implementation would go here
        // For now, assume connected for testing
        #[cfg(target_os = "windows")]
        {
            // In real implementation:
            // let mut state = XINPUT_STATE::default();
            // let result = unsafe { XInputGetState(self.user_index, &mut state) };
            // self.connected = result == ERROR_SUCCESS;
            // self.connected
            
            // Placeholder for non-Windows or when XInput is not available
            self.connected = true;
            true
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // XInput is Windows-only
            self.connected = false;
            false
        }
    }

    /// Set rumble intensities for both motors
    ///
    /// # Arguments
    ///
    /// * `channels` - Rumble channel intensities (0.0-1.0 for each motor)
    ///
    /// # Errors
    ///
    /// Returns error if device is not connected or API call fails
    ///
    /// # Notes
    ///
    /// Values are automatically clamped to [0.0, 1.0] range.
    pub fn set_rumble(&mut self, channels: RumbleChannels) -> Result<()> {
        if !self.connected {
            return Err(XInputRumbleError::NotConnected);
        }

        // Clamp values to valid range
        let low_freq = channels.low_freq.clamp(0.0, 1.0);
        let high_freq = channels.high_freq.clamp(0.0, 1.0);

        // Platform-specific implementation
        #[cfg(target_os = "windows")]
        {
            // In real implementation:
            // let vibration = XINPUT_VIBRATION {
            //     wLeftMotorSpeed: (low_freq * 65535.0) as u16,
            //     wRightMotorSpeed: (high_freq * 65535.0) as u16,
            // };
            // 
            // let result = unsafe { XInputSetState(self.user_index, &vibration) };
            // if result != ERROR_SUCCESS {
            //     return Err(XInputRumbleError::ApiError(format!("XInputSetState failed: {}", result)));
            // }
            
            // Placeholder for testing
            tracing::debug!(
                "XInput rumble set: user={}, low={:.2}, high={:.2}",
                self.user_index,
                low_freq,
                high_freq
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            return Err(XInputRumbleError::ApiError(
                "XInput is only supported on Windows".to_string(),
            ));
        }

        // Update state
        self.last_low_freq = low_freq;
        self.last_high_freq = high_freq;
        self.last_update = Instant::now();

        Ok(())
    }

    /// Stop all rumble (set both motors to zero)
    pub fn stop(&mut self) -> Result<()> {
        self.set_rumble(RumbleChannels::default())
    }

    /// Get last rumble values
    pub fn get_last_rumble(&self) -> RumbleChannels {
        RumbleChannels {
            low_freq: self.last_low_freq,
            high_freq: self.last_high_freq,
        }
    }

    /// Get user index
    pub fn user_index(&self) -> u32 {
        self.user_index
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get time since last update
    pub fn time_since_last_update(&self) -> std::time::Duration {
        self.last_update.elapsed()
    }
}

impl Drop for XInputRumbleDevice {
    fn drop(&mut self) {
        // Ensure motors are stopped when device is dropped
        let _ = self.stop();
    }
}

/// Map telemetry-based effects to XInput rumble channels
///
/// This function provides a simple mapping from synthesized FFB effects to
/// XInput rumble intensities. It's designed to provide basic feedback for
/// controllers without full force feedback support.
///
/// # Effect Mapping
///
/// - **Buffeting/Stall**: Low-frequency motor
/// - **Engine vibration**: High-frequency motor
/// - **Aerodynamic loads**: Not supported (requires directional torque)
/// - **Spring/Damper**: Not supported (requires force feedback)
pub fn map_effects_to_rumble(
    buffeting_intensity: f32,
    engine_vibration: f32,
) -> RumbleChannels {
    RumbleChannels {
        low_freq: buffeting_intensity.clamp(0.0, 1.0),
        high_freq: engine_vibration.clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xinput_device_creation() {
        // Valid user indices
        for i in 0..=3 {
            let device = XInputRumbleDevice::new(i);
            assert!(device.is_ok());
            assert_eq!(device.unwrap().user_index(), i);
        }

        // Invalid user index
        let device = XInputRumbleDevice::new(4);
        assert!(device.is_err());
    }

    #[test]
    fn test_rumble_clamping() {
        let mut device = XInputRumbleDevice::new(0).unwrap();
        device.connected = true; // Simulate connected device

        // Test clamping of out-of-range values
        let channels = RumbleChannels {
            low_freq: 1.5,  // Should clamp to 1.0
            high_freq: -0.5, // Should clamp to 0.0
        };

        let result = device.set_rumble(channels);
        
        // On Windows, this should succeed; on other platforms, it should fail
        #[cfg(target_os = "windows")]
        {
            assert!(result.is_ok());
            let last = device.get_last_rumble();
            assert_eq!(last.low_freq, 1.0);
            assert_eq!(last.high_freq, 0.0);
        }

        #[cfg(not(target_os = "windows"))]
        {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_stop_rumble() {
        let mut device = XInputRumbleDevice::new(0).unwrap();
        device.connected = true;

        // Set some rumble
        let channels = RumbleChannels {
            low_freq: 0.5,
            high_freq: 0.7,
        };
        let _ = device.set_rumble(channels);

        // Stop rumble
        let result = device.stop();
        
        #[cfg(target_os = "windows")]
        {
            assert!(result.is_ok());
            let last = device.get_last_rumble();
            assert_eq!(last.low_freq, 0.0);
            assert_eq!(last.high_freq, 0.0);
        }
    }

    #[test]
    fn test_not_connected_error() {
        let mut device = XInputRumbleDevice::new(0).unwrap();
        // Don't set connected flag

        let channels = RumbleChannels {
            low_freq: 0.5,
            high_freq: 0.5,
        };

        let result = device.set_rumble(channels);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), XInputRumbleError::NotConnected));
    }

    #[test]
    fn test_effect_mapping() {
        // Test basic mapping
        let channels = map_effects_to_rumble(0.5, 0.7);
        assert_eq!(channels.low_freq, 0.5);
        assert_eq!(channels.high_freq, 0.7);

        // Test clamping in mapping function
        let channels = map_effects_to_rumble(1.5, -0.5);
        assert_eq!(channels.low_freq, 1.0);
        assert_eq!(channels.high_freq, 0.0);

        // Test zero values
        let channels = map_effects_to_rumble(0.0, 0.0);
        assert_eq!(channels.low_freq, 0.0);
        assert_eq!(channels.high_freq, 0.0);
    }

    #[test]
    fn test_drop_stops_rumble() {
        let mut device = XInputRumbleDevice::new(0).unwrap();
        device.connected = true;

        // Set some rumble
        let channels = RumbleChannels {
            low_freq: 0.8,
            high_freq: 0.6,
        };
        let _ = device.set_rumble(channels);

        // Drop should call stop()
        drop(device);
        // Can't verify the actual stop was called, but at least ensure no panic
    }

    #[test]
    fn test_time_tracking() {
        let device = XInputRumbleDevice::new(0).unwrap();
        
        // Should have very recent timestamp
        let elapsed = device.time_since_last_update();
        assert!(elapsed.as_millis() < 100);
    }
}
