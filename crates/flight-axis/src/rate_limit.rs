// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis rate limiter.
//!
//! Limits how fast an axis value can change per tick, preventing instantaneous jumps.
//! A limit of `0.0` means no rate limiting (passthrough).

use std::collections::HashMap;

/// Per-axis rate limiter.
///
/// Limits how fast an axis value can change per tick, preventing instantaneous jumps.
/// A limit of `0.0` means no rate limiting (passthrough).
#[derive(Debug, Clone, PartialEq)]
pub struct AxisRateLimiter {
    /// Last output value.
    current: f32,
    /// Maximum change per tick (`0.0` = unlimited / passthrough).
    max_rate: f32,
}

impl AxisRateLimiter {
    /// Creates a new `AxisRateLimiter` with the given `max_rate`, current initialised to `0.0`.
    pub fn new(max_rate: f32) -> Self {
        Self {
            current: 0.0,
            max_rate,
        }
    }

    /// Creates a passthrough `AxisRateLimiter` with no rate limiting (`max_rate = 0.0`).
    pub fn unlimited() -> Self {
        Self::new(0.0)
    }

    /// Advances the limiter toward `target` by at most `max_rate` per tick.
    ///
    /// If `max_rate` is `0.0`, returns `target` unchanged. Output is always clamped to `[-1.0, 1.0]`.
    #[inline]
    pub fn apply(&mut self, target: f32) -> f32 {
        if self.max_rate == 0.0 {
            self.current = target;
            return target;
        }
        let delta = target - self.current;
        let step = delta.clamp(-self.max_rate, self.max_rate);
        self.current = (self.current + step).clamp(-1.0, 1.0);
        self.current
    }

    /// Resets the current value to `0.0`.
    #[inline]
    pub fn reset(&mut self) {
        self.current = 0.0;
    }

    /// Returns the current output value.
    #[inline]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Returns the configured maximum rate of change per tick.
    #[inline]
    pub fn max_rate(&self) -> f32 {
        self.max_rate
    }
}

/// Manages rate limiters for a collection of named axes.
///
/// Not intended for use on the RT hot path due to `HashMap` heap allocation.
#[derive(Debug, Clone, Default)]
pub struct AxisRateLimiterBank {
    limits: HashMap<String, AxisRateLimiter>,
}

impl AxisRateLimiterBank {
    /// Creates a new, empty `AxisRateLimiterBank`.
    pub fn new() -> Self {
        Self {
            limits: HashMap::new(),
        }
    }

    /// Returns a reference to the `AxisRateLimiter` for the given axis, if it exists.
    pub fn get(&self, axis: &str) -> Option<&AxisRateLimiter> {
        self.limits.get(axis)
    }

    /// Returns a mutable reference to the `AxisRateLimiter` for the given axis, if it exists.
    pub fn get_mut(&mut self, axis: &str) -> Option<&mut AxisRateLimiter> {
        self.limits.get_mut(axis)
    }

    /// Applies rate limiting for the named axis to `target`.
    ///
    /// Returns `target` unchanged if the axis is not present in the bank.
    #[inline]
    pub fn apply(&mut self, axis: &str, target: f32) -> f32 {
        match self.limits.get_mut(axis) {
            Some(limiter) => limiter.apply(target),
            None => target,
        }
    }

    /// Resets all axis rate limiters to `0.0`.
    pub fn reset_all(&mut self) {
        for limiter in self.limits.values_mut() {
            limiter.reset();
        }
    }

    /// Inserts or updates the rate limiter for the named axis.
    ///
    /// If the axis already exists, only the `max_rate` is updated; the current value is preserved.
    pub fn set_rate(&mut self, axis: &str, max_rate: f32) {
        self.limits
            .entry(axis.to_string())
            .and_modify(|l| l.max_rate = max_rate)
            .or_insert_with(|| AxisRateLimiter::new(max_rate));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── AxisRateLimiter unit tests ────────────────────────────────────────────

    #[test]
    fn test_rate_limiter_new_initial_values() {
        let limiter = AxisRateLimiter::new(0.1);
        assert_eq!(limiter.current(), 0.0);
        assert_eq!(limiter.max_rate(), 0.1);
    }

    #[test]
    fn test_rate_limiter_unlimited_constructor() {
        let limiter = AxisRateLimiter::unlimited();
        assert_eq!(limiter.max_rate(), 0.0);
        assert_eq!(limiter.current(), 0.0);
    }

    #[test]
    fn test_rate_limiter_unlimited_passthrough() {
        let mut limiter = AxisRateLimiter::new(0.0);
        let out = limiter.apply(0.8);
        assert_eq!(out, 0.8);
        assert_eq!(limiter.current(), 0.8);
    }

    #[test]
    fn test_rate_limiter_limits_rise() {
        let mut limiter = AxisRateLimiter::new(0.1);
        let out = limiter.apply(1.0);
        assert!((out - 0.1).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_rate_limiter_limits_fall() {
        let mut limiter = AxisRateLimiter::new(0.1);
        // Advance to 1.0 over 10 steps
        for _ in 0..10 {
            limiter.apply(1.0);
        }
        assert!((limiter.current() - 1.0).abs() < 1e-6);
        let out = limiter.apply(0.0);
        assert!((out - 0.9).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_rate_limiter_reaches_target_eventually() {
        let mut limiter = AxisRateLimiter::new(0.1);
        let mut out = 0.0f32;
        for _ in 0..10 {
            out = limiter.apply(1.0);
        }
        assert!((out - 1.0).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_rate_limiter_negative_target() {
        let mut limiter = AxisRateLimiter::new(0.1);
        let mut out = 0.0f32;
        for _ in 0..5 {
            out = limiter.apply(-0.5);
        }
        assert!((out - (-0.5)).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_rate_limiter_exact_target_no_overshoot() {
        let mut limiter = AxisRateLimiter::new(0.5);
        // delta=0.3 < max_rate=0.5 → jumps straight to target
        let out = limiter.apply(0.3);
        assert!((out - 0.3).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_rate_limiter_reset_clears_current() {
        let mut limiter = AxisRateLimiter::new(0.1);
        limiter.apply(1.0);
        assert!(limiter.current() > 0.0);
        limiter.reset();
        assert_eq!(limiter.current(), 0.0);
    }

    #[test]
    fn test_rate_limiter_zero_target() {
        let mut limiter = AxisRateLimiter::new(0.1);
        // Advance to 0.5 over 5 steps
        for _ in 0..5 {
            limiter.apply(1.0);
        }
        assert!((limiter.current() - 0.5).abs() < 1e-6);
        let out = limiter.apply(0.0);
        assert!((out - 0.4).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_rate_limiter_clamped_output() {
        // Use a large rate so we can try to push past ±1.0 with an out-of-range target
        let mut limiter = AxisRateLimiter::new(0.5);
        // Pump to 1.0 (2 steps of 0.5)
        limiter.apply(1.0);
        limiter.apply(1.0);
        // Now try to push further — output must stay clamped at 1.0
        let out = limiter.apply(2.0);
        assert!(out <= 1.0, "out={out} exceeded upper bound");
        assert!(out >= -1.0, "out={out} exceeded lower bound");
    }

    #[test]
    fn test_rate_limiter_large_max_rate() {
        // max_rate=2.0 means any delta ≤ 2.0 is applied in one tick (effectively unlimited for [-1,1])
        let mut limiter = AxisRateLimiter::new(2.0);
        let out = limiter.apply(0.8);
        assert!((out - 0.8).abs() < 1e-6, "out={out}");
    }

    // ── AxisRateLimiterBank unit tests ────────────────────────────────────────

    #[test]
    fn test_rate_bank_unknown_axis_passthrough() {
        let mut bank = AxisRateLimiterBank::new();
        let out = bank.apply("pitch", 0.75);
        assert_eq!(out, 0.75);
    }

    #[test]
    fn test_rate_bank_set_rate_inserts() {
        let mut bank = AxisRateLimiterBank::new();
        assert!(bank.get("roll").is_none());
        bank.set_rate("roll", 0.1);
        let limiter = bank.get("roll").expect("limiter should exist");
        assert_eq!(limiter.max_rate(), 0.1);
        assert_eq!(limiter.current(), 0.0);
    }

    #[test]
    fn test_rate_bank_apply_limits_correctly() {
        let mut bank = AxisRateLimiterBank::new();
        bank.set_rate("pitch", 0.1);
        let out = bank.apply("pitch", 1.0);
        assert!((out - 0.1).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_rate_bank_reset_all() {
        let mut bank = AxisRateLimiterBank::new();
        bank.set_rate("pitch", 0.1);
        bank.set_rate("roll", 0.1);
        bank.apply("pitch", 1.0);
        bank.apply("roll", -1.0);
        bank.reset_all();
        assert_eq!(bank.get("pitch").unwrap().current(), 0.0);
        assert_eq!(bank.get("roll").unwrap().current(), 0.0);
    }

    // ── Proptests ─────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_output_always_in_bounds(
            max_rate in 0.001f32..=0.5,
            target in -2.0f32..=2.0,
        ) {
            let mut limiter = AxisRateLimiter::new(max_rate);
            for _ in 0..50 {
                let out = limiter.apply(target);
                prop_assert!(out >= -1.0 && out <= 1.0, "out={out} not in [-1.0, 1.0]");
            }
        }

        #[test]
        fn proptest_rate_limited_no_overshoot(
            max_rate in 0.001f32..=0.5,
            target in -1.0f32..=1.0,
        ) {
            let mut limiter = AxisRateLimiter::new(max_rate);
            for _ in 0..200 {
                let prev = limiter.current();
                let out = limiter.apply(target);
                if target > prev {
                    prop_assert!(out <= target + 1e-5,
                        "Overshot upward: prev={prev}, out={out}, target={target}");
                    prop_assert!(out >= prev - 1e-5,
                        "Wrong direction: prev={prev}, out={out}, target={target}");
                } else if target < prev {
                    prop_assert!(out >= target - 1e-5,
                        "Overshot downward: prev={prev}, out={out}, target={target}");
                    prop_assert!(out <= prev + 1e-5,
                        "Wrong direction: prev={prev}, out={out}, target={target}");
                }
            }
        }

        #[test]
        fn proptest_unlimited_passthrough(input in -1.0f32..=1.0) {
            let mut limiter = AxisRateLimiter::unlimited();
            let out = limiter.apply(input);
            prop_assert_eq!(out, input);
        }
    }
}
