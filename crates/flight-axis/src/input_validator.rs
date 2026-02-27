// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis input validation (REQ-708).
//!
//! Sanitises raw axis input before processing:
//! - NaN is replaced with the last valid value (or `0.0` initially).
//! - Infinite values are clamped to `±1.0`.
//!
//! Violation counters are maintained for diagnostics.
//!
//! Zero-allocation. RT-safe (ADR-004).

/// Validates and sanitises axis input values.
///
/// # Real-time safety
/// - Zero allocations on the hot path.
/// - No locks or blocking operations.
#[derive(Debug, Clone, Copy)]
pub struct InputValidator {
    last_valid: f32,
    nan_count: u64,
    inf_count: u64,
}

impl InputValidator {
    /// Creates a new `InputValidator` with an initial valid value of `0.0`.
    pub const fn new() -> Self {
        Self {
            last_valid: 0.0,
            nan_count: 0,
            inf_count: 0,
        }
    }

    /// Validates an input value.
    ///
    /// - NaN → replaced with last valid value.
    /// - `+Inf` → clamped to `1.0`.
    /// - `-Inf` → clamped to `-1.0`.
    /// - Normal values pass through unchanged.
    ///
    /// Zero-allocation — safe to call from RT code.
    #[inline]
    pub fn update(&mut self, value: f32) -> f32 {
        if value.is_nan() {
            self.nan_count += 1;
            return self.last_valid;
        }
        if value.is_infinite() {
            self.inf_count += 1;
            let clamped = if value > 0.0 { 1.0 } else { -1.0 };
            self.last_valid = clamped;
            return clamped;
        }
        self.last_valid = value;
        value
    }

    /// Returns the number of NaN values replaced.
    #[inline]
    pub fn nan_count(&self) -> u64 {
        self.nan_count
    }

    /// Returns the number of infinite values clamped.
    #[inline]
    pub fn inf_count(&self) -> u64 {
        self.inf_count
    }

    /// Returns the last valid output value.
    #[inline]
    pub fn last_valid(&self) -> f32 {
        self.last_valid
    }

    /// Resets the validator state and counters.
    #[inline]
    pub fn reset(&mut self) {
        self.last_valid = 0.0;
        self.nan_count = 0;
        self.inf_count = 0;
    }
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_replaced_with_last_valid() {
        let mut v = InputValidator::new();
        v.update(0.5);
        let out = v.update(f32::NAN);
        assert_eq!(out, 0.5);
        assert_eq!(v.nan_count(), 1);
    }

    #[test]
    fn inf_clamped_to_bounds() {
        let mut v = InputValidator::new();
        assert_eq!(v.update(f32::INFINITY), 1.0);
        assert_eq!(v.update(f32::NEG_INFINITY), -1.0);
        assert_eq!(v.inf_count(), 2);
    }

    #[test]
    fn normal_values_pass_through() {
        let mut v = InputValidator::new();
        assert_eq!(v.update(0.0), 0.0);
        assert_eq!(v.update(0.75), 0.75);
        assert_eq!(v.update(-0.33), -0.33);
        assert_eq!(v.nan_count(), 0);
        assert_eq!(v.inf_count(), 0);
    }

    #[test]
    fn counters_increment_correctly() {
        let mut v = InputValidator::new();
        v.update(f32::NAN);
        v.update(f32::NAN);
        v.update(f32::INFINITY);
        assert_eq!(v.nan_count(), 2);
        assert_eq!(v.inf_count(), 1);
    }

    #[test]
    fn nan_with_no_prior_valid_returns_zero() {
        let mut v = InputValidator::new();
        let out = v.update(f32::NAN);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn reset_clears_state_and_counters() {
        let mut v = InputValidator::new();
        v.update(0.5);
        v.update(f32::NAN);
        v.update(f32::INFINITY);
        v.reset();
        assert_eq!(v.nan_count(), 0);
        assert_eq!(v.inf_count(), 0);
        assert_eq!(v.last_valid(), 0.0);
    }
}
