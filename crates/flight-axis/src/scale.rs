// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis scaling.
//!
//! Multiplies the axis value by a `factor` and clamps the result to `[min, max]`.

/// Errors returned by [`AxisScale::new`].
#[derive(Debug, PartialEq)]
pub enum ScaleError {
    /// `min >= max`.
    InvalidRange,
    /// `factor` is NaN or infinite.
    InvalidFactor,
}

impl std::fmt::Display for ScaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScaleError::InvalidRange => write!(f, "min must be less than max"),
            ScaleError::InvalidFactor => write!(f, "factor must be finite"),
        }
    }
}

impl std::error::Error for ScaleError {}

/// Per-axis scale configuration.
///
/// Applies `(value * factor).clamp(min, max)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisScale {
    /// Multiplier applied before clamping.
    pub factor: f32,
    /// Output minimum (default `-1.0`).
    pub min: f32,
    /// Output maximum (default `1.0`).
    pub max: f32,
}

impl Default for AxisScale {
    fn default() -> Self {
        Self {
            factor: 1.0,
            min: -1.0,
            max: 1.0,
        }
    }
}

impl AxisScale {
    /// Creates a new `AxisScale`.
    ///
    /// # Errors
    ///
    /// Returns [`ScaleError::InvalidFactor`] if `factor` is NaN or infinite.
    /// Returns [`ScaleError::InvalidRange`] if `min >= max`.
    pub fn new(factor: f32, min: f32, max: f32) -> Result<Self, ScaleError> {
        if !factor.is_finite() {
            return Err(ScaleError::InvalidFactor);
        }
        if min >= max {
            return Err(ScaleError::InvalidRange);
        }
        Ok(Self { factor, min, max })
    }

    /// Scales `value` by `factor` and clamps the result to `[min, max]`.
    #[inline]
    pub fn apply(&self, value: f32) -> f32 {
        (value * self.factor).clamp(self.min, self.max)
    }
}

/// Index-based bank of axis scalers.
pub struct ScaleBank {
    scalers: Vec<AxisScale>,
}

impl ScaleBank {
    /// Creates a new `ScaleBank` with `count` axes, all using default scaling
    /// (`factor = 1.0`, `min = -1.0`, `max = 1.0`).
    pub fn new(count: usize) -> Self {
        Self {
            scalers: vec![AxisScale::default(); count],
        }
    }

    /// Applies the scaler at `axis_index` to `value`.
    ///
    /// Returns `value` unchanged (no panic) if `axis_index` is out of bounds.
    #[inline]
    pub fn apply(&self, axis_index: usize, value: f32) -> f32 {
        match self.scalers.get(axis_index) {
            Some(s) => s.apply(value),
            None => value,
        }
    }

    /// Replaces the scaler at `axis_index` with `scale`.
    ///
    /// Does nothing if `axis_index` is out of bounds.
    pub fn set_scale(&mut self, axis_index: usize, scale: AxisScale) {
        if let Some(s) = self.scalers.get_mut(axis_index) {
            *s = scale;
        }
    }

    /// Returns the number of axes in the bank.
    pub fn len(&self) -> usize {
        self.scalers.len()
    }

    /// Returns `true` if the bank contains no axes.
    pub fn is_empty(&self) -> bool {
        self.scalers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── AxisScale unit tests ──────────────────────────────────────────────────

    #[test]
    fn test_scale_default_passthrough() {
        let s = AxisScale::default();
        assert_eq!(s.apply(0.5), 0.5);
        assert_eq!(s.apply(-0.5), -0.5);
        assert_eq!(s.apply(0.0), 0.0);
    }

    #[test]
    fn test_scale_factor_half() {
        let s = AxisScale::new(0.5, -1.0, 1.0).unwrap();
        assert!((s.apply(0.8) - 0.4).abs() < 1e-6);
        assert!((s.apply(-0.8) - (-0.4)).abs() < 1e-6);
    }

    #[test]
    fn test_scale_clamp_max() {
        let s = AxisScale::new(2.0, -1.0, 1.0).unwrap();
        assert_eq!(s.apply(0.8), 1.0);
    }

    #[test]
    fn test_scale_clamp_min() {
        let s = AxisScale::new(2.0, -1.0, 1.0).unwrap();
        assert_eq!(s.apply(-0.8), -1.0);
    }

    #[test]
    fn test_scale_invalid_range_error() {
        assert_eq!(
            AxisScale::new(1.0, 1.0, -1.0),
            Err(ScaleError::InvalidRange)
        );
        assert_eq!(AxisScale::new(1.0, 0.5, 0.5), Err(ScaleError::InvalidRange));
    }

    #[test]
    fn test_scale_nan_factor_error() {
        assert_eq!(
            AxisScale::new(f32::NAN, -1.0, 1.0),
            Err(ScaleError::InvalidFactor)
        );
        assert_eq!(
            AxisScale::new(f32::INFINITY, -1.0, 1.0),
            Err(ScaleError::InvalidFactor)
        );
        assert_eq!(
            AxisScale::new(f32::NEG_INFINITY, -1.0, 1.0),
            Err(ScaleError::InvalidFactor)
        );
    }

    #[test]
    fn test_scale_negative_factor() {
        let s = AxisScale::new(-1.0, -1.0, 1.0).unwrap();
        assert_eq!(s.apply(0.5), -0.5);
        assert_eq!(s.apply(-0.5), 0.5);
    }

    #[test]
    fn test_scale_zero_factor() {
        let s = AxisScale::new(0.0, -1.0, 1.0).unwrap();
        assert_eq!(s.apply(0.9), 0.0);
        assert_eq!(s.apply(-0.9), 0.0);
    }

    // ── ScaleBank unit tests ──────────────────────────────────────────────────

    #[test]
    fn test_bank_apply() {
        let mut bank = ScaleBank::new(3);
        bank.set_scale(1, AxisScale::new(2.0, -1.0, 1.0).unwrap());
        assert!((bank.apply(1, 0.4) - 0.8).abs() < 1e-6);
        assert_eq!(bank.apply(0, 0.5), 0.5);
        // out of bounds → passthrough
        assert_eq!(bank.apply(99, 0.42), 0.42);
    }

    #[test]
    fn test_bank_set_scale() {
        let mut bank = ScaleBank::new(2);
        let s = AxisScale::new(0.5, -0.5, 0.5).unwrap();
        bank.set_scale(0, s);
        assert!((bank.apply(0, 1.0) - 0.5).abs() < 1e-6);
        // set_scale out of bounds: no panic
        bank.set_scale(99, AxisScale::default());
    }

    // ── Proptests ────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_output_always_in_range(
            factor in -10.0f32..=10.0f32,
            min in -10.0f32..=-0.01f32,
            max in 0.01f32..=10.0f32,
            value in -10.0f32..=10.0f32,
        ) {
            // min < 0 and max > 0 so min < max is always satisfied
            let s = AxisScale::new(factor, min, max).unwrap();
            let out = s.apply(value);
            prop_assert!(out >= min && out <= max, "out={out} not in [{min}, {max}]");
        }
    }
}
