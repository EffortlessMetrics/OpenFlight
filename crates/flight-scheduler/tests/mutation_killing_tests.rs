// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted mutation-killing tests for flight-scheduler.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use flight_scheduler::executor::TickExecutor;
use flight_scheduler::pll::{PhaseLockLoop, Pll};

// ---------------------------------------------------------------------------
// 1. tick_rate_correct_value
//    Pll with zero error must return exactly the nominal period.
// ---------------------------------------------------------------------------
#[test]
fn tick_rate_correct_value() {
    let mut pll = Pll::new(0.001, 4_000_000.0);
    let period = pll.update(0.0);
    assert_eq!(
        period, 4_000_000.0,
        "zero error must yield exactly the nominal period"
    );
}

// ---------------------------------------------------------------------------
// 2. task_execution_order_by_registration
//    Tasks must execute in the order they were registered.
// ---------------------------------------------------------------------------
#[test]
fn task_execution_order_by_registration() {
    let log: Arc<Mutex<Vec<&str>>> = Arc::new(Mutex::new(Vec::new()));
    let mut exec = TickExecutor::new();

    let l = log.clone();
    exec.register_task("a", move || l.lock().unwrap().push("a"));
    let l = log.clone();
    exec.register_task("b", move || l.lock().unwrap().push("b"));
    let l = log.clone();
    exec.register_task("c", move || l.lock().unwrap().push("c"));

    exec.run_tick(4_000_000);
    assert_eq!(
        *log.lock().unwrap(),
        vec!["a", "b", "c"],
        "tasks must run in registration order"
    );
}

// ---------------------------------------------------------------------------
// 3. overrun_detection_threshold_exact
//    Overrun flag and counter must track budget violations precisely.
// ---------------------------------------------------------------------------
#[test]
fn overrun_detection_threshold_exact() {
    let mut exec = TickExecutor::new();
    exec.register_task("slow", || {
        std::thread::sleep(Duration::from_micros(100));
    });

    // 1 ns budget — guaranteed overrun
    let r1 = exec.run_tick(1);
    assert!(r1.overrun, "must detect overrun with 1 ns budget");
    assert_eq!(exec.overrun_count(), 1, "overrun counter must be 1");

    // 1 s budget — no overrun
    let r2 = exec.run_tick(1_000_000_000);
    assert!(!r2.overrun, "must not flag overrun with 1 s budget");
    assert_eq!(
        exec.overrun_count(),
        1,
        "overrun counter must stay at 1 after a non-overrun tick"
    );
}

// ---------------------------------------------------------------------------
// 4. pll_convergence_direction
//    Positive error → shorter period; negative error → longer period.
// ---------------------------------------------------------------------------
#[test]
fn pll_convergence_direction() {
    let nominal = 4_000_000.0;

    // Positive error (late) → period must decrease to catch up.
    let mut pll = Pll::new(0.001, nominal);
    let period_late = pll.update(1000.0);
    assert!(
        period_late < nominal,
        "positive error must shorten period (got {period_late})"
    );

    // Negative error (early) → period must increase to slow down.
    let mut pll = Pll::new(0.001, nominal);
    let period_early = pll.update(-1000.0);
    assert!(
        period_early > nominal,
        "negative error must lengthen period (got {period_early})"
    );
}

// ---------------------------------------------------------------------------
// 5. pll_lock_detection_transitions
//    Lock / unlock transitions honour hysteresis counter.
// ---------------------------------------------------------------------------
#[test]
fn pll_lock_detection_transitions() {
    let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0).with_lock_detection(
        50_000.0,  // lock threshold
        200_000.0, // unlock threshold
        3,         // hysteresis
    );

    // Initially NOT locked.
    assert!(!pll.locked(), "PLL must start unlocked");

    // Feed 3 ticks with error = 0 (well below lock threshold).
    for _ in 0..3 {
        pll.tick(0.0);
    }
    assert!(pll.locked(), "PLL must lock after 3 low-error ticks");

    // Feed 3 ticks with error = 300_000 (above unlock threshold).
    for _ in 0..3 {
        pll.tick(300_000.0);
    }
    assert!(
        !pll.locked(),
        "PLL must unlock after 3 high-error ticks"
    );
}
