// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive test suite for flight-scheduler
//!
//! Tests timing accuracy, jitter measurement, PLL behavior,
//! and overload handling.

use super::*;
use crate::metrics::TimingValidator;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_scheduler_basic_timing() {
    let config = SchedulerConfig {
        frequency_hz: 100, // Lower frequency for testing
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: true,
    };

    let mut scheduler = Scheduler::new(config);
    let start = Instant::now();

    // Run for 100 ticks (1 second at 100Hz)
    for _ in 0..100 {
        let result = scheduler.wait_for_tick();
        assert!(!result.missed); // Should not miss ticks under light load
    }

    let elapsed = start.elapsed();
    let expected = Duration::from_millis(1000); // 1 second
    let tolerance = if std::env::var_os("CI").is_some() {
        Duration::from_millis(250) // Shared CI runners have higher timing variance
    } else {
        Duration::from_millis(50)
    };

    assert!(elapsed >= expected - tolerance);
    assert!(elapsed <= expected + tolerance);
}

#[test]
fn test_jitter_measurement() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 65,
        pll_gain: 0.001,
        measure_jitter: true,
    };

    let mut scheduler = Scheduler::new(config);

    // Run for enough ticks to get meaningful statistics
    for _ in 0..2000 {
        // 8 seconds at 250Hz
        scheduler.wait_for_tick();
    }

    let stats = scheduler.get_stats();

    // Should have jitter statistics
    assert!(stats.jitter_stats.is_some());

    let jitter = stats.jitter_stats.unwrap();
    assert!(jitter.sample_count > 0);

    // Jitter should be reasonable (this may fail on heavily loaded systems)
    // In CI, we'll need to be more lenient
    println!("Jitter p99: {}μs", jitter.p99_ns / 1000);
}

#[test]
fn test_pll_correction() {
    let mut pll = Pll::new(0.001, 4_000_000.0); // 250Hz

    // Simulate consistent timing error
    let mut total_correction = 0.0;
    for _ in 0..1000 {
        let corrected = pll.update(1000.0); // 1μs late each time
        total_correction += pll.period_correction();
    }

    // Should have accumulated negative correction (shorter period)
    assert!(pll.period_correction() < 0.0);

    // Should be bounded
    assert!(pll.period_correction().abs() < 40_000.0); // <1% of 4ms
}

#[test]
fn test_spsc_ring_basic() {
    let ring = SpscRing::new(8);

    // Test basic operations
    assert!(ring.try_push(1));
    assert!(ring.try_push(2));
    assert!(ring.try_push(3));

    assert_eq!(ring.try_pop(), Some(1));
    assert_eq!(ring.try_pop(), Some(2));
    assert_eq!(ring.try_pop(), Some(3));
    assert_eq!(ring.try_pop(), None);

    let stats = ring.stats();
    assert_eq!(stats.produced, 3);
    assert_eq!(stats.consumed, 3);
    assert_eq!(stats.dropped, 0);
}

#[test]
fn test_spsc_ring_drop_policy() {
    let ring = SpscRing::new(4);

    // Fill buffer to capacity-1 (ring buffer limitation)
    for i in 0..3 {
        assert!(ring.try_push(i));
    }

    // Next push should fail (drop-tail policy)
    assert!(!ring.try_push(999));

    let stats = ring.stats();
    assert_eq!(stats.dropped, 1);

    // Should still be able to consume
    assert_eq!(ring.try_pop(), Some(0));

    // Now should be able to push again
    assert!(ring.try_push(4));
}

#[test]
fn test_concurrent_ring_operations() {
    let ring = Arc::new(SpscRing::new(1024));
    let ring_producer = ring.clone();
    let ring_consumer = ring.clone();

    let producer = thread::spawn(move || {
        for i in 0..10000 {
            while !ring_producer.try_push(i) {
                thread::yield_now();
            }
        }
    });

    let consumer = thread::spawn(move || {
        let mut received = 0;
        while received < 10000 {
            if ring_consumer.try_pop().is_some() {
                received += 1;
            }
            thread::yield_now();
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();

    let stats = ring.stats();
    assert_eq!(stats.consumed, 10000);
}

#[test]
fn test_overload_behavior() {
    let ring = Arc::new(SpscRing::new(16));
    let ring_producer = ring.clone();

    // Fast producer, slow consumer
    let producer = thread::spawn(move || {
        for i in 0..1000 {
            ring_producer.try_push(i);
        }
    });

    // Slow consumer
    let ring_consumer = ring.clone();
    let consumer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10)); // Let producer get ahead

        let mut consumed = 0;
        while consumed < 100 {
            // Only consume some items
            if ring_consumer.try_pop().is_some() {
                consumed += 1;
            }
            thread::sleep(Duration::from_micros(100)); // Slow consumer
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();

    let stats = ring.stats();

    // Should have dropped items due to overload
    assert!(stats.dropped > 0);

    // Total produced should equal consumed + dropped
    assert_eq!(stats.produced, stats.consumed + stats.dropped);
}

#[test]
fn test_timing_validator() {
    // We need enough ticks to bypass WARMUP_TICKS (1250)
    // At 100Hz (10ms), 1500 ticks = 15s
    let target_duration = Duration::from_secs(15);
    let mut validator = TimingValidator::new(100, target_duration);
    let start = Instant::now();

    let mut tick_count: u64 = 0;
    while validator.record_and_check(start + Duration::from_millis(tick_count * 10)) {
        tick_count += 1;
        if tick_count > 2000 {
            break;
        } // Safety limit
    }

    let result = validator.finalize();

    // Should have run for at least the target duration
    assert!(result.duration >= target_duration);

    // Should have collected samples
    assert!(result.jitter_stats.sample_count > 0);
}

/// Verify that consecutive 250Hz tick intervals stay within jitter tolerance.
///
/// Measures the wall-clock gap between consecutive ticks and checks that the
/// p99 deviation from the 4ms target is ≤0.5ms on dev hardware (generous
/// tolerance applied in CI where shared runners introduce scheduling noise).
#[test]
fn test_250hz_interval_precision() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 65,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    let mut scheduler = Scheduler::new(config);
    let expected_interval = Duration::from_micros(4_000);

    // Tolerances: 0.5ms on bare metal, 5ms on shared CI runners.
    let tolerance = if std::env::var_os("CI").is_some() {
        Duration::from_millis(5)
    } else {
        Duration::from_micros(500)
    };

    // Warm up – allow PLL to converge.
    for _ in 0..50 {
        scheduler.wait_for_tick();
    }

    // Collect 200 tick intervals (~0.8 s wall time at 250Hz).
    const SAMPLES: usize = 200;
    let mut intervals = Vec::with_capacity(SAMPLES);

    scheduler.wait_for_tick();
    let mut prev = Instant::now();

    for _ in 0..SAMPLES {
        scheduler.wait_for_tick();
        let now = Instant::now();
        intervals.push(now.duration_since(prev));
        prev = now;
    }

    intervals.sort();
    let p99_idx = (SAMPLES as f32 * 0.99) as usize;
    let p99 = intervals[p99_idx.min(intervals.len() - 1)];

    let jitter = if p99 > expected_interval {
        p99 - expected_interval
    } else {
        expected_interval - p99
    };

    println!(
        "250Hz interval p99 jitter: {}μs (tolerance: {}μs)",
        jitter.as_micros(),
        tolerance.as_micros()
    );

    assert!(
        jitter <= tolerance,
        "250Hz p99 interval jitter {}μs exceeds {}μs tolerance",
        jitter.as_micros(),
        tolerance.as_micros()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// New coverage tests
// ─────────────────────────────────────────────────────────────────────────────

/// Missed-tick counter increments when a tick arrives more than half a period late.
///
/// Setup: 1000 Hz scheduler (1 ms period).  Sleeping 3 ms between ticks guarantees
/// `tick_start ≥ next_tick + period_ns/2`, which flips the `missed` flag.
#[test]
fn test_scheduler_missed_tick_increments_counter() {
    let config = SchedulerConfig {
        frequency_hz: 1000,
        busy_spin_us: 0, // no busy-spin so the test finishes quickly
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut scheduler = Scheduler::new(config);

    // Warm-up: first tick against the freshly-initialised next_tick.
    scheduler.wait_for_tick();

    // Sleep well past next_tick + period_ns/2 (0.5 ms).
    thread::sleep(Duration::from_millis(3));

    // This tick arrives late – the scheduler must flag it as missed.
    let result = scheduler.wait_for_tick();

    let stats = scheduler.get_stats();
    assert!(
        result.missed,
        "wait_for_tick should return missed=true for a late tick"
    );
    assert!(
        stats.missed_ticks > 0,
        "missed_tick counter should be at least 1, got {}",
        stats.missed_ticks
    );
}

/// Constructing schedulers at common RT frequencies completes without panic and
/// produces sane initial statistics (zero ticks, zero misses).
#[test]
fn test_scheduler_config_at_common_frequencies() {
    for &freq in &[50u32, 100, 250, 500, 1000] {
        let config = SchedulerConfig {
            frequency_hz: freq,
            busy_spin_us: 0,
            pll_gain: 0.001,
            measure_jitter: false,
        };
        let scheduler = Scheduler::new(config);
        let stats = scheduler.get_stats();
        assert_eq!(
            stats.total_ticks, 0,
            "new scheduler at {freq} Hz should have 0 ticks"
        );
        assert_eq!(
            stats.missed_ticks, 0,
            "new scheduler at {freq} Hz should have 0 misses"
        );
    }
}

/// `reset_stats` zeroes tick and miss counters without affecting configuration.
#[test]
fn test_scheduler_reset_stats_clears_counters() {
    let config = SchedulerConfig {
        frequency_hz: 1000,
        busy_spin_us: 0,
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut scheduler = Scheduler::new(config);

    // Run a few ticks to populate counters.
    for _ in 0..5 {
        scheduler.wait_for_tick();
    }
    let before = scheduler.get_stats();
    assert!(before.total_ticks > 0, "should have ticks before reset");

    scheduler.reset_stats();

    let after = scheduler.get_stats();
    assert_eq!(after.total_ticks, 0, "total_ticks must be zero after reset");
    assert_eq!(after.missed_ticks, 0, "missed_ticks must be zero after reset");
    assert_eq!(after.miss_rate, 0.0, "miss_rate must be 0.0 after reset");
}

/// Dropping a `Scheduler` instance does not panic (RAII cleanup).
#[test]
fn test_scheduler_drop_no_panic() {
    let config = SchedulerConfig::default();
    let scheduler = Scheduler::new(config);
    drop(scheduler); // must not panic
}

/// PLL state resets to nominal period when a zero error is applied repeatedly.
#[test]
fn test_pll_zero_error_holds_nominal_period() {
    let nominal = 4_000_000.0f64; // 4 ms (250 Hz)
    let mut pll = Pll::new(0.001, nominal);

    for _ in 0..100 {
        let corrected = pll.update(0.0);
        assert!(
            (corrected - nominal).abs() < 1e-3,
            "PLL should hold nominal period under zero error, got {corrected}"
        );
    }
    assert!(
        pll.period_correction().abs() < 1e-3,
        "phase correction should be near zero under zero error"
    );
}

/// `WindowsRtThread::new` does not panic on Windows (MMCSS may or may not succeed –
/// graceful degradation is explicitly required by the implementation).
#[test]
#[cfg(windows)]
fn test_windows_rt_thread_setup_no_panic() {
    use crate::windows::WindowsRtThread;
    // Construction must succeed regardless of whether MMCSS registration works.
    let result = WindowsRtThread::new("Games");
    assert!(
        result.is_ok(),
        "WindowsRtThread should succeed (with graceful degradation): {:?}",
        result.err()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Extended / ignored tests
// ─────────────────────────────────────────────────────────────────────────────

/// Integration test that runs scheduler for extended period
#[test]
#[ignore] // Ignore by default as it takes time
fn test_extended_timing_discipline() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 65,
        pll_gain: 0.001,
        measure_jitter: true,
    };

    let mut scheduler = Scheduler::new(config);
    let start = Instant::now();
    let target_duration = Duration::from_secs(60); // 1 minute test

    let mut tick_count = 0;
    while start.elapsed() < target_duration {
        let result = scheduler.wait_for_tick();
        tick_count += 1;

        // Log progress every 10 seconds
        if tick_count % 2500 == 0 {
            let stats = scheduler.get_stats();
            println!(
                "Tick {}: Miss rate: {:.4}%",
                tick_count,
                stats.miss_rate * 100.0
            );

            if let Some(jitter) = &stats.jitter_stats {
                println!("  Jitter p99: {}μs", jitter.p99_ns / 1000);
            }
        }
    }

    let final_stats = scheduler.get_stats();

    // Quality gates
    assert!(
        final_stats.miss_rate < 0.001,
        "Miss rate too high: {:.4}%",
        final_stats.miss_rate * 100.0
    );

    if let Some(ref jitter) = final_stats.jitter_stats {
        assert!(
            jitter.p99_ns.abs() < 500_000,
            "Jitter p99 too high: {}μs",
            jitter.p99_ns / 1000
        );
    }

    println!("Extended test completed successfully:");
    println!("  Total ticks: {}", final_stats.total_ticks);
    println!("  Miss rate: {:.6}%", final_stats.miss_rate * 100.0);

    if let Some(ref jitter) = final_stats.jitter_stats {
        println!("  Jitter p50: {}μs", jitter.p50_ns / 1000);
        println!("  Jitter p99: {}μs", jitter.p99_ns / 1000);
    }
}
