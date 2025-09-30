//! Trim correctness for force feedback devices
//!
//! Implements rate and jerk limited setpoint changes to prevent torque steps.
//! Handles both FFB devices (true force feedback) and non-FFB devices (spring-centered).

use std::time::{Duration, Instant};

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
    /// Last update timestamp
    last_update: Instant,
    /// Active setpoint change
    active_change: Option<SetpointChange>,
}

impl TrimController {
    /// Create new trim controller
    pub fn new(max_torque_nm: f32) -> Self {
        Self {
            mode: TrimMode::ForceFeedback,
            max_torque_nm,
            current_setpoint_nm: 0.0,
            target_setpoint_nm: 0.0,
            current_rate_nm_per_s: 0.0,
            limits: TrimLimits::default(),
            spring_config: SpringConfig::default(),
            spring_frozen: false,
            last_update: Instant::now(),
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
            TrimMode::ForceFeedback => {
                self.apply_ffb_setpoint_change(change)
            }
            TrimMode::SpringCentered => {
                self.apply_spring_setpoint_change(change)
            }
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
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        match self.mode {
            TrimMode::ForceFeedback => self.update_ffb(dt),
            TrimMode::SpringCentered => self.update_spring(dt),
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
            self.current_rate_nm_per_s = 0.0;
            self.active_change = None;
            return TrimOutput::ForceFeedback {
                setpoint_nm: self.current_setpoint_nm,
                rate_nm_per_s: 0.0,
            };
        }

        // Calculate desired rate to reach target
        let desired_rate = if error > 0.0 {
            self.limits.max_rate_nm_per_s.min(error / dt)
        } else {
            (-self.limits.max_rate_nm_per_s).max(error / dt)
        };

        // Apply jerk limiting
        let rate_error = desired_rate - self.current_rate_nm_per_s;
        let max_rate_change = self.limits.max_jerk_nm_per_s2 * dt;
        let rate_change = rate_error.clamp(-max_rate_change, max_rate_change);
        
        self.current_rate_nm_per_s += rate_change;
        self.current_setpoint_nm += self.current_rate_nm_per_s * dt;

        TrimOutput::ForceFeedback {
            setpoint_nm: self.current_setpoint_nm,
            rate_nm_per_s: self.current_rate_nm_per_s,
        }
    }

    /// Update spring trim with freeze/ramp logic
    fn update_spring(&mut self, _dt: f32) -> TrimOutput {
        if self.spring_frozen {
            // Check if we should start ramping spring back
            if let Some(_change) = &self.active_change {
                // Start ramping after a brief hold period
                if self.last_update.elapsed() > Duration::from_millis(100) {
                    self.ramp_spring_enable(Duration::from_millis(150));
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
        }
    }

    /// Ramp spring re-enable over specified duration
    pub fn ramp_spring_enable(&mut self, _ramp_duration: Duration) {
        if self.mode == TrimMode::SpringCentered && self.spring_frozen {
            // In a real implementation, this would start a gradual ramp
            // For now, we'll just unfreeze after the duration
            self.spring_frozen = false;
            self.active_change = None;
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
            if self.current_rate_nm_per_s.abs() > 0.001 {
                let time_s = remaining / self.current_rate_nm_per_s.abs();
                Some(Duration::from_secs_f32(time_s))
            } else {
                None
            }
        } else {
            None
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
        
        // Simulate updates until convergence
        for _ in 0..1000 {
            let output = controller.update();
            if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                if (setpoint_nm - 1.0).abs() < 0.001 {
                    break;
                }
            }
            // Simulate time passing
            std::thread::sleep(Duration::from_millis(1));
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