// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis filter chain composing all processing stages in the correct order.
//!
//! Pipeline order: `calibration → invert → deadzone → curve → ema_smoothing → rate_limit → trim → normalize`

use crate::stages::StageSlot;
use crate::{
    AxisCalibration, AxisInvert, AxisNormalizer, AxisRateLimiter, AxisTrim, DeadzoneConfig,
    DeadzoneProcessor, EmaFilter, NormalizeConfig, ResponseCurve,
};

/// Configuration for an axis filter chain.
#[derive(Debug, Clone, Default)]
pub struct AxisChainConfig {
    /// Optional calibration from raw u16 input (stage 1).
    pub calibration: Option<AxisCalibration>,
    /// Inversion (stage 2, before deadzone).
    pub invert: AxisInvert,
    /// Deadzone (stage 3).
    pub deadzone: DeadzoneConfig,
    /// Response curve (stage 4). `None` means passthrough.
    pub curve: Option<ResponseCurve>,
    /// EMA smoothing alpha in `[0.0, 1.0]` (stage 5). `None` means passthrough.
    pub smoothing: Option<f32>,
    /// Rate limit max change per tick (stage 6). `None` means passthrough.
    pub rate_limit: Option<f32>,
    /// Trim offset, additive, in `[-1.0, 1.0]` (stage 7).
    pub trim: f32,
}

/// Output of each pipeline stage, for diagnostics.
#[derive(Debug, Clone, Default)]
pub struct ChainStageValues {
    /// After calibration or the raw f32 input.
    pub raw_f32: f32,
    /// After inversion stage.
    pub after_invert: f32,
    /// After deadzone stage.
    pub after_deadzone: f32,
    /// After response curve stage.
    pub after_curve: f32,
    /// After EMA smoothing stage.
    pub after_smoothing: f32,
    /// After rate limiter stage.
    pub after_rate_limit: f32,
    /// Final output after trim.
    pub output: f32,
    /// Final validated output after normalization guard (always in [-1.0, 1.0]).
    pub validated: f32,
}

/// Composed axis processing chain.
///
/// Applies stages in order:
/// `calibration → invert → deadzone → curve → ema_smoothing → rate_limit → trim → normalize`
pub struct AxisChain {
    config: AxisChainConfig,
    ema: Option<EmaFilter>,
    rate_limiter: Option<AxisRateLimiter>,
    trim: AxisTrim,
    deadzone: DeadzoneProcessor,
    normalizer: AxisNormalizer,
}

impl AxisChain {
    /// Creates a new `AxisChain` from the given configuration.
    pub fn new(config: AxisChainConfig) -> Self {
        let ema = config.smoothing.map(EmaFilter::new);
        let rate_limiter = config.rate_limit.map(AxisRateLimiter::new);
        // Allow full [-1.0, 1.0] trim range.
        let mut trim = AxisTrim::new(1.0, 0.01);
        trim.set_offset(config.trim);
        let deadzone = DeadzoneProcessor::new(config.deadzone);
        Self {
            config,
            ema,
            rate_limiter,
            trim,
            deadzone,
            normalizer: AxisNormalizer::new(NormalizeConfig::default()),
        }
    }

    /// Processes a raw `u16` input through the full chain.
    ///
    /// Converts to f32 via the configured [`AxisCalibration`], falling back to
    /// [`AxisCalibration::default_full_range`] when none is set.
    ///
    /// Returns `(output, stage_values)`.
    pub fn process_raw(&mut self, raw: u16) -> (f32, ChainStageValues) {
        let raw_f32 = match &self.config.calibration {
            Some(cal) => cal.normalize(raw),
            None => AxisCalibration::default_full_range().normalize(raw),
        };
        self.run_pipeline(raw_f32)
    }

    /// Processes a pre-normalized f32 input through the chain, skipping calibration.
    ///
    /// Returns `(output, stage_values)`.
    pub fn process_f32(&mut self, input: f32) -> (f32, ChainStageValues) {
        self.run_pipeline(input)
    }

    /// Runs stages 2–8 on a normalized f32 value.
    fn run_pipeline(&mut self, raw_f32: f32) -> (f32, ChainStageValues) {
        // Stage 2: invert
        let after_invert = self.config.invert.apply(raw_f32);

        // Stage 3: deadzone
        let after_deadzone = self.deadzone.apply(after_invert);

        // Stage 4: response curve (operates on [0.0, 1.0]; sign is preserved for bipolar input)
        let after_curve = if let Some(ref curve) = self.config.curve {
            let abs_val = after_deadzone.abs();
            let mapped = curve.evaluate(abs_val);
            if after_deadzone < 0.0 {
                -mapped
            } else {
                mapped
            }
        } else {
            after_deadzone
        };

        // Stage 5: EMA smoothing
        let after_smoothing = if let Some(ref mut ema) = self.ema {
            ema.apply(after_curve)
        } else {
            after_curve
        };

        // Stage 6: rate limiter
        let after_rate_limit = if let Some(ref mut rl) = self.rate_limiter {
            rl.apply(after_smoothing)
        } else {
            after_smoothing
        };

        // Stage 7: trim (additive offset + clamp)
        let output = self.trim.apply(after_rate_limit);

        // Stage 8: normalization guard — ensures output is always in [-1.0, 1.0]
        let validated = self.normalizer.process(output);

        let stages = ChainStageValues {
            raw_f32,
            after_invert,
            after_deadzone,
            after_curve,
            after_smoothing,
            after_rate_limit,
            output,
            validated,
        };

        (validated, stages)
    }

    /// Returns the current trim offset.
    pub fn trim_offset(&self) -> f32 {
        self.trim.offset()
    }

    /// Sets the trim offset, clamped to `[-1.0, 1.0]`.
    pub fn set_trim(&mut self, value: f32) {
        let clamped = value.clamp(-1.0, 1.0);
        self.config.trim = clamped;
        self.trim.set_offset(clamped);
    }

    /// Returns a reference to the current configuration.
    pub fn config(&self) -> &AxisChainConfig {
        &self.config
    }

    /// Resets stateful stages (EMA filter and rate limiter) to their initial state.
    pub fn reset(&mut self) {
        if let Some(ref mut ema) = self.ema {
            ema.reset();
        }
        if let Some(ref mut rl) = self.rate_limiter {
            rl.reset();
        }
    }
}

// ---------------------------------------------------------------------------
// RtAxisChain — zero-allocation ordered chain with enable/disable
// ---------------------------------------------------------------------------

/// Maximum number of stages in an [`RtAxisChain`].
pub const MAX_CHAIN_STAGES: usize = 8;

/// A chain slot: a stage with an independent enable flag.
#[derive(Debug, Clone, Copy)]
struct ChainSlot {
    stage: StageSlot,
    enabled: bool,
}

impl Default for ChainSlot {
    fn default() -> Self {
        Self {
            stage: StageSlot::Empty,
            enabled: true,
        }
    }
}

/// Zero-allocation axis processing chain with per-stage enable/disable.
///
/// Composes up to [`MAX_CHAIN_STAGES`] processing stages into an ordered
/// pipeline. Each stage can be individually enabled or disabled at runtime
/// without reallocating. All state lives on the stack (ADR-004).
///
/// # Example
///
/// ```rust
/// use flight_axis::chain::RtAxisChain;
/// use flight_axis::stages::{
///     StageSlot, DeadzoneStage, DeadzoneShape, SaturationStage,
/// };
///
/// let mut chain = RtAxisChain::new();
/// chain.push(StageSlot::Deadzone(DeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear)));
/// chain.push(StageSlot::Saturation(SaturationStage::bipolar()));
///
/// let output = chain.process(0.5);
/// assert!(output >= -1.0 && output <= 1.0);
///
/// // Disable deadzone
/// chain.set_enabled(0, false);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RtAxisChain {
    slots: [ChainSlot; MAX_CHAIN_STAGES],
    count: usize,
}

impl RtAxisChain {
    /// Creates an empty chain (passthrough).
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [ChainSlot::default(); MAX_CHAIN_STAGES],
            count: 0,
        }
    }

    /// Process an input value through all enabled stages in order.
    #[inline]
    pub fn process(&mut self, input: f64) -> f64 {
        let mut value = input;
        for slot in &mut self.slots[..self.count] {
            if slot.enabled {
                value = slot.stage.process(value);
            }
        }
        value
    }

    /// Appends a stage to the end. Returns `true` on success, `false` if full.
    pub fn push(&mut self, stage: StageSlot) -> bool {
        if self.count >= MAX_CHAIN_STAGES {
            return false;
        }
        self.slots[self.count] = ChainSlot {
            stage,
            enabled: true,
        };
        self.count += 1;
        true
    }

    /// Replaces the stage at `index` with a new stage slot.
    ///
    /// Returns `true` on success, `false` if index is out of range.
    pub fn configure(&mut self, index: usize, stage: StageSlot) -> bool {
        if index >= self.count {
            return false;
        }
        self.slots[index].stage = stage;
        true
    }

    /// Enables or disables the stage at `index`.
    ///
    /// Returns `true` on success, `false` if index is out of range.
    pub fn set_enabled(&mut self, index: usize, enabled: bool) -> bool {
        if index >= self.count {
            return false;
        }
        self.slots[index].enabled = enabled;
        true
    }

    /// Returns whether the stage at `index` is enabled.
    #[must_use]
    pub fn is_enabled(&self, index: usize) -> Option<bool> {
        if index >= self.count {
            return None;
        }
        Some(self.slots[index].enabled)
    }

    /// Returns the number of stages (including disabled ones).
    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.count
    }

    /// Returns the name of the stage at `index`, or `None` if out of range.
    #[must_use]
    pub fn stage_name(&self, index: usize) -> Option<&'static str> {
        if index >= self.count {
            return None;
        }
        Some(self.slots[index].stage.name())
    }

    /// Removes the stage at `index`, shifting later stages left.
    ///
    /// Returns `true` on success, `false` if index is out of range.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.count {
            return false;
        }
        for i in index..self.count - 1 {
            self.slots[i] = self.slots[i + 1];
        }
        self.slots[self.count - 1] = ChainSlot::default();
        self.count -= 1;
        true
    }
}

impl Default for RtAxisChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::{
        ClampStage, CurveStage, CurveType, DeadzoneShape, DeadzoneStage, DetentPosition,
        DetentStage, SaturationStage, SlewRateLimiter, SmoothingStage,
    };
    use crate::{AxisCalibration, AxisInvert, DeadzoneConfig, ResponseCurve};
    use proptest::prelude::*;

    #[test]
    fn test_chain_default_passthrough() {
        let mut chain = AxisChain::new(AxisChainConfig::default());
        let (output, _) = chain.process_f32(0.5);
        assert!((output - 0.5).abs() < 1e-5, "expected 0.5, got {output}");
    }

    #[test]
    fn test_chain_with_deadzone() {
        let config = AxisChainConfig {
            deadzone: DeadzoneConfig::center_only(0.05).unwrap(),
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        let (output, _) = chain.process_f32(0.02);
        assert_eq!(output, 0.0, "input 0.02 within 5% deadzone should be 0.0");
    }

    #[test]
    fn test_chain_with_invert() {
        let config = AxisChainConfig {
            invert: AxisInvert::new(true),
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        let (output, _) = chain.process_f32(0.5);
        assert!(
            (output - (-0.5)).abs() < 1e-5,
            "expected -0.5, got {output}"
        );
    }

    #[test]
    fn test_chain_with_smoothing() {
        let config = AxisChainConfig {
            smoothing: Some(0.1), // heavy smoothing: alpha=0.1
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        // First call seeds EMA state to 0.0.
        chain.process_f32(0.0);
        // Second call: state = 0.1 * 1.0 + 0.9 * 0.0 = 0.1
        let (output, _) = chain.process_f32(1.0);
        assert!(output < 0.5, "EMA should smooth rapid change, got {output}");
    }

    #[test]
    fn test_chain_with_trim() {
        let config = AxisChainConfig {
            trim: 0.1,
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        let (output, _) = chain.process_f32(0.3);
        assert!((output - 0.4).abs() < 1e-5, "expected 0.4, got {output}");
    }

    #[test]
    fn test_chain_with_rate_limit() {
        let config = AxisChainConfig {
            rate_limit: Some(0.1),
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        // Jump from 0.0 to 1.0; rate limit = 0.1/tick → output should be 0.1
        let (output, _) = chain.process_f32(1.0);
        assert!(
            (output - 0.1).abs() < 1e-5,
            "expected rate-limited output 0.1, got {output}"
        );
    }

    #[test]
    fn test_chain_raw_input() {
        let config = AxisChainConfig {
            calibration: Some(AxisCalibration::default_full_range()),
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        let (out_max, _) = chain.process_raw(65535);
        assert!(
            (out_max - 1.0).abs() < 1e-5,
            "max raw should map to 1.0, got {out_max}"
        );
        let (out_min, _) = chain.process_raw(0);
        assert!(
            (out_min - (-1.0)).abs() < 1e-5,
            "min raw should map to -1.0, got {out_min}"
        );
    }

    #[test]
    fn test_chain_stage_values_populated() {
        let config = AxisChainConfig {
            invert: AxisInvert::new(true),
            deadzone: DeadzoneConfig::center_only(0.05).unwrap(),
            smoothing: Some(0.5),
            rate_limit: Some(0.5),
            trim: 0.05,
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        let (_, stages) = chain.process_f32(0.5);

        assert!((stages.raw_f32 - 0.5).abs() < 1e-5, "raw_f32 should be 0.5");
        assert!(
            (stages.after_invert - (-0.5)).abs() < 1e-5,
            "after_invert should be -0.5"
        );
        // |-0.5| > 0.05, so deadzone passes through (negative)
        assert!(
            stages.after_deadzone != 0.0,
            "after_deadzone should be non-zero"
        );
        assert!(
            stages.after_curve.is_finite(),
            "after_curve should be finite"
        );
        assert!(
            stages.after_smoothing.is_finite(),
            "after_smoothing should be finite"
        );
        assert!(
            stages.after_rate_limit.is_finite(),
            "after_rate_limit should be finite"
        );
        assert!(stages.output.is_finite(), "output should be finite");
    }

    #[test]
    fn test_chain_reset() {
        let config = AxisChainConfig {
            smoothing: Some(0.1),
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        // Warm up the EMA toward 1.0
        for _ in 0..20 {
            chain.process_f32(1.0);
        }
        chain.reset();
        // After reset the EMA re-seeds on first call, so output == input
        let (output, _) = chain.process_f32(0.3);
        assert!(
            (output - 0.3).abs() < 1e-5,
            "after reset EMA should re-seed: got {output}"
        );
    }

    #[test]
    fn test_chain_with_curve() {
        let config = AxisChainConfig {
            curve: Some(ResponseCurve::linear_identity()),
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        let (output, _) = chain.process_f32(0.5);
        assert!(
            (output - 0.5).abs() < 1e-4,
            "identity curve should passthrough 0.5, got {output}"
        );
    }

    #[test]
    fn test_chain_with_all_stages() {
        let config = AxisChainConfig {
            calibration: Some(AxisCalibration::default_full_range()),
            invert: AxisInvert::new(false),
            deadzone: DeadzoneConfig::center_only(0.05).unwrap(),
            curve: Some(ResponseCurve::linear_identity()),
            smoothing: Some(0.5),
            rate_limit: Some(0.5),
            trim: 0.05,
        };
        let mut chain = AxisChain::new(config);
        for raw in [0u16, 16384, 32767, 49151, 65535] {
            let (output, _) = chain.process_raw(raw);
            assert!(
                (-1.0..=1.0).contains(&output),
                "output {output} out of [-1.0, 1.0] for raw={raw}"
            );
        }
    }

    #[test]
    fn test_chain_pipeline_order() {
        // Verify that invert fires before deadzone by inspecting stage values.
        let config = AxisChainConfig {
            invert: AxisInvert::new(true),
            deadzone: DeadzoneConfig::center_only(0.05).unwrap(),
            ..Default::default()
        };
        let mut chain = AxisChain::new(config);
        let (_, stages) = chain.process_f32(0.5);

        // Invert should have fired first: 0.5 → -0.5
        assert!(
            (stages.after_invert - (-0.5)).abs() < 1e-5,
            "invert should fire before deadzone: after_invert={}",
            stages.after_invert
        );
        // Deadzone is then applied to -0.5; |-0.5| > 0.05, result should remain negative
        assert!(
            stages.after_deadzone < 0.0,
            "deadzone should preserve sign of post-invert value: after_deadzone={}",
            stages.after_deadzone
        );
    }

    proptest! {
        #[test]
        fn proptest_output_in_range(input in -1.0f32..=1.0f32) {
            let config = AxisChainConfig {
                deadzone: DeadzoneConfig::center_only(0.05).unwrap(),
                smoothing: Some(0.3),
                rate_limit: Some(0.2),
                trim: 0.05,
                ..Default::default()
            };
            let mut chain = AxisChain::new(config);
            let (output, _) = chain.process_f32(input);
            prop_assert!(
                (-1.0..=1.0).contains(&output),
                "output {output} out of [-1.0, 1.0] for input={input}"
            );
        }

        #[test]
        fn proptest_identity_chain_passthrough(input in -1.0f32..=1.0f32) {
            let mut chain = AxisChain::new(AxisChainConfig::default());
            let (output, _) = chain.process_f32(input);
            prop_assert!(
                (output - input).abs() < 1e-5,
                "identity chain should passthrough: input={input}, output={output}"
            );
        }
    }

    // =====================================================================
    // RtAxisChain tests
    // =====================================================================

    const TOL: f64 = 1e-10;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < TOL
    }

    #[test]
    fn rt_chain_empty_passthrough() {
        let mut chain = RtAxisChain::new();
        assert!(approx(chain.process(0.75), 0.75));
        assert_eq!(chain.stage_count(), 0);
    }

    #[test]
    fn rt_chain_push_and_process() {
        let mut chain = RtAxisChain::new();
        chain.push(StageSlot::Clamp(ClampStage::new(-0.5, 0.5)));
        assert_eq!(chain.stage_count(), 1);
        assert!(approx(chain.process(0.8), 0.5));
        assert!(approx(chain.process(-0.8), -0.5));
        assert!(approx(chain.process(0.3), 0.3));
    }

    #[test]
    fn rt_chain_multi_stage_ordering() {
        let mut chain = RtAxisChain::new();
        // deadzone → curve → saturation
        chain.push(StageSlot::Deadzone(DeadzoneStage::new(
            0.0,
            0.1,
            DeadzoneShape::Linear,
        )));
        chain.push(StageSlot::Curve(CurveStage::expo(1.0)));
        chain.push(StageSlot::Saturation(SaturationStage::bipolar()));
        assert_eq!(chain.stage_count(), 3);

        // Input 0.05 → within deadzone → 0.0
        assert!(approx(chain.process(0.05), 0.0));

        // Input 0.55 → after deadzone: (0.55-0.1)/0.9 = 0.5 → after expo: 0.5^2 = 0.25
        assert!(approx(chain.process(0.55), 0.25));
    }

    #[test]
    fn rt_chain_disable_enable() {
        let mut chain = RtAxisChain::new();
        chain.push(StageSlot::Deadzone(DeadzoneStage::new(
            0.0,
            0.1,
            DeadzoneShape::Linear,
        )));
        chain.push(StageSlot::Saturation(SaturationStage::bipolar()));

        // Deadzone enabled: input 0.05 → 0.0
        assert!(approx(chain.process(0.05), 0.0));

        // Disable deadzone
        assert!(chain.set_enabled(0, false));
        assert_eq!(chain.is_enabled(0), Some(false));
        assert_eq!(chain.is_enabled(1), Some(true));

        // Deadzone disabled: input 0.05 passes through
        assert!(approx(chain.process(0.05), 0.05));

        // Re-enable deadzone
        assert!(chain.set_enabled(0, true));
        assert!(approx(chain.process(0.05), 0.0));
    }

    #[test]
    fn rt_chain_configure_replaces_stage() {
        let mut chain = RtAxisChain::new();
        chain.push(StageSlot::Clamp(ClampStage::new(-0.5, 0.5)));

        assert!(approx(chain.process(0.8), 0.5));

        // Replace with wider clamp
        assert!(chain.configure(0, StageSlot::Clamp(ClampStage::new(-1.0, 1.0))));
        assert!(approx(chain.process(0.8), 0.8));
    }

    #[test]
    fn rt_chain_configure_out_of_range() {
        let mut chain = RtAxisChain::new();
        assert!(!chain.configure(0, StageSlot::Clamp(ClampStage::unit())));
    }

    #[test]
    fn rt_chain_enable_out_of_range() {
        let chain = RtAxisChain::new();
        assert_eq!(chain.is_enabled(0), None);
    }

    #[test]
    fn rt_chain_set_enabled_out_of_range() {
        let mut chain = RtAxisChain::new();
        assert!(!chain.set_enabled(0, false));
    }

    #[test]
    fn rt_chain_remove() {
        let mut chain = RtAxisChain::new();
        chain.push(StageSlot::Deadzone(DeadzoneStage::new(
            0.0,
            0.1,
            DeadzoneShape::Linear,
        )));
        chain.push(StageSlot::Clamp(ClampStage::new(-0.5, 0.5)));
        assert_eq!(chain.stage_count(), 2);

        assert!(chain.remove(0));
        assert_eq!(chain.stage_count(), 1);
        assert_eq!(chain.stage_name(0), Some("clamp"));

        // Deadzone removed: input 0.05 goes to clamp as-is
        assert!(approx(chain.process(0.05), 0.05));
    }

    #[test]
    fn rt_chain_remove_out_of_range() {
        let mut chain = RtAxisChain::new();
        assert!(!chain.remove(0));
    }

    #[test]
    fn rt_chain_stage_names() {
        let mut chain = RtAxisChain::new();
        chain.push(StageSlot::Deadzone(DeadzoneStage::new(
            0.0,
            0.05,
            DeadzoneShape::Linear,
        )));
        chain.push(StageSlot::Curve(CurveStage::linear()));
        chain.push(StageSlot::Saturation(SaturationStage::bipolar()));

        assert_eq!(chain.stage_name(0), Some("deadzone"));
        assert_eq!(chain.stage_name(1), Some("curve"));
        assert_eq!(chain.stage_name(2), Some("saturation"));
        assert_eq!(chain.stage_name(3), None);
    }

    #[test]
    fn rt_chain_max_stages() {
        let mut chain = RtAxisChain::new();
        for _ in 0..MAX_CHAIN_STAGES {
            assert!(chain.push(StageSlot::Clamp(ClampStage::unit())));
        }
        assert_eq!(chain.stage_count(), MAX_CHAIN_STAGES);
        assert!(!chain.push(StageSlot::Clamp(ClampStage::unit())));
    }

    #[test]
    fn rt_chain_full_pipeline_with_detents() {
        let mut det = DetentStage::new();
        det.add(0.0, 0.05, 1.0);
        det.add(1.0, 0.05, 1.0);

        let mut chain = RtAxisChain::new();
        chain.push(StageSlot::Deadzone(DeadzoneStage::new(
            0.0,
            0.02,
            DeadzoneShape::Linear,
        )));
        chain.push(StageSlot::Curve(CurveStage::expo(0.3)));
        chain.push(StageSlot::Smoothing(SmoothingStage::ema(0.8)));
        chain.push(StageSlot::SlewRate(SlewRateLimiter::new(0.1)));
        chain.push(StageSlot::Detent(det));
        chain.push(StageSlot::Saturation(SaturationStage::bipolar()));
        assert_eq!(chain.stage_count(), 6);

        // Process several ticks — output always in [-1.0, 1.0]
        for input in [0.0, 0.01, 0.3, 0.5, 0.99, 1.0, -0.5, -1.0] {
            let out = chain.process(input);
            assert!(
                (-1.0..=1.0).contains(&out),
                "output {out} out of [-1.0, 1.0] for input={input}"
            );
        }
    }

    #[test]
    fn rt_chain_ordering_matters() {
        // Clamp then invert vs. invert then clamp
        let mut chain_clamp_first = RtAxisChain::new();
        chain_clamp_first.push(StageSlot::Clamp(ClampStage::new(0.0, 1.0)));
        chain_clamp_first.push(StageSlot::Curve(CurveStage::expo(1.0)));

        let mut chain_expo_first = RtAxisChain::new();
        chain_expo_first.push(StageSlot::Curve(CurveStage::expo(1.0)));
        chain_expo_first.push(StageSlot::Clamp(ClampStage::new(0.0, 1.0)));

        // Input -0.5:
        // clamp_first: clamp(-0.5) = 0.0, expo(0.0) = 0.0
        let out1 = chain_clamp_first.process(-0.5);
        // expo_first: expo(-0.5) = -0.25, clamp(-0.25) = 0.0
        let out2 = chain_expo_first.process(-0.5);

        assert!(approx(out1, 0.0));
        assert!(approx(out2, 0.0));

        // Input 0.5:
        // clamp_first: clamp(0.5) = 0.5, expo(0.5) = 0.25
        let out3 = chain_clamp_first.process(0.5);
        // expo_first: expo(0.5) = 0.25, clamp(0.25) = 0.25
        let out4 = chain_expo_first.process(0.5);

        assert!(approx(out3, 0.25));
        assert!(approx(out4, 0.25));

        // Input 0.8: ordering gives different results
        // clamp_first: clamp(0.8)=0.8, expo(0.8)=0.64
        let out5 = chain_clamp_first.process(0.8);
        // expo_first: expo(0.8)=0.64, clamp(0.64)=0.64
        let out6 = chain_expo_first.process(0.8);

        assert!(approx(out5, 0.64));
        assert!(approx(out6, 0.64));
    }

    #[test]
    fn rt_chain_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<RtAxisChain>();
    }

    #[test]
    fn rt_chain_default_is_empty() {
        let chain = RtAxisChain::default();
        assert_eq!(chain.stage_count(), 0);
    }
}
