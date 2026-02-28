// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Zero-allocation axis mixer for combining multiple axes into one output.
//!
//! Supports weighted sum, max, min, and priority-based mixing modes.
//! All state is stack-allocated with a fixed maximum of [`MAX_MIXER_INPUTS`] inputs.
//!
//! # Use case
//!
//! Combine physical axis + trim offset = final output:
//!
//! ```rust
//! use flight_axis::mixer::{AxisMixer, MixMode};
//!
//! let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
//! let physical_axis = 0.5;
//! let trim = 0.05;
//! let output = mixer.combine(&[physical_axis, trim]);
//! assert!((output - 0.55).abs() < 1e-10);
//! ```

/// Maximum number of inputs an [`AxisMixer`] can combine.
pub const MAX_MIXER_INPUTS: usize = 8;

/// Mixing mode for combining multiple axis inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixMode {
    /// Weighted sum of all inputs, clamped to `[-1.0, 1.0]`.
    WeightedSum,
    /// Take the maximum value across all inputs.
    Max,
    /// Take the minimum value across all inputs.
    Min,
    /// Use the input with the highest configured weight (priority).
    Priority,
}

/// Zero-allocation axis mixer.
///
/// Combines up to [`MAX_MIXER_INPUTS`] axis values into a single output
/// using the configured [`MixMode`] and per-input weights.
#[derive(Debug, Clone, Copy)]
pub struct AxisMixer {
    mode: MixMode,
    weights: [f64; MAX_MIXER_INPUTS],
    count: usize,
}

impl AxisMixer {
    /// Creates a new mixer with the given mode and zero inputs.
    #[must_use]
    pub fn new(mode: MixMode) -> Self {
        Self {
            mode,
            weights: [1.0; MAX_MIXER_INPUTS],
            count: 0,
        }
    }

    /// Creates a mixer pre-configured with the given weights.
    ///
    /// At most [`MAX_MIXER_INPUTS`] weights are used.
    #[must_use]
    pub fn with_weights(mode: MixMode, weights: &[f64]) -> Self {
        let mut mixer = Self::new(mode);
        let n = weights.len().min(MAX_MIXER_INPUTS);
        for (i, &w) in weights.iter().take(n).enumerate() {
            mixer.weights[i] = w;
        }
        mixer.count = n;
        mixer
    }

    /// Adds an input with the given weight. Returns `true` on success.
    pub fn add_input(&mut self, weight: f64) -> bool {
        if self.count >= MAX_MIXER_INPUTS {
            return false;
        }
        self.weights[self.count] = weight;
        self.count += 1;
        true
    }

    /// Sets the weight for the input at `index`. Returns `true` on success.
    pub fn set_weight(&mut self, index: usize, weight: f64) -> bool {
        if index >= self.count {
            return false;
        }
        self.weights[index] = weight;
        true
    }

    /// Returns the number of configured inputs.
    #[must_use]
    pub fn input_count(&self) -> usize {
        self.count
    }

    /// Returns the configured mixing mode.
    #[must_use]
    pub fn mode(&self) -> MixMode {
        self.mode
    }

    /// Combine the given `values` into a single output.
    ///
    /// Only the first `min(values.len(), input_count())` values are used.
    /// Returns `0.0` when no inputs are available.
    #[inline]
    pub fn combine(&self, values: &[f64]) -> f64 {
        let n = values.len().min(self.count);
        if n == 0 {
            return 0.0;
        }
        match self.mode {
            MixMode::WeightedSum => {
                let mut sum = 0.0_f64;
                for (&v, &w) in values[..n].iter().zip(&self.weights[..n]) {
                    let safe_v = if v.is_finite() { v } else { 0.0 };
                    sum += safe_v * w;
                }
                sum.clamp(-1.0, 1.0)
            }
            MixMode::Max => {
                let mut max = f64::NEG_INFINITY;
                for &v in &values[..n] {
                    if v.is_finite() && v > max {
                        max = v;
                    }
                }
                if max.is_finite() { max } else { 0.0 }
            }
            MixMode::Min => {
                let mut min = f64::INFINITY;
                for &v in &values[..n] {
                    if v.is_finite() && v < min {
                        min = v;
                    }
                }
                if min.is_finite() { min } else { 0.0 }
            }
            MixMode::Priority => {
                let mut best_idx = 0;
                let mut best_weight = f64::NEG_INFINITY;
                for i in 0..n {
                    if self.weights[i] > best_weight {
                        best_weight = self.weights[i];
                        best_idx = i;
                    }
                }
                let v = values[best_idx];
                if v.is_finite() { v } else { 0.0 }
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-10;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < TOL
    }

    // === WeightedSum tests ===============================================

    #[test]
    fn weighted_sum_equal_weights() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[0.3, 0.2]), 0.5));
    }

    #[test]
    fn weighted_sum_different_weights() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[0.5, 0.5]);
        assert!(approx(mixer.combine(&[0.6, 0.4]), 0.5));
    }

    #[test]
    fn weighted_sum_clamps_to_bounds() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[0.8, 0.8]), 1.0));
        assert!(approx(mixer.combine(&[-0.8, -0.8]), -1.0));
    }

    #[test]
    fn weighted_sum_axis_plus_trim() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        let physical = 0.5;
        let trim = 0.05;
        assert!(approx(mixer.combine(&[physical, trim]), 0.55));
    }

    #[test]
    fn weighted_sum_zero_weights() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[0.0, 0.0]);
        assert!(approx(mixer.combine(&[0.8, -0.3]), 0.0));
    }

    // === Max tests =======================================================

    #[test]
    fn max_picks_largest() {
        let mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0, 1.0]);
        assert!(approx(mixer.combine(&[0.3, 0.8, 0.5]), 0.8));
    }

    #[test]
    fn max_with_negatives() {
        let mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[-0.3, -0.8]), -0.3));
    }

    // === Min tests =======================================================

    #[test]
    fn min_picks_smallest() {
        let mixer = AxisMixer::with_weights(MixMode::Min, &[1.0, 1.0, 1.0]);
        assert!(approx(mixer.combine(&[0.3, 0.8, 0.5]), 0.3));
    }

    #[test]
    fn min_with_negatives() {
        let mixer = AxisMixer::with_weights(MixMode::Min, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[-0.3, -0.8]), -0.8));
    }

    // === Priority tests ==================================================

    #[test]
    fn priority_picks_highest_weight_input() {
        let mixer = AxisMixer::with_weights(MixMode::Priority, &[1.0, 10.0, 5.0]);
        // Weight 10.0 is highest → pick values[1]
        assert!(approx(mixer.combine(&[0.1, 0.9, 0.5]), 0.9));
    }

    #[test]
    fn priority_equal_weights_picks_first() {
        let mixer = AxisMixer::with_weights(MixMode::Priority, &[1.0, 1.0]);
        // Equal weights → first has highest (or equal), picks values[0]
        // Actually first is found first in the loop
        // Let me check: loop finds first index with weight > best_weight
        // Starting at NEG_INFINITY, weights[0]=1.0 > NEG_INFINITY → best=0
        // weights[1]=1.0 is NOT > 1.0 → stays 0
        assert!(approx(mixer.combine(&[0.3, 0.7]), 0.3));
    }

    // === Empty / edge case tests =========================================

    #[test]
    fn empty_mixer_returns_zero() {
        let mixer = AxisMixer::new(MixMode::WeightedSum);
        assert_eq!(mixer.combine(&[]), 0.0);
        assert_eq!(mixer.combine(&[0.5]), 0.0); // count=0
    }

    #[test]
    fn more_values_than_inputs_only_uses_count() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0]);
        // Only 1 input configured, ignores second value
        assert!(approx(mixer.combine(&[0.3, 999.0]), 0.3));
    }

    #[test]
    fn fewer_values_than_inputs() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0, 1.0]);
        // Only 2 values provided for 3 inputs
        assert!(approx(mixer.combine(&[0.3, 0.2]), 0.5));
    }

    #[test]
    fn nan_treated_as_zero_in_weighted_sum() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[0.5, f64::NAN]), 0.5));
    }

    #[test]
    fn nan_skipped_in_max() {
        let mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[0.3, f64::NAN]), 0.3));
    }

    #[test]
    fn nan_skipped_in_min() {
        let mixer = AxisMixer::with_weights(MixMode::Min, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[-0.3, f64::NAN]), -0.3));
    }

    #[test]
    fn all_nan_returns_zero() {
        let mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0]);
        assert_eq!(mixer.combine(&[f64::NAN, f64::NAN]), 0.0);
    }

    #[test]
    fn inf_treated_as_zero_in_weighted_sum() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        assert!(approx(mixer.combine(&[0.5, f64::INFINITY]), 0.5));
    }

    #[test]
    fn priority_nan_input_returns_zero() {
        let mixer = AxisMixer::with_weights(MixMode::Priority, &[1.0, 10.0]);
        // Highest weight picks values[1] which is NaN → returns 0.0
        assert_eq!(mixer.combine(&[0.5, f64::NAN]), 0.0);
    }

    // === Mutation tests ===================================================

    #[test]
    fn add_input_increments_count() {
        let mut mixer = AxisMixer::new(MixMode::WeightedSum);
        assert_eq!(mixer.input_count(), 0);
        assert!(mixer.add_input(1.0));
        assert_eq!(mixer.input_count(), 1);
        assert!(mixer.add_input(0.5));
        assert_eq!(mixer.input_count(), 2);
    }

    #[test]
    fn add_input_beyond_max_fails() {
        let mut mixer = AxisMixer::new(MixMode::WeightedSum);
        for _ in 0..MAX_MIXER_INPUTS {
            assert!(mixer.add_input(1.0));
        }
        assert!(!mixer.add_input(1.0));
        assert_eq!(mixer.input_count(), MAX_MIXER_INPUTS);
    }

    #[test]
    fn set_weight_valid_index() {
        let mut mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        assert!(mixer.set_weight(1, 2.0));
        // Now weight[0]=1.0, weight[1]=2.0
        // 0.5*1.0 + 0.3*2.0 = 0.5+0.6 = 1.0 (clamped)
        assert!(approx(mixer.combine(&[0.5, 0.3]), 1.0));
    }

    #[test]
    fn set_weight_invalid_index() {
        let mut mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0]);
        assert!(!mixer.set_weight(5, 2.0));
    }

    #[test]
    fn mode_accessor() {
        let mixer = AxisMixer::new(MixMode::Priority);
        assert_eq!(mixer.mode(), MixMode::Priority);
    }

    // === Zero-allocation verification ====================================

    #[test]
    fn verify_mixer_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<AxisMixer>();
        assert_copy::<MixMode>();
    }

    #[test]
    fn verify_mixer_stack_size() {
        assert!(
            std::mem::size_of::<AxisMixer>() < 256,
            "AxisMixer too large: {}",
            std::mem::size_of::<AxisMixer>()
        );
    }
}
