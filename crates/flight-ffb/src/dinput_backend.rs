// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DirectInput FFB backend for the FFB pipeline
//!
//! This module provides a `DirectInputBackend` that implements the FFB output
//! interface, connecting the FFB pipeline to DirectInput devices. It handles:
//!
//! - Rate limiting (60-100Hz) to avoid USB saturation
//! - Dirty-checking to skip redundant effect updates
//! - Integration with SafetyEnvelope for torque processing
//! - Device disconnect detection and fault handling
//!
//! # Usage
//!
//! ```ignore
//! let mut backend = DirectInputBackend::new(device)?;
//! backend.initialize()?;
//!
//! // In the FFB loop (called at RT rate, internally rate-limited)
//! backend.set_axis_torque(0, pitch_torque)?;
//! backend.set_axis_torque(1, roll_torque)?;
//! ```

#![allow(dead_code)]

use std::time::{Duration, Instant};

use crate::dinput_device::{DInputError, DirectInputFfbDevice, FfbCapabilities};
use crate::safety_envelope::SafetyEnvelope;

/// Default update rate (Hz) for DirectInput effects
/// This is rate-limited to avoid USB saturation
pub const DEFAULT_UPDATE_RATE_HZ: u32 = 100;

/// Minimum update interval (enforced rate limit)
pub const MIN_UPDATE_INTERVAL: Duration = Duration::from_millis(10); // 100 Hz max

/// Dirty-check threshold - skip updates if change is less than this percentage
pub const DIRTY_CHECK_THRESHOLD_PERCENT: f32 = 1.0;

/// DirectInput backend errors
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("DirectInput error: {0}")]
    DInputError(#[from] DInputError),

    #[error("Backend not initialized")]
    NotInitialized,

    #[error("Device disconnected")]
    DeviceDisconnected,

    #[error("Invalid axis index: {0}")]
    InvalidAxis(u32),

    #[error("Safety envelope rejected value: {value} Nm (limit: {limit} Nm)")]
    SafetyRejected { value: f32, limit: f32 },
}

pub type Result<T> = std::result::Result<T, BackendError>;

/// Axis effect state for dirty-checking and rate limiting
#[derive(Debug)]
struct AxisEffectState {
    /// Effect handle index in DirectInputFfbDevice
    effect_handle: Option<usize>,
    /// Last torque value sent (in Nm)
    last_torque_nm: f32,
    /// Last update time
    last_update: Instant,
    /// Whether effect has been started
    is_started: bool,
}

impl Default for AxisEffectState {
    fn default() -> Self {
        Self {
            effect_handle: None,
            last_torque_nm: 0.0,
            last_update: Instant::now() - Duration::from_secs(1), // Allow immediate first update
            is_started: false,
        }
    }
}

/// DirectInput FFB backend
///
/// This struct provides the connection between the FFB pipeline and DirectInput
/// devices. It handles rate limiting, dirty-checking, and safety envelope
/// integration.
///
/// # Rate Limiting
///
/// DirectInput effect updates are rate-limited to 60-100Hz to avoid USB
/// saturation. The RT loop may call this at 250Hz, but actual USB writes
/// only occur at the configured rate.
///
/// # Dirty-Checking
///
/// Effect parameters are only updated if they've changed by more than 1%.
/// This further reduces USB traffic for steady-state conditions.
pub struct DirectInputBackend {
    /// The underlying DirectInput device
    device: DirectInputFfbDevice,
    /// Constant force effect states for each axis (pitch, roll)
    axis_effects: [AxisEffectState; 2],
    /// Minimum interval between updates (rate limiting)
    min_update_interval: Duration,
    /// Safety envelope for torque limiting
    safety_envelope: Option<SafetyEnvelope>,
    /// Whether the backend has been initialized
    initialized: bool,
    /// Device capabilities (cached)
    capabilities: Option<FfbCapabilities>,
    /// Statistics: total update calls
    stats_update_calls: u64,
    /// Statistics: actual USB writes (after rate limiting)
    stats_usb_writes: u64,
    /// Statistics: skipped due to dirty-check
    stats_dirty_skips: u64,
}

impl DirectInputBackend {
    /// Create a new DirectInput backend with a device
    ///
    /// # Arguments
    /// * `device` - The DirectInput FFB device to use
    pub fn new(device: DirectInputFfbDevice) -> Self {
        Self {
            device,
            axis_effects: [AxisEffectState::default(), AxisEffectState::default()],
            min_update_interval: MIN_UPDATE_INTERVAL,
            safety_envelope: None,
            initialized: false,
            capabilities: None,
            stats_update_calls: 0,
            stats_usb_writes: 0,
            stats_dirty_skips: 0,
        }
    }

    /// Create a new DirectInput backend with a device and safety envelope
    ///
    /// # Arguments
    /// * `device` - The DirectInput FFB device to use
    /// * `safety_envelope` - Safety envelope for torque limiting
    pub fn new_with_safety(device: DirectInputFfbDevice, safety_envelope: SafetyEnvelope) -> Self {
        Self {
            device,
            axis_effects: [AxisEffectState::default(), AxisEffectState::default()],
            min_update_interval: MIN_UPDATE_INTERVAL,
            safety_envelope: Some(safety_envelope),
            initialized: false,
            capabilities: None,
            stats_update_calls: 0,
            stats_usb_writes: 0,
            stats_dirty_skips: 0,
        }
    }

    /// Set the update rate limit
    ///
    /// # Arguments
    /// * `hz` - Update rate in Hz (clamped to 60-100 Hz)
    pub fn set_update_rate(&mut self, hz: u32) {
        let clamped_hz = hz.clamp(60, 100);
        self.min_update_interval = Duration::from_micros(1_000_000 / clamped_hz as u64);
        tracing::debug!(
            "DirectInput backend update rate set to {} Hz (interval: {:?})",
            clamped_hz,
            self.min_update_interval
        );
    }

    /// Set the safety envelope
    pub fn set_safety_envelope(&mut self, envelope: SafetyEnvelope) {
        self.safety_envelope = Some(envelope);
    }

    /// Initialize the backend
    ///
    /// This method:
    /// 1. Initializes the DirectInput device
    /// 2. Queries device capabilities
    /// 3. Acquires the device
    /// 4. Creates constant force effects for pitch and roll axes
    pub fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Initialize DirectInput device
        self.device.initialize()?;

        // Query capabilities
        let caps = self.device.query_capabilities()?;
        self.capabilities = Some(caps.clone());

        // Acquire device
        self.device.acquire(0)?;

        // Create constant force effects for each axis
        for axis in 0..2 {
            let handle = self.device.create_constant_force_effect(axis)?;
            self.axis_effects[axis as usize].effect_handle = Some(handle);

            // Start the effect (with zero force)
            self.device.start_effect(handle)?;
            self.axis_effects[axis as usize].is_started = true;
        }

        self.initialized = true;
        tracing::info!("DirectInput backend initialized successfully");

        Ok(())
    }

    /// Set axis torque with rate limiting and dirty-checking
    ///
    /// # Arguments
    /// * `axis` - Axis index (0 = pitch, 1 = roll)
    /// * `torque_nm` - Torque in Newton-meters
    ///
    /// # Returns
    /// * `Ok(true)` if the update was actually sent to the device
    /// * `Ok(false)` if the update was skipped (rate limit or dirty-check)
    /// * `Err` on error
    pub fn set_axis_torque(&mut self, axis: u32, torque_nm: f32) -> Result<bool> {
        self.stats_update_calls += 1;

        if !self.initialized {
            return Err(BackendError::NotInitialized);
        }

        if axis > 1 {
            return Err(BackendError::InvalidAxis(axis));
        }

        // Apply safety envelope if configured
        let safe_torque = if let Some(ref mut envelope) = self.safety_envelope {
            let max_torque = self
                .capabilities
                .as_ref()
                .map(|c| c.max_torque_nm)
                .unwrap_or(15.0);

            // Check for NaN/Inf before processing
            if torque_nm.is_nan() || torque_nm.is_infinite() {
                return Err(BackendError::SafetyRejected {
                    value: torque_nm,
                    limit: max_torque,
                });
            }

            // Apply envelope (clamps and validates)
            // Second parameter is safe_for_ffb - true since we're actively sending FFB
            envelope.apply(torque_nm, true).map_err(|_| BackendError::SafetyRejected {
                value: torque_nm,
                limit: max_torque,
            })?
        } else {
            torque_nm
        };

        let axis_state = &mut self.axis_effects[axis as usize];

        // Rate limiting: check if enough time has passed since last update
        let now = Instant::now();
        if now.duration_since(axis_state.last_update) < self.min_update_interval {
            return Ok(false); // Skip due to rate limit
        }

        // Dirty-checking: skip if change is less than threshold
        let torque_diff = (safe_torque - axis_state.last_torque_nm).abs();
        let max_torque = self
            .capabilities
            .as_ref()
            .map(|c| c.max_torque_nm)
            .unwrap_or(15.0);
        let threshold = max_torque * DIRTY_CHECK_THRESHOLD_PERCENT / 100.0;

        if torque_diff < threshold && axis_state.last_torque_nm != 0.0 {
            self.stats_dirty_skips += 1;
            return Ok(false); // Skip due to dirty-check
        }

        // Get effect handle
        let handle = axis_state
            .effect_handle
            .ok_or(BackendError::NotInitialized)?;

        // Update the effect
        match self.device.set_constant_force(handle, safe_torque) {
            Ok(()) => {
                axis_state.last_torque_nm = safe_torque;
                axis_state.last_update = now;
                self.stats_usb_writes += 1;
                Ok(true)
            }
            Err(DInputError::DeviceDisconnected) => Err(BackendError::DeviceDisconnected),
            Err(e) => Err(BackendError::DInputError(e)),
        }
    }

    /// Emergency stop: immediately set all axes to zero
    ///
    /// This bypasses rate limiting and dirty-checking.
    pub fn emergency_stop(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(BackendError::NotInitialized);
        }

        for axis in 0..2 {
            let axis_state = &mut self.axis_effects[axis];
            if let Some(handle) = axis_state.effect_handle {
                // Stop the effect
                let _ = self.device.stop_effect(handle);
                axis_state.last_torque_nm = 0.0;
                axis_state.is_started = false;
            }
        }

        tracing::warn!("DirectInput backend: emergency stop executed");
        Ok(())
    }

    /// Resume from emergency stop
    ///
    /// Restarts the effects that were stopped.
    pub fn resume(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(BackendError::NotInitialized);
        }

        for axis in 0..2 {
            let axis_state = &mut self.axis_effects[axis];
            if let Some(handle) = axis_state.effect_handle {
                if !axis_state.is_started {
                    self.device.start_effect(handle)?;
                    axis_state.is_started = true;
                }
            }
        }

        tracing::info!("DirectInput backend: resumed from emergency stop");
        Ok(())
    }

    /// Get the underlying device
    pub fn device(&self) -> &DirectInputFfbDevice {
        &self.device
    }

    /// Get mutable access to the underlying device
    pub fn device_mut(&mut self) -> &mut DirectInputFfbDevice {
        &mut self.device
    }

    /// Check if backend is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get device capabilities
    pub fn capabilities(&self) -> Option<&FfbCapabilities> {
        self.capabilities.as_ref()
    }

    /// Get backend statistics
    pub fn statistics(&self) -> BackendStatistics {
        BackendStatistics {
            update_calls: self.stats_update_calls,
            usb_writes: self.stats_usb_writes,
            dirty_skips: self.stats_dirty_skips,
            write_rate: if self.stats_update_calls > 0 {
                self.stats_usb_writes as f64 / self.stats_update_calls as f64
            } else {
                0.0
            },
        }
    }

    /// Reset statistics
    pub fn reset_statistics(&mut self) {
        self.stats_update_calls = 0;
        self.stats_usb_writes = 0;
        self.stats_dirty_skips = 0;
    }

    /// Get last torque value for an axis
    pub fn get_last_torque(&self, axis: u32) -> Option<f32> {
        if axis <= 1 {
            Some(self.axis_effects[axis as usize].last_torque_nm)
        } else {
            None
        }
    }
}

impl Drop for DirectInputBackend {
    fn drop(&mut self) {
        // Stop all effects before dropping
        if self.initialized {
            let _ = self.emergency_stop();
        }
        tracing::debug!("DirectInput backend dropped");
    }
}

/// Backend statistics
#[derive(Debug, Clone)]
pub struct BackendStatistics {
    /// Total number of update calls
    pub update_calls: u64,
    /// Actual USB writes (after rate limiting)
    pub usb_writes: u64,
    /// Updates skipped due to dirty-check
    pub dirty_skips: u64,
    /// Ratio of writes to calls (efficiency)
    pub write_rate: f64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_axis_effect_state() {
        let state = AxisEffectState::default();
        assert!(state.effect_handle.is_none());
        assert_eq!(state.last_torque_nm, 0.0);
        assert!(!state.is_started);
    }

    #[test]
    fn test_backend_creation() {
        #[cfg(windows)]
        {
            let device = DirectInputFfbDevice::new("{00000000-0000-0000-0000-000000000000}".to_string()).unwrap();
            let backend = DirectInputBackend::new(device);

            assert!(!backend.is_initialized());
            assert!(backend.capabilities().is_none());
        }
    }

    #[test]
    fn test_update_rate_clamping() {
        #[cfg(windows)]
        {
            let device = DirectInputFfbDevice::new("{00000000-0000-0000-0000-000000000000}".to_string()).unwrap();
            let mut backend = DirectInputBackend::new(device);

            // Test clamping to minimum
            backend.set_update_rate(30);
            assert_eq!(
                backend.min_update_interval,
                Duration::from_micros(1_000_000 / 60)
            );

            // Test clamping to maximum
            backend.set_update_rate(200);
            assert_eq!(
                backend.min_update_interval,
                Duration::from_micros(1_000_000 / 100)
            );

            // Test valid rate
            backend.set_update_rate(80);
            assert_eq!(
                backend.min_update_interval,
                Duration::from_micros(1_000_000 / 80)
            );
        }
    }

    #[test]
    fn test_statistics() {
        #[cfg(windows)]
        {
            let device = DirectInputFfbDevice::new("{00000000-0000-0000-0000-000000000000}".to_string()).unwrap();
            let backend = DirectInputBackend::new(device);

            let stats = backend.statistics();
            assert_eq!(stats.update_calls, 0);
            assert_eq!(stats.usb_writes, 0);
            assert_eq!(stats.dirty_skips, 0);
            assert_eq!(stats.write_rate, 0.0);
        }
    }

    #[test]
    fn test_not_initialized_error() {
        #[cfg(windows)]
        {
            let device = DirectInputFfbDevice::new("{00000000-0000-0000-0000-000000000000}".to_string()).unwrap();
            let mut backend = DirectInputBackend::new(device);

            let result = backend.set_axis_torque(0, 5.0);
            assert!(matches!(result, Err(BackendError::NotInitialized)));
        }
    }

    #[test]
    fn test_invalid_axis_error() {
        #[cfg(windows)]
        {
            // We can't fully initialize without a real device, so this test
            // verifies the error handling for invalid axis indices
            let device = DirectInputFfbDevice::new("{00000000-0000-0000-0000-000000000000}".to_string()).unwrap();
            let mut backend = DirectInputBackend::new(device);
            backend.initialized = true; // Force initialized for test

            let result = backend.set_axis_torque(5, 5.0);
            assert!(matches!(result, Err(BackendError::InvalidAxis(5))));
        }
    }
}
