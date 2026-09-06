// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Cross-platform contract tests for `JitterMeasurement` warmup semantics.
//!
//! These tests use deterministic synthetic timestamps. They verify interval
//! selection only; they do not claim anything about host or hardware jitter.

#[cfg(unix)]
use flight_scheduler::unix::JitterMeasurement;
#[cfg(windows)]
use flight_scheduler::windows::JitterMeasurement;

const TARGET_HZ: u32 = 250;
const WARMUP_SECONDS: u32 = 1;
const TARGET_PERIOD_NS: u64 = 4_000_000;
const EARLY_PERIOD_NS: u64 = 3_500_000;
const LATE_PERIOD_NS: u64 = 4_500_000;
const BASE_NS: u64 = 1_000_000_000;
const WARMUP_INTERVALS_AFTER_BASELINE: usize = 249;
const MEASURED_INTERVALS: usize = 50;

fn record_interval(jitter: &mut JitterMeasurement, now_ns: &mut u64, interval_ns: u64) {
    *now_ns = now_ns
        .checked_add(interval_ns)
        .expect("synthetic timestamp must remain in range");
    jitter.record_tick(*now_ns);
}

#[test]
fn warmup_interval_jitter_is_excluded_from_statistics() {
    let mut jitter = JitterMeasurement::new(TARGET_HZ, WARMUP_SECONDS);
    let mut now_ns = BASE_NS;

    // Tick 1 establishes the timestamp baseline. At 250 Hz with a one-second
    // warmup, ticks 1..=250 are warmup and the interval ending at tick 251 is
    // the first measured interval.
    jitter.record_tick(now_ns);

    // Make the warmup visibly bad: every warmup interval is ±500 us from the
    // 4 ms target. A test that merely applies a constant timestamp offset would
    // not work because JitterMeasurement measures interval error.
    for index in 0..WARMUP_INTERVALS_AFTER_BASELINE {
        let interval_ns = if index % 2 == 0 {
            LATE_PERIOD_NS
        } else {
            EARLY_PERIOD_NS
        };
        record_interval(&mut jitter, &mut now_ns, interval_ns);
    }

    // All measured intervals are exact. Warmup leakage would make p99 500 us.
    for _ in 0..MEASURED_INTERVALS {
        record_interval(&mut jitter, &mut now_ns, TARGET_PERIOD_NS);
    }

    let stats = jitter.compute_stats();

    assert_eq!(stats.samples, MEASURED_INTERVALS);
    assert_eq!(stats.p50_ns, 0);
    assert_eq!(stats.p95_ns, 0);
    assert_eq!(stats.p99_ns, 0);
    assert_eq!(jitter.tick_count(), 300);
}

#[test]
fn post_warmup_interval_jitter_is_included_in_statistics() {
    let mut jitter = JitterMeasurement::new(TARGET_HZ, WARMUP_SECONDS);
    let mut now_ns = BASE_NS;

    jitter.record_tick(now_ns);

    // Exact warmup intervals establish the inverse case.
    for _ in 0..WARMUP_INTERVALS_AFTER_BASELINE {
        record_interval(&mut jitter, &mut now_ns, TARGET_PERIOD_NS);
    }

    // Every measured interval is ±500 us from target, so absolute jitter is
    // exactly 500_000 ns regardless of which side of the target it lands on.
    for index in 0..MEASURED_INTERVALS {
        let interval_ns = if index % 2 == 0 {
            LATE_PERIOD_NS
        } else {
            EARLY_PERIOD_NS
        };
        record_interval(&mut jitter, &mut now_ns, interval_ns);
    }

    let stats = jitter.compute_stats();

    assert_eq!(stats.samples, MEASURED_INTERVALS);
    assert_eq!(stats.p50_ns, 500_000);
    assert_eq!(stats.p95_ns, 500_000);
    assert_eq!(stats.p99_ns, 500_000);
    assert_eq!(jitter.tick_count(), 300);
}
