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
