// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis mode switching (REQ-819).
//!
//! Provides three sensitivity modes — Normal, Precision, and Fast — each with
//! a configurable scale factor. The active mode is selected at runtime and
//! applied to axis values via [`ModeSwitcher::apply`].
//!
//! RT-safe: no heap allocation.

/// Sensitivity mode for an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisMode {
    /// Default 1:1 sensitivity.
    Normal,
    /// Reduced sensitivity (default 0.5×).
    Precision,
    /// Increased sensitivity (default 2.0×).
    Fast,
}

/// Per-mode scale factors.
///
/// RT-safe: no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeSwitcher {
    /// Currently active mode.
    mode: AxisMode,
    /// Scale factor for [`AxisMode::Normal`].
    normal_factor: f64,
    /// Scale factor for [`AxisMode::Precision`].
    precision_factor: f64,
    /// Scale factor for [`AxisMode::Fast`].
    fast_factor: f64,
}

impl Default for ModeSwitcher {
    fn default() -> Self {
        Self {
            mode: AxisMode::Normal,
            normal_factor: 1.0,
            precision_factor: 0.5,
            fast_factor: 2.0,
        }
    }
}

impl ModeSwitcher {
    /// Creates a new `ModeSwitcher` with custom scale factors.
    ///
    /// RT-safe: no heap allocation.
    #[must_use]
    pub const fn new(normal: f64, precision: f64, fast: f64) -> Self {
        Self {
            mode: AxisMode::Normal,
            normal_factor: normal,
            precision_factor: precision,
            fast_factor: fast,
        }
    }

    /// Returns the currently active mode.
    #[must_use]
    pub const fn mode(&self) -> AxisMode {
        self.mode
    }

    /// Switches to the given mode.
    pub fn set_mode(&mut self, mode: AxisMode) {
        self.mode = mode;
    }

    /// Returns the scale factor for the currently active mode.
    #[must_use]
    pub fn current_factor(&self) -> f64 {
        match self.mode {
            AxisMode::Normal => self.normal_factor,
            AxisMode::Precision => self.precision_factor,
            AxisMode::Fast => self.fast_factor,
        }
    }

    /// Scales `value` by the current mode's factor.
    ///
    /// RT-safe: no heap allocation.
    #[inline]
    #[must_use]
    pub fn apply(&self, value: f64) -> f64 {
        value * self.current_factor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_normal_passthrough() {
        let ms = ModeSwitcher::default();
        assert_eq!(ms.mode(), AxisMode::Normal);
        assert!((ms.apply(0.5) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn precision_halves_input() {
        let mut ms = ModeSwitcher::default();
        ms.set_mode(AxisMode::Precision);
        assert!((ms.apply(0.8) - 0.4).abs() < 1e-12);
        assert!((ms.apply(-0.6) - (-0.3)).abs() < 1e-12);
    }

    #[test]
    fn fast_doubles_input() {
        let mut ms = ModeSwitcher::default();
        ms.set_mode(AxisMode::Fast);
        assert!((ms.apply(0.3) - 0.6).abs() < 1e-12);
    }

    #[test]
    fn custom_factors() {
        let mut ms = ModeSwitcher::new(1.0, 0.25, 4.0);
        ms.set_mode(AxisMode::Precision);
        assert!((ms.apply(1.0) - 0.25).abs() < 1e-12);
        ms.set_mode(AxisMode::Fast);
        assert!((ms.apply(0.5) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn zero_input_always_zero() {
        let mut ms = ModeSwitcher::default();
        for mode in [AxisMode::Normal, AxisMode::Precision, AxisMode::Fast] {
            ms.set_mode(mode);
            assert_eq!(ms.apply(0.0), 0.0);
        }
    }

    #[test]
    fn negative_input_scales_correctly() {
        let mut ms = ModeSwitcher::default();
        ms.set_mode(AxisMode::Fast);
        assert!((ms.apply(-0.5) - (-1.0)).abs() < 1e-12);
    }

    #[test]
    fn mode_round_trip() {
        let mut ms = ModeSwitcher::default();
        ms.set_mode(AxisMode::Fast);
        assert_eq!(ms.mode(), AxisMode::Fast);
        ms.set_mode(AxisMode::Normal);
        assert_eq!(ms.mode(), AxisMode::Normal);
    }
}
