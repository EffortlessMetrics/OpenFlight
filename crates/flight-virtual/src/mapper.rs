// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Input-to-output mapping layer for virtual devices.
//!
//! [`VirtualDeviceMapper`] sits between physical input events and one or
//! more [`VirtualBackend`]s, applying transformations (curves, deadzones,
//! button modes) on the way through.
//!
//! # Mapping modes
//!
//! * **Many → one (merge):** multiple physical axes feed a single virtual
//!   axis.  The merge strategy is configurable (first-wins, sum, max).
//! * **One → many (split):** a single physical axis fans out to several
//!   virtual axes (each with its own transform).
//! * **Toggle / pulse buttons:** a physical momentary button can be mapped
//!   as a latching toggle or as a fixed-duration pulse on the virtual side.

use crate::backend::{HatDirection, VirtualBackend, VirtualBackendError};
use serde::{Deserialize, Serialize};

// ── Axis transformation ─────────────────────────────────────────────

/// Defines how a physical axis value is transformed before it reaches
/// the virtual device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisTransform {
    /// Dead-zone around center (0.0 – 1.0).  Values within this band
    /// are snapped to zero.
    pub deadzone: f32,
    /// Scale factor applied after dead-zone removal.
    pub scale: f32,
    /// Constant offset added after scaling.
    pub offset: f32,
    /// If `true`, invert the axis (multiply by −1 *before* scale).
    pub invert: bool,
}

impl Default for AxisTransform {
    fn default() -> Self {
        Self {
            deadzone: 0.0,
            scale: 1.0,
            offset: 0.0,
            invert: false,
        }
    }
}

impl AxisTransform {
    /// Apply the transformation to a raw `[-1, 1]` input value.
    pub fn apply(&self, mut value: f32) -> f32 {
        if self.invert {
            value = -value;
        }

        // Dead-zone
        if value.abs() < self.deadzone {
            value = 0.0;
        } else {
            // Re-scale the remaining range so full deflection still reaches ±1.
            let sign = value.signum();
            value = sign * (value.abs() - self.deadzone) / (1.0 - self.deadzone);
        }

        let result = value * self.scale + self.offset;
        result.clamp(-1.0, 1.0)
    }
}

// ── Button mode ─────────────────────────────────────────────────────

/// How a physical button press translates to the virtual button.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonMode {
    /// 1-to-1: pressed when held, released when released.
    #[default]
    Direct,
    /// Latching toggle: each press flips the virtual state.
    Toggle,
    /// Fixed-length pulse: pressing produces a virtual press for the
    /// given number of update ticks, regardless of how long the
    /// physical button is held.
    Pulse { ticks: u16 },
}

// ── Merge strategy ──────────────────────────────────────────────────

/// Strategy when multiple physical axes map to the same virtual axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// First non-zero value wins.
    #[default]
    FirstNonZero,
    /// Sum all inputs (clamped to ±1).
    Sum,
    /// Use the value with the largest absolute magnitude.
    MaxAbs,
}

// ── Mapping descriptors ─────────────────────────────────────────────

/// A single axis mapping entry.
#[derive(Debug, Clone)]
pub struct AxisMapping {
    /// Source physical axis index.
    pub src_axis: u8,
    /// Target virtual axis index.
    pub dst_axis: u8,
    /// Transformation applied to the value.
    pub transform: AxisTransform,
}

/// A single button mapping entry.
#[derive(Debug, Clone)]
pub struct ButtonMapping {
    /// Source physical button index.
    pub src_button: u8,
    /// Target virtual button index.
    pub dst_button: u8,
    /// How presses are translated.
    pub mode: ButtonMode,
}

/// A single hat mapping entry.
#[derive(Debug, Clone)]
pub struct HatMapping {
    /// Source physical hat index.
    pub src_hat: u8,
    /// Target virtual hat index.
    pub dst_hat: u8,
}

// ── Mapper ──────────────────────────────────────────────────────────

/// Maps physical input events to one or more virtual devices, applying
/// transformations along the way.
pub struct VirtualDeviceMapper<B: VirtualBackend> {
    backend: B,
    axis_mappings: Vec<AxisMapping>,
    button_mappings: Vec<ButtonMapping>,
    hat_mappings: Vec<HatMapping>,
    merge_strategy: MergeStrategy,
    /// Internal toggle state per virtual button (for Toggle mode).
    toggle_state: Vec<bool>,
    /// Previous physical button state to detect edges for toggle/pulse.
    prev_physical_buttons: Vec<bool>,
    /// Remaining pulse ticks per virtual button.
    pulse_remaining: Vec<u16>,
}

impl<B: VirtualBackend> VirtualDeviceMapper<B> {
    /// Create a new mapper wrapping the given backend.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            axis_mappings: Vec::new(),
            button_mappings: Vec::new(),
            hat_mappings: Vec::new(),
            merge_strategy: MergeStrategy::default(),
            toggle_state: Vec::new(),
            prev_physical_buttons: Vec::new(),
            pulse_remaining: Vec::new(),
        }
    }

    /// Borrow the underlying backend (e.g. for reading state).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Mutably borrow the underlying backend.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Set the merge strategy for multi-input axis mappings.
    pub fn set_merge_strategy(&mut self, strategy: MergeStrategy) {
        self.merge_strategy = strategy;
    }

    // ── Mapping registration ────────────────────────────────────────

    /// Add an axis mapping (physical → virtual).
    pub fn add_axis_mapping(&mut self, mapping: AxisMapping) {
        self.axis_mappings.push(mapping);
    }

    /// Add a button mapping.
    pub fn add_button_mapping(&mut self, mapping: ButtonMapping) {
        // Grow internal tracking arrays as needed.
        let max_dst = mapping.dst_button as usize + 1;
        if self.toggle_state.len() < max_dst {
            self.toggle_state.resize(max_dst, false);
            self.pulse_remaining.resize(max_dst, 0);
        }

        let max_src = mapping.src_button as usize + 1;
        if self.prev_physical_buttons.len() < max_src {
            self.prev_physical_buttons.resize(max_src, false);
        }

        self.button_mappings.push(mapping);
    }

    /// Add a hat mapping.
    pub fn add_hat_mapping(&mut self, mapping: HatMapping) {
        self.hat_mappings.push(mapping);
    }

    // ── Update path ─────────────────────────────────────────────────

    /// Push a set of physical axis values through the mapper.
    ///
    /// `physical_axes` is indexed by physical axis id; missing entries
    /// are treated as 0.0.
    pub fn update_axes(&mut self, physical_axes: &[f32]) -> Result<(), VirtualBackendError> {
        // Group mappings by virtual axis to support merge.
        // Use a simple approach: collect transformed values per dst.
        let num_virtual = self.backend.axis_count() as usize;
        let mut buckets: Vec<Vec<f32>> = vec![Vec::new(); num_virtual];

        for mapping in &self.axis_mappings {
            let raw = physical_axes
                .get(mapping.src_axis as usize)
                .copied()
                .unwrap_or(0.0);
            let transformed = mapping.transform.apply(raw);
            if (mapping.dst_axis as usize) < num_virtual {
                buckets[mapping.dst_axis as usize].push(transformed);
            }
        }

        for (dst, values) in buckets.iter().enumerate() {
            if values.is_empty() {
                continue;
            }
            let merged = self.merge(values);
            self.backend.set_axis(dst as u8, merged)?;
        }

        Ok(())
    }

    /// Push a set of physical button states through the mapper.
    ///
    /// `physical_buttons` is indexed by physical button id.
    pub fn update_buttons(&mut self, physical_buttons: &[bool]) -> Result<(), VirtualBackendError> {
        // First, tick pulse counters.
        for (dst, remaining) in self.pulse_remaining.iter_mut().enumerate() {
            if *remaining > 0 {
                *remaining -= 1;
                if *remaining == 0 {
                    self.backend.set_button(dst as u8, false)?;
                }
            }
        }

        for mapping in &self.button_mappings {
            let current = physical_buttons
                .get(mapping.src_button as usize)
                .copied()
                .unwrap_or(false);
            let prev = self
                .prev_physical_buttons
                .get(mapping.src_button as usize)
                .copied()
                .unwrap_or(false);

            let rising_edge = current && !prev;

            match mapping.mode {
                ButtonMode::Direct => {
                    self.backend.set_button(mapping.dst_button, current)?;
                }
                ButtonMode::Toggle => {
                    if rising_edge {
                        let dst = mapping.dst_button as usize;
                        self.toggle_state[dst] = !self.toggle_state[dst];
                        self.backend
                            .set_button(mapping.dst_button, self.toggle_state[dst])?;
                    }
                }
                ButtonMode::Pulse { ticks } => {
                    if rising_edge {
                        self.backend.set_button(mapping.dst_button, true)?;
                        self.pulse_remaining[mapping.dst_button as usize] = ticks;
                    }
                }
            }
        }

        // Store current physical state for next call's edge detection.
        for mapping in &self.button_mappings {
            let src = mapping.src_button as usize;
            let val = physical_buttons.get(src).copied().unwrap_or(false);
            if src < self.prev_physical_buttons.len() {
                self.prev_physical_buttons[src] = val;
            }
        }

        Ok(())
    }

    /// Push a set of physical hat directions through the mapper.
    pub fn update_hats(
        &mut self,
        physical_hats: &[HatDirection],
    ) -> Result<(), VirtualBackendError> {
        for mapping in &self.hat_mappings {
            let dir = physical_hats
                .get(mapping.src_hat as usize)
                .copied()
                .unwrap_or(HatDirection::Centered);
            self.backend.set_hat(mapping.dst_hat, dir)?;
        }
        Ok(())
    }

    // ── Internal ────────────────────────────────────────────────────

    fn merge(&self, values: &[f32]) -> f32 {
        match self.merge_strategy {
            MergeStrategy::FirstNonZero => values
                .iter()
                .copied()
                .find(|v| v.abs() > f32::EPSILON)
                .unwrap_or(0.0),
            MergeStrategy::Sum => {
                let s: f32 = values.iter().sum();
                s.clamp(-1.0, 1.0)
            }
            MergeStrategy::MaxAbs => values
                .iter()
                .copied()
                .max_by(|a, b| a.abs().partial_cmp(&b.abs()).unwrap())
                .unwrap_or(0.0),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    // Helper to build an acquired mapper.
    fn mapper_with_mock() -> VirtualDeviceMapper<MockBackend> {
        let mut backend = MockBackend::joystick();
        backend.acquire().unwrap();
        VirtualDeviceMapper::new(backend)
    }

    // ── AxisTransform unit tests ────────────────────────────────────

    #[test]
    fn test_axis_transform_identity() {
        let t = AxisTransform::default();
        assert!((t.apply(0.5) - 0.5).abs() < f32::EPSILON);
        assert!((t.apply(-1.0) - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_axis_transform_invert() {
        let t = AxisTransform {
            invert: true,
            ..Default::default()
        };
        assert!((t.apply(0.75) - (-0.75)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_axis_transform_deadzone() {
        let t = AxisTransform {
            deadzone: 0.2,
            ..Default::default()
        };
        // Inside dead-zone → 0
        assert!((t.apply(0.1)).abs() < f32::EPSILON);
        // Just outside dead-zone → scaled to start from 0
        let v = t.apply(0.2 + f32::EPSILON * 100.0);
        assert!(v.abs() < 0.01);
        // Full deflection → 1.0
        assert!((t.apply(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_axis_transform_scale_and_offset() {
        let t = AxisTransform {
            scale: 0.5,
            offset: 0.25,
            ..Default::default()
        };
        // 0 * 0.5 + 0.25 = 0.25
        assert!((t.apply(0.0) - 0.25).abs() < f32::EPSILON);
        // 1 * 0.5 + 0.25 = 0.75
        assert!((t.apply(1.0) - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_axis_transform_clamps() {
        let t = AxisTransform {
            scale: 2.0,
            ..Default::default()
        };
        assert!((t.apply(1.0) - 1.0).abs() < f32::EPSILON);
        assert!((t.apply(-1.0) - (-1.0)).abs() < f32::EPSILON);
    }

    // ── Mapper axis tests ───────────────────────────────────────────

    #[test]
    fn test_mapper_axis_passthrough() {
        let mut m = mapper_with_mock();
        m.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });

        m.update_axes(&[0.75]).unwrap();
        assert!((m.backend().get_axis(0).unwrap() - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mapper_axis_with_transform() {
        let mut m = mapper_with_mock();
        m.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 1,
            transform: AxisTransform {
                invert: true,
                scale: 0.5,
                ..Default::default()
            },
        });

        m.update_axes(&[0.8]).unwrap();
        let expected = -0.8 * 0.5; // inverted then scaled
        assert!((m.backend().get_axis(1).unwrap() - expected).abs() < 0.01);
    }

    // ── Merge tests ─────────────────────────────────────────────────

    #[test]
    fn test_mapper_merge_first_non_zero() {
        let mut m = mapper_with_mock();
        m.set_merge_strategy(MergeStrategy::FirstNonZero);

        // Two sources → same virtual axis.
        m.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });
        m.add_axis_mapping(AxisMapping {
            src_axis: 1,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });

        // First is zero, second is non-zero → picks second.
        m.update_axes(&[0.0, 0.6]).unwrap();
        assert!((m.backend().get_axis(0).unwrap() - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mapper_merge_sum() {
        let mut m = mapper_with_mock();
        m.set_merge_strategy(MergeStrategy::Sum);

        m.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });
        m.add_axis_mapping(AxisMapping {
            src_axis: 1,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });

        m.update_axes(&[0.3, 0.4]).unwrap();
        assert!((m.backend().get_axis(0).unwrap() - 0.7).abs() < f32::EPSILON);

        // Overflow clamps.
        m.update_axes(&[0.8, 0.9]).unwrap();
        assert!((m.backend().get_axis(0).unwrap() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mapper_merge_max_abs() {
        let mut m = mapper_with_mock();
        m.set_merge_strategy(MergeStrategy::MaxAbs);

        m.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });
        m.add_axis_mapping(AxisMapping {
            src_axis: 1,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });

        m.update_axes(&[0.3, -0.8]).unwrap();
        assert!((m.backend().get_axis(0).unwrap() - (-0.8)).abs() < f32::EPSILON);
    }

    // ── Split (one → many) ──────────────────────────────────────────

    #[test]
    fn test_mapper_split_one_to_many() {
        let mut m = mapper_with_mock();

        // Physical axis 0 → virtual 0 (normal) and virtual 1 (inverted).
        m.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });
        m.add_axis_mapping(AxisMapping {
            src_axis: 0,
            dst_axis: 1,
            transform: AxisTransform {
                invert: true,
                ..Default::default()
            },
        });

        m.update_axes(&[0.5]).unwrap();
        assert!((m.backend().get_axis(0).unwrap() - 0.5).abs() < f32::EPSILON);
        assert!((m.backend().get_axis(1).unwrap() - (-0.5)).abs() < f32::EPSILON);
    }

    // ── Button tests ────────────────────────────────────────────────

    #[test]
    fn test_mapper_button_direct() {
        let mut m = mapper_with_mock();
        m.add_button_mapping(ButtonMapping {
            src_button: 0,
            dst_button: 0,
            mode: ButtonMode::Direct,
        });

        m.update_buttons(&[true]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        m.update_buttons(&[false]).unwrap();
        assert!(!m.backend().get_button(0).unwrap());
    }

    #[test]
    fn test_mapper_button_toggle() {
        let mut m = mapper_with_mock();
        m.add_button_mapping(ButtonMapping {
            src_button: 0,
            dst_button: 0,
            mode: ButtonMode::Toggle,
        });

        // Press → toggle on.
        m.update_buttons(&[true]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        // Release (no change to virtual).
        m.update_buttons(&[false]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        // Press again → toggle off.
        m.update_buttons(&[true]).unwrap();
        assert!(!m.backend().get_button(0).unwrap());
    }

    #[test]
    fn test_mapper_button_pulse() {
        let mut m = mapper_with_mock();
        m.add_button_mapping(ButtonMapping {
            src_button: 0,
            dst_button: 0,
            mode: ButtonMode::Pulse { ticks: 3 },
        });

        // Trigger pulse (rising edge).
        m.update_buttons(&[true]).unwrap();
        assert!(m.backend().get_button(0).unwrap());

        // Release physical, but pulse still active (tick 1).
        m.update_buttons(&[false]).unwrap();
        assert!(m.backend().get_button(0).unwrap()); // 2 remaining

        // Tick 2.
        m.update_buttons(&[false]).unwrap();
        assert!(m.backend().get_button(0).unwrap()); // 1 remaining

        // Tick 3 → expires.
        m.update_buttons(&[false]).unwrap();
        assert!(!m.backend().get_button(0).unwrap());
    }

    // ── Hat tests ───────────────────────────────────────────────────

    #[test]
    fn test_mapper_hat_passthrough() {
        let mut m = mapper_with_mock();
        m.add_hat_mapping(HatMapping {
            src_hat: 0,
            dst_hat: 0,
        });

        m.update_hats(&[HatDirection::NorthEast]).unwrap();
        assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::NorthEast);
    }

    #[test]
    fn test_mapper_hat_remap() {
        let mut m = mapper_with_mock();
        // Physical hat 2 → virtual hat 0.
        m.add_hat_mapping(HatMapping {
            src_hat: 2,
            dst_hat: 0,
        });

        m.update_hats(&[
            HatDirection::Centered,
            HatDirection::Centered,
            HatDirection::West,
        ])
        .unwrap();
        assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::West);
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn test_mapper_no_mappings_is_noop() {
        let mut m = mapper_with_mock();
        m.update_axes(&[0.5, 0.6]).unwrap();
        m.update_buttons(&[true, false]).unwrap();
        m.update_hats(&[HatDirection::North]).unwrap();

        // Nothing should have been written.
        assert!((m.backend().get_axis(0).unwrap()).abs() < f32::EPSILON);
        assert!(!m.backend().get_button(0).unwrap());
        assert_eq!(m.backend().get_hat(0).unwrap(), HatDirection::Centered);
    }

    #[test]
    fn test_mapper_missing_physical_defaults_to_zero() {
        let mut m = mapper_with_mock();
        m.add_axis_mapping(AxisMapping {
            src_axis: 5,
            dst_axis: 0,
            transform: AxisTransform::default(),
        });

        // Only 2 physical axes provided — src 5 is out of range, defaults to 0.
        m.update_axes(&[0.1, 0.2]).unwrap();
        assert!((m.backend().get_axis(0).unwrap()).abs() < f32::EPSILON);
    }
}
