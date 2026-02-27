// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis inversion.
//!
//! When enabled, multiplies the axis output by `-1.0`.
//! Typical position in the pipeline: after deadzone, before curve.

/// Per-axis inversion configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisInvert {
    /// When `true`, the axis value is negated.
    pub enabled: bool,
}

impl Default for AxisInvert {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl AxisInvert {
    /// Creates a new `AxisInvert` with the given `enabled` state.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Applies inversion to `value`.
    ///
    /// Returns `-value` when enabled, or `value` unchanged when disabled.
    #[inline]
    pub fn apply(&self, value: f32) -> f32 {
        if self.enabled { -value } else { value }
    }

    /// Toggles the enabled state.
    #[inline]
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

/// Index-based bank of axis invertors.
pub struct InvertBank {
    invertors: Vec<AxisInvert>,
}

impl InvertBank {
    /// Creates a new `InvertBank` with `count` axes, all non-inverted.
    pub fn new(count: usize) -> Self {
        Self {
            invertors: vec![AxisInvert::default(); count],
        }
    }

    /// Applies the invertor at `axis_index` to `value`.
    ///
    /// Returns `value` unchanged (no panic) if `axis_index` is out of bounds.
    #[inline]
    pub fn apply(&self, axis_index: usize, value: f32) -> f32 {
        match self.invertors.get(axis_index) {
            Some(inv) => inv.apply(value),
            None => value,
        }
    }

    /// Sets the inversion state for the axis at `axis_index`.
    ///
    /// Does nothing if `axis_index` is out of bounds.
    pub fn set_inverted(&mut self, axis_index: usize, enabled: bool) {
        if let Some(inv) = self.invertors.get_mut(axis_index) {
            inv.enabled = enabled;
        }
    }

    /// Toggles the inversion state for the axis at `axis_index`.
    ///
    /// Does nothing if `axis_index` is out of bounds.
    pub fn toggle(&mut self, axis_index: usize) {
        if let Some(inv) = self.invertors.get_mut(axis_index) {
            inv.toggle();
        }
    }

    /// Returns `true` if the axis at `axis_index` is inverted.
    ///
    /// Returns `false` if `axis_index` is out of bounds.
    pub fn is_inverted(&self, axis_index: usize) -> bool {
        self.invertors
            .get(axis_index)
            .map_or(false, |inv| inv.enabled)
    }

    /// Returns the number of axes in the bank.
    pub fn len(&self) -> usize {
        self.invertors.len()
    }

    /// Returns `true` if the bank contains no axes.
    pub fn is_empty(&self) -> bool {
        self.invertors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── AxisInvert unit tests ─────────────────────────────────────────────────

    #[test]
    fn test_invert_disabled_passthrough() {
        let inv = AxisInvert::new(false);
        assert_eq!(inv.apply(0.5), 0.5);
        assert_eq!(inv.apply(-0.7), -0.7);
    }

    #[test]
    fn test_invert_enabled_negates() {
        let inv = AxisInvert::new(true);
        assert_eq!(inv.apply(0.5), -0.5);
        assert_eq!(inv.apply(-0.7), 0.7);
    }

    #[test]
    fn test_invert_zero_unchanged() {
        let inv_on = AxisInvert::new(true);
        let inv_off = AxisInvert::new(false);
        assert_eq!(inv_on.apply(0.0), 0.0);
        assert_eq!(inv_off.apply(0.0), 0.0);
    }

    #[test]
    fn test_invert_toggle() {
        let mut inv = AxisInvert::new(false);
        assert!(!inv.enabled);
        inv.toggle();
        assert!(inv.enabled);
        inv.toggle();
        assert!(!inv.enabled);
    }

    #[test]
    fn test_invert_positive_one() {
        let inv = AxisInvert::new(true);
        assert_eq!(inv.apply(1.0), -1.0);
    }

    #[test]
    fn test_invert_negative_one() {
        let inv = AxisInvert::new(true);
        assert_eq!(inv.apply(-1.0), 1.0);
    }

    // ── InvertBank unit tests ─────────────────────────────────────────────────

    #[test]
    fn test_bank_apply() {
        let mut bank = InvertBank::new(3);
        bank.set_inverted(1, true);
        assert_eq!(bank.apply(0, 0.5), 0.5);
        assert_eq!(bank.apply(1, 0.5), -0.5);
        assert_eq!(bank.apply(2, 0.5), 0.5);
    }

    #[test]
    fn test_bank_set_inverted() {
        let mut bank = InvertBank::new(2);
        bank.set_inverted(0, true);
        assert!(bank.is_inverted(0));
        bank.set_inverted(0, false);
        assert!(!bank.is_inverted(0));
    }

    #[test]
    fn test_bank_toggle() {
        let mut bank = InvertBank::new(2);
        assert!(!bank.is_inverted(0));
        bank.toggle(0);
        assert!(bank.is_inverted(0));
        bank.toggle(0);
        assert!(!bank.is_inverted(0));
    }

    #[test]
    fn test_bank_bounds_check() {
        let mut bank = InvertBank::new(2);
        // Out of bounds: no panic, value returned unchanged
        assert_eq!(bank.apply(99, 0.42), 0.42);
        bank.set_inverted(99, true); // no panic
        bank.toggle(99); // no panic
        assert!(!bank.is_inverted(99)); // returns false
    }

    // ── Proptests ────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_double_invert_identity(x in -1.0f32..=1.0f32) {
            let inv = AxisInvert::new(true);
            prop_assert_eq!(inv.apply(inv.apply(x)), x);
        }
    }
}
