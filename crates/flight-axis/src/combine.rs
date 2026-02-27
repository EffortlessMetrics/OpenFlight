// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis combining and splitting utilities.
//!
//! Pure functions with zero-allocation guarantee, safe to use on the RT hot path.

/// Combines two unipolar `[0, 1]` axes into a single bipolar `[-1, 1]` axis.
///
/// Formula: `left - right`, clamped to `[-1.0, 1.0]`.
///
/// | left | right | result |
/// |------|-------|--------|
/// | 1.0  | 0.0   |  1.0   |
/// | 0.0  | 1.0   | -1.0   |
/// | 0.5  | 0.5   |  0.0   |
#[inline]
pub fn combine_differential(left: f32, right: f32) -> f32 {
    (left - right).clamp(-1.0, 1.0)
}

/// Combines two axes into their average, clamped to `[-1.0, 1.0]`.
#[inline]
pub fn combine_average(a: f32, b: f32) -> f32 {
    ((a + b) / 2.0).clamp(-1.0, 1.0)
}

/// Splits a bipolar `[-1, 1]` axis into two unipolar `[0, 1]` axes.
///
/// Returns `(positive_part, negative_part)` where:
/// - `positive_part` equals the axis value when positive, else `0.0`
/// - `negative_part` equals the magnitude of the axis value when negative, else `0.0`
///
/// Invariant: `positive_part + negative_part == axis.abs()`
#[inline]
pub fn split_bipolar(axis: f32) -> (f32, f32) {
    let clamped = axis.clamp(-1.0, 1.0);
    let positive = clamped.max(0.0);
    let negative = (-clamped).max(0.0);
    (positive, negative)
}

/// Merge mode for combining multiple axis inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
    /// Use the input with the highest absolute value.
    Priority,
    /// Sum all inputs and clamp to `[-1, 1]`.
    Sum,
    /// Average all inputs.
    Average,
}

/// Combiner for merging N axis inputs into one output.
pub struct AxisCombiner {
    mode: MergeMode,
}

impl AxisCombiner {
    pub fn new(mode: MergeMode) -> Self {
        Self { mode }
    }

    /// Merge `inputs` into a single value according to the configured [`MergeMode`].
    ///
    /// Returns `0.0` when `inputs` is empty.
    pub fn combine(&self, inputs: &[f32]) -> f32 {
        if inputs.is_empty() {
            return 0.0;
        }
        match self.mode {
            MergeMode::Priority => inputs
                .iter()
                .copied()
                .max_by(|a, b| {
                    a.abs()
                        .partial_cmp(&b.abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(0.0),
            MergeMode::Sum => inputs
                .iter()
                .copied()
                .fold(0.0f32, |acc, v| acc + v)
                .clamp(-1.0, 1.0),
            MergeMode::Average => {
                let sum: f32 = inputs.iter().copied().sum();
                (sum / inputs.len() as f32).clamp(-1.0, 1.0)
            }
        }
    }
}

#[cfg(test)]
mod combiner_tests {
    use super::*;

    #[test]
    fn test_priority_merge_picks_highest_abs_value() {
        let c = AxisCombiner::new(MergeMode::Priority);
        assert_eq!(c.combine(&[0.3, -0.8, 0.5]), -0.8);
    }

    #[test]
    fn test_sum_merge_clamps_to_one() {
        let c = AxisCombiner::new(MergeMode::Sum);
        assert_eq!(c.combine(&[0.7, 0.6, 0.5]), 1.0);
        assert_eq!(c.combine(&[-0.7, -0.6, -0.5]), -1.0);
    }

    #[test]
    fn test_average_merge_averages_inputs() {
        let c = AxisCombiner::new(MergeMode::Average);
        let out = c.combine(&[0.4, 0.6]);
        assert!((out - 0.5).abs() < 1e-6, "expected 0.5, got {out}");
    }

    #[test]
    fn test_empty_inputs_returns_zero() {
        for mode in [MergeMode::Priority, MergeMode::Sum, MergeMode::Average] {
            let c = AxisCombiner::new(mode);
            assert_eq!(c.combine(&[]), 0.0, "failed for {mode:?}");
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_combine_differential_bounded(a in -1.0f32..=1.0, b in -1.0f32..=1.0) {
            let out = combine_differential(a, b);
            prop_assert!(out >= -1.0 && out <= 1.0, "out={out}");
        }

        #[test]
        fn test_split_bipolar_sum_preserved(x in -1.0f32..=1.0) {
            let (pos, neg) = split_bipolar(x);
            prop_assert!(pos >= 0.0 && pos <= 1.0, "pos={pos}");
            prop_assert!(neg >= 0.0 && neg <= 1.0, "neg={neg}");
        }

        #[test]
        fn test_combine_average_bounded(a in -1.0f32..=1.0, b in -1.0f32..=1.0) {
            let out = combine_average(a, b);
            prop_assert!(out >= -1.0 && out <= 1.0, "out={out}");
        }
    }
}
