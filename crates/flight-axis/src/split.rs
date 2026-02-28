// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis sign-splitting: split a bipolar axis into two unipolar axes.
//!
//! Useful for e.g. splitting a combined brake axis ([-1, 1]) into
//! left brake ([0, 1]) and right brake ([0, 1]).

/// Configuration for sign-split output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitConfig {
    /// If true, the positive half is inverted (1.0 → 0.0, 0.0 → 1.0).
    pub invert_positive: bool,
    /// If true, the negative half is inverted.
    pub invert_negative: bool,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self { invert_positive: false, invert_negative: false }
    }
}

/// Output of an axis split operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitOutput {
    /// Positive half: maps [0, 1] from input [center, max].
    pub positive: f32,
    /// Negative half: maps [0, 1] from input [center, min].
    pub negative: f32,
}

/// Split a bipolar axis value into positive and negative halves.
pub struct AxisSplitter {
    config: SplitConfig,
}

impl AxisSplitter {
    pub fn new(config: SplitConfig) -> Self {
        Self { config }
    }

    /// Split input in `[-1.0, 1.0]` into positive and negative halves.
    pub fn split(&self, input: f32) -> SplitOutput {
        let clamped = input.clamp(-1.0, 1.0);
        let positive = if clamped >= 0.0 { clamped } else { 0.0 };
        let negative = if clamped <= 0.0 { -clamped } else { 0.0 };

        let positive = if self.config.invert_positive { 1.0 - positive } else { positive };
        let negative = if self.config.invert_negative { 1.0 - negative } else { negative };

        SplitOutput { positive, negative }
    }

    /// Combine positive and negative halves back to bipolar.
    ///
    /// `positive - negative` (both in `[0, 1]`) → `[-1, 1]`.
    pub fn combine(&self, positive: f32, negative: f32) -> f32 {
        let p = if self.config.invert_positive { 1.0 - positive } else { positive };
        let n = if self.config.invert_negative { 1.0 - negative } else { negative };
        (p - n).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn splitter() -> AxisSplitter {
        AxisSplitter::new(SplitConfig::default())
    }

    #[test]
    fn test_positive_input_produces_correct_positive_half() {
        let out = splitter().split(0.5);
        assert_eq!(out.positive, 0.5);
        assert_eq!(out.negative, 0.0);
    }

    #[test]
    fn test_negative_input_produces_correct_negative_half() {
        let out = splitter().split(-0.5);
        assert_eq!(out.positive, 0.0);
        assert_eq!(out.negative, 0.5);
    }

    #[test]
    fn test_zero_input_produces_both_zero() {
        let out = splitter().split(0.0);
        assert_eq!(out.positive, 0.0);
        assert_eq!(out.negative, 0.0);
    }

    #[test]
    fn test_split_combine_roundtrip() {
        let s = splitter();
        for &v in &[-1.0f32, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0] {
            let out = s.split(v);
            let back = s.combine(out.positive, out.negative);
            assert!((back - v).abs() < 1e-6, "roundtrip failed for {v}: got {back}");
        }
    }

    #[test]
    fn test_invert_positive_flips_positive_half() {
        let s = AxisSplitter::new(SplitConfig { invert_positive: true, invert_negative: false });
        let out = s.split(1.0);
        assert_eq!(out.positive, 0.0);
        assert_eq!(out.negative, 0.0);

        let out = s.split(0.0);
        assert_eq!(out.positive, 1.0);
    }

    #[test]
    fn test_invert_negative_flips_negative_half() {
        let s = AxisSplitter::new(SplitConfig { invert_positive: false, invert_negative: true });
        let out = s.split(-1.0);
        assert_eq!(out.negative, 0.0);
        assert_eq!(out.positive, 0.0);

        let out = s.split(0.0);
        assert_eq!(out.negative, 1.0);
    }

    #[test]
    fn test_max_input_full_positive() {
        let out = splitter().split(1.0);
        assert_eq!(out.positive, 1.0);
        assert_eq!(out.negative, 0.0);
    }

    #[test]
    fn test_min_input_full_negative() {
        let out = splitter().split(-1.0);
        assert_eq!(out.positive, 0.0);
        assert_eq!(out.negative, 1.0);
    }

    #[test]
    fn test_out_of_range_input_is_clamped() {
        let out = splitter().split(2.0);
        assert_eq!(out.positive, 1.0);
        assert_eq!(out.negative, 0.0);

        let out = splitter().split(-3.0);
        assert_eq!(out.positive, 0.0);
        assert_eq!(out.negative, 1.0);
    }

    #[test]
    fn test_property_positive_half_always_in_0_1() {
        let s = splitter();
        for i in -100i32..=100 {
            let v = i as f32 / 50.0;
            let out = s.split(v);
            assert!(
                (0.0..=1.0).contains(&out.positive),
                "positive={} out of range for input={v}",
                out.positive
            );
        }
    }

    #[test]
    fn test_property_negative_half_always_in_0_1() {
        let s = splitter();
        for i in -100i32..=100 {
            let v = i as f32 / 50.0;
            let out = s.split(v);
            assert!(
                (0.0..=1.0).contains(&out.negative),
                "negative={} out of range for input={v}",
                out.negative
            );
        }
    }
}
