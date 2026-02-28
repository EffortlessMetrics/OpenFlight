// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Domain-specific assertion helpers for flight simulation testing.
//!
//! These complement the basic assertions in [`super::assertions`] with
//! higher-level checks commonly needed in axis, timing, and telemetry tests.

/// Assert that an axis value lies within `[min, max]`.
pub fn assert_axis_in_range(value: f64, min: f64, max: f64, name: &str) {
    assert!(
        value >= min && value <= max,
        "axis '{name}' value {value} outside range [{min}, {max}]"
    );
}

/// Assert that a sequence of values is monotonically non-decreasing.
pub fn assert_monotonic(values: &[f64], name: &str) {
    for i in 1..values.len() {
        assert!(
            values[i] >= values[i - 1],
            "'{name}' not monotonic at index {i}: {} > {}",
            values[i - 1],
            values[i]
        );
    }
}

/// Assert that input/output pairs exhibit a symmetric deadzone — values within
/// `tolerance` of zero on the input side should map to approximately zero output.
pub fn assert_symmetric_deadzone(values: &[(f64, f64)], tolerance: f64) {
    for &(input, output) in values {
        if input.abs() <= tolerance {
            assert!(
                output.abs() <= tolerance,
                "deadzone violation: input {input} (within tolerance {tolerance}) \
                 produced output {output}"
            );
        }
    }
}

/// Assert that a measured latency is below a threshold.
pub fn assert_latency_under(duration_us: u64, max_us: u64, label: &str) {
    assert!(
        duration_us <= max_us,
        "'{label}' latency {duration_us}µs exceeds maximum {max_us}µs"
    );
}

/// Assert that peak-to-peak jitter across samples is below a threshold.
pub fn assert_jitter_under(samples_us: &[u64], max_jitter_us: u64) {
    if samples_us.len() < 2 {
        return;
    }
    let min = samples_us.iter().copied().min().unwrap();
    let max = samples_us.iter().copied().max().unwrap();
    let jitter = max - min;
    assert!(
        jitter <= max_jitter_us,
        "jitter {jitter}µs exceeds maximum {max_jitter_us}µs (range {min}–{max})"
    );
}

/// Assert that no value is NaN.
pub fn assert_no_nan(values: &[f64], label: &str) {
    for (i, v) in values.iter().enumerate() {
        assert!(!v.is_nan(), "'{label}' contains NaN at index {i}");
    }
}

/// Assert that no value is infinite.
pub fn assert_no_inf(values: &[f64], label: &str) {
    for (i, v) in values.iter().enumerate() {
        assert!(!v.is_infinite(), "'{label}' contains Inf at index {i}: {v}");
    }
}

/// Assert that the average frequency derived from inter-sample intervals is
/// within `tolerance_pct` percent of `target_hz`.
///
/// `samples` contains timestamps in microseconds.
pub fn assert_frequency_within(samples: &[u64], target_hz: f64, tolerance_pct: f64) {
    if samples.len() < 2 {
        return;
    }
    let total_interval: u64 = samples.windows(2).map(|w| w[1].saturating_sub(w[0])).sum();
    let avg_interval_us = total_interval as f64 / (samples.len() - 1) as f64;
    let measured_hz = 1_000_000.0 / avg_interval_us;
    let deviation_pct = ((measured_hz - target_hz) / target_hz).abs() * 100.0;
    assert!(
        deviation_pct <= tolerance_pct,
        "frequency {measured_hz:.1}Hz deviates {deviation_pct:.1}% from target \
         {target_hz}Hz (tolerance {tolerance_pct}%)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- assert_axis_in_range --

    #[test]
    fn axis_in_range_centre() {
        assert_axis_in_range(0.0, -1.0, 1.0, "pitch");
    }

    #[test]
    fn axis_in_range_boundaries() {
        assert_axis_in_range(-1.0, -1.0, 1.0, "roll");
        assert_axis_in_range(1.0, -1.0, 1.0, "roll");
    }

    #[test]
    #[should_panic(expected = "outside range")]
    fn axis_in_range_out_of_bounds() {
        assert_axis_in_range(1.5, -1.0, 1.0, "yaw");
    }

    // -- assert_monotonic --

    #[test]
    fn monotonic_ascending() {
        assert_monotonic(&[0.0, 0.25, 0.5, 1.0], "curve");
    }

    #[test]
    #[should_panic(expected = "not monotonic")]
    fn monotonic_violation() {
        assert_monotonic(&[0.0, 1.0, 0.5], "curve");
    }

    // -- assert_symmetric_deadzone --

    #[test]
    fn symmetric_deadzone_pass() {
        let pairs = vec![(0.01, 0.0), (-0.02, 0.0), (0.5, 0.45)];
        assert_symmetric_deadzone(&pairs, 0.05);
    }

    #[test]
    #[should_panic(expected = "deadzone violation")]
    fn symmetric_deadzone_fail() {
        let pairs = vec![(0.01, 0.3)];
        assert_symmetric_deadzone(&pairs, 0.05);
    }

    // -- assert_latency_under --

    #[test]
    fn latency_under_threshold() {
        assert_latency_under(200, 300, "hid_write");
    }

    #[test]
    #[should_panic(expected = "latency")]
    fn latency_over_threshold() {
        assert_latency_under(500, 300, "hid_write");
    }

    // -- assert_jitter_under --

    #[test]
    fn jitter_within_budget() {
        assert_jitter_under(&[4000, 4050, 3980, 4020], 100);
    }

    #[test]
    #[should_panic(expected = "jitter")]
    fn jitter_exceeded() {
        assert_jitter_under(&[4000, 5000], 500);
    }

    #[test]
    fn jitter_single_sample_ok() {
        assert_jitter_under(&[1000], 0);
    }

    // -- assert_no_nan / assert_no_inf --

    #[test]
    fn no_nan_clean() {
        assert_no_nan(&[0.0, 1.0, -1.0], "axes");
    }

    #[test]
    #[should_panic(expected = "NaN")]
    fn no_nan_detected() {
        assert_no_nan(&[0.0, f64::NAN], "axes");
    }

    #[test]
    fn no_inf_clean() {
        assert_no_inf(&[0.0, 1e10, -1e10], "telemetry");
    }

    #[test]
    #[should_panic(expected = "Inf")]
    fn no_inf_detected() {
        assert_no_inf(&[f64::INFINITY], "telemetry");
    }

    // -- assert_frequency_within --

    #[test]
    fn frequency_250hz() {
        // 250 Hz ⇒ 4000µs intervals
        let samples: Vec<u64> = (0..100).map(|i| i * 4000).collect();
        assert_frequency_within(&samples, 250.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "frequency")]
    fn frequency_off_target() {
        // ~125 Hz when we expect 250 Hz
        let samples: Vec<u64> = (0..100).map(|i| i * 8000).collect();
        assert_frequency_within(&samples, 250.0, 1.0);
    }

    #[test]
    fn frequency_single_sample_ok() {
        assert_frequency_within(&[0], 250.0, 1.0);
    }
}
