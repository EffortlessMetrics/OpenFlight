// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Trim correctness for force feedback devices
//!
//! Implements rate and jerk limited setpoint changes to prevent torque steps.
//! Handles both FFB devices (true force feedback) and non-FFB devices (spring-centered).

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::TimeSource;

/// Trim operation mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrimMode {
    /// Non-FFB device with spring centering
    SpringCentered,
    /// True FFB device with force feedback
    ForceFeedback,
}

/// Trim limits for safe setpoint changes
#[derive(Debug, Clone)]
pub struct TrimLimits {
    /// Maximum rate of change in Nm/s
    pub max_rate_nm_per_s: f32,
    /// Maximum jerk (rate of rate change) in Nm/s²
    pub max_jerk_nm_per_s2: f32,
}

impl Default for TrimLimits {
    fn default() -> Self {
        Self {
            max_rate_nm_per_s: 5.0,   // 5 Nm/s max rate
            max_jerk_nm_per_s2: 20.0, // 20 Nm/s² max jerk
        }
    }
}

impl TrimLimits {
    /// Validate that limits are reasonable and within safety bounds.
    pub fn validate_trim_limits(&self) -> Result<(), String> {
        if self.max_rate_nm_per_s <= 0.0 {
            return Err("max_rate_nm_per_s must be positive".to_string());
        }

        if self.max_jerk_nm_per_s2 <= 0.0 {
            return Err("max_jerk_nm_per_s2 must be positive".to_string());
        }

        // Safety cap: 50 Nm/s is the physical ceiling for consumer FFB hardware.
        if self.max_rate_nm_per_s > 50.0 {
            return Err(format!(
                "max_rate_nm_per_s {:.1} exceeds safety limit of 50.0 Nm/s",
                self.max_rate_nm_per_s
            ));
        }

        if self.max_jerk_nm_per_s2 > 200.0 {
            return Err(format!(
                "max_jerk_nm_per_s2 {:.1} exceeds safety limit of 200.0 Nm/s\u{00b2}",
                self.max_jerk_nm_per_s2
            ));
        }

        // Jerk must be at least 2× rate for smooth, controllable response.
        if self.max_jerk_nm_per_s2 < self.max_rate_nm_per_s * 2.0 {
            return Err(
                "max_jerk_nm_per_s2 must be at least 2\u{00d7} max_rate_nm_per_s for proper response"
                    .to_string(),
            );
        }

        Ok(())
    }
}

/// Setpoint change request
#[derive(Debug, Clone)]
pub struct SetpointChange {
    /// Target setpoint in Nm
    pub target_nm: f32,
    /// Limits to apply during change
    pub limits: TrimLimits,
}

/// Spring configuration for non-FFB devices
#[derive(Debug, Clone)]
pub struct SpringConfig {
    /// Spring strength (0.0 to 1.0)
    pub strength: f32,
    /// Center position (-1.0 to 1.0)
    pub center: f32,
    /// Deadband around center
    pub deadband: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            strength: 0.8,
            center: 0.0,
            deadband: 0.05,
        }
    }
}

/// Trim controller state
#[derive(Debug)]
pub struct TrimController {
    /// Current trim mode
    mode: TrimMode,
    /// Maximum device torque
    max_torque_nm: f32,
    /// Current setpoint in Nm
    current_setpoint_nm: f32,
    /// Target setpoint in Nm
    target_setpoint_nm: f32,
    /// Current rate in Nm/s
    current_rate_nm_per_s: f32,
    /// Trim limits
    limits: TrimLimits,
    /// Spring configuration (for non-FFB mode)
    spring_config: SpringConfig,
    /// Whether spring is currently frozen
    spring_frozen: bool,
    /// Spring ramp start time (for gradual re-enable)
    spring_ramp_start: Option<Instant>,
    /// Spring ramp duration
    spring_ramp_duration: Duration,
    /// When the spring freeze started (for hold-then-ramp timing)
    spring_freeze_start: Option<Instant>,
    /// Last update timestamp
    last_update: Instant,
    /// Time source for deterministic operation
    time_source: Arc<dyn TimeSource>,
    /// Active setpoint change
    active_change: Option<SetpointChange>,
}

impl TrimController {
    /// Create new trim controller
    pub fn new(max_torque_nm: f32) -> Self {
        Self::with_time_source(max_torque_nm, Arc::new(crate::DefaultTimeSource))
    }

    /// Create new trim controller with time source
    pub fn with_time_source(max_torque_nm: f32, time_source: Arc<dyn TimeSource>) -> Self {
        Self {
            mode: TrimMode::ForceFeedback,
            max_torque_nm,
            current_setpoint_nm: 0.0,
            target_setpoint_nm: 0.0,
            current_rate_nm_per_s: 0.0,
            limits: TrimLimits::default(),
            spring_config: SpringConfig::default(),
            spring_frozen: false,
            spring_ramp_start: None,
            spring_ramp_duration: Duration::from_millis(150),
            spring_freeze_start: None,
            last_update: time_source.now(),
            time_source,
            active_change: None,
        }
    }

    /// Set trim mode
    pub fn set_mode(&mut self, mode: TrimMode) {
        self.mode = mode;

        // Reset state when changing modes
        self.current_setpoint_nm = 0.0;
        self.target_setpoint_nm = 0.0;
        self.current_rate_nm_per_s = 0.0;
        self.spring_frozen = false;
        self.spring_ramp_start = None;
        self.spring_freeze_start = None;
        self.active_change = None;
    }

    /// Get current trim mode
    pub fn mode(&self) -> TrimMode {
        self.mode
    }

    /// Set trim limits
    pub fn set_limits(&mut self, limits: TrimLimits) {
        self.limits = limits;
    }

    /// Get current trim limits
    pub fn limits(&self) -> &TrimLimits {
        &self.limits
    }

    /// Apply setpoint change with rate/jerk limiting
    pub fn apply_setpoint_change(&mut self, change: SetpointChange) -> Result<(), String> {
        // Validate target is within device limits
        if change.target_nm.abs() > self.max_torque_nm {
            return Err(format!(
                "Target setpoint {} Nm exceeds device limit {} Nm",
                change.target_nm, self.max_torque_nm
            ));
        }

        match self.mode {
            TrimMode::ForceFeedback => self.apply_ffb_setpoint_change(change),
            TrimMode::SpringCentered => self.apply_spring_setpoint_change(change),
        }
    }

    /// Apply setpoint change for FFB devices
    fn apply_ffb_setpoint_change(&mut self, change: SetpointChange) -> Result<(), String> {
        self.target_setpoint_nm = change.target_nm;
        self.limits = change.limits.clone();
        self.active_change = Some(change);
        Ok(())
    }

    /// Apply setpoint change for spring-centered devices
    fn apply_spring_setpoint_change(&mut self, change: SetpointChange) -> Result<(), String> {
        // For spring devices, we freeze the spring, change center, then re-enable
        self.freeze_spring();

        // Convert torque setpoint to spring center position
        let new_center = (change.target_nm / self.max_torque_nm).clamp(-1.0, 1.0);
        self.spring_config.center = new_center;

        self.target_setpoint_nm = change.target_nm;
        self.active_change = Some(change);

        Ok(())
    }

    /// Update trim controller (call at regular intervals)
    pub fn update(&mut self) -> TrimOutput {
        let now = self.time_source.now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        match self.mode {
            TrimMode::ForceFeedback => self.update_ffb(dt),
            TrimMode::SpringCentered => self.update_spring(dt),
        }
    }

    /// Update with an explicit timestep (for deterministic testing).
    pub fn update_with_dt(&mut self, dt_secs: f32) -> TrimOutput {
        match self.mode {
            TrimMode::ForceFeedback => self.update_ffb(dt_secs),
            TrimMode::SpringCentered => self.update_spring(dt_secs),
        }
    }

    /// Update FFB trim with rate/jerk limiting
    fn update_ffb(&mut self, dt: f32) -> TrimOutput {
        if self.active_change.is_none() {
            return TrimOutput::ForceFeedback {
                setpoint_nm: self.current_setpoint_nm,
                rate_nm_per_s: self.current_rate_nm_per_s,
            };
        }

        let error = self.target_setpoint_nm - self.current_setpoint_nm;

        // Check if we've reached the target
        if error.abs() < 0.001 {
            self.current_setpoint_nm = self.target_setpoint_nm;
            self.current_rate_nm_per_s = 0.0;
            self.active_change = None;
            return TrimOutput::ForceFeedback {
                setpoint_nm: self.current_setpoint_nm,
                rate_nm_per_s: 0.0,
            };
        }

        // Calculate desired rate using look-ahead deceleration.
        //
        // A jerk-limited controller must begin braking well before the target.
        // The stopping distance at current rate `v` with max jerk `j` is v²/(2j).
        // Clamp the desired rate to the maximum that still allows stopping in time:
        //   v_max_decel = sqrt(2 * j * |error|)
        //
        // This prevents the classic overshoot that occurs when the controller
        // only responds to error/dt (which only kicks in at the last step).
        let braking_limit = (2.0 * self.limits.max_jerk_nm_per_s2 * error.abs()).sqrt();
        let desired_rate = if error > 0.0 {
            self.limits.max_rate_nm_per_s.min(braking_limit)
        } else {
            (-self.limits.max_rate_nm_per_s).max(-braking_limit)
        };

        // Apply jerk limiting
        let rate_error = desired_rate - self.current_rate_nm_per_s;
        let max_rate_change = self.limits.max_jerk_nm_per_s2 * dt;
        let rate_change = rate_error.clamp(-max_rate_change, max_rate_change);

        self.current_rate_nm_per_s += rate_change;
        // Clamp setpoint to [min(current, target), max(current, target)] to
        // prevent floating-point creep past the target in the final step.
        self.current_setpoint_nm = (self.current_setpoint_nm + self.current_rate_nm_per_s * dt)
            .clamp(
                self.current_setpoint_nm.min(self.target_setpoint_nm),
                self.current_setpoint_nm.max(self.target_setpoint_nm),
            );

        TrimOutput::ForceFeedback {
            setpoint_nm: self.current_setpoint_nm,
            rate_nm_per_s: self.current_rate_nm_per_s,
        }
    }

    /// Update spring trim with freeze/ramp logic
    fn update_spring(&mut self, _dt: f32) -> TrimOutput {
        // Handle spring ramp if active
        if let Some(ramp_start) = self.spring_ramp_start {
            let ramp_elapsed = self.time_source.now().duration_since(ramp_start);

            if ramp_elapsed >= self.spring_ramp_duration {
                // Ramp complete - unfreeze spring
                self.spring_frozen = false;
                self.spring_ramp_start = None;
                self.active_change = None;
            } else {
                // Ramp in progress - gradually increase spring strength
                let ramp_progress =
                    ramp_elapsed.as_secs_f32() / self.spring_ramp_duration.as_secs_f32();
                let target_strength = self.spring_config.strength;

                // Create ramped config with gradually increasing strength
                let mut ramped_config = self.spring_config.clone();
                ramped_config.strength = target_strength * ramp_progress;

                return TrimOutput::SpringCentered {
                    config: ramped_config,
                    frozen: false, // Not frozen during ramp, just reduced strength
                };
            }
        } else if self.spring_frozen {
            // Check if we should start ramping spring back
            if let Some(_change) = &self.active_change {
                // Start ramping after a brief hold period (measured from when freeze began)
                let freeze_elapsed = self
                    .spring_freeze_start
                    .map(|t| self.time_source.now().duration_since(t))
                    .unwrap_or(Duration::from_millis(200));
                if freeze_elapsed > Duration::from_millis(100) {
                    self.spring_ramp_start = Some(self.time_source.now());
                    self.spring_freeze_start = None;
                }
            }
        }

        TrimOutput::SpringCentered {
            config: self.spring_config.clone(),
            frozen: self.spring_frozen,
        }
    }

    /// Freeze spring for trim hold (non-FFB devices)
    pub fn freeze_spring(&mut self) {
        if self.mode == TrimMode::SpringCentered {
            self.spring_frozen = true;
            self.spring_freeze_start = Some(self.time_source.now());
        }
    }

    /// Ramp spring re-enable over specified duration
    pub fn ramp_spring_enable(&mut self, ramp_duration: Duration) {
        if self.mode == TrimMode::SpringCentered && self.spring_frozen {
            // Start gradual ramp by setting up a ramp state
            self.spring_ramp_start = Some(self.time_source.now());
            self.spring_ramp_duration = ramp_duration;
            // Don't unfreeze immediately - let update() handle the ramp
        }
    }

    /// Set spring configuration
    pub fn set_spring_config(&mut self, config: SpringConfig) {
        self.spring_config = config;
    }

    /// Get current spring configuration
    pub fn spring_config(&self) -> &SpringConfig {
        &self.spring_config
    }

    /// Check if trim change is in progress
    pub fn is_changing(&self) -> bool {
        self.active_change.is_some()
    }

    /// Get current setpoint
    pub fn current_setpoint_nm(&self) -> f32 {
        self.current_setpoint_nm
    }

    /// Get target setpoint
    pub fn target_setpoint_nm(&self) -> f32 {
        self.target_setpoint_nm
    }

    /// Get current rate
    pub fn current_rate_nm_per_s(&self) -> f32 {
        self.current_rate_nm_per_s
    }

    /// Estimate time to complete current change
    pub fn estimated_completion_time(&self) -> Option<Duration> {
        if let Some(_change) = &self.active_change {
            let remaining = (self.target_setpoint_nm - self.current_setpoint_nm).abs();
            // Use current rate if moving, otherwise use max_rate as upper-bound estimate
            let effective_rate = if self.current_rate_nm_per_s.abs() > 0.001 {
                self.current_rate_nm_per_s.abs()
            } else {
                self.limits.max_rate_nm_per_s
            };
            if effective_rate > 0.001 {
                let time_s = remaining / effective_rate;
                Some(Duration::from_secs_f32(time_s))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if spring is currently ramping
    pub fn is_spring_ramping(&self) -> bool {
        self.spring_ramp_start.is_some()
    }

    /// Get spring ramp progress (0.0 to 1.0)
    pub fn get_spring_ramp_progress(&self) -> Option<f32> {
        if let Some(ramp_start) = self.spring_ramp_start {
            let elapsed = self.time_source.now().duration_since(ramp_start).as_secs_f32();
            let total = self.spring_ramp_duration.as_secs_f32();
            Some((elapsed / total).min(1.0))
        } else {
            None
        }
    }

    /// Validate that no torque steps occur during setpoint changes
    pub fn validate_no_torque_steps(
        &self,
        previous_output: f32,
        current_output: f32,
        dt: f32,
    ) -> Result<(), String> {
        if dt <= 0.0 {
            return Ok(()); // Skip validation for invalid dt
        }

        let torque_change = (current_output - previous_output).abs();
        let rate = torque_change / dt;

        // Check against rate limit with some tolerance for discrete sampling
        let tolerance_factor = 1.1; // 10% tolerance
        if rate > self.limits.max_rate_nm_per_s * tolerance_factor {
            return Err(format!(
                "Torque step detected: rate {} Nm/s exceeds limit {} Nm/s",
                rate, self.limits.max_rate_nm_per_s
            ));
        }

        Ok(())
    }

    /// Get detailed trim state for diagnostics
    pub fn get_trim_state(&self) -> TrimState {
        TrimState {
            mode: self.mode,
            current_setpoint_nm: self.current_setpoint_nm,
            target_setpoint_nm: self.target_setpoint_nm,
            current_rate_nm_per_s: self.current_rate_nm_per_s,
            limits: self.limits.clone(),
            spring_config: self.spring_config.clone(),
            spring_frozen: self.spring_frozen,
            spring_ramping: self.is_spring_ramping(),
            spring_ramp_progress: self.get_spring_ramp_progress(),
            is_changing: self.is_changing(),
            estimated_completion: self.estimated_completion_time(),
        }
    }
}

/// Output from trim controller
#[derive(Debug, Clone)]
pub enum TrimOutput {
    /// Force feedback device output
    ForceFeedback {
        /// Current setpoint in Nm
        setpoint_nm: f32,
        /// Current rate in Nm/s
        rate_nm_per_s: f32,
    },
    /// Spring-centered device output
    SpringCentered {
        /// Spring configuration
        config: SpringConfig,
        /// Whether spring is frozen
        frozen: bool,
    },
}

/// Detailed trim state for diagnostics and validation
#[derive(Debug, Clone)]
pub struct TrimState {
    /// Current trim mode
    pub mode: TrimMode,
    /// Current setpoint in Nm
    pub current_setpoint_nm: f32,
    /// Target setpoint in Nm
    pub target_setpoint_nm: f32,
    /// Current rate in Nm/s
    pub current_rate_nm_per_s: f32,
    /// Trim limits
    pub limits: TrimLimits,
    /// Spring configuration
    pub spring_config: SpringConfig,
    /// Whether spring is frozen
    pub spring_frozen: bool,
    /// Whether spring is ramping
    pub spring_ramping: bool,
    /// Spring ramp progress (0.0 to 1.0)
    pub spring_ramp_progress: Option<f32>,
    /// Whether trim change is in progress
    pub is_changing: bool,
    /// Estimated completion time
    pub estimated_completion: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_limits_default() {
        let limits = TrimLimits::default();
        assert_eq!(limits.max_rate_nm_per_s, 5.0);
        assert_eq!(limits.max_jerk_nm_per_s2, 20.0);
    }

    #[test]
    fn test_spring_config_default() {
        let config = SpringConfig::default();
        assert_eq!(config.strength, 0.8);
        assert_eq!(config.center, 0.0);
        assert_eq!(config.deadband, 0.05);
    }

    #[test]
    fn test_trim_controller_creation() {
        let controller = TrimController::new(15.0);
        assert_eq!(controller.mode(), TrimMode::ForceFeedback);
        assert_eq!(controller.current_setpoint_nm(), 0.0);
        assert_eq!(controller.target_setpoint_nm(), 0.0);
        assert!(!controller.is_changing());
    }

    #[test]
    fn test_mode_switching() {
        let mut controller = TrimController::new(15.0);

        controller.set_mode(TrimMode::SpringCentered);
        assert_eq!(controller.mode(), TrimMode::SpringCentered);

        controller.set_mode(TrimMode::ForceFeedback);
        assert_eq!(controller.mode(), TrimMode::ForceFeedback);
    }

    #[test]
    fn test_setpoint_validation() {
        let mut controller = TrimController::new(10.0);

        let valid_change = SetpointChange {
            target_nm: 5.0,
            limits: TrimLimits::default(),
        };
        assert!(controller.apply_setpoint_change(valid_change).is_ok());

        let invalid_change = SetpointChange {
            target_nm: 15.0, // Exceeds 10.0 limit
            limits: TrimLimits::default(),
        };
        assert!(controller.apply_setpoint_change(invalid_change).is_err());
    }

    #[test]
    fn test_ffb_setpoint_change() {
        let mut controller = TrimController::new(15.0);
        controller.set_mode(TrimMode::ForceFeedback);

        let change = SetpointChange {
            target_nm: 5.0,
            limits: TrimLimits::default(),
        };

        assert!(controller.apply_setpoint_change(change).is_ok());
        assert_eq!(controller.target_setpoint_nm(), 5.0);
        assert!(controller.is_changing());
    }

    #[test]
    fn test_spring_setpoint_change() {
        let mut controller = TrimController::new(15.0);
        controller.set_mode(TrimMode::SpringCentered);

        let change = SetpointChange {
            target_nm: 7.5, // Should map to 0.5 center position
            limits: TrimLimits::default(),
        };

        assert!(controller.apply_setpoint_change(change).is_ok());
        assert_eq!(controller.target_setpoint_nm(), 7.5);
        assert_eq!(controller.spring_config().center, 0.5);
        assert!(controller.is_changing());
    }

    #[test]
    fn test_spring_freeze() {
        let mut controller = TrimController::new(15.0);
        controller.set_mode(TrimMode::SpringCentered);

        controller.freeze_spring();

        let output = controller.update();
        if let TrimOutput::SpringCentered { frozen, .. } = output {
            assert!(frozen);
        } else {
            panic!("Expected SpringCentered output");
        }
    }

    #[test]
    fn test_ffb_update_convergence() {
        let mut controller = TrimController::new(15.0);
        controller.set_mode(TrimMode::ForceFeedback);

        let change = SetpointChange {
            target_nm: 1.0,
            limits: TrimLimits {
                max_rate_nm_per_s: 10.0,
                max_jerk_nm_per_s2: 100.0,
            },
        };

        controller.apply_setpoint_change(change).unwrap();

        // Simulate updates with a fixed 1 ms timestep (deterministic, no real sleep).
        // With max_rate=10 Nm/s and max_jerk=100 Nm/s², the controller needs
        // ~100 steps to reach max rate and ~100 more to cover 1 Nm → 200 steps total.
        for _ in 0..500 {
            let output = controller.update_with_dt(0.001);
            if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                if (setpoint_nm - 1.0).abs() < 0.001 {
                    break;
                }
            }
        }

        // Check final state - should be close to target
        assert!((controller.current_setpoint_nm() - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_rate_limiting() {
        let mut controller = TrimController::new(15.0);
        controller.set_mode(TrimMode::ForceFeedback);

        let change = SetpointChange {
            target_nm: 10.0,
            limits: TrimLimits {
                max_rate_nm_per_s: 2.0, // Slow rate
                max_jerk_nm_per_s2: 5.0,
            },
        };

        controller.apply_setpoint_change(change).unwrap();

        // First update should respect rate limit
        let output = controller.update();
        if let TrimOutput::ForceFeedback { rate_nm_per_s, .. } = output {
            assert!(rate_nm_per_s.abs() <= 2.0);
        }
    }

    #[test]
    fn test_completion_time_estimation() {
        let mut controller = TrimController::new(15.0);
        controller.set_mode(TrimMode::ForceFeedback);

        let change = SetpointChange {
            target_nm: 5.0,
            limits: TrimLimits::default(),
        };

        controller.apply_setpoint_change(change).unwrap();

        // Verify that we can call the estimation function
        let _estimated = controller.estimated_completion_time();

        // Verify that the controller is in changing state
        assert!(controller.is_changing());

        // Verify target is set correctly
        assert_eq!(controller.target_setpoint_nm(), 5.0);
    }
}
