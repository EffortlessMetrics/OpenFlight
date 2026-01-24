// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! FFB Pipeline Integration
//!
//! Wires SafetyEnvelope into the FFB output pipeline, ensuring all torque commands
//! pass through safety checks before reaching hardware.
//!
//! **Pipeline:** raw_torque → SafetyEnvelope::apply_limits(safe_for_ffb) → DirectInput/XInput/OFP-1
//!
//! **Validates: Requirements FFB-SAFETY-01.1-6**
//!
//! # Safety Guarantees
//! - Torque never exceeds device max_torque_nm
//! - Slew and jerk limits enforced per axis
//! - When safe_for_ffb == false, ramp to zero in ≤50ms from fault detection
//! - Hardware-critical faults (over-temp/current) latch, require power cycle
//! - Captures `fault_initial_torque` at fault detection (not `last_setpoint`)

use std::time::{Duration, Instant};
use thiserror::Error;

use crate::fault::FaultType;
use crate::safety_envelope::{SafetyEnvelope, SafetyEnvelopeConfig, SafetyEnvelopeError};

/// FFB Pipeline errors
#[derive(Debug, Error)]
pub enum FfbPipelineError {
    #[error("Safety envelope error: {0}")]
    SafetyEnvelopeError(#[from] SafetyEnvelopeError),

    #[error("Invalid torque value: {value} (must be finite)")]
    InvalidTorque { value: f32 },

    #[error("NaN/Inf detected in pipeline at {context}: {value}")]
    NanInPipeline { value: f32, context: String },

    #[error("Pipeline not initialized")]
    NotInitialized,

    #[error("Hardware fault latched: {fault_type:?} - requires power cycle")]
    HardwareFaultLatched { fault_type: FaultType },

    #[error("Output backend error: {message}")]
    OutputError { message: String },
}

pub type FfbPipelineResult<T> = std::result::Result<T, FfbPipelineError>;

/// Per-axis safety envelope for independent pitch/roll control
#[derive(Debug)]
pub struct AxisSafetyEnvelope {
    /// Axis identifier (0 = pitch, 1 = roll)
    pub axis_index: u32,
    /// Safety envelope for this axis
    envelope: SafetyEnvelope,
    /// Last raw torque input (before safety processing)
    last_raw_torque_nm: f32,
    /// Last safe torque output (after safety processing)
    last_safe_torque_nm: f32,
    /// Timestamp of last update
    last_update: Instant,
}

impl AxisSafetyEnvelope {
    /// Create new axis safety envelope
    pub fn new(axis_index: u32, config: SafetyEnvelopeConfig) -> FfbPipelineResult<Self> {
        let envelope = SafetyEnvelope::new(config)?;
        Ok(Self {
            axis_index,
            envelope,
            last_raw_torque_nm: 0.0,
            last_safe_torque_nm: 0.0,
            last_update: Instant::now(),
        })
    }

    /// Apply safety limits to raw torque
    ///
    /// **Validates: Requirements FFB-SAFETY-01.1-4**
    pub fn apply(&mut self, raw_torque_nm: f32, safe_for_ffb: bool) -> FfbPipelineResult<f32> {
        self.last_raw_torque_nm = raw_torque_nm;
        self.last_update = Instant::now();

        let safe_torque = self.envelope.apply(raw_torque_nm, safe_for_ffb)?;
        self.last_safe_torque_nm = safe_torque;

        Ok(safe_torque)
    }

    /// Trigger fault ramp-down
    ///
    /// **Validates: Requirement FFB-SAFETY-01.6**
    ///
    /// Captures `fault_initial_torque` at fault detection time (not `last_setpoint`)
    pub fn trigger_fault_ramp(&mut self) {
        self.envelope.trigger_fault_ramp();
    }

    /// Clear fault state
    pub fn clear_fault(&mut self) {
        self.envelope.clear_fault();
    }

    /// Check if in fault ramp
    pub fn is_in_fault_ramp(&self) -> bool {
        self.envelope.is_in_fault_ramp()
    }

    /// Get fault ramp progress (0.0 to 1.0)
    pub fn get_fault_ramp_progress(&self) -> Option<f32> {
        self.envelope.get_fault_ramp_progress()
    }

    /// Get last raw torque input
    pub fn get_last_raw_torque(&self) -> f32 {
        self.last_raw_torque_nm
    }

    /// Get last safe torque output
    pub fn get_last_safe_torque(&self) -> f32 {
        self.last_safe_torque_nm
    }

    /// Get underlying envelope for configuration
    pub fn get_envelope(&self) -> &SafetyEnvelope {
        &self.envelope
    }

    /// Get mutable underlying envelope for configuration
    pub fn get_envelope_mut(&mut self) -> &mut SafetyEnvelope {
        &mut self.envelope
    }

    /// Reset axis state
    pub fn reset(&mut self) {
        self.envelope.reset();
        self.last_raw_torque_nm = 0.0;
        self.last_safe_torque_nm = 0.0;
        self.last_update = Instant::now();
    }
}

/// Hardware fault category for determining recovery requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultCategory {
    /// Transient fault - can be cleared via explicit user action
    Transient,
    /// Hardware-critical fault - requires power cycle to re-enable high-torque mode
    HardwareCritical,
}

impl FaultCategory {
    /// Categorize a fault type
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    pub fn from_fault_type(fault_type: &FaultType) -> Self {
        match fault_type {
            // Hardware-critical faults require power cycle
            FaultType::OverTemp | FaultType::OverCurrent | FaultType::EncoderInvalid => {
                FaultCategory::HardwareCritical
            }
            // Transient faults can be cleared via user action
            FaultType::UsbStall
            | FaultType::EndpointError
            | FaultType::NanValue
            | FaultType::PluginOverrun
            | FaultType::EndpointWedged
            | FaultType::DeviceTimeout
            | FaultType::DeviceDisconnect
            | FaultType::UserEmergencyStop
            | FaultType::HardwareEmergencyStop => FaultCategory::Transient,
        }
    }

    /// Check if this fault category requires power cycle
    pub fn requires_power_cycle(&self) -> bool {
        matches!(self, FaultCategory::HardwareCritical)
    }
}

/// FFB output backend trait
///
/// Implemented by DirectInput, XInput, and OFP-1 backends
pub trait FfbOutputBackend: Send {
    /// Set torque for a specific axis
    fn set_axis_torque(&mut self, axis_index: u32, torque_nm: f32) -> FfbPipelineResult<()>;

    /// Get maximum torque for this backend
    fn get_max_torque_nm(&self) -> f32;

    /// Check if backend is connected
    fn is_connected(&self) -> bool;

    /// Get backend name for logging
    fn backend_name(&self) -> &str;
}

/// FFB Pipeline configuration
#[derive(Debug, Clone)]
pub struct FfbPipelineConfig {
    /// Maximum torque in Newton-meters (from device capabilities)
    pub max_torque_nm: f32,
    /// Maximum slew rate in Nm/s
    pub max_slew_rate_nm_per_s: f32,
    /// Maximum jerk in Nm/s²
    pub max_jerk_nm_per_s2: f32,
    /// Fault ramp-down time (must be 50ms per requirements)
    pub fault_ramp_time: Duration,
    /// Timestep for rate calculations (typically 4ms for 250Hz loop)
    pub timestep_s: f32,
    /// Number of axes (typically 2: pitch and roll)
    pub num_axes: u32,
}

impl Default for FfbPipelineConfig {
    fn default() -> Self {
        Self {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004, // 250Hz = 4ms
            num_axes: 2,
        }
    }
}

/// FFB Pipeline state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfbPipelineState {
    /// Pipeline is idle (no active output)
    Idle,
    /// Pipeline is active and outputting torque
    Active,
    /// Pipeline is in fault ramp-down
    FaultRamp,
    /// Pipeline is faulted (latched)
    Faulted,
}

/// FFB Pipeline - integrates SafetyEnvelope with output backends
///
/// **Pipeline:** raw_torque → SafetyEnvelope::apply_limits(safe_for_ffb) → DirectInput/XInput/OFP-1
///
/// **Validates: Requirements FFB-SAFETY-01.1-6**
#[derive(Debug)]
pub struct FfbPipeline {
    config: FfbPipelineConfig,
    /// Per-axis safety envelopes
    axis_envelopes: Vec<AxisSafetyEnvelope>,
    /// Current pipeline state
    state: FfbPipelineState,
    /// Latched hardware fault (requires power cycle)
    latched_hardware_fault: Option<FaultType>,
    /// Last safe_for_ffb value
    last_safe_for_ffb: bool,
    /// Timestamp when fault was detected
    fault_detection_time: Option<Instant>,
    /// Torque values at fault detection (per axis)
    fault_initial_torques: Vec<f32>,
    /// Statistics
    stats: FfbPipelineStats,
}

/// FFB Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct FfbPipelineStats {
    /// Total frames processed
    pub frames_processed: u64,
    /// Frames where torque was clamped
    pub frames_clamped: u64,
    /// Frames where slew rate was limited
    pub frames_slew_limited: u64,
    /// Fault ramps triggered
    pub fault_ramps_triggered: u64,
    /// Hardware faults latched
    pub hardware_faults_latched: u64,
    /// NaN/Inf values detected in pipeline
    pub nan_inf_detected: u64,
}

impl FfbPipeline {
    /// Create new FFB pipeline
    pub fn new(config: FfbPipelineConfig) -> FfbPipelineResult<Self> {
        let mut axis_envelopes = Vec::with_capacity(config.num_axes as usize);
        let mut fault_initial_torques = Vec::with_capacity(config.num_axes as usize);

        for axis_index in 0..config.num_axes {
            let envelope_config = SafetyEnvelopeConfig {
                max_torque_nm: config.max_torque_nm,
                max_slew_rate_nm_per_s: config.max_slew_rate_nm_per_s,
                max_jerk_nm_per_s2: config.max_jerk_nm_per_s2,
                fault_ramp_time: config.fault_ramp_time,
                timestep_s: config.timestep_s,
            };

            axis_envelopes.push(AxisSafetyEnvelope::new(axis_index, envelope_config)?);
            fault_initial_torques.push(0.0);
        }

        Ok(Self {
            config,
            axis_envelopes,
            state: FfbPipelineState::Idle,
            latched_hardware_fault: None,
            last_safe_for_ffb: false,
            fault_detection_time: None,
            fault_initial_torques,
            stats: FfbPipelineStats::default(),
        })
    }

    /// Process raw torque through safety envelope and return safe torque
    ///
    /// **Pipeline:** raw_torque → SafetyEnvelope::apply_limits(safe_for_ffb) → output
    ///
    /// **Validates: Requirements FFB-SAFETY-01.1-6**
    ///
    /// # Arguments
    /// * `axis_index` - Axis to process (0 = pitch, 1 = roll)
    /// * `raw_torque_nm` - Raw torque command in Newton-meters
    /// * `safe_for_ffb` - Whether FFB is safe (from telemetry sanity gate)
    ///
    /// # Returns
    /// Safe torque value after applying all safety constraints
    pub fn process_torque(
        &mut self,
        axis_index: u32,
        raw_torque_nm: f32,
        safe_for_ffb: bool,
    ) -> FfbPipelineResult<f32> {
        // **Validates: Requirement FFB-SAFETY-01.6**
        // Check for NaN/Inf in pipeline input - triggers fault if detected
        if !raw_torque_nm.is_finite() {
            self.stats.nan_inf_detected += 1;
            return Err(FfbPipelineError::InvalidTorque {
                value: raw_torque_nm,
            });
        }

        // Check for latched hardware fault
        if let Some(fault_type) = &self.latched_hardware_fault {
            return Err(FfbPipelineError::HardwareFaultLatched {
                fault_type: fault_type.clone(),
            });
        }

        // Validate axis index
        if axis_index >= self.config.num_axes {
            return Err(FfbPipelineError::OutputError {
                message: format!(
                    "Invalid axis index: {} (max: {})",
                    axis_index,
                    self.config.num_axes - 1
                ),
            });
        }

        // Detect transition from safe to unsafe
        if self.last_safe_for_ffb && !safe_for_ffb {
            self.handle_fault_detection();
        }

        self.last_safe_for_ffb = safe_for_ffb;

        // Get axis envelope
        let axis_envelope = &mut self.axis_envelopes[axis_index as usize];

        // Apply safety envelope
        let safe_torque = axis_envelope.apply(raw_torque_nm, safe_for_ffb)?;

        // Update state
        self.update_state();

        // Update statistics
        self.stats.frames_processed += 1;
        if safe_torque.abs() < raw_torque_nm.abs() - 0.01 {
            self.stats.frames_clamped += 1;
        }

        Ok(safe_torque)
    }

    /// Process torque with NaN/Inf detection that triggers fault state
    ///
    /// **Validates: Requirement FFB-SAFETY-01.6**
    /// NaN or Inf in FFB pipeline triggers fault handler and ramp to zero within 50ms
    ///
    /// This method is the preferred entry point when you want NaN/Inf detection
    /// to trigger a full fault response (state transition to Faulted, 50ms ramp-down).
    ///
    /// # Arguments
    /// * `axis_index` - Axis to process (0 = pitch, 1 = roll)
    /// * `raw_torque_nm` - Raw torque command in Newton-meters
    /// * `safe_for_ffb` - Whether FFB is safe (from telemetry sanity gate)
    /// * `context` - Description of where the value came from (for logging)
    ///
    /// # Returns
    /// * `Ok(safe_torque)` - Safe torque value after applying all safety constraints
    /// * `Err(NanInPipeline)` - If NaN/Inf was detected and fault was triggered
    pub fn process_torque_with_nan_detection(
        &mut self,
        axis_index: u32,
        raw_torque_nm: f32,
        safe_for_ffb: bool,
        context: &str,
    ) -> FfbPipelineResult<f32> {
        // **Validates: Requirement FFB-SAFETY-01.6**
        // Check for NaN/Inf in pipeline input - triggers fault if detected
        if !raw_torque_nm.is_finite() {
            self.stats.nan_inf_detected += 1;

            // Trigger fault detection and 50ms ramp-down
            self.handle_nan_in_pipeline(raw_torque_nm, context);

            return Err(FfbPipelineError::NanInPipeline {
                value: raw_torque_nm,
                context: context.to_string(),
            });
        }

        // Delegate to standard processing
        self.process_torque(axis_index, raw_torque_nm, safe_for_ffb)
    }

    /// Handle NaN/Inf detection in pipeline
    ///
    /// **Validates: Requirement FFB-SAFETY-01.6**
    /// Triggers fault ramp-down and transitions to FaultRamp state
    fn handle_nan_in_pipeline(&mut self, value: f32, context: &str) {
        tracing::error!(
            "NaN/Inf detected in FFB pipeline at {}: {} - triggering 50ms ramp-to-zero",
            context,
            value
        );

        // Trigger fault detection (captures fault_initial_torque and starts ramp)
        self.handle_fault_detection();

        // Update state to reflect NaN fault
        self.state = FfbPipelineState::FaultRamp;
    }

    /// Check a value for NaN/Inf without processing torque
    ///
    /// **Validates: Requirement FFB-SAFETY-01.6**
    /// Useful for checking intermediate pipeline values before they reach process_torque
    ///
    /// # Arguments
    /// * `value` - The value to check
    /// * `context` - Description of where the value came from (for logging)
    ///
    /// # Returns
    /// * `Ok(())` - If value is finite
    /// * `Err(NanInPipeline)` - If NaN/Inf was detected and fault was triggered
    pub fn check_value_for_nan(&mut self, value: f32, context: &str) -> FfbPipelineResult<()> {
        if !value.is_finite() {
            self.stats.nan_inf_detected += 1;
            self.handle_nan_in_pipeline(value, context);

            return Err(FfbPipelineError::NanInPipeline {
                value,
                context: context.to_string(),
            });
        }
        Ok(())
    }

    /// Process torque for all axes at once
    ///
    /// # Arguments
    /// * `raw_torques_nm` - Raw torque commands per axis
    /// * `safe_for_ffb` - Whether FFB is safe
    ///
    /// # Returns
    /// Safe torque values per axis
    pub fn process_all_axes(
        &mut self,
        raw_torques_nm: &[f32],
        safe_for_ffb: bool,
    ) -> FfbPipelineResult<Vec<f32>> {
        if raw_torques_nm.len() != self.config.num_axes as usize {
            return Err(FfbPipelineError::OutputError {
                message: format!(
                    "Expected {} torque values, got {}",
                    self.config.num_axes,
                    raw_torques_nm.len()
                ),
            });
        }

        let mut safe_torques = Vec::with_capacity(raw_torques_nm.len());

        for (axis_index, &raw_torque) in raw_torques_nm.iter().enumerate() {
            let safe_torque = self.process_torque(axis_index as u32, raw_torque, safe_for_ffb)?;
            safe_torques.push(safe_torque);
        }

        Ok(safe_torques)
    }

    /// Handle fault detection - captures fault_initial_torque
    ///
    /// **Validates: Requirement FFB-SAFETY-01.6**
    ///
    /// Captures `fault_initial_torque` at fault detection (not `last_setpoint`)
    fn handle_fault_detection(&mut self) {
        let now = Instant::now();

        // Only capture if not already in fault
        if self.fault_detection_time.is_none() {
            self.fault_detection_time = Some(now);

            // Capture fault_initial_torque for each axis
            for (i, axis_envelope) in self.axis_envelopes.iter().enumerate() {
                self.fault_initial_torques[i] = axis_envelope.get_last_safe_torque();
            }

            // Trigger fault ramp on all axes
            for axis_envelope in &mut self.axis_envelopes {
                axis_envelope.trigger_fault_ramp();
            }

            self.state = FfbPipelineState::FaultRamp;
            self.stats.fault_ramps_triggered += 1;

            tracing::warn!(
                "FFB fault detected - initiating 50ms ramp-down from torques: {:?}",
                self.fault_initial_torques
            );
        }
    }

    /// Handle hardware fault (over-temp, over-current)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    ///
    /// Hardware-critical faults latch and require power cycle
    pub fn handle_hardware_fault(&mut self, fault_type: FaultType) -> FfbPipelineResult<()> {
        let category = FaultCategory::from_fault_type(&fault_type);

        // Trigger fault ramp first
        self.handle_fault_detection();

        if category.requires_power_cycle() {
            self.latched_hardware_fault = Some(fault_type.clone());
            self.state = FfbPipelineState::Faulted;
            self.stats.hardware_faults_latched += 1;

            tracing::error!(
                "Hardware-critical fault latched: {:?} - requires power cycle to re-enable high-torque mode",
                fault_type
            );
        }

        Ok(())
    }

    /// Clear transient fault (user-initiated)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.10**
    ///
    /// Transient faults can be cleared via explicit user action after cause is resolved
    pub fn clear_transient_fault(&mut self) -> FfbPipelineResult<()> {
        // Cannot clear hardware-critical faults
        if let Some(fault_type) = &self.latched_hardware_fault {
            return Err(FfbPipelineError::HardwareFaultLatched {
                fault_type: fault_type.clone(),
            });
        }

        // Clear fault state on all axes
        for axis_envelope in &mut self.axis_envelopes {
            axis_envelope.clear_fault();
        }

        self.fault_detection_time = None;
        self.state = FfbPipelineState::Idle;

        tracing::info!("Transient fault cleared by user action");

        Ok(())
    }

    /// Reset from hardware fault (requires power cycle confirmation)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    pub fn reset_from_hardware_fault(&mut self, power_cycled: bool) -> FfbPipelineResult<()> {
        if !power_cycled {
            if let Some(fault_type) = &self.latched_hardware_fault {
                return Err(FfbPipelineError::HardwareFaultLatched {
                    fault_type: fault_type.clone(),
                });
            }
        }

        // Clear all state
        self.latched_hardware_fault = None;
        self.fault_detection_time = None;

        for axis_envelope in &mut self.axis_envelopes {
            axis_envelope.reset();
        }

        for torque in &mut self.fault_initial_torques {
            *torque = 0.0;
        }

        self.state = FfbPipelineState::Idle;
        self.last_safe_for_ffb = false;

        tracing::info!("FFB pipeline reset after power cycle");

        Ok(())
    }

    /// Update pipeline state based on axis states
    fn update_state(&mut self) {
        // Check if any axis is in fault ramp
        let any_in_fault_ramp = self.axis_envelopes.iter().any(|e| e.is_in_fault_ramp());

        if self.latched_hardware_fault.is_some() {
            self.state = FfbPipelineState::Faulted;
        } else if any_in_fault_ramp {
            self.state = FfbPipelineState::FaultRamp;
        } else if self.last_safe_for_ffb {
            self.state = FfbPipelineState::Active;
        } else {
            // Check if fault ramp completed
            if self.fault_detection_time.is_some() {
                let elapsed = self.fault_detection_time.unwrap().elapsed();
                if elapsed >= self.config.fault_ramp_time {
                    // Ramp complete, stay in idle until cleared
                    self.state = FfbPipelineState::Idle;
                }
            } else {
                self.state = FfbPipelineState::Idle;
            }
        }
    }

    /// Get current pipeline state
    pub fn get_state(&self) -> FfbPipelineState {
        self.state
    }

    /// Get latched hardware fault
    pub fn get_latched_fault(&self) -> Option<&FaultType> {
        self.latched_hardware_fault.as_ref()
    }

    /// Check if pipeline has latched hardware fault
    pub fn has_latched_fault(&self) -> bool {
        self.latched_hardware_fault.is_some()
    }

    /// Get fault initial torques (captured at fault detection)
    pub fn get_fault_initial_torques(&self) -> &[f32] {
        &self.fault_initial_torques
    }

    /// Get time since fault detection
    pub fn get_fault_elapsed_time(&self) -> Option<Duration> {
        self.fault_detection_time.map(|t| t.elapsed())
    }

    /// Get fault ramp progress (0.0 to 1.0) for an axis
    pub fn get_fault_ramp_progress(&self, axis_index: u32) -> Option<f32> {
        if axis_index < self.config.num_axes {
            self.axis_envelopes[axis_index as usize].get_fault_ramp_progress()
        } else {
            None
        }
    }

    /// Get axis envelope for inspection
    pub fn get_axis_envelope(&self, axis_index: u32) -> Option<&AxisSafetyEnvelope> {
        if axis_index < self.config.num_axes {
            Some(&self.axis_envelopes[axis_index as usize])
        } else {
            None
        }
    }

    /// Get mutable axis envelope for configuration
    pub fn get_axis_envelope_mut(&mut self, axis_index: u32) -> Option<&mut AxisSafetyEnvelope> {
        if axis_index < self.config.num_axes {
            Some(&mut self.axis_envelopes[axis_index as usize])
        } else {
            None
        }
    }

    /// Get pipeline statistics
    pub fn get_stats(&self) -> &FfbPipelineStats {
        &self.stats
    }

    /// Get configuration
    pub fn get_config(&self) -> &FfbPipelineConfig {
        &self.config
    }

    /// Update configuration (preserves state)
    pub fn update_config(&mut self, config: FfbPipelineConfig) -> FfbPipelineResult<()> {
        // Update each axis envelope
        for axis_envelope in &mut self.axis_envelopes {
            let envelope_config = SafetyEnvelopeConfig {
                max_torque_nm: config.max_torque_nm,
                max_slew_rate_nm_per_s: config.max_slew_rate_nm_per_s,
                max_jerk_nm_per_s2: config.max_jerk_nm_per_s2,
                fault_ramp_time: config.fault_ramp_time,
                timestep_s: config.timestep_s,
            };
            axis_envelope
                .get_envelope_mut()
                .update_config(envelope_config)?;
        }

        self.config = config;
        Ok(())
    }

    /// Reset pipeline state
    pub fn reset(&mut self) {
        for axis_envelope in &mut self.axis_envelopes {
            axis_envelope.reset();
        }

        for torque in &mut self.fault_initial_torques {
            *torque = 0.0;
        }

        self.state = FfbPipelineState::Idle;
        self.latched_hardware_fault = None;
        self.last_safe_for_ffb = false;
        self.fault_detection_time = None;
    }
}

impl Default for FfbPipeline {
    fn default() -> Self {
        Self::new(FfbPipelineConfig::default()).expect("Default config should be valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// **Test: Pipeline Torque Clamping**
    /// **Validates: Requirement FFB-SAFETY-01.1**
    #[test]
    fn test_pipeline_torque_clamping() {
        let config = FfbPipelineConfig {
            max_torque_nm: 10.0,
            max_slew_rate_nm_per_s: 1000.0, // High to not interfere
            max_jerk_nm_per_s2: 10000.0,
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        // Request torque exceeding max
        let safe_torque = pipeline.process_torque(0, 15.0, true).unwrap();

        // Should be clamped (may not reach max immediately due to slew limiting)
        assert!(
            safe_torque <= 10.0,
            "Torque should be clamped to max: {}",
            safe_torque
        );

        // Process multiple frames to reach steady state
        for _ in 0..100 {
            let safe_torque = pipeline.process_torque(0, 15.0, true).unwrap();
            assert!(
                safe_torque <= 10.0,
                "Torque should remain clamped: {}",
                safe_torque
            );
        }

        // Final torque should be at max
        let final_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(
            (final_torque - 10.0).abs() < 0.5,
            "Should reach max torque: {}",
            final_torque
        );
    }

    /// **Test: Pipeline Slew Rate Limiting**
    /// **Validates: Requirement FFB-SAFETY-01.2**
    #[test]
    fn test_pipeline_slew_rate_limiting() {
        let config = FfbPipelineConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 10.0, // 10 Nm/s limit
            max_jerk_nm_per_s2: 10000.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        let mut last_torque = 0.0;
        let timestep = 0.004;
        let max_slew_rate = 10.0;

        for _ in 0..50 {
            let safe_torque = pipeline.process_torque(0, 15.0, true).unwrap();
            let delta = safe_torque - last_torque;
            let slew_rate = delta / timestep;

            // Check slew rate limit (with tolerance)
            assert!(
                slew_rate.abs() <= max_slew_rate + 0.1,
                "Slew rate exceeded: {} > {}",
                slew_rate.abs(),
                max_slew_rate
            );

            last_torque = safe_torque;
        }
    }

    /// **Test: Pipeline Jerk Limiting**
    /// **Validates: Requirement FFB-SAFETY-01.3**
    #[test]
    fn test_pipeline_jerk_limiting() {
        let config = FfbPipelineConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 100.0, // 100 Nm/s² jerk limit
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        let mut last_torque = 0.0;
        let mut last_slew_rate = 0.0;
        let timestep = 0.004;
        let max_jerk = 100.0;

        for i in 0..50 {
            let safe_torque = pipeline.process_torque(0, 15.0, true).unwrap();
            let delta = safe_torque - last_torque;
            let slew_rate = delta / timestep;
            let jerk = (slew_rate - last_slew_rate) / timestep;

            // Check jerk limit (skip first iteration, with tolerance)
            if i > 0 {
                assert!(
                    jerk.abs() <= max_jerk + 1.0,
                    "Jerk exceeded at iteration {}: {} > {}",
                    i,
                    jerk.abs(),
                    max_jerk
                );
            }

            last_torque = safe_torque;
            last_slew_rate = slew_rate;
        }
    }

    /// **Test: Pipeline safe_for_ffb Enforcement**
    /// **Validates: Requirement FFB-SAFETY-01.4**
    #[test]
    fn test_pipeline_safe_for_ffb_enforcement() {
        let config = FfbPipelineConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        // Build up torque
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        let torque_before = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(torque_before > 0.0, "Should have built up torque");

        // Set safe_for_ffb to false and wait for 50ms ramp to complete
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(60) {
            pipeline.process_torque(0, 10.0, false).unwrap();
            thread::sleep(Duration::from_millis(4));
        }

        // Should ramp to zero
        let final_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(
            final_torque.abs() < 0.5,
            "Should ramp to zero when safe_for_ffb=false: {}",
            final_torque
        );
    }

    /// **Test: Pipeline 50ms Fault Ramp**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_pipeline_50ms_fault_ramp() {
        let config = FfbPipelineConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        // Build up torque
        for _ in 0..100 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        let initial_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(
            initial_torque > 8.0,
            "Should have built up significant torque: {}",
            initial_torque
        );

        // Trigger fault by setting safe_for_ffb to false
        pipeline.process_torque(0, 10.0, false).unwrap();

        // Verify fault was detected
        assert_eq!(pipeline.get_state(), FfbPipelineState::FaultRamp);

        // Verify fault_initial_torque was captured
        let fault_initial = pipeline.get_fault_initial_torques()[0];
        assert!(
            (fault_initial - initial_torque).abs() < 0.1,
            "fault_initial_torque should match torque at fault detection: {} vs {}",
            fault_initial,
            initial_torque
        );

        // Simulate 50ms of updates
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(60) {
            pipeline.process_torque(0, 10.0, false).unwrap();
            thread::sleep(Duration::from_millis(4));
        }

        // Should be at zero after 50ms
        let final_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(
            final_torque.abs() < 0.5,
            "Should reach zero within 50ms: {}",
            final_torque
        );
    }

    /// **Test: Pipeline Hardware Fault Latching**
    /// **Validates: Requirement FFB-SAFETY-01.9**
    #[test]
    fn test_pipeline_hardware_fault_latching() {
        let mut pipeline = FfbPipeline::default();

        // Build up torque
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Trigger hardware fault (over-temp)
        pipeline.handle_hardware_fault(FaultType::OverTemp).unwrap();

        // Should be in faulted state
        assert_eq!(pipeline.get_state(), FfbPipelineState::Faulted);
        assert!(pipeline.has_latched_fault());
        assert_eq!(pipeline.get_latched_fault(), Some(&FaultType::OverTemp));

        // Should not be able to process torque
        let result = pipeline.process_torque(0, 10.0, true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::HardwareFaultLatched { .. })
        ));

        // Should not be able to clear without power cycle
        let result = pipeline.clear_transient_fault();
        assert!(matches!(
            result,
            Err(FfbPipelineError::HardwareFaultLatched { .. })
        ));

        // Reset with power cycle
        pipeline.reset_from_hardware_fault(true).unwrap();

        // Should be able to process torque again
        assert!(!pipeline.has_latched_fault());
        let result = pipeline.process_torque(0, 5.0, true);
        assert!(result.is_ok());
    }

    /// **Test: Pipeline Transient Fault Clearing**
    /// **Validates: Requirement FFB-SAFETY-01.10**
    #[test]
    fn test_pipeline_transient_fault_clearing() {
        let mut pipeline = FfbPipeline::default();

        // Build up torque
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Trigger transient fault (safe_for_ffb = false)
        pipeline.process_torque(0, 10.0, false).unwrap();

        // Wait for ramp to complete
        thread::sleep(Duration::from_millis(60));
        for _ in 0..20 {
            pipeline.process_torque(0, 10.0, false).unwrap();
        }

        // Clear transient fault
        pipeline.clear_transient_fault().unwrap();

        // Should be able to process torque again
        let result = pipeline.process_torque(0, 5.0, true);
        assert!(result.is_ok());
        assert_eq!(pipeline.get_state(), FfbPipelineState::Active);
    }

    /// **Test: Pipeline Multi-Axis Processing**
    #[test]
    fn test_pipeline_multi_axis() {
        let config = FfbPipelineConfig {
            num_axes: 2,
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        // Process both axes
        let safe_torques = pipeline.process_all_axes(&[5.0, 3.0], true).unwrap();

        assert_eq!(safe_torques.len(), 2);

        // Both axes should have processed torque
        let pitch_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        let roll_torque = pipeline
            .get_axis_envelope(1)
            .unwrap()
            .get_last_safe_torque();

        assert!(pitch_torque > 0.0, "Pitch axis should have torque");
        assert!(roll_torque > 0.0, "Roll axis should have torque");
    }

    /// **Test: Pipeline Invalid Torque Rejection**
    #[test]
    fn test_pipeline_invalid_torque_rejection() {
        let mut pipeline = FfbPipeline::default();

        // Test NaN
        let result = pipeline.process_torque(0, f32::NAN, true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::InvalidTorque { .. })
        ));

        // Test infinity
        let result = pipeline.process_torque(0, f32::INFINITY, true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::InvalidTorque { .. })
        ));
    }

    /// **Test: Pipeline Statistics**
    #[test]
    fn test_pipeline_statistics() {
        let mut pipeline = FfbPipeline::default();

        // Process some frames
        for _ in 0..100 {
            pipeline.process_torque(0, 5.0, true).unwrap();
        }

        let stats = pipeline.get_stats();
        assert_eq!(stats.frames_processed, 100);
    }

    /// **Test: Fault Category Classification**
    /// **Validates: Requirement FFB-SAFETY-01.9**
    #[test]
    fn test_fault_category_classification() {
        // Hardware-critical faults
        assert_eq!(
            FaultCategory::from_fault_type(&FaultType::OverTemp),
            FaultCategory::HardwareCritical
        );
        assert_eq!(
            FaultCategory::from_fault_type(&FaultType::OverCurrent),
            FaultCategory::HardwareCritical
        );

        // Transient faults
        assert_eq!(
            FaultCategory::from_fault_type(&FaultType::UsbStall),
            FaultCategory::Transient
        );
        assert_eq!(
            FaultCategory::from_fault_type(&FaultType::NanValue),
            FaultCategory::Transient
        );
        assert_eq!(
            FaultCategory::from_fault_type(&FaultType::DeviceTimeout),
            FaultCategory::Transient
        );

        // Check power cycle requirement
        assert!(FaultCategory::HardwareCritical.requires_power_cycle());
        assert!(!FaultCategory::Transient.requires_power_cycle());
    }

    /// **Test: Pipeline Configuration Update**
    #[test]
    fn test_pipeline_config_update() {
        let mut pipeline = FfbPipeline::default();

        // Build up some state
        for _ in 0..50 {
            pipeline.process_torque(0, 5.0, true).unwrap();
        }

        let torque_before = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();

        // Update configuration
        let new_config = FfbPipelineConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 100.0,
            ..Default::default()
        };

        pipeline.update_config(new_config).unwrap();

        // State should be preserved
        let torque_after = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert_eq!(
            torque_before, torque_after,
            "State should be preserved after config update"
        );

        // New limits should be applied
        assert_eq!(pipeline.get_config().max_torque_nm, 20.0);
    }

    /// **Test: Pipeline Reset**
    #[test]
    fn test_pipeline_reset() {
        let mut pipeline = FfbPipeline::default();

        // Build up state
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Trigger fault
        pipeline.process_torque(0, 10.0, false).unwrap();

        assert!(
            pipeline
                .get_axis_envelope(0)
                .unwrap()
                .get_last_safe_torque()
                != 0.0
        );

        // Reset
        pipeline.reset();

        assert_eq!(pipeline.get_state(), FfbPipelineState::Idle);
        assert!(!pipeline.has_latched_fault());
        assert_eq!(
            pipeline
                .get_axis_envelope(0)
                .unwrap()
                .get_last_safe_torque(),
            0.0
        );
    }

    // =========================================================================
    // NaN/Inf Detection Tests
    // **Validates: Requirement FFB-SAFETY-01.6**
    // =========================================================================

    /// **Test: NaN Detection in Pipeline Input**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_nan_detection_in_pipeline_input() {
        let mut pipeline = FfbPipeline::default();

        // Build up some torque first
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Test NaN detection with process_torque
        let result = pipeline.process_torque(0, f32::NAN, true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::InvalidTorque { .. })
        ));

        // Verify NaN was counted in statistics
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);
    }

    /// **Test: Positive Infinity Detection in Pipeline Input**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_positive_infinity_detection_in_pipeline_input() {
        let mut pipeline = FfbPipeline::default();

        // Build up some torque first
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Test positive infinity detection
        let result = pipeline.process_torque(0, f32::INFINITY, true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::InvalidTorque { .. })
        ));

        // Verify infinity was counted in statistics
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);
    }

    /// **Test: Negative Infinity Detection in Pipeline Input**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_negative_infinity_detection_in_pipeline_input() {
        let mut pipeline = FfbPipeline::default();

        // Build up some torque first
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Test negative infinity detection
        let result = pipeline.process_torque(0, f32::NEG_INFINITY, true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::InvalidTorque { .. })
        ));

        // Verify infinity was counted in statistics
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);
    }

    /// **Test: NaN Detection with Fault Triggering**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_nan_detection_with_fault_triggering() {
        let mut pipeline = FfbPipeline::default();

        // Build up some torque first
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        let initial_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(initial_torque > 0.0, "Should have built up torque");

        // Test NaN detection with fault triggering
        let result =
            pipeline.process_torque_with_nan_detection(0, f32::NAN, true, "test_axis_input");
        assert!(matches!(
            result,
            Err(FfbPipelineError::NanInPipeline { .. })
        ));

        // Verify fault state was triggered
        assert_eq!(pipeline.get_state(), FfbPipelineState::FaultRamp);

        // Verify NaN was counted in statistics
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);

        // Verify fault ramp was triggered
        assert_eq!(pipeline.get_stats().fault_ramps_triggered, 1);
    }

    /// **Test: Infinity Detection with Fault Triggering**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_infinity_detection_with_fault_triggering() {
        let mut pipeline = FfbPipeline::default();

        // Build up some torque first
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Test infinity detection with fault triggering
        let result =
            pipeline.process_torque_with_nan_detection(0, f32::INFINITY, true, "test_axis_input");
        assert!(matches!(
            result,
            Err(FfbPipelineError::NanInPipeline { .. })
        ));

        // Verify fault state was triggered
        assert_eq!(pipeline.get_state(), FfbPipelineState::FaultRamp);

        // Verify infinity was counted in statistics
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);
    }

    /// **Test: NaN Detection Triggers 50ms Ramp-to-Zero**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_nan_detection_triggers_50ms_ramp_to_zero() {
        let config = FfbPipelineConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        // Build up significant torque
        for _ in 0..100 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        let initial_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(
            initial_torque > 8.0,
            "Should have built up significant torque: {}",
            initial_torque
        );

        // Trigger NaN fault
        let _ = pipeline.process_torque_with_nan_detection(0, f32::NAN, true, "test_nan_ramp");

        // Verify fault_initial_torque was captured
        let fault_initial = pipeline.get_fault_initial_torques()[0];
        assert!(
            (fault_initial - initial_torque).abs() < 0.1,
            "fault_initial_torque should match torque at fault detection: {} vs {}",
            fault_initial,
            initial_torque
        );

        // Simulate 50ms of updates (using safe_for_ffb=false to continue ramp)
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(60) {
            // Use process_torque with safe_for_ffb=false to continue the ramp
            let _ = pipeline.process_torque(0, 0.0, false);
            thread::sleep(Duration::from_millis(4));
        }

        // Should be at zero after 50ms
        let final_torque = pipeline
            .get_axis_envelope(0)
            .unwrap()
            .get_last_safe_torque();
        assert!(
            final_torque.abs() < 0.5,
            "Should reach zero within 50ms after NaN detection: {}",
            final_torque
        );
    }

    /// **Test: check_value_for_nan Method**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_check_value_for_nan_method() {
        let mut pipeline = FfbPipeline::default();

        // Build up some torque first
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Test valid value
        let result = pipeline.check_value_for_nan(5.0, "test_valid");
        assert!(result.is_ok());
        assert_eq!(pipeline.get_stats().nan_inf_detected, 0);

        // Test NaN value
        let result = pipeline.check_value_for_nan(f32::NAN, "test_nan");
        assert!(matches!(
            result,
            Err(FfbPipelineError::NanInPipeline { .. })
        ));
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);
        assert_eq!(pipeline.get_state(), FfbPipelineState::FaultRamp);
    }

    /// **Test: check_value_for_nan with Infinity**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_check_value_for_nan_with_infinity() {
        let mut pipeline = FfbPipeline::default();

        // Build up some torque first
        for _ in 0..50 {
            pipeline.process_torque(0, 10.0, true).unwrap();
        }

        // Test positive infinity
        let result = pipeline.check_value_for_nan(f32::INFINITY, "test_pos_inf");
        assert!(matches!(
            result,
            Err(FfbPipelineError::NanInPipeline { .. })
        ));
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);

        // Reset pipeline
        pipeline.reset();

        // Test negative infinity
        let result = pipeline.check_value_for_nan(f32::NEG_INFINITY, "test_neg_inf");
        assert!(matches!(
            result,
            Err(FfbPipelineError::NanInPipeline { .. })
        ));
        assert_eq!(pipeline.get_stats().nan_inf_detected, 2);
    }

    /// **Test: Multiple NaN/Inf Detections Counted**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_multiple_nan_inf_detections_counted() {
        let mut pipeline = FfbPipeline::default();

        // Trigger multiple NaN/Inf detections
        let _ = pipeline.process_torque(0, f32::NAN, true);
        let _ = pipeline.process_torque(0, f32::INFINITY, true);
        let _ = pipeline.process_torque(0, f32::NEG_INFINITY, true);
        let _ = pipeline.process_torque(0, f32::NAN, true);

        // Verify all were counted
        assert_eq!(pipeline.get_stats().nan_inf_detected, 4);
    }

    /// **Test: NaN Detection in Multi-Axis Processing**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_nan_detection_in_multi_axis_processing() {
        let config = FfbPipelineConfig {
            num_axes: 2,
            ..Default::default()
        };

        let mut pipeline = FfbPipeline::new(config).unwrap();

        // Test NaN in first axis
        let result = pipeline.process_all_axes(&[f32::NAN, 5.0], true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::InvalidTorque { .. })
        ));
        assert_eq!(pipeline.get_stats().nan_inf_detected, 1);

        // Reset and test NaN in second axis
        pipeline.reset();
        let result = pipeline.process_all_axes(&[5.0, f32::INFINITY], true);
        assert!(matches!(
            result,
            Err(FfbPipelineError::InvalidTorque { .. })
        ));
        assert_eq!(pipeline.get_stats().nan_inf_detected, 2);
    }

    /// **Test: NaN Detection Error Contains Context**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_nan_detection_error_contains_context() {
        let mut pipeline = FfbPipeline::default();

        let result =
            pipeline.process_torque_with_nan_detection(0, f32::NAN, true, "pitch_axis_input");

        match result {
            Err(FfbPipelineError::NanInPipeline { value, context }) => {
                assert!(value.is_nan());
                assert_eq!(context, "pitch_axis_input");
            }
            _ => panic!("Expected NanInPipeline error"),
        }
    }

    /// **Test: Valid Values Pass Through NaN Detection**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_valid_values_pass_through_nan_detection() {
        let mut pipeline = FfbPipeline::default();

        // Test various valid values
        let valid_values = [
            0.0,
            1.0,
            -1.0,
            10.0,
            -10.0,
            0.001,
            -0.001,
            f32::MIN_POSITIVE,
            f32::MAX / 2.0,
        ];

        for &value in &valid_values {
            let result = pipeline.process_torque_with_nan_detection(0, value, true, "test_valid");
            assert!(result.is_ok(), "Valid value {} should pass through", value);
        }

        // No NaN/Inf should have been detected
        assert_eq!(pipeline.get_stats().nan_inf_detected, 0);
    }

    /// **Test: NaN Detection After Fault Recovery**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    #[test]
    fn test_nan_detection_after_fault_recovery() {
        let mut pipeline = FfbPipeline::default();

        // Trigger NaN fault
        let _ = pipeline.process_torque_with_nan_detection(0, f32::NAN, true, "first_nan");
        assert_eq!(pipeline.get_state(), FfbPipelineState::FaultRamp);

        // Wait for ramp to complete
        thread::sleep(Duration::from_millis(60));
        for _ in 0..20 {
            let _ = pipeline.process_torque(0, 0.0, false);
        }

        // Clear fault
        pipeline.clear_transient_fault().unwrap();
        assert_eq!(pipeline.get_state(), FfbPipelineState::Idle);

        // Verify NaN detection still works after recovery
        let _ = pipeline.process_torque_with_nan_detection(0, f32::NAN, true, "second_nan");
        assert_eq!(pipeline.get_state(), FfbPipelineState::FaultRamp);
        assert_eq!(pipeline.get_stats().nan_inf_detected, 2);
    }
}
