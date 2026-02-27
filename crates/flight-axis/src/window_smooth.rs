// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Windowed moving average filter.
//!
//! Provides both a compile-time const-generic [`WindowSmoother<W>`] and a
//! runtime-configurable [`DynWindowSmoother`]. Both use fixed-size arrays —
//! no heap allocation — satisfying the RT zero-allocation constraint (ADR-004).

use thiserror::Error;

/// Errors returned by [`DynWindowSmoother::new`].
#[derive(Debug, Error, PartialEq)]
pub enum WindowSmoothError {
    /// Requested window size is zero or larger than the maximum (64).
    #[error("window size must be between 1 and 64, got {0}")]
    InvalidWindowSize(usize),
}

/// Runtime window size configuration for [`DynWindowSmoother`].
pub struct WindowSmoothConfig {
    /// Number of samples in the sliding window. Valid range: 1–64.
    pub window: usize,
}

/// Windowed moving average filter with a compile-time constant window size.
///
/// Uses a fixed-size ring buffer. No heap allocation.
#[derive(Debug, Clone, Copy)]
pub struct WindowSmoother<const W: usize> {
    buffer: [f32; W],
    index: usize,
    /// Number of samples inserted so far (saturates at `W`).
    count: usize,
}

impl<const W: usize> WindowSmoother<W> {
    /// Creates a new, empty `WindowSmoother`.
    pub const fn new() -> Self {
        Self {
            buffer: [0.0; W],
            index: 0,
            count: 0,
        }
    }

    /// Pushes `sample` into the window and returns the current windowed average.
    ///
    /// Uses only the samples inserted so far (partial window) until the buffer
    /// is full. Once full, the oldest sample is overwritten.
    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        self.buffer[self.index] = sample;
        self.index = (self.index + 1) % W;
        if self.count < W {
            self.count += 1;
        }
        let sum: f32 = self.buffer[..self.count].iter().sum();
        sum / self.count as f32
    }

    /// Resets the smoother to its initial empty state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Returns the number of samples currently held in the window.
    pub fn count(&self) -> usize {
        self.count
    }
}

impl<const W: usize> Default for WindowSmoother<W> {
    fn default() -> Self {
        Self::new()
    }
}

/// Runtime-configurable windowed moving average filter.
///
/// Uses a fixed `[f32; 64]` backing array — no heap allocation.
/// Supports window sizes 1–64.
#[derive(Debug, PartialEq)]
pub struct DynWindowSmoother {
    buffer: [f32; 64],
    window: usize,
    index: usize,
    count: usize,
}

impl DynWindowSmoother {
    /// Creates a new `DynWindowSmoother` with the given window size.
    ///
    /// # Errors
    ///
    /// Returns [`WindowSmoothError::InvalidWindowSize`] if `window` is 0 or greater than 64.
    pub fn new(window: usize) -> Result<Self, WindowSmoothError> {
        if window == 0 || window > 64 {
            return Err(WindowSmoothError::InvalidWindowSize(window));
        }
        Ok(Self {
            buffer: [0.0; 64],
            window,
            index: 0,
            count: 0,
        })
    }

    /// Pushes `sample` into the window and returns the current windowed average.
    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        self.buffer[self.index] = sample;
        self.index = (self.index + 1) % self.window;
        if self.count < self.window {
            self.count += 1;
        }
        let sum: f32 = self.buffer[..self.count].iter().sum();
        sum / self.count as f32
    }

    /// Resets the smoother to its initial empty state (preserves window size).
    pub fn reset(&mut self) {
        self.buffer = [0.0; 64];
        self.index = 0;
        self.count = 0;
    }

    /// Returns the configured window size.
    pub fn window(&self) -> usize {
        self.window
    }

    /// Returns the number of samples currently held in the window.
    pub fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_1_is_passthrough() {
        let mut s = DynWindowSmoother::new(1).unwrap();
        assert!((s.process(0.42) - 0.42).abs() < 1e-6);
        assert!((s.process(-0.7) - (-0.7)).abs() < 1e-6);
        assert!((s.process(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_window_2_averages_two_samples() {
        let mut s = DynWindowSmoother::new(2).unwrap();
        let out1 = s.process(0.0);
        assert!((out1 - 0.0).abs() < 1e-6);
        let out2 = s.process(1.0);
        assert!((out2 - 0.5).abs() < 1e-6);
        // Third sample: window slides, oldest (0.0) is replaced by third sample (0.0)
        let out3 = s.process(0.0);
        assert!((out3 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_window_4_correct_average() {
        let mut s = DynWindowSmoother::new(4).unwrap();
        s.process(1.0);
        s.process(2.0);
        s.process(3.0);
        let out = s.process(4.0);
        // Window is full: (1+2+3+4)/4 = 2.5
        assert!((out - 2.5).abs() < 1e-5);
    }

    #[test]
    fn test_reset_clears_buffer() {
        let mut s = DynWindowSmoother::new(4).unwrap();
        s.process(1.0);
        s.process(1.0);
        s.reset();
        assert_eq!(s.count(), 0);
        let out = s.process(0.5);
        // After reset, only one sample: average = 0.5
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_window_size_larger_than_64_invalid() {
        assert_eq!(
            DynWindowSmoother::new(65),
            Err(WindowSmoothError::InvalidWindowSize(65))
        );
        assert_eq!(
            DynWindowSmoother::new(0),
            Err(WindowSmoothError::InvalidWindowSize(0))
        );
    }

    #[test]
    fn test_initial_samples_use_partial_window() {
        let mut s = DynWindowSmoother::new(4).unwrap();
        let out1 = s.process(0.4);
        assert!((out1 - 0.4).abs() < 1e-6, "1 sample: avg={out1}");
        let out2 = s.process(0.8);
        // (0.4 + 0.8) / 2 = 0.6
        assert!((out2 - 0.6).abs() < 1e-5, "2 samples: avg={out2}");
    }

    #[test]
    fn test_output_bounded_by_input_range() {
        let mut s = DynWindowSmoother::new(8).unwrap();
        let inputs = [-1.0f32, -0.5, 0.0, 0.5, 1.0, 0.3, -0.3, 0.7];
        for &input in &inputs {
            let out = s.process(input);
            assert!(
                out >= -1.0 && out <= 1.0,
                "out={out} outside [-1.0, 1.0] for input={input}"
            );
        }
    }

    #[test]
    fn test_const_generic_window_smoother_w4() {
        let mut s = WindowSmoother::<4>::new();
        s.process(1.0);
        s.process(2.0);
        s.process(3.0);
        let out = s.process(4.0);
        assert!((out - 2.5).abs() < 1e-5, "out={out}");
        assert_eq!(s.count(), 4);
    }

    #[test]
    fn test_window_smoother_tracks_constant_input() {
        let mut s = DynWindowSmoother::new(8).unwrap();
        for _ in 0..20 {
            let out = s.process(0.75);
            assert!(
                (out - 0.75).abs() < 1e-5,
                "constant input should converge immediately, got {out}"
            );
        }
    }

    #[test]
    fn test_window_smoother_ramp_latency() {
        // Ramp from 0 to 1 over 8 samples, window=4.
        // The moving average should lag behind the ramp.
        let mut s = DynWindowSmoother::new(4).unwrap();
        let mut last_out = 0.0f32;
        for i in 0..8u32 {
            let input = i as f32 / 7.0; // 0.0 → 1.0
            let out = s.process(input);
            // Output must not overshoot input
            assert!(out <= input + 1e-5, "out={out} > input={input}");
            last_out = out;
        }
        // After the ramp completes, output should be below 1.0 (lagged)
        // but approaching it. With window=4 and last 4 inputs being ~0.57..1.0,
        // the final average is < 1.0.
        assert!(
            last_out < 1.0,
            "expected output to lag behind ramp top, got {last_out}"
        );
    }

    #[test]
    fn test_const_generic_window_smoother_default() {
        let s = WindowSmoother::<4>::default();
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn test_dyn_window_smoother_max_window_64() {
        let mut s = DynWindowSmoother::new(64).unwrap();
        for _ in 0..64 {
            s.process(1.0);
        }
        assert!((s.process(1.0) - 1.0).abs() < 1e-5);
    }
}
