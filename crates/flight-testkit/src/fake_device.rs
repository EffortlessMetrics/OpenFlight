// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Builder-pattern fake HID devices with configurable signal patterns and fault injection.
//!
//! Use [`FakeDeviceBuilder`] to construct devices with specific axis/button/hat
//! counts, then generate deterministic HID-like reports via signal patterns.

/// Signal pattern for generating deterministic axis data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SignalPattern {
    /// Constant value on every sample.
    Constant(f64),
    /// Linear ramp from `start` to `end` over `steps` samples, then repeats.
    Ramp {
        start: f64,
        end: f64,
        steps: u32,
    },
    /// Sine wave with given `amplitude`, `frequency_hz`, and `phase` (radians).
    Sine {
        amplitude: f64,
        frequency_hz: f64,
        phase: f64,
    },
    /// Pseudo-random values in `[min, max]` from a deterministic seed.
    RandomSeeded {
        seed: u64,
        min: f64,
        max: f64,
    },
    /// Alternates between `low` and `high` every `period` samples.
    Step {
        low: f64,
        high: f64,
        period: u32,
    },
}

/// Fault types that can be injected into device reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultType {
    /// Simulate a device disconnect (no more reports).
    Disconnect,
    /// Produce a report with out-of-range values.
    CorruptReport,
    /// Repeat the last report indefinitely (stale data).
    StaleData,
}

/// A single generated report frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FakeReport {
    pub axes: Vec<f64>,
    pub buttons: Vec<bool>,
    pub hats: Vec<i8>,
}

/// A fake HID device built via [`FakeDeviceBuilder`].
#[derive(Debug)]
pub struct FakeDevice {
    pub name: String,
    pub num_axes: usize,
    pub num_buttons: usize,
    pub num_hats: usize,
    patterns: Vec<SignalPattern>,
    fault: Option<FaultType>,
    sample_index: u64,
    last_report: Option<FakeReport>,
    disconnected: bool,
    rng_state: u64,
}

impl FakeDevice {
    /// Generate the next report based on configured patterns and faults.
    ///
    /// Returns `None` if the device is disconnected.
    pub fn next_report(&mut self) -> Option<FakeReport> {
        if self.disconnected {
            return None;
        }

        if let Some(FaultType::Disconnect) = self.fault {
            self.disconnected = true;
            return None;
        }

        if let Some(FaultType::StaleData) = self.fault
            && let Some(ref report) = self.last_report
        {
            return Some(report.clone());
        }

        let idx = self.sample_index;
        self.sample_index += 1;

        let mut axes = Vec::with_capacity(self.num_axes);
        for i in 0..self.num_axes {
            let pattern = self
                .patterns
                .get(i)
                .copied()
                .unwrap_or(SignalPattern::Constant(0.0));
            axes.push(self.generate_sample(pattern, idx));
        }

        if let Some(FaultType::CorruptReport) = self.fault {
            axes.fill(f64::NAN);
        }

        let buttons = vec![false; self.num_buttons];
        let hats = vec![0i8; self.num_hats];

        let report = FakeReport {
            axes,
            buttons,
            hats,
        };
        self.last_report = Some(report.clone());
        Some(report)
    }

    /// Inject a fault into subsequent reports.
    pub fn inject_fault(&mut self, fault: FaultType) {
        self.fault = Some(fault);
    }

    /// Clear any injected fault.
    pub fn clear_fault(&mut self) {
        self.fault = None;
        self.disconnected = false;
    }

    /// Reset the sample counter and clear faults.
    pub fn reset(&mut self) {
        self.sample_index = 0;
        self.fault = None;
        self.disconnected = false;
        self.last_report = None;
    }

    fn generate_sample(&mut self, pattern: SignalPattern, idx: u64) -> f64 {
        match pattern {
            SignalPattern::Constant(v) => v,
            SignalPattern::Ramp { start, end, steps } => {
                if steps == 0 {
                    return start;
                }
                let t = (idx % u64::from(steps)) as f64 / (f64::from(steps) - 1.0).max(1.0);
                start + (end - start) * t
            }
            SignalPattern::Sine {
                amplitude,
                frequency_hz,
                phase,
            } => {
                // Assume 250 Hz sample rate.
                let t = idx as f64 / 250.0;
                amplitude * (2.0 * std::f64::consts::PI * frequency_hz * t + phase).sin()
            }
            SignalPattern::RandomSeeded { seed, min, max } => {
                // Simple xorshift64 for determinism.
                let mut state = seed.wrapping_add(idx).wrapping_mul(6_364_136_223_846_793_005);
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                self.rng_state = state;
                let norm = (state as f64) / (u64::MAX as f64);
                min + (max - min) * norm
            }
            SignalPattern::Step {
                low,
                high,
                period,
            } => {
                if period == 0 {
                    return low;
                }
                let phase = (idx / u64::from(period)) % 2;
                if phase == 0 { low } else { high }
            }
        }
    }
}

/// Fluent builder for [`FakeDevice`].
#[derive(Debug, Clone)]
pub struct FakeDeviceBuilder {
    name: String,
    num_axes: usize,
    num_buttons: usize,
    num_hats: usize,
    patterns: Vec<SignalPattern>,
}

impl FakeDeviceBuilder {
    /// Start building a device with the given product name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            num_axes: 0,
            num_buttons: 0,
            num_hats: 0,
            patterns: Vec::new(),
        }
    }

    /// Set the number of axes.
    #[must_use]
    pub fn axes(mut self, count: usize) -> Self {
        self.num_axes = count;
        self
    }

    /// Set the number of buttons.
    #[must_use]
    pub fn buttons(mut self, count: usize) -> Self {
        self.num_buttons = count;
        self
    }

    /// Set the number of hat switches.
    #[must_use]
    pub fn hats(mut self, count: usize) -> Self {
        self.num_hats = count;
        self
    }

    /// Assign a signal pattern to a specific axis index.
    #[must_use]
    pub fn pattern(mut self, axis_index: usize, pattern: SignalPattern) -> Self {
        // Extend patterns vector if needed.
        if self.patterns.len() <= axis_index {
            self.patterns
                .resize(axis_index + 1, SignalPattern::Constant(0.0));
        }
        self.patterns[axis_index] = pattern;
        self
    }

    /// Build the fake device.
    #[must_use]
    pub fn build(self) -> FakeDevice {
        FakeDevice {
            name: self.name,
            num_axes: self.num_axes,
            num_buttons: self.num_buttons,
            num_hats: self.num_hats,
            patterns: self.patterns,
            fault: None,
            sample_index: 0,
            last_report: None,
            disconnected: false,
            rng_state: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_creates_device_with_correct_counts() {
        let dev = FakeDeviceBuilder::new("X56 Stick")
            .axes(5)
            .buttons(24)
            .hats(2)
            .build();
        assert_eq!(dev.name, "X56 Stick");
        assert_eq!(dev.num_axes, 5);
        assert_eq!(dev.num_buttons, 24);
        assert_eq!(dev.num_hats, 2);
    }

    #[test]
    fn constant_pattern_produces_same_value() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(0, SignalPattern::Constant(0.75))
            .build();
        for _ in 0..10 {
            let report = dev.next_report().unwrap();
            assert!((report.axes[0] - 0.75).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn ramp_pattern_sweeps_values() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(
                0,
                SignalPattern::Ramp {
                    start: 0.0,
                    end: 1.0,
                    steps: 5,
                },
            )
            .build();
        let values: Vec<f64> = (0..5).map(|_| dev.next_report().unwrap().axes[0]).collect();
        assert!((values[0] - 0.0).abs() < f64::EPSILON);
        assert!((values[4] - 1.0).abs() < f64::EPSILON);
        // Monotonically increasing
        for w in values.windows(2) {
            assert!(w[1] >= w[0]);
        }
    }

    #[test]
    fn sine_pattern_oscillates() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(
                0,
                SignalPattern::Sine {
                    amplitude: 1.0,
                    frequency_hz: 1.0,
                    phase: 0.0,
                },
            )
            .build();
        let values: Vec<f64> = (0..250).map(|_| dev.next_report().unwrap().axes[0]).collect();
        // First sample at t=0 should be ~0
        assert!(values[0].abs() < 0.1);
        // Should have positive and negative values
        assert!(values.iter().any(|&v| v > 0.5));
        assert!(values.iter().any(|&v| v < -0.5));
    }

    #[test]
    fn step_pattern_alternates() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(
                0,
                SignalPattern::Step {
                    low: -1.0,
                    high: 1.0,
                    period: 2,
                },
            )
            .build();
        let v0 = dev.next_report().unwrap().axes[0];
        let v1 = dev.next_report().unwrap().axes[0];
        let v2 = dev.next_report().unwrap().axes[0];
        let v3 = dev.next_report().unwrap().axes[0];
        assert!((v0 - (-1.0)).abs() < f64::EPSILON);
        assert!((v1 - (-1.0)).abs() < f64::EPSILON);
        assert!((v2 - 1.0).abs() < f64::EPSILON);
        assert!((v3 - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn random_seeded_is_deterministic() {
        let pattern = SignalPattern::RandomSeeded {
            seed: 42,
            min: -1.0,
            max: 1.0,
        };
        let mut dev1 = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(0, pattern)
            .build();
        let mut dev2 = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(0, pattern)
            .build();
        for _ in 0..20 {
            let a = dev1.next_report().unwrap().axes[0];
            let b = dev2.next_report().unwrap().axes[0];
            assert!((a - b).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn random_seeded_within_bounds() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(
                0,
                SignalPattern::RandomSeeded {
                    seed: 123,
                    min: 0.0,
                    max: 1.0,
                },
            )
            .build();
        for _ in 0..100 {
            let v = dev.next_report().unwrap().axes[0];
            assert!((0.0..=1.0).contains(&v), "value {v} out of [0, 1]");
        }
    }

    #[test]
    fn disconnect_fault_stops_reports() {
        let mut dev = FakeDeviceBuilder::new("Test").axes(1).build();
        assert!(dev.next_report().is_some());
        dev.inject_fault(FaultType::Disconnect);
        assert!(dev.next_report().is_none());
        assert!(dev.next_report().is_none());
    }

    #[test]
    fn corrupt_report_produces_nan() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(2)
            .pattern(0, SignalPattern::Constant(0.5))
            .build();
        dev.inject_fault(FaultType::CorruptReport);
        let report = dev.next_report().unwrap();
        assert!(report.axes[0].is_nan());
        assert!(report.axes[1].is_nan());
    }

    #[test]
    fn stale_data_repeats_last_report() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(
                0,
                SignalPattern::Ramp {
                    start: 0.0,
                    end: 1.0,
                    steps: 10,
                },
            )
            .build();
        let first = dev.next_report().unwrap();
        dev.inject_fault(FaultType::StaleData);
        let stale1 = dev.next_report().unwrap();
        let stale2 = dev.next_report().unwrap();
        assert_eq!(first, stale1);
        assert_eq!(first, stale2);
    }

    #[test]
    fn clear_fault_resumes_normal() {
        let mut dev = FakeDeviceBuilder::new("Test").axes(1).build();
        dev.inject_fault(FaultType::Disconnect);
        assert!(dev.next_report().is_none());
        dev.clear_fault();
        assert!(dev.next_report().is_some());
    }

    #[test]
    fn reset_clears_everything() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(1)
            .pattern(
                0,
                SignalPattern::Ramp {
                    start: 0.0,
                    end: 1.0,
                    steps: 5,
                },
            )
            .build();
        let _ = dev.next_report();
        let _ = dev.next_report();
        dev.inject_fault(FaultType::Disconnect);
        dev.reset();
        let report = dev.next_report().unwrap();
        // Should be back at sample 0
        assert!((report.axes[0] - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn report_has_correct_button_and_hat_counts() {
        let mut dev = FakeDeviceBuilder::new("Full")
            .axes(3)
            .buttons(12)
            .hats(2)
            .build();
        let report = dev.next_report().unwrap();
        assert_eq!(report.axes.len(), 3);
        assert_eq!(report.buttons.len(), 12);
        assert_eq!(report.hats.len(), 2);
    }

    #[test]
    fn unspecified_axes_default_to_zero() {
        let mut dev = FakeDeviceBuilder::new("Test")
            .axes(3)
            .pattern(0, SignalPattern::Constant(1.0))
            .build();
        let report = dev.next_report().unwrap();
        assert!((report.axes[0] - 1.0).abs() < f64::EPSILON);
        assert!((report.axes[1] - 0.0).abs() < f64::EPSILON);
        assert!((report.axes[2] - 0.0).abs() < f64::EPSILON);
    }
}
