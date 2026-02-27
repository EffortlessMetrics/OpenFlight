// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Dedicated deadzone processing for axis inputs.
//!
//! Maps input `[-1.0, 1.0]` to output `[-1.0, 1.0]` with:
//! - **Center deadzone**: values within `[-center, +center]` collapse to `0.0`.
//! - **Edge deadzone**: values within `edge` of `±1.0` saturate to `±1.0`.
//! - Both applied together via a single rescaling formula.
//!
//! # Example
//!
//! ```rust
//! use flight_axis::deadzone::{DeadzoneConfig, DeadzoneProcessor};
//!
//! let config = DeadzoneConfig::center_only(0.05).unwrap();
//! let proc = DeadzoneProcessor::new(config);
//!
//! // Within deadzone → 0.0
//! assert_eq!(proc.apply(0.02), 0.0);
//!
//! // Outside deadzone → rescaled
//! let out = proc.apply(1.0);
//! assert!((out - 1.0).abs() < 1e-6);
//! ```

use thiserror::Error;

/// Errors returned when constructing a [`DeadzoneConfig`].
#[derive(Debug, Error, PartialEq)]
pub enum DeadzoneError {
    /// `center` is negative or ≥ 0.5.
    #[error("center deadzone must be in [0.0, 0.5), got invalid value")]
    InvalidCenter,
    /// `edge` is negative or ≥ 0.5.
    #[error("edge deadzone must be in [0.0, 0.5), got invalid value")]
    InvalidEdge,
    /// `center + edge` ≥ 1.0, leaving no active range.
    #[error("center + edge must be < 1.0 to leave an active range")]
    Overlap,
}

/// Deadzone configuration for a single axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeadzoneConfig {
    /// Center deadzone half-width in `[0.0, 0.5)`. Default `0.0`.
    pub center: f32,
    /// Edge (saturation) deadzone width in `[0.0, 0.5)`. Default `0.0`.
    pub edge: f32,
}

impl Default for DeadzoneConfig {
    fn default() -> Self {
        Self {
            center: 0.0,
            edge: 0.0,
        }
    }
}

impl DeadzoneConfig {
    /// Creates a new config, validating that both values are in `[0.0, 0.5)` and
    /// `center + edge < 1.0`.
    ///
    /// Validation order: Overlap is checked before individual bounds so that callers
    /// providing both values too large see `Overlap` rather than a single-field error.
    pub fn new(center: f32, edge: f32) -> Result<Self, DeadzoneError> {
        if center + edge >= 1.0 {
            return Err(DeadzoneError::Overlap);
        }
        if center < 0.0 || center >= 0.5 {
            return Err(DeadzoneError::InvalidCenter);
        }
        if edge < 0.0 || edge >= 0.5 {
            return Err(DeadzoneError::InvalidEdge);
        }
        Ok(Self { center, edge })
    }

    /// Creates a config with only a center deadzone (edge = 0.0).
    pub fn center_only(center: f32) -> Result<Self, DeadzoneError> {
        Self::new(center, 0.0)
    }
}

/// Applies center and edge deadzones to a single axis value.
///
/// Processing steps per sample:
/// 1. Clamp input to `[-1.0, 1.0]`.
/// 2. If `|input| <= center` → return `0.0`.
/// 3. Rescale: `output = sign(input) * (|input| - center) / (1.0 - center - edge)`.
/// 4. Clamp output to `[-1.0, 1.0]` (naturally saturates the edge region).
pub struct DeadzoneProcessor {
    config: DeadzoneConfig,
}

impl DeadzoneProcessor {
    /// Creates a processor from a validated [`DeadzoneConfig`].
    pub fn new(config: DeadzoneConfig) -> Self {
        Self { config }
    }

    /// Applies the deadzone mapping to `input`.
    #[inline]
    pub fn apply(&self, input: f32) -> f32 {
        let input = input.clamp(-1.0, 1.0);
        let abs = input.abs();
        let center = self.config.center;
        let edge = self.config.edge;

        if abs <= center {
            return 0.0;
        }

        let sign = if input >= 0.0 { 1.0_f32 } else { -1.0_f32 };
        let denominator = 1.0 - center - edge;
        // denominator > 0 is guaranteed by DeadzoneConfig::new validation.
        let scaled = (abs - center) / denominator;
        (sign * scaled).clamp(-1.0, 1.0)
    }

    /// Returns the current configuration.
    pub fn config(&self) -> DeadzoneConfig {
        self.config
    }
}

/// A bank of [`DeadzoneProcessor`]s, one per axis.
pub struct DeadzoneBank {
    processors: Vec<DeadzoneProcessor>,
}

impl DeadzoneBank {
    /// Creates a bank of `count` processors, all with default (no-op) configs.
    pub fn new(count: usize) -> Self {
        let processors = (0..count)
            .map(|_| DeadzoneProcessor::new(DeadzoneConfig::default()))
            .collect();
        Self { processors }
    }

    /// Applies the deadzone for `axis_index` to `value`.
    ///
    /// Returns `value` unchanged if `axis_index` is out of range.
    pub fn apply(&self, axis_index: usize, value: f32) -> f32 {
        match self.processors.get(axis_index) {
            Some(proc) => proc.apply(value),
            None => value,
        }
    }

    /// Replaces the config for a single axis.
    ///
    /// Does nothing if `axis_index` is out of range.
    pub fn set_config(&mut self, axis_index: usize, config: DeadzoneConfig) {
        if let Some(proc) = self.processors.get_mut(axis_index) {
            *proc = DeadzoneProcessor::new(config);
        }
    }

    /// Returns the number of axes in this bank.
    pub fn len(&self) -> usize {
        self.processors.len()
    }

    /// Returns `true` if the bank contains no axes.
    pub fn is_empty(&self) -> bool {
        self.processors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── unit tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_zero_deadzone_passthrough() {
        let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
        assert!((proc.apply(0.5) - 0.5).abs() < f32::EPSILON);
        assert!((proc.apply(-0.75) - (-0.75)).abs() < f32::EPSILON);
        assert!((proc.apply(0.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_center_deadzone_within_zone() {
        // 5% deadzone, input 0.02 < 0.05 → 0.0
        let config = DeadzoneConfig::center_only(0.05).unwrap();
        let proc = DeadzoneProcessor::new(config);
        assert_eq!(proc.apply(0.02), 0.0);
        assert_eq!(proc.apply(-0.02), 0.0);
        assert_eq!(proc.apply(0.0), 0.0);
    }

    #[test]
    fn test_center_deadzone_outside_zone() {
        // center=0.05, edge=0.0 → denom = 0.95
        // input 0.1 → (0.1 - 0.05) / 0.95 ≈ 0.052631
        let config = DeadzoneConfig::center_only(0.05).unwrap();
        let proc = DeadzoneProcessor::new(config);
        let expected = 0.05_f32 / 0.95_f32;
        let out = proc.apply(0.1);
        assert!(
            (out - expected).abs() < 1e-6,
            "expected {expected}, got {out}"
        );
    }

    #[test]
    fn test_center_deadzone_full_deflection() {
        let config = DeadzoneConfig::center_only(0.05).unwrap();
        let proc = DeadzoneProcessor::new(config);
        assert!((proc.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_center_deadzone_negative() {
        // Symmetric: -0.1 with DZ 0.05 → -(0.1-0.05)/0.95
        let config = DeadzoneConfig::center_only(0.05).unwrap();
        let proc = DeadzoneProcessor::new(config);
        let expected = -(0.05_f32 / 0.95_f32);
        let out = proc.apply(-0.1);
        assert!(
            (out - expected).abs() < 1e-6,
            "expected {expected}, got {out}"
        );
    }

    #[test]
    fn test_edge_deadzone_saturation() {
        // edge=0.1, center=0.0 → denom=0.9; input 0.95 → 0.95/0.9 > 1.0 → clamp to 1.0
        let config = DeadzoneConfig::new(0.0, 0.1).unwrap();
        let proc = DeadzoneProcessor::new(config);
        assert_eq!(proc.apply(0.95), 1.0);
        assert_eq!(proc.apply(-0.95), -1.0);
    }

    #[test]
    fn test_edge_deadzone_below() {
        // edge=0.1, center=0.0 → denom=0.9; input 0.5 → 0.5/0.9 ≈ 0.5556
        let config = DeadzoneConfig::new(0.0, 0.1).unwrap();
        let proc = DeadzoneProcessor::new(config);
        let expected = 0.5_f32 / 0.9_f32;
        let out = proc.apply(0.5);
        assert!(
            (out - expected).abs() < 1e-6,
            "expected {expected}, got {out}"
        );
    }

    #[test]
    fn test_combined_center_and_edge() {
        // center=0.05, edge=0.1 → denom=0.85; input 0.5 → (0.5-0.05)/0.85 ≈ 0.5294
        let config = DeadzoneConfig::new(0.05, 0.1).unwrap();
        let proc = DeadzoneProcessor::new(config);
        let expected = 0.45_f32 / 0.85_f32;
        let out = proc.apply(0.5);
        assert!(
            (out - expected).abs() < 1e-6,
            "expected {expected}, got {out}"
        );
    }

    #[test]
    fn test_clamp_above_one() {
        let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
        assert_eq!(proc.apply(1.5), 1.0);
    }

    #[test]
    fn test_clamp_below_neg_one() {
        let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
        assert_eq!(proc.apply(-1.5), -1.0);
    }

    #[test]
    fn test_bank_apply() {
        let bank = DeadzoneBank::new(3);
        // Default config → passthrough
        assert!((bank.apply(0, 0.5) - 0.5).abs() < f32::EPSILON);
        assert!((bank.apply(2, -0.3) - (-0.3)).abs() < f32::EPSILON);
        // Out-of-range index → passthrough
        assert!((bank.apply(99, 0.7) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bank_set_config() {
        let mut bank = DeadzoneBank::new(2);
        let config = DeadzoneConfig::center_only(0.1).unwrap();
        bank.set_config(0, config);

        // axis 0: center deadzone 0.1
        assert_eq!(bank.apply(0, 0.05), 0.0);
        let expected = (0.5 - 0.1) / 0.9_f32;
        assert!((bank.apply(0, 0.5) - expected).abs() < 1e-6);

        // axis 1: still default → passthrough
        assert!((bank.apply(1, 0.5) - 0.5).abs() < f32::EPSILON);

        assert_eq!(bank.len(), 2);
        assert!(!bank.is_empty());
    }

    #[test]
    fn test_invalid_center_error() {
        assert_eq!(
            DeadzoneConfig::new(0.6, 0.0),
            Err(DeadzoneError::InvalidCenter)
        );
        assert_eq!(
            DeadzoneConfig::new(-0.1, 0.0),
            Err(DeadzoneError::InvalidCenter)
        );
    }

    #[test]
    fn test_overlap_error() {
        // Overlap is checked before individual bounds, so center=0.4 + edge=0.7 = 1.1 >= 1.0
        // returns Overlap even though edge would also be InvalidEdge on its own.
        assert_eq!(DeadzoneConfig::new(0.4, 0.7), Err(DeadzoneError::Overlap));
        // Both individually > 0.5 but Overlap still fires first.
        assert_eq!(DeadzoneConfig::new(0.6, 0.6), Err(DeadzoneError::Overlap));
    }

    // ── proptests ───────────────────────────────────────────────────────────

    proptest! {
        /// Output is always in [-1.0, 1.0] for any valid center/edge combo and input.
        #[test]
        fn prop_output_in_range(
            input in -2.0f32..=2.0f32,
            center in 0.0f32..0.49f32,
            edge in 0.0f32..0.49f32,
        ) {
            if center + edge < 1.0 {
                let config = DeadzoneConfig::new(center, edge).unwrap();
                let proc = DeadzoneProcessor::new(config);
                let out = proc.apply(input);
                prop_assert!(
                    out >= -1.0 && out <= 1.0,
                    "output {out} out of [-1,1] for input={input}, center={center}, edge={edge}"
                );
            }
        }

        /// Zero deadzone is the identity (modulo f32 clamping).
        #[test]
        fn prop_zero_deadzone_identity(input in -1.0f32..=1.0f32) {
            let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
            let out = proc.apply(input);
            prop_assert!(
                (out - input).abs() < f32::EPSILON * 4.0,
                "zero deadzone should be identity: input={input}, output={out}"
            );
        }

        /// Deadzone is antisymmetric: apply(-x) == -apply(x).
        #[test]
        fn prop_antisymmetric(
            input in -1.0f32..=1.0f32,
            center in 0.0f32..0.49f32,
            edge in 0.0f32..0.49f32,
        ) {
            if center + edge < 1.0 {
                let config = DeadzoneConfig::new(center, edge).unwrap();
                let proc = DeadzoneProcessor::new(config);
                let pos = proc.apply(input);
                let neg = proc.apply(-input);
                prop_assert!(
                    (pos + neg).abs() < 1e-6,
                    "antisymmetry violated: apply({input})={pos}, apply({})={neg}",
                    -input
                );
            }
        }
    }
}
