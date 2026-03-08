// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Haptic effect primitives for tactile feedback
//!
//! Provides [`HapticPulse`] and [`HapticPattern`] building blocks plus
//! pre-built flight-specific patterns (stall warning, touchdown, turbulence).
//! The [`HapticEffect`] trait allows any effect to be sampled at arbitrary
//! time steps for integration with the RT spine.

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

// ── Pulse ────────────────────────────────────────────────────────────

/// A single haptic pulse with duration, intensity, and frequency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HapticPulse {
    /// Duration of the pulse in milliseconds.
    pub duration_ms: f64,
    /// Intensity of the pulse, clamped to 0.0–1.0.
    pub intensity: f64,
    /// Vibration frequency in Hz (clamped to 1.0–500.0).
    pub frequency_hz: f64,
}

impl HapticPulse {
    /// Create a new pulse with validated parameters.
    pub fn new(duration_ms: f64, intensity: f64, frequency_hz: f64) -> Self {
        Self {
            duration_ms: duration_ms.max(0.0),
            intensity: intensity.clamp(0.0, 1.0),
            frequency_hz: frequency_hz.clamp(1.0, 500.0),
        }
    }

    /// Duration expressed in seconds.
    #[inline]
    pub fn duration_s(&self) -> f64 {
        self.duration_ms / 1000.0
    }
}

// ── Pattern ──────────────────────────────────────────────────────────

/// A sequence of [`HapticPulse`]s separated by inter-pulse delays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HapticPattern {
    /// Ordered list of `(pulse, delay_after_ms)` pairs.
    /// The delay is the silence gap *after* the pulse finishes.
    pub steps: Vec<(HapticPulse, f64)>,
}

impl HapticPattern {
    /// Create an empty pattern.
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Append a pulse followed by a delay (ms).
    pub fn push(&mut self, pulse: HapticPulse, delay_after_ms: f64) {
        self.steps.push((pulse, delay_after_ms.max(0.0)));
    }

    /// Total duration of the pattern in milliseconds (pulses + delays).
    pub fn total_duration_ms(&self) -> f64 {
        self.steps.iter().map(|(p, d)| p.duration_ms + d).sum()
    }

    /// Number of pulses in the pattern.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the pattern is empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Default for HapticPattern {
    fn default() -> Self {
        Self::new()
    }
}

// ── Pre-built flight patterns ────────────────────────────────────────

/// Landing-gear extension/retraction pattern: thud + short rumble.
pub fn landing_gear() -> HapticPattern {
    let mut p = HapticPattern::new();
    p.push(HapticPulse::new(80.0, 0.9, 30.0), 20.0);
    p.push(HapticPulse::new(200.0, 0.4, 25.0), 0.0);
    p
}

/// Stall-warning buffet: rapid repeating pulses.
pub fn stall_warning() -> HapticPattern {
    let mut p = HapticPattern::new();
    for _ in 0..6 {
        p.push(HapticPulse::new(60.0, 0.85, 12.0), 40.0);
    }
    p
}

/// Touchdown impact: single sharp hit with decay rumble.
pub fn touchdown() -> HapticPattern {
    let mut p = HapticPattern::new();
    p.push(HapticPulse::new(50.0, 1.0, 35.0), 10.0);
    p.push(HapticPulse::new(300.0, 0.5, 20.0), 0.0);
    p
}

/// Engine vibration scaled by RPM (0–5 000 RPM range).
pub fn engine_vibration(rpm: f64) -> HapticPattern {
    let rpm = rpm.clamp(0.0, 5000.0);
    let freq = (rpm / 60.0).clamp(1.0, 500.0);
    let intensity = (rpm / 5000.0).clamp(0.0, 1.0) * 0.35;

    let mut p = HapticPattern::new();
    p.push(HapticPulse::new(500.0, intensity, freq), 0.0);
    p
}

/// Turbulence effect scaled by intensity (0.0–1.0).
pub fn turbulence(intensity: f64) -> HapticPattern {
    let intensity = intensity.clamp(0.0, 1.0);
    let mut p = HapticPattern::new();
    // Irregular-feeling burst sequence
    p.push(HapticPulse::new(120.0, intensity * 0.7, 8.0), 30.0);
    p.push(HapticPulse::new(80.0, intensity * 0.9, 12.0), 50.0);
    p.push(HapticPulse::new(150.0, intensity * 0.6, 6.0), 20.0);
    p.push(HapticPulse::new(100.0, intensity * 0.8, 10.0), 0.0);
    p
}

// ── HapticEffect trait ───────────────────────────────────────────────

/// Trait for time-stepped haptic effect evaluation.
///
/// Implementors produce a signed output sample in \[-1.0, 1.0\] for each
/// time step, allowing integration with the 250 Hz RT spine.
pub trait HapticEffect {
    /// Advance by `dt_s` seconds and return the output sample.
    fn next_sample(&mut self, dt_s: f64) -> f64;

    /// Whether the effect has finished playing.
    fn is_finished(&self) -> bool;
}

/// Plays back a [`HapticPattern`] sample-by-sample.
pub struct PatternPlayer {
    pattern: HapticPattern,
    /// Current position within the total pattern timeline (seconds).
    cursor_s: f64,
}

impl PatternPlayer {
    /// Create a player for the given pattern.
    pub fn new(pattern: HapticPattern) -> Self {
        Self {
            pattern,
            cursor_s: 0.0,
        }
    }
}

impl HapticEffect for PatternPlayer {
    fn next_sample(&mut self, dt_s: f64) -> f64 {
        let total_s = self.pattern.total_duration_ms() / 1000.0;
        if self.cursor_s >= total_s {
            return 0.0;
        }

        // Locate which step the cursor falls in.
        let mut offset_s = 0.0_f64;
        let mut sample = 0.0_f64;

        for (pulse, delay_ms) in &self.pattern.steps {
            let pulse_end = offset_s + pulse.duration_s();
            let step_end = pulse_end + delay_ms / 1000.0;

            if self.cursor_s < pulse_end {
                // Inside this pulse
                let t = self.cursor_s - offset_s;
                sample = pulse.intensity * (2.0 * PI * pulse.frequency_hz * t).sin();
                break;
            } else if self.cursor_s < step_end {
                // Inside the inter-pulse gap
                sample = 0.0;
                break;
            }
            offset_s = step_end;
        }

        self.cursor_s += dt_s;
        sample.clamp(-1.0, 1.0)
    }

    fn is_finished(&self) -> bool {
        self.cursor_s >= self.pattern.total_duration_ms() / 1000.0
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── HapticPulse ─────────────────────────────────────────────────

    #[test]
    fn test_pulse_creation() {
        let p = HapticPulse::new(100.0, 0.5, 50.0);
        assert_eq!(p.duration_ms, 100.0);
        assert_eq!(p.intensity, 0.5);
        assert_eq!(p.frequency_hz, 50.0);
    }

    #[test]
    fn test_pulse_intensity_clamping() {
        let low = HapticPulse::new(10.0, -0.5, 20.0);
        assert_eq!(low.intensity, 0.0);

        let high = HapticPulse::new(10.0, 2.0, 20.0);
        assert_eq!(high.intensity, 1.0);
    }

    #[test]
    fn test_pulse_frequency_clamping() {
        let low = HapticPulse::new(10.0, 0.5, 0.0);
        assert_eq!(low.frequency_hz, 1.0);

        let high = HapticPulse::new(10.0, 0.5, 1000.0);
        assert_eq!(high.frequency_hz, 500.0);
    }

    #[test]
    fn test_pulse_negative_duration() {
        let p = HapticPulse::new(-50.0, 0.5, 20.0);
        assert_eq!(p.duration_ms, 0.0);
    }

    #[test]
    fn test_pulse_duration_s() {
        let p = HapticPulse::new(250.0, 1.0, 40.0);
        assert!((p.duration_s() - 0.25).abs() < 1e-9);
    }

    // ── HapticPattern ───────────────────────────────────────────────

    #[test]
    fn test_empty_pattern() {
        let p = HapticPattern::new();
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
        assert_eq!(p.total_duration_ms(), 0.0);
    }

    #[test]
    fn test_pattern_push_and_duration() {
        let mut p = HapticPattern::new();
        p.push(HapticPulse::new(100.0, 1.0, 50.0), 50.0);
        p.push(HapticPulse::new(200.0, 0.5, 30.0), 0.0);

        assert_eq!(p.len(), 2);
        assert!(!p.is_empty());
        // 100 + 50 + 200 + 0 = 350
        assert!((p.total_duration_ms() - 350.0).abs() < 1e-9);
    }

    // ── Pre-built patterns ──────────────────────────────────────────

    #[test]
    fn test_landing_gear_pattern() {
        let p = landing_gear();
        assert_eq!(p.len(), 2);
        assert!(p.total_duration_ms() > 0.0);
        assert!(p.steps[0].0.intensity > 0.5, "first pulse should be strong");
    }

    #[test]
    fn test_stall_warning_pattern() {
        let p = stall_warning();
        assert_eq!(p.len(), 6);
        assert!(p.total_duration_ms() > 0.0);
        for (pulse, _) in &p.steps {
            assert!(pulse.intensity > 0.0);
        }
    }

    #[test]
    fn test_touchdown_pattern() {
        let p = touchdown();
        assert_eq!(p.len(), 2);
        assert_eq!(p.steps[0].0.intensity, 1.0, "first hit must be max");
    }

    #[test]
    fn test_engine_vibration_pattern() {
        let idle = engine_vibration(600.0);
        let cruise = engine_vibration(2400.0);
        assert!(idle.steps[0].0.intensity < cruise.steps[0].0.intensity);
    }

    #[test]
    fn test_engine_vibration_clamping() {
        let p = engine_vibration(999_999.0);
        assert!(p.steps[0].0.intensity <= 0.35 + 1e-9);
        assert!(p.steps[0].0.frequency_hz <= 500.0);
    }

    #[test]
    fn test_turbulence_zero() {
        let p = turbulence(0.0);
        for (pulse, _) in &p.steps {
            assert_eq!(pulse.intensity, 0.0);
        }
    }

    #[test]
    fn test_turbulence_full() {
        let p = turbulence(1.0);
        for (pulse, _) in &p.steps {
            assert!(pulse.intensity > 0.0);
            assert!(pulse.intensity <= 1.0);
        }
    }

    #[test]
    fn test_turbulence_clamping() {
        let p = turbulence(5.0);
        for (pulse, _) in &p.steps {
            assert!(pulse.intensity <= 1.0);
        }
    }

    // ── HapticEffect / PatternPlayer ────────────────────────────────

    #[test]
    fn test_pattern_player_produces_output() {
        let pat = touchdown();
        let mut player = PatternPlayer::new(pat);

        let mut any_nonzero = false;
        for _ in 0..500 {
            let s = player.next_sample(1.0 / 250.0);
            assert!((-1.0..=1.0).contains(&s), "output must be clamped");
            if s.abs() > 0.001 {
                any_nonzero = true;
            }
        }
        assert!(any_nonzero, "player should produce non-zero output");
    }

    #[test]
    fn test_pattern_player_finishes() {
        let mut pat = HapticPattern::new();
        pat.push(HapticPulse::new(10.0, 1.0, 50.0), 0.0);
        let mut player = PatternPlayer::new(pat);

        assert!(!player.is_finished());
        // Advance well past the 10 ms duration
        for _ in 0..100 {
            player.next_sample(1.0 / 250.0);
        }
        assert!(player.is_finished());
    }

    #[test]
    fn test_empty_pattern_player() {
        let mut player = PatternPlayer::new(HapticPattern::new());
        assert!(player.is_finished());
        assert_eq!(player.next_sample(0.004), 0.0);
    }

    #[test]
    fn test_pattern_player_silence_gap() {
        let mut pat = HapticPattern::new();
        // 10 ms pulse then 1000 ms gap then 10 ms pulse
        pat.push(HapticPulse::new(10.0, 1.0, 50.0), 1000.0);
        pat.push(HapticPulse::new(10.0, 1.0, 50.0), 0.0);

        let mut player = PatternPlayer::new(pat);

        // Skip past first pulse into the gap (≈12 ms in)
        for _ in 0..3 {
            player.next_sample(1.0 / 250.0); // 4 ms each
        }
        // Should be in the silence gap now
        let gap_sample = player.next_sample(1.0 / 250.0);
        assert!(
            gap_sample.abs() < 0.01,
            "should be silent in gap, got {}",
            gap_sample
        );
    }
}
