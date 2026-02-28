// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis accumulator mode for rotary encoders.
//!
//! Sums delta inputs to an absolute position, clamped to `[min, max]`.
//! Zero-allocation. RT-safe.

/// Configuration for [`AxisAccumulator`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccumulatorConfig {
    /// Minimum output value.
    pub min: f32,
    /// Maximum output value.
    pub max: f32,
    /// Delta scale factor (e.g., `1.0` means each click is 1 unit).
    pub scale: f32,
    /// Wrap mode: if `true`, wrap around at limits instead of clamping.
    pub wrap: bool,
}

impl AccumulatorConfig {
    /// Creates a clamping accumulator config.
    pub const fn new(min: f32, max: f32, scale: f32) -> Self {
        Self {
            min,
            max,
            scale,
            wrap: false,
        }
    }

    /// Creates a wrapping accumulator config.
    pub const fn wrapping(min: f32, max: f32, scale: f32) -> Self {
        Self {
            min,
            max,
            scale,
            wrap: true,
        }
    }
}

impl Default for AccumulatorConfig {
    fn default() -> Self {
        Self::new(-1.0, 1.0, 0.1)
    }
}

/// Accumulates delta inputs (e.g., rotary encoder clicks) to an absolute position.
///
/// # Real-time safety
/// - Zero allocations on the hot path.
/// - No locks or blocking operations.
pub struct AxisAccumulator {
    config: AccumulatorConfig,
    position: f32,
}

impl AxisAccumulator {
    /// Creates a new accumulator with the given config, starting at position `0.0`
    /// (clamped to `[min, max]`).
    pub fn new(config: AccumulatorConfig) -> Self {
        let position = 0.0_f32.clamp(config.min, config.max);
        Self { config, position }
    }

    /// Applies a delta and returns the new accumulated position.
    ///
    /// The position is either clamped or wrapped within `[min, max]` depending on
    /// [`AccumulatorConfig::wrap`].
    #[inline]
    pub fn update(&mut self, delta: f32) -> f32 {
        let new_pos = self.position + delta * self.config.scale;
        self.position = if self.config.wrap {
            wrap_range(new_pos, self.config.min, self.config.max)
        } else {
            new_pos.clamp(self.config.min, self.config.max)
        };
        self.position
    }

    /// Returns the current accumulated position.
    #[inline]
    pub fn position(&self) -> f32 {
        self.position
    }

    /// Resets the position to `0.0`, clamped to `[min, max]`.
    #[inline]
    pub fn reset_to_zero(&mut self) {
        self.position = 0.0_f32.clamp(self.config.min, self.config.max);
    }

    /// Returns a reference to the current configuration.
    #[inline]
    pub fn config(&self) -> &AccumulatorConfig {
        &self.config
    }
}

/// Wraps `value` into `[min, max)`.
fn wrap_range(value: f32, min: f32, max: f32) -> f32 {
    let range = max - min;
    if range <= 0.0 {
        return min;
    }
    let shifted = value - min;
    let wrapped = shifted - (shifted / range).floor() * range;
    min + wrapped
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-6;

    fn default_acc() -> AxisAccumulator {
        AxisAccumulator::new(AccumulatorConfig::new(-1.0, 1.0, 0.1))
    }

    #[test]
    fn test_accumulate_positive_deltas() {
        let mut acc = default_acc();
        for _ in 0..5 {
            acc.update(1.0);
        }
        assert!((acc.position() - 0.5).abs() < EPS, "got {}", acc.position());
    }

    #[test]
    fn test_accumulate_negative_deltas() {
        let mut acc = default_acc();
        // Start at 0.5 by applying five +1 clicks.
        for _ in 0..5 {
            acc.update(1.0);
        }
        assert!((acc.position() - 0.5).abs() < EPS);
        // Then apply five -1 clicks to return to 0.0.
        for _ in 0..5 {
            acc.update(-1.0);
        }
        assert!(acc.position().abs() < EPS, "got {}", acc.position());
    }

    #[test]
    fn test_clamp_at_max() {
        let mut acc = default_acc();
        for _ in 0..100 {
            acc.update(1.0);
        }
        assert_eq!(acc.position(), 1.0);
    }

    #[test]
    fn test_clamp_at_min() {
        let mut acc = default_acc();
        for _ in 0..100 {
            acc.update(-1.0);
        }
        assert_eq!(acc.position(), -1.0);
    }

    #[test]
    fn test_wrap_positive() {
        // Range [0.0, 1.0], scale 0.1, wrap=true.
        // 11 clicks of +1 from 0.0 → 1.1 → wraps to ~0.1.
        let mut acc = AxisAccumulator::new(AccumulatorConfig::wrapping(0.0, 1.0, 0.1));
        for _ in 0..11 {
            acc.update(1.0);
        }
        assert!((acc.position() - 0.1).abs() < EPS, "got {}", acc.position());
    }

    #[test]
    fn test_wrap_negative() {
        // Range [0.0, 1.0], scale 0.1, wrap=true, start at 0.0.
        // One click of -1 → -0.1 → wraps to 0.9.
        let mut acc = AxisAccumulator::new(AccumulatorConfig::wrapping(0.0, 1.0, 0.1));
        acc.update(-1.0);
        assert!((acc.position() - 0.9).abs() < EPS, "got {}", acc.position());
    }

    #[test]
    fn test_reset_to_zero() {
        let mut acc = default_acc();
        for _ in 0..5 {
            acc.update(1.0);
        }
        assert!((acc.position() - 0.5).abs() < EPS);
        acc.reset_to_zero();
        assert!(
            acc.position().abs() < EPS,
            "after reset: {}",
            acc.position()
        );
    }

    #[test]
    fn test_filter_trait() {
        // Verify update() behaves as a stateful single-value processor (analogous to a filter).
        let mut acc = default_acc();
        let out1 = acc.update(1.0);
        assert!((out1 - 0.1).abs() < EPS);
        let out2 = acc.update(1.0);
        assert!((out2 - 0.2).abs() < EPS);
        acc.reset_to_zero();
        assert!(acc.position().abs() < EPS);
    }

    #[test]
    fn test_scale_factor() {
        // scale=0.5: each delta +1 → position increases by 0.5.
        let mut acc = AxisAccumulator::new(AccumulatorConfig::new(-10.0, 10.0, 0.5));
        let out = acc.update(1.0);
        assert!((out - 0.5).abs() < EPS, "got {out}");
        let out2 = acc.update(1.0);
        assert!((out2 - 1.0).abs() < EPS, "got {out2}");
    }
}
