// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis velocity computation — rate of change per tick.
//!
//! Computes `velocity = (current - previous) / dt`, then applies optional
//! EMA smoothing.  Zero-allocation after construction; RT-safe (ADR-004).

/// Configuration for velocity computation.
///
/// `smooth_alpha` uses the *retention* convention: `0.0` means no smoothing
/// (instant velocity), `1.0` would freeze the output entirely.
/// This is intentionally **opposite** to the `alpha` convention used by
/// [`crate::EmaFilter`], where `1.0` means passthrough.
#[derive(Debug, Clone, Copy)]
pub struct VelocityConfig {
    /// Tick period in seconds (`1.0 / 250.0` for 250 Hz).
    pub tick_period_secs: f32,
    /// EMA smoothing retention factor in `[0.0, 1.0]`.
    /// `0.0` = instant (no smoothing). Higher = more smoothing.
    pub smooth_alpha: f32,
}

impl VelocityConfig {
    /// 250 Hz spine default: 4 ms period, moderate smoothing.
    pub const fn new_250hz() -> Self {
        Self {
            tick_period_secs: 1.0 / 250.0,
            smooth_alpha: 0.3,
        }
    }
}

impl Default for VelocityConfig {
    fn default() -> Self {
        Self::new_250hz()
    }
}

/// Computes axis velocity (units/sec) from successive position samples.
///
/// Uses first-difference differentiation followed by optional EMA smoothing.
/// RT-safe: only stack state, no heap allocation (ADR-004).
///
/// # Smoothing formula
///
/// ```text
/// smoothed = (1 - smooth_alpha) * raw + smooth_alpha * smoothed_prev
/// ```
///
/// With `smooth_alpha = 0.0` this reduces to `smoothed = raw` (no lag).
#[derive(Debug, Clone, Copy)]
pub struct AxisVelocity {
    config: VelocityConfig,
    prev: f32,
    smoothed: f32,
    initialized: bool,
}

impl AxisVelocity {
    /// Creates a new `AxisVelocity` with the given configuration.
    ///
    /// # Panics
    ///
    /// Panics if `smooth_alpha` is outside `[0.0, 1.0]` or
    /// `tick_period_secs` is not finite and positive.
    pub fn new(config: VelocityConfig) -> Self {
        assert!(
            (0.0..=1.0).contains(&config.smooth_alpha),
            "smooth_alpha must be in [0.0, 1.0], got {}",
            config.smooth_alpha
        );
        assert!(
            config.tick_period_secs.is_finite() && config.tick_period_secs > 0.0,
            "tick_period_secs must be finite and > 0, got {}",
            config.tick_period_secs
        );
        Self {
            config,
            prev: 0.0,
            smoothed: 0.0,
            initialized: false,
        }
    }

    /// Convenience constructor for the 250 Hz default configuration.
    pub fn with_default_250hz() -> Self {
        Self::new(VelocityConfig::new_250hz())
    }

    /// Updates the velocity estimate with a new position sample.
    ///
    /// Returns the smoothed velocity in units/sec.
    ///
    /// The first call after construction or [`reset`](Self::reset) seeds the
    /// previous-position register and returns `0.0` (cold-start).
    #[inline]
    pub fn update(&mut self, position: f32) -> f32 {
        if !self.initialized {
            self.prev = position;
            self.initialized = true;
            return 0.0;
        }
        let raw = (position - self.prev) / self.config.tick_period_secs;
        self.prev = position;
        self.smoothed =
            (1.0 - self.config.smooth_alpha) * raw + self.config.smooth_alpha * self.smoothed;
        self.smoothed
    }

    /// Returns the last computed smoothed velocity (units/sec).
    #[inline]
    pub fn velocity(&self) -> f32 {
        self.smoothed
    }

    /// Resets all internal state (e.g. on device reconnect).
    ///
    /// The next [`update`](Self::update) call will be treated as a cold start.
    #[inline]
    pub fn reset(&mut self) {
        self.prev = 0.0;
        self.smoothed = 0.0;
        self.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 250.0;

    #[test]
    fn test_velocity_zero_at_rest() {
        let mut v = AxisVelocity::with_default_250hz();
        // Cold start — returns 0.
        assert_eq!(v.update(0.5), 0.0);
        // Same position repeated — raw = 0, smoothed stays at 0.
        for _ in 0..10 {
            let vel = v.update(0.5);
            assert!(vel.abs() < 1e-6, "expected ~0, got {vel}");
        }
    }

    #[test]
    fn test_velocity_positive_movement() {
        let config = VelocityConfig {
            tick_period_secs: DT,
            smooth_alpha: 0.0,
        };
        let mut v = AxisVelocity::new(config);
        v.update(0.0); // seed
        let vel = v.update(0.04); // +0.04 per tick
        assert!(vel > 0.0, "expected positive velocity, got {vel}");
    }

    #[test]
    fn test_velocity_negative_movement() {
        let config = VelocityConfig {
            tick_period_secs: DT,
            smooth_alpha: 0.0,
        };
        let mut v = AxisVelocity::new(config);
        v.update(0.0); // seed
        let vel = v.update(-0.04);
        assert!(vel < 0.0, "expected negative velocity, got {vel}");
    }

    #[test]
    fn test_velocity_units() {
        // step of 1.0 in one 250 Hz tick → raw = 1.0 / (1/250) = 250.0 units/sec.
        let config = VelocityConfig {
            tick_period_secs: DT,
            smooth_alpha: 0.0,
        };
        let mut v = AxisVelocity::new(config);
        v.update(0.0); // seed at 0
        let vel = v.update(1.0);
        assert!(
            (vel - 250.0).abs() < 1e-3,
            "expected 250.0 units/sec, got {vel}"
        );
    }

    #[test]
    fn test_velocity_smoothing() {
        // With high smooth_alpha, a step spike is significantly dampened.
        let config = VelocityConfig {
            tick_period_secs: DT,
            smooth_alpha: 0.8,
        };
        let mut v = AxisVelocity::new(config);
        v.update(0.0); // seed
        let spike = v.update(1.0); // raw = 250.0; smoothed = 0.2 * 250 = 50.0
        assert!(
            spike < 250.0,
            "spike={spike} should be dampened below 250.0"
        );
        assert!(spike > 0.0, "spike should be positive, got {spike}");
        // Return to rest — smoothed should decay toward 0, not stay at spike.
        let decayed = v.update(1.0); // position held, raw = 0
        assert!(
            decayed.abs() < spike.abs(),
            "velocity should decay: {decayed} should be < {spike}"
        );
    }

    #[test]
    fn test_velocity_reset() {
        let mut v = AxisVelocity::with_default_250hz();
        v.update(0.0);
        let _ = v.update(1.0); // generates non-zero velocity
        v.reset();
        // First update after reset is cold start → 0.0.
        let after_reset = v.update(0.5);
        assert_eq!(after_reset, 0.0, "cold start after reset should be 0.0");
        // velocity() accessor also reflects reset.
        assert_eq!(v.velocity(), 0.0);
    }

    #[test]
    fn test_velocity_no_smoothing() {
        // smooth_alpha = 0.0 means no lag: output equals raw velocity instantly.
        let config = VelocityConfig {
            tick_period_secs: DT,
            smooth_alpha: 0.0,
        };
        let mut v = AxisVelocity::new(config);
        v.update(0.0); // seed
        // Step up.
        let up = v.update(1.0);
        assert!((up - 250.0).abs() < 1e-3, "expected 250.0, got {up}");
        // Step down.
        let down = v.update(0.0);
        assert!(
            (down - (-250.0)).abs() < 1e-3,
            "expected -250.0, got {down}"
        );
        // Hold — velocity should snap back to 0 immediately.
        let hold = v.update(0.0);
        assert!(hold.abs() < 1e-6, "expected 0.0 (no lag), got {hold}");
    }

    #[test]
    #[should_panic(expected = "smooth_alpha must be in [0.0, 1.0]")]
    fn test_velocity_invalid_alpha_panics() {
        AxisVelocity::new(VelocityConfig {
            tick_period_secs: DT,
            smooth_alpha: 1.5,
        });
    }

    #[test]
    #[should_panic(expected = "tick_period_secs must be finite and > 0")]
    fn test_velocity_invalid_period_panics() {
        AxisVelocity::new(VelocityConfig {
            tick_period_secs: 0.0,
            smooth_alpha: 0.3,
        });
    }
}
