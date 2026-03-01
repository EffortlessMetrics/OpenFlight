// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fake HID device that produces deterministic input reports on demand.

use std::sync::atomic::{AtomicU64, Ordering};

/// A single HID report containing axes, buttons, and hat-switch values.
#[derive(Debug, Clone, PartialEq)]
pub struct HidReport {
    /// Axis values, normalised to `[-1.0, 1.0]`.
    pub axes: Vec<f64>,
    /// Button states (`true` = pressed).
    pub buttons: Vec<bool>,
    /// Hat-switch positions (`None` = centred, `Some(deg)` = direction).
    pub hats: Vec<Option<u16>>,
}

/// Configuration for jitter injection on report timing.
#[derive(Debug, Clone, Copy)]
pub struct JitterConfig {
    /// Maximum delay variance in microseconds added to each report.
    pub max_variance_us: u64,
    /// Simple LCG seed — deterministic pseudo-random.
    seed: u64,
}

impl JitterConfig {
    /// Create a new jitter config with the given maximum variance.
    pub fn new(max_variance_us: u64) -> Self {
        Self {
            max_variance_us,
            seed: 12345,
        }
    }

    /// Return the next jitter value in microseconds and advance the PRNG.
    fn next(&mut self) -> u64 {
        if self.max_variance_us == 0 {
            return 0;
        }
        // LCG: x_{n+1} = (a * x_n + c) mod m
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.seed % self.max_variance_us
    }
}

/// A fake HID device for deterministic testing.
///
/// Reports can be generated from a scripted sequence or by reading the
/// device's current axis/button/hat state. Counters track how many
/// reports have been sent and received.
#[derive(Debug)]
pub struct FakeHidDevice {
    /// Human-readable device name.
    pub name: String,
    /// USB vendor ID.
    pub vid: u16,
    /// USB product ID.
    pub pid: u16,
    /// Number of axes.
    pub axis_count: usize,
    /// Number of buttons.
    pub button_count: usize,
    /// Number of hat switches.
    pub hat_count: usize,

    current_axes: Vec<f64>,
    current_buttons: Vec<bool>,
    current_hats: Vec<Option<u16>>,

    sequence: Vec<HidReport>,
    position: usize,
    jitter: Option<JitterConfig>,

    reports_sent: AtomicU64,
    reports_received: AtomicU64,
}

impl FakeHidDevice {
    /// Create a new fake device with the given identity and capacities.
    pub fn new(
        name: impl Into<String>,
        vid: u16,
        pid: u16,
        axis_count: usize,
        button_count: usize,
        hat_count: usize,
    ) -> Self {
        Self {
            name: name.into(),
            vid,
            pid,
            axis_count,
            button_count,
            hat_count,
            current_axes: vec![0.0; axis_count],
            current_buttons: vec![false; button_count],
            current_hats: vec![None; hat_count],
            sequence: Vec::new(),
            position: 0,
            jitter: None,
            reports_sent: AtomicU64::new(0),
            reports_received: AtomicU64::new(0),
        }
    }

    // -- configuration -------------------------------------------------------

    /// Enable jitter injection with the given maximum variance.
    pub fn with_jitter(mut self, max_variance_us: u64) -> Self {
        self.jitter = Some(JitterConfig::new(max_variance_us));
        self
    }

    /// Load a scripted sequence of reports for replay.
    pub fn with_script(mut self, reports: Vec<HidReport>) -> Self {
        self.sequence = reports;
        self.position = 0;
        self
    }

    // -- live state -----------------------------------------------------------

    /// Set a single axis value.
    ///
    /// # Panics
    /// Panics if `index >= axis_count`.
    pub fn set_axis(&mut self, index: usize, value: f64) {
        self.current_axes[index] = value;
    }

    /// Set a single button state.
    ///
    /// # Panics
    /// Panics if `index >= button_count`.
    pub fn set_button(&mut self, index: usize, pressed: bool) {
        self.current_buttons[index] = pressed;
    }

    /// Set a single hat-switch position.
    ///
    /// # Panics
    /// Panics if `index >= hat_count`.
    pub fn set_hat(&mut self, index: usize, position: Option<u16>) {
        self.current_hats[index] = position;
    }

    // -- report generation ----------------------------------------------------

    /// Generate a report from the device's current live state.
    pub fn read_current(&self) -> HidReport {
        self.reports_sent.fetch_add(1, Ordering::Relaxed);
        HidReport {
            axes: self.current_axes.clone(),
            buttons: self.current_buttons.clone(),
            hats: self.current_hats.clone(),
        }
    }

    /// Consume the next report from the scripted sequence.
    ///
    /// Returns `None` when the sequence is exhausted.
    pub fn next_report(&mut self) -> Option<HidReport> {
        if self.position < self.sequence.len() {
            let report = self.sequence[self.position].clone();
            self.position += 1;
            self.reports_sent.fetch_add(1, Ordering::Relaxed);
            Some(report)
        } else {
            None
        }
    }

    /// Record that a report was received by the consumer side.
    pub fn ack_received(&self) {
        self.reports_received.fetch_add(1, Ordering::Relaxed);
    }

    // -- jitter ---------------------------------------------------------------

    /// Return the jitter (in µs) that would be applied to the next report.
    ///
    /// Returns `0` when jitter injection is disabled.
    pub fn next_jitter_us(&mut self) -> u64 {
        match self.jitter.as_mut() {
            Some(j) => j.next(),
            None => 0,
        }
    }

    // -- counters -------------------------------------------------------------

    /// Total number of reports generated (via `read_current` or `next_report`).
    pub fn reports_sent(&self) -> u64 {
        self.reports_sent.load(Ordering::Relaxed)
    }

    /// Total number of reports acknowledged as received.
    pub fn reports_received(&self) -> u64 {
        self.reports_received.load(Ordering::Relaxed)
    }

    /// Reset playback position and counters.
    pub fn reset(&mut self) {
        self.position = 0;
        self.reports_sent.store(0, Ordering::Relaxed);
        self.reports_received.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_device_defaults() {
        let dev = FakeHidDevice::new("Stick", 0x06a3, 0x0762, 4, 12, 1);
        assert_eq!(dev.name, "Stick");
        assert_eq!(dev.axis_count, 4);
        assert_eq!(dev.button_count, 12);
        assert_eq!(dev.hat_count, 1);
        assert_eq!(dev.reports_sent(), 0);
        assert_eq!(dev.reports_received(), 0);
    }

    #[test]
    fn set_axis_button_hat() {
        let mut dev = FakeHidDevice::new("Dev", 0, 0, 3, 2, 1);
        dev.set_axis(1, 0.75);
        dev.set_button(0, true);
        dev.set_hat(0, Some(90));

        let report = dev.read_current();
        assert!((report.axes[1] - 0.75).abs() < f64::EPSILON);
        assert!(report.buttons[0]);
        assert_eq!(report.hats[0], Some(90));
    }

    #[test]
    fn scripted_sequence_replay() {
        let reports = vec![
            HidReport {
                axes: vec![0.0, 0.0],
                buttons: vec![false],
                hats: vec![],
            },
            HidReport {
                axes: vec![1.0, -1.0],
                buttons: vec![true],
                hats: vec![],
            },
        ];

        let mut dev = FakeHidDevice::new("Dev", 0, 0, 2, 1, 0).with_script(reports);

        let r0 = dev.next_report().unwrap();
        assert!((r0.axes[0]).abs() < f64::EPSILON);

        let r1 = dev.next_report().unwrap();
        assert!((r1.axes[0] - 1.0).abs() < f64::EPSILON);
        assert!(r1.buttons[0]);

        assert!(dev.next_report().is_none());
        assert_eq!(dev.reports_sent(), 2);
    }

    #[test]
    fn counters_track_send_and_receive() {
        let mut dev = FakeHidDevice::new("Dev", 0, 0, 1, 0, 0).with_script(vec![HidReport {
            axes: vec![0.5],
            buttons: vec![],
            hats: vec![],
        }]);
        dev.next_report();
        dev.ack_received();
        assert_eq!(dev.reports_sent(), 1);
        assert_eq!(dev.reports_received(), 1);
    }

    #[test]
    fn jitter_injection_produces_values() {
        let mut dev = FakeHidDevice::new("Dev", 0, 0, 1, 0, 0).with_jitter(1000);
        let mut jitters = Vec::new();
        for _ in 0..10 {
            jitters.push(dev.next_jitter_us());
        }
        // All values should be < max_variance_us
        assert!(jitters.iter().all(|&j| j < 1000));
        // Not all the same (very unlikely with LCG)
        assert!(
            jitters.windows(2).any(|w| w[0] != w[1]),
            "jitter values should vary"
        );
    }

    #[test]
    fn no_jitter_returns_zero() {
        let mut dev = FakeHidDevice::new("Dev", 0, 0, 1, 0, 0);
        assert_eq!(dev.next_jitter_us(), 0);
    }

    #[test]
    fn reset_clears_position_and_counters() {
        let mut dev = FakeHidDevice::new("Dev", 0, 0, 1, 0, 0).with_script(vec![HidReport {
            axes: vec![0.5],
            buttons: vec![],
            hats: vec![],
        }]);
        dev.next_report();
        dev.ack_received();
        dev.reset();

        assert_eq!(dev.reports_sent(), 0);
        assert_eq!(dev.reports_received(), 0);
        // Sequence is replayable after reset
        assert!(dev.next_report().is_some());
    }

    #[test]
    fn read_current_increments_sent() {
        let dev = FakeHidDevice::new("Dev", 0, 0, 2, 0, 0);
        dev.read_current();
        dev.read_current();
        assert_eq!(dev.reports_sent(), 2);
    }

    #[test]
    #[should_panic]
    fn set_axis_out_of_bounds_panics() {
        let mut dev = FakeHidDevice::new("Dev", 0, 0, 2, 0, 0);
        dev.set_axis(5, 1.0);
    }
}
