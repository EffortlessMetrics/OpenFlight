// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! FFB effect mixer — combines multiple synthesis effects with priority
//!
//! [`EffectMixer`] holds a fixed-size array of active effect slots (RT-safe,
//! zero-allocation). Effects are summed and clamped to −1.0…+1.0. When the
//! combined force exceeds the budget, higher-priority effects take precedence.

use crate::effect_library::SynthEffect;

/// Maximum number of concurrent effects in the mixer.
pub const MAX_MIXER_EFFECTS: usize = 16;

/// A slot in the effect mixer.
#[derive(Clone)]
struct EffectSlot {
    /// Boxed trait object is *not* used — we store a function pointer + state
    /// captured via an enum. See [`MixerEntry`].
    entry: MixerEntry,
    /// Priority (higher = more important). Range: 0..=255.
    priority: u8,
    /// Per-effect gain multiplier, 0.0 to 1.0.
    gain: f64,
    /// Unique ID for removal.
    id: u32,
    /// Whether the slot is occupied.
    active: bool,
}

impl Default for EffectSlot {
    fn default() -> Self {
        Self {
            entry: MixerEntry::Empty,
            priority: 0,
            gain: 1.0,
            id: 0,
            active: false,
        }
    }
}

/// RT-safe enum capturing all supported effect types inline (no heap).
#[derive(Clone)]
enum MixerEntry {
    Empty,
    Constant(crate::effect_library::ConstantForce),
    Spring(crate::effect_library::SpringForce),
    Damper(crate::effect_library::DamperForce),
    Friction(crate::effect_library::FrictionForce),
    Periodic(crate::effect_library::PeriodicForce),
}

impl MixerEntry {
    #[inline]
    fn compute(&self, position: f64, velocity: f64, dt_s: f64) -> f64 {
        match self {
            MixerEntry::Empty => 0.0,
            MixerEntry::Constant(e) => e.compute(position, velocity, dt_s),
            MixerEntry::Spring(e) => e.compute(position, velocity, dt_s),
            MixerEntry::Damper(e) => e.compute(position, velocity, dt_s),
            MixerEntry::Friction(e) => e.compute(position, velocity, dt_s),
            MixerEntry::Periodic(e) => e.compute(position, velocity, dt_s),
        }
    }
}

/// Fixed-size effect mixer with priority-based combination.
///
/// # RT Safety
/// All storage is inline (fixed arrays, no heap). Suitable for 250 Hz hot path.
pub struct EffectMixer {
    slots: [EffectSlot; MAX_MIXER_EFFECTS],
    next_id: u32,
}

impl Default for EffectMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectMixer {
    /// Create an empty mixer.
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| EffectSlot::default()),
            next_id: 1,
        }
    }

    /// Number of active effects.
    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.active).count()
    }

    /// Add a constant-force effect. Returns the assigned ID, or `None` if full.
    pub fn add_constant(
        &mut self,
        effect: crate::effect_library::ConstantForce,
        priority: u8,
        gain: f64,
    ) -> Option<u32> {
        self.insert(MixerEntry::Constant(effect), priority, gain)
    }

    /// Add a spring-force effect.
    pub fn add_spring(
        &mut self,
        effect: crate::effect_library::SpringForce,
        priority: u8,
        gain: f64,
    ) -> Option<u32> {
        self.insert(MixerEntry::Spring(effect), priority, gain)
    }

    /// Add a damper-force effect.
    pub fn add_damper(
        &mut self,
        effect: crate::effect_library::DamperForce,
        priority: u8,
        gain: f64,
    ) -> Option<u32> {
        self.insert(MixerEntry::Damper(effect), priority, gain)
    }

    /// Add a friction-force effect.
    pub fn add_friction(
        &mut self,
        effect: crate::effect_library::FrictionForce,
        priority: u8,
        gain: f64,
    ) -> Option<u32> {
        self.insert(MixerEntry::Friction(effect), priority, gain)
    }

    /// Add a periodic-force effect.
    pub fn add_periodic(
        &mut self,
        effect: crate::effect_library::PeriodicForce,
        priority: u8,
        gain: f64,
    ) -> Option<u32> {
        self.insert(MixerEntry::Periodic(effect), priority, gain)
    }

    /// Remove an effect by its ID. Returns `true` if found and removed.
    pub fn remove_effect(&mut self, id: u32) -> bool {
        for slot in &mut self.slots {
            if slot.active && slot.id == id {
                slot.active = false;
                slot.entry = MixerEntry::Empty;
                return true;
            }
        }
        false
    }

    /// Remove all effects.
    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.active = false;
            slot.entry = MixerEntry::Empty;
        }
    }

    /// Compute combined force from all active effects with priority weighting.
    ///
    /// Effects are sorted by priority (descending). Forces are accumulated
    /// until the budget (1.0) is reached; lower-priority effects are scaled
    /// down when the total would exceed the budget.
    #[inline]
    pub fn compute_combined(&self, position: f64, velocity: f64, dt_s: f64) -> f64 {
        let pos = if position.is_finite() { position } else { 0.0 };
        let vel = if velocity.is_finite() { velocity } else { 0.0 };
        let dt = if dt_s.is_finite() && dt_s >= 0.0 {
            dt_s
        } else {
            0.0
        };

        // Collect active indices sorted by priority (descending).
        // Fixed-size scratch to avoid allocation.
        let mut indices: [usize; MAX_MIXER_EFFECTS] = [0; MAX_MIXER_EFFECTS];
        let mut count = 0usize;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.active {
                indices[count] = i;
                count += 1;
            }
        }

        // Simple insertion sort on the small array (max 16 elements).
        for i in 1..count {
            let mut j = i;
            while j > 0 && self.slots[indices[j]].priority > self.slots[indices[j - 1]].priority {
                indices.swap(j, j - 1);
                j -= 1;
            }
        }

        let mut total = 0.0_f64;
        let mut budget_remaining = 1.0_f64;

        for &idx in &indices[..count] {
            let slot = &self.slots[idx];
            let raw = slot.entry.compute(pos, vel, dt) * slot.gain.clamp(0.0, 1.0);
            let abs_raw = raw.abs();

            if budget_remaining <= 0.0 {
                break;
            }

            if abs_raw <= budget_remaining {
                total += raw;
                budget_remaining -= abs_raw;
            } else {
                // Scale this effect to fit remaining budget.
                let scale = budget_remaining / abs_raw;
                total += raw * scale;
                budget_remaining = 0.0;
            }
        }

        total.clamp(-1.0, 1.0)
    }

    // Internal: find a free slot and insert.
    fn insert(&mut self, entry: MixerEntry, priority: u8, gain: f64) -> Option<u32> {
        for slot in &mut self.slots {
            if !slot.active {
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1);
                slot.entry = entry;
                slot.priority = priority;
                slot.gain = gain.clamp(0.0, 1.0);
                slot.id = id;
                slot.active = true;
                return Some(id);
            }
        }
        None
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_library::*;

    const EPS: f64 = 1e-6;

    #[test]
    fn empty_mixer_returns_zero() {
        let m = EffectMixer::new();
        assert!(m.compute_combined(0.0, 0.0, 0.004).abs() < EPS);
    }

    #[test]
    fn single_constant_passthrough() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(0.6), 100, 1.0);
        let f = m.compute_combined(0.0, 0.0, 0.004);
        assert!((f - 0.6).abs() < EPS);
    }

    #[test]
    fn two_effects_sum() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(0.3), 100, 1.0);
        m.add_constant(ConstantForce::new(0.4), 100, 1.0);
        let f = m.compute_combined(0.0, 0.0, 0.004);
        assert!((f - 0.7).abs() < EPS);
    }

    #[test]
    fn clamping_at_one() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(0.8), 100, 1.0);
        m.add_constant(ConstantForce::new(0.8), 100, 1.0);
        let f = m.compute_combined(0.0, 0.0, 0.004);
        // Budget system limits to 1.0
        assert!((f - 1.0).abs() < EPS);
    }

    #[test]
    fn priority_higher_gets_full_budget() {
        let mut m = EffectMixer::new();
        // High priority fills budget
        m.add_constant(ConstantForce::new(0.9), 200, 1.0);
        // Low priority gets scaled down
        m.add_constant(ConstantForce::new(0.5), 50, 1.0);
        let f = m.compute_combined(0.0, 0.0, 0.004);
        // 0.9 fills budget, 0.5 gets scaled to fit remaining 0.1
        assert!((f - 1.0).abs() < EPS);
    }

    #[test]
    fn remove_effect() {
        let mut m = EffectMixer::new();
        let id = m.add_constant(ConstantForce::new(0.5), 100, 1.0).unwrap();
        assert_eq!(m.active_count(), 1);
        assert!(m.remove_effect(id));
        assert_eq!(m.active_count(), 0);
        assert!(m.compute_combined(0.0, 0.0, 0.004).abs() < EPS);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut m = EffectMixer::new();
        assert!(!m.remove_effect(999));
    }

    #[test]
    fn clear_removes_all() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(0.3), 100, 1.0);
        m.add_constant(ConstantForce::new(0.4), 100, 1.0);
        assert_eq!(m.active_count(), 2);
        m.clear();
        assert_eq!(m.active_count(), 0);
        assert!(m.compute_combined(0.0, 0.0, 0.004).abs() < EPS);
    }

    #[test]
    fn gain_scales_effect() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(1.0), 100, 0.5);
        let f = m.compute_combined(0.0, 0.0, 0.004);
        assert!((f - 0.5).abs() < EPS);
    }

    #[test]
    fn full_mixer_returns_none() {
        let mut m = EffectMixer::new();
        for i in 0..MAX_MIXER_EFFECTS {
            assert!(
                m.add_constant(ConstantForce::new(0.01), 100, 1.0).is_some(),
                "slot {i}"
            );
        }
        assert!(m.add_constant(ConstantForce::new(0.01), 100, 1.0).is_none());
    }

    #[test]
    fn spring_in_mixer() {
        let mut m = EffectMixer::new();
        m.add_spring(SpringForce::new(0.0, 1.0, 0.0, 1.0), 100, 1.0);
        let f = m.compute_combined(0.5, 0.0, 0.004);
        assert!(f < 0.0, "spring should push back");
    }

    #[test]
    fn damper_in_mixer() {
        let mut m = EffectMixer::new();
        m.add_damper(DamperForce::new(1.0, 1.0), 100, 1.0);
        let f = m.compute_combined(0.0, 0.5, 0.004);
        assert!(f < 0.0, "damper should resist motion");
    }

    #[test]
    fn friction_in_mixer() {
        let mut m = EffectMixer::new();
        m.add_friction(FrictionForce::new(0.4, 1.0), 100, 1.0);
        let f = m.compute_combined(0.0, 1.0, 0.004);
        assert!((f - -0.4).abs() < EPS);
    }

    #[test]
    fn periodic_in_mixer() {
        let mut m = EffectMixer::new();
        m.add_periodic(
            PeriodicForce::new(SynthWaveform::Sine, 1.0, 1.0, 0.0, 0.0),
            100,
            1.0,
        );
        // At dt=0.25 → sin(π/2) = 1.0
        let f = m.compute_combined(0.0, 0.0, 0.25);
        assert!((f - 1.0).abs() < 1e-4);
    }

    #[test]
    fn nan_inputs_return_finite() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(0.5), 100, 1.0);
        let f = m.compute_combined(f64::NAN, f64::NAN, f64::NAN);
        assert!(f.is_finite());
    }

    #[test]
    fn negative_clamping() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(-0.8), 100, 1.0);
        m.add_constant(ConstantForce::new(-0.8), 100, 1.0);
        let f = m.compute_combined(0.0, 0.0, 0.004);
        assert!((f - -1.0).abs() < EPS);
    }

    #[test]
    fn mixed_positive_negative_cancel() {
        let mut m = EffectMixer::new();
        m.add_constant(ConstantForce::new(0.5), 100, 1.0);
        m.add_constant(ConstantForce::new(-0.5), 100, 1.0);
        let f = m.compute_combined(0.0, 0.0, 0.004);
        assert!(f.abs() < EPS);
    }
}
