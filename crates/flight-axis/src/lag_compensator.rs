//! Input lag compensation using linear velocity prediction.
//!
//! Predicts future axis position by estimating velocity from recent samples
//! and extrapolating over a configurable prediction horizon.
//!
//! Zero-allocation on hot path — all state is inline.

/// Configuration for lag compensation.
#[derive(Debug, Clone, Copy)]
pub struct LagCompensatorConfig {
    /// Prediction horizon in seconds.
    /// Typical values: 0.008–0.016 (8–16ms for a 60Hz display).
    pub horizon_secs: f32,
    /// Minimum input change to consider the axis "moving".
    /// Below this, prediction is suppressed.
    pub movement_threshold: f32,
    /// Maximum predicted position change (clamped output delta).
    pub max_prediction: f32,
}

impl Default for LagCompensatorConfig {
    fn default() -> Self {
        Self {
            horizon_secs: 0.010,
            movement_threshold: 0.001,
            max_prediction: 0.05,
        }
    }
}

/// Input lag compensator using single-step velocity estimation.
#[derive(Debug, Clone)]
pub struct LagCompensator {
    last_value: f32,
    last_dt: f32,
    config: LagCompensatorConfig,
    initialized: bool,
}

impl LagCompensator {
    /// Create a new lag compensator.
    pub fn new(config: LagCompensatorConfig) -> Self {
        Self {
            last_value: 0.0,
            last_dt: 1.0 / 250.0, // default 250Hz
            config,
            initialized: false,
        }
    }

    /// Process a new sample and return the lag-compensated output.
    ///
    /// `value` is the current axis position `[-1.0, 1.0]`.
    /// `dt_secs` is the time since the last sample in seconds.
    ///
    /// Zero-allocation — safe to call from RT code.
    pub fn process(&mut self, value: f32, dt_secs: f32) -> f32 {
        if value.is_nan() || value.is_infinite() {
            return value;
        }
        if !self.initialized {
            self.last_value = value;
            self.last_dt = dt_secs;
            self.initialized = true;
            return value;
        }

        let delta = value - self.last_value;

        // Suppress prediction when axis is at rest
        if delta.abs() < self.config.movement_threshold {
            self.last_value = value;
            self.last_dt = dt_secs;
            return value;
        }

        // Estimate velocity (units per second)
        let dt = if dt_secs > 0.0 { dt_secs } else { self.last_dt };
        let velocity = delta / dt;

        // Predict ahead by horizon_secs
        let predicted_delta = (velocity * self.config.horizon_secs)
            .clamp(-self.config.max_prediction, self.config.max_prediction);

        let predicted = (value + predicted_delta).clamp(-1.0, 1.0);

        self.last_value = value;
        self.last_dt = dt_secs;

        predicted
    }

    /// Reset state to uninitialized.
    pub fn reset(&mut self) {
        self.last_value = 0.0;
        self.initialized = false;
    }

    /// Whether the compensator has been initialized with at least one sample.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for LagCompensator {
    fn default() -> Self {
        Self::new(LagCompensatorConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 250.0;

    #[test]
    fn first_sample_passes_through() {
        let mut c = LagCompensator::new(LagCompensatorConfig::default());
        assert_eq!(c.process(0.5, DT), 0.5);
    }

    #[test]
    fn static_axis_unchanged() {
        let mut c = LagCompensator::new(LagCompensatorConfig::default());
        c.process(0.5, DT);
        let out = c.process(0.5, DT);
        assert_eq!(out, 0.5); // no movement → no prediction
    }

    #[test]
    fn moving_axis_is_predicted_ahead() {
        let config = LagCompensatorConfig {
            horizon_secs: 0.01,
            movement_threshold: 0.0001,
            max_prediction: 0.5,
        };
        let mut c = LagCompensator::new(config);
        c.process(0.0, DT);
        let out = c.process(0.01, DT);
        // velocity = 0.01 / DT = 2.5 units/s; delta = 2.5 * 0.01 = 0.025
        // predicted = 0.01 + 0.025 = 0.035
        assert!(out > 0.01);
    }

    #[test]
    fn output_is_clamped_to_valid_range() {
        let config = LagCompensatorConfig {
            horizon_secs: 1.0, // large horizon
            movement_threshold: 0.0001,
            max_prediction: 0.5,
        };
        let mut c = LagCompensator::new(config);
        c.process(0.9, DT);
        let out = c.process(0.95, DT);
        assert!(out <= 1.0);
    }

    #[test]
    fn nan_passes_through_unchanged() {
        let mut c = LagCompensator::new(LagCompensatorConfig::default());
        c.process(0.5, DT);
        let out = c.process(f32::NAN, DT);
        assert!(out.is_nan());
    }

    #[test]
    fn reset_clears_initialized() {
        let mut c = LagCompensator::new(LagCompensatorConfig::default());
        c.process(0.5, DT);
        assert!(c.is_initialized());
        c.reset();
        assert!(!c.is_initialized());
    }

    #[test]
    fn negative_movement_predicts_backward() {
        let config = LagCompensatorConfig {
            horizon_secs: 0.01,
            movement_threshold: 0.0001,
            max_prediction: 0.5,
        };
        let mut c = LagCompensator::new(config);
        c.process(0.0, DT);
        let out = c.process(-0.01, DT);
        assert!(out < -0.01);
    }

    #[test]
    fn max_prediction_clamped() {
        let config = LagCompensatorConfig {
            horizon_secs: 1.0,
            movement_threshold: 0.0001,
            max_prediction: 0.01,
        };
        let mut c = LagCompensator::new(config);
        c.process(0.0, DT);
        let out = c.process(0.5, DT);
        // Delta was clamped to 0.01
        assert!(out <= 0.5 + 0.01 + 1e-6);
    }

    #[test]
    fn zero_dt_uses_last_dt() {
        let mut c = LagCompensator::new(LagCompensatorConfig::default());
        c.process(0.0, DT);
        // dt = 0 should not divide by zero
        let out = c.process(0.1, 0.0);
        assert!(out.is_finite());
    }
}
