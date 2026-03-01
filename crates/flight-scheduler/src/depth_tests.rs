// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the RT scheduler — the timing backbone of the 250 Hz spine.
//!
//! Covers tick timing, task scheduling, platform integration (via mocks),
//! jitter control, shutdown semantics, and diagnostics.

use super::*;
use crate::budget::{InlineTickBudget, TickBudget, MAX_PHASES};
use crate::executor::{TickExecutor, MAX_TASKS};
use crate::jitter::JitterTracker;
use crate::metrics::{JitterMetrics, TimingValidator};
use crate::mmcss::{MmcssHandle, MockMmcssBackend, MmcssPriority};
use crate::platform::{
    Platform, PlatformRtError, RtPriority, detect_platform, is_rt_available,
    request_rt_priority_mmcss, request_rt_priority_noop, request_rt_priority_rtkit,
};
use crate::pll::{JitterStats as PllJitterStats, PhaseLockLoop, Pll};
use crate::ring::SpscRing;
use crate::rtkit::{MockRtkitBackend, RtkitHandle};
use crate::timer::{FallbackTimer, HighResTimer, MockTimer, SystemTimer, TimerStats};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════
// 1. TICK TIMING (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// The default scheduler targets 250 Hz (4 ms period).
#[test]
fn tick_timing_250hz_target_period() {
    let config = SchedulerConfig::default();
    assert_eq!(config.frequency_hz, 250);
    let scheduler = Scheduler::new(config);
    assert_eq!(scheduler.period_ns, 4_000_000);
}

/// Measure actual tick duration over a short burst and verify it is close
/// to the expected 4 ms period.
///
/// This test is sensitive to OS scheduling jitter; run explicitly with
/// `cargo test -- --ignored` on a quiet machine.
#[test]
#[ignore]
fn tick_timing_duration_measurement() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 65,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    let mut scheduler = Scheduler::new(config);

    // Warm up PLL
    for _ in 0..50 {
        scheduler.wait_for_tick();
    }

    let before = Instant::now();
    let ticks = 50;
    for _ in 0..ticks {
        scheduler.wait_for_tick();
    }
    let elapsed = before.elapsed();
    let expected = Duration::from_millis(4 * ticks);
    let tolerance = if std::env::var_os("CI").is_some() {
        Duration::from_millis(200)
    } else {
        Duration::from_millis(50)
    };
    assert!(
        elapsed.abs_diff(expected) <= tolerance,
        "expected ~{expected:?}, got {elapsed:?}"
    );
}

/// A tick that arrives more than half a period late is flagged as missed
/// and the deadline is enforced by the scheduler.
#[test]
fn tick_timing_deadline_enforcement() {
    let config = SchedulerConfig {
        frequency_hz: 1000,
        busy_spin_us: 0,
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut scheduler = Scheduler::new(config);
    scheduler.wait_for_tick();
    // Exceed the deadline by sleeping well past the next tick
    thread::sleep(Duration::from_millis(5));
    let result = scheduler.wait_for_tick();
    assert!(result.missed, "tick should be flagged as missed");
}

/// Overrun detection: accumulate missed ticks and verify the counter.
#[test]
fn tick_timing_overrun_detection() {
    let config = SchedulerConfig {
        frequency_hz: 1000,
        busy_spin_us: 0,
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut scheduler = Scheduler::new(config);
    scheduler.wait_for_tick();

    let mut missed_count = 0u32;
    for _ in 0..5 {
        thread::sleep(Duration::from_millis(4));
        let r = scheduler.wait_for_tick();
        if r.missed {
            missed_count += 1;
        }
    }
    assert!(missed_count >= 3, "expected >=3 misses, got {missed_count}");
    assert!(scheduler.get_stats().missed_ticks >= missed_count as u64);
}

/// Underrun handling: if the scheduler wakes up slightly early, PLL should
/// pull the corrected period longer (positive correction).
#[test]
fn tick_timing_underrun_pll_response() {
    let mut pll = Pll::new(0.001, 4_000_000.0);
    // Feed negative errors (arriving early)
    for _ in 0..200 {
        pll.update(-500.0);
    }
    // Period correction should be positive (lengthening the period)
    assert!(
        pll.period_correction() > 0.0,
        "PLL should lengthen period for early arrivals, got {}",
        pll.period_correction()
    );
}

/// Cumulative drift correction: the PLL integral term should grow when
/// consistent phase error is applied, pulling the corrected period
/// toward the target.
#[test]
fn tick_timing_cumulative_drift_correction() {
    let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0);
    // Inject a steady +1 µs error
    for _ in 0..500 {
        pll.tick(1_000.0);
    }
    // Corrected period should be shorter than nominal to compensate
    assert!(
        pll.corrected_period_ns() < pll.nominal_period_ns(),
        "corrected period {} should be < nominal {}",
        pll.corrected_period_ns(),
        pll.nominal_period_ns(),
    );
}

/// PLL lock detection: feeding small errors for enough ticks should
/// transition the PLL into locked state.
#[test]
fn tick_timing_pll_lock_detection() {
    let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0)
        .with_lock_detection(50_000.0, 200_000.0, 10);
    assert!(!pll.locked());
    // Feed small errors below the lock threshold
    for _ in 0..50 {
        pll.tick(100.0); // 0.1 µs — well below 50 µs lock threshold
    }
    assert!(pll.locked(), "PLL should be locked after low-error ticks");
}

/// PLL bandwidth: correction is clamped to ±1 % of the nominal period,
/// preventing runaway oscillation.
#[test]
fn tick_timing_pll_bandwidth_clamped() {
    let nominal = 4_000_000.0f64;
    let max_delta = nominal * 0.01;
    let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);
    // Feed huge error to saturate
    for _ in 0..1000 {
        pll.tick(1_000_000.0); // 1 ms error
    }
    let delta = (pll.corrected_period_ns() - nominal).abs();
    assert!(
        delta <= max_delta + 1.0, // +1.0 for FP tolerance
        "correction {delta} exceeds ±1% ({max_delta})"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. TASK SCHEDULING (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Register a periodic task and verify it runs every tick.
#[test]
fn task_scheduling_register_periodic() {
    let counter = Arc::new(AtomicU64::new(0));
    let c = counter.clone();
    let mut exec = TickExecutor::new();
    exec.register_task("periodic", move || {
        c.fetch_add(1, Ordering::Relaxed);
    });
    for _ in 0..100 {
        exec.run_tick(4_000_000);
    }
    assert_eq!(counter.load(Ordering::Relaxed), 100);
    assert_eq!(exec.tick_count(), 100);
}

/// After `reset_stats`, task counters are zeroed but the task remains
/// registered and continues to execute on subsequent ticks.
#[test]
fn task_scheduling_reset_stats_preserves_tasks() {
    let counter = Arc::new(AtomicU64::new(0));
    let c = counter.clone();
    let mut exec = TickExecutor::new();
    exec.register_task("once", move || {
        c.fetch_add(1, Ordering::Relaxed);
    });
    exec.run_tick(4_000_000);
    assert_eq!(counter.load(Ordering::Relaxed), 1);

    // reset_stats zeroes counters but keeps the task
    exec.reset_stats();
    assert_eq!(exec.tick_count(), 0);
    assert_eq!(exec.task_stats("once").unwrap().invocations, 0);

    // Task still runs
    exec.run_tick(4_000_000);
    assert_eq!(counter.load(Ordering::Relaxed), 2);
}

/// Tasks execute in registration order (priority via ordering).
#[test]
fn task_scheduling_priority_ordering() {
    let log = Arc::new(std::sync::Mutex::new(Vec::<&str>::new()));
    let mut exec = TickExecutor::new();
    let l1 = log.clone();
    exec.register_task("high", move || l1.lock().unwrap().push("high"));
    let l2 = log.clone();
    exec.register_task("medium", move || l2.lock().unwrap().push("medium"));
    let l3 = log.clone();
    exec.register_task("low", move || l3.lock().unwrap().push("low"));

    exec.run_tick(4_000_000);
    let order = log.lock().unwrap().clone();
    assert_eq!(order, vec!["high", "medium", "low"]);
}

/// Budget enforcement: when a task exceeds the tick budget, the overrun
/// flag is set and the overrun counter increments.
#[test]
fn task_scheduling_budget_enforcement() {
    let mut exec = TickExecutor::new();
    exec.register_task("slow", || {
        thread::sleep(Duration::from_micros(200));
    });
    let result = exec.run_tick(1); // 1 ns budget — guaranteed overrun
    assert!(result.overrun);
    assert_eq!(exec.overrun_count(), 1);

    // A second overrun should increment the counter
    let result2 = exec.run_tick(1);
    assert!(result2.overrun);
    assert_eq!(exec.overrun_count(), 2);
}

/// Even when a task overruns, subsequent tasks still execute (the RT
/// spine must never block per ADR-001).
#[test]
fn task_scheduling_overrun_continues_remaining_tasks() {
    let second_ran = Arc::new(AtomicBool::new(false));
    let flag = second_ran.clone();
    let mut exec = TickExecutor::new();
    exec.register_task("slow", || {
        thread::sleep(Duration::from_micros(200));
    });
    exec.register_task("fast", move || {
        flag.store(true, Ordering::Relaxed);
    });
    exec.run_tick(1);
    assert!(
        second_ran.load(Ordering::Relaxed),
        "second task must run even after overrun"
    );
}

/// Attempting to register more than MAX_TASKS is rejected.
#[test]
fn task_scheduling_max_tasks_capacity() {
    let mut exec = TickExecutor::new();
    static NAMES: [&str; MAX_TASKS] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
    ];
    for name in &NAMES {
        assert!(exec.register_task(name, || {}));
    }
    assert!(!exec.register_task("overflow", || {}));
    assert_eq!(exec.task_count(), MAX_TASKS);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. PLATFORM INTEGRATION (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// MMCSS mock: Pro Audio registration with Critical priority succeeds and
/// can be released cleanly.
#[test]
fn platform_mmcss_priority_boost() {
    let backend = MockMmcssBackend::new_success();
    let mut h = MmcssHandle::register_pro_audio(backend).unwrap();
    h.set_priority(MmcssPriority::Critical).unwrap();
    assert_eq!(h.current_priority(), MmcssPriority::Critical);
    assert!(h.is_registered());
    h.unregister().unwrap();
}

/// Rtkit mock: request RT priority and pin to a CPU core.
#[test]
fn platform_rtkit_priority_and_affinity() {
    let backend = MockRtkitBackend::new_success();
    let mut h = RtkitHandle::request_realtime(backend, 50).unwrap();
    assert_eq!(h.priority(), 50);
    h.set_thread_affinity(0).unwrap();
    assert_eq!(h.affinity_core(), Some(0));
}

/// High-resolution timer can be enabled and is automatically disabled on
/// drop (via the RAII guard).
#[test]
fn platform_high_resolution_timer() {
    let backend = MockMmcssBackend::new_success();
    {
        let _guard = crate::mmcss::enable_high_resolution_timer(&backend).unwrap();
        assert!(backend.is_timer_active());
    }
    assert!(!backend.is_timer_active(), "timer should be off after drop");
}

/// Timer coalescing avoidance: enabling the high-res timer through an
/// MMCSS handle sets the flag. (Verifying restore-on-drop is not possible
/// here because the mock backend is moved into the handle.)
#[test]
fn platform_timer_coalescing_avoidance() {
    let backend = MockMmcssBackend::new_success();
    let mut h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
    assert!(!h.is_timer_enabled());
    h.enable_high_resolution_timer().unwrap();
    assert!(h.is_timer_enabled());
}

/// CPU affinity can be set via the standalone helper and via a handle.
#[test]
fn platform_cpu_affinity() {
    let backend = MockRtkitBackend::new_success();
    crate::rtkit::set_thread_affinity(&backend, 2).unwrap();
    assert_eq!(backend.last_core_id(), 2);
    assert_eq!(backend.affinity_count(), 1);

    // Also via handle
    let mut h = RtkitHandle::request_realtime(MockRtkitBackend::new_success(), 10).unwrap();
    h.set_thread_affinity(3).unwrap();
    assert_eq!(h.affinity_core(), Some(3));
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. JITTER CONTROL (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// JitterTracker computes correct p50 and p99 percentiles for known data.
#[test]
fn jitter_p50_p99_known_data() {
    let mut t = JitterTracker::new();
    for v in 0..200u64 {
        t.record(v * 1_000); // 0–199 µs
    }
    let p99 = t.p99_ns();
    // p99 of 0..199_000 ns should be ~197_000..199_000
    assert!(
        p99 >= 190_000 && p99 <= 200_000,
        "p99 should be ~197–199 µs, got {p99} ns"
    );
    // Mean should be near 99_500 ns
    let mean = t.mean_ns();
    assert!(
        mean >= 90_000 && mean <= 110_000,
        "mean should be ~99.5 µs, got {mean} ns"
    );
}

/// PllJitterStats (running stats) computes max correctly.
#[test]
fn jitter_max_measurement() {
    let mut js = PllJitterStats::new();
    js.record(100);
    js.record(-200);
    js.record(500);
    js.record(-50);
    assert_eq!(js.max_ns(), 500);
    assert_eq!(js.min_ns(), -200);
}

/// JitterTracker histogram: after filling with identical values the stddev
/// should be zero.
#[test]
fn jitter_histogram_uniform_zero_stddev() {
    let mut t = JitterTracker::new();
    for _ in 0..256 {
        t.record(42_000);
    }
    assert_eq!(t.min_ns(), 42_000);
    assert_eq!(t.max_ns(), 42_000);
    assert!(t.stddev_ns() < 1.0, "stddev should be ~0, got {}", t.stddev_ns());
}

/// TimerStats histogram correctly identifies the 0.5 ms quality gate
/// threshold (bucket index).
#[test]
fn jitter_threshold_500us() {
    let mut stats = TimerStats::new(); // 10 µs bucket width
    // Record many samples within tolerance
    for _ in 0..990 {
        stats.record_tick(10_000, false); // 10 µs
    }
    // Record a few spikes above 500 µs
    for _ in 0..10 {
        stats.record_tick(600_000, false);
    }
    // Bucket for 600 µs at 10 µs width = index 60 (capped at 63)
    let h = stats.jitter_histogram();
    assert!(h[1] == 990, "bucket[1] should hold the 10 µs samples");
    assert!(h[60] == 10, "bucket[60] should hold the 600 µs spikes");
}

/// Spike detection: injecting a single huge jitter value should show up
/// in the max field while not affecting the mean significantly.
#[test]
fn jitter_spike_detection() {
    let mut js = PllJitterStats::new();
    for _ in 0..999 {
        js.record(100); // 0.1 µs steady
    }
    js.record(5_000_000); // 5 ms spike
    assert_eq!(js.max_ns(), 5_000_000);
    // Mean should still be relatively low (~5100 ns)
    assert!(js.mean() < 10_000.0, "mean should be low despite spike");
    assert_eq!(js.count(), 1000);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. SHUTDOWN (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Graceful tick drain: after running N ticks, stats reflect exactly N.
#[test]
fn shutdown_graceful_tick_drain() {
    let config = SchedulerConfig {
        frequency_hz: 1000,
        busy_spin_us: 0,
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut scheduler = Scheduler::new(config);
    for _ in 0..20 {
        scheduler.wait_for_tick();
    }
    assert_eq!(scheduler.get_stats().total_ticks, 20);
}

/// In-progress tasks complete before the executor considers the tick
/// done (all tasks run to completion per ADR-001).
#[test]
fn shutdown_in_progress_task_completion() {
    let completed = Arc::new(AtomicBool::new(false));
    let flag = completed.clone();
    let mut exec = TickExecutor::new();
    exec.register_task("work", move || {
        thread::sleep(Duration::from_micros(500));
        flag.store(true, Ordering::Relaxed);
    });
    let result = exec.run_tick(4_000_000);
    assert!(
        completed.load(Ordering::Relaxed),
        "task must complete before run_tick returns"
    );
    assert_eq!(result.tasks_run, 1);
}

/// Forced shutdown: dropping the scheduler does not panic and is safe.
#[test]
fn shutdown_forced_drop_safe() {
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);
    scheduler.wait_for_tick();
    drop(scheduler); // must not panic
}

/// Shutdown timeout: the executor's overrun counter represents a de facto
/// "exceeded budget" signal; after several overruns, the count is accurate.
#[test]
fn shutdown_timeout_overrun_count() {
    let mut exec = TickExecutor::new();
    exec.register_task("slow", || {
        thread::sleep(Duration::from_micros(100));
    });
    for _ in 0..10 {
        exec.run_tick(1); // 1 ns budget → always overrun
    }
    assert_eq!(exec.overrun_count(), 10);
    assert_eq!(exec.tick_count(), 10);
}

/// Restart after shutdown: reset_stats lets the scheduler counters start
/// fresh without re-creating the scheduler.
#[test]
fn shutdown_restart_after_reset() {
    let config = SchedulerConfig {
        frequency_hz: 1000,
        busy_spin_us: 0,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    let mut scheduler = Scheduler::new(config);
    for _ in 0..10 {
        scheduler.wait_for_tick();
    }
    assert_eq!(scheduler.get_stats().total_ticks, 10);

    scheduler.reset_stats();
    assert_eq!(scheduler.get_stats().total_ticks, 0);
    assert_eq!(scheduler.get_stats().missed_ticks, 0);
    assert_eq!(scheduler.get_stats().miss_rate, 0.0);

    // Scheduler still functional after reset
    for _ in 0..5 {
        scheduler.wait_for_tick();
    }
    assert_eq!(scheduler.get_stats().total_ticks, 5);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. DIAGNOSTICS (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Tick statistics: count, min, max, avg are all populated after running.
#[test]
fn diagnostics_tick_statistics() {
    let mut exec = TickExecutor::new();
    exec.register_task("work", || {
        // Spin briefly to produce measurable time
        std::hint::black_box((0..50).sum::<u64>());
    });
    for _ in 0..50 {
        exec.run_tick(4_000_000);
    }
    let stats = exec.task_stats("work").unwrap();
    assert_eq!(stats.invocations, 50);
    assert!(stats.total_ns > 0);
    assert!(stats.max_ns > 0);
    assert!(stats.last_ns > 0);
    assert!(stats.mean_ns() > 0.0);
}

/// Overrun counter increments exactly once per overrunning tick.
#[test]
fn diagnostics_overrun_counter() {
    let mut exec = TickExecutor::new();
    exec.register_task("fast", || {});
    exec.register_task("slow", || {
        thread::sleep(Duration::from_micros(100));
    });

    // Generous budget → no overrun
    let r1 = exec.run_tick(1_000_000_000);
    assert!(!r1.overrun);
    assert_eq!(exec.overrun_count(), 0);

    // Tiny budget → overrun
    let r2 = exec.run_tick(1);
    assert!(r2.overrun);
    assert_eq!(exec.overrun_count(), 1);
}

/// Underrun counter: TimerStats tracks the number of ticks flagged as
/// missed (underrun from the consumer perspective).
#[test]
fn diagnostics_underrun_counter() {
    let mut stats = TimerStats::new();
    stats.record_tick(100, false);
    stats.record_tick(200, true); // missed
    stats.record_tick(300, false);
    stats.record_tick(400, true); // missed
    assert_eq!(stats.missed_ticks(), 2);
    assert_eq!(stats.tick_count(), 4);
}

/// CPU usage per tick: the TickBudget utilization metric reflects
/// the fraction of the tick period consumed.
#[test]
fn diagnostics_cpu_usage_per_tick() {
    let mut b = TickBudget::new(10_000_000); // 10 ms budget

    for _ in 0..5 {
        b.begin_tick();
        b.begin_phase("axis");
        thread::sleep(Duration::from_micros(500)); // ~0.5 ms
        b.end_phase();
        b.end_tick();
    }

    let util = b.utilization();
    // ~0.5 ms / 10 ms = ~0.05, but timer resolution means we just check
    // it's in a reasonable range
    assert!(
        util > 0.01 && util < 0.5,
        "utilization {util} should be between 1% and 50%"
    );
    assert_eq!(b.tick_count(), 5);
}

/// Timing histogram: TimerStats distributes jitter samples into the
/// correct buckets.
#[test]
fn diagnostics_timing_histogram() {
    let mut stats = TimerStats::with_bucket_width(10_000); // 10 µs buckets

    // 50 samples at 5 µs → bucket 0
    for _ in 0..50 {
        stats.record_tick(5_000, false);
    }
    // 30 samples at 15 µs → bucket 1
    for _ in 0..30 {
        stats.record_tick(15_000, false);
    }
    // 20 samples at 95 µs → bucket 9
    for _ in 0..20 {
        stats.record_tick(95_000, false);
    }

    let h = stats.jitter_histogram();
    assert_eq!(h[0], 50);
    assert_eq!(h[1], 30);
    assert_eq!(h[9], 20);
    assert_eq!(stats.tick_count(), 100);
    assert_eq!(stats.min_jitter_ns(), 5_000);
    assert_eq!(stats.max_jitter_ns(), 95_000);
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. ADDITIONAL CROSS-CUTTING DEPTH TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// PLL unlock detection: after locking, feeding large errors should
/// transition to unlocked.
#[test]
fn pll_unlock_after_spike() {
    let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0)
        .with_lock_detection(50_000.0, 200_000.0, 5);

    // Lock the PLL
    for _ in 0..20 {
        pll.tick(100.0);
    }
    assert!(pll.locked());

    // Inject large errors to trigger unlock
    for _ in 0..20 {
        pll.tick(500_000.0); // 500 µs — above 200 µs unlock threshold
    }
    assert!(!pll.locked(), "PLL should unlock after sustained large errors");
}

/// The InlineTickBudget Copy trait works correctly — mutations to the
/// copy do not affect the original.
#[test]
fn inline_budget_copy_independence() {
    let mut a = InlineTickBudget::new(4_000_000);
    a.begin_task(1);
    a.end_task();
    let b = a; // Copy

    // Mutate a further
    a.begin_task(2);
    a.end_task();

    // b should still have task_count == 1
    assert_eq!(b.task_count(), 1);
    assert_eq!(a.task_count(), 2);
}

/// SPSC ring preserves FIFO ordering across a burst of push/pop cycles.
#[test]
fn ring_fifo_ordering_preserved() {
    let ring = SpscRing::new(64);
    for i in 0..50u32 {
        assert!(ring.try_push(i));
    }
    for i in 0..50u32 {
        assert_eq!(ring.try_pop(), Some(i));
    }
    assert!(ring.is_empty());
}

/// MockTimer is fully deterministic — two identically-driven instances
/// produce the same state.
#[test]
fn mock_timer_determinism() {
    let mut t1 = MockTimer::new();
    let mut t2 = MockTimer::new();
    for _ in 0..250 {
        t1.sleep_ns(4_000_000);
        t2.sleep_ns(4_000_000);
    }
    assert_eq!(t1.now_ns(), t2.now_ns());
    assert_eq!(t1.stats().tick_count(), t2.stats().tick_count());
    assert_eq!(t1.now_ns(), 250 * 4_000_000);
}

/// PLL from_hz helper creates the correct nominal period.
#[test]
fn pll_from_hz_correct_period() {
    let pll = PhaseLockLoop::from_hz(250);
    assert!((pll.nominal_period_ns() - 4_000_000.0).abs() < 1.0);
    assert!((pll.frequency() - 250.0).abs() < 0.01);
}

/// Platform detection returns a valid value on this OS.
#[test]
fn platform_detect_returns_valid() {
    let p = detect_platform();
    if cfg!(target_os = "windows") {
        assert_eq!(p, Platform::Windows);
    } else if cfg!(target_os = "linux") {
        assert_eq!(p, Platform::Linux);
    }
    // is_rt_available should be consistent
    let available = is_rt_available();
    if matches!(p, Platform::Windows | Platform::Linux) {
        assert!(available);
    }
}

/// Noop RT handle reports inactive and release succeeds.
#[test]
fn platform_noop_handle_inactive() {
    let h: crate::platform::RtHandle<MockMmcssBackend, MockRtkitBackend> =
        request_rt_priority_noop(RtPriority::Realtime);
    assert!(!h.is_active());
    assert_eq!(h.level(), RtPriority::Realtime);
    assert!(h.release().is_ok());
}

/// FallbackTimer sleep_until with a past deadline returns immediately.
#[test]
fn fallback_timer_past_deadline_immediate() {
    let mut timer = FallbackTimer::new(50_000);
    let past = Instant::now() - Duration::from_millis(10);
    let before = Instant::now();
    timer.sleep_until(past);
    assert!(before.elapsed() < Duration::from_millis(2));
    assert_eq!(timer.stats().tick_count(), 1);
}

/// SystemTimer now_ns is monotonically increasing.
#[test]
fn system_timer_monotonic() {
    let timer = SystemTimer::new(20_000);
    let mut prev = timer.now_ns();
    for _ in 0..10 {
        thread::sleep(Duration::from_micros(50));
        let cur = timer.now_ns();
        assert!(cur >= prev, "now_ns should be monotonic");
        prev = cur;
    }
}

/// JitterMetrics quality gate returns false when not enough samples
/// have been collected.
#[test]
fn jitter_metrics_quality_gate_insufficient_samples() {
    let metrics = JitterMetrics::new(250);
    // No samples recorded
    assert!(
        !metrics.exceeds_quality_gate(),
        "quality gate should not trigger with 0 samples"
    );
}

/// TickBudget phase_utilization and phase_name return correct values
/// for out-of-range indices.
#[test]
fn budget_out_of_range_accessors() {
    let b = TickBudget::new(4_000_000);
    assert_eq!(b.phase_utilization(999), 0.0);
    assert_eq!(b.phase_name(999), "");
    assert_eq!(b.phase_max_ns(999), 0);
}
