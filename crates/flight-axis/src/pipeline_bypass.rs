// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-stage pipeline bypass (REQ-801).
//!
//! [`StageBypass`] tracks which pipeline stages are bypassed using a compact
//! `u32` bitfield.  Each stage is identified by a [`PipelineStage`] variant
//! whose discriminant maps to a single bit.
//!
//! RT-safe: no heap allocation — all operations are pure bitfield ops on a
//! stack-allocated `u32`.

/// Named pipeline stages.
///
/// Each variant's discriminant corresponds to the bit position in the
/// [`StageBypass`] bitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PipelineStage {
    /// Deadzone processing.
    Deadzone = 0,
    /// Response curve shaping.
    Curve = 1,
    /// Exponential moving-average smoothing.
    Ema = 2,
    /// Slew / rate-limit filter.
    RateLimit = 3,
    /// Trim offset.
    Trim = 4,
    /// Jitter suppression.
    Jitter = 5,
    /// Detent snapping.
    Detent = 6,
    /// Axis scaling.
    Scale = 7,
    /// Axis inversion.
    Invert = 8,
    /// Normalization.
    Normalize = 9,
    /// Quantization.
    Quantize = 10,
}

impl PipelineStage {
    /// Returns the bit mask for this stage.
    #[inline]
    #[must_use]
    const fn mask(self) -> u32 {
        1u32 << (self as u8)
    }
}

/// Bitfield tracking bypassed pipeline stages.
///
/// RT-safe: no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageBypass {
    bits: u32,
}

impl StageBypass {
    /// Creates a new `StageBypass` with no stages bypassed.
    ///
    /// RT-safe: no heap allocation.
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    /// Creates a `StageBypass` from a raw bitfield value.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Returns the raw bitfield.
    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.bits
    }

    /// Sets or clears the bypass flag for `stage`.
    ///
    /// RT-safe: no heap allocation.
    pub fn set_bypass(&mut self, stage: PipelineStage, enabled: bool) {
        if enabled {
            self.bits |= stage.mask();
        } else {
            self.bits &= !stage.mask();
        }
    }

    /// Returns `true` if `stage` is currently bypassed.
    ///
    /// RT-safe: no heap allocation.
    #[inline]
    #[must_use]
    pub const fn is_bypassed(&self, stage: PipelineStage) -> bool {
        self.bits & stage.mask() != 0
    }

    /// Bypasses all known stages.
    pub fn bypass_all(&mut self) {
        self.bits = ALL_STAGES_MASK;
    }

    /// Clears all bypasses (enables all stages).
    pub fn clear_all(&mut self) {
        self.bits = 0;
    }

    /// Returns the number of stages currently bypassed.
    #[must_use]
    pub const fn bypassed_count(&self) -> u32 {
        self.bits.count_ones()
    }
}

/// Bitmask covering every [`PipelineStage`] variant.
const ALL_STAGES_MASK: u32 = (1u32 << 11) - 1; // bits 0..=10

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_no_bypasses() {
        let sb = StageBypass::new();
        assert_eq!(sb.bits(), 0);
        assert!(!sb.is_bypassed(PipelineStage::Deadzone));
        assert!(!sb.is_bypassed(PipelineStage::Curve));
    }

    #[test]
    fn set_and_query_single_stage() {
        let mut sb = StageBypass::new();
        sb.set_bypass(PipelineStage::Ema, true);
        assert!(sb.is_bypassed(PipelineStage::Ema));
        assert!(!sb.is_bypassed(PipelineStage::Deadzone));
    }

    #[test]
    fn clear_single_stage() {
        let mut sb = StageBypass::new();
        sb.set_bypass(PipelineStage::Trim, true);
        assert!(sb.is_bypassed(PipelineStage::Trim));
        sb.set_bypass(PipelineStage::Trim, false);
        assert!(!sb.is_bypassed(PipelineStage::Trim));
    }

    #[test]
    fn multiple_stages_independent() {
        let mut sb = StageBypass::new();
        sb.set_bypass(PipelineStage::Jitter, true);
        sb.set_bypass(PipelineStage::Scale, true);
        assert!(sb.is_bypassed(PipelineStage::Jitter));
        assert!(sb.is_bypassed(PipelineStage::Scale));
        assert!(!sb.is_bypassed(PipelineStage::Curve));
        assert_eq!(sb.bypassed_count(), 2);
    }

    #[test]
    fn bypass_all_then_clear() {
        let mut sb = StageBypass::new();
        sb.bypass_all();
        assert!(sb.is_bypassed(PipelineStage::Deadzone));
        assert!(sb.is_bypassed(PipelineStage::Quantize));
        assert_eq!(sb.bypassed_count(), 11);
        sb.clear_all();
        assert_eq!(sb.bypassed_count(), 0);
    }

    #[test]
    fn from_bits_round_trip() {
        let mut sb = StageBypass::new();
        sb.set_bypass(PipelineStage::Detent, true);
        sb.set_bypass(PipelineStage::Invert, true);
        let restored = StageBypass::from_bits(sb.bits());
        assert_eq!(sb, restored);
    }

    #[test]
    fn default_is_empty() {
        let sb = StageBypass::default();
        assert_eq!(sb.bits(), 0);
        assert_eq!(sb.bypassed_count(), 0);
    }
}
