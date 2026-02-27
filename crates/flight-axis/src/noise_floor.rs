//! Axis noise floor detection for automatic micro-deadzone calibration.
//!
//! Measures signal variance when the axis is at rest and applies an
//! adaptive micro-deadzone above the detected noise level.
//!
//! Zero-allocation on hot path — all state is inline.

/// Configuration for noise floor detection.
#[derive(Debug, Clone, Copy)]
pub struct NoiseFloorConfig {
    /// Number of samples used for variance measurement window.
    /// Must be at least 2.
    pub window_size: usize,
    /// Multiplier applied to RMS noise to compute auto-deadzone.
    /// Typical value: 2.5–4.0.
    pub deadzone_multiplier: f32,
    /// Maximum auto-deadzone radius (clamped if noise is very high).
    pub max_auto_deadzone: f32,
    /// Minimum movement threshold to detect "not at rest".
    pub rest_threshold: f32,
}

impl Default for NoiseFloorConfig {
    fn default() -> Self {
        Self {
            window_size: 64,
            deadzone_multiplier: 3.0,
            max_auto_deadzone: 0.02,
            rest_threshold: 0.01,
        }
    }
}

/// Noise floor detector state.
///
/// Uses a running variance estimate over a fixed window.
/// All storage is on the stack via const-generic array.
#[derive(Clone)]
pub struct NoiseFloorDetector<const N: usize> {
    samples: [f32; N],
    head: usize,
    count: usize,
    sum: f64,
    sum_sq: f64,
    config: NoiseFloorConfig,
}

/// Type alias for 64-sample detector (default).
pub type NoiseFloorDetector64 = NoiseFloorDetector<64>;

impl<const N: usize> NoiseFloorDetector<N> {
    /// Create a new detector with the given configuration.
    ///
    /// # Panics
    /// Panics in debug builds if `N < 2`.
    pub fn new(config: NoiseFloorConfig) -> Self {
        debug_assert!(N >= 2, "NoiseFloorDetector requires N >= 2");
        Self {
            samples: [0.0f32; N],
            head: 0,
            count: 0,
            sum: 0.0f64,
            sum_sq: 0.0f64,
            config,
        }
    }

    /// Push a new sample and update running statistics.
    ///
    /// Zero-allocation — safe to call from RT code.
    pub fn push(&mut self, value: f32) {
        if value.is_nan() || value.is_infinite() {
            return;
        }
        if self.count == N {
            // Remove oldest sample from accumulators
            let old = self.samples[self.head] as f64;
            self.sum -= old;
            self.sum_sq -= old * old;
        } else {
            self.count += 1;
        }
        self.samples[self.head] = value;
        let v64 = value as f64;
        self.sum += v64;
        self.sum_sq += v64 * v64;
        self.head = (self.head + 1) % N;
    }

    /// Number of samples collected so far.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Mean of collected samples.
    pub fn mean(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }
        (self.sum / self.count as f64) as f32
    }

    /// Root mean square of samples (unbiased variance approximation).
    pub fn rms_noise(&self) -> f32 {
        if self.count < 2 {
            return 0.0;
        }
        let mean = self.sum / self.count as f64;
        let variance = (self.sum_sq / self.count as f64) - (mean * mean);
        variance.abs().sqrt() as f32
    }

    /// Computed auto-deadzone radius based on current noise level.
    pub fn auto_deadzone(&self) -> f32 {
        let dz = self.rms_noise() * self.config.deadzone_multiplier;
        dz.min(self.config.max_auto_deadzone)
    }

    /// Returns `true` if the axis appears to be at rest.
    pub fn is_at_rest(&self) -> bool {
        if self.count < 2 {
            return true;
        }
        let mean = self.mean();
        let max_deviation = self.samples[..self.count]
            .iter()
            .map(|&s| (s - mean).abs())
            .fold(0.0f32, f32::max);
        max_deviation < self.config.rest_threshold
    }

    /// Reset all state to zero.
    pub fn reset(&mut self) {
        self.samples = [0.0f32; N];
        self.head = 0;
        self.count = 0;
        self.sum = 0.0f64;
        self.sum_sq = 0.0f64;
    }
}

impl<const N: usize> Default for NoiseFloorDetector<N> {
    fn default() -> Self {
        Self::new(NoiseFloorConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_detector_is_empty() {
        let d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        assert_eq!(d.count(), 0);
        assert_eq!(d.mean(), 0.0);
        assert_eq!(d.rms_noise(), 0.0);
    }

    #[test]
    fn single_sample_gives_zero_noise() {
        let mut d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        d.push(0.5);
        assert_eq!(d.count(), 1);
        assert_eq!(d.rms_noise(), 0.0);
    }

    #[test]
    fn constant_signal_has_zero_noise() {
        let mut d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        for _ in 0..64 {
            d.push(0.3);
        }
        assert!(d.rms_noise() < 1e-4);
    }

    #[test]
    fn noisy_signal_has_positive_noise() {
        let mut d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        for i in 0..64 {
            let v = if i % 2 == 0 { 0.01f32 } else { -0.01f32 };
            d.push(v);
        }
        assert!(d.rms_noise() > 0.005);
    }

    #[test]
    fn auto_deadzone_clamped_to_max() {
        let config = NoiseFloorConfig {
            deadzone_multiplier: 100.0,
            max_auto_deadzone: 0.02,
            ..Default::default()
        };
        let mut d: NoiseFloorDetector<64> = NoiseFloorDetector::new(config);
        for i in 0..64 {
            d.push(if i % 2 == 0 { 0.1 } else { -0.1 });
        }
        assert!(d.auto_deadzone() <= 0.02 + f32::EPSILON);
    }

    #[test]
    fn is_at_rest_with_constant_signal() {
        let mut d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        for _ in 0..20 {
            d.push(0.5);
        }
        assert!(d.is_at_rest());
    }

    #[test]
    fn is_not_at_rest_with_varying_signal() {
        let mut d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        for i in 0..64 {
            d.push(if i < 32 { 0.0 } else { 0.5 });
        }
        assert!(!d.is_at_rest());
    }

    #[test]
    fn nan_is_ignored() {
        let mut d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        d.push(0.5);
        d.push(f32::NAN);
        assert_eq!(d.count(), 1);
    }

    #[test]
    fn ring_buffer_wraps_correctly() {
        let mut d: NoiseFloorDetector<4> = NoiseFloorDetector::new(NoiseFloorConfig::default());
        for v in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
            d.push(v);
        }
        assert_eq!(d.count(), 4);
        // Mean should be (3+4+5+6)/4 = 4.5
        assert!((d.mean() - 4.5).abs() < 1e-4);
    }

    #[test]
    fn reset_clears_state() {
        let mut d: NoiseFloorDetector64 = NoiseFloorDetector::new(NoiseFloorConfig::default());
        for _ in 0..10 {
            d.push(0.5);
        }
        d.reset();
        assert_eq!(d.count(), 0);
        assert_eq!(d.rms_noise(), 0.0);
    }
}
