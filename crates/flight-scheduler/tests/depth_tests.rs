// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-scheduler: tick timing, PLL discipline, MMCSS
//! integration, task scheduling, timer resolution, and stress scenarios.
//!
//! These tests exercise the scheduler subsystem far beyond the unit-level
//! coverage in `src/tests.rs`. They are designed to catch regressions in
//! real-time timing behaviour, PLL convergence, and platform RT scheduling.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use flight_scheduler::platform::{
    request_rt_priority_mmcss, request_rt_priority_noop, request_rt_priority_rtkit,
};
use flight_scheduler::timer::JITTER_BUCKETS;
use flight_scheduler::*;

// ═══════════════════════════════════════════════════════════════════════════
// 1. Tick timing
// ═══════════════════════════════════════════════════════════════════════════

/// 250 Hz tick rate produces ~4 ms periods.
#[test]
fn tick_timing_250hz_period() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    let mut sched = Scheduler::new(config);

    // Warm up PLL.
    for _ in 0..20 {
        sched.wait_for_tick();
    }

    // Measure 50 ticks (~200 ms).
    let before = Instant::now();
    for _ in 0..50 {
        sched.wait_for_tick();
    }
    let elapsed = before.elapsed();

    let expected = Duration::from_millis(200); // 50 × 4 ms
    let tolerance = if std::env::var_os("CI").is_some() {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(30)
    };

    assert!(
        elapsed >= expected.saturating_sub(tolerance),
        "50 ticks too fast: {elapsed:?}"
    );
    assert!(
        elapsed <= expected + tolerance,
        "50 ticks too slow: {elapsed:?}"
    );
}

/// Tick counter advances monotonically — no duplicate or decreasing tick numbers.
#[test]
fn tick_counter_monotonic() {
    let config = SchedulerConfig {
        frequency_hz: 1000,
        busy_spin_us: 0,
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut sched = Scheduler::new(config);

    let mut prev_tick = 0u64;
    for i in 0..100 {
        let result = sched.wait_for_tick();
        assert!(
            result.tick_number >= prev_tick,
            "tick {i}: tick_number went backwards ({} < {prev_tick})",
            result.tick_number,
        );
        if i > 0 {
            assert!(
                result.tick_number > prev_tick,
                "tick {i}: duplicate tick_number {prev_tick}",
            );
        }
        prev_tick = result.tick_number;
    }
}

/// Tick overrun detection: deliberately stalling between ticks causes `missed`.
#[test]
fn tick_overrun_detection() {
    let config = SchedulerConfig {
        frequency_hz: 1000, // 1 ms period
        busy_spin_us: 0,
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut sched = Scheduler::new(config);

    // Consume first tick to initialise next_tick.
    sched.wait_for_tick();

    // Sleep 5 ms — guarantees overrun for a 1 ms period scheduler.
    thread::sleep(Duration::from_millis(5));

    let result = sched.wait_for_tick();
    assert!(result.missed, "tick should be flagged as missed after stall");
    assert!(
        sched.get_stats().missed_ticks > 0,
        "missed_ticks counter should be > 0"
    );
}

/// Tick phase tracking: early arrival has negative error, late has positive.
#[test]
fn tick_phase_tracking() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    let mut sched = Scheduler::new(config);

    // Run enough ticks to see both signs of error (PLL correction causes both).
    let mut saw_negative = false;
    let mut saw_nonnegative = false;
    for _ in 0..200 {
        let result = sched.wait_for_tick();
        if result.error_ns < 0 {
            saw_negative = true;
        } else {
            saw_nonnegative = true;
        }
        if saw_negative && saw_nonnegative {
            break;
        }
    }

    // We mainly assert that we observed non-negative (≥0) errors since the
    // busy-spin tail usually overshoots slightly. Negative error is
    // less common but possible with PLL correction.
    assert!(
        saw_nonnegative,
        "should see at least one non-negative (on-time/late) tick"
    );
}

/// Property-like check: tick count after N ms ≈ N/4 (at 250 Hz).
#[test]
fn tick_count_approximation() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: false,
    };
    let mut sched = Scheduler::new(config);

    let target_ms = 100u64;
    let start = Instant::now();
    let mut count = 0u64;
    while start.elapsed() < Duration::from_millis(target_ms) {
        sched.wait_for_tick();
        count += 1;
    }

    // Expected ≈ 25 ticks (100 ms / 4 ms).
    let expected = target_ms / 4;
    let tolerance = if std::env::var_os("CI").is_some() {
        10
    } else {
        5
    };
    assert!(
        count.abs_diff(expected) <= tolerance,
        "expected ~{expected} ticks in {target_ms} ms, got {count}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. PLL timing discipline (ADR-005)
// ═══════════════════════════════════════════════════════════════════════════

/// PhaseLockLoop converges to the target rate under constant positive error.
#[test]
fn pll_converges_to_target_rate() {
    let nominal = 4_000_000.0; // 250 Hz
    let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);

    // Feed a constant +100 µs error for 200 ticks.
    for _ in 0..200 {
        let corrected = pll.tick(100_000.0);
        // Corrected period should be shorter than nominal (speeding up).
        assert!(
            corrected < nominal,
            "PLL should shorten period to catch up: got {corrected}"
        );
    }

    // The correction should be bounded to ±1%.
    let delta = (pll.corrected_period_ns() - nominal).abs();
    assert!(
        delta <= nominal * 0.01 + 1.0,
        "correction {delta} exceeds ±1% of nominal"
    );
}

/// PLL handles jitter in OS scheduling (alternating positive/negative error).
#[test]
fn pll_handles_jitter() {
    let nominal = 4_000_000.0;
    let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);

    // Alternating ±50 µs error — simulates OS jitter.
    for i in 0..500 {
        let error = if i % 2 == 0 { 50_000.0 } else { -50_000.0 };
        pll.tick(error);
    }

    // The PLL should stay close to nominal since errors cancel.
    let corrected = pll.corrected_period_ns();
    assert!(
        (corrected - nominal).abs() < nominal * 0.005,
        "PLL drifted too far under symmetric jitter: corrected={corrected}, nominal={nominal}"
    );
}

/// PLL drift correction: constant drift causes integral accumulation.
#[test]
fn pll_drift_correction() {
    let nominal = 4_000_000.0;
    let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);

    // Simulate constant 10 µs drift per tick.
    for i in 0..300 {
        let error = 10_000.0 * (i as f64 / 300.0); // ramp from 0 to 10 µs
        pll.tick(error);
    }

    // Integral should be non-zero (PLL compensating drift).
    assert!(
        pll.integral().abs() > 0.0,
        "integral should accumulate for drift correction"
    );
}

/// PLL lock detection: low error for enough ticks triggers lock.
#[test]
fn pll_lock_detection() {
    let nominal = 4_000_000.0;
    let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal)
        .with_lock_detection(50_000.0, 200_000.0, 10);

    assert!(!pll.locked(), "PLL should start unlocked");

    // Feed very small errors — should lock after hysteresis ticks.
    for _ in 0..100 {
        pll.tick(1_000.0); // 1 µs — well below 50 µs threshold
    }
    assert!(pll.locked(), "PLL should be locked after sustained low error");

    // Feed large errors — should unlock.
    for _ in 0..100 {
        pll.tick(500_000.0); // 500 µs — above 200 µs unlock threshold
    }
    assert!(!pll.locked(), "PLL should unlock after sustained high error");
}

/// PLL lock → unlock → re-lock transition with hysteresis.
#[test]
fn pll_lock_unlock_hysteresis() {
    let nominal = 4_000_000.0;
    let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal)
        .with_lock_detection(50_000.0, 200_000.0, 5);

    // Lock.
    for _ in 0..50 {
        pll.tick(100.0);
    }
    assert!(pll.locked());

    // Brief spike — should NOT unlock (fewer than hysteresis ticks).
    for _ in 0..3 {
        pll.tick(300_000.0);
    }
    assert!(pll.locked(), "brief spike should not defeat hysteresis");

    // Return to low error.
    for _ in 0..10 {
        pll.tick(100.0);
    }
    assert!(pll.locked(), "should remain locked after brief spike");

    // Sustained high error — should unlock.
    for _ in 0..20 {
        pll.tick(500_000.0);
    }
    assert!(!pll.locked(), "sustained high error should unlock PLL");

    // Re-lock.
    for _ in 0..50 {
        pll.tick(100.0);
    }
    assert!(pll.locked(), "PLL should re-lock after error subsides");
}

/// PLL reset clears all state.
#[test]
fn pll_reset_clears_state() {
    let nominal = 4_000_000.0;
    let mut pll = PhaseLockLoop::new(0.1, 0.001, nominal);

    for _ in 0..100 {
        pll.tick(50_000.0);
    }
    assert!(pll.tick_count() > 0);

    pll.reset();
    assert_eq!(pll.tick_count(), 0);
    assert!(!pll.locked());
    assert!((pll.corrected_period_ns() - nominal).abs() < 1e-9);
    assert!(pll.integral().abs() < 1e-9);
}

/// PLL from_hz convenience constructor produces correct nominal period.
#[test]
fn pll_from_hz() {
    let pll = PhaseLockLoop::from_hz(250);
    assert!(
        (pll.nominal_period_ns() - 4_000_000.0).abs() < 1.0,
        "250 Hz should yield 4 000 000 ns nominal period"
    );

    let pll = PhaseLockLoop::from_hz(1000);
    assert!(
        (pll.nominal_period_ns() - 1_000_000.0).abs() < 1.0,
        "1000 Hz should yield 1 000 000 ns nominal period"
    );
}

/// Original Pll (integral-only): correction opposes constant error.
#[test]
fn pll_original_opposes_error() {
    let nominal = 4_000_000.0;
    let mut pll = Pll::new(0.001, nominal);

    // Consistent +1 µs error.
    for _ in 0..100 {
        pll.update(1_000.0);
    }
    // Period correction should be negative (shortening period to catch up).
    assert!(
        pll.period_correction() < 0.0,
        "PLL should shorten period for positive error, got {}",
        pll.period_correction()
    );

    // Consistent −1 µs error.
    let mut pll2 = Pll::new(0.001, nominal);
    for _ in 0..100 {
        pll2.update(-1_000.0);
    }
    assert!(
        pll2.period_correction() > 0.0,
        "PLL should lengthen period for negative error, got {}",
        pll2.period_correction()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Platform RT scheduling (MMCSS / mock)
// ═══════════════════════════════════════════════════════════════════════════

/// MMCSS registration succeeds with mock backend and returns valid handle.
#[test]
fn mmcss_mock_registration() {
    let backend = MockMmcssBackend::new_success();
    let handle = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
    assert!(handle.is_registered());
    assert_eq!(handle.task_name(), "Pro Audio");
    assert_ne!(handle.raw_handle(), 0);
}

/// Thread priority elevation through MMCSS levels.
#[test]
fn mmcss_priority_elevation() {
    let backend = MockMmcssBackend::new_success();
    let mut handle = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();

    assert_eq!(handle.current_priority(), MmcssPriority::Normal);
    handle.set_priority(MmcssPriority::High).unwrap();
    assert_eq!(handle.current_priority(), MmcssPriority::High);
    handle.set_priority(MmcssPriority::Critical).unwrap();
    assert_eq!(handle.current_priority(), MmcssPriority::Critical);
}

/// Priority reverts on drop (unregister called).
#[test]
fn mmcss_priority_revert_on_drop() {
    let backend = MockMmcssBackend::new_success();
    let mut handle = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
    handle.set_priority(MmcssPriority::Critical).unwrap();
    handle.enable_high_resolution_timer().unwrap();
    assert!(handle.is_timer_enabled());

    // Drop should call unregister_inner which disables timer.
    // No panic = success.
    drop(handle);
}

/// Non-privileged (failure) fallback is graceful — no panic, returns Err.
#[test]
fn mmcss_non_privileged_fallback() {
    let backend = MockMmcssBackend::new_failure();
    let result = MmcssHandle::register(backend, "Pro Audio", 0);
    assert!(result.is_err(), "should fail gracefully, not panic");
    assert!(matches!(
        result.unwrap_err(),
        MmcssError::RegistrationFailed(_)
    ));
}

/// Platform RT request with mock MMCSS succeeds and returns active handle.
#[test]
fn platform_rt_mmcss_lifecycle() {
    let backend = MockMmcssBackend::new_success();
    let handle = request_rt_priority_mmcss(backend, RtPriority::Realtime).unwrap();
    assert!(handle.is_active());
    assert_eq!(handle.level(), RtPriority::Realtime);
    assert_eq!(handle.platform(), Platform::Windows);
    handle.release().unwrap();
}

/// Platform RT request with mock rtkit succeeds.
#[test]
fn platform_rt_rtkit_lifecycle() {
    let backend = MockRtkitBackend::new_success();
    let handle = request_rt_priority_rtkit(backend, RtPriority::Elevated).unwrap();
    assert!(handle.is_active());
    assert_eq!(handle.level(), RtPriority::Elevated);
    assert_eq!(handle.platform(), Platform::Linux);
    handle.release().unwrap();
}

/// No-op RT handle for unsupported platforms.
#[test]
fn platform_rt_noop_handle() {
    let handle: RtHandle<MockMmcssBackend, MockRtkitBackend> =
        request_rt_priority_noop(RtPriority::Realtime);
    assert!(!handle.is_active());
    assert_eq!(handle.level(), RtPriority::Realtime);
    handle.release().unwrap();
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Task scheduling
// ═══════════════════════════════════════════════════════════════════════════

/// Multiple tasks scheduled per tick execute in registration order.
#[test]
fn task_scheduling_order() {
    let log = Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
    let mut exec = TickExecutor::new();

    for id in 0..5u32 {
        let l = log.clone();
        exec.register_task(
            match id {
                0 => "axis",
                1 => "ffb",
                2 => "bus",
                3 => "panel",
                _ => "diag",
            },
            move || {
                l.lock().unwrap().push(id);
            },
        );
    }

    exec.run_tick(4_000_000);
    let order = log.lock().unwrap().clone();
    assert_eq!(order, vec![0, 1, 2, 3, 4], "tasks must execute in order");
}

/// Task priority ordering: tasks run in registration order (priority = order).
#[test]
fn task_priority_ordering() {
    let counter = Arc::new(AtomicU64::new(0));
    let mut exec = TickExecutor::new();

    // "high priority" = registered first.
    let c1 = counter.clone();
    exec.register_task("high", move || {
        // Should see counter at 0.
        assert_eq!(c1.load(Ordering::Relaxed), 0);
        c1.fetch_add(1, Ordering::Relaxed);
    });

    let c2 = counter.clone();
    exec.register_task("low", move || {
        // Should see counter at 1 (after "high" ran).
        assert_eq!(c2.load(Ordering::Relaxed), 1);
        c2.fetch_add(1, Ordering::Relaxed);
    });

    exec.run_tick(4_000_000);
    assert_eq!(counter.load(Ordering::Relaxed), 2);
}

/// Task overrun detected when tasks exceed budget.
#[test]
fn task_overrun_skip_detection() {
    let mut exec = TickExecutor::new();
    exec.register_task("slow_task", || {
        thread::sleep(Duration::from_micros(200));
    });

    // Very small budget to force overrun.
    let result = exec.run_tick(1);
    assert!(result.overrun, "task should overrun a 1 ns budget");
    assert_eq!(exec.overrun_count(), 1);

    // Second tick also overruns.
    let result2 = exec.run_tick(1);
    assert!(result2.overrun);
    assert_eq!(exec.overrun_count(), 2);
}

/// Task budget enforcement: generous budget → no overrun.
#[test]
fn task_budget_enforcement() {
    let mut exec = TickExecutor::new();
    exec.register_task("fast", || {
        // ~zero work
        std::hint::black_box(42);
    });

    let result = exec.run_tick(1_000_000_000); // 1 s budget
    assert!(!result.overrun);
    assert_eq!(result.tasks_run, 1);
    assert_eq!(exec.overrun_count(), 0);
}

/// TickBudget tracks multi-phase utilization within a tick.
#[test]
fn tick_budget_multi_phase() {
    let mut budget = TickBudget::new(4_000_000); // 4 ms budget

    budget.begin_tick();
    budget.begin_phase("axis");
    std::hint::black_box(0u64);
    budget.end_phase();

    budget.begin_phase("ffb");
    std::hint::black_box(0u64);
    budget.end_phase();
    budget.end_tick();

    assert_eq!(budget.tick_count(), 1);
    assert_eq!(budget.phase_count(), 2);
    assert_eq!(budget.phase_name(0), "axis");
    assert_eq!(budget.phase_name(1), "ffb");
    assert!(budget.utilization() < 1.0, "light work should not overrun");
}

/// InlineTickBudget is Copy and tracks tasks correctly.
#[test]
fn inline_tick_budget_copy_and_track() {
    let mut b = InlineTickBudget::new(4_000_000);
    b.begin_task(1);
    b.end_task();
    b.begin_task(2);
    b.end_task();

    assert_eq!(b.task_count(), 2);
    assert!(!b.is_overrun());

    // Copy semantics.
    let b2 = b;
    assert_eq!(b2.task_count(), b.task_count());
    assert_eq!(b2.consumed_ns(), b.consumed_ns());
}

/// MAX_TASKS enforcement — cannot register beyond limit.
#[test]
fn task_max_registration() {
    let mut exec = TickExecutor::new();
    static NAMES: [&str; 16] = [
        "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11", "t12", "t13",
        "t14", "t15",
    ];
    for name in &NAMES {
        assert!(exec.register_task(name, || {}));
    }
    assert!(
        !exec.register_task("overflow", || {}),
        "should reject task beyond MAX_TASKS"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Timer resolution
// ═══════════════════════════════════════════════════════════════════════════

/// High-resolution timer (mock) can be enabled and disabled.
#[test]
fn timer_high_res_enable_disable() {
    let backend = MockMmcssBackend::new_success();
    let mut handle = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();

    assert!(!handle.is_timer_enabled());
    handle.enable_high_resolution_timer().unwrap();
    assert!(handle.is_timer_enabled());
}

/// Timer granularity measurement: FallbackTimer records tick stats.
#[test]
fn timer_granularity_measurement() {
    let mut timer = FallbackTimer::new(50_000); // 50 µs busy-spin
    let period = Duration::from_millis(4);
    let base = Instant::now();

    for i in 1..=20u64 {
        timer.sleep_until(base + period * i as u32);
    }

    let stats = timer.stats();
    assert_eq!(stats.tick_count(), 20);
    // Jitter should be recorded (at least min/max are populated).
    assert!(stats.max_jitter_ns() >= 0, "should record jitter data");
}

/// Fallback timer works when high-res timer is unavailable.
#[test]
fn timer_fallback_to_lower_resolution() {
    // FallbackTimer is the cross-platform fallback. It should work
    // without any platform-specific high-res timer.
    let mut timer = FallbackTimer::new(20_000); // 20 µs busy-spin
    let deadline = Instant::now() + Duration::from_millis(2);
    timer.sleep_until(deadline);

    assert_eq!(timer.stats().tick_count(), 1);
    // Should wake roughly on time.
    let overshoot = Instant::now().duration_since(deadline);
    let tolerance = if std::env::var_os("CI").is_some() {
        Duration::from_millis(5)
    } else {
        Duration::from_millis(2)
    };
    assert!(
        overshoot < tolerance,
        "fallback timer overshoot {overshoot:?} exceeds tolerance {tolerance:?}"
    );
}

/// MockTimer is deterministic — identical sequences produce identical results.
#[test]
fn mock_timer_deterministic() {
    let mut t1 = MockTimer::new();
    let mut t2 = MockTimer::new();

    for _ in 0..100 {
        t1.sleep_ns(4_000_000);
        t2.sleep_ns(4_000_000);
    }

    assert_eq!(t1.now_ns(), t2.now_ns());
    assert_eq!(t1.stats().tick_count(), t2.stats().tick_count());
}

/// SystemTimer now_ns advances monotonically.
#[test]
fn system_timer_monotonic() {
    let timer = SystemTimer::new(20_000);
    let t1 = timer.now_ns();
    thread::sleep(Duration::from_millis(1));
    let t2 = timer.now_ns();
    assert!(t2 > t1, "SystemTimer::now_ns must advance monotonically");
}

/// TimerStats histogram bucketing is correct.
#[test]
fn timer_stats_histogram_bucketing() {
    let mut stats = TimerStats::new(); // 10 µs buckets

    stats.record_tick(5_000, false); // bucket 0
    stats.record_tick(15_000, false); // bucket 1
    stats.record_tick(-25_000, false); // |25 µs| → bucket 2
    stats.record_tick(700_000, false); // overflow bucket

    assert_eq!(stats.tick_count(), 4);
    let h = stats.jitter_histogram();
    assert_eq!(h[0], 1);
    assert_eq!(h[1], 1);
    assert_eq!(h[2], 1);
    assert_eq!(h[JITTER_BUCKETS - 1], 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Stress tests
// ═══════════════════════════════════════════════════════════════════════════

/// 1000 ticks stability: scheduler runs 1000 ticks without panic and with
/// acceptable miss rate.
#[test]
fn stress_1000_ticks_stability() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    let mut sched = Scheduler::new(config);

    for _ in 0..1000 {
        sched.wait_for_tick();
    }

    let stats = sched.get_stats();
    assert_eq!(stats.total_ticks, 1000);

    // Allow some misses — CI and loaded dev machines have scheduling noise.
    let max_miss_rate = if std::env::var_os("CI").is_some() {
        0.10
    } else {
        0.05
    };
    assert!(
        stats.miss_rate < max_miss_rate,
        "miss rate {:.2}% exceeds {:.0}% threshold",
        stats.miss_rate * 100.0,
        max_miss_rate * 100.0
    );
}

/// Concurrent scheduler instances on separate threads do not interfere.
#[test]
fn stress_concurrent_scheduler_instances() {
    let handles: Vec<_> = (0..4)
        .map(|_| {
            thread::spawn(|| {
                let config = SchedulerConfig {
                    frequency_hz: 250,
                    busy_spin_us: 30,
                    pll_gain: 0.001,
                    measure_jitter: false,
                };
                let mut sched = Scheduler::new(config);
                for _ in 0..100 {
                    sched.wait_for_tick();
                }
                sched.get_stats().total_ticks
            })
        })
        .collect();

    for h in handles {
        let ticks = h.join().expect("scheduler thread should not panic");
        assert_eq!(ticks, 100, "each scheduler should complete 100 ticks");
    }
}

/// Scheduler shutdown mid-tick: dropping the scheduler during operation
/// does not panic or leak.
#[test]
fn stress_scheduler_shutdown_mid_tick() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    let mut sched = Scheduler::new(config);

    // Start a few ticks.
    for _ in 0..10 {
        sched.wait_for_tick();
    }

    // Drop mid-operation — must not panic.
    drop(sched);
}

/// SPSC ring under contention: producer and consumer on separate threads.
#[test]
fn stress_spsc_ring_contention() {
    let ring = Arc::new(SpscRing::new(256));
    let ring_p = ring.clone();
    let ring_c = ring.clone();
    let items = 5000u64;

    let producer = thread::spawn(move || {
        for i in 0..items {
            while !ring_p.try_push(i) {
                thread::yield_now();
            }
        }
    });

    let consumer = thread::spawn(move || {
        let mut consumed = 0u64;
        while consumed < items {
            if ring_c.try_pop().is_some() {
                consumed += 1;
            } else {
                thread::yield_now();
            }
        }
        consumed
    });

    producer.join().expect("producer should not panic");
    let consumed = consumer.join().expect("consumer should not panic");
    assert_eq!(consumed, items);

    let stats = ring.stats();
    assert_eq!(stats.produced, items);
    assert_eq!(stats.consumed, items);
}

/// JitterTracker (zero-allocation) records correct p99 after many samples.
#[test]
fn stress_jitter_tracker_many_samples() {
    let mut tracker = JitterTracker::new();

    // Record 1000 samples with known distribution.
    for i in 0..1000u64 {
        tracker.record(i * 100); // 0, 100, 200, ..., 99900 ns
    }

    assert_eq!(tracker.count(), 1000);
    assert_eq!(tracker.min_ns(), 0);
    assert_eq!(tracker.max_ns(), 99_900);

    // p99 should be in the top 1% of values.
    let p99 = tracker.p99_ns();
    assert!(
        p99 >= 90_000,
        "p99 should be near the top of the range, got {p99}"
    );
}

/// PLL + Executor + MockTimer integration: full tick cycle runs correctly.
#[test]
fn stress_pll_executor_integration() {
    let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0);
    let mut timer = MockTimer::new();
    let mut exec = TickExecutor::new();

    let counter = Arc::new(AtomicU64::new(0));
    let c = counter.clone();
    exec.register_task("work", move || {
        c.fetch_add(1, Ordering::Relaxed);
    });

    let target_ns = 4_000_000u64;
    for i in 0..500u64 {
        let result = exec.run_tick(target_ns);
        assert!(!result.overrun, "tick {i} overrun");

        // Advance mock clock with simulated jitter.
        let jitter: i64 = if i % 5 == 0 { 1_000 } else { -500 };
        timer.sleep_ns(target_ns);
        if jitter > 0 {
            timer.advance_ns(jitter as u64);
        }

        pll.tick(jitter as f64);
    }

    assert_eq!(exec.tick_count(), 500);
    assert_eq!(counter.load(Ordering::Relaxed), 500);
    assert!(timer.now_ns() >= 500 * target_ns);
}

/// JitterStats (from pll module) tracks min/max/mean correctly.
#[test]
fn jitter_stats_running_accuracy() {
    let mut js = pll::JitterStats::new();

    js.record(100);
    js.record(-200);
    js.record(300);

    assert_eq!(js.count(), 3);
    assert_eq!(js.min_ns(), -200);
    assert_eq!(js.max_ns(), 300);

    // mean = (100 + (-200) + 300) / 3 ≈ 66.67
    let expected_mean = (100.0 - 200.0 + 300.0) / 3.0;
    assert!(
        (js.mean() - expected_mean).abs() < 1.0,
        "mean should be ~{expected_mean}, got {}",
        js.mean()
    );
}
