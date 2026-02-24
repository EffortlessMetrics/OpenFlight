// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Soft-stop ramp implementation for safe torque shutdown
//!
//! Provides controlled torque ramp to zero within 50ms constraint with
//! audio/LED cues and comprehensive timing validation.

use std::time::{Duration, Instant};
use thiserror::Error;

/// Soft-stop ramp profile types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RampProfile {
    /// Linear ramp from current to zero
    Linear,
    /// Exponential decay (faster initial drop)
    Exponential,
    /// S-curve for smooth start/end
    SCurve,
}

/// Soft-stop configuration
#[derive(Debug, Clone)]
pub struct SoftStopConfig {
    /// Maximum time to ramp torque to zero
    pub max_ramp_time: Duration,
    /// Ramp profile to use
    pub profile: RampProfile,
    /// Enable audio cue on soft-stop
    pub audio_cue: bool,
    /// Enable LED indication on soft-stop
    pub led_indication: bool,
    /// Minimum torque threshold (below this is considered zero)
    pub zero_threshold_nm: f32,
}

impl Default for SoftStopConfig {
    fn default() -> Self {
        Self {
            max_ramp_time: Duration::from_millis(50),
            profile: RampProfile::Linear,
            audio_cue: true,
            led_indication: true,
            zero_threshold_nm: 0.01, // 10mNm threshold
        }
    }
}

/// Soft-stop ramp state
#[derive(Debug, Clone)]
pub struct SoftStopState {
    /// Whether ramp is currently active
    pub active: bool,
    /// Initial torque when ramp started
    pub initial_torque_nm: f32,
    /// Current target torque
    pub current_torque_nm: f32,
    /// When the ramp started
    pub start_time: Instant,
    /// Configuration for this ramp
    pub config: SoftStopConfig,
    /// Whether audio cue was triggered
    pub audio_cue_triggered: bool,
    /// Whether LED indication was triggered
    pub led_indication_triggered: bool,
}

/// Soft-stop ramp controller
#[derive(Debug)]
pub struct SoftStopController {
    state: Option<SoftStopState>,
    config: SoftStopConfig,
}

/// Soft-stop errors
#[derive(Debug, Error)]
pub enum SoftStopError {
    #[error("Ramp already active")]
    RampAlreadyActive,
    #[error("No active ramp to update")]
    NoActiveRamp,
    #[error("Ramp timeout exceeded: {elapsed:?} > {max:?}")]
    RampTimeout { elapsed: Duration, max: Duration },
    #[error("Invalid torque value: {value}")]
    InvalidTorque { value: f32 },
}

pub type SoftStopResult<T> = std::result::Result<T, SoftStopError>;

impl SoftStopController {
    /// Create new soft-stop controller
    pub fn new(config: SoftStopConfig) -> Self {
        Self {
            state: None,
            config,
        }
    }

    /// Start soft-stop ramp from current torque
    pub fn start_ramp(&mut self, current_torque_nm: f32) -> SoftStopResult<()> {
        if self.state.is_some() {
            return Err(SoftStopError::RampAlreadyActive);
        }

        if !current_torque_nm.is_finite() {
            return Err(SoftStopError::InvalidTorque {
                value: current_torque_nm,
            });
        }

        let now = Instant::now();
        self.state = Some(SoftStopState {
            active: true,
            initial_torque_nm: current_torque_nm,
            current_torque_nm,
            start_time: now,
            config: self.config.clone(),
            audio_cue_triggered: false,
            led_indication_triggered: false,
        });

        Ok(())
    }

    /// Update ramp and get current target torque
    pub fn update(&mut self) -> SoftStopResult<Option<f32>> {
        let state = match &mut self.state {
            Some(state) if state.active => state,
            Some(_) => return Ok(None), // Ramp completed
            None => return Ok(None),    // No active ramp
        };

        let now = Instant::now();
        let elapsed = now.duration_since(state.start_time);

        // Calculate progress (0.0 to 1.0)
        let progress = (elapsed.as_secs_f32()
            / state.config.max_ramp_time.as_secs_f32())
        .clamp(0.0, 1.0);

        // Apply ramp profile
        let ramp_factor = match state.config.profile {
            RampProfile::Linear => 1.0 - progress,
            RampProfile::Exponential => (-3.0 * progress).exp(), // e^(-3t) for fast initial drop
            RampProfile::SCurve => {
                // S-curve using smoothstep function
                let smooth_progress = progress * progress * (3.0 - 2.0 * progress);
                1.0 - smooth_progress
            }
        };

        // Calculate current torque
        state.current_torque_nm = state.initial_torque_nm * ramp_factor;

        // Elapsed-past-deadline check: if we're beyond max_ramp_time and the ramp
        // would have reached zero (ramp_factor≈0 at progress=1.0), complete
        // gracefully.  Only error if torque is still dangerously above threshold.
        if elapsed > state.config.max_ramp_time {
            if state.current_torque_nm.abs() <= state.config.zero_threshold_nm {
                state.current_torque_nm = 0.0;
                state.active = false;
                return Ok(None);
            }
            state.active = false;
            state.current_torque_nm = 0.0;
            return Err(SoftStopError::RampTimeout {
                elapsed,
                max: state.config.max_ramp_time,
            });
        }

        // Check if we've reached zero threshold
        if state.current_torque_nm.abs() <= state.config.zero_threshold_nm {
            state.current_torque_nm = 0.0;
            state.active = false;
        }

        Ok(Some(state.current_torque_nm))
    }

    /// Check if ramp is active
    pub fn is_active(&self) -> bool {
        self.state.as_ref().map_or(false, |s| s.active)
    }

    /// Get current ramp state
    pub fn get_state(&self) -> Option<&SoftStopState> {
        self.state.as_ref()
    }

    /// Get ramp progress (0.0 to 1.0)
    pub fn get_progress(&self) -> Option<f32> {
        self.state.as_ref().and_then(|state| {
            if state.active {
                let elapsed = state.start_time.elapsed();
                let progress = elapsed.as_secs_f32() / state.config.max_ramp_time.as_secs_f32();
                Some(progress.clamp(0.0, 1.0))
            } else {
                None
            }
        })
    }

    /// Get elapsed time since ramp start
    pub fn get_elapsed_time(&self) -> Option<Duration> {
        self.state.as_ref().map(|state| state.start_time.elapsed())
    }

    /// Mark audio cue as triggered
    pub fn mark_audio_cue_triggered(&mut self) {
        if let Some(state) = &mut self.state {
            state.audio_cue_triggered = true;
        }
    }

    /// Mark LED indication as triggered
    pub fn mark_led_indication_triggered(&mut self) {
        if let Some(state) = &mut self.state {
            state.led_indication_triggered = true;
        }
    }

    /// Check if audio cue should be triggered
    pub fn should_trigger_audio_cue(&self) -> bool {
        self.state.as_ref().map_or(false, |state| {
            state.active && state.config.audio_cue && !state.audio_cue_triggered
        })
    }

    /// Check if LED indication should be triggered
    pub fn should_trigger_led_indication(&self) -> bool {
        self.state.as_ref().map_or(false, |state| {
            state.active && state.config.led_indication && !state.led_indication_triggered
        })
    }

    /// Reset controller (clear any active ramp)
    pub fn reset(&mut self) {
        self.state = None;
    }

    /// Update configuration
    pub fn update_config(&mut self, config: SoftStopConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &SoftStopConfig {
        &self.config
    }
}

impl Default for SoftStopController {
    fn default() -> Self {
        Self::new(SoftStopConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_linear_ramp() {
        let config = SoftStopConfig {
            max_ramp_time: Duration::from_millis(100),
            profile: RampProfile::Linear,
            ..Default::default()
        };

        let mut controller = SoftStopController::new(config);

        // Start ramp from 10 Nm
        controller.start_ramp(10.0).unwrap();
        assert!(controller.is_active());

        // Should start at full torque
        let torque = controller.update().unwrap().unwrap();
        assert!((torque - 10.0).abs() < 0.1);

        // Wait and check progress
        thread::sleep(Duration::from_millis(50));
        let torque = controller.update().unwrap().unwrap();
        assert!(torque < 10.0 && torque > 0.0);

        // Check progress calculation
        let progress = controller.get_progress().unwrap();
        assert!(progress > 0.4 && progress < 0.6); // Should be around 50%
    }

    #[test]
    fn test_exponential_ramp() {
        let config = SoftStopConfig {
            max_ramp_time: Duration::from_millis(100),
            profile: RampProfile::Exponential,
            ..Default::default()
        };

        let mut controller = SoftStopController::new(config);
        controller.start_ramp(10.0).unwrap();

        // Exponential should drop faster initially
        thread::sleep(Duration::from_millis(25));
        let torque = controller.update().unwrap().unwrap();

        // Should have dropped significantly (more than linear)
        assert!(torque < 5.0);
    }

    #[test]
    fn test_ramp_completion() {
        let config = SoftStopConfig {
            max_ramp_time: Duration::from_millis(100), // Increased for test stability
            zero_threshold_nm: 0.1,
            ..Default::default()
        };

        let mut controller = SoftStopController::new(config);
        controller.start_ramp(1.0).unwrap();

        // Wait for completion
        thread::sleep(Duration::from_millis(120));
        let torque = controller.update().unwrap();

        // Should be complete (None) or zero
        assert!(torque.is_none() || torque.unwrap() == 0.0);
        assert!(!controller.is_active());
    }

    #[test]
    fn test_ramp_timeout() {
        // Exponential ramp: at progress=1.0, ramp_factor = exp(-3) ≈ 0.0498.
        // With initial=10.0 Nm and default threshold=0.01 Nm:
        // torque at deadline ≈ 10.0 * 0.0498 = 0.498 Nm >> threshold → RampTimeout.
        let config = SoftStopConfig {
            max_ramp_time: Duration::from_millis(10),
            profile: RampProfile::Exponential,
            ..Default::default()
        };

        let mut controller = SoftStopController::new(config);
        controller.start_ramp(10.0).unwrap();

        // Wait longer than timeout
        thread::sleep(Duration::from_millis(20));

        let result = controller.update();
        assert!(matches!(result, Err(SoftStopError::RampTimeout { .. })));
        assert!(!controller.is_active());
    }

    #[test]
    fn test_cue_triggering() {
        let config = SoftStopConfig {
            audio_cue: true,
            led_indication: true,
            ..Default::default()
        };

        let mut controller = SoftStopController::new(config);
        controller.start_ramp(10.0).unwrap();

        // Should need to trigger cues
        assert!(controller.should_trigger_audio_cue());
        assert!(controller.should_trigger_led_indication());

        // Mark as triggered
        controller.mark_audio_cue_triggered();
        controller.mark_led_indication_triggered();

        // Should no longer need triggering
        assert!(!controller.should_trigger_audio_cue());
        assert!(!controller.should_trigger_led_indication());
    }

    #[test]
    fn test_invalid_torque() {
        let mut controller = SoftStopController::default();

        // Test NaN
        let result = controller.start_ramp(f32::NAN);
        assert!(matches!(result, Err(SoftStopError::InvalidTorque { .. })));

        // Test infinity
        let result = controller.start_ramp(f32::INFINITY);
        assert!(matches!(result, Err(SoftStopError::InvalidTorque { .. })));
    }

    #[test]
    fn test_double_start() {
        let mut controller = SoftStopController::default();

        controller.start_ramp(10.0).unwrap();

        // Second start should fail
        let result = controller.start_ramp(5.0);
        assert!(matches!(result, Err(SoftStopError::RampAlreadyActive)));
    }

    #[test]
    fn test_reset() {
        let mut controller = SoftStopController::default();

        controller.start_ramp(10.0).unwrap();
        assert!(controller.is_active());

        controller.reset();
        assert!(!controller.is_active());
        assert!(controller.get_state().is_none());
    }

    #[test]
    fn test_s_curve_profile() {
        let config = SoftStopConfig {
            max_ramp_time: Duration::from_millis(100),
            profile: RampProfile::SCurve,
            ..Default::default()
        };

        let mut controller = SoftStopController::new(config);
        controller.start_ramp(10.0).unwrap();

        // S-curve should start slow, accelerate, then slow down
        let initial_torque = controller.update().unwrap().unwrap();

        thread::sleep(Duration::from_millis(10));
        let early_torque = controller.update().unwrap().unwrap();

        thread::sleep(Duration::from_millis(30));
        let mid_torque = controller.update().unwrap().unwrap();

        // Should show S-curve characteristics
        assert!(initial_torque > early_torque);
        assert!(early_torque > mid_torque);

        // The rate of change should be different at different points
        let early_rate = (initial_torque - early_torque) / 0.01; // per 10ms
        let mid_rate = (early_torque - mid_torque) / 0.03; // per 30ms

        // Mid section should have higher rate (steeper part of S-curve)
        assert!(mid_rate > early_rate * 0.5); // Allow some tolerance
    }
}
