// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-tick task executor for the RT scheduler.
//!
//! [`TickExecutor`] manages an ordered list of per-tick tasks with budget
//! tracking. Tasks are registered at setup time; the hot path
//! ([`run_tick`](TickExecutor::run_tick)) only iterates a pre-allocated
//! array — **zero allocation** per ADR-004.

use std::time::Instant;

/// Maximum number of tasks that can be registered.
pub const MAX_TASKS: usize = 16;

// ---------------------------------------------------------------------------
// TaskStats
// ---------------------------------------------------------------------------

/// Per-task timing statistics (zero-allocation, stack-only).
#[derive(Debug, Clone, Copy)]
pub struct TaskStats {
    /// Task name.
    pub name: &'static str,
    /// Number of times the task has been invoked.
    pub invocations: u64,
    /// Cumulative execution time (nanoseconds).
    pub total_ns: u64,
    /// Worst-case single invocation (nanoseconds).
    pub max_ns: u64,
    /// Most recent invocation duration (nanoseconds).
    pub last_ns: u64,
}

impl TaskStats {
    const EMPTY: Self = Self {
        name: "",
        invocations: 0,
        total_ns: 0,
        max_ns: 0,
        last_ns: 0,
    };

    /// Mean execution time per invocation (nanoseconds).
    pub fn mean_ns(&self) -> f64 {
        if self.invocations == 0 {
            0.0
        } else {
            self.total_ns as f64 / self.invocations as f64
        }
    }
}

// ---------------------------------------------------------------------------
// TickExecutionResult
// ---------------------------------------------------------------------------

/// Result of a single [`TickExecutor::run_tick`] call.
#[derive(Debug, Clone)]
pub struct TickExecutionResult {
    /// Total wall-clock time consumed by all tasks (nanoseconds).
    pub total_ns: u64,
    /// Number of tasks that were executed.
    pub tasks_run: usize,
    /// `true` if total execution time exceeded the budget.
    pub overrun: bool,
}

// ---------------------------------------------------------------------------
// TickExecutor
// ---------------------------------------------------------------------------

type TaskFn = Box<dyn FnMut() + Send>;

struct TaskSlot {
    callback: TaskFn,
    stats: TaskStats,
}

/// Ordered per-tick task executor with budget tracking.
///
/// Register tasks at startup with [`register_task`](Self::register_task),
/// then call [`run_tick`](Self::run_tick) each tick to execute them in
/// order.
///
/// Up to [`MAX_TASKS`] tasks are supported. The internal storage is
/// pre-allocated at construction; `run_tick` performs **no allocation**.
///
/// # Overrun handling
///
/// If a task or the tick as a whole exceeds the budget, the executor logs a
/// `tracing::warn!` and **continues** — the RT spine must never block
/// (ADR-001).
pub struct TickExecutor {
    // Pre-allocated to MAX_TASKS. The Vec is allocated once at construction
    // (non-RT path). The hot path (run_tick) only iterates the existing
    // slice — no allocation.
    tasks: Vec<TaskSlot>,
    tick_count: u64,
    overrun_count: u64,
}

impl TickExecutor {
    /// Create a new executor with no registered tasks.
    pub fn new() -> Self {
        Self {
            tasks: Vec::with_capacity(MAX_TASKS),
            tick_count: 0,
            overrun_count: 0,
        }
    }

    /// Register a named per-tick task.
    ///
    /// Tasks execute in registration order. Returns `true` if the task was
    /// registered, `false` if [`MAX_TASKS`] has been reached.
    pub fn register_task(
        &mut self,
        name: &'static str,
        callback: impl FnMut() + Send + 'static,
    ) -> bool {
        if self.tasks.len() >= MAX_TASKS {
            return false;
        }
        self.tasks.push(TaskSlot {
            callback: Box::new(callback),
            stats: TaskStats {
                name,
                ..TaskStats::EMPTY
            },
        });
        true
    }

    /// Execute all registered tasks within the given budget.
    ///
    /// Returns a [`TickExecutionResult`] summarising timing and overrun
    /// status. Tasks always run to completion — the budget is advisory and
    /// used only for overrun detection.
    pub fn run_tick(&mut self, budget_ns: u64) -> TickExecutionResult {
        let tick_start = Instant::now();
        let mut tasks_run = 0usize;

        for slot in self.tasks.iter_mut() {
            let task_start = Instant::now();
            (slot.callback)();
            let task_elapsed_ns = task_start.elapsed().as_nanos() as u64;

            slot.stats.invocations += 1;
            slot.stats.total_ns += task_elapsed_ns;
            slot.stats.last_ns = task_elapsed_ns;
            if task_elapsed_ns > slot.stats.max_ns {
                slot.stats.max_ns = task_elapsed_ns;
            }

            tasks_run += 1;
        }

        let total_ns = tick_start.elapsed().as_nanos() as u64;
        let overrun = total_ns > budget_ns;

        if overrun {
            self.overrun_count += 1;
            tracing::warn!(
                total_ns,
                budget_ns,
                "tick budget overrun ({} tasks)",
                tasks_run
            );
        }

        self.tick_count += 1;

        TickExecutionResult {
            total_ns,
            tasks_run,
            overrun,
        }
    }

    /// Look up per-task statistics by name.
    pub fn task_stats(&self, name: &str) -> Option<&TaskStats> {
        self.tasks
            .iter()
            .find(|slot| slot.stats.name == name)
            .map(|slot| &slot.stats)
    }

    /// Number of ticks executed so far.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Number of ticks that exceeded their budget.
    pub fn overrun_count(&self) -> u64 {
        self.overrun_count
    }

    /// Number of registered tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Reset tick and overrun counters (does not remove tasks).
    pub fn reset_stats(&mut self) {
        self.tick_count = 0;
        self.overrun_count = 0;
        for slot in self.tasks.iter_mut() {
            slot.stats.invocations = 0;
            slot.stats.total_ns = 0;
            slot.stats.max_ns = 0;
            slot.stats.last_ns = 0;
        }
    }
}

impl Default for TickExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    #[test]
    fn executor_empty_run() {
        let mut exec = TickExecutor::new();
        let result = exec.run_tick(4_000_000);
        assert_eq!(result.tasks_run, 0);
        assert!(!result.overrun);
        assert_eq!(exec.tick_count(), 1);
    }

    #[test]
    fn executor_register_and_run() {
        let counter = Arc::new(AtomicU64::new(0));
        let c = counter.clone();

        let mut exec = TickExecutor::new();
        assert!(exec.register_task("inc", move || {
            c.fetch_add(1, Ordering::Relaxed);
        }));

        exec.run_tick(4_000_000);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        exec.run_tick(4_000_000);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn executor_task_ordering() {
        let log = Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
        let mut exec = TickExecutor::new();

        let l1 = log.clone();
        exec.register_task("first", move || {
            l1.lock().unwrap().push(1);
        });
        let l2 = log.clone();
        exec.register_task("second", move || {
            l2.lock().unwrap().push(2);
        });
        let l3 = log.clone();
        exec.register_task("third", move || {
            l3.lock().unwrap().push(3);
        });

        exec.run_tick(4_000_000);
        assert_eq!(*log.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn executor_max_tasks_enforced() {
        let mut exec = TickExecutor::new();
        static NAMES: [&str; 16] = [
            "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11", "t12", "t13",
            "t14", "t15",
        ];
        for (i, name) in NAMES.iter().enumerate() {
            assert!(exec.register_task(name, || {}), "task {i} should register");
        }
        assert_eq!(exec.task_count(), MAX_TASKS);
        assert!(
            !exec.register_task("overflow", || {}),
            "should reject beyond MAX_TASKS"
        );
    }

    #[test]
    fn executor_budget_overrun_detected() {
        let mut exec = TickExecutor::new();
        exec.register_task("slow", || {
            std::thread::sleep(Duration::from_micros(100));
        });
        // 1ns budget — guaranteed overrun
        let result = exec.run_tick(1);
        assert!(result.overrun);
        assert_eq!(exec.overrun_count(), 1);
    }

    #[test]
    fn executor_no_overrun_with_generous_budget() {
        let mut exec = TickExecutor::new();
        exec.register_task("fast", || {
            let _ = 1 + 1;
        });
        let result = exec.run_tick(1_000_000_000); // 1s budget
        assert!(!result.overrun);
    }

    #[test]
    fn executor_task_stats_tracked() {
        let mut exec = TickExecutor::new();
        exec.register_task("work", || {
            std::thread::sleep(Duration::from_micros(10));
        });

        for _ in 0..5 {
            exec.run_tick(100_000_000);
        }

        let stats = exec.task_stats("work").expect("should find task");
        assert_eq!(stats.invocations, 5);
        assert!(stats.total_ns > 0);
        assert!(stats.max_ns > 0);
        assert!(stats.last_ns > 0);
        assert!(stats.mean_ns() > 0.0);
    }

    #[test]
    fn executor_task_stats_unknown_returns_none() {
        let exec = TickExecutor::new();
        assert!(exec.task_stats("nonexistent").is_none());
    }

    #[test]
    fn executor_tick_count_increments() {
        let mut exec = TickExecutor::new();
        assert_eq!(exec.tick_count(), 0);
        for i in 1..=10u64 {
            exec.run_tick(4_000_000);
            assert_eq!(exec.tick_count(), i);
        }
    }

    #[test]
    fn executor_reset_stats() {
        let counter = Arc::new(AtomicU64::new(0));
        let c = counter.clone();

        let mut exec = TickExecutor::new();
        exec.register_task("inc", move || {
            c.fetch_add(1, Ordering::Relaxed);
        });

        for _ in 0..5 {
            exec.run_tick(4_000_000);
        }
        assert_eq!(exec.tick_count(), 5);
        assert_eq!(exec.task_stats("inc").unwrap().invocations, 5);

        exec.reset_stats();
        assert_eq!(exec.tick_count(), 0);
        assert_eq!(exec.overrun_count(), 0);
        assert_eq!(exec.task_stats("inc").unwrap().invocations, 0);

        // Tasks still registered and functional
        exec.run_tick(4_000_000);
        assert_eq!(counter.load(Ordering::Relaxed), 6);
    }

    // -- Integration: PLL + Executor + MockTimer tick cycle ----------------

    #[test]
    fn integration_pll_executor_tick_cycle() {
        use crate::pll::{JitterStats, PhaseLockLoop};
        use crate::timer::MockTimer;

        let mut pll = PhaseLockLoop::new(0.1, 0.001, 4_000_000.0);
        let mut jitter = JitterStats::new();
        let mut timer = MockTimer::new();
        let mut exec = TickExecutor::new();

        let work_counter = Arc::new(AtomicU64::new(0));
        let wc = work_counter.clone();
        exec.register_task("axis", move || {
            wc.fetch_add(1, Ordering::Relaxed);
        });

        let target_ns = 4_000_000u64;

        for i in 0..100u64 {
            // Execute tasks
            let result = exec.run_tick(target_ns);
            assert!(!result.overrun, "tick {i} overrun");

            // Advance mock clock by target period + small simulated error
            let error_ns: i64 = if i % 3 == 0 { 500 } else { -200 };
            timer.sleep_ns(target_ns);
            if error_ns > 0 {
                timer.advance_ns(error_ns as u64);
            }

            // Feed error to PLL
            let pll_result = pll.tick_with_result(error_ns as f64);
            jitter.record(error_ns);

            // Verify PLL corrects in the right direction
            if error_ns > 0 {
                assert!(pll_result.correction_ns >= 0.0);
            }
        }

        // Verify all components tracked 100 iterations
        assert_eq!(exec.tick_count(), 100);
        assert_eq!(pll.tick_count(), 100);
        assert_eq!(jitter.count(), 100);
        assert_eq!(work_counter.load(Ordering::Relaxed), 100);
        assert!(timer.now_ns() >= 100 * target_ns);

        // Jitter stats should reflect the injected pattern
        assert!(jitter.min_ns() <= -200);
        assert!(jitter.max_ns() >= 500);
    }
}
