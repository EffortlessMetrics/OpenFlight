// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safety interlock for force feedback output
//!
//! Prevents dangerous FFB force levels by enforcing soft/hard limits,
//! ramp-rate limiting, and emergency stop. All hot-path operations use
//! atomic counters — no heap allocations.
//!
//! **Validates: ADR-009 Safety Interlock Design**

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Safety interlock for force feedback output.
///
/// Ensures forces never exceed safe limits and provides emergency stop.
/// Thread-safe emergency stop via [`AtomicBool`]; clamp counter via [`AtomicU64`].
pub struct SafetyInterlock {
    max_force_percent: f64,
    ramp_rate_limit: f64,
    emergency_stopped: AtomicBool,
    force_clamp_count: AtomicU64,
    last_output_force: f64,
    soft_limit_percent: f64,
    hard_limit_percent: f64,
    enabled: bool,
}

/// Result of passing a force value through the safety interlock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SafetyInterlockResult {
    /// Force passed through unchanged.
    Passed(f64),
    /// Force reduced to the soft limit.
    SoftLimited(f64),
    /// Force clamped to the hard limit.
    HardLimited(f64),
    /// Force change rate was limited.
    RampLimited(f64),
    /// All force zeroed due to emergency stop.
    EmergencyStopped,
}

/// Configuration for [`SafetyInterlock`].
#[derive(Debug, Clone, Copy)]
pub struct SafetyConfig {
    /// Soft warning threshold (0.0–100.0).
    pub soft_limit_percent: f64,
    /// Absolute maximum (0.0–100.0).
    pub hard_limit_percent: f64,
    /// Maximum change per tick (% per 4 ms).
    pub ramp_rate_limit: f64,
    /// Initial maximum force percent.
    pub initial_max_force: f64,
}

impl SafetyInterlock {
    /// Create a new safety interlock from the given configuration.
    pub fn new(config: SafetyConfig) -> Self {
        Self {
            max_force_percent: config.initial_max_force,
            ramp_rate_limit: config.ramp_rate_limit,
            emergency_stopped: AtomicBool::new(false),
            force_clamp_count: AtomicU64::new(0),
            last_output_force: 0.0,
            soft_limit_percent: config.soft_limit_percent,
            hard_limit_percent: config.hard_limit_percent,
            enabled: true,
        }
    }

    /// Main safety gate — checks a requested force value against all limits.
    ///
    /// Priority order:
    /// 1. Emergency stop → zero force
    /// 2. Disabled → pass through
    /// 3. Hard limit
    /// 4. Soft limit
    /// 5. Ramp-rate limit
    /// 6. Pass through
    pub fn check_force(&mut self, requested_force: f64) -> SafetyInterlockResult {
        // 1. Emergency stop
        if self.emergency_stopped.load(Ordering::Acquire) {
            self.last_output_force = 0.0;
            return SafetyInterlockResult::EmergencyStopped;
        }

        // 2. Disabled — pass everything
        if !self.enabled {
            self.last_output_force = requested_force;
            return SafetyInterlockResult::Passed(requested_force);
        }

        let abs_force = requested_force.abs();
        let sign = requested_force.signum();

        // 3. Hard limit
        if abs_force > self.hard_limit_percent {
            self.force_clamp_count.fetch_add(1, Ordering::Relaxed);
            let clamped = self.hard_limit_percent * sign;
            self.last_output_force = clamped;
            return SafetyInterlockResult::HardLimited(clamped);
        }

        // 4. Soft limit
        if abs_force > self.soft_limit_percent {
            let clamped = self.soft_limit_percent * sign;
            self.last_output_force = clamped;
            return SafetyInterlockResult::SoftLimited(clamped);
        }

        // 5. Ramp-rate limit
        let delta = (requested_force - self.last_output_force).abs();
        if delta > self.ramp_rate_limit {
            let direction = (requested_force - self.last_output_force).signum();
            let clamped = self.last_output_force + direction * self.ramp_rate_limit;
            self.last_output_force = clamped;
            return SafetyInterlockResult::RampLimited(clamped);
        }

        // 6. Passed
        self.last_output_force = requested_force;
        SafetyInterlockResult::Passed(requested_force)
    }

    /// Trigger emergency stop — zeroes all force immediately.
    pub fn emergency_stop(&self) {
        self.emergency_stopped.store(true, Ordering::Release);
    }

    /// Release emergency stop so normal operation can resume.
    pub fn release_emergency_stop(&self) {
        self.emergency_stopped.store(false, Ordering::Release);
    }

    /// Returns `true` if emergency stop is currently active.
    pub fn is_emergency_stopped(&self) -> bool {
        self.emergency_stopped.load(Ordering::Acquire)
    }

    /// Adjust the maximum force at runtime.
    pub fn set_max_force(&mut self, percent: f64) {
        self.max_force_percent = percent;
    }

    /// Number of times force was hard-limited.
    pub fn clamp_count(&self) -> u64 {
        self.force_clamp_count.load(Ordering::Relaxed)
    }

    /// Reset the hard-limit clamp counter to zero.
    pub fn reset_clamp_count(&self) {
        self.force_clamp_count.store(0, Ordering::Relaxed);
    }

    /// Last force value that was output.
    pub fn last_output(&self) -> f64 {
        self.last_output_force
    }

    /// Whether the interlock is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable the safety interlock.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the safety interlock (forces pass through unchecked).
    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn default_config() -> SafetyConfig {
        SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 10.0,
            initial_max_force: 100.0,
        }
    }

    // 1. Force below limits passes through
    #[test]
    fn safety_interlock_force_below_limits_passes() {
        let mut il = SafetyInterlock::new(default_config());
        il.last_output_force = 45.0;
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::Passed(50.0));
        assert_eq!(il.last_output(), 50.0);
    }

    // 2. Force above soft limit is reduced
    #[test]
    fn safety_interlock_soft_limit_reduces_force() {
        let mut il = SafetyInterlock::new(default_config());
        // Ramp up to avoid ramp-rate limiting
        il.last_output_force = 85.0;
        let result = il.check_force(85.0);
        assert_eq!(result, SafetyInterlockResult::SoftLimited(80.0));
    }

    // 3. Force above hard limit is clamped
    #[test]
    fn safety_interlock_hard_limit_clamps_force() {
        let mut il = SafetyInterlock::new(default_config());
        il.last_output_force = 100.0;
        let result = il.check_force(120.0);
        assert_eq!(result, SafetyInterlockResult::HardLimited(100.0));
    }

    // 4. Emergency stop zeroes force
    #[test]
    fn safety_interlock_emergency_stop_zeroes_force() {
        let mut il = SafetyInterlock::new(default_config());
        il.emergency_stop();
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
        assert_eq!(il.last_output(), 0.0);
    }

    // 5. Release emergency stop resumes normal operation
    #[test]
    fn safety_interlock_release_emergency_stop_resumes() {
        let mut il = SafetyInterlock::new(default_config());
        il.emergency_stop();
        assert!(il.is_emergency_stopped());
        il.release_emergency_stop();
        assert!(!il.is_emergency_stopped());
        let result = il.check_force(5.0);
        assert_eq!(result, SafetyInterlockResult::Passed(5.0));
    }

    // 6. Ramp rate limiting works
    #[test]
    fn safety_interlock_ramp_rate_limiting() {
        let mut il = SafetyInterlock::new(default_config());
        // last_output_force starts at 0; jump to 50 exceeds ramp_rate_limit of 10
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::RampLimited(10.0));
        assert_eq!(il.last_output(), 10.0);
    }

    // 7. Clamp count increments correctly
    #[test]
    fn safety_interlock_clamp_count_increments() {
        let mut il = SafetyInterlock::new(default_config());
        il.last_output_force = 100.0;
        assert_eq!(il.clamp_count(), 0);
        il.check_force(110.0);
        assert_eq!(il.clamp_count(), 1);
        il.last_output_force = 100.0;
        il.check_force(120.0);
        assert_eq!(il.clamp_count(), 2);
    }

    // 8. Reset clamp count
    #[test]
    fn safety_interlock_reset_clamp_count() {
        let mut il = SafetyInterlock::new(default_config());
        il.last_output_force = 100.0;
        il.check_force(110.0);
        assert_eq!(il.clamp_count(), 1);
        il.reset_clamp_count();
        assert_eq!(il.clamp_count(), 0);
    }

    // 9. Disabled interlock passes everything
    #[test]
    fn safety_interlock_disabled_passes_everything() {
        let mut il = SafetyInterlock::new(default_config());
        il.disable();
        let result = il.check_force(200.0);
        assert_eq!(result, SafetyInterlockResult::Passed(200.0));
    }

    // 10. Enable / disable toggle
    #[test]
    fn safety_interlock_enable_disable_toggle() {
        let mut il = SafetyInterlock::new(default_config());
        assert!(il.is_enabled());
        il.disable();
        assert!(!il.is_enabled());
        il.enable();
        assert!(il.is_enabled());
    }

    // 11. Negative forces handled (absolute value comparison)
    #[test]
    fn safety_interlock_negative_forces_handled() {
        let mut il = SafetyInterlock::new(default_config());
        il.last_output_force = -100.0;

        // Negative force exceeding hard limit
        let result = il.check_force(-120.0);
        assert_eq!(result, SafetyInterlockResult::HardLimited(-100.0));

        // Negative force exceeding soft limit
        il.last_output_force = -85.0;
        let result = il.check_force(-85.0);
        assert_eq!(result, SafetyInterlockResult::SoftLimited(-80.0));

        // Negative force within limits
        il.last_output_force = -50.0;
        let result = il.check_force(-50.0);
        assert_eq!(result, SafetyInterlockResult::Passed(-50.0));
    }

    // 12. Concurrent emergency stop (atomic safety)
    #[test]
    fn safety_interlock_concurrent_emergency_stop() {
        let il = Arc::new(SafetyInterlock::new(default_config()));
        let il2 = Arc::clone(&il);

        let handle = std::thread::spawn(move || {
            il2.emergency_stop();
        });
        handle.join().unwrap();

        assert!(il.is_emergency_stopped());
        il.release_emergency_stop();
        assert!(!il.is_emergency_stopped());
    }
}
