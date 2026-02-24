// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Classic washout filter for motion platform cue generation.
//!
//! ## Background
//!
//! A motion platform has limited travel — it cannot sustain a constant tilt or
//! displacement indefinitely. The *washout filter* solves this by:
//!
//! 1. **Translational channels (surge/sway/heave)**: A high-pass filter passes
//!    transient acceleration cues and gradually returns the platform to neutral.
//!    This gives the pilot an onset cue for acceleration without saturating travel.
//!
//! 2. **Angular channels (roll/pitch)**: The raw attitude angle is used directly
//!    for sustained tilt cues. A low-pass filter smooths rapid changes.
//!
//! 3. **Yaw**: Angular rate is integrated and high-pass filtered, producing onset
//!    cues for rotation without saturating the platform.

use crate::config::WashoutConfig;

/// First-order high-pass filter state.
///
/// Discrete-time implementation using the bilinear transform:
/// `y[n] = α * (y[n-1] + x[n] - x[n-1])`
/// where `α = τ / (τ + dt)` and `τ = 1 / (2π * f_c)`.
#[derive(Debug, Clone)]
pub struct HighPassFilter {
    alpha: f32,
    prev_input: f32,
    prev_output: f32,
}

impl HighPassFilter {
    /// Create a new high-pass filter with the given corner frequency and sample interval.
    pub fn new(corner_freq_hz: f32, sample_dt: f32) -> Self {
        let tau = 1.0 / (2.0 * std::f32::consts::PI * corner_freq_hz.max(1e-6));
        let alpha = tau / (tau + sample_dt);
        Self {
            alpha,
            prev_input: 0.0,
            prev_output: 0.0,
        }
    }

    /// Process one sample and return the filtered output.
    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.alpha * (self.prev_output + input - self.prev_input);
        self.prev_input = input;
        self.prev_output = output;
        output
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.prev_input = 0.0;
        self.prev_output = 0.0;
    }
}

/// First-order low-pass filter state.
///
/// Discrete-time implementation:
/// `y[n] = (1 - α) * y[n-1] + α * x[n]`
/// where `α = dt / (τ + dt)` and `τ = 1 / (2π * f_c)`.
#[derive(Debug, Clone)]
pub struct LowPassFilter {
    alpha: f32,
    prev_output: f32,
}

impl LowPassFilter {
    /// Create a new low-pass filter with the given corner frequency and sample interval.
    pub fn new(corner_freq_hz: f32, sample_dt: f32) -> Self {
        let tau = 1.0 / (2.0 * std::f32::consts::PI * corner_freq_hz.max(1e-6));
        let alpha = sample_dt / (tau + sample_dt);
        Self {
            alpha,
            prev_output: 0.0,
        }
    }

    /// Process one sample and return the filtered output.
    pub fn process(&mut self, input: f32) -> f32 {
        let output = (1.0 - self.alpha) * self.prev_output + self.alpha * input;
        self.prev_output = output;
        output
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.prev_output = 0.0;
    }
}

/// Classic 6DOF washout filter bank.
///
/// Holds one filter per degree of freedom:
/// - Surge, sway, heave: high-pass (transient onset cues)
/// - Roll, pitch: low-pass (sustained tilt cues)
/// - Yaw: high-pass (onset rotation cue)
#[derive(Debug, Clone)]
pub struct WashoutFilter {
    pub surge_hp: HighPassFilter,
    pub sway_hp: HighPassFilter,
    pub heave_hp: HighPassFilter,
    pub roll_lp: LowPassFilter,
    pub pitch_lp: LowPassFilter,
    pub yaw_hp: HighPassFilter,
}

impl WashoutFilter {
    /// Create a new washout filter bank from configuration and sample rate.
    ///
    /// `sample_dt` is the time between ticks in seconds (e.g. `1.0 / 250.0` for 250 Hz).
    pub fn new(config: &WashoutConfig, sample_dt: f32) -> Self {
        Self {
            surge_hp: HighPassFilter::new(config.hp_frequency_hz, sample_dt),
            sway_hp: HighPassFilter::new(config.hp_frequency_hz, sample_dt),
            heave_hp: HighPassFilter::new(config.hp_frequency_hz, sample_dt),
            roll_lp: LowPassFilter::new(config.lp_frequency_hz, sample_dt),
            pitch_lp: LowPassFilter::new(config.lp_frequency_hz, sample_dt),
            yaw_hp: HighPassFilter::new(config.hp_frequency_hz, sample_dt),
        }
    }

    /// Reset all filter states.
    pub fn reset(&mut self) {
        self.surge_hp.reset();
        self.sway_hp.reset();
        self.heave_hp.reset();
        self.roll_lp.reset();
        self.pitch_lp.reset();
        self.yaw_hp.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_hp_filter_step_response_decays() {
        // After a constant input, the HP filter output should decay toward zero
        let mut hp = HighPassFilter::new(1.0, 0.01); // 1 Hz, 100 Hz sample rate
        let mut output = 0.0_f32;
        for _ in 0..1000 {
            output = hp.process(1.0);
        }
        // After many samples with constant input, output should be near zero
        assert!(output.abs() < 0.01, "HP filter did not wash out: {output}");
    }

    #[test]
    fn test_hp_filter_passes_step_onset() {
        // On the first sample after a step, the HP filter should pass most of the signal
        let mut hp = HighPassFilter::new(0.5, 1.0 / 250.0);
        let first_output = hp.process(1.0);
        assert!(
            first_output > 0.9,
            "HP filter should pass step onset: {first_output}"
        );
    }

    #[test]
    fn test_lp_filter_steady_state() {
        // LP filter should converge toward the input value
        let mut lp = LowPassFilter::new(5.0, 0.004); // 5 Hz, 250 Hz sample rate
        let mut output = 0.0_f32;
        for _ in 0..5000 {
            output = lp.process(1.0);
        }
        assert_abs_diff_eq!(output, 1.0, epsilon = 0.01);
    }

    #[test]
    fn test_lp_filter_attenuates_high_freq() {
        // High-frequency oscillation should be attenuated
        let mut lp = LowPassFilter::new(1.0, 0.004); // 1 Hz cutoff, 250 Hz sample rate
        let mut output = 0.0_f32;
        // Drive with 50 Hz oscillation (well above cutoff)
        for i in 0..250 {
            let input = (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 250.0).sin();
            output = lp.process(input);
        }
        // Should be heavily attenuated
        assert!(output.abs() < 0.1, "LP filter did not attenuate HF: {output}");
    }

    #[test]
    fn test_washout_filter_creation() {
        let config = WashoutConfig::default();
        let _wf = WashoutFilter::new(&config, 1.0 / 250.0);
    }

    #[test]
    fn test_washout_filter_reset() {
        let config = WashoutConfig::default();
        let mut wf = WashoutFilter::new(&config, 1.0 / 250.0);
        wf.surge_hp.process(1.0);
        wf.roll_lp.process(0.5);
        wf.reset();
        // After reset, first output with zero input should be zero
        assert_eq!(wf.surge_hp.process(0.0), 0.0);
        assert_eq!(wf.roll_lp.process(0.0), 0.0);
    }
}
