// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tactile effect mixing with gain control and frequency-band separation
//!
//! Wraps [`TactileEngine`] to provide per-effect gain, master gain, and
//! frequency-band gains. All processing is zero-allocation.

use crate::engine::{MAX_EFFECTS, TactileEffect, TactileEngine};

/// Frequency band classification for tactile effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyBand {
    /// Sub-bass / bass (< 40 Hz) — bass shakers.
    Low = 0,
    /// Mid-range (40–150 Hz) — rumble motors.
    Mid = 1,
    /// High-range (≥ 150 Hz) — fine textures.
    High = 2,
}

/// Low ↔ mid boundary in Hz.
const BAND_LOW_MID_HZ: f64 = 40.0;
/// Mid ↔ high boundary in Hz.
const BAND_MID_HIGH_HZ: f64 = 150.0;

/// Per-band and combined output from the mixer.
#[derive(Debug, Clone, Copy)]
pub struct MixerOutput {
    /// Combined output, clamped to \[-1.0, 1.0\].
    pub combined: f64,
    /// Low-band contribution, clamped to \[-1.0, 1.0\].
    pub low: f64,
    /// Mid-band contribution, clamped to \[-1.0, 1.0\].
    pub mid: f64,
    /// High-band contribution, clamped to \[-1.0, 1.0\].
    pub high: f64,
}

/// Tactile mixer with per-effect gain, master gain, and band separation.
///
/// All state is stack-allocated — no heap allocation on the hot path.
pub struct TactileMixer {
    engine: TactileEngine,
    slot_gains: [f64; MAX_EFFECTS],
    master_gain: f64,
    band_gains: [f64; 3],
}

impl TactileMixer {
    /// Create a new mixer with unity gains.
    pub fn new() -> Self {
        Self {
            engine: TactileEngine::new(),
            slot_gains: [1.0; MAX_EFFECTS],
            master_gain: 1.0,
            band_gains: [1.0; 3],
        }
    }

    // ── effect management ───────────────────────────────────────────

    /// Queue an effect and return its slot index.
    pub fn add_effect(&mut self, effect: TactileEffect) -> Option<usize> {
        self.engine.add_effect(effect)
    }

    /// Queue an effect with an explicit per-slot gain.
    pub fn add_effect_with_gain(&mut self, effect: TactileEffect, gain: f64) -> Option<usize> {
        if let Some(idx) = self.engine.add_effect(effect) {
            self.slot_gains[idx] = gain.clamp(0.0, 2.0);
            Some(idx)
        } else {
            None
        }
    }

    /// Remove a single effect by slot index, resetting its gain to 1.0.
    pub fn remove_effect(&mut self, slot: usize) {
        self.engine.remove_effect(slot);
        if slot < MAX_EFFECTS {
            self.slot_gains[slot] = 1.0;
        }
    }

    /// Stop all effects and reset every slot gain to 1.0.
    pub fn clear(&mut self) {
        self.engine.clear();
        self.slot_gains = [1.0; MAX_EFFECTS];
    }

    /// Number of active effects.
    pub fn active_count(&self) -> usize {
        self.engine.active_count()
    }

    /// Total ticks processed.
    pub fn tick_count(&self) -> u64 {
        self.engine.tick_count()
    }

    // ── gain control ────────────────────────────────────────────────

    /// Set the gain for a specific slot (clamped to 0.0–2.0).
    pub fn set_slot_gain(&mut self, slot: usize, gain: f64) {
        if slot < MAX_EFFECTS {
            self.slot_gains[slot] = gain.clamp(0.0, 2.0);
        }
    }

    /// Current gain for a slot.
    pub fn slot_gain(&self, slot: usize) -> f64 {
        if slot < MAX_EFFECTS {
            self.slot_gains[slot]
        } else {
            0.0
        }
    }

    /// Set the master output gain (clamped to 0.0–2.0).
    pub fn set_master_gain(&mut self, gain: f64) {
        self.master_gain = gain.clamp(0.0, 2.0);
    }

    /// Current master gain.
    pub fn master_gain(&self) -> f64 {
        self.master_gain
    }

    /// Set the gain for a frequency band (clamped to 0.0–2.0).
    pub fn set_band_gain(&mut self, band: FrequencyBand, gain: f64) {
        self.band_gains[band as usize] = gain.clamp(0.0, 2.0);
    }

    /// Current gain for a frequency band.
    pub fn band_gain(&self, band: FrequencyBand) -> f64 {
        self.band_gains[band as usize]
    }

    // ── tick ────────────────────────────────────────────────────────

    /// Advance one tick and return the mixed, gain-adjusted output.
    pub fn tick(&mut self) -> MixerOutput {
        let mut low = 0.0_f64;
        let mut mid = 0.0_f64;
        let mut high = 0.0_f64;

        for (i, slot) in self.engine.slots.iter_mut().enumerate() {
            if slot.active {
                let sample = slot.compute_sample() * self.slot_gains[i];
                let freq = slot.effect.frequency_hz();

                if freq < BAND_LOW_MID_HZ {
                    low += sample * self.band_gains[FrequencyBand::Low as usize];
                } else if freq < BAND_MID_HIGH_HZ {
                    mid += sample * self.band_gains[FrequencyBand::Mid as usize];
                } else {
                    high += sample * self.band_gains[FrequencyBand::High as usize];
                }

                slot.elapsed_ticks += 1;
                if slot.is_expired() {
                    slot.active = false;
                }
            }
        }

        self.engine.tick_count += 1;

        let combined = (low + mid + high) * self.master_gain;
        MixerOutput {
            combined: combined.clamp(-1.0, 1.0),
            low: (low * self.master_gain).clamp(-1.0, 1.0),
            mid: (mid * self.master_gain).clamp(-1.0, 1.0),
            high: (high * self.master_gain).clamp(-1.0, 1.0),
        }
    }

    // ── helpers ─────────────────────────────────────────────────────

    /// Borrow the underlying engine (read-only).
    pub fn engine(&self) -> &TactileEngine {
        &self.engine
    }

    /// Classify a frequency into its band.
    pub fn classify_band(freq_hz: f64) -> FrequencyBand {
        if freq_hz < BAND_LOW_MID_HZ {
            FrequencyBand::Low
        } else if freq_hz < BAND_MID_HIGH_HZ {
            FrequencyBand::Mid
        } else {
            FrequencyBand::High
        }
    }
}

impl Default for TactileMixer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{MAX_EFFECTS, TexturePattern};

    // ── creation ────────────────────────────────────────────────────

    #[test]
    fn test_mixer_creation() {
        let mixer = TactileMixer::new();
        assert_eq!(mixer.active_count(), 0);
        assert_eq!(mixer.master_gain(), 1.0);
        assert_eq!(mixer.band_gain(FrequencyBand::Low), 1.0);
        assert_eq!(mixer.band_gain(FrequencyBand::Mid), 1.0);
        assert_eq!(mixer.band_gain(FrequencyBand::High), 1.0);
    }

    // ── basic tick ──────────────────────────────────────────────────

    #[test]
    fn test_mixer_tick_impact_at_t0() {
        let mut mixer = TactileMixer::new();
        mixer.add_effect(TactileEffect::Impact {
            magnitude: 0.5,
            decay_rate: 5.0,
        });
        let out = mixer.tick();
        assert!(
            (out.combined - 0.5).abs() < 0.01,
            "impact at t=0 should produce ~magnitude"
        );
    }

    // ── master gain ─────────────────────────────────────────────────

    #[test]
    fn test_master_gain() {
        let mut mixer = TactileMixer::new();
        mixer.set_master_gain(0.5);
        mixer.add_effect(TactileEffect::Impact {
            magnitude: 1.0,
            decay_rate: 5.0,
        });
        let out = mixer.tick();
        assert!((out.combined - 0.5).abs() < 0.01);
    }

    // ── slot gain ───────────────────────────────────────────────────

    #[test]
    fn test_slot_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Impact {
                magnitude: 1.0,
                decay_rate: 5.0,
            })
            .unwrap();
        mixer.set_slot_gain(slot, 0.25);
        let out = mixer.tick();
        assert!((out.combined - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_add_effect_with_gain() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect_with_gain(
                TactileEffect::Impact {
                    magnitude: 1.0,
                    decay_rate: 5.0,
                },
                0.3,
            )
            .unwrap();
        assert!((mixer.slot_gain(slot) - 0.3).abs() < 1e-9);
        let out = mixer.tick();
        assert!((out.combined - 0.3).abs() < 0.01);
    }

    // ── band gain ───────────────────────────────────────────────────

    #[test]
    fn test_band_gain_low() {
        let mut mixer = TactileMixer::new();
        mixer.set_band_gain(FrequencyBand::Low, 0.5);
        // Impact → 20 Hz → Low band
        mixer.add_effect(TactileEffect::Impact {
            magnitude: 1.0,
            decay_rate: 5.0,
        });
        let out = mixer.tick();
        assert!((out.combined - 0.5).abs() < 0.01);
        assert!((out.low - 0.5).abs() < 0.01);
        assert_eq!(out.mid, 0.0);
        assert_eq!(out.high, 0.0);
    }

    // ── frequency band separation ───────────────────────────────────

    #[test]
    fn test_band_separation() {
        let mut mixer = TactileMixer::new();
        // Low: Impact (20 Hz)
        mixer.add_effect(TactileEffect::Impact {
            magnitude: 0.5,
            decay_rate: 5.0,
        });
        // Mid: Rumble at 80 Hz
        mixer.add_effect(TactileEffect::Rumble {
            frequency_hz: 80.0,
            amplitude: 0.5,
            duration_ticks: 100,
        });
        // High: Texture at 200 Hz
        mixer.add_effect(TactileEffect::Texture {
            frequency_hz: 200.0,
            amplitude: 0.5,
            pattern: TexturePattern::Square,
        });

        let out = mixer.tick();
        // Impact at t=0 → low ≈ 0.5
        assert!(out.low.abs() > 0.1, "low band should have impact");
    }

    // ── summation ───────────────────────────────────────────────────

    #[test]
    fn test_mixing_summation() {
        let mut mixer = TactileMixer::new();
        mixer.add_effect(TactileEffect::Impact {
            magnitude: 0.3,
            decay_rate: 5.0,
        });
        mixer.add_effect(TactileEffect::Impact {
            magnitude: 0.4,
            decay_rate: 5.0,
        });
        let out = mixer.tick();
        assert!((out.combined - 0.7).abs() < 0.01, "should sum to 0.7");
    }

    // ── clamping ────────────────────────────────────────────────────

    #[test]
    fn test_output_clamping() {
        let mut mixer = TactileMixer::new();
        for _ in 0..MAX_EFFECTS {
            mixer.add_effect(TactileEffect::Impact {
                magnitude: 1.0,
                decay_rate: 0.1,
            });
        }
        let out = mixer.tick();
        assert!((-1.0..=1.0).contains(&out.combined));
        assert!((-1.0..=1.0).contains(&out.low));
        assert!((-1.0..=1.0).contains(&out.mid));
        assert!((-1.0..=1.0).contains(&out.high));
    }

    // ── clear / remove ──────────────────────────────────────────────

    #[test]
    fn test_mixer_clear() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Rumble {
                frequency_hz: 50.0,
                amplitude: 0.5,
                duration_ticks: 100,
            })
            .unwrap();
        mixer.set_slot_gain(slot, 0.3);

        mixer.clear();
        assert_eq!(mixer.active_count(), 0);
        assert_eq!(mixer.slot_gain(slot), 1.0, "gain should reset");
    }

    #[test]
    fn test_mixer_remove_effect() {
        let mut mixer = TactileMixer::new();
        let slot = mixer
            .add_effect(TactileEffect::Rumble {
                frequency_hz: 50.0,
                amplitude: 0.5,
                duration_ticks: 100,
            })
            .unwrap();
        mixer.set_slot_gain(slot, 0.5);

        mixer.remove_effect(slot);
        assert_eq!(mixer.active_count(), 0);
        assert_eq!(mixer.slot_gain(slot), 1.0);
    }

    // ── gain clamping ───────────────────────────────────────────────

    #[test]
    fn test_gain_clamping() {
        let mut mixer = TactileMixer::new();

        mixer.set_master_gain(5.0);
        assert_eq!(mixer.master_gain(), 2.0);
        mixer.set_master_gain(-1.0);
        assert_eq!(mixer.master_gain(), 0.0);

        mixer.set_band_gain(FrequencyBand::Low, 3.0);
        assert_eq!(mixer.band_gain(FrequencyBand::Low), 2.0);

        mixer.set_slot_gain(0, -0.5);
        assert_eq!(mixer.slot_gain(0), 0.0);
        mixer.set_slot_gain(0, 10.0);
        assert_eq!(mixer.slot_gain(0), 2.0);
    }

    // ── classify_band ───────────────────────────────────────────────

    #[test]
    fn test_classify_band() {
        assert_eq!(TactileMixer::classify_band(10.0), FrequencyBand::Low);
        assert_eq!(TactileMixer::classify_band(39.9), FrequencyBand::Low);
        assert_eq!(TactileMixer::classify_band(40.0), FrequencyBand::Mid);
        assert_eq!(TactileMixer::classify_band(100.0), FrequencyBand::Mid);
        assert_eq!(TactileMixer::classify_band(149.9), FrequencyBand::Mid);
        assert_eq!(TactileMixer::classify_band(150.0), FrequencyBand::High);
        assert_eq!(TactileMixer::classify_band(500.0), FrequencyBand::High);
    }

    // ── zero-allocation ─────────────────────────────────────────────

    #[test]
    fn test_mixer_fixed_size() {
        let size = std::mem::size_of::<TactileMixer>();
        assert!(size > 0);
        // Must be engine + slot_gains + master + band_gains — no heap
        let expected = std::mem::size_of::<TactileEngine>()
            + std::mem::size_of::<[f64; MAX_EFFECTS]>()
            + std::mem::size_of::<f64>()
            + std::mem::size_of::<[f64; 3]>();
        assert_eq!(size, expected, "mixer should have no hidden heap fields");
    }

    #[test]
    fn test_sustained_mixing() {
        let mut mixer = TactileMixer::new();
        for i in 0..5_000u32 {
            if mixer.active_count() < MAX_EFFECTS {
                mixer.add_effect(TactileEffect::Rumble {
                    frequency_hz: 50.0,
                    amplitude: 0.05,
                    duration_ticks: 10 + (i % 20),
                });
            }
            let out = mixer.tick();
            assert!((-1.0..=1.0).contains(&out.combined));
        }
    }
}
