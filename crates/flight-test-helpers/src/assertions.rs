// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared assertion helpers for integration and acceptance tests.

use flight_adapter_common::AdapterState;
use flight_bus::BusSnapshot;
use flight_device_common::DeviceHealth;

/// Assert an adapter state transition produced the expected state.
pub fn assert_adapter_state_transition(expected: AdapterState, actual: AdapterState) {
    assert_eq!(
        expected, actual,
        "unexpected adapter state transition: expected {expected:?}, got {actual:?}"
    );
}

/// Assert that a bus snapshot passes structural validation.
pub fn assert_snapshot_valid(snapshot: &BusSnapshot) {
    if let Err(err) = snapshot.validate() {
        panic!("bus snapshot validation failed: {err}");
    }
}

/// Assert that a device is still operational (healthy or degraded).
pub fn assert_device_connected(health: &DeviceHealth) {
    assert!(
        health.is_operational(),
        "device is not operational: {:?}",
        health
    );
}

/// Assert that two floating-point values are approximately equal within `tolerance`.
pub fn assert_approx_eq(a: f64, b: f64, tolerance: f64) {
    assert!(
        (a - b).abs() <= tolerance,
        "values not approximately equal: {a} vs {b} (tolerance {tolerance})"
    );
}

/// Assert that `value` lies within `[min, max]` inclusive.
pub fn assert_in_range(value: f64, min: f64, max: f64) {
    assert!(
        value >= min && value <= max,
        "value {value} is outside range [{min}, {max}]"
    );
}

/// Assert that a sequence of values is monotonically non-decreasing.
pub fn assert_monotonic(values: &[f64]) {
    for i in 1..values.len() {
        assert!(
            values[i] >= values[i - 1],
            "sequence not monotonic at index {i}: {} > {}",
            values[i - 1],
            values[i]
        );
    }
}

/// Assert that the rate of change between consecutive samples does not exceed `max_rate_per_sample`.
pub fn assert_bounded_rate(values: &[f64], max_rate_per_sample: f64) {
    for i in 1..values.len() {
        let rate = (values[i] - values[i - 1]).abs();
        assert!(
            rate <= max_rate_per_sample,
            "rate {rate} exceeds max {max_rate_per_sample} between index {} and {i}",
            i - 1
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approx_eq_within_tolerance() {
        assert_approx_eq(1.0, 1.0001, 0.001);
    }

    #[test]
    #[should_panic(expected = "values not approximately equal")]
    fn approx_eq_outside_tolerance() {
        assert_approx_eq(1.0, 2.0, 0.001);
    }

    #[test]
    fn in_range_inside() {
        assert_in_range(0.5, 0.0, 1.0);
        assert_in_range(0.0, 0.0, 1.0);
        assert_in_range(1.0, 0.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "outside range")]
    fn in_range_outside() {
        assert_in_range(1.5, 0.0, 1.0);
    }

    #[test]
    fn monotonic_valid() {
        assert_monotonic(&[1.0, 2.0, 2.0, 3.0]);
    }

    #[test]
    #[should_panic(expected = "not monotonic")]
    fn monotonic_invalid() {
        assert_monotonic(&[1.0, 3.0, 2.0]);
    }

    #[test]
    fn monotonic_empty_and_single() {
        assert_monotonic(&[]);
        assert_monotonic(&[42.0]);
    }

    #[test]
    fn bounded_rate_valid() {
        assert_bounded_rate(&[0.0, 0.1, 0.2, 0.3], 0.15);
    }

    #[test]
    #[should_panic(expected = "rate")]
    fn bounded_rate_exceeded() {
        assert_bounded_rate(&[0.0, 0.5, 0.51], 0.1);
    }
}
