// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis force trim system
//!
//! [`TrimState`] tracks trim offsets for up to [`MAX_TRIM_AXES`] axes and
//! adjusts force output so that the trimmed position feels like neutral.
//!
//! # RT Safety
//! All storage is inline fixed-size arrays — zero-allocation on the hot path.

/// Maximum number of axes supported by the trim system.
pub const MAX_TRIM_AXES: usize = 8;

/// Per-axis force trim state.
///
/// Stores trim offsets and applies them to force output so that the pilot's
/// current stick position becomes the new neutral.
pub struct SynthTrimState {
    /// Trim offset per axis, −1.0 to +1.0.
    offsets: [f64; MAX_TRIM_AXES],
}

impl Default for SynthTrimState {
    fn default() -> Self {
        Self::new()
    }
}

impl SynthTrimState {
    /// Create a new trim state with all axes centred (offset = 0).
    pub fn new() -> Self {
        Self {
            offsets: [0.0; MAX_TRIM_AXES],
        }
    }

    /// Set the trim offset for an axis. Clamped to −1.0…+1.0.
    ///
    /// Returns `false` if `axis` is out of range.
    pub fn set_trim(&mut self, axis: usize, offset: f64) -> bool {
        if axis >= MAX_TRIM_AXES {
            return false;
        }
        let val = if offset.is_finite() {
            offset.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.offsets[axis] = val;
        true
    }

    /// Clear the trim for a single axis (reset to centre).
    ///
    /// Returns `false` if `axis` is out of range.
    pub fn clear_trim(&mut self, axis: usize) -> bool {
        if axis >= MAX_TRIM_AXES {
            return false;
        }
        self.offsets[axis] = 0.0;
        true
    }

    /// Clear all trim offsets.
    pub fn clear_all(&mut self) {
        self.offsets = [0.0; MAX_TRIM_AXES];
    }

    /// Get the current trim offset for an axis.
    pub fn get_trim(&self, axis: usize) -> f64 {
        if axis >= MAX_TRIM_AXES {
            return 0.0;
        }
        self.offsets[axis]
    }

    /// Apply trim to a force value for the given axis.
    ///
    /// The trim offset shifts the neutral point: the returned force is adjusted
    /// so that the trimmed position experiences zero residual force, while
    /// displacement from the trim point still generates proportional force.
    ///
    /// Output is clamped to −1.0…+1.0.
    #[inline]
    pub fn apply_trim(&self, axis: usize, force: f64) -> f64 {
        if axis >= MAX_TRIM_AXES {
            return clamp_force(force);
        }
        let f = if force.is_finite() { force } else { 0.0 };
        clamp_force(f + self.offsets[axis])
    }
}

#[inline]
fn clamp_force(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(-1.0, 1.0)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    #[test]
    fn default_offsets_are_zero() {
        let t = SynthTrimState::new();
        for axis in 0..MAX_TRIM_AXES {
            assert!(t.get_trim(axis).abs() < EPS);
        }
    }

    #[test]
    fn set_and_get_trim() {
        let mut t = SynthTrimState::new();
        assert!(t.set_trim(0, 0.3));
        assert!((t.get_trim(0) - 0.3).abs() < EPS);
    }

    #[test]
    fn trim_clamps_to_range() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, 2.0);
        assert!((t.get_trim(0) - 1.0).abs() < EPS);
        t.set_trim(0, -5.0);
        assert!((t.get_trim(0) - -1.0).abs() < EPS);
    }

    #[test]
    fn clear_trim_resets() {
        let mut t = SynthTrimState::new();
        t.set_trim(1, 0.7);
        assert!(t.clear_trim(1));
        assert!(t.get_trim(1).abs() < EPS);
    }

    #[test]
    fn clear_all_resets_everything() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, 0.5);
        t.set_trim(3, -0.4);
        t.clear_all();
        for axis in 0..MAX_TRIM_AXES {
            assert!(t.get_trim(axis).abs() < EPS);
        }
    }

    #[test]
    fn apply_trim_shifts_force() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, 0.2);
        // Force of 0.0 → offset of +0.2
        let f = t.apply_trim(0, 0.0);
        assert!((f - 0.2).abs() < EPS);
    }

    #[test]
    fn apply_trim_clamps_output() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, 0.5);
        // 0.8 + 0.5 = 1.3 → clamped to 1.0
        let f = t.apply_trim(0, 0.8);
        assert!((f - 1.0).abs() < EPS);
    }

    #[test]
    fn apply_trim_negative() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, -0.3);
        let f = t.apply_trim(0, -0.8);
        // -0.8 + -0.3 = -1.1 → clamped to -1.0
        assert!((f - -1.0).abs() < EPS);
    }

    #[test]
    fn apply_trim_no_offset_passthrough() {
        let t = SynthTrimState::new();
        let f = t.apply_trim(0, 0.5);
        assert!((f - 0.5).abs() < EPS);
    }

    #[test]
    fn out_of_range_axis_set() {
        let mut t = SynthTrimState::new();
        assert!(!t.set_trim(MAX_TRIM_AXES, 0.5));
    }

    #[test]
    fn out_of_range_axis_clear() {
        let mut t = SynthTrimState::new();
        assert!(!t.clear_trim(MAX_TRIM_AXES));
    }

    #[test]
    fn out_of_range_axis_get() {
        let t = SynthTrimState::new();
        assert!(t.get_trim(MAX_TRIM_AXES).abs() < EPS);
    }

    #[test]
    fn out_of_range_axis_apply() {
        let t = SynthTrimState::new();
        let f = t.apply_trim(MAX_TRIM_AXES, 0.5);
        assert!((f - 0.5).abs() < EPS);
    }

    #[test]
    fn nan_offset_sanitised() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, f64::NAN);
        assert!(t.get_trim(0).abs() < EPS);
    }

    #[test]
    fn inf_offset_sanitised() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, f64::INFINITY);
        // Infinity is not finite → set to 0.0
        assert!(t.get_trim(0).abs() < EPS);
    }

    #[test]
    fn nan_force_sanitised() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, 0.3);
        let f = t.apply_trim(0, f64::NAN);
        // NaN force → 0.0, then + 0.3
        assert!((f - 0.3).abs() < EPS);
    }

    #[test]
    fn multiple_axes_independent() {
        let mut t = SynthTrimState::new();
        t.set_trim(0, 0.1);
        t.set_trim(1, -0.2);
        t.set_trim(2, 0.3);
        assert!((t.apply_trim(0, 0.0) - 0.1).abs() < EPS);
        assert!((t.apply_trim(1, 0.0) - -0.2).abs() < EPS);
        assert!((t.apply_trim(2, 0.0) - 0.3).abs() < EPS);
    }
}
