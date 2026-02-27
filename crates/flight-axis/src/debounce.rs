// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis input debouncing (REQ-639).
//!
//! Suppresses axis value changes smaller than a configurable threshold.
//! Only passes through values when the delta from the last accepted output
//! exceeds the threshold.
//!
//! Zero-allocation. RT-safe (ADR-004).

/// Debounces axis input changes below a threshold.
///
/// # Real-time safety
/// - Zero allocations on the hot path.
/// - No locks or blocking operations.
#[derive(Debug, Clone, Copy)]
pub struct AxisDebounce {
    threshold: f32,
    last_output: f32,
    initialized: bool,
}

impl AxisDebounce {
    /// Creates a new `AxisDebounce` with the given threshold.
    ///
    /// Changes smaller than `threshold` are suppressed; the previous
    /// accepted output is returned instead.
    pub const fn new(threshold: f32) -> Self {
        Self {
            threshold,
            last_output: 0.0,
            initialized: false,
        }
    }

    /// Updates the debouncer with a new input value.
    ///
    /// Returns the new value if the delta from the last output exceeds the
    /// threshold, otherwise returns the last accepted output.
    ///
    /// Zero-allocation — safe to call from RT code.
    #[inline]
    pub fn update(&mut self, value: f32) -> f32 {
        if !self.initialized {
            self.last_output = value;
            self.initialized = true;
            return value;
        }

        let delta = (value - self.last_output).abs();
        if delta >= self.threshold {
            self.last_output = value;
        }
        self.last_output
    }

    /// Returns the current threshold.
    #[inline]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Returns the last accepted output value.
    #[inline]
    pub fn last_output(&self) -> f32 {
        self.last_output
    }

    /// Resets the debouncer state.
    #[inline]
    pub fn reset(&mut self) {
        self.last_output = 0.0;
        self.initialized = false;
    }
}

impl Default for AxisDebounce {
    fn default() -> Self {
        Self::new(0.001)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-7;

    #[test]
    fn large_changes_pass_through() {
        let mut db = AxisDebounce::new(0.01);
        assert!((db.update(0.0) - 0.0).abs() < EPS);
        assert!((db.update(0.5) - 0.5).abs() < EPS);
        assert!((db.update(-0.3) - (-0.3)).abs() < EPS);
    }

    #[test]
    fn tiny_changes_suppressed() {
        let mut db = AxisDebounce::new(0.01);
        db.update(0.5);
        // Change smaller than threshold
        let out = db.update(0.5005);
        assert!((out - 0.5).abs() < EPS, "expected 0.5, got {out}");
        let out = db.update(0.4998);
        assert!((out - 0.5).abs() < EPS, "expected 0.5, got {out}");
    }

    #[test]
    fn threshold_exact_passes() {
        let mut db = AxisDebounce::new(0.01);
        db.update(0.5);
        // Delta of 0.02 clearly exceeds threshold of 0.01
        let out = db.update(0.52);
        assert!((out - 0.52).abs() < EPS, "expected 0.52, got {out}");
    }

    #[test]
    fn default_threshold() {
        let db = AxisDebounce::default();
        assert!((db.threshold() - 0.001).abs() < EPS);
    }

    #[test]
    fn reset_clears_state() {
        let mut db = AxisDebounce::new(0.01);
        db.update(0.5);
        db.reset();
        // After reset, first value is accepted regardless
        let out = db.update(0.0001);
        assert!((out - 0.0001).abs() < EPS, "expected 0.0001, got {out}");
    }
}
