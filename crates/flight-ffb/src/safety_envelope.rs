// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safety envelope for force feedback torque output
//!
//! Implements comprehensive safety checks including:
//! - Torque magnitude clamping to device limits
//! - Slew rate limiting (ΔNm/Δt ≤ configured limit)
//! - Jerk limiting (Δ²Nm/Δt² ≤ configured limit)
//! - safe_for_ffb flag enforcement (zero torque when false)
//! - 50ms ramp-to-zero on fault with explicit fault timestamp tracking
//!
//! **Validates: Requirements FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4, FFB-SAFETY-01.6**

use std::time::{Duration, Instant};
use thiserror::Error;

/// Safety envelope configuration
#[derive(Debug, Clone)]
pub struct SafetyEnvelopeConfig {
    /// Maximum torque magnitude in Newton-meters
    pub max_torque_nm: f32,
    /// Maximum slew rate in Nm/s (rate of change of torque)
    pub max_slew_rate_nm_per_s: f32,
    /// Maximum jerk in Nm/s² (rate of change of slew rate)
    pub max_jerk_nm_per_s2: f32,
    /// Fault ramp-down time (must be 50ms per requirements)
    pub fault_ramp_time: Duration,
    /// Timestep for rate calculations (typically 4ms for 250Hz loop)
    pub timestep_s: f32,
}

impl Default for SafetyEnvelopeConfig {
    fn default() -> Self {
        Self {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004, // 250Hz = 4ms
        }
    }
}

/// Safety envelope state
#[derive(Debug, Clone)]
struct SafetyEnvelopeState {
    /// Last output torque
    last_torque_nm: f32,
    /// Last slew rate (for jerk calculation)
    last_slew_rate_nm_per_s: f32,
    /// Timestamp of last update
    last_update: Instant,
    /// Fault timestamp (if in fault ramp-down)
    fault_timestamp: Option<Instant>,
    /// Initial torque when fault occurred
    fault_initial_torque_nm: f32,
}

/// Safety envelope for torque output
#[derive(Debug)]
pub struct SafetyEnvelope {
    config: SafetyEnvelopeConfig,
    state: SafetyEnvelopeState,
}

/// Safety envelope errors
#[derive(Debug, Error)]
pub enum SafetyEnvelopeError {
    #[error("Invalid torque value: {value} (must be finite)")]
    InvalidTorque { value: f32 },
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },
}

pub type SafetyEnvelopeResult<T> = std::result::Result<T, SafetyEnvelopeError>;

impl SafetyEnvelope {
    /// Create new safety envelope with configuration
    pub fn new(config: SafetyEnvelopeConfig) -> SafetyEnvelopeResult<Self> {
        // Validate configuration
        if !config.max_torque_nm.is_finite() || config.max_torque_nm <= 0.0 {
            return Err(SafetyEnvelopeError::InvalidConfig {
                message: format!("Invalid max_torque_nm: {}", config.max_torque_nm),
            });
        }
        if !config.max_slew_rate_nm_per_s.is_finite() || config.max_slew_rate_nm_per_s <= 0.0 {
            return Err(SafetyEnvelopeError::InvalidConfig {
                message: format!(
                    "Invalid max_slew_rate_nm_per_s: {}",
                    config.max_slew_rate_nm_per_s
                ),
            });
        }
        if !config.max_jerk_nm_per_s2.is_finite() || config.max_jerk_nm_per_s2 <= 0.0 {
            return Err(SafetyEnvelopeError::InvalidConfig {
                message: format!("Invalid max_jerk_nm_per_s2: {}", config.max_jerk_nm_per_s2),
            });
        }

        let now = Instant::now();
        Ok(Self {
            config,
            state: SafetyEnvelopeState {
                last_torque_nm: 0.0,
                last_slew_rate_nm_per_s: 0.0,
                last_update: now,
                fault_timestamp: None,
                fault_initial_torque_nm: 0.0,
            },
        })
    }

    /// Apply safety envelope to desired torque
    ///
    /// **Validates: Requirements FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4**
    ///
    /// # Arguments
    /// * `desired_torque_nm` - Desired torque output
    /// * `safe_for_ffb` - Whether FFB is safe to apply (from telemetry sanity gate)
    ///
    /// # Returns
    /// Safe torque output after applying all safety constraints
    pub fn apply(
        &mut self,
        desired_torque_nm: f32,
        safe_for_ffb: bool,
    ) -> SafetyEnvelopeResult<f32> {
        // Validate input
        if !desired_torque_nm.is_finite() {
            return Err(SafetyEnvelopeError::InvalidTorque {
                value: desired_torque_nm,
            });
        }

        let now = Instant::now();
        let dt = self.config.timestep_s;

        // **Requirement FFB-SAFETY-01.4**: Enforce safe_for_ffb flag
        // When safe_for_ffb is false, output zero torque regardless of desired value
        let target_torque = if safe_for_ffb { desired_torque_nm } else { 0.0 };

        // **Requirement FFB-SAFETY-01.6**: Handle fault ramp-down
        // If in fault state, override with direct ramp-to-zero (bypasses rate limiting)
        let (final_torque, final_slew_rate) = if let Some(fault_time) = self.state.fault_timestamp {
            let elapsed = now.duration_since(fault_time);
            let torque = if elapsed >= self.config.fault_ramp_time {
                // Ramp complete, stay at zero
                0.0
            } else {
                // Linear ramp from initial torque to zero over fault_ramp_time
                let progress = elapsed.as_secs_f32() / self.config.fault_ramp_time.as_secs_f32();
                let ramp_factor = 1.0 - progress;
                self.state.fault_initial_torque_nm * ramp_factor
            };
            (torque, 0.0) // Reset slew rate during fault
        } else {
            // Normal operation: apply all safety constraints

            // **Requirement FFB-SAFETY-01.1**: Clamp to device maximum
            let clamped_torque =
                target_torque.clamp(-self.config.max_torque_nm, self.config.max_torque_nm);

            // Calculate desired change
            let desired_delta = clamped_torque - self.state.last_torque_nm;
            let _desired_slew_rate = desired_delta / dt;

            // **Requirement FFB-SAFETY-01.3**: Apply slew rate limiting
            let max_delta = self.config.max_slew_rate_nm_per_s * dt;
            let limited_delta = desired_delta.clamp(-max_delta, max_delta);
            let limited_slew_rate = limited_delta / dt;

            // **Requirement FFB-SAFETY-01.3**: Apply jerk limiting
            let desired_jerk = (limited_slew_rate - self.state.last_slew_rate_nm_per_s) / dt;
            let max_jerk = self.config.max_jerk_nm_per_s2;
            let limited_jerk = desired_jerk.clamp(-max_jerk, max_jerk);
            let final_slew_rate = self.state.last_slew_rate_nm_per_s + (limited_jerk * dt);

            // Calculate final torque
            let final_delta = final_slew_rate * dt;
            let final_torque = self.state.last_torque_nm + final_delta;

            // Final clamp to ensure we never exceed limits due to numerical errors
            let clamped_final =
                final_torque.clamp(-self.config.max_torque_nm, self.config.max_torque_nm);

            // Recalculate actual slew rate based on clamped output
            let actual_delta = clamped_final - self.state.last_torque_nm;
            let actual_slew_rate = actual_delta / dt;

            (clamped_final, actual_slew_rate)
        };

        // Update state
        self.state.last_torque_nm = final_torque;
        self.state.last_slew_rate_nm_per_s = final_slew_rate;
        self.state.last_update = now;

        Ok(final_torque)
    }

    /// Trigger fault ramp-down
    ///
    /// **Validates: Requirement FFB-SAFETY-01.6**
    ///
    /// Initiates a 50ms ramp to zero with explicit fault timestamp tracking.
    /// Once triggered, all subsequent calls to `apply()` will return ramped-down values.
    pub fn trigger_fault_ramp(&mut self) {
        if self.state.fault_timestamp.is_none() {
            let now = Instant::now();
            self.state.fault_timestamp = Some(now);
            self.state.fault_initial_torque_nm = self.state.last_torque_nm;
        }
    }

    /// Clear fault state (requires explicit reset)
    pub fn clear_fault(&mut self) {
        self.state.fault_timestamp = None;
        self.state.fault_initial_torque_nm = 0.0;
    }

    /// Check if currently in fault ramp-down
    pub fn is_in_fault_ramp(&self) -> bool {
        self.state.fault_timestamp.is_some()
    }

    /// Get fault ramp progress (0.0 to 1.0), or None if not in fault
    pub fn get_fault_ramp_progress(&self) -> Option<f32> {
        self.state.fault_timestamp.map(|fault_time| {
            let elapsed = fault_time.elapsed();
            let progress = elapsed.as_secs_f32() / self.config.fault_ramp_time.as_secs_f32();
            progress.clamp(0.0, 1.0)
        })
    }

    /// Get time since fault was triggered
    pub fn get_fault_elapsed_time(&self) -> Option<Duration> {
        self.state.fault_timestamp.map(|t| t.elapsed())
    }

    /// Get last output torque
    pub fn get_last_torque(&self) -> f32 {
        self.state.last_torque_nm
    }

    /// Get last slew rate
    pub fn get_last_slew_rate(&self) -> f32 {
        self.state.last_slew_rate_nm_per_s
    }

    /// Get configuration
    pub fn get_config(&self) -> &SafetyEnvelopeConfig {
        &self.config
    }

    /// Update configuration (preserves state)
    pub fn update_config(&mut self, config: SafetyEnvelopeConfig) -> SafetyEnvelopeResult<()> {
        // Validate new configuration
        if !config.max_torque_nm.is_finite() || config.max_torque_nm <= 0.0 {
            return Err(SafetyEnvelopeError::InvalidConfig {
                message: format!("Invalid max_torque_nm: {}", config.max_torque_nm),
            });
        }
        if !config.max_slew_rate_nm_per_s.is_finite() || config.max_slew_rate_nm_per_s <= 0.0 {
            return Err(SafetyEnvelopeError::InvalidConfig {
                message: format!(
                    "Invalid max_slew_rate_nm_per_s: {}",
                    config.max_slew_rate_nm_per_s
                ),
            });
        }
        if !config.max_jerk_nm_per_s2.is_finite() || config.max_jerk_nm_per_s2 <= 0.0 {
            return Err(SafetyEnvelopeError::InvalidConfig {
                message: format!("Invalid max_jerk_nm_per_s2: {}", config.max_jerk_nm_per_s2),
            });
        }

        self.config = config;
        Ok(())
    }

    /// Reset state (clears all history)
    pub fn reset(&mut self) {
        let now = Instant::now();
        self.state = SafetyEnvelopeState {
            last_torque_nm: 0.0,
            last_slew_rate_nm_per_s: 0.0,
            last_update: now,
            fault_timestamp: None,
            fault_initial_torque_nm: 0.0,
        };
    }
}

impl Default for SafetyEnvelope {
    fn default() -> Self {
        Self::new(SafetyEnvelopeConfig::default()).expect("Default config should be valid")
    }
}

#[cfg(test)]
#[path = "safety_envelope_tests.rs"]
mod tests;
