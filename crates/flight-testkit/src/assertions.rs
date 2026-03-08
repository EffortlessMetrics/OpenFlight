// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Domain-specific assertion helpers for flight simulation integration tests.

use std::time::Duration;

/// Assert that an axis value lies within `[min, max]`.
///
/// # Panics
///
/// Panics with a descriptive message if the value is out of range.
pub fn assert_axis_in_range(value: f64, min: f64, max: f64) {
    assert!(
        value >= min && value <= max,
        "axis value {value} outside range [{min}, {max}]"
    );
}

/// Assert that a sequence of values is monotonically non-decreasing.
///
/// # Panics
///
/// Panics at the first violation.
pub fn assert_monotonic(values: &[f64]) {
    for i in 1..values.len() {
        assert!(
            values[i] >= values[i - 1],
            "not monotonic at index {i}: {} > {}",
            values[i - 1],
            values[i]
        );
    }
}

/// Assert that no value in a snapshot (slice) is NaN.
///
/// # Panics
///
/// Panics if any value is NaN.
pub fn assert_no_nan(values: &[f64]) {
    for (i, v) in values.iter().enumerate() {
        assert!(!v.is_nan(), "NaN found at index {i}");
    }
}

/// Assert that no value is infinite.
///
/// # Panics
///
/// Panics if any value is infinite.
pub fn assert_no_inf(values: &[f64]) {
    for (i, v) in values.iter().enumerate() {
        assert!(!v.is_infinite(), "Inf found at index {i}: {v}");
    }
}

/// Assert that a measured latency is under a limit.
///
/// # Panics
///
/// Panics if `actual >= limit`.
pub fn assert_latency_under(actual: Duration, limit: Duration) {
    assert!(
        actual < limit,
        "latency {:?} exceeds limit {:?}",
        actual,
        limit
    );
}

/// Assert that the p99 of a set of duration samples is within `limit`.
///
/// Requires at least one sample.
///
/// # Panics
///
/// Panics if p99 exceeds `limit` or if `samples` is empty.
pub fn assert_jitter_p99(samples: &[Duration], limit: Duration) {
    assert!(!samples.is_empty(), "no samples provided for jitter p99");
    let mut sorted: Vec<Duration> = samples.to_vec();
    sorted.sort();
    let idx = ((sorted.len() as f64) * 0.99).ceil() as usize - 1;
    let p99 = sorted[idx.min(sorted.len() - 1)];
    assert!(
        p99 <= limit,
        "jitter p99 {:?} exceeds limit {:?}",
        p99,
        limit
    );
}

/// Assert that two floating-point values are approximately equal.
///
/// # Panics
///
/// Panics if the absolute difference exceeds `tolerance`.
pub fn assert_approx_eq(a: f64, b: f64, tolerance: f64) {
    assert!(
        (a - b).abs() <= tolerance,
        "values not approximately equal: {a} vs {b} (tolerance {tolerance})"
    );
}

/// Assert that the rate of change between consecutive values does not exceed a limit.
///
/// # Panics
///
/// Panics at the first violation.
pub fn assert_bounded_rate(values: &[f64], max_rate: f64) {
    for i in 1..values.len() {
        let rate = (values[i] - values[i - 1]).abs();
        assert!(
            rate <= max_rate,
            "rate {rate} exceeds max {max_rate} between index {} and {i}",
            i - 1
        );
    }
}

/// Assert that the average frequency derived from inter-sample intervals
/// is within `tolerance_pct` percent of `target_hz`.
///
/// `timestamps_us` contains timestamps in microseconds.
pub fn assert_frequency_within(timestamps_us: &[u64], target_hz: f64, tolerance_pct: f64) {
    if timestamps_us.len() < 2 {
        return;
    }
    let total_interval: u64 = timestamps_us
        .windows(2)
        .map(|w| w[1].saturating_sub(w[0]))
        .sum();
    let avg_interval_us = total_interval as f64 / (timestamps_us.len() - 1) as f64;
    let measured_hz = 1_000_000.0 / avg_interval_us;
    let deviation_pct = ((measured_hz - target_hz) / target_hz).abs() * 100.0;
    assert!(
        deviation_pct <= tolerance_pct,
        "frequency {measured_hz:.1}Hz deviates {deviation_pct:.1}% from target {target_hz}Hz \
         (tolerance {tolerance_pct}%)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // -- assert_axis_in_range --

    #[test]
    fn axis_in_range_pass() {
        assert_axis_in_range(0.5, -1.0, 1.0);
        assert_axis_in_range(-1.0, -1.0, 1.0);
        assert_axis_in_range(1.0, -1.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "outside range")]
    fn axis_in_range_fail() {
        assert_axis_in_range(1.5, -1.0, 1.0);
    }

    // -- assert_monotonic --

    #[test]
    fn monotonic_pass() {
        assert_monotonic(&[1.0, 2.0, 2.0, 3.0]);
    }

    #[test]
    fn monotonic_empty_and_single() {
        assert_monotonic(&[]);
        assert_monotonic(&[42.0]);
    }

    #[test]
    #[should_panic(expected = "not monotonic")]
    fn monotonic_fail() {
        assert_monotonic(&[1.0, 3.0, 2.0]);
    }

    // -- assert_no_nan --

    #[test]
    fn no_nan_clean() {
        assert_no_nan(&[0.0, 1.0, -1.0]);
    }

    #[test]
    #[should_panic(expected = "NaN")]
    fn no_nan_detected() {
        assert_no_nan(&[0.0, f64::NAN]);
    }

    // -- assert_no_inf --

    #[test]
    fn no_inf_clean() {
        assert_no_inf(&[0.0, 1e10, -1e10]);
    }

    #[test]
    #[should_panic(expected = "Inf")]
    fn no_inf_detected() {
        assert_no_inf(&[f64::INFINITY]);
    }

    // -- assert_latency_under --

    #[test]
    fn latency_pass() {
        assert_latency_under(Duration::from_micros(200), Duration::from_micros(300));
    }

    #[test]
    #[should_panic(expected = "latency")]
    fn latency_fail() {
        assert_latency_under(Duration::from_micros(500), Duration::from_micros(300));
    }

    // -- assert_jitter_p99 --

    #[test]
    fn jitter_p99_pass() {
        // Jitter samples: all within 0–400µs, so p99 < 500µs.
        let samples: Vec<Duration> = (0..100)
            .map(|i| Duration::from_micros(i % 50))
            .collect();
        assert_jitter_p99(&samples, Duration::from_micros(500));
    }

    #[test]
    #[should_panic(expected = "jitter p99")]
    fn jitter_p99_fail() {
        // p99 is 10ms which exceeds 500µs limit.
        let mut samples: Vec<Duration> = vec![Duration::from_micros(100); 50];
        // Add many large values so they dominate p99.
        samples.extend(vec![Duration::from_millis(10); 50]);
        assert_jitter_p99(&samples, Duration::from_micros(500));
    }

    #[test]
    fn jitter_p99_single_sample() {
        assert_jitter_p99(&[Duration::from_micros(100)], Duration::from_micros(500));
    }

    // -- assert_approx_eq --

    #[test]
    fn approx_eq_pass() {
        assert_approx_eq(1.0, 1.0001, 0.001);
    }

    #[test]
    #[should_panic(expected = "not approximately equal")]
    fn approx_eq_fail() {
        assert_approx_eq(1.0, 2.0, 0.001);
    }

    // -- assert_bounded_rate --

    #[test]
    fn bounded_rate_pass() {
        assert_bounded_rate(&[0.0, 0.1, 0.2, 0.3], 0.15);
    }

    #[test]
    #[should_panic(expected = "rate")]
    fn bounded_rate_fail() {
        assert_bounded_rate(&[0.0, 0.5, 0.51], 0.1);
    }

    // -- assert_frequency_within --

    #[test]
    fn frequency_250hz() {
        let samples: Vec<u64> = (0..100).map(|i| i * 4000).collect();
        assert_frequency_within(&samples, 250.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "frequency")]
    fn frequency_off_target() {
        let samples: Vec<u64> = (0..100).map(|i| i * 8000).collect();
        assert_frequency_within(&samples, 250.0, 1.0);
    }

    #[test]
    fn frequency_single_sample_ok() {
        assert_frequency_within(&[0], 250.0, 1.0);
    }
}
