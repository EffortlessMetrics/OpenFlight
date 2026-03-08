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

use crate::TimeSource;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    /// Time source for deterministic testing
    time_source: Arc<dyn TimeSource>,
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
    /// * `time_source` - Time source for deterministic testing
    ///
    /// # Errors
    ///
    /// Returns error if user_index is invalid (>3)
    pub fn new(user_index: u32, time_source: Arc<dyn TimeSource>) -> Result<Self> {
        if user_index > 3 {
            return Err(XInputRumbleError::InvalidUserIndex(user_index));
        }

        let now = time_source.now();

        Ok(Self {
            user_index,
            last_low_freq: 0.0,
            last_high_freq: 0.0,
            last_update: now,
            connected: false,
            time_source,
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
        self.last_update = self.time_source.now();

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
        self.time_source.now().duration_since(self.last_update)
    }

    /// Apply vibration to both motors (side-effect free mapping)
    ///
    /// This is the primary interface for FFB synthesis → XInput rumble mapping.
    /// It provides a simple, side-effect free way to set both motor intensities
    /// from synthesized FFB effects.
    ///
    /// # Arguments
    ///
    /// * `low` - Low-frequency motor intensity (0.0-1.0)
    ///   Used for: buffeting, stall vibration, low-frequency aerodynamic effects
    /// * `high` - High-frequency motor intensity (0.0-1.0)
    ///   Used for: engine vibration, fine texture effects, high-frequency feedback
    ///
    /// # Returns
    ///
    /// Returns Ok(()) on success, or an error if the device is not connected
    /// or the API call fails.
    ///
    /// # Notes
    ///
    /// - Values are automatically clamped to [0.0, 1.0] range
    /// - This method is side-effect free in terms of FFB synthesis - it only
    ///   maps the provided values to XInput rumble without modifying any
    ///   synthesis state
    ///
    /// # Requirements
    ///
    /// Validates: Requirements FFB-HID-01.5
    pub fn apply_vibration(&mut self, low: f32, high: f32) -> Result<()> {
        self.set_rumble(RumbleChannels {
            low_freq: low,
            high_freq: high,
        })
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
pub fn map_effects_to_rumble(buffeting_intensity: f32, engine_vibration: f32) -> RumbleChannels {
    RumbleChannels {
        low_freq: buffeting_intensity.clamp(0.0, 1.0),
        high_freq: engine_vibration.clamp(0.0, 1.0),
    }
}

/// Map FFB synthesis EffectOutput to XInput rumble channels
///
/// This function provides a side-effect free mapping from the FFB synthesis
/// pipeline output to XInput rumble channels. It extracts the relevant
/// components from the synthesized effects and maps them to the two
/// available rumble motors.
///
/// # Mapping Strategy
///
/// The mapping uses the following strategy:
///
/// - **Low-frequency motor**: Receives the overall effect intensity scaled by
///   a low-frequency factor. This captures buffeting, stall vibration, and
///   other low-frequency aerodynamic effects.
///
/// - **High-frequency motor**: Receives a frequency-based component that
///   captures engine vibration and fine texture effects. Higher synthesis
///   frequencies result in more high-frequency motor activity.
///
/// # Arguments
///
/// * `effect_output` - The synthesized FFB effect output from TelemetrySynthEngine
///
/// # Returns
///
/// RumbleChannels with low_freq and high_freq values in [0.0, 1.0] range
///
/// # Notes
///
/// - This mapping is intentionally simple and side-effect free
/// - XInput rumble cannot model directional forces, spring centering, or damping
/// - The mapping provides coarse vibration feedback only
///
/// # Requirements
///
/// Validates: Requirements FFB-HID-01.5
pub fn map_ffb_synthesis_to_rumble(effect_output: &crate::EffectOutput) -> RumbleChannels {
    // Extract intensity and frequency from effect output
    let intensity = effect_output.intensity;
    let frequency_hz = effect_output.frequency_hz;

    // Low-frequency motor: overall intensity with emphasis on low-frequency effects
    // Scale by intensity and apply a slight reduction for very high frequencies
    let low_freq_factor = if frequency_hz > 20.0 {
        0.7 // Reduce low motor for high-frequency effects
    } else {
        1.0
    };
    let low_freq = (intensity * low_freq_factor).clamp(0.0, 1.0);

    // High-frequency motor: frequency-based component
    // Higher synthesis frequencies drive more high-frequency motor activity
    // Normalize frequency to 0-1 range (assuming max useful frequency ~30Hz for rumble)
    let freq_normalized = (frequency_hz / 30.0).clamp(0.0, 1.0);
    let high_freq = (intensity * freq_normalized).clamp(0.0, 1.0);

    RumbleChannels {
        low_freq,
        high_freq,
    }
}

/// FFB backend selection result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfbBackend {
    /// DirectInput FFB device available
    DirectInput,
    /// XInput rumble fallback (limited FFB)
    XInputRumble,
    /// No FFB device available
    None,
}

/// Detect available FFB backend based on device presence
///
/// This function implements the mode negotiation logic for FFB backend selection:
/// - DirectInput FFB when FFB device present
/// - XInput rumble when only XInput available
/// - Off when nothing available
///
/// # Arguments
///
/// * `has_directinput_device` - Whether a DirectInput FFB device is connected
/// * `xinput_user_index` - Optional XInput user index to check (0-3)
/// * `time_source` - Time source for deterministic testing
///
/// # Returns
///
/// The selected FFB backend based on device availability
///
/// # Requirements
///
/// Validates: Requirements FFB-HID-01.5
pub fn detect_ffb_backend(
    has_directinput_device: bool,
    xinput_user_index: Option<u32>,
    time_source: Arc<dyn TimeSource>,
) -> FfbBackend {
    // Prefer DirectInput FFB when available
    if has_directinput_device {
        return FfbBackend::DirectInput;
    }

    // Fall back to XInput rumble if available
    if let Some(user_index) = xinput_user_index {
        if user_index <= 3 {
            // Check if XInput device is connected
            let mut device = match XInputRumbleDevice::new(user_index, time_source) {
                Ok(d) => d,
                Err(_) => return FfbBackend::None,
            };

            if device.check_connection() {
                return FfbBackend::XInputRumble;
            }
        }
    }

    // No FFB device available
    FfbBackend::None
}

/// FFB output router that directs effects to the appropriate backend
///
/// This struct manages the routing of FFB effects to either DirectInput
/// or XInput rumble based on the detected backend.
#[derive(Debug)]
pub struct FfbOutputRouter {
    backend: FfbBackend,
    xinput_device: Option<XInputRumbleDevice>,
    time_source: Arc<dyn TimeSource>,
}

impl FfbOutputRouter {
    /// Create a new FFB output router
    ///
    /// # Arguments
    ///
    /// * `backend` - The detected FFB backend to use
    /// * `xinput_user_index` - XInput user index if using XInput rumble
    /// * `time_source` - Time source for deterministic testing
    pub fn new(
        backend: FfbBackend,
        xinput_user_index: Option<u32>,
        time_source: Arc<dyn TimeSource>,
    ) -> Result<Self> {
        let xinput_device = if backend == FfbBackend::XInputRumble {
            let user_index = xinput_user_index.unwrap_or(0);
            let mut device = XInputRumbleDevice::new(user_index, time_source.clone())?;
            device.check_connection();
            Some(device)
        } else {
            None
        };

        Ok(Self {
            backend,
            xinput_device,
            time_source,
        })
    }

    /// Get the current backend
    pub fn backend(&self) -> FfbBackend {
        self.backend
    }

    /// Route FFB synthesis output to the appropriate backend
    ///
    /// For DirectInput, this returns the effect output unchanged for
    /// processing by the DirectInput device layer.
    ///
    /// For XInput rumble, this maps the effect output to rumble channels
    /// and applies the vibration.
    ///
    /// # Arguments
    ///
    /// * `effect_output` - The synthesized FFB effect output
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error if the backend operation fails
    pub fn route_effect(&mut self, effect_output: &crate::EffectOutput) -> Result<()> {
        match self.backend {
            FfbBackend::DirectInput => {
                // DirectInput effects are handled by the DirectInput device layer
                // This router just passes through for DirectInput
                Ok(())
            }
            FfbBackend::XInputRumble => {
                if let Some(ref mut device) = self.xinput_device {
                    let rumble = map_ffb_synthesis_to_rumble(effect_output);
                    device.apply_vibration(rumble.low_freq, rumble.high_freq)
                } else {
                    Err(XInputRumbleError::NotConnected)
                }
            }
            FfbBackend::None => {
                // No backend available, silently succeed
                Ok(())
            }
        }
    }

    /// Stop all FFB output
    pub fn stop(&mut self) -> Result<()> {
        if let Some(ref mut device) = self.xinput_device {
            device.stop()?;
        }
        Ok(())
    }

    /// Check if the backend is connected and operational
    pub fn is_connected(&mut self) -> bool {
        match self.backend {
            FfbBackend::DirectInput => true, // Assume DirectInput is always connected if selected
            FfbBackend::XInputRumble => {
                if let Some(ref mut device) = self.xinput_device {
                    device.check_connection()
                } else {
                    false
                }
            }
            FfbBackend::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FakeTimeSource;

    #[test]
    fn test_xinput_device_creation() {
        let time_source = Arc::new(FakeTimeSource::new());
        // Valid user indices
        for i in 0..=3 {
            let device = XInputRumbleDevice::new(i, time_source.clone());
            assert!(device.is_ok());
            assert_eq!(device.unwrap().user_index(), i);
        }

        // Invalid user index
        let device = XInputRumbleDevice::new(4, time_source.clone());
        assert!(device.is_err());
    }

    #[test]
    fn test_rumble_clamping() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
        device.connected = true; // Simulate connected device

        // Test clamping of out-of-range values
        let channels = RumbleChannels {
            low_freq: 1.5,   // Should clamp to 1.0
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
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
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
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
        // Don't set connected flag

        let channels = RumbleChannels {
            low_freq: 0.5,
            high_freq: 0.5,
        };

        let result = device.set_rumble(channels);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            XInputRumbleError::NotConnected
        ));
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
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
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
        let time_source = Arc::new(FakeTimeSource::new());
        let device = XInputRumbleDevice::new(0, time_source).unwrap();

        // Should have very recent timestamp
        let elapsed = device.time_since_last_update();
        assert!(elapsed.as_millis() < 100);
    }

    #[test]
    fn test_apply_vibration() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
        device.connected = true;

        // Test apply_vibration method
        let result = device.apply_vibration(0.5, 0.7);

        #[cfg(target_os = "windows")]
        {
            assert!(result.is_ok());
            let last = device.get_last_rumble();
            assert_eq!(last.low_freq, 0.5);
            assert_eq!(last.high_freq, 0.7);
        }

        #[cfg(not(target_os = "windows"))]
        {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_apply_vibration_clamping() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
        device.connected = true;

        // Test clamping in apply_vibration
        let result = device.apply_vibration(1.5, -0.5);

        #[cfg(target_os = "windows")]
        {
            assert!(result.is_ok());
            let last = device.get_last_rumble();
            assert_eq!(last.low_freq, 1.0);
            assert_eq!(last.high_freq, 0.0);
        }
    }

    #[test]
    fn test_ffb_synthesis_to_rumble_mapping() {
        // Test low-frequency effect (< 20Hz)
        let effect_low_freq = crate::EffectOutput {
            torque_nm: 2.0,
            frequency_hz: 10.0,
            intensity: 0.8,
            active_effects: vec!["stall_buffet".to_string()],
        };
        let rumble = map_ffb_synthesis_to_rumble(&effect_low_freq);
        // Low freq should be full intensity (no reduction for low frequency)
        assert!((rumble.low_freq - 0.8).abs() < 0.01);
        // High freq should be scaled by frequency (10/30 * 0.8 ≈ 0.267)
        assert!((rumble.high_freq - 0.267).abs() < 0.01);

        // Test high-frequency effect (> 20Hz)
        let effect_high_freq = crate::EffectOutput {
            torque_nm: 1.5,
            frequency_hz: 25.0,
            intensity: 0.6,
            active_effects: vec!["engine_vibration".to_string()],
        };
        let rumble = map_ffb_synthesis_to_rumble(&effect_high_freq);
        // Low freq should be reduced (0.6 * 0.7 = 0.42)
        assert!((rumble.low_freq - 0.42).abs() < 0.01);
        // High freq should be scaled by frequency (25/30 * 0.6 = 0.5)
        assert!((rumble.high_freq - 0.5).abs() < 0.01);

        // Test zero intensity
        let effect_zero = crate::EffectOutput {
            torque_nm: 0.0,
            frequency_hz: 15.0,
            intensity: 0.0,
            active_effects: vec![],
        };
        let rumble = map_ffb_synthesis_to_rumble(&effect_zero);
        assert_eq!(rumble.low_freq, 0.0);
        assert_eq!(rumble.high_freq, 0.0);

        // Test max frequency clamping
        let effect_max_freq = crate::EffectOutput {
            torque_nm: 3.0,
            frequency_hz: 50.0, // Above 30Hz max
            intensity: 1.0,
            active_effects: vec!["ground_roll".to_string()],
        };
        let rumble = map_ffb_synthesis_to_rumble(&effect_max_freq);
        // Low freq should be reduced (1.0 * 0.7 = 0.7)
        assert!((rumble.low_freq - 0.7).abs() < 0.01);
        // High freq should be clamped to 1.0 (50/30 > 1.0, but clamped)
        assert_eq!(rumble.high_freq, 1.0);
    }

    #[test]
    fn test_ffb_backend_detection_directinput_preferred() {
        let time_source = Arc::new(FakeTimeSource::new());
        // DirectInput should be preferred when available
        let backend = detect_ffb_backend(true, Some(0), time_source.clone());
        assert_eq!(backend, FfbBackend::DirectInput);

        // DirectInput should be preferred even without XInput
        let backend = detect_ffb_backend(true, None, time_source);
        assert_eq!(backend, FfbBackend::DirectInput);
    }

    #[test]
    fn test_ffb_backend_detection_xinput_fallback() {
        let time_source = Arc::new(FakeTimeSource::new());
        // XInput should be used when DirectInput is not available
        // Note: On non-Windows, XInput will not be connected
        let backend = detect_ffb_backend(false, Some(0), time_source);

        #[cfg(target_os = "windows")]
        {
            assert_eq!(backend, FfbBackend::XInputRumble);
        }

        #[cfg(not(target_os = "windows"))]
        {
            // XInput is not available on non-Windows
            assert_eq!(backend, FfbBackend::None);
        }
    }

    #[test]
    fn test_ffb_backend_detection_none() {
        let time_source = Arc::new(FakeTimeSource::new());
        // No backend when nothing is available
        let backend = detect_ffb_backend(false, None, time_source.clone());
        assert_eq!(backend, FfbBackend::None);

        // Invalid XInput index should result in None
        let backend = detect_ffb_backend(false, Some(5), time_source);
        assert_eq!(backend, FfbBackend::None);
    }

    #[test]
    fn test_ffb_output_router_directinput() {
        let time_source = Arc::new(FakeTimeSource::new());
        let router = FfbOutputRouter::new(FfbBackend::DirectInput, None, time_source).unwrap();
        assert_eq!(router.backend(), FfbBackend::DirectInput);
    }

    #[test]
    fn test_ffb_output_router_none() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut router = FfbOutputRouter::new(FfbBackend::None, None, time_source).unwrap();
        assert_eq!(router.backend(), FfbBackend::None);
        assert!(!router.is_connected());

        // Routing should succeed silently for None backend
        let effect = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 15.0,
            intensity: 0.5,
            active_effects: vec![],
        };
        assert!(router.route_effect(&effect).is_ok());
    }

    #[test]
    fn test_ffb_output_router_stop() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut router = FfbOutputRouter::new(FfbBackend::None, None, time_source).unwrap();
        // Stop should succeed for None backend
        assert!(router.stop().is_ok());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_ffb_output_router_xinput() {
        let time_source = Arc::new(FakeTimeSource::new());
        let router = FfbOutputRouter::new(FfbBackend::XInputRumble, Some(0), time_source);
        assert!(router.is_ok());
        let router = router.unwrap();
        assert_eq!(router.backend(), FfbBackend::XInputRumble);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_ffb_output_router_xinput_routing() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut router = FfbOutputRouter::new(FfbBackend::XInputRumble, Some(0), time_source).unwrap();

        let effect = crate::EffectOutput {
            torque_nm: 2.0,
            frequency_hz: 15.0,
            intensity: 0.6,
            active_effects: vec!["stall_buffet".to_string()],
        };

        // Routing should succeed on Windows
        let result = router.route_effect(&effect);
        assert!(result.is_ok());
    }

    // ============================================================================
    // Additional comprehensive tests for XInput rumble mapping logic
    // These tests validate FFB-HID-01.5 requirements and can run in CI without
    // a real controller by using mocked/simulated device state.
    // ============================================================================

    /// Test RumbleChannels default values
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_rumble_channels_default() {
        let channels = RumbleChannels::default();
        assert_eq!(channels.low_freq, 0.0);
        assert_eq!(channels.high_freq, 0.0);
    }

    /// Test all valid XInput user indices (0-3)
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_xinput_all_valid_user_indices() {
        let time_source = Arc::new(FakeTimeSource::new());
        for i in 0..=3 {
            let device = XInputRumbleDevice::new(i, time_source.clone());
            assert!(device.is_ok(), "User index {} should be valid", i);
            let device = device.unwrap();
            assert_eq!(device.user_index(), i);
            assert!(!device.is_connected()); // Not connected by default
        }
    }

    /// Test all invalid XInput user indices (4+)
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_xinput_invalid_user_indices() {
        let time_source = Arc::new(FakeTimeSource::new());
        for i in 4..=10 {
            let device = XInputRumbleDevice::new(i, time_source.clone());
            assert!(device.is_err(), "User index {} should be invalid", i);
            match device.unwrap_err() {
                XInputRumbleError::InvalidUserIndex(idx) => assert_eq!(idx, i),
                _ => panic!("Expected InvalidUserIndex error"),
            }
        }
    }

    /// Test boundary values for rumble clamping
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_rumble_boundary_values() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
        device.connected = true;

        // Test exact boundary values
        let test_cases = [
            (0.0, 0.0, 0.0, 0.0),      // Minimum
            (1.0, 1.0, 1.0, 1.0),      // Maximum
            (0.5, 0.5, 0.5, 0.5),      // Middle
            (-0.001, 0.0, 0.0, 0.0),   // Just below minimum
            (1.001, 1.0, 1.0, 1.0),    // Just above maximum
            (f32::MIN, 0.0, 0.0, 0.0), // Extreme negative
            (f32::MAX, 1.0, 1.0, 1.0), // Extreme positive
        ];

        for (low_in, high_in, expected_low, expected_high) in test_cases {
            let channels = RumbleChannels {
                low_freq: low_in,
                high_freq: high_in,
            };

            #[cfg(target_os = "windows")]
            {
                let result = device.set_rumble(channels);
                assert!(result.is_ok());
                let last = device.get_last_rumble();
                assert_eq!(
                    last.low_freq, expected_low,
                    "Low freq clamping failed for input {}",
                    low_in
                );
                assert_eq!(
                    last.high_freq, expected_high,
                    "High freq clamping failed for input {}",
                    high_in
                );
            }
        }
    }

    /// Test FFB synthesis mapping with various effect types
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_ffb_synthesis_mapping_effect_types() {
        // Stall buffet - low frequency, high intensity
        let stall_buffet = crate::EffectOutput {
            torque_nm: 3.0,
            frequency_hz: 8.0,
            intensity: 0.9,
            active_effects: vec!["stall_buffet".to_string()],
        };
        let rumble = map_ffb_synthesis_to_rumble(&stall_buffet);
        assert!(
            (rumble.low_freq - 0.9).abs() < 0.01,
            "Stall buffet should have high low_freq"
        );
        assert!(
            rumble.high_freq < rumble.low_freq,
            "Stall buffet should favor low_freq motor"
        );

        // Engine vibration - high frequency
        let engine_vib = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 28.0,
            intensity: 0.5,
            active_effects: vec!["engine_vibration".to_string()],
        };
        let rumble = map_ffb_synthesis_to_rumble(&engine_vib);
        assert!(
            (rumble.low_freq - 0.35).abs() < 0.01,
            "Engine vib low_freq should be reduced"
        );
        assert!(
            rumble.high_freq > 0.4,
            "Engine vib should have significant high_freq"
        );

        // Ground roll - medium frequency
        let ground_roll = crate::EffectOutput {
            torque_nm: 2.0,
            frequency_hz: 15.0,
            intensity: 0.7,
            active_effects: vec!["ground_roll".to_string()],
        };
        let rumble = map_ffb_synthesis_to_rumble(&ground_roll);
        assert!(
            (rumble.low_freq - 0.7).abs() < 0.01,
            "Ground roll should have full low_freq"
        );
        // 15/30 * 0.7 = 0.35
        assert!(
            (rumble.high_freq - 0.35).abs() < 0.01,
            "Ground roll high_freq calculation"
        );
    }

    /// Test FFB synthesis mapping frequency threshold at exactly 20Hz
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_ffb_synthesis_frequency_threshold() {
        // Just below 20Hz - no reduction
        let below_threshold = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 19.9,
            intensity: 1.0,
            active_effects: vec![],
        };
        let rumble = map_ffb_synthesis_to_rumble(&below_threshold);
        assert_eq!(rumble.low_freq, 1.0, "Below 20Hz should have full low_freq");

        // Just above 20Hz - reduced
        let above_threshold = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 20.1,
            intensity: 1.0,
            active_effects: vec![],
        };
        let rumble = map_ffb_synthesis_to_rumble(&above_threshold);
        assert!(
            (rumble.low_freq - 0.7).abs() < 0.01,
            "Above 20Hz should have reduced low_freq"
        );

        // Exactly at 20Hz - no reduction (boundary is > 20)
        let at_threshold = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 20.0,
            intensity: 1.0,
            active_effects: vec![],
        };
        let rumble = map_ffb_synthesis_to_rumble(&at_threshold);
        assert_eq!(
            rumble.low_freq, 1.0,
            "At exactly 20Hz should have full low_freq"
        );
    }

    /// Test FFB synthesis mapping with zero and near-zero values
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_ffb_synthesis_zero_values() {
        // Zero intensity
        let zero_intensity = crate::EffectOutput {
            torque_nm: 5.0,
            frequency_hz: 15.0,
            intensity: 0.0,
            active_effects: vec!["test".to_string()],
        };
        let rumble = map_ffb_synthesis_to_rumble(&zero_intensity);
        assert_eq!(rumble.low_freq, 0.0);
        assert_eq!(rumble.high_freq, 0.0);

        // Zero frequency
        let zero_freq = crate::EffectOutput {
            torque_nm: 2.0,
            frequency_hz: 0.0,
            intensity: 0.8,
            active_effects: vec![],
        };
        let rumble = map_ffb_synthesis_to_rumble(&zero_freq);
        assert_eq!(rumble.low_freq, 0.8);
        assert_eq!(rumble.high_freq, 0.0); // 0/30 * 0.8 = 0

        // Near-zero intensity
        let near_zero = crate::EffectOutput {
            torque_nm: 0.1,
            frequency_hz: 10.0,
            intensity: 0.001,
            active_effects: vec![],
        };
        let rumble = map_ffb_synthesis_to_rumble(&near_zero);
        assert!(rumble.low_freq < 0.01);
        assert!(rumble.high_freq < 0.01);
    }

    /// Test map_effects_to_rumble with various input combinations
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_map_effects_to_rumble_combinations() {
        // Both channels active
        let channels = map_effects_to_rumble(0.6, 0.4);
        assert_eq!(channels.low_freq, 0.6);
        assert_eq!(channels.high_freq, 0.4);

        // Only low frequency
        let channels = map_effects_to_rumble(0.8, 0.0);
        assert_eq!(channels.low_freq, 0.8);
        assert_eq!(channels.high_freq, 0.0);

        // Only high frequency
        let channels = map_effects_to_rumble(0.0, 0.9);
        assert_eq!(channels.low_freq, 0.0);
        assert_eq!(channels.high_freq, 0.9);

        // Both at maximum
        let channels = map_effects_to_rumble(1.0, 1.0);
        assert_eq!(channels.low_freq, 1.0);
        assert_eq!(channels.high_freq, 1.0);
    }

    /// Test FfbBackend enum equality and debug
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_ffb_backend_enum() {
        assert_eq!(FfbBackend::DirectInput, FfbBackend::DirectInput);
        assert_eq!(FfbBackend::XInputRumble, FfbBackend::XInputRumble);
        assert_eq!(FfbBackend::None, FfbBackend::None);
        assert_ne!(FfbBackend::DirectInput, FfbBackend::XInputRumble);
        assert_ne!(FfbBackend::DirectInput, FfbBackend::None);
        assert_ne!(FfbBackend::XInputRumble, FfbBackend::None);

        // Test Debug trait
        let debug_str = format!("{:?}", FfbBackend::DirectInput);
        assert!(debug_str.contains("DirectInput"));
    }

    /// Test backend detection with all user index combinations
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_backend_detection_all_user_indices() {
        let time_source = Arc::new(FakeTimeSource::new());
        // DirectInput always wins regardless of XInput index
        for i in 0..=3 {
            let backend = detect_ffb_backend(true, Some(i), time_source.clone());
            assert_eq!(backend, FfbBackend::DirectInput);
        }

        // Without DirectInput, valid XInput indices should work (on Windows)
        for i in 0..=3 {
            let backend = detect_ffb_backend(false, Some(i), time_source.clone());
            #[cfg(target_os = "windows")]
            assert_eq!(backend, FfbBackend::XInputRumble);
            #[cfg(not(target_os = "windows"))]
            assert_eq!(backend, FfbBackend::None);
        }

        // Invalid XInput indices should result in None
        for i in 4..=10 {
            let backend = detect_ffb_backend(false, Some(i), time_source.clone());
            assert_eq!(backend, FfbBackend::None);
        }
    }

    /// Test XInputRumbleDevice state tracking
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_device_state_tracking() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();

        // Initial state
        assert!(!device.is_connected());
        let initial_rumble = device.get_last_rumble();
        assert_eq!(initial_rumble.low_freq, 0.0);
        assert_eq!(initial_rumble.high_freq, 0.0);

        // Simulate connection
        device.connected = true;
        assert!(device.is_connected());

        // Set rumble and verify state tracking
        #[cfg(target_os = "windows")]
        {
            let _ = device.set_rumble(RumbleChannels {
                low_freq: 0.3,
                high_freq: 0.7,
            });
            let rumble = device.get_last_rumble();
            assert_eq!(rumble.low_freq, 0.3);
            assert_eq!(rumble.high_freq, 0.7);

            // Update again
            let _ = device.set_rumble(RumbleChannels {
                low_freq: 0.9,
                high_freq: 0.1,
            });
            let rumble = device.get_last_rumble();
            assert_eq!(rumble.low_freq, 0.9);
            assert_eq!(rumble.high_freq, 0.1);
        }
    }

    /// Test FfbOutputRouter with DirectInput backend passthrough
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_router_directinput_passthrough() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut router = FfbOutputRouter::new(FfbBackend::DirectInput, None, time_source).unwrap();

        // DirectInput routing should succeed (passthrough)
        let effect = crate::EffectOutput {
            torque_nm: 5.0,
            frequency_hz: 20.0,
            intensity: 0.8,
            active_effects: vec!["constant_force".to_string()],
        };

        let result = router.route_effect(&effect);
        assert!(result.is_ok());

        // DirectInput is always considered connected
        assert!(router.is_connected());
    }

    /// Test FfbOutputRouter stop behavior for all backends
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_router_stop_all_backends() {
        let time_source = Arc::new(FakeTimeSource::new());
        // None backend
        let mut router_none = FfbOutputRouter::new(FfbBackend::None, None, time_source.clone()).unwrap();
        assert!(router_none.stop().is_ok());

        // DirectInput backend
        let mut router_di = FfbOutputRouter::new(FfbBackend::DirectInput, None, time_source.clone()).unwrap();
        assert!(router_di.stop().is_ok());

        // XInput backend (Windows only)
        #[cfg(target_os = "windows")]
        {
            let mut router_xi = FfbOutputRouter::new(FfbBackend::XInputRumble, Some(0), time_source).unwrap();
            assert!(router_xi.stop().is_ok());
        }
    }

    /// Test error message formatting
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_error_messages() {
        let not_connected = XInputRumbleError::NotConnected;
        assert!(format!("{}", not_connected).contains("not connected"));

        let invalid_index = XInputRumbleError::InvalidUserIndex(5);
        assert!(format!("{}", invalid_index).contains("5"));
        assert!(format!("{}", invalid_index).contains("0-3"));

        let api_error = XInputRumbleError::ApiError("test error".to_string());
        assert!(format!("{}", api_error).contains("test error"));
    }

    /// Test multiple rapid rumble updates (simulates real-time FFB loop)
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_rapid_rumble_updates() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();
        device.connected = true;

        // Simulate 100 rapid updates (like a 100Hz FFB loop)
        #[cfg(target_os = "windows")]
        {
            for i in 0..100 {
                let intensity = (i as f32 / 100.0).sin().abs();
                let result = device.apply_vibration(intensity, 1.0 - intensity);
                assert!(result.is_ok());

                let rumble = device.get_last_rumble();
                assert!((rumble.low_freq - intensity).abs() < 0.001);
                assert!((rumble.high_freq - (1.0 - intensity)).abs() < 0.001);
            }
        }
    }

    /// Test FFB synthesis mapping preserves effect information
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_ffb_synthesis_mapping_preserves_proportions() {
        // Test that relative proportions are maintained
        let effect1 = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 10.0,
            intensity: 0.4,
            active_effects: vec![],
        };
        let effect2 = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 10.0,
            intensity: 0.8,
            active_effects: vec![],
        };

        let rumble1 = map_ffb_synthesis_to_rumble(&effect1);
        let rumble2 = map_ffb_synthesis_to_rumble(&effect2);

        // Double intensity should result in double rumble (within clamping limits)
        assert!((rumble2.low_freq / rumble1.low_freq - 2.0).abs() < 0.01);
        assert!((rumble2.high_freq / rumble1.high_freq - 2.0).abs() < 0.01);
    }

    /// Test XInput device connection state changes
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_connection_state_changes() {
        let time_source = Arc::new(FakeTimeSource::new());
        let mut device = XInputRumbleDevice::new(0, time_source).unwrap();

        // Initially not connected
        assert!(!device.is_connected());

        // Attempt to set rumble while disconnected
        let result = device.set_rumble(RumbleChannels {
            low_freq: 0.5,
            high_freq: 0.5,
        });
        assert!(matches!(
            result.unwrap_err(),
            XInputRumbleError::NotConnected
        ));

        // Simulate connection
        device.connected = true;
        assert!(device.is_connected());

        // Now rumble should work (on Windows)
        #[cfg(target_os = "windows")]
        {
            let result = device.set_rumble(RumbleChannels {
                low_freq: 0.5,
                high_freq: 0.5,
            });
            assert!(result.is_ok());
        }

        // Simulate disconnection
        device.connected = false;
        assert!(!device.is_connected());

        // Rumble should fail again
        let result = device.set_rumble(RumbleChannels {
            low_freq: 0.5,
            high_freq: 0.5,
        });
        assert!(matches!(
            result.unwrap_err(),
            XInputRumbleError::NotConnected
        ));
    }

    /// Test that torque_nm field in EffectOutput doesn't affect rumble mapping
    /// (XInput rumble is intensity-based, not torque-based)
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_torque_independence() {
        let effect_low_torque = crate::EffectOutput {
            torque_nm: 0.1,
            frequency_hz: 15.0,
            intensity: 0.5,
            active_effects: vec![],
        };
        let effect_high_torque = crate::EffectOutput {
            torque_nm: 10.0,
            frequency_hz: 15.0,
            intensity: 0.5,
            active_effects: vec![],
        };

        let rumble_low = map_ffb_synthesis_to_rumble(&effect_low_torque);
        let rumble_high = map_ffb_synthesis_to_rumble(&effect_high_torque);

        // Rumble should be identical regardless of torque
        assert_eq!(rumble_low.low_freq, rumble_high.low_freq);
        assert_eq!(rumble_low.high_freq, rumble_high.high_freq);
    }

    /// Test active_effects field doesn't affect rumble mapping
    /// (mapping is purely based on intensity and frequency)
    /// Validates: Requirements FFB-HID-01.5
    #[test]
    fn test_active_effects_independence() {
        let effect_no_effects = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 15.0,
            intensity: 0.6,
            active_effects: vec![],
        };
        let effect_many_effects = crate::EffectOutput {
            torque_nm: 1.0,
            frequency_hz: 15.0,
            intensity: 0.6,
            active_effects: vec![
                "stall_buffet".to_string(),
                "engine_vibration".to_string(),
                "ground_roll".to_string(),
            ],
        };

        let rumble_no = map_ffb_synthesis_to_rumble(&effect_no_effects);
        let rumble_many = map_ffb_synthesis_to_rumble(&effect_many_effects);

        // Rumble should be identical regardless of active effects list
        assert_eq!(rumble_no.low_freq, rumble_many.low_freq);
        assert_eq!(rumble_no.high_freq, rumble_many.high_freq);
    }
}
