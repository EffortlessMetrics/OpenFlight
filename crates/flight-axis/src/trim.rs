// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis trim offset system.
//!
//! Trim is applied as an additive offset AFTER deadzone/expo processing
//! but BEFORE final output clamping.

use std::collections::HashMap;

/// Per-axis trim offset.
///
/// Stores a persistent additive offset that shifts the axis output within
/// `[-1.0, 1.0]`. The offset itself is bounded by `±max_range`.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTrim {
    /// Current trim offset, clamped to `[-max_range, max_range]`.
    offset: f32,
    /// Maximum absolute trim range (default `0.3`).
    max_range: f32,
    /// Increment/decrement step size (default `0.01`).
    step: f32,
}

impl Default for AxisTrim {
    fn default() -> Self {
        Self {
            offset: 0.0,
            max_range: 0.3,
            step: 0.01,
        }
    }
}

impl AxisTrim {
    /// Creates a new `AxisTrim` with the given `max_range` and `step`, offset initialised to `0.0`.
    pub fn new(max_range: f32, step: f32) -> Self {
        Self {
            offset: 0.0,
            max_range,
            step,
        }
    }

    /// Returns the current trim offset.
    #[inline]
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Applies the trim offset to `input` and clamps the result to `[-1.0, 1.0]`.
    #[inline]
    pub fn apply(&self, input: f32) -> f32 {
        (input + self.offset).clamp(-1.0, 1.0)
    }

    /// Increases the trim offset by one step, clamped to `max_range`.
    #[inline]
    pub fn increment(&mut self) {
        self.offset = (self.offset + self.step).min(self.max_range);
    }

    /// Decreases the trim offset by one step, clamped to `-max_range`.
    #[inline]
    pub fn decrement(&mut self) {
        self.offset = (self.offset - self.step).max(-self.max_range);
    }

    /// Resets the trim offset to `0.0`.
    #[inline]
    pub fn reset(&mut self) {
        self.offset = 0.0;
    }

    /// Sets the trim offset directly, clamped to `±max_range`.
    #[inline]
    pub fn set_offset(&mut self, offset: f32) {
        self.offset = offset.clamp(-self.max_range, self.max_range);
    }
}

/// Manages trim offsets for a collection of named axes.
///
/// Not intended for use on the RT hot path due to `HashMap` heap allocation.
#[derive(Debug, Clone, Default)]
pub struct AxisTrimBank {
    trims: HashMap<String, AxisTrim>,
}

impl AxisTrimBank {
    /// Creates a new, empty `AxisTrimBank`.
    pub fn new() -> Self {
        Self {
            trims: HashMap::new(),
        }
    }

    /// Returns a reference to the `AxisTrim` for the given axis, if it exists.
    pub fn get(&self, axis: &str) -> Option<&AxisTrim> {
        self.trims.get(axis)
    }

    /// Returns a mutable reference to the `AxisTrim` for the given axis, if it exists.
    pub fn get_mut(&mut self, axis: &str) -> Option<&mut AxisTrim> {
        self.trims.get_mut(axis)
    }

    /// Returns a mutable reference to the `AxisTrim` for the given axis, inserting
    /// a default `AxisTrim` if it does not yet exist.
    pub fn get_or_insert(&mut self, axis: &str) -> &mut AxisTrim {
        self.trims.entry(axis.to_string()).or_default()
    }

    /// Resets all axis trims to `0.0`.
    pub fn reset_all(&mut self) {
        for trim in self.trims.values_mut() {
            trim.reset();
        }
    }

    /// Applies the trim for the named axis to `input`.
    ///
    /// Returns `input` unchanged if the axis is not present in the bank.
    #[inline]
    pub fn apply(&self, axis: &str, input: f32) -> f32 {
        match self.trims.get(axis) {
            Some(trim) => trim.apply(input),
            None => input,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── AxisTrim unit tests ───────────────────────────────────────────────────

    #[test]
    fn test_trim_default_values() {
        let t = AxisTrim::default();
        assert_eq!(t.max_range, 0.3);
        assert_eq!(t.step, 0.01);
        assert_eq!(t.offset(), 0.0);
    }

    #[test]
    fn test_trim_apply_zero() {
        let t = AxisTrim::default();
        assert_eq!(t.apply(0.5), 0.5);
    }

    #[test]
    fn test_trim_apply_positive_offset() {
        let mut t = AxisTrim::default();
        t.set_offset(0.1);
        assert!((t.apply(0.5) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn test_trim_apply_negative_offset() {
        let mut t = AxisTrim::default();
        t.set_offset(-0.1);
        assert!((t.apply(0.5) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn test_trim_apply_clamps_to_1() {
        let mut t = AxisTrim::default();
        t.set_offset(0.3);
        assert_eq!(t.apply(0.8), 1.0);
    }

    #[test]
    fn test_trim_apply_clamps_to_neg1() {
        let mut t = AxisTrim::default();
        t.set_offset(-0.3);
        assert_eq!(t.apply(-0.8), -1.0);
    }

    #[test]
    fn test_trim_increment() {
        let mut t = AxisTrim::default();
        t.increment();
        t.increment();
        t.increment();
        assert!((t.offset() - 3.0 * t.step).abs() < 1e-6);
    }

    #[test]
    fn test_trim_decrement() {
        let mut t = AxisTrim::default();
        t.decrement();
        t.decrement();
        t.decrement();
        assert!((t.offset() - (-3.0 * t.step)).abs() < 1e-6);
    }

    #[test]
    fn test_trim_increment_clamps_to_max_range() {
        let mut t = AxisTrim::default();
        for _ in 0..100 {
            t.increment();
        }
        assert!((t.offset() - t.max_range).abs() < 1e-6);
    }

    #[test]
    fn test_trim_decrement_clamps_to_neg_max_range() {
        let mut t = AxisTrim::default();
        for _ in 0..100 {
            t.decrement();
        }
        assert!((t.offset() - (-t.max_range)).abs() < 1e-6);
    }

    #[test]
    fn test_trim_reset() {
        let mut t = AxisTrim::default();
        t.set_offset(0.2);
        assert_ne!(t.offset(), 0.0);
        t.reset();
        assert_eq!(t.offset(), 0.0);
    }

    #[test]
    fn test_trim_set_offset_clamped() {
        let mut t = AxisTrim::default();
        t.set_offset(99.9);
        assert_eq!(t.offset(), t.max_range);
    }

    // ── AxisTrimBank unit tests ───────────────────────────────────────────────

    #[test]
    fn test_trimbank_apply_unknown_axis() {
        let bank = AxisTrimBank::new();
        assert_eq!(bank.apply("pitch", 0.75), 0.75);
    }

    #[test]
    fn test_trimbank_get_or_insert() {
        let mut bank = AxisTrimBank::new();
        assert!(bank.get("roll").is_none());
        let t = bank.get_or_insert("roll");
        assert_eq!(t.offset(), 0.0);
        assert!(bank.get("roll").is_some());
    }

    #[test]
    fn test_trimbank_reset_all() {
        let mut bank = AxisTrimBank::new();
        bank.get_or_insert("pitch").set_offset(0.2);
        bank.get_or_insert("roll").set_offset(-0.15);
        bank.reset_all();
        assert_eq!(bank.get("pitch").unwrap().offset(), 0.0);
        assert_eq!(bank.get("roll").unwrap().offset(), 0.0);
    }

    // ── Proptests ────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_apply_output_bounded(
            offset in -0.5f32..=0.5,
            input in -1.0f32..=1.0,
        ) {
            let mut t = AxisTrim::new(1.0, 0.01);
            t.set_offset(offset);
            let out = t.apply(input);
            prop_assert!((-1.0..=1.0).contains(&out), "out={out}");
        }

        #[test]
        fn proptest_increment_bounded(max_range in 0.01f32..=1.0, step in 0.001f32..=0.1) {
            let mut t = AxisTrim::new(max_range, step);
            for _ in 0..1000 {
                t.increment();
            }
            prop_assert!(t.offset() <= max_range + 1e-6, "offset={} max_range={}", t.offset(), max_range);
        }

        #[test]
        fn proptest_decrement_bounded(max_range in 0.01f32..=1.0, step in 0.001f32..=0.1) {
            let mut t = AxisTrim::new(max_range, step);
            for _ in 0..1000 {
                t.decrement();
            }
            prop_assert!(t.offset() >= -max_range - 1e-6, "offset={} max_range={}", t.offset(), max_range);
        }
    }
}
