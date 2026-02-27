// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis jitter suppression with configurable threshold.
//!
//! Suppresses micro-jitter (noise below `threshold`) while allowing larger,
//! intentional movements through. Uses a simple last-value comparison.

/// Configuration for jitter suppression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JitterConfig {
    /// Minimum change required to update output (0.0 = disabled).
    pub threshold: f32,
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self { threshold: 0.001 }
    }
}

impl JitterConfig {
    /// Create with threshold disabled.
    pub fn disabled() -> Self {
        Self { threshold: 0.0 }
    }

    /// Validate: threshold must be finite and in [0, 1].
    pub fn validate(&self) -> Result<(), JitterError> {
        if !self.threshold.is_finite() || self.threshold < 0.0 || self.threshold > 1.0 {
            return Err(JitterError::InvalidThreshold(self.threshold));
        }
        Ok(())
    }
}

/// Jitter suppression state for a single axis.
#[derive(Debug, Clone, Copy)]
pub struct JitterSuppressor {
    config: JitterConfig,
    last_output: f32,
    suppressed_count: u32,
}

impl JitterSuppressor {
    /// Creates a new [`JitterSuppressor`] with the given configuration.
    pub fn new(config: JitterConfig) -> Result<Self, JitterError> {
        config.validate()?;
        Ok(Self {
            config,
            last_output: 0.0,
            suppressed_count: 0,
        })
    }

    /// Process one sample. Returns the (possibly unchanged) output.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.config.threshold <= 0.0 {
            self.last_output = input;
            return input;
        }
        let delta = (input - self.last_output).abs();
        if delta >= self.config.threshold {
            self.last_output = input;
        } else {
            self.suppressed_count += 1;
        }
        self.last_output
    }

    /// Returns the number of suppressed samples since creation or last reset.
    #[inline]
    pub fn suppressed_count(&self) -> u32 {
        self.suppressed_count
    }

    /// Returns the last output value.
    #[inline]
    pub fn last_output(&self) -> f32 {
        self.last_output
    }

    /// Resets state: clears last output and suppressed count.
    #[inline]
    pub fn reset(&mut self) {
        self.last_output = 0.0;
        self.suppressed_count = 0;
    }
}

/// Bank of jitter suppressors for multiple axes.
pub struct JitterBank<const N: usize> {
    suppressors: [JitterSuppressor; N],
}

impl<const N: usize> JitterBank<N> {
    /// Creates a new [`JitterBank`] with `N` suppressors, each using `config`.
    pub fn new(config: JitterConfig) -> Result<Self, JitterError> {
        config.validate()?;
        let suppressor = JitterSuppressor {
            config,
            last_output: 0.0,
            suppressed_count: 0,
        };
        Ok(Self {
            suppressors: [suppressor; N],
        })
    }

    /// Processes all `N` axes, writing results to `outputs`.
    pub fn process(&mut self, inputs: &[f32; N], outputs: &mut [f32; N]) {
        for ((out, suppressor), &input) in outputs
            .iter_mut()
            .zip(self.suppressors.iter_mut())
            .zip(inputs.iter())
        {
            *out = suppressor.process(input);
        }
    }

    /// Resets all suppressors in the bank.
    pub fn reset_all(&mut self) {
        for s in &mut self.suppressors {
            s.reset();
        }
    }
}

/// Errors from jitter suppression.
#[derive(Debug, thiserror::Error)]
pub enum JitterError {
    #[error("Threshold must be in [0, 1], got {0}")]
    InvalidThreshold(f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_threshold_passes_all() {
        let mut s = JitterSuppressor::new(JitterConfig::disabled()).unwrap();
        assert_eq!(s.process(0.5), 0.5);
        assert_eq!(s.process(0.0001), 0.0001);
        assert_eq!(s.process(-0.3), -0.3);
    }

    #[test]
    fn test_small_change_suppressed() {
        let mut s = JitterSuppressor::new(JitterConfig { threshold: 0.01 }).unwrap();
        // delta = 0.5 >= 0.01, passes and seeds last_output to 0.5
        s.process(0.5);
        // delta = 0.005 < 0.01, suppressed
        let out = s.process(0.505);
        assert_eq!(out, 0.5, "small change should be suppressed, got {out}");
    }

    #[test]
    fn test_large_change_passes() {
        let mut s = JitterSuppressor::new(JitterConfig { threshold: 0.01 }).unwrap();
        s.process(0.0); // seed last_output to 0.0
        let out = s.process(0.5); // delta = 0.5 >= 0.01
        assert!(
            (out - 0.5).abs() < 1e-6,
            "large change should pass, got {out}"
        );
    }

    #[test]
    fn test_suppressed_count_increments() {
        let mut s = JitterSuppressor::new(JitterConfig { threshold: 0.01 }).unwrap();
        s.process(0.5); // seeds, count stays 0
        s.process(0.501); // delta = 0.001 < 0.01, suppressed
        s.process(0.502); // delta = 0.002 < 0.01, suppressed
        s.process(0.503); // delta = 0.003 < 0.01, suppressed
        assert_eq!(s.suppressed_count(), 3);
    }

    #[test]
    fn test_suppressed_count_not_incremented_on_pass() {
        let mut s = JitterSuppressor::new(JitterConfig { threshold: 0.01 }).unwrap();
        s.process(0.0); // seed
        s.process(0.5); // large change, passes
        assert_eq!(s.suppressed_count(), 0);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut s = JitterSuppressor::new(JitterConfig::default()).unwrap();
        s.process(0.5);
        s.process(0.501);
        s.reset();
        assert_eq!(s.last_output(), 0.0);
        assert_eq!(s.suppressed_count(), 0);
    }

    #[test]
    fn test_negative_threshold_invalid() {
        let result = JitterSuppressor::new(JitterConfig { threshold: -0.1 });
        assert!(result.is_err(), "negative threshold should return Err");
    }

    #[test]
    fn test_threshold_above_one_invalid() {
        let result = JitterSuppressor::new(JitterConfig { threshold: 1.5 });
        assert!(result.is_err(), "threshold > 1 should return Err");
    }

    #[test]
    fn test_bank_processes_multiple_axes() {
        let config = JitterConfig { threshold: 0.01 };
        let mut bank: JitterBank<3> = JitterBank::new(config).unwrap();

        // Seed all axes from 0.0 to 0.5 (large change, all pass)
        let inputs = [0.5f32; 3];
        let mut outputs = [0.0f32; 3];
        bank.process(&inputs, &mut outputs);
        for (i, &out) in outputs.iter().enumerate() {
            assert!(
                (out - 0.5).abs() < 1e-6,
                "axis {i}: expected 0.5, got {out}"
            );
        }

        // axis 0: large change; axis 1 & 2: small changes
        let inputs2 = [0.8f32, 0.505f32, 0.502f32];
        bank.process(&inputs2, &mut outputs);
        assert!(
            (outputs[0] - 0.8).abs() < 1e-6,
            "axis 0 large change should pass"
        );
        assert!(
            (outputs[1] - 0.5).abs() < 1e-6,
            "axis 1 small change should be suppressed"
        );
        assert!(
            (outputs[2] - 0.5).abs() < 1e-6,
            "axis 2 small change should be suppressed"
        );
    }

    #[test]
    fn test_exact_threshold_passes() {
        let mut s = JitterSuppressor::new(JitterConfig { threshold: 0.01 }).unwrap();
        s.process(0.0); // seed
        // delta == threshold satisfies delta >= threshold (inclusive)
        let out = s.process(0.01);
        assert!(
            (out - 0.01).abs() < 1e-6,
            "exact threshold should pass, got {out}"
        );
    }
}
