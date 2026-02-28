// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Zero-allocation tactile feedback engine
//!
//! Provides a fixed-slot effect engine for transducer output (ButtKicker,
//! bass shakers, rumble motors). All processing uses pre-allocated arrays
//! with no heap allocation on the hot path.

use std::f64::consts::PI;

/// Maximum number of simultaneous effects.
pub const MAX_EFFECTS: usize = 16;

/// Processing tick rate in Hz (matches the RT spine).
pub const TICK_RATE_HZ: f64 = 250.0;

/// Amplitude below which an impact effect is considered expired.
const IMPACT_EXPIRE_THRESHOLD: f64 = 0.001;

/// Texture waveform pattern for repeating effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TexturePattern {
    /// Rising sawtooth wave.
    Sawtooth,
    /// Square wave (50 % duty cycle).
    Square,
    /// Triangle wave.
    Triangle,
}

/// A parameterised tactile effect that can be queued in the engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TactileEffect {
    /// Continuous vibration at a fixed frequency for a bounded duration.
    Rumble {
        frequency_hz: f64,
        amplitude: f64,
        duration_ticks: u32,
    },
    /// Single-hit impulse with exponential decay (landing, weapon fire).
    Impact { magnitude: f64, decay_rate: f64 },
    /// Repeating waveform texture (runway surface, turbulence).
    Texture {
        frequency_hz: f64,
        amplitude: f64,
        pattern: TexturePattern,
    },
    /// Engine vibration — sine wave whose frequency tracks RPM.
    Engine { rpm: f64, amplitude: f64 },
}

impl TactileEffect {
    /// Dominant frequency of the effect in Hz.
    #[inline]
    pub fn frequency_hz(&self) -> f64 {
        match *self {
            Self::Rumble { frequency_hz, .. } | Self::Texture { frequency_hz, .. } => frequency_hz,
            Self::Impact { .. } => 20.0,
            Self::Engine { rpm, .. } => rpm / 60.0,
        }
    }
}

// ── Internal slot ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub(crate) struct EffectSlot {
    pub(crate) effect: TactileEffect,
    pub(crate) elapsed_ticks: u32,
    pub(crate) active: bool,
}

impl EffectSlot {
    pub(crate) const EMPTY: Self = Self {
        effect: TactileEffect::Rumble {
            frequency_hz: 0.0,
            amplitude: 0.0,
            duration_ticks: 0,
        },
        elapsed_ticks: 0,
        active: false,
    };

    #[inline]
    pub(crate) fn compute_sample(&self) -> f64 {
        let t = self.elapsed_ticks as f64 / TICK_RATE_HZ;
        match self.effect {
            TactileEffect::Rumble {
                frequency_hz,
                amplitude,
                ..
            } => amplitude * (2.0 * PI * frequency_hz * t).sin(),

            TactileEffect::Impact {
                magnitude,
                decay_rate,
            } => magnitude * (-decay_rate * t).exp(),

            TactileEffect::Texture {
                frequency_hz,
                amplitude,
                pattern,
            } => {
                let phase = (frequency_hz * t).fract();
                let wave = match pattern {
                    TexturePattern::Sawtooth => 2.0 * phase - 1.0,
                    TexturePattern::Square => {
                        if phase < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    TexturePattern::Triangle => {
                        if phase < 0.5 {
                            4.0 * phase - 1.0
                        } else {
                            3.0 - 4.0 * phase
                        }
                    }
                };
                amplitude * wave
            }

            TactileEffect::Engine { rpm, amplitude } => {
                let freq = rpm / 60.0;
                amplitude * (2.0 * PI * freq * t).sin()
            }
        }
    }

    #[inline]
    pub(crate) fn is_expired(&self) -> bool {
        match self.effect {
            TactileEffect::Rumble { duration_ticks, .. } => self.elapsed_ticks >= duration_ticks,
            TactileEffect::Impact {
                magnitude,
                decay_rate,
            } => {
                let t = self.elapsed_ticks as f64 / TICK_RATE_HZ;
                (magnitude * (-decay_rate * t).exp()).abs() < IMPACT_EXPIRE_THRESHOLD
            }
            TactileEffect::Texture { .. } | TactileEffect::Engine { .. } => false,
        }
    }
}

// ── Public engine ────────────────────────────────────────────────────

/// Zero-allocation tactile feedback engine with pre-allocated effect slots.
///
/// All state is held in a fixed-size array — no heap allocation occurs on
/// the hot path (`tick`, `add_effect`, `clear`).
pub struct TactileEngine {
    pub(crate) slots: [EffectSlot; MAX_EFFECTS],
    pub(crate) tick_count: u64,
}

impl TactileEngine {
    /// Create a new engine with all slots empty.
    pub fn new() -> Self {
        Self {
            slots: [EffectSlot::EMPTY; MAX_EFFECTS],
            tick_count: 0,
        }
    }

    /// Queue a tactile effect. Returns the slot index, or `None` if full.
    pub fn add_effect(&mut self, effect: TactileEffect) -> Option<usize> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if !slot.active {
                *slot = EffectSlot {
                    effect,
                    elapsed_ticks: 0,
                    active: true,
                };
                return Some(i);
            }
        }
        None
    }

    /// Advance one tick and return the combined output clamped to \[-1.0, 1.0\].
    pub fn tick(&mut self) -> f64 {
        let mut output = 0.0_f64;
        for slot in &mut self.slots {
            if slot.active {
                output += slot.compute_sample();
                slot.elapsed_ticks += 1;
                if slot.is_expired() {
                    slot.active = false;
                }
            }
        }
        self.tick_count += 1;
        output.clamp(-1.0, 1.0)
    }

    /// Stop all effects immediately.
    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.active = false;
        }
    }

    /// Remove a specific effect by slot index.
    pub fn remove_effect(&mut self, slot: usize) {
        if slot < MAX_EFFECTS {
            self.slots[slot].active = false;
        }
    }

    /// Number of currently active effects.
    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.active).count()
    }

    /// Total ticks processed so far.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
}

impl Default for TactileEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── construction ────────────────────────────────────────────────

    #[test]
    fn test_engine_creation() {
        let engine = TactileEngine::new();
        assert_eq!(engine.active_count(), 0);
        assert_eq!(engine.tick_count(), 0);
    }

    #[test]
    fn test_empty_engine_tick() {
        let mut engine = TactileEngine::new();
        assert_eq!(engine.tick(), 0.0, "empty engine must produce zero");
    }

    // ── add / remove / clear ────────────────────────────────────────

    #[test]
    fn test_add_effect() {
        let mut engine = TactileEngine::new();
        let slot = engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 0.5,
            duration_ticks: 100,
        });
        assert_eq!(slot, Some(0));
        assert_eq!(engine.active_count(), 1);
    }

    #[test]
    fn test_add_effect_full() {
        let mut engine = TactileEngine::new();
        for i in 0..MAX_EFFECTS {
            assert_eq!(
                engine.add_effect(TactileEffect::Rumble {
                    frequency_hz: 50.0,
                    amplitude: 0.1,
                    duration_ticks: 1000,
                }),
                Some(i)
            );
        }
        assert_eq!(
            engine.add_effect(TactileEffect::Impact {
                magnitude: 1.0,
                decay_rate: 5.0
            }),
            None
        );
        assert_eq!(engine.active_count(), MAX_EFFECTS);
    }

    #[test]
    fn test_clear() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 0.5,
            duration_ticks: 100,
        });
        engine.add_effect(TactileEffect::Impact {
            magnitude: 1.0,
            decay_rate: 5.0,
        });
        assert_eq!(engine.active_count(), 2);
        engine.clear();
        assert_eq!(engine.active_count(), 0);
    }

    #[test]
    fn test_remove_effect() {
        let mut engine = TactileEngine::new();
        let slot = engine
            .add_effect(TactileEffect::Rumble {
                frequency_hz: 50.0,
                amplitude: 0.5,
                duration_ticks: 100,
            })
            .unwrap();
        engine.remove_effect(slot);
        assert_eq!(engine.active_count(), 0);
    }

    // ── individual effect types ─────────────────────────────────────

    #[test]
    fn test_rumble_output() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Rumble {
            frequency_hz: 50.0,
            amplitude: 0.5,
            duration_ticks: 25,
        });

        // t = 0  →  sin(0) = 0
        let first = engine.tick();
        assert!(first.abs() < 0.01, "sine at t=0 should be ~0");

        let mut max_abs = 0.0_f64;
        for _ in 1..25 {
            max_abs = max_abs.max(engine.tick().abs());
        }
        assert!(max_abs > 0.1, "rumble must produce non-zero output");
        assert_eq!(
            engine.active_count(),
            0,
            "rumble must expire after duration"
        );
    }

    #[test]
    fn test_impact_output_and_decay() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Impact {
            magnitude: 1.0,
            decay_rate: 10.0,
        });

        let first = engine.tick();
        assert!(
            (first - 1.0).abs() < 0.01,
            "impact first sample should be ~magnitude"
        );

        let second = engine.tick();
        assert!(second < first, "impact must decay");

        // Run until expired
        for _ in 0..2000 {
            engine.tick();
            if engine.active_count() == 0 {
                break;
            }
        }
        assert_eq!(engine.active_count(), 0, "impact must eventually expire");
    }

    #[test]
    fn test_texture_sawtooth() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Texture {
            frequency_hz: 50.0,
            amplitude: 0.8,
            pattern: TexturePattern::Sawtooth,
        });

        let mut samples = [0.0_f64; 50];
        for s in &mut samples {
            *s = engine.tick();
        }
        let range = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - samples.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(range > 0.5, "sawtooth should swing widely");
        assert_eq!(engine.active_count(), 1, "texture must not self-expire");
    }

    #[test]
    fn test_texture_square() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Texture {
            frequency_hz: 25.0,
            amplitude: 1.0,
            pattern: TexturePattern::Square,
        });

        let mut samples = [0.0_f64; 20];
        for s in &mut samples {
            *s = engine.tick();
        }
        assert!(
            samples.iter().any(|&s| s > 0.5),
            "square needs positive half"
        );
        assert!(
            samples.iter().any(|&s| s < -0.5),
            "square needs negative half"
        );
    }

    #[test]
    fn test_texture_triangle() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Texture {
            frequency_hz: 25.0,
            amplitude: 1.0,
            pattern: TexturePattern::Triangle,
        });

        for _ in 0..40 {
            let s = engine.tick();
            assert!((-1.0..=1.0).contains(&s), "triangle must be bounded");
        }
    }

    #[test]
    fn test_engine_vibration() {
        let mut engine = TactileEngine::new();
        engine.add_effect(TactileEffect::Engine {
            rpm: 2400.0,
            amplitude: 0.3,
        });

        let first = engine.tick();
        assert!(first.abs() < 0.01, "sin(0) ≈ 0");

        let mut max_abs = 0.0_f64;
        for _ in 0..50 {
            max_abs = max_abs.max(engine.tick().abs());
        }
        assert!(max_abs > 0.1, "engine effect must produce output");
        assert!(
            max_abs <= 0.3 + 1e-9,
            "engine output must not exceed amplitude"
        );
        assert_eq!(engine.active_count(), 1, "engine must not self-expire");
    }

    // ── lifecycle ───────────────────────────────────────────────────

    #[test]
    fn test_effect_lifecycle_add_play_expire_reuse() {
        let mut engine = TactileEngine::new();
        let slot = engine
            .add_effect(TactileEffect::Rumble {
                frequency_hz: 50.0,
                amplitude: 0.5,
                duration_ticks: 5,
            })
            .unwrap();

        for _ in 0..5 {
            engine.tick();
        }
        assert_eq!(engine.active_count(), 0, "should have expired");

        let reused = engine
            .add_effect(TactileEffect::Impact {
                magnitude: 0.8,
                decay_rate: 10.0,
            })
            .unwrap();
        assert_eq!(reused, slot, "expired slot must be reused");
    }

    // ── clamping ────────────────────────────────────────────────────

    #[test]
    fn test_output_clamping() {
        let mut engine = TactileEngine::new();
        for _ in 0..MAX_EFFECTS {
            engine.add_effect(TactileEffect::Impact {
                magnitude: 1.0,
                decay_rate: 0.1,
            });
        }
        let sample = engine.tick();
        assert!((-1.0..=1.0).contains(&sample), "output must be clamped");
    }

    // ── tick counter ────────────────────────────────────────────────

    #[test]
    fn test_tick_count() {
        let mut engine = TactileEngine::new();
        for _ in 0..100 {
            engine.tick();
        }
        assert_eq!(engine.tick_count(), 100);
    }

    // ── frequency helpers ───────────────────────────────────────────

    #[test]
    fn test_effect_frequency_hz() {
        assert_eq!(
            TactileEffect::Rumble {
                frequency_hz: 50.0,
                amplitude: 0.5,
                duration_ticks: 10
            }
            .frequency_hz(),
            50.0
        );
        assert_eq!(
            TactileEffect::Impact {
                magnitude: 1.0,
                decay_rate: 10.0
            }
            .frequency_hz(),
            20.0
        );
        assert_eq!(
            TactileEffect::Texture {
                frequency_hz: 80.0,
                amplitude: 0.5,
                pattern: TexturePattern::Triangle,
            }
            .frequency_hz(),
            80.0
        );
        assert!(
            (TactileEffect::Engine {
                rpm: 1200.0,
                amplitude: 0.3
            }
            .frequency_hz()
                - 20.0)
                .abs()
                < 0.01
        );
    }

    // ── zero-allocation verification ────────────────────────────────

    #[test]
    fn test_fixed_size_layout() {
        let slot_size = std::mem::size_of::<EffectSlot>();
        let expected = MAX_EFFECTS * slot_size + std::mem::size_of::<u64>();
        assert_eq!(
            std::mem::size_of::<TactileEngine>(),
            expected,
            "engine size must be exactly slots + tick_count (no heap indirection)"
        );
    }

    #[test]
    fn test_effect_is_copy() {
        let a = TactileEffect::Rumble {
            frequency_hz: 1.0,
            amplitude: 0.5,
            duration_ticks: 10,
        };
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn test_sustained_ticking_no_growth() {
        let mut engine = TactileEngine::new();
        for i in 0..5_000u32 {
            if engine.active_count() < MAX_EFFECTS {
                engine.add_effect(TactileEffect::Rumble {
                    frequency_hz: 50.0,
                    amplitude: 0.05,
                    duration_ticks: 10 + (i % 20),
                });
            }
            let v = engine.tick();
            assert!((-1.0..=1.0).contains(&v));
        }
    }
}
