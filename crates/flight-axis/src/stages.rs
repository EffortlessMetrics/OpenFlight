// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Zero-allocation pipeline processing stages for the 250Hz RT spine.
//!
//! All stages implement the [`Stage`] trait and use only stack allocation:
//! no `Vec`, `Box`, `String`, or heap allocation of any kind (ADR-004).
//!
//! The [`RtAxisPipeline`] composes stages into an ordered processing chain
//! using a fixed-size array with enum dispatch — fully stack-allocated.
//!
//! # Example
//!
//! ```rust
//! use flight_axis::stages::{
//!     RtAxisPipeline, DeadzoneStage, DeadzoneShape,
//!     CurveStage, CurveType, ClampStage,
//! };
//!
//! let mut pipeline = RtAxisPipeline::builder()
//!     .deadzone(0.0, 0.05, DeadzoneShape::Linear)
//!     .curve(CurveType::Expo(0.3))
//!     .clamp(-1.0, 1.0)
//!     .build();
//!
//! let output = pipeline.process(0.5);
//! assert!(output > 0.0 && output <= 1.0);
//! ```

/// Maximum number of custom curve control points.
pub const MAX_CURVE_POINTS: usize = 16;

/// Maximum SMA window size.
pub const MAX_SMA_WINDOW: usize = 32;

/// Maximum number of stages in an [`RtAxisPipeline`].
pub const MAX_STAGES: usize = 16;

// ---------------------------------------------------------------------------
// Stage trait
// ---------------------------------------------------------------------------

/// Trait for zero-allocation pipeline processing stages.
///
/// All implementations must be RT-safe: no heap allocation, no locks,
/// no blocking syscalls on the `process` hot path.
pub trait Stage {
    /// Process a single input value and return the output.
    fn process(&mut self, input: f64) -> f64;
    /// Returns the static name of this stage.
    fn name(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// DeadzoneStage
// ---------------------------------------------------------------------------

/// Deadzone rescaling shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadzoneShape {
    /// Linear rescaling outside the deadzone.
    Linear,
    /// Cubic rescaling for a smoother transition at the deadzone boundary.
    Cubic,
}

/// Deadzone stage: suppresses input near a center point and rescales the remainder.
///
/// - `center`: the axis center value (typically `0.0`)
/// - `width`: half-width of the deadzone
/// - `shape`: rescaling curve outside the deadzone
#[derive(Debug, Clone, Copy)]
pub struct DeadzoneStage {
    center: f64,
    width: f64,
    shape: DeadzoneShape,
}

impl DeadzoneStage {
    /// Creates a new deadzone stage.
    #[must_use]
    pub fn new(center: f64, width: f64, shape: DeadzoneShape) -> Self {
        Self {
            center,
            width: width.abs(),
            shape,
        }
    }

    /// Returns the configured width.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.width
    }
}

impl Stage for DeadzoneStage {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return 0.0;
        }
        let offset = input - self.center;
        let abs_offset = offset.abs();
        if abs_offset <= self.width {
            return 0.0;
        }
        let sign = offset.signum();
        let active = abs_offset - self.width;
        let max_active = (1.0 - self.width).max(f64::EPSILON);
        let normalized = (active / max_active).clamp(0.0, 1.0);
        let shaped = match self.shape {
            DeadzoneShape::Linear => normalized,
            DeadzoneShape::Cubic => normalized * normalized * normalized,
        };
        sign * shaped
    }

    fn name(&self) -> &'static str {
        "deadzone"
    }
}

// ---------------------------------------------------------------------------
// CurveStage
// ---------------------------------------------------------------------------

/// Response curve type.
#[expect(
    clippy::large_enum_variant,
    reason = "zero-alloc: boxing would violate ADR-004"
)]
#[derive(Debug, Clone, Copy)]
pub enum CurveType {
    /// Linear pass-through (identity).
    Linear,
    /// Exponential curve: `sign(x) * |x|^(1 + expo)`.
    Expo(f64),
    /// Custom piecewise-linear curve defined by control points.
    /// Points are `(input, output)` pairs sorted by input, both in `[0.0, 1.0]`.
    Custom {
        /// Control points array.
        points: [(f64, f64); MAX_CURVE_POINTS],
        /// Number of valid points (must be ≥ 2).
        count: usize,
    },
}

/// Response curve stage.
#[derive(Debug, Clone, Copy)]
pub struct CurveStage {
    curve_type: CurveType,
}

impl CurveStage {
    /// Creates a new curve stage.
    #[must_use]
    pub fn new(curve_type: CurveType) -> Self {
        Self { curve_type }
    }

    /// Creates a linear (identity) curve stage.
    #[must_use]
    pub fn linear() -> Self {
        Self::new(CurveType::Linear)
    }

    /// Creates an exponential curve stage.
    #[must_use]
    pub fn expo(factor: f64) -> Self {
        Self::new(CurveType::Expo(factor))
    }

    /// Creates a custom piecewise-linear curve from a slice of `(x, y)` points.
    ///
    /// Returns `None` if fewer than 2 points or more than [`MAX_CURVE_POINTS`].
    #[must_use]
    pub fn custom(pts: &[(f64, f64)]) -> Option<Self> {
        if pts.len() < 2 || pts.len() > MAX_CURVE_POINTS {
            return None;
        }
        let mut points = [(0.0, 0.0); MAX_CURVE_POINTS];
        for (i, &p) in pts.iter().enumerate() {
            points[i] = p;
        }
        Some(Self::new(CurveType::Custom {
            points,
            count: pts.len(),
        }))
    }
}

impl Stage for CurveStage {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return 0.0;
        }
        match self.curve_type {
            CurveType::Linear => input,
            CurveType::Expo(expo) => input.signum() * input.abs().powf(1.0 + expo),
            CurveType::Custom { points, count } => {
                if count < 2 {
                    return input;
                }
                let sign = input.signum();
                let abs_in = input.abs().clamp(0.0, 1.0);
                let mut y = abs_in;
                for i in 1..count {
                    if abs_in <= points[i].0 || i == count - 1 {
                        let (x0, y0) = points[i - 1];
                        let (x1, y1) = points[i];
                        let dx = x1 - x0;
                        if dx > f64::EPSILON {
                            let t = (abs_in - x0) / dx;
                            y = y0 + t * (y1 - y0);
                        } else {
                            y = y0;
                        }
                        break;
                    }
                }
                sign * y.clamp(0.0, 1.0)
            }
        }
    }

    fn name(&self) -> &'static str {
        "curve"
    }
}

// ---------------------------------------------------------------------------
// SmoothingStage
// ---------------------------------------------------------------------------

/// Smoothing filter type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmoothingType {
    /// Exponential Moving Average. `alpha = 1.0` → no smoothing, `0.0` → frozen.
    Ema { alpha: f64 },
    /// Simple Moving Average with a fixed window (1..=[`MAX_SMA_WINDOW`]).
    Sma { window: usize },
}

/// Smoothing filter stage (EMA or SMA). All state is stack-allocated.
#[derive(Debug, Clone, Copy)]
pub struct SmoothingStage {
    filter_type: SmoothingType,
    ema_prev: f64,
    ema_initialized: bool,
    sma_buffer: [f64; MAX_SMA_WINDOW],
    sma_index: usize,
    sma_count: usize,
}

impl SmoothingStage {
    /// Creates a new EMA smoothing stage.
    #[must_use]
    pub fn ema(alpha: f64) -> Self {
        Self {
            filter_type: SmoothingType::Ema {
                alpha: alpha.clamp(0.0, 1.0),
            },
            ema_prev: 0.0,
            ema_initialized: false,
            sma_buffer: [0.0; MAX_SMA_WINDOW],
            sma_index: 0,
            sma_count: 0,
        }
    }

    /// Creates a new SMA smoothing stage.
    #[must_use]
    pub fn sma(window: usize) -> Self {
        Self {
            filter_type: SmoothingType::Sma {
                window: window.clamp(1, MAX_SMA_WINDOW),
            },
            ema_prev: 0.0,
            ema_initialized: false,
            sma_buffer: [0.0; MAX_SMA_WINDOW],
            sma_index: 0,
            sma_count: 0,
        }
    }

    /// Resets internal state.
    pub fn reset(&mut self) {
        self.ema_prev = 0.0;
        self.ema_initialized = false;
        self.sma_buffer = [0.0; MAX_SMA_WINDOW];
        self.sma_index = 0;
        self.sma_count = 0;
    }
}

impl Stage for SmoothingStage {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return self.ema_prev;
        }
        match self.filter_type {
            SmoothingType::Ema { alpha } => {
                if !self.ema_initialized {
                    self.ema_prev = input;
                    self.ema_initialized = true;
                    return input;
                }
                let output = alpha * input + (1.0 - alpha) * self.ema_prev;
                self.ema_prev = output;
                output
            }
            SmoothingType::Sma { window } => {
                self.sma_buffer[self.sma_index] = input;
                self.sma_index = (self.sma_index + 1) % window;
                if self.sma_count < window {
                    self.sma_count += 1;
                }
                let sum: f64 = self.sma_buffer[..self.sma_count].iter().sum();
                let output = sum / self.sma_count as f64;
                self.ema_prev = output; // track for NaN fallback
                output
            }
        }
    }

    fn name(&self) -> &'static str {
        "smoothing"
    }
}

// ---------------------------------------------------------------------------
// SlewRateLimiter
// ---------------------------------------------------------------------------

/// Slew rate limiter: limits the rate of change per call.
///
/// Useful for throttle and trim axes to prevent sudden jumps.
/// `max_rate = 0.0` means unlimited (passthrough).
#[derive(Debug, Clone, Copy)]
pub struct SlewRateLimiter {
    max_rate: f64,
    current: f64,
    initialized: bool,
}

impl SlewRateLimiter {
    /// Creates a new slew rate limiter.
    #[must_use]
    pub fn new(max_rate: f64) -> Self {
        Self {
            max_rate: max_rate.abs(),
            current: 0.0,
            initialized: false,
        }
    }

    /// Returns the current output value.
    #[must_use]
    pub fn current(&self) -> f64 {
        self.current
    }
}

impl Stage for SlewRateLimiter {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return self.current;
        }
        if !self.initialized {
            self.current = input;
            self.initialized = true;
            return input;
        }
        if self.max_rate <= 0.0 {
            self.current = input;
            return input;
        }
        let delta = input - self.current;
        let clamped = delta.clamp(-self.max_rate, self.max_rate);
        self.current += clamped;
        self.current
    }

    fn name(&self) -> &'static str {
        "slew_rate"
    }
}

// ---------------------------------------------------------------------------
// ClampStage
// ---------------------------------------------------------------------------

/// Hard clamp to an output range.
#[derive(Debug, Clone, Copy)]
pub struct ClampStage {
    min: f64,
    max: f64,
}

impl ClampStage {
    /// Creates a new clamp stage.
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Standard `[-1.0, 1.0]` clamp.
    #[must_use]
    pub fn unit() -> Self {
        Self::new(-1.0, 1.0)
    }
}

impl Stage for ClampStage {
    fn process(&mut self, input: f64) -> f64 {
        if input.is_nan() {
            return 0.0;
        }
        input.clamp(self.min, self.max)
    }

    fn name(&self) -> &'static str {
        "clamp"
    }
}

// ---------------------------------------------------------------------------
// InvertStage
// ---------------------------------------------------------------------------

/// Inverts axis direction (multiplies by −1).
#[derive(Debug, Clone, Copy, Default)]
pub struct InvertStage;

impl InvertStage {
    /// Creates a new invert stage.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Stage for InvertStage {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return 0.0;
        }
        -input
    }

    fn name(&self) -> &'static str {
        "invert"
    }
}

// ---------------------------------------------------------------------------
// RescaleStage
// ---------------------------------------------------------------------------

/// Maps input from one range to another via linear interpolation.
#[derive(Debug, Clone, Copy)]
pub struct RescaleStage {
    in_min: f64,
    in_max: f64,
    out_min: f64,
    out_max: f64,
}

impl RescaleStage {
    /// Creates a new rescale stage.
    #[must_use]
    pub fn new(in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> Self {
        Self {
            in_min,
            in_max,
            out_min,
            out_max,
        }
    }
}

impl Stage for RescaleStage {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return self.out_min;
        }
        let in_range = self.in_max - self.in_min;
        if in_range.abs() < f64::EPSILON {
            return self.out_min;
        }
        let normalized = (input - self.in_min) / in_range;
        self.out_min + normalized * (self.out_max - self.out_min)
    }

    fn name(&self) -> &'static str {
        "rescale"
    }
}

// ---------------------------------------------------------------------------
// NoiseGate
// ---------------------------------------------------------------------------

/// Suppresses small fluctuations below a threshold.
///
/// If the change from the last output is below `threshold`, the output
/// is held at the previous value. Useful for noisy potentiometers.
#[derive(Debug, Clone, Copy)]
pub struct NoiseGate {
    threshold: f64,
    last_output: f64,
    initialized: bool,
}

impl NoiseGate {
    /// Creates a new noise gate.
    #[must_use]
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold: threshold.abs(),
            last_output: 0.0,
            initialized: false,
        }
    }
}

impl Stage for NoiseGate {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return self.last_output;
        }
        if !self.initialized {
            self.last_output = input;
            self.initialized = true;
            return input;
        }
        if (input - self.last_output).abs() >= self.threshold {
            self.last_output = input;
        }
        self.last_output
    }

    fn name(&self) -> &'static str {
        "noise_gate"
    }
}

// ---------------------------------------------------------------------------
// DetentStage
// ---------------------------------------------------------------------------

/// Maximum number of inline detent positions.
pub const MAX_DETENTS: usize = 8;

/// A single magnetic detent position (zero-allocation, inline).
#[derive(Debug, Clone, Copy)]
pub struct DetentPosition {
    /// The detent snap position in the axis range.
    pub position: f64,
    /// Half-width of the detent capture zone.
    pub width: f64,
    /// Strength of the snap (0.0 = no snap/passthrough, 1.0 = full snap to position).
    pub strength: f64,
}

impl DetentPosition {
    /// Creates a new detent position.
    #[must_use]
    pub fn new(position: f64, width: f64, strength: f64) -> Self {
        Self {
            position,
            width: width.abs(),
            strength: strength.clamp(0.0, 1.0),
        }
    }
}

/// Detent stage: snaps input to magnetic detent positions.
///
/// Up to [`MAX_DETENTS`] positions stored inline. When the input is within
/// `width` of a detent, it is interpolated toward the detent position based
/// on `strength`. Zero-allocation; all state is on the stack (ADR-004).
#[derive(Debug, Clone, Copy)]
pub struct DetentStage {
    detents: [DetentPosition; MAX_DETENTS],
    count: usize,
}

impl DetentStage {
    /// Creates an empty detent stage (passthrough).
    #[must_use]
    pub fn new() -> Self {
        Self {
            detents: [DetentPosition {
                position: 0.0,
                width: 0.0,
                strength: 0.0,
            }; MAX_DETENTS],
            count: 0,
        }
    }

    /// Creates a detent stage from a slice of positions.
    ///
    /// Returns `None` if more than [`MAX_DETENTS`] are provided.
    #[must_use]
    pub fn from_positions(positions: &[DetentPosition]) -> Option<Self> {
        if positions.len() > MAX_DETENTS {
            return None;
        }
        let mut stage = Self::new();
        for (i, p) in positions.iter().enumerate() {
            stage.detents[i] = *p;
        }
        stage.count = positions.len();
        Some(stage)
    }

    /// Adds a detent position. Returns `false` if full.
    pub fn add(&mut self, position: f64, width: f64, strength: f64) -> bool {
        if self.count >= MAX_DETENTS {
            return false;
        }
        self.detents[self.count] = DetentPosition::new(position, width, strength);
        self.count += 1;
        true
    }

    /// Returns the number of configured detents.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }
}

impl Default for DetentStage {
    fn default() -> Self {
        Self::new()
    }
}

impl Stage for DetentStage {
    fn process(&mut self, input: f64) -> f64 {
        if !input.is_finite() {
            return 0.0;
        }
        // Find the closest detent that captures this input
        for i in 0..self.count {
            let det = &self.detents[i];
            let distance = (input - det.position).abs();
            if distance <= det.width {
                // Interpolate: strength=1.0 → snap fully, strength=0.0 → passthrough
                return input + (det.position - input) * det.strength;
            }
        }
        input
    }

    fn name(&self) -> &'static str {
        "detent"
    }
}

// ---------------------------------------------------------------------------
// SaturationStage
// ---------------------------------------------------------------------------

/// Saturation stage: clamps output to configurable min/max range.
///
/// Designed as an axis-type-aware output limiter with named constructors
/// for common axis types (bipolar joystick vs. unipolar throttle).
#[derive(Debug, Clone, Copy)]
pub struct SaturationStage {
    min: f64,
    max: f64,
}

impl SaturationStage {
    /// Creates a new saturation stage with the given range.
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Bipolar axis range: −1.0 to 1.0 (joystick, rudder).
    #[must_use]
    pub fn bipolar() -> Self {
        Self::new(-1.0, 1.0)
    }

    /// Unipolar axis range: 0.0 to 1.0 (throttle, mixture, prop).
    #[must_use]
    pub fn unipolar() -> Self {
        Self::new(0.0, 1.0)
    }

    /// Returns the configured minimum.
    #[must_use]
    pub fn min(&self) -> f64 {
        self.min
    }

    /// Returns the configured maximum.
    #[must_use]
    pub fn max(&self) -> f64 {
        self.max
    }
}

impl Stage for SaturationStage {
    fn process(&mut self, input: f64) -> f64 {
        if input.is_nan() {
            return 0.0;
        }
        input.clamp(self.min, self.max)
    }

    fn name(&self) -> &'static str {
        "saturation"
    }
}

// ---------------------------------------------------------------------------
// StageSlot — enum dispatch for zero-allocation pipelines
// ---------------------------------------------------------------------------

/// A pipeline stage slot using enum dispatch instead of trait objects.
///
/// This is the key to zero-allocation: every variant lives on the stack
/// inside a fixed-size array, with no `Box` or `dyn` indirection.
#[derive(Debug, Clone, Copy)]
pub enum StageSlot {
    /// Empty slot (passthrough).
    Empty,
    /// Deadzone processing.
    Deadzone(DeadzoneStage),
    /// Response curve.
    Curve(CurveStage),
    /// Smoothing filter (EMA or SMA).
    Smoothing(SmoothingStage),
    /// Slew rate limiter.
    SlewRate(SlewRateLimiter),
    /// Hard clamp.
    Clamp(ClampStage),
    /// Axis inversion.
    Invert(InvertStage),
    /// Range rescaling.
    Rescale(RescaleStage),
    /// Noise gate.
    NoiseGate(NoiseGate),
    /// Magnetic detent snapping.
    Detent(DetentStage),
    /// Saturation (axis-type-aware clamping).
    Saturation(SaturationStage),
}

impl StageSlot {
    /// Process a value through this slot.
    #[inline]
    pub fn process(&mut self, input: f64) -> f64 {
        match self {
            StageSlot::Empty => input,
            StageSlot::Deadzone(s) => s.process(input),
            StageSlot::Curve(s) => s.process(input),
            StageSlot::Smoothing(s) => s.process(input),
            StageSlot::SlewRate(s) => s.process(input),
            StageSlot::Clamp(s) => s.process(input),
            StageSlot::Invert(s) => s.process(input),
            StageSlot::Rescale(s) => s.process(input),
            StageSlot::NoiseGate(s) => s.process(input),
            StageSlot::Detent(s) => s.process(input),
            StageSlot::Saturation(s) => s.process(input),
        }
    }

    /// Returns `true` if this is an empty slot.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, StageSlot::Empty)
    }

    /// Returns the name of the stage in this slot.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            StageSlot::Empty => "empty",
            StageSlot::Deadzone(s) => s.name(),
            StageSlot::Curve(s) => s.name(),
            StageSlot::Smoothing(s) => s.name(),
            StageSlot::SlewRate(s) => s.name(),
            StageSlot::Clamp(s) => s.name(),
            StageSlot::Invert(s) => s.name(),
            StageSlot::Rescale(s) => s.name(),
            StageSlot::NoiseGate(s) => s.name(),
            StageSlot::Detent(s) => s.name(),
            StageSlot::Saturation(s) => s.name(),
        }
    }
}

// ---------------------------------------------------------------------------
// StageDiagnostic / PipelineDiagnostics
// ---------------------------------------------------------------------------

/// Per-stage input/output diagnostic entry.
#[derive(Debug, Clone, Copy)]
pub struct StageDiagnostic {
    /// Stage name.
    pub name: &'static str,
    /// Value entering this stage.
    pub input: f64,
    /// Value leaving this stage.
    pub output: f64,
}

/// Complete pipeline diagnostic snapshot. Fully stack-allocated.
#[derive(Debug, Clone, Copy)]
pub struct PipelineDiagnostics {
    /// Per-stage entries.
    pub entries: [StageDiagnostic; MAX_STAGES],
    /// Number of valid entries.
    pub count: usize,
    /// Raw input to the pipeline.
    pub raw_input: f64,
    /// Final output of the pipeline.
    pub final_output: f64,
}

// ---------------------------------------------------------------------------
// RtAxisPipeline
// ---------------------------------------------------------------------------

/// Zero-allocation axis processing pipeline for the 250Hz RT spine.
///
/// Uses a fixed-size array of [`StageSlot`] with enum dispatch.
/// Maximum [`MAX_STAGES`] stages. All state lives on the stack.
#[derive(Debug, Clone, Copy)]
pub struct RtAxisPipeline {
    slots: [StageSlot; MAX_STAGES],
    count: usize,
}

impl RtAxisPipeline {
    /// Creates an empty pipeline (passthrough).
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [StageSlot::Empty; MAX_STAGES],
            count: 0,
        }
    }

    /// Returns a builder for fluent pipeline construction.
    #[must_use]
    pub fn builder() -> RtPipelineBuilder {
        RtPipelineBuilder::new()
    }

    /// Process an input value through all stages in order.
    #[inline]
    pub fn process(&mut self, input: f64) -> f64 {
        let mut value = input;
        for slot in &mut self.slots[..self.count] {
            value = slot.process(value);
        }
        value
    }

    /// Insert a stage at `index`, shifting later stages right.
    ///
    /// Returns `true` on success, `false` if the pipeline is full or index is out of range.
    pub fn insert_stage(&mut self, index: usize, slot: StageSlot) -> bool {
        if self.count >= MAX_STAGES || index > self.count {
            return false;
        }
        // Shift right
        let mut i = self.count;
        while i > index {
            self.slots[i] = self.slots[i - 1];
            i -= 1;
        }
        self.slots[index] = slot;
        self.count += 1;
        true
    }

    /// Remove the stage at `index`, shifting later stages left.
    ///
    /// Returns `true` on success, `false` if the index is out of range.
    pub fn remove_stage(&mut self, index: usize) -> bool {
        if index >= self.count {
            return false;
        }
        for i in index..self.count - 1 {
            self.slots[i] = self.slots[i + 1];
        }
        self.slots[self.count - 1] = StageSlot::Empty;
        self.count -= 1;
        true
    }

    /// Append a stage to the end. Returns `true` on success.
    pub fn push_stage(&mut self, slot: StageSlot) -> bool {
        self.insert_stage(self.count, slot)
    }

    /// Returns the number of active stages.
    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.count
    }

    /// Returns a slice of the active stages.
    #[must_use]
    pub fn stages(&self) -> &[StageSlot] {
        &self.slots[..self.count]
    }

    /// Run the pipeline and collect per-stage input/output diagnostics.
    ///
    /// **Warning**: this mutates stateful stages (smoothing, slew rate, noise gate).
    pub fn diagnostics(&mut self, input: f64) -> PipelineDiagnostics {
        let mut entries = [StageDiagnostic {
            name: "empty",
            input: 0.0,
            output: 0.0,
        }; MAX_STAGES];
        let mut value = input;
        for (slot, entry) in self.slots[..self.count]
            .iter_mut()
            .zip(entries[..self.count].iter_mut())
        {
            let stage_input = value;
            value = slot.process(value);
            *entry = StageDiagnostic {
                name: slot.name(),
                input: stage_input,
                output: value,
            };
        }
        PipelineDiagnostics {
            entries,
            count: self.count,
            raw_input: input,
            final_output: value,
        }
    }
}

impl Default for RtAxisPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RtPipelineBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for [`RtAxisPipeline`]. Zero-allocation.
#[derive(Debug, Clone, Copy)]
pub struct RtPipelineBuilder {
    pipeline: RtAxisPipeline,
}

impl RtPipelineBuilder {
    /// Creates a new empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pipeline: RtAxisPipeline::new(),
        }
    }

    /// Appends a raw [`StageSlot`].
    #[must_use]
    pub fn stage(mut self, slot: StageSlot) -> Self {
        self.pipeline.push_stage(slot);
        self
    }

    /// Appends a deadzone stage.
    #[must_use]
    pub fn deadzone(self, center: f64, width: f64, shape: DeadzoneShape) -> Self {
        self.stage(StageSlot::Deadzone(DeadzoneStage::new(
            center, width, shape,
        )))
    }

    /// Appends a response curve stage.
    #[must_use]
    pub fn curve(self, curve_type: CurveType) -> Self {
        self.stage(StageSlot::Curve(CurveStage::new(curve_type)))
    }

    /// Appends an EMA smoothing stage.
    #[must_use]
    pub fn smoothing_ema(self, alpha: f64) -> Self {
        self.stage(StageSlot::Smoothing(SmoothingStage::ema(alpha)))
    }

    /// Appends an SMA smoothing stage.
    #[must_use]
    pub fn smoothing_sma(self, window: usize) -> Self {
        self.stage(StageSlot::Smoothing(SmoothingStage::sma(window)))
    }

    /// Appends a slew rate limiter stage.
    #[must_use]
    pub fn slew_rate(self, max_rate: f64) -> Self {
        self.stage(StageSlot::SlewRate(SlewRateLimiter::new(max_rate)))
    }

    /// Appends a clamp stage.
    #[must_use]
    pub fn clamp(self, min: f64, max: f64) -> Self {
        self.stage(StageSlot::Clamp(ClampStage::new(min, max)))
    }

    /// Appends an invert stage.
    #[must_use]
    pub fn invert(self) -> Self {
        self.stage(StageSlot::Invert(InvertStage))
    }

    /// Appends a rescale stage.
    #[must_use]
    pub fn rescale(self, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> Self {
        self.stage(StageSlot::Rescale(RescaleStage::new(
            in_min, in_max, out_min, out_max,
        )))
    }

    /// Appends a noise gate stage.
    #[must_use]
    pub fn noise_gate(self, threshold: f64) -> Self {
        self.stage(StageSlot::NoiseGate(NoiseGate::new(threshold)))
    }

    /// Appends a detent stage from a slice of detent positions.
    ///
    /// Ignores the call if more than [`MAX_DETENTS`] positions are provided.
    #[must_use]
    pub fn detent(self, positions: &[DetentPosition]) -> Self {
        if let Some(stage) = DetentStage::from_positions(positions) {
            self.stage(StageSlot::Detent(stage))
        } else {
            self
        }
    }

    /// Appends a saturation stage with the given min/max range.
    #[must_use]
    pub fn saturation(self, min: f64, max: f64) -> Self {
        self.stage(StageSlot::Saturation(SaturationStage::new(min, max)))
    }

    /// Appends a bipolar saturation stage (−1.0 to 1.0).
    #[must_use]
    pub fn saturation_bipolar(self) -> Self {
        self.stage(StageSlot::Saturation(SaturationStage::bipolar()))
    }

    /// Appends a unipolar saturation stage (0.0 to 1.0).
    #[must_use]
    pub fn saturation_unipolar(self) -> Self {
        self.stage(StageSlot::Saturation(SaturationStage::unipolar()))
    }

    /// Consumes the builder and returns the finished pipeline.
    #[must_use]
    pub fn build(self) -> RtAxisPipeline {
        self.pipeline
    }
}

impl Default for RtPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-10;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < TOL
    }

    // === DeadzoneStage tests ==============================================

    #[test]
    fn deadzone_linear_center_suppressed() {
        let mut dz = DeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear);
        assert_eq!(dz.process(0.0), 0.0);
        assert_eq!(dz.process(0.03), 0.0);
        assert_eq!(dz.process(-0.04), 0.0);
        assert_eq!(dz.process(0.05), 0.0);
    }

    #[test]
    fn deadzone_linear_full_deflection() {
        let mut dz = DeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear);
        assert!(approx(dz.process(1.0), 1.0));
        assert!(approx(dz.process(-1.0), -1.0));
    }

    #[test]
    fn deadzone_linear_rescale() {
        let mut dz = DeadzoneStage::new(0.0, 0.1, DeadzoneShape::Linear);
        // input=0.55 → (0.55-0.1)/(1.0-0.1) = 0.45/0.9 = 0.5
        assert!(approx(dz.process(0.55), 0.5));
        assert!(approx(dz.process(-0.55), -0.5));
    }

    #[test]
    fn deadzone_cubic_smooth_transition() {
        let mut dz = DeadzoneStage::new(0.0, 0.1, DeadzoneShape::Cubic);
        // Just outside deadzone: normalized=(0.11-0.1)/0.9 ≈ 0.0111, cubic ≈ 1.37e-6
        let out = dz.process(0.11);
        assert!(out > 0.0 && out < 0.01);
        // Full deflection
        assert!(approx(dz.process(1.0), 1.0));
    }

    #[test]
    fn deadzone_with_center_offset() {
        let mut dz = DeadzoneStage::new(0.5, 0.05, DeadzoneShape::Linear);
        assert_eq!(dz.process(0.52), 0.0); // within deadzone
        assert!(dz.process(0.6) > 0.0); // outside deadzone
    }

    #[test]
    fn deadzone_nan_returns_zero() {
        let mut dz = DeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear);
        assert_eq!(dz.process(f64::NAN), 0.0);
    }

    #[test]
    fn deadzone_inf_returns_zero() {
        let mut dz = DeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear);
        assert_eq!(dz.process(f64::INFINITY), 0.0);
        assert_eq!(dz.process(f64::NEG_INFINITY), 0.0);
    }

    // === CurveStage tests ================================================

    #[test]
    fn curve_linear_passthrough() {
        let mut c = CurveStage::linear();
        assert!(approx(c.process(0.5), 0.5));
        assert!(approx(c.process(-0.3), -0.3));
    }

    #[test]
    fn curve_expo_square() {
        let mut c = CurveStage::expo(1.0);
        // 0.5^(1+1) = 0.25
        assert!(approx(c.process(0.5), 0.25));
        assert!(approx(c.process(-0.5), -0.25));
    }

    #[test]
    fn curve_expo_zero_is_linear() {
        let mut c = CurveStage::expo(0.0);
        assert!(approx(c.process(0.7), 0.7));
    }

    #[test]
    fn curve_expo_endpoints() {
        let mut c = CurveStage::expo(0.5);
        assert!(approx(c.process(0.0), 0.0));
        assert!(approx(c.process(1.0), 1.0));
        assert!(approx(c.process(-1.0), -1.0));
    }

    #[test]
    fn curve_custom_piecewise_linear() {
        let mut c = CurveStage::custom(&[(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)]).unwrap();
        // At x=0.25 (between points 0 and 1): t=0.5, y=0.0+0.5*0.8=0.4
        assert!(approx(c.process(0.25), 0.4));
        // At x=0.5: y=0.8
        assert!(approx(c.process(0.5), 0.8));
        // At x=0.75 (between points 1 and 2): t=0.5, y=0.8+0.5*0.2=0.9
        assert!(approx(c.process(0.75), 0.9));
    }

    #[test]
    fn curve_custom_negative_input() {
        let mut c = CurveStage::custom(&[(0.0, 0.0), (1.0, 1.0)]).unwrap();
        assert!(approx(c.process(-0.5), -0.5));
    }

    #[test]
    fn curve_custom_too_few_points() {
        assert!(CurveStage::custom(&[(0.0, 0.0)]).is_none());
    }

    #[test]
    fn curve_nan_returns_zero() {
        let mut c = CurveStage::expo(0.5);
        assert_eq!(c.process(f64::NAN), 0.0);
    }

    // === SmoothingStage tests ============================================

    #[test]
    fn smoothing_ema_first_sample_seeds() {
        let mut s = SmoothingStage::ema(0.5);
        assert!(approx(s.process(0.8), 0.8));
    }

    #[test]
    fn smoothing_ema_converges() {
        let mut s = SmoothingStage::ema(0.5);
        s.process(0.0); // seed
        let out1 = s.process(1.0); // 0.5*1.0 + 0.5*0.0 = 0.5
        assert!(approx(out1, 0.5));
        let out2 = s.process(1.0); // 0.5*1.0 + 0.5*0.5 = 0.75
        assert!(approx(out2, 0.75));
    }

    #[test]
    fn smoothing_ema_alpha_one_passthrough() {
        let mut s = SmoothingStage::ema(1.0);
        s.process(0.0);
        assert!(approx(s.process(0.42), 0.42));
        assert!(approx(s.process(-0.7), -0.7));
    }

    #[test]
    fn smoothing_sma_window_1_passthrough() {
        let mut s = SmoothingStage::sma(1);
        assert!(approx(s.process(0.5), 0.5));
        assert!(approx(s.process(0.8), 0.8));
    }

    #[test]
    fn smoothing_sma_window_4() {
        let mut s = SmoothingStage::sma(4);
        s.process(1.0);
        s.process(2.0);
        s.process(3.0);
        let out = s.process(4.0);
        assert!(approx(out, 2.5));
    }

    #[test]
    fn smoothing_sma_partial_window() {
        let mut s = SmoothingStage::sma(4);
        let out1 = s.process(0.4);
        assert!(approx(out1, 0.4));
        let out2 = s.process(0.8);
        assert!(approx(out2, 0.6));
    }

    #[test]
    fn smoothing_reset_clears_state() {
        let mut s = SmoothingStage::ema(0.5);
        s.process(1.0);
        s.process(1.0);
        s.reset();
        let out = s.process(0.3);
        assert!(approx(out, 0.3)); // re-seeded
    }

    #[test]
    fn smoothing_nan_returns_previous() {
        let mut s = SmoothingStage::ema(0.5);
        s.process(0.5);
        assert!(approx(s.process(f64::NAN), 0.5));
    }

    // === SlewRateLimiter tests ===========================================

    #[test]
    fn slew_rate_limits_rise() {
        let mut slew = SlewRateLimiter::new(0.1);
        slew.process(0.0); // initialize
        let out = slew.process(1.0);
        assert!(approx(out, 0.1));
    }

    #[test]
    fn slew_rate_limits_fall() {
        let mut slew = SlewRateLimiter::new(0.1);
        // Ramp up to 1.0
        for _ in 0..10 {
            slew.process(1.0);
        }
        assert!(approx(slew.current(), 1.0));
        let out = slew.process(0.0);
        assert!(approx(out, 0.9));
    }

    #[test]
    fn slew_rate_reaches_target() {
        let mut slew = SlewRateLimiter::new(0.25);
        slew.process(0.0);
        for _ in 0..4 {
            slew.process(1.0);
        }
        assert!(approx(slew.current(), 1.0));
    }

    #[test]
    fn slew_rate_zero_is_unlimited() {
        let mut slew = SlewRateLimiter::new(0.0);
        slew.process(0.0);
        assert!(approx(slew.process(1.0), 1.0));
        assert!(approx(slew.process(-1.0), -1.0));
    }

    #[test]
    fn slew_rate_nan_holds_current() {
        let mut slew = SlewRateLimiter::new(0.1);
        slew.process(0.5);
        assert!(approx(slew.process(f64::NAN), 0.5));
    }

    // === ClampStage tests ================================================

    #[test]
    fn clamp_within_range() {
        let mut c = ClampStage::new(-0.5, 0.5);
        assert!(approx(c.process(0.3), 0.3));
    }

    #[test]
    fn clamp_above_max() {
        let mut c = ClampStage::new(-0.5, 0.5);
        assert!(approx(c.process(0.8), 0.5));
    }

    #[test]
    fn clamp_below_min() {
        let mut c = ClampStage::new(-0.5, 0.5);
        assert!(approx(c.process(-0.8), -0.5));
    }

    #[test]
    fn clamp_nan_returns_zero() {
        let mut c = ClampStage::unit();
        assert_eq!(c.process(f64::NAN), 0.0);
    }

    #[test]
    fn clamp_inf_clamped() {
        let mut c = ClampStage::unit();
        assert!(approx(c.process(f64::INFINITY), 1.0));
        assert!(approx(c.process(f64::NEG_INFINITY), -1.0));
    }

    // === InvertStage tests ===============================================

    #[test]
    fn invert_negates() {
        let mut inv = InvertStage::new();
        assert!(approx(inv.process(0.5), -0.5));
        assert!(approx(inv.process(-0.7), 0.7));
    }

    #[test]
    fn invert_zero_unchanged() {
        let mut inv = InvertStage::new();
        assert_eq!(inv.process(0.0), 0.0);
    }

    #[test]
    fn invert_nan_returns_zero() {
        let mut inv = InvertStage::new();
        assert_eq!(inv.process(f64::NAN), 0.0);
    }

    // === RescaleStage tests ==============================================

    #[test]
    fn rescale_identity() {
        let mut r = RescaleStage::new(-1.0, 1.0, -1.0, 1.0);
        assert!(approx(r.process(0.5), 0.5));
        assert!(approx(r.process(-0.5), -0.5));
    }

    #[test]
    fn rescale_unit_to_percentage() {
        let mut r = RescaleStage::new(-1.0, 1.0, 0.0, 100.0);
        assert!(approx(r.process(-1.0), 0.0));
        assert!(approx(r.process(0.0), 50.0));
        assert!(approx(r.process(1.0), 100.0));
    }

    #[test]
    fn rescale_narrow_to_wide() {
        let mut r = RescaleStage::new(0.0, 1.0, -1.0, 1.0);
        assert!(approx(r.process(0.0), -1.0));
        assert!(approx(r.process(0.5), 0.0));
        assert!(approx(r.process(1.0), 1.0));
    }

    #[test]
    fn rescale_zero_input_range() {
        let mut r = RescaleStage::new(0.5, 0.5, 0.0, 1.0);
        assert!(approx(r.process(0.7), 0.0)); // returns out_min
    }

    #[test]
    fn rescale_nan_returns_out_min() {
        let mut r = RescaleStage::new(0.0, 1.0, -1.0, 1.0);
        assert!(approx(r.process(f64::NAN), -1.0));
    }

    // === NoiseGate tests =================================================

    #[test]
    fn noise_gate_first_sample_passes() {
        let mut ng = NoiseGate::new(0.05);
        assert!(approx(ng.process(0.5), 0.5));
    }

    #[test]
    fn noise_gate_suppresses_small_change() {
        let mut ng = NoiseGate::new(0.05);
        ng.process(0.5);
        assert!(approx(ng.process(0.52), 0.5)); // change=0.02 < 0.05
    }

    #[test]
    fn noise_gate_passes_large_change() {
        let mut ng = NoiseGate::new(0.05);
        ng.process(0.5);
        assert!(approx(ng.process(0.6), 0.6)); // change=0.1 >= 0.05
    }

    #[test]
    fn noise_gate_holds_through_jitter() {
        let mut ng = NoiseGate::new(0.05);
        ng.process(0.5);
        ng.process(0.51);
        ng.process(0.49);
        ng.process(0.52);
        assert!(approx(ng.process(0.48), 0.5)); // still within threshold of 0.5
    }

    #[test]
    fn noise_gate_nan_holds_previous() {
        let mut ng = NoiseGate::new(0.05);
        ng.process(0.7);
        assert!(approx(ng.process(f64::NAN), 0.7));
    }

    // === Pipeline composition tests ======================================

    #[test]
    fn pipeline_empty_passthrough() {
        let mut p = RtAxisPipeline::new();
        assert!(approx(p.process(0.75), 0.75));
    }

    #[test]
    fn pipeline_single_stage() {
        let mut p = RtAxisPipeline::builder().clamp(-0.5, 0.5).build();
        assert!(approx(p.process(0.8), 0.5));
    }

    #[test]
    fn pipeline_multi_stage_order() {
        // Deadzone → Curve → Clamp
        let mut p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.1, DeadzoneShape::Linear)
            .curve(CurveType::Expo(1.0))
            .clamp(-1.0, 1.0)
            .build();

        assert_eq!(p.process(0.05), 0.0); // within deadzone

        let out = p.process(0.55);
        // After deadzone: (0.55-0.1)/0.9 = 0.5
        // After expo(1.0): 0.5^2 = 0.25
        assert!(approx(out, 0.25));
    }

    #[test]
    fn pipeline_invert_then_clamp() {
        let mut p = RtAxisPipeline::builder().invert().clamp(-0.5, 0.5).build();
        assert!(approx(p.process(0.8), -0.5));
        assert!(approx(p.process(-0.3), 0.3));
    }

    #[test]
    fn pipeline_insert_stage() {
        let mut p = RtAxisPipeline::builder().clamp(-1.0, 1.0).build();
        assert_eq!(p.stage_count(), 1);

        // Insert invert before clamp
        assert!(p.insert_stage(0, StageSlot::Invert(InvertStage)));
        assert_eq!(p.stage_count(), 2);
        assert_eq!(p.stages()[0].name(), "invert");
        assert_eq!(p.stages()[1].name(), "clamp");

        assert!(approx(p.process(0.5), -0.5));
    }

    #[test]
    fn pipeline_remove_stage() {
        let mut p = RtAxisPipeline::builder().invert().clamp(-0.5, 0.5).build();
        assert_eq!(p.stage_count(), 2);

        assert!(p.remove_stage(0)); // remove invert
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.stages()[0].name(), "clamp");

        assert!(approx(p.process(0.8), 0.5)); // no inversion
    }

    #[test]
    fn pipeline_insert_out_of_range() {
        let mut p = RtAxisPipeline::new();
        assert!(!p.insert_stage(1, StageSlot::Invert(InvertStage)));
    }

    #[test]
    fn pipeline_remove_out_of_range() {
        let mut p = RtAxisPipeline::new();
        assert!(!p.remove_stage(0));
    }

    #[test]
    fn pipeline_max_stages() {
        let mut p = RtAxisPipeline::new();
        for _ in 0..MAX_STAGES {
            assert!(p.push_stage(StageSlot::Clamp(ClampStage::unit())));
        }
        assert_eq!(p.stage_count(), MAX_STAGES);
        assert!(!p.push_stage(StageSlot::Clamp(ClampStage::unit())));
    }

    #[test]
    fn pipeline_stage_count_and_stages() {
        let p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Linear)
            .clamp(-1.0, 1.0)
            .build();
        assert_eq!(p.stage_count(), 3);
        assert_eq!(p.stages()[0].name(), "deadzone");
        assert_eq!(p.stages()[1].name(), "curve");
        assert_eq!(p.stages()[2].name(), "clamp");
    }

    // === Pipeline diagnostics tests ======================================

    #[test]
    fn pipeline_diagnostics_captures_io() {
        let mut p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.1, DeadzoneShape::Linear)
            .clamp(-1.0, 1.0)
            .build();

        let diag = p.diagnostics(0.55);
        assert_eq!(diag.count, 2);
        assert!(approx(diag.raw_input, 0.55));
        assert_eq!(diag.entries[0].name, "deadzone");
        assert!(approx(diag.entries[0].input, 0.55));
        assert!(approx(diag.entries[0].output, 0.5));
        assert_eq!(diag.entries[1].name, "clamp");
        assert!(approx(diag.entries[1].input, 0.5));
        assert!(approx(diag.entries[1].output, 0.5));
        assert!(approx(diag.final_output, 0.5));
    }

    #[test]
    fn pipeline_diagnostics_empty() {
        let mut p = RtAxisPipeline::new();
        let diag = p.diagnostics(0.42);
        assert_eq!(diag.count, 0);
        assert!(approx(diag.raw_input, 0.42));
        assert!(approx(diag.final_output, 0.42));
    }

    // === Full pipeline scenario tests ====================================

    #[test]
    fn full_joystick_pipeline() {
        // Typical joystick: deadzone → expo curve → smoothing → clamp
        let mut p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.03, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .smoothing_ema(0.8)
            .clamp(-1.0, 1.0)
            .build();

        // Process multiple frames
        for _ in 0..10 {
            let out = p.process(0.5);
            assert!(out >= 0.0 && out <= 1.0);
        }
    }

    #[test]
    fn throttle_pipeline_with_slew_rate() {
        // Throttle: rescale 0..1 → slew rate limit → clamp 0..1
        let mut p = RtAxisPipeline::builder()
            .rescale(0.0, 1.0, -1.0, 1.0)
            .slew_rate(0.1)
            .clamp(-1.0, 1.0)
            .build();

        let out1 = p.process(0.5); // rescale → 0.0, slew initializes to 0.0
        assert!(approx(out1, 0.0));
        let out2 = p.process(1.0); // rescale → 1.0, slew limits to 0.0+0.1=0.1
        assert!(approx(out2, 0.1));
    }

    // === Edge cases ======================================================

    #[test]
    fn all_stages_handle_nan() {
        let mut p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.5))
            .smoothing_ema(0.5)
            .slew_rate(0.1)
            .clamp(-1.0, 1.0)
            .invert()
            .rescale(-1.0, 1.0, 0.0, 100.0)
            .noise_gate(0.01)
            .build();

        // Seed with a valid value first
        p.process(0.5);
        let out = p.process(f64::NAN);
        assert!(out.is_finite(), "NaN should not propagate: got {out}");
    }

    #[test]
    fn all_stages_handle_inf() {
        let mut p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.5))
            .clamp(-1.0, 1.0)
            .build();

        let out = p.process(f64::INFINITY);
        assert!(out.is_finite(), "Inf should not propagate: got {out}");
    }

    #[test]
    fn very_large_values_handled() {
        let mut p = RtAxisPipeline::builder().clamp(-1.0, 1.0).build();
        assert!(approx(p.process(1e300), 1.0));
        assert!(approx(p.process(-1e300), -1.0));
    }

    // === Zero-allocation verification ====================================

    #[test]
    fn verify_all_types_are_copy() {
        // Copy types cannot contain heap allocations (Vec, Box, String, etc.)
        fn assert_copy<T: Copy>() {}
        assert_copy::<DeadzoneStage>();
        assert_copy::<CurveStage>();
        assert_copy::<SmoothingStage>();
        assert_copy::<SlewRateLimiter>();
        assert_copy::<ClampStage>();
        assert_copy::<InvertStage>();
        assert_copy::<RescaleStage>();
        assert_copy::<NoiseGate>();
        assert_copy::<StageSlot>();
        assert_copy::<RtAxisPipeline>();
        assert_copy::<RtPipelineBuilder>();
        assert_copy::<StageDiagnostic>();
        assert_copy::<PipelineDiagnostics>();
    }

    #[test]
    fn verify_stack_sizes_reasonable() {
        assert!(
            std::mem::size_of::<StageSlot>() < 512,
            "StageSlot too large: {}",
            std::mem::size_of::<StageSlot>()
        );
        assert!(
            std::mem::size_of::<RtAxisPipeline>() < 8192,
            "RtAxisPipeline too large: {}",
            std::mem::size_of::<RtAxisPipeline>()
        );
        assert!(
            std::mem::size_of::<PipelineDiagnostics>() < 1024,
            "PipelineDiagnostics too large: {}",
            std::mem::size_of::<PipelineDiagnostics>()
        );
    }

    // === Builder tests ===================================================

    #[test]
    fn builder_produces_correct_order() {
        let p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .invert()
            .clamp(-1.0, 1.0)
            .build();
        assert_eq!(p.stage_count(), 3);
        let names: Vec<&str> = p.stages().iter().map(|s| s.name()).collect();
        assert_eq!(names, vec!["deadzone", "invert", "clamp"]);
    }

    #[test]
    fn builder_default_is_empty() {
        let p = RtPipelineBuilder::default().build();
        assert_eq!(p.stage_count(), 0);
    }

    // === StageSlot tests =================================================

    #[test]
    fn stage_slot_empty_passthrough() {
        let mut slot = StageSlot::Empty;
        assert!(approx(slot.process(0.42), 0.42));
        assert!(slot.is_empty());
    }

    #[test]
    fn stage_slot_names_correct() {
        assert_eq!(StageSlot::Empty.name(), "empty");
        assert_eq!(
            StageSlot::Deadzone(DeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear)).name(),
            "deadzone"
        );
        assert_eq!(StageSlot::Curve(CurveStage::linear()).name(), "curve");
        assert_eq!(
            StageSlot::Smoothing(SmoothingStage::ema(0.5)).name(),
            "smoothing"
        );
        assert_eq!(
            StageSlot::SlewRate(SlewRateLimiter::new(0.1)).name(),
            "slew_rate"
        );
        assert_eq!(StageSlot::Clamp(ClampStage::unit()).name(), "clamp");
        assert_eq!(StageSlot::Invert(InvertStage).name(), "invert");
        assert_eq!(
            StageSlot::Rescale(RescaleStage::new(0.0, 1.0, 0.0, 1.0)).name(),
            "rescale"
        );
        assert_eq!(
            StageSlot::NoiseGate(NoiseGate::new(0.01)).name(),
            "noise_gate"
        );
        assert_eq!(StageSlot::Detent(DetentStage::new()).name(), "detent");
        assert_eq!(
            StageSlot::Saturation(SaturationStage::bipolar()).name(),
            "saturation"
        );
    }

    // === DetentStage tests ===============================================

    #[test]
    fn detent_empty_passthrough() {
        let mut d = DetentStage::new();
        assert!(approx(d.process(0.5), 0.5));
        assert!(approx(d.process(-0.3), -0.3));
    }

    #[test]
    fn detent_snaps_within_width() {
        let mut d = DetentStage::new();
        d.add(0.0, 0.05, 1.0); // snap to 0.0 within ±0.05, full strength
        assert!(approx(d.process(0.03), 0.0)); // within width → snap to 0.0
        assert!(approx(d.process(-0.04), 0.0)); // within width → snap to 0.0
        assert!(approx(d.process(0.0), 0.0)); // exact center
    }

    #[test]
    fn detent_passthrough_outside_width() {
        let mut d = DetentStage::new();
        d.add(0.0, 0.05, 1.0);
        assert!(approx(d.process(0.1), 0.1)); // outside width → unchanged
        assert!(approx(d.process(-0.5), -0.5));
        assert!(approx(d.process(1.0), 1.0));
    }

    #[test]
    fn detent_partial_strength() {
        let mut d = DetentStage::new();
        d.add(0.5, 0.1, 0.5); // half-strength snap
        // Input 0.45, distance=0.05 < width=0.1: lerp = 0.45 + (0.5 - 0.45) * 0.5 = 0.475
        let out = d.process(0.45);
        assert!(approx(out, 0.475));
    }

    #[test]
    fn detent_zero_strength_passthrough() {
        let mut d = DetentStage::new();
        d.add(0.5, 0.1, 0.0); // zero strength
        assert!(approx(d.process(0.45), 0.45)); // within width but no snap
    }

    #[test]
    fn detent_multiple_positions() {
        let mut d = DetentStage::new();
        d.add(0.0, 0.03, 1.0); // idle
        d.add(0.5, 0.03, 1.0); // climb
        d.add(1.0, 0.03, 1.0); // TOGA
        assert!(approx(d.process(0.01), 0.0)); // near idle → snap
        assert!(approx(d.process(0.49), 0.5)); // near climb → snap
        assert!(approx(d.process(0.99), 1.0)); // near TOGA → snap
        assert!(approx(d.process(0.3), 0.3)); // free range → passthrough
    }

    #[test]
    fn detent_from_positions_slice() {
        let positions = [
            DetentPosition::new(0.0, 0.05, 1.0),
            DetentPosition::new(1.0, 0.05, 1.0),
        ];
        let mut d = DetentStage::from_positions(&positions).unwrap();
        assert_eq!(d.count(), 2);
        assert!(approx(d.process(0.02), 0.0));
        assert!(approx(d.process(0.98), 1.0));
    }

    #[test]
    fn detent_from_positions_too_many() {
        let positions = [DetentPosition::new(0.0, 0.05, 1.0); MAX_DETENTS + 1];
        assert!(DetentStage::from_positions(&positions).is_none());
    }

    #[test]
    fn detent_add_returns_false_when_full() {
        let mut d = DetentStage::new();
        for i in 0..MAX_DETENTS {
            assert!(d.add(i as f64 * 0.1, 0.01, 1.0));
        }
        assert!(!d.add(0.9, 0.01, 1.0)); // 9th detent rejected
    }

    #[test]
    fn detent_nan_returns_zero() {
        let mut d = DetentStage::new();
        d.add(0.0, 0.05, 1.0);
        assert_eq!(d.process(f64::NAN), 0.0);
    }

    #[test]
    fn detent_inf_returns_zero() {
        let mut d = DetentStage::new();
        d.add(0.0, 0.05, 1.0);
        assert_eq!(d.process(f64::INFINITY), 0.0);
    }

    // === SaturationStage tests ==========================================

    #[test]
    fn saturation_bipolar_clamps() {
        let mut s = SaturationStage::bipolar();
        assert!(approx(s.process(0.5), 0.5)); // within range
        assert!(approx(s.process(1.5), 1.0)); // above max
        assert!(approx(s.process(-1.5), -1.0)); // below min
        assert!(approx(s.process(1.0), 1.0)); // at boundary
        assert!(approx(s.process(-1.0), -1.0)); // at boundary
    }

    #[test]
    fn saturation_unipolar_clamps() {
        let mut s = SaturationStage::unipolar();
        assert!(approx(s.process(0.5), 0.5));
        assert!(approx(s.process(1.5), 1.0));
        assert!(approx(s.process(-0.5), 0.0));
        assert!(approx(s.process(0.0), 0.0));
        assert!(approx(s.process(1.0), 1.0));
    }

    #[test]
    fn saturation_custom_range() {
        let mut s = SaturationStage::new(-0.5, 0.5);
        assert!(approx(s.process(0.3), 0.3));
        assert!(approx(s.process(0.8), 0.5));
        assert!(approx(s.process(-0.8), -0.5));
    }

    #[test]
    fn saturation_accessors() {
        let s = SaturationStage::new(-0.5, 0.75);
        assert!(approx(s.min(), -0.5));
        assert!(approx(s.max(), 0.75));
    }

    #[test]
    fn saturation_nan_returns_zero() {
        let mut s = SaturationStage::bipolar();
        assert_eq!(s.process(f64::NAN), 0.0);
    }

    #[test]
    fn saturation_inf_clamped() {
        let mut s = SaturationStage::bipolar();
        assert!(approx(s.process(f64::INFINITY), 1.0));
        assert!(approx(s.process(f64::NEG_INFINITY), -1.0));
    }

    // === Pipeline with new stages ========================================

    #[test]
    fn pipeline_with_detent_stage() {
        let detents = [
            DetentPosition::new(0.0, 0.05, 1.0),
            DetentPosition::new(1.0, 0.05, 1.0),
        ];
        let mut p = RtAxisPipeline::builder()
            .detent(&detents)
            .saturation_unipolar()
            .build();
        assert_eq!(p.stage_count(), 2);
        assert!(approx(p.process(0.02), 0.0)); // detent snap
        assert!(approx(p.process(0.5), 0.5)); // passthrough
    }

    #[test]
    fn pipeline_detent_then_saturation() {
        let detents = [DetentPosition::new(0.0, 0.1, 1.0)];
        let mut p = RtAxisPipeline::builder()
            .detent(&detents)
            .saturation(-1.0, 1.0)
            .build();
        assert!(approx(p.process(0.05), 0.0)); // detent snap then saturation pass
        assert!(approx(p.process(1.5), 1.0)); // detent pass, saturation clamp
    }

    #[test]
    fn full_throttle_pipeline_with_detents() {
        let detents = [
            DetentPosition::new(0.0, 0.03, 1.0), // idle
            DetentPosition::new(0.5, 0.03, 1.0), // climb
            DetentPosition::new(1.0, 0.03, 1.0), // TOGA
        ];
        let mut p = RtAxisPipeline::builder()
            .deadzone(0.0, 0.02, DeadzoneShape::Linear)
            .detent(&detents)
            .slew_rate(0.05)
            .saturation_unipolar()
            .build();

        // Multiple frames - output should always be in [0.0, 1.0]
        for input in [0.0, 0.01, 0.3, 0.49, 0.5, 0.75, 0.99, 1.0] {
            let out = p.process(input);
            assert!(
                (0.0..=1.0).contains(&out),
                "output {out} out of [0.0, 1.0] for input={input}"
            );
        }
    }

    // === Updated zero-allocation verification ============================

    #[test]
    fn verify_new_types_are_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<DetentPosition>();
        assert_copy::<DetentStage>();
        assert_copy::<SaturationStage>();
    }
}
