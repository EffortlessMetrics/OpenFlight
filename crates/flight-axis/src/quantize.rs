// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis output quantization — limit output to N discrete steps.
//!
//! Implements REQ-661: axis engine output quantization for legacy sims
//! (e.g. 12-bit ADC with 4096 levels).
//!
//! Zero-allocation. RT-safe. Applied as a final pipeline stage.

/// Configuration for output quantization.
///
/// Defines the number of discrete levels and the output value range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantizeConfig {
    /// Number of discrete output levels (including both endpoints).
    ///
    /// For example, `steps = 4096` gives 12-bit resolution.
    /// Clamped to a minimum of 2 on construction.
    pub steps: u32,
    /// Minimum output value (default `-1.0`).
    pub min: f32,
    /// Maximum output value (default `1.0`).
    pub max: f32,
}

impl QuantizeConfig {
    /// Creates a configuration with `steps` levels over `[-1.0, 1.0]`.
    pub const fn new(steps: u32) -> Self {
        Self { steps, min: -1.0, max: 1.0 }
    }

    /// Creates a configuration with `steps` levels over `[min, max]`.
    pub const fn with_range(steps: u32, min: f32, max: f32) -> Self {
        Self { steps, min, max }
    }
}

impl Default for QuantizeConfig {
    fn default() -> Self {
        Self::new(4096)
    }
}

/// Quantizes axis output to a fixed number of evenly-spaced discrete steps.
///
/// Zero-allocation: all state is stack-resident. RT-safe.
///
/// # Example
///
/// ```rust
/// use flight_axis::{AxisQuantize, QuantizeConfig};
///
/// let mut q = AxisQuantize::new(QuantizeConfig::new(4096));
/// let out = q.process(0.5);
/// assert!((out - 0.5).abs() < 1e-3);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct AxisQuantize {
    config: QuantizeConfig,
    /// Pre-computed step size to avoid repeated division on the hot path.
    step_size: f32,
}

impl AxisQuantize {
    /// Creates a new [`AxisQuantize`] from the given configuration.
    ///
    /// `steps` is clamped to a minimum of 2.
    pub fn new(config: QuantizeConfig) -> Self {
        let steps = config.steps.max(2);
        let step_size = (config.max - config.min) / (steps - 1) as f32;
        Self { config: QuantizeConfig { steps, ..config }, step_size }
    }

    /// Returns the quantized value nearest to `value`. RT-safe.
    ///
    /// Values outside `[min, max]` are clamped before quantization.
    #[inline]
    pub fn quantize(&self, value: f32) -> f32 {
        let clamped = value.clamp(self.config.min, self.config.max);
        let normalized = (clamped - self.config.min) / (self.config.max - self.config.min);
        let step_idx = (normalized * (self.config.steps - 1) as f32).round() as u32;
        let step_idx = step_idx.min(self.config.steps - 1);
        self.config.min + step_idx as f32 * self.step_size
    }

    /// Quantizes `value`. Equivalent to [`quantize`](Self::quantize). RT-safe.
    #[inline]
    pub fn process(&mut self, value: f32) -> f32 {
        self.quantize(value)
    }

    /// No-op reset — quantization is stateless.
    #[inline]
    pub fn reset(&mut self) {}

    /// Returns the active configuration.
    #[inline]
    pub fn config(&self) -> &QuantizeConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quantizer(steps: u32) -> AxisQuantize {
        AxisQuantize::new(QuantizeConfig::new(steps))
    }

    // ── Boundary values ───────────────────────────────────────────────────────

    #[test]
    fn test_quantize_boundary_values() {
        let q = quantizer(4096);
        assert_eq!(q.quantize(-1.0), -1.0, "min boundary must be preserved");
        assert_eq!(q.quantize(1.0), 1.0, "max boundary must be preserved");
    }

    // ── Midpoint ──────────────────────────────────────────────────────────────

    #[test]
    fn test_quantize_midpoint() {
        // 3 steps → [-1.0, 0.0, 1.0]; 0.0 is an exact grid point.
        // normalized(0.0) = 0.5, step_idx = round(0.5 * 2) = 1, result = -1 + 1*1 = 0.0
        let q = quantizer(3);
        assert_eq!(q.quantize(0.0), 0.0);
    }

    // ── Rounding ──────────────────────────────────────────────────────────────

    #[test]
    fn test_quantize_rounds_down() {
        // 3 steps → [-1.0, 0.0, 1.0], step_size = 1.0
        // value = -0.6: normalized = 0.2, step_idx = round(0.4) = 0, result = -1.0
        let q = quantizer(3);
        assert_eq!(q.quantize(-0.6), -1.0);
    }

    #[test]
    fn test_quantize_rounds_to_nearest() {
        // 3 steps → [-1.0, 0.0, 1.0], step_size = 1.0
        // value = 0.4: normalized = 0.7, step_idx = round(1.4) = 1, result = 0.0
        let q = quantizer(3);
        assert_eq!(q.quantize(0.4), 0.0);
    }

    // ── Out-of-range clamping ─────────────────────────────────────────────────

    #[test]
    fn test_quantize_clamps_beyond_max() {
        let q = quantizer(100);
        assert_eq!(q.quantize(1.5), 1.0);
    }

    #[test]
    fn test_quantize_clamps_below_min() {
        let q = quantizer(100);
        assert_eq!(q.quantize(-1.5), -1.0);
    }

    // ── High resolution ───────────────────────────────────────────────────────

    #[test]
    fn test_quantize_high_resolution() {
        // 4096 steps; the nearest grid point to 0.5 is within step_size/2.
        let config = QuantizeConfig::new(4096);
        let q = AxisQuantize::new(config);
        let step_size = 2.0_f32 / (4096 - 1) as f32;
        let result = q.quantize(0.5);
        assert!(
            (result - 0.5).abs() <= step_size / 2.0 + f32::EPSILON,
            "result {result} should be within step_size/2 of 0.5"
        );
    }

    // ── Binary (2-step) quantization ──────────────────────────────────────────

    #[test]
    fn test_quantize_2_steps() {
        // 2 steps → [-1.0, 1.0]; negative values → -1.0, positive → 1.0
        let q = quantizer(2);
        assert_eq!(q.quantize(-0.1), -1.0);
        assert_eq!(q.quantize(0.1), 1.0);
        assert_eq!(q.quantize(-1.0), -1.0);
        assert_eq!(q.quantize(1.0), 1.0);
    }

    // ── process() delegation ──────────────────────────────────────────────────

    #[test]
    fn test_process_delegates_to_quantize() {
        let config = QuantizeConfig::new(256);
        let mut q = AxisQuantize::new(config);
        let immut_q = AxisQuantize::new(config);
        for &v in &[-1.0_f32, -0.5, 0.0, 0.3, 1.0] {
            assert_eq!(
                q.process(v),
                immut_q.quantize(v),
                "process({v}) must equal quantize({v})"
            );
        }
    }

    // ── Custom range ──────────────────────────────────────────────────────────

    #[test]
    fn test_quantize_custom_range() {
        // 3 steps over [0.0, 1.0] → [0.0, 0.5, 1.0]
        let config = QuantizeConfig::with_range(3, 0.0, 1.0);
        let q = AxisQuantize::new(config);
        assert_eq!(q.quantize(0.0), 0.0);
        assert_eq!(q.quantize(1.0), 1.0);
        // 0.26: normalized = 0.26, step_idx = round(0.52) = 1, result = 0.5
        assert_eq!(q.quantize(0.26), 0.5);
    }

    // ── steps < 2 is clamped ──────────────────────────────────────────────────

    #[test]
    fn test_quantize_steps_clamped_to_2() {
        // steps=0 should be treated as steps=2
        let config = QuantizeConfig::new(0);
        let q = AxisQuantize::new(config);
        assert_eq!(q.config().steps, 2);
        assert_eq!(q.quantize(-0.5), -1.0);
        assert_eq!(q.quantize(0.5), 1.0);
    }
}
