// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-axis peak output tracking across a session (REQ-672).
//!
//! Records the highest positive and lowest negative values seen.
//! Zero-allocation. RT-safe (ADR-004).

/// Tracks peak positive and negative axis output values.
///
/// # Real-time safety
/// - Zero allocations on the hot path.
/// - No locks or blocking operations.
#[derive(Debug, Clone, Copy)]
pub struct PeakHold {
    peak_pos: f32,
    peak_neg: f32,
}

impl PeakHold {
    /// Creates a new `PeakHold` with both peaks at zero.
    pub const fn new() -> Self {
        Self {
            peak_pos: 0.0,
            peak_neg: 0.0,
        }
    }

    /// Updates the peak tracker with a new value and returns it unchanged.
    ///
    /// Zero-allocation — safe to call from RT code.
    #[inline]
    pub fn update(&mut self, value: f32) -> f32 {
        if value > self.peak_pos {
            self.peak_pos = value;
        }
        if value < self.peak_neg {
            self.peak_neg = value;
        }
        value
    }

    /// Returns the highest positive value recorded since last reset.
    #[inline]
    pub fn peak_positive(&self) -> f32 {
        self.peak_pos
    }

    /// Returns the lowest negative value recorded since last reset.
    #[inline]
    pub fn peak_negative(&self) -> f32 {
        self.peak_neg
    }

    /// Resets both peaks to zero.
    #[inline]
    pub fn reset(&mut self) {
        self.peak_pos = 0.0;
        self.peak_neg = 0.0;
    }
}

impl Default for PeakHold {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_peaks_are_zero() {
        let ph = PeakHold::new();
        assert_eq!(ph.peak_positive(), 0.0);
        assert_eq!(ph.peak_negative(), 0.0);
    }

    #[test]
    fn positive_spike_tracked() {
        let mut ph = PeakHold::new();
        ph.update(0.5);
        ph.update(0.9);
        ph.update(0.3);
        assert_eq!(ph.peak_positive(), 0.9);
        assert_eq!(ph.peak_negative(), 0.0);
    }

    #[test]
    fn negative_spike_tracked() {
        let mut ph = PeakHold::new();
        ph.update(-0.2);
        ph.update(-0.8);
        ph.update(-0.1);
        assert_eq!(ph.peak_negative(), -0.8);
        assert_eq!(ph.peak_positive(), 0.0);
    }

    #[test]
    fn reset_clears_peaks() {
        let mut ph = PeakHold::new();
        ph.update(0.9);
        ph.update(-0.7);
        assert_eq!(ph.peak_positive(), 0.9);
        assert_eq!(ph.peak_negative(), -0.7);

        ph.reset();
        assert_eq!(ph.peak_positive(), 0.0);
        assert_eq!(ph.peak_negative(), 0.0);
    }

    #[test]
    fn update_returns_value_unchanged() {
        let mut ph = PeakHold::new();
        assert_eq!(ph.update(0.42), 0.42);
        assert_eq!(ph.update(-0.13), -0.13);
    }
}
