// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Virtual output frame generation with optional low-pass smoothing.
//!
//! [`VirtualOutput`] takes the current controller state and produces
//! [`OutputFrame`]s suitable for consumption by simulator adapters.

use crate::backend::HatDirection;
use crate::virtual_controller::ControllerSnapshot;

/// Configuration for [`VirtualOutput`].
#[derive(Debug, Clone)]
pub struct VirtualOutputConfig {
    /// Target output rate in Hz.
    pub output_rate_hz: f64,
    /// Enable low-pass smoothing on axis values.
    pub smoothing_enabled: bool,
    /// Smoothing alpha (0.0 = no update, 1.0 = no smoothing). Default 0.2.
    pub smoothing_alpha: f64,
}

impl Default for VirtualOutputConfig {
    fn default() -> Self {
        Self {
            output_rate_hz: 250.0,
            smoothing_enabled: false,
            smoothing_alpha: 0.2,
        }
    }
}

/// A single output frame produced by [`VirtualOutput`].
#[derive(Debug, Clone)]
pub struct OutputFrame {
    /// Smoothed axis values.
    pub axes: Vec<f64>,
    /// Button states.
    pub buttons: Vec<bool>,
    /// Hat switch directions.
    pub hats: Vec<HatDirection>,
    /// Cumulative timestamp in seconds since the first frame.
    pub timestamp: f64,
}

/// Produces simulator-compatible output frames from controller snapshots.
pub struct VirtualOutput {
    config: VirtualOutputConfig,
    /// Smoothed axis state carried between frames.
    smoothed_axes: Vec<f64>,
    /// Minimum interval between frames (seconds).
    min_interval: f64,
    /// Time accumulated since the last emitted frame.
    accumulated_time: f64,
    /// Cumulative wall-clock time fed into the output.
    total_time: f64,
}

impl VirtualOutput {
    /// Create a new output stage.
    pub fn new(config: VirtualOutputConfig) -> Self {
        let min_interval = if config.output_rate_hz > 0.0 {
            1.0 / config.output_rate_hz
        } else {
            0.0
        };
        Self {
            config,
            smoothed_axes: Vec::new(),
            min_interval,
            accumulated_time: 0.0,
            total_time: 0.0,
        }
    }

    /// Borrow the configuration.
    pub fn config(&self) -> &VirtualOutputConfig {
        &self.config
    }

    /// Compute the next output frame from the given snapshot.
    ///
    /// `dt_s` is the elapsed wall-clock time since the previous call.
    /// Returns `None` if the rate limiter determines it is too early.
    pub fn compute_frame(
        &mut self,
        snapshot: &ControllerSnapshot,
        dt_s: f64,
    ) -> Option<OutputFrame> {
        self.accumulated_time += dt_s;
        self.total_time += dt_s;

        if self.accumulated_time < self.min_interval {
            return None;
        }
        self.accumulated_time -= self.min_interval;

        // Grow smoothed state to match snapshot.
        if self.smoothed_axes.len() < snapshot.axes.len() {
            self.smoothed_axes.resize(snapshot.axes.len(), 0.0);
        }

        let axes = if self.config.smoothing_enabled {
            let alpha = self.config.smoothing_alpha.clamp(0.0, 1.0);
            for (smoothed, &raw) in self.smoothed_axes.iter_mut().zip(snapshot.axes.iter()) {
                *smoothed += alpha * (raw - *smoothed);
            }
            self.smoothed_axes.clone()
        } else {
            // Copy raw values into smoothed state so a later enable is seamless.
            self.smoothed_axes.copy_from_slice(&snapshot.axes);
            snapshot.axes.clone()
        };

        Some(OutputFrame {
            axes,
            buttons: snapshot.buttons.clone(),
            hats: snapshot.hats.clone(),
            timestamp: self.total_time,
        })
    }

    /// Reset internal smoothing state and timing.
    pub fn reset(&mut self) {
        self.smoothed_axes.clear();
        self.accumulated_time = 0.0;
        self.total_time = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_snapshot(axis_value: f64) -> ControllerSnapshot {
        ControllerSnapshot {
            axes: vec![axis_value; 2],
            buttons: vec![false; 4],
            hats: vec![HatDirection::Centered],
        }
    }

    #[test]
    fn test_frame_generation_basic() {
        let config = VirtualOutputConfig {
            output_rate_hz: 250.0,
            smoothing_enabled: false,
            ..Default::default()
        };
        let mut out = VirtualOutput::new(config);
        let snap = default_snapshot(0.5);

        // First frame with enough dt should succeed.
        let frame = out.compute_frame(&snap, 0.01).unwrap();
        assert_eq!(frame.axes.len(), 2);
        assert!((frame.axes[0] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rate_limiting() {
        let config = VirtualOutputConfig {
            output_rate_hz: 100.0, // 10ms intervals
            smoothing_enabled: false,
            ..Default::default()
        };
        let mut out = VirtualOutput::new(config);
        let snap = default_snapshot(0.0);

        // First call with 10ms dt → frame.
        assert!(out.compute_frame(&snap, 0.01).is_some());
        // Immediately again with tiny dt → rate limited.
        assert!(out.compute_frame(&snap, 0.001).is_none());
        // After accumulating enough time → another frame.
        assert!(out.compute_frame(&snap, 0.01).is_some());
    }

    #[test]
    fn test_smoothing_dampens_step() {
        let config = VirtualOutputConfig {
            output_rate_hz: 250.0,
            smoothing_enabled: true,
            smoothing_alpha: 0.2,
        };
        let mut out = VirtualOutput::new(config);

        // Start at 0.
        let snap_zero = default_snapshot(0.0);
        let _ = out.compute_frame(&snap_zero, 0.004);

        // Step to 1.0 — smoothed value must lag.
        let snap_one = default_snapshot(1.0);
        let frame = out.compute_frame(&snap_one, 0.004).unwrap();
        assert!(
            frame.axes[0] < 0.5,
            "smoothed should lag: {}",
            frame.axes[0]
        );
        assert!(frame.axes[0] > 0.0, "smoothed should start moving");
    }

    #[test]
    fn test_smoothing_converges() {
        let config = VirtualOutputConfig {
            output_rate_hz: 1000.0,
            smoothing_enabled: true,
            smoothing_alpha: 0.3,
        };
        let mut out = VirtualOutput::new(config);
        let snap = default_snapshot(1.0);

        let mut last_value = 0.0;
        for _ in 0..100 {
            if let Some(frame) = out.compute_frame(&snap, 0.001) {
                last_value = frame.axes[0];
            }
        }
        assert!(
            (last_value - 1.0).abs() < 0.01,
            "should converge to target: {last_value}"
        );
    }

    #[test]
    fn test_timestamp_advances() {
        let config = VirtualOutputConfig::default();
        let mut out = VirtualOutput::new(config);
        let snap = default_snapshot(0.0);

        let f1 = out.compute_frame(&snap, 0.01).unwrap();
        let f2 = out.compute_frame(&snap, 0.01).unwrap();
        assert!(f2.timestamp > f1.timestamp);
    }

    #[test]
    fn test_buttons_and_hats_passed_through() {
        let config = VirtualOutputConfig {
            smoothing_enabled: false,
            ..Default::default()
        };
        let mut out = VirtualOutput::new(config);
        let snap = ControllerSnapshot {
            axes: vec![0.0],
            buttons: vec![true, false, true],
            hats: vec![HatDirection::North],
        };

        let frame = out.compute_frame(&snap, 0.01).unwrap();
        assert_eq!(frame.buttons, vec![true, false, true]);
        assert_eq!(frame.hats, vec![HatDirection::North]);
    }

    #[test]
    fn test_reset_clears_state() {
        let config = VirtualOutputConfig {
            smoothing_enabled: true,
            smoothing_alpha: 0.1,
            ..Default::default()
        };
        let mut out = VirtualOutput::new(config);
        let snap = default_snapshot(1.0);

        // Drive smoothing up.
        for _ in 0..50 {
            let _ = out.compute_frame(&snap, 0.004);
        }
        out.reset();

        // After reset, smoothing starts from 0 again.
        let snap_step = default_snapshot(1.0);
        let frame = out.compute_frame(&snap_step, 0.004).unwrap();
        assert!(frame.axes[0] < 0.5, "should be near 0 after reset");
    }

    #[test]
    fn test_zero_rate_always_emits() {
        let config = VirtualOutputConfig {
            output_rate_hz: 0.0,
            smoothing_enabled: false,
            ..Default::default()
        };
        let mut out = VirtualOutput::new(config);
        let snap = default_snapshot(0.0);

        // With 0 Hz rate, every call should produce a frame.
        assert!(out.compute_frame(&snap, 0.0).is_some());
        assert!(out.compute_frame(&snap, 0.0).is_some());
    }
}
