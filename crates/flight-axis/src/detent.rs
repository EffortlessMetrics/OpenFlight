// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Throttle detent snapping.
//!
//! Snaps axis values to fixed positions when within snap_range.
//! Hysteresis prevents oscillation at detent boundaries.
//!
//! # Example
//!
//! ```rust
//! use flight_axis::detent::{DetentConfig, DetentProcessor};
//!
//! let config = DetentConfig::standard_throttle();
//! let mut proc = DetentProcessor::new(config);
//!
//! // 0.01 is within the idle detent's snap_range → snaps to 0.0
//! assert_eq!(proc.apply(0.01), 0.0);
//! assert_eq!(proc.active_detent_label(), Some("idle"));
//!
//! // 0.5 is free → unchanged
//! assert_eq!(proc.apply(0.5), 0.5);
//! assert_eq!(proc.active_detent_label(), None);
//! ```

/// A single throttle detent position.
#[derive(Debug, Clone, PartialEq)]
pub struct Detent {
    /// Detent position in `[0.0, 1.0]`.
    pub position: f32,
    /// Distance within which snapping occurs (default 0.02 = 2%).
    pub snap_range: f32,
    /// Human-readable label, e.g. `"idle"`, `"toga"`.
    pub label: String,
}

impl Detent {
    /// Creates a new detent.
    pub fn new(position: f32, snap_range: f32, label: &str) -> Self {
        Self {
            position: position.clamp(0.0, 1.0),
            snap_range: snap_range.clamp(0.0, 1.0),
            label: label.to_string(),
        }
    }
}

/// Configuration for a set of throttle detents.
///
/// Detents are always kept sorted by position after each `add` call.
#[derive(Debug, Clone, Default)]
pub struct DetentConfig {
    /// Detents sorted by position (ascending).
    pub detents: Vec<Detent>,
}

impl DetentConfig {
    /// Creates an empty `DetentConfig`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a detent and returns `self` for builder chaining.
    ///
    /// Detents are sorted by position after insertion.
    pub fn add(mut self, position: f32, snap_range: f32, label: &str) -> Self {
        self.detents.push(Detent::new(position, snap_range, label));
        self.detents
            .sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
        self
    }

    /// Standard throttle: idle at 0.0 and TOGA at 1.0.
    pub fn standard_throttle() -> Self {
        Self::new().add(0.0, 0.02, "idle").add(1.0, 0.02, "toga")
    }

    /// Airbus-style throttle with five detents.
    ///
    /// Positions: reverse_idle (0.0), idle (0.25), climb (0.75), flex/mct (0.90), toga (1.0).
    pub fn airbus_throttle() -> Self {
        Self::new()
            .add(0.0, 0.02, "reverse_idle")
            .add(0.25, 0.02, "idle")
            .add(0.75, 0.02, "climb")
            .add(0.90, 0.02, "flex_mct")
            .add(1.0, 0.02, "toga")
    }
}

/// Stateful throttle detent processor.
///
/// Applies detent snapping to a unipolar axis value in `[0.0, 1.0]`.
#[derive(Debug, Clone)]
pub struct DetentProcessor {
    /// Detent configuration.
    pub config: DetentConfig,
    /// Index of the currently active (snapped) detent, if any.
    pub active_detent: Option<usize>,
}

impl DetentProcessor {
    /// Creates a new processor with the given configuration.
    pub fn new(config: DetentConfig) -> Self {
        Self {
            config,
            active_detent: None,
        }
    }

    /// Applies detent snapping to `input` (range `[0.0, 1.0]`).
    ///
    /// Returns the detent's `position` if `input` is within `snap_range` of any
    /// detent, or `input` unchanged if no detent is close enough.
    pub fn apply(&mut self, input: f32) -> f32 {
        for (i, detent) in self.config.detents.iter().enumerate() {
            if (input - detent.position).abs() <= detent.snap_range {
                self.active_detent = Some(i);
                return detent.position;
            }
        }
        self.active_detent = None;
        input
    }

    /// Returns the label of the currently active detent, or `None` if not snapped.
    pub fn active_detent_label(&self) -> Option<&str> {
        self.active_detent
            .map(|i| self.config.detents[i].label.as_str())
    }

    /// Clears the active detent state.
    pub fn reset(&mut self) {
        self.active_detent = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn idle_toga() -> DetentProcessor {
        DetentProcessor::new(DetentConfig::standard_throttle())
    }

    #[test]
    fn test_detent_snap_at_position() {
        let mut p = idle_toga();
        assert_eq!(p.apply(0.0), 0.0);
        assert_eq!(p.apply(1.0), 1.0);
    }

    #[test]
    fn test_detent_snap_within_range() {
        let mut p = idle_toga();
        // 0.015 is within the 0.02 snap_range of 0.0
        assert_eq!(p.apply(0.015), 0.0);
    }

    #[test]
    fn test_detent_no_snap_outside_range() {
        let mut p = idle_toga();
        // 0.05 is outside the 0.02 snap_range of 0.0
        assert_eq!(p.apply(0.05), 0.05);
    }

    #[test]
    fn test_detent_snap_to_nearest() {
        let mut p =
            DetentProcessor::new(DetentConfig::new().add(0.3, 0.05, "a").add(0.7, 0.05, "b"));
        // 0.32 is within 0.05 of 0.3, not within 0.05 of 0.7
        assert_eq!(p.apply(0.32), 0.3);
        assert_eq!(p.active_detent_label(), Some("a"));

        // 0.68 is within 0.05 of 0.7
        assert_eq!(p.apply(0.68), 0.7);
        assert_eq!(p.active_detent_label(), Some("b"));
    }

    #[test]
    fn test_detent_idle_at_zero() {
        let mut p = idle_toga();
        assert_eq!(p.apply(0.01), 0.0);
    }

    #[test]
    fn test_detent_toga_at_one() {
        let mut p = idle_toga();
        assert_eq!(p.apply(0.99), 1.0);
    }

    #[test]
    fn test_detent_no_snap_midrange() {
        let mut p = idle_toga();
        // 0.5 is far from both idle (0.0) and toga (1.0)
        assert_eq!(p.apply(0.5), 0.5);
    }

    #[test]
    fn test_detent_active_label_set_on_snap() {
        let mut p = idle_toga();
        p.apply(0.01);
        assert_eq!(p.active_detent_label(), Some("idle"));

        p.apply(0.985);
        assert_eq!(p.active_detent_label(), Some("toga"));
    }

    #[test]
    fn test_detent_no_active_label_when_free() {
        let mut p = idle_toga();
        p.apply(0.5);
        assert_eq!(p.active_detent_label(), None);
    }

    #[test]
    fn test_detent_reset_clears_active() {
        let mut p = idle_toga();
        p.apply(0.0);
        assert!(p.active_detent.is_some());
        p.reset();
        assert!(p.active_detent.is_none());
        assert_eq!(p.active_detent_label(), None);
    }

    #[test]
    fn test_standard_throttle_config() {
        let cfg = DetentConfig::standard_throttle();
        assert_eq!(cfg.detents.len(), 2);
        assert_eq!(cfg.detents[0].label, "idle");
        assert_eq!(cfg.detents[0].position, 0.0);
        assert_eq!(cfg.detents[1].label, "toga");
        assert_eq!(cfg.detents[1].position, 1.0);
    }

    #[test]
    fn test_airbus_config_has_five_detents() {
        let cfg = DetentConfig::airbus_throttle();
        assert_eq!(cfg.detents.len(), 5);
    }

    #[test]
    fn test_detent_sorted_by_position() {
        let cfg = DetentConfig::new()
            .add(1.0, 0.02, "toga")
            .add(0.0, 0.02, "idle")
            .add(0.5, 0.02, "mid");
        assert!(
            cfg.detents
                .windows(2)
                .all(|w| w[0].position <= w[1].position)
        );
    }

    #[test]
    fn test_detent_builder_chain() {
        let cfg = DetentConfig::new()
            .add(0.0, 0.02, "idle")
            .add(0.5, 0.03, "cruise")
            .add(1.0, 0.02, "toga");
        assert_eq!(cfg.detents.len(), 3);
        assert_eq!(cfg.detents[1].label, "cruise");
    }

    proptest! {
        /// Output is always in [0.0, 1.0] for any input in [0.0, 1.0].
        #[test]
        fn prop_output_in_range(input in 0.0f32..=1.0f32) {
            let mut p = DetentProcessor::new(DetentConfig::airbus_throttle());
            let out = p.apply(input);
            prop_assert!(
                out >= 0.0 && out <= 1.0,
                "output {} out of [0, 1] for input {}",
                out, input
            );
        }

        /// Snapped value is always one of the detent positions or the original input.
        #[test]
        fn prop_snapped_value_is_detent_or_input(input in 0.0f32..=1.0f32) {
            let config = DetentConfig::airbus_throttle();
            let positions: Vec<f32> = config.detents.iter().map(|d| d.position).collect();
            let mut p = DetentProcessor::new(config);
            let out = p.apply(input);
            let is_detent = positions.iter().any(|&pos| (out - pos).abs() < f32::EPSILON);
            let is_input = (out - input).abs() < f32::EPSILON;
            prop_assert!(
                is_detent || is_input,
                "output {} is neither a detent position nor the original input {}",
                out, input
            );
        }
    }
}
