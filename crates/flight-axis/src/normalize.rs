// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis value normalization and sanity validation.
//!
//! Ensures axis values are always in [-1.0, 1.0] after pipeline processing.
//! Handles NaN/Inf by replacing with 0.0 and incrementing an error counter.

/// Configuration for axis value normalization validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizeConfig {
    /// If true, NaN/Inf are replaced with 0.0. If false, they are clamped (but NaN
    /// cannot be meaningfully clamped, so `sanitize_nan = false` leaves NaN through
    /// the clamp stage — prefer leaving this at `true` in production).
    pub sanitize_nan: bool,
    /// If true, values outside [-1.0, 1.0] are clamped. Always true in practice;
    /// the field exists so callers can read the configured policy.
    pub clamp: bool,
}

impl Default for NormalizeConfig {
    fn default() -> Self {
        Self {
            sanitize_nan: true,
            clamp: true,
        }
    }
}

/// Axis normalization validator — ensures output is in [-1.0, 1.0].
///
/// Zero-allocation: all state is stack-resident (two `u32` counters).
#[derive(Debug, Clone, Copy)]
pub struct AxisNormalizer {
    config: NormalizeConfig,
    nan_count: u32,
    clamp_count: u32,
}

impl AxisNormalizer {
    /// Creates a new [`AxisNormalizer`] with the given configuration.
    pub fn new(config: NormalizeConfig) -> Self {
        Self {
            config,
            nan_count: 0,
            clamp_count: 0,
        }
    }

    /// Validate and normalize `input` to [-1.0, 1.0].
    ///
    /// - If `input` is NaN or Inf and `sanitize_nan` is `true`: returns `0.0` and
    ///   increments [`nan_count`](Self::nan_count).
    /// - If `input` is outside `[-1.0, 1.0]`: clamps it and increments
    ///   [`clamp_count`](Self::clamp_count).
    /// - Otherwise: returns `input` unchanged.
    pub fn process(&mut self, input: f32) -> f32 {
        if !input.is_finite() && self.config.sanitize_nan {
            self.nan_count += 1;
            return 0.0;
        }
        if !(-1.0..=1.0).contains(&input) {
            self.clamp_count += 1;
            return input.clamp(-1.0, 1.0);
        }
        input
    }

    /// Returns the number of NaN/Inf values replaced since the last [`reset_counters`](Self::reset_counters).
    pub fn nan_count(&self) -> u32 {
        self.nan_count
    }

    /// Returns the number of out-of-range values clamped since the last [`reset_counters`](Self::reset_counters).
    pub fn clamp_count(&self) -> u32 {
        self.clamp_count
    }

    /// Resets both diagnostic counters to zero.
    pub fn reset_counters(&mut self) {
        self.nan_count = 0;
        self.clamp_count = 0;
    }
}

/// A bank of `N` independent [`AxisNormalizer`]s, one per axis.
///
/// Zero-allocation: the array lives on the stack for small `N`.
pub struct NormalizerBank<const N: usize> {
    normalizers: [AxisNormalizer; N],
}

impl<const N: usize> NormalizerBank<N> {
    /// Creates a bank where every normalizer shares the same [`NormalizeConfig`].
    pub fn new(config: NormalizeConfig) -> Self {
        Self {
            normalizers: [AxisNormalizer::new(config); N],
        }
    }

    /// Processes each element of `inputs` through its corresponding normalizer,
    /// writing results into `outputs`.
    pub fn process(&mut self, inputs: &[f32; N], outputs: &mut [f32; N]) {
        for i in 0..N {
            outputs[i] = self.normalizers[i].process(inputs[i]);
        }
    }

    /// Returns the sum of [`nan_count`](AxisNormalizer::nan_count) across all axes.
    pub fn total_nan_count(&self) -> u32 {
        self.normalizers.iter().map(|n| n.nan_count()).sum()
    }

    /// Returns the sum of [`clamp_count`](AxisNormalizer::clamp_count) across all axes.
    pub fn total_clamp_count(&self) -> u32 {
        self.normalizers.iter().map(|n| n.clamp_count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_normalizer() -> AxisNormalizer {
        AxisNormalizer::new(NormalizeConfig::default())
    }

    #[test]
    fn test_valid_value_passes_unchanged() {
        let mut n = default_normalizer();
        assert_eq!(n.process(0.5), 0.5);
    }

    #[test]
    fn test_nan_is_replaced_with_zero() {
        let mut n = default_normalizer();
        assert_eq!(n.process(f32::NAN), 0.0);
    }

    #[test]
    fn test_inf_is_replaced_with_zero() {
        let mut n = default_normalizer();
        assert_eq!(n.process(f32::INFINITY), 0.0);
    }

    #[test]
    fn test_neg_inf_is_replaced_with_zero() {
        let mut n = default_normalizer();
        assert_eq!(n.process(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn test_value_over_one_is_clamped() {
        let mut n = default_normalizer();
        assert_eq!(n.process(1.5), 1.0);
    }

    #[test]
    fn test_value_under_minus_one_is_clamped() {
        let mut n = default_normalizer();
        assert_eq!(n.process(-1.5), -1.0);
    }

    #[test]
    fn test_nan_count_increments() {
        let mut n = default_normalizer();
        n.process(f32::NAN);
        n.process(f32::INFINITY);
        n.process(f32::NEG_INFINITY);
        assert_eq!(n.nan_count(), 3);
    }

    #[test]
    fn test_clamp_count_increments() {
        let mut n = default_normalizer();
        n.process(1.5);
        n.process(-2.0);
        assert_eq!(n.clamp_count(), 2);
    }

    #[test]
    fn test_reset_counters_clears_counts() {
        let mut n = default_normalizer();
        n.process(f32::NAN);
        n.process(1.5);
        n.reset_counters();
        assert_eq!(n.nan_count(), 0);
        assert_eq!(n.clamp_count(), 0);
    }

    #[test]
    fn test_bank_processes_all_axes() {
        let mut bank: NormalizerBank<3> = NormalizerBank::new(NormalizeConfig::default());
        let inputs = [f32::NAN, 1.5, 0.3];
        let mut outputs = [0.0f32; 3];
        bank.process(&inputs, &mut outputs);
        assert_eq!(outputs[0], 0.0, "NaN should become 0.0");
        assert_eq!(outputs[1], 1.0, "1.5 should be clamped to 1.0");
        assert!(
            (outputs[2] - 0.3).abs() < f32::EPSILON,
            "0.3 should pass unchanged"
        );
        assert_eq!(bank.total_nan_count(), 1);
        assert_eq!(bank.total_clamp_count(), 1);
    }

    #[test]
    fn test_minus_one_and_plus_one_are_valid() {
        let mut n = default_normalizer();
        assert_eq!(n.process(-1.0), -1.0);
        assert_eq!(n.process(1.0), 1.0);
        assert_eq!(n.nan_count(), 0);
        assert_eq!(n.clamp_count(), 0);
    }
}
