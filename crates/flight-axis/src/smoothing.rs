// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! EMA (Exponential Moving Average) smoothing filter for axis inputs.
//!
//! Formula: `output = alpha * input + (1.0 - alpha) * state`
//! - `alpha = 1.0` → passthrough (no smoothing)
//! - `alpha = 0.0` → frozen (never updates after first sample)
//! - `alpha in (0, 1)` → smoothed output

use std::collections::HashMap;

/// Error type for adaptive EMA configuration validation.
#[derive(Debug, thiserror::Error)]
pub enum AdaptiveEmaError {
    /// `alpha_slow` must be in (0, 1].
    #[error("alpha_slow must be in (0, 1], got {0}")]
    InvalidAlphaSlow(f32),
    /// `alpha_fast` must be in (0, 1] and >= alpha_slow.
    #[error("alpha_fast must be in (0, 1] and >= alpha_slow, got {0}")]
    InvalidAlphaFast(f32),
    /// `velocity_threshold` must be > 0.
    #[error("velocity_threshold must be > 0, got {0}")]
    InvalidThreshold(f32),
}

/// Configuration for adaptive EMA smoothing.
///
/// When axis velocity is low, uses `alpha_slow` (more smoothing).
/// When axis velocity is high, uses `alpha_fast` (less smoothing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveEmaConfig {
    /// Alpha for low-velocity conditions (0 < alpha_slow <= alpha_fast <= 1).
    pub alpha_slow: f32,
    /// Alpha for high-velocity conditions (alpha_slow <= alpha_fast <= 1).
    pub alpha_fast: f32,
    /// Absolute velocity (change per tick) at or above which `alpha_fast` is used.
    pub velocity_threshold: f32,
}

impl Default for AdaptiveEmaConfig {
    fn default() -> Self {
        Self {
            alpha_slow: 0.05,
            alpha_fast: 0.5,
            velocity_threshold: 0.01,
        }
    }
}

impl AdaptiveEmaConfig {
    /// Validates the configuration.
    ///
    /// Returns an error if any constraint is violated:
    /// - `alpha_slow` must be in (0, 1]
    /// - `alpha_fast` must be in (0, 1] and >= `alpha_slow`
    /// - `velocity_threshold` must be > 0
    pub fn validate(&self) -> Result<(), AdaptiveEmaError> {
        if self.alpha_slow <= 0.0 || self.alpha_slow > 1.0 {
            return Err(AdaptiveEmaError::InvalidAlphaSlow(self.alpha_slow));
        }
        if self.alpha_fast <= 0.0 || self.alpha_fast > 1.0 || self.alpha_fast < self.alpha_slow {
            return Err(AdaptiveEmaError::InvalidAlphaFast(self.alpha_fast));
        }
        if self.velocity_threshold <= 0.0 {
            return Err(AdaptiveEmaError::InvalidThreshold(self.velocity_threshold));
        }
        Ok(())
    }
}

/// Adaptive EMA smoother: adjusts alpha based on input velocity (rate of change).
///
/// Uses `alpha_slow` when `|input - last_input| < velocity_threshold`,
/// and `alpha_fast` otherwise. Both values must satisfy `0 < alpha_slow <= alpha_fast <= 1`.
///
/// Uses only stack state — zero allocations on the hot path (ADR-004).
#[derive(Debug, Clone, Copy)]
pub struct AdaptiveEmaSmoother {
    config: AdaptiveEmaConfig,
    last_output: f32,
    last_input: f32,
    tick: u64,
}

impl AdaptiveEmaSmoother {
    /// Creates a new `AdaptiveEmaSmoother` with the given configuration.
    ///
    /// Returns an error if the configuration fails validation.
    pub fn new(config: AdaptiveEmaConfig) -> Result<Self, AdaptiveEmaError> {
        config.validate()?;
        Ok(Self {
            config,
            last_output: 0.0,
            last_input: 0.0,
            tick: 0,
        })
    }

    /// Applies the adaptive EMA filter to `input` and returns the new output.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let velocity = (input - self.last_input).abs();
        let alpha = if velocity >= self.config.velocity_threshold {
            self.config.alpha_fast
        } else {
            self.config.alpha_slow
        };
        let output = alpha * input + (1.0 - alpha) * self.last_output;
        self.last_input = input;
        self.last_output = output;
        self.tick += 1;
        output
    }

    /// Resets the filter state to zero.
    #[inline]
    pub fn reset(&mut self) {
        self.last_output = 0.0;
        self.last_input = 0.0;
        self.tick = 0;
    }

    /// Returns the number of samples processed since the last reset.
    #[inline]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Returns the current filter output (last processed value).
    #[inline]
    pub fn current_output(&self) -> f32 {
        self.last_output
    }
}

/// EMA (Exponential Moving Average) smoothing filter for axis inputs.
///
/// Formula: `output = alpha * input + (1.0 - alpha) * state`
/// - `alpha = 1.0` → passthrough (no smoothing)
/// - `alpha = 0.0` → frozen (never updates after first sample)
/// - `alpha in (0, 1)` → smoothed output
#[derive(Debug, Clone, PartialEq)]
pub struct EmaFilter {
    /// Smoothing factor in `[0.0, 1.0]`.
    alpha: f32,
    /// Previous output value.
    state: f32,
    /// `false` until the first sample has been applied.
    initialized: bool,
}

impl EmaFilter {
    /// Creates a new `EmaFilter` with the given `alpha`.
    ///
    /// # Panics
    ///
    /// Panics if `alpha` is outside `[0.0, 1.0]`.
    pub fn new(alpha: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&alpha),
            "alpha must be in [0.0, 1.0], got {alpha}"
        );
        Self {
            alpha,
            state: 0.0,
            initialized: false,
        }
    }

    /// Creates a passthrough `EmaFilter` (`alpha = 1.0`, no smoothing).
    pub fn passthrough() -> Self {
        Self::new(1.0)
    }

    /// Applies the filter to `input` and returns the new output.
    ///
    /// On the first call the filter is seeded with `input` directly (no lag on startup).
    /// Subsequent calls apply the EMA formula.
    #[inline]
    pub fn apply(&mut self, input: f32) -> f32 {
        if !self.initialized {
            self.state = input;
            self.initialized = true;
            return input;
        }
        self.state = self.alpha * input + (1.0 - self.alpha) * self.state;
        self.state
    }

    /// Resets the filter: clears state and marks it as uninitialized.
    #[inline]
    pub fn reset(&mut self) {
        self.state = 0.0;
        self.initialized = false;
    }

    /// Returns the configured smoothing factor.
    #[inline]
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Returns the current internal state (last output).
    #[inline]
    pub fn state(&self) -> f32 {
        self.state
    }
}

/// Manages EMA smoothing filters for a collection of named axes.
///
/// Not intended for use on the RT hot path due to `HashMap` heap allocation.
#[derive(Debug, Clone, Default)]
pub struct EmaFilterBank {
    filters: HashMap<String, EmaFilter>,
}

impl EmaFilterBank {
    /// Creates a new, empty `EmaFilterBank`.
    pub fn new() -> Self {
        Self {
            filters: HashMap::new(),
        }
    }

    /// Returns a reference to the `EmaFilter` for the given axis, if it exists.
    pub fn get(&self, axis: &str) -> Option<&EmaFilter> {
        self.filters.get(axis)
    }

    /// Applies smoothing for the named axis to `input`.
    ///
    /// Returns `input` unchanged if the axis is not present in the bank.
    #[inline]
    pub fn apply(&mut self, axis: &str, input: f32) -> f32 {
        match self.filters.get_mut(axis) {
            Some(filter) => filter.apply(input),
            None => input,
        }
    }

    /// Inserts or updates the smoothing filter for the named axis.
    ///
    /// If the axis already exists, only `alpha` is updated; the current state is preserved.
    pub fn set_alpha(&mut self, axis: &str, alpha: f32) {
        self.filters
            .entry(axis.to_string())
            .and_modify(|f| f.alpha = alpha)
            .or_insert_with(|| EmaFilter::new(alpha));
    }

    /// Resets all filters to their uninitialized state.
    pub fn reset_all(&mut self) {
        for filter in self.filters.values_mut() {
            filter.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── EmaFilter unit tests ──────────────────────────────────────────────────

    #[test]
    fn test_ema_passthrough_alpha_one() {
        let mut f = EmaFilter::new(1.0);
        assert_eq!(f.apply(0.5), 0.5);
    }

    #[test]
    fn test_ema_heavy_smoothing() {
        let mut f = EmaFilter::new(0.5);
        // First apply seeds state to 0.0 (initial input)
        f.apply(0.0);
        // Second apply: 0.5 * 1.0 + 0.5 * 0.0 = 0.5
        let out = f.apply(1.0);
        assert!((out - 0.5).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_ema_second_application() {
        let mut f = EmaFilter::new(0.5);
        f.apply(0.0); // seed state = 0.0
        f.apply(1.0); // state = 0.5
        // 0.5 * 1.0 + 0.5 * 0.5 = 0.75
        let out = f.apply(1.0);
        assert!((out - 0.75).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_ema_converges_to_constant_input() {
        let mut f = EmaFilter::new(0.2);
        for _ in 0..100 {
            f.apply(1.0);
        }
        assert!(
            (f.state() - 1.0).abs() < 1e-4,
            "state={} did not converge to 1.0",
            f.state()
        );
    }

    #[test]
    fn test_ema_first_sample_initializes() {
        let mut f = EmaFilter::new(0.3);
        let out = f.apply(0.7);
        assert!((out - 0.7).abs() < 1e-6, "out={out}");
        assert!((f.state() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_ema_reset_reinitializes() {
        let mut f = EmaFilter::new(0.5);
        f.apply(0.5);
        f.apply(0.5);
        f.reset();
        // After reset, next apply seeds with input directly
        let out = f.apply(0.9);
        assert!((out - 0.9).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn test_ema_alpha_zero_freezes() {
        let mut f = EmaFilter::new(0.0);
        // First apply seeds state = 0.3
        f.apply(0.3);
        // Subsequent: 0.0 * input + 1.0 * state → state never changes
        let out1 = f.apply(1.0);
        let out2 = f.apply(-1.0);
        assert!((out1 - 0.3).abs() < 1e-6, "out1={out1}");
        assert!((out2 - 0.3).abs() < 1e-6, "out2={out2}");
    }

    #[test]
    fn test_ema_negative_input() {
        let mut f = EmaFilter::new(0.5);
        for _ in 0..50 {
            f.apply(-1.0);
        }
        assert!(
            f.state() < 0.0,
            "state should be negative, got {}",
            f.state()
        );
    }

    #[test]
    fn test_ema_clamped_output() {
        let mut f = EmaFilter::new(0.5);
        for _ in 0..50 {
            let out = f.apply(0.8);
            assert!(out >= -1.0 && out <= 1.0, "out={out} outside [-1.0, 1.0]");
        }
        for _ in 0..50 {
            let out = f.apply(-0.8);
            assert!(out >= -1.0 && out <= 1.0, "out={out} outside [-1.0, 1.0]");
        }
    }

    #[test]
    #[should_panic(expected = "alpha must be in [0.0, 1.0]")]
    fn test_ema_new_invalid_alpha_panics() {
        EmaFilter::new(1.5);
    }

    #[test]
    #[should_panic(expected = "alpha must be in [0.0, 1.0]")]
    fn test_ema_new_negative_alpha_panics() {
        EmaFilter::new(-0.1);
    }

    // ── EmaFilterBank unit tests ──────────────────────────────────────────────

    #[test]
    fn test_ema_bank_unknown_passthrough() {
        let mut bank = EmaFilterBank::new();
        let out = bank.apply("pitch", 0.42);
        assert_eq!(out, 0.42);
    }

    #[test]
    fn test_ema_bank_set_alpha_inserts() {
        let mut bank = EmaFilterBank::new();
        assert!(bank.get("roll").is_none());
        bank.set_alpha("roll", 0.3);
        let f = bank.get("roll").expect("filter should exist");
        assert!((f.alpha() - 0.3).abs() < 1e-6);
    }

    #[test]
    fn test_ema_bank_reset_all() {
        let mut bank = EmaFilterBank::new();
        bank.set_alpha("pitch", 0.5);
        bank.set_alpha("roll", 0.5);
        bank.apply("pitch", 1.0);
        bank.apply("roll", -1.0);
        bank.reset_all();
        assert_eq!(bank.get("pitch").unwrap().state(), 0.0);
        assert_eq!(bank.get("roll").unwrap().state(), 0.0);
    }

    // ── AdaptiveEmaSmoother unit tests ───────────────────────────────────────

    #[test]
    fn test_adaptive_ema_low_velocity_uses_slow_alpha() {
        let config = AdaptiveEmaConfig {
            alpha_slow: 0.05,
            alpha_fast: 0.5,
            velocity_threshold: 0.05,
        };
        let mut s = AdaptiveEmaSmoother::new(config).unwrap();
        // velocity = |0.01 - 0.0| = 0.01 < 0.05 → alpha_slow
        let out = s.process(0.01_f32);
        // output = 0.05 * 0.01 + 0.95 * 0.0 = 0.0005
        let measured_alpha = out / 0.01_f32;
        assert!(
            (measured_alpha - 0.05).abs() < 1e-5,
            "expected alpha_slow=0.05, got {measured_alpha}"
        );
    }

    #[test]
    fn test_adaptive_ema_high_velocity_uses_fast_alpha() {
        let config = AdaptiveEmaConfig {
            alpha_slow: 0.05,
            alpha_fast: 0.5,
            velocity_threshold: 0.05,
        };
        let mut s = AdaptiveEmaSmoother::new(config).unwrap();
        // velocity = |1.0 - 0.0| = 1.0 >= 0.05 → alpha_fast
        let out = s.process(1.0_f32);
        // output = 0.5 * 1.0 + 0.5 * 0.0 = 0.5
        assert!(
            (out - 0.5).abs() < 1e-5,
            "expected alpha_fast output 0.5, got {out}"
        );
    }

    #[test]
    fn test_adaptive_ema_convergence_to_constant() {
        let mut s = AdaptiveEmaSmoother::new(AdaptiveEmaConfig::default()).unwrap();
        for _ in 0..1000 {
            s.process(1.0_f32);
        }
        assert!(
            (s.current_output() - 1.0).abs() < 1e-3,
            "output={} did not converge to 1.0",
            s.current_output()
        );
    }

    #[test]
    fn test_adaptive_ema_reset_clears_state() {
        let mut s = AdaptiveEmaSmoother::new(AdaptiveEmaConfig::default()).unwrap();
        for _ in 0..20 {
            s.process(1.0_f32);
        }
        s.reset();
        assert_eq!(s.tick(), 0, "tick should be 0 after reset");
        assert_eq!(s.current_output(), 0.0, "output should be 0.0 after reset");
        // Verify last_input is cleared: process 0.0 should give zero output
        let out = s.process(0.0_f32);
        assert_eq!(out, 0.0, "output after reset+zero-input should be 0.0");
    }

    #[test]
    fn test_adaptive_ema_output_bounded() {
        let mut s = AdaptiveEmaSmoother::new(AdaptiveEmaConfig::default()).unwrap();
        let inputs: &[f32] = &[-1.0, -0.5, 0.0, 0.5, 1.0, 0.3, -0.7, 1.0, -1.0];
        for &inp in inputs {
            let out = s.process(inp);
            assert!(
                out >= -1.0 && out <= 1.0 && out.is_finite(),
                "out={out} not in [-1.0, 1.0] for inp={inp}"
            );
        }
    }

    #[test]
    fn test_adaptive_ema_alpha_slow_zero_invalid() {
        let config = AdaptiveEmaConfig {
            alpha_slow: 0.0,
            alpha_fast: 0.5,
            velocity_threshold: 0.01,
        };
        assert!(
            matches!(
                config.validate(),
                Err(AdaptiveEmaError::InvalidAlphaSlow(_))
            ),
            "alpha_slow=0.0 should be invalid"
        );
    }

    #[test]
    fn test_adaptive_ema_alpha_fast_less_than_slow_invalid() {
        let config = AdaptiveEmaConfig {
            alpha_slow: 0.5,
            alpha_fast: 0.3,
            velocity_threshold: 0.01,
        };
        assert!(
            matches!(
                config.validate(),
                Err(AdaptiveEmaError::InvalidAlphaFast(_))
            ),
            "alpha_fast < alpha_slow should be invalid"
        );
    }

    #[test]
    fn test_adaptive_ema_threshold_zero_invalid() {
        let config = AdaptiveEmaConfig {
            alpha_slow: 0.05,
            alpha_fast: 0.5,
            velocity_threshold: 0.0,
        };
        assert!(
            matches!(
                config.validate(),
                Err(AdaptiveEmaError::InvalidThreshold(_))
            ),
            "velocity_threshold=0.0 should be invalid"
        );
    }

    #[test]
    fn test_adaptive_ema_tick_increments() {
        let mut s = AdaptiveEmaSmoother::new(AdaptiveEmaConfig::default()).unwrap();
        assert_eq!(s.tick(), 0);
        s.process(0.1_f32);
        assert_eq!(s.tick(), 1);
        s.process(0.2_f32);
        assert_eq!(s.tick(), 2);
        s.process(0.3_f32);
        assert_eq!(s.tick(), 3);
    }

    #[test]
    fn test_adaptive_ema_default_config_valid() {
        let config = AdaptiveEmaConfig::default();
        assert!(config.validate().is_ok(), "default config should be valid");
        assert!(
            AdaptiveEmaSmoother::new(config).is_ok(),
            "default config should construct successfully"
        );
    }

    // ── Proptests ─────────────────────────────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_output_always_in_bounds(
            alpha in 0.0f32..=1.0,
            input in -1.0f32..=1.0,
        ) {
            let mut f = EmaFilter::new(alpha);
            for _ in 0..50 {
                let out = f.apply(input);
                prop_assert!(
                    out >= -1.0 && out <= 1.0,
                    "out={out} not in [-1.0, 1.0]"
                );
            }
        }

        #[test]
        fn proptest_passthrough_equals_input(input in -1.0f32..=1.0) {
            let mut f = EmaFilter::passthrough();
            // After seeding (first call returns input directly), every subsequent call
            // with alpha=1.0 also returns input exactly.
            let out = f.apply(input);
            prop_assert_eq!(out, input);
            let out2 = f.apply(input);
            prop_assert_eq!(out2, input);
        }

        #[test]
        fn proptest_convergence_bound(
            alpha in 0.01f32..=0.99,
            target in -1.0f32..=1.0,
            n in 1usize..=30,
        ) {
            let mut f = EmaFilter::new(alpha);
            // Seed with the target so we measure pure EMA decay from a known start.
            f.apply(0.0); // seed state = 0.0
            for _ in 0..n {
                f.apply(target);
            }
            // After n applications: |state - target| ≤ |target| * (1 - alpha)^n
            let expected_bound = target.abs() * (1.0 - alpha).powi(n as i32) + 1e-5;
            prop_assert!(
                (f.state() - target).abs() <= expected_bound,
                "state={}, target={target}, bound={expected_bound}",
                f.state()
            );
        }
    }
}
