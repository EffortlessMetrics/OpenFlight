// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-tick time budget tracking for the RT scheduler.
//!
//! [`TickBudget`] tracks how much wall-clock time each processing phase
//! consumes within every tick, enabling overrun detection and utilization
//! monitoring.
//!
//! All structures use fixed-size arrays — **zero allocation on the hot path**
//! per ADR-004.

use std::time::Instant;

/// Maximum number of distinct processing phases that can be tracked.
pub const MAX_PHASES: usize = 8;

/// Sentinel value indicating no active phase.
const NO_PHASE: usize = MAX_PHASES;

// ---------------------------------------------------------------------------
// PhaseSlot — internal per-phase bookkeeping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct PhaseSlot {
    name: &'static str,
    cumulative_ns: u64,
    max_ns: u64,
    invocations: u64,
}

impl PhaseSlot {
    const EMPTY: Self = Self {
        name: "",
        cumulative_ns: 0,
        max_ns: 0,
        invocations: 0,
    };
}

// ---------------------------------------------------------------------------
// TickBudget
// ---------------------------------------------------------------------------

/// Per-tick time budget tracker (zero-allocation).
///
/// Call [`begin_tick`](Self::begin_tick) at tick start, bracket each processing
/// phase with [`begin_phase`](Self::begin_phase) / [`end_phase`](Self::end_phase),
/// then call [`end_tick`](Self::end_tick).
///
/// # Example
///
/// ```ignore
/// let mut budget = TickBudget::new(4_000_000); // 4 ms budget (250 Hz)
///
/// budget.begin_tick();
///
/// budget.begin_phase("axis");
/// // … axis processing …
/// budget.end_phase();
///
/// budget.begin_phase("ffb");
/// // … FFB processing …
/// budget.end_phase();
///
/// budget.end_tick();
///
/// println!("utilization: {:.1}%", budget.utilization() * 100.0);
/// ```
pub struct TickBudget {
    tick_period_ns: u64,
    phases: [PhaseSlot; MAX_PHASES],
    phase_count: usize,
    active_phase: usize,
    phase_start: Instant,
    tick_start: Instant,
    tick_active: bool,
    overrun_count: u64,
    tick_count: u64,
    total_used_ns: u64,
}

impl TickBudget {
    /// Create a new budget tracker with the given tick period in nanoseconds.
    pub fn new(tick_period_ns: u64) -> Self {
        let now = Instant::now();
        Self {
            tick_period_ns,
            phases: [PhaseSlot::EMPTY; MAX_PHASES],
            phase_count: 0,
            active_phase: NO_PHASE,
            phase_start: now,
            tick_start: now,
            tick_active: false,
            overrun_count: 0,
            tick_count: 0,
            total_used_ns: 0,
        }
    }

    /// Mark the start of a new tick.
    #[inline]
    pub fn begin_tick(&mut self) {
        self.tick_start = Instant::now();
        self.tick_active = true;
    }

    /// Mark the start of a named processing phase.
    ///
    /// Phase names must be `&'static str` to guarantee zero allocation.
    /// If [`MAX_PHASES`] distinct phases have already been registered, the
    /// call is silently ignored.
    #[inline]
    pub fn begin_phase(&mut self, name: &'static str) {
        // Find or register the phase (linear scan — MAX_PHASES is small)
        let idx = self.find_or_register(name);
        if idx < MAX_PHASES {
            self.active_phase = idx;
            self.phase_start = Instant::now();
        }
    }

    /// Mark the end of the current processing phase.
    #[inline]
    pub fn end_phase(&mut self) {
        if self.active_phase >= MAX_PHASES {
            return;
        }

        let elapsed_ns = self.phase_start.elapsed().as_nanos() as u64;
        let slot = &mut self.phases[self.active_phase];
        slot.cumulative_ns += elapsed_ns;
        slot.invocations += 1;
        if elapsed_ns > slot.max_ns {
            slot.max_ns = elapsed_ns;
        }

        self.active_phase = NO_PHASE;
    }

    /// Mark the end of the current tick.
    ///
    /// Checks whether the tick exceeded its budget and updates overrun counters.
    #[inline]
    pub fn end_tick(&mut self) {
        if !self.tick_active {
            return;
        }

        // Close any unclosed phase
        if self.active_phase < MAX_PHASES {
            self.end_phase();
        }

        let tick_elapsed_ns = self.tick_start.elapsed().as_nanos() as u64;
        self.total_used_ns += tick_elapsed_ns;
        self.tick_count += 1;

        if tick_elapsed_ns > self.tick_period_ns {
            self.overrun_count += 1;
        }

        self.tick_active = false;
    }

    /// Number of ticks that exceeded the configured budget.
    pub fn overrun_count(&self) -> u64 {
        self.overrun_count
    }

    /// Overall utilization as a fraction `[0.0, ∞)` of tick budget.
    ///
    /// A value of `0.5` means half the available tick time is used on average.
    /// Values above `1.0` indicate the workload consistently overruns.
    pub fn utilization(&self) -> f64 {
        if self.tick_count == 0 || self.tick_period_ns == 0 {
            return 0.0;
        }
        let budget_ns = self.tick_count * self.tick_period_ns;
        self.total_used_ns as f64 / budget_ns as f64
    }

    /// Utilization of a single phase by index.
    ///
    /// Returns `0.0` if the index is out of range or no ticks have been recorded.
    pub fn phase_utilization(&self, index: usize) -> f64 {
        if index >= self.phase_count || self.tick_count == 0 || self.tick_period_ns == 0 {
            return 0.0;
        }
        let budget_ns = self.tick_count * self.tick_period_ns;
        self.phases[index].cumulative_ns as f64 / budget_ns as f64
    }

    /// Name of the phase at the given index, or `""` if out of range.
    pub fn phase_name(&self, index: usize) -> &'static str {
        if index >= self.phase_count {
            ""
        } else {
            self.phases[index].name
        }
    }

    /// Maximum duration observed for a single invocation of the given phase (ns).
    pub fn phase_max_ns(&self, index: usize) -> u64 {
        if index >= self.phase_count {
            0
        } else {
            self.phases[index].max_ns
        }
    }

    /// Number of distinct phases currently registered.
    pub fn phase_count(&self) -> usize {
        self.phase_count
    }

    /// Number of ticks completed.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Configured tick period in nanoseconds.
    pub fn tick_period_ns(&self) -> u64 {
        self.tick_period_ns
    }

    /// Reset all counters and phase data.
    pub fn reset(&mut self) {
        self.phases = [PhaseSlot::EMPTY; MAX_PHASES];
        self.phase_count = 0;
        self.active_phase = NO_PHASE;
        self.tick_active = false;
        self.overrun_count = 0;
        self.tick_count = 0;
        self.total_used_ns = 0;
    }

    // -- internal helpers -----------------------------------------------

    /// Find an existing phase slot by name or register a new one.
    /// Returns `MAX_PHASES` if the table is full.
    #[inline]
    fn find_or_register(&mut self, name: &'static str) -> usize {
        // Linear scan — MAX_PHASES is ≤ 8, so this is fast and branch-free-ish.
        for i in 0..self.phase_count {
            // Compare by pointer identity first (cheap), then by value.
            if std::ptr::eq(self.phases[i].name, name) || self.phases[i].name == name {
                return i;
            }
        }

        if self.phase_count >= MAX_PHASES {
            return MAX_PHASES;
        }

        let idx = self.phase_count;
        self.phases[idx].name = name;
        self.phase_count += 1;
        idx
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn budget_empty_returns_zeros() {
        let b = TickBudget::new(4_000_000);
        assert_eq!(b.tick_count(), 0);
        assert_eq!(b.overrun_count(), 0);
        assert!((b.utilization() - 0.0).abs() < f64::EPSILON);
        assert_eq!(b.phase_count(), 0);
    }

    #[test]
    fn budget_single_tick_no_overrun() {
        let mut b = TickBudget::new(100_000_000); // 100 ms — generous budget

        b.begin_tick();
        b.begin_phase("axis");
        // Tiny spin to simulate work
        let _ = (0..100).sum::<u64>();
        b.end_phase();
        b.end_tick();

        assert_eq!(b.tick_count(), 1);
        assert_eq!(b.overrun_count(), 0);
        assert!(b.utilization() < 1.0);
    }

    #[test]
    fn budget_overrun_detected() {
        // 1 ns budget — everything overruns
        let mut b = TickBudget::new(1);

        b.begin_tick();
        b.begin_phase("work");
        // Spin for a measurable amount of time
        std::thread::sleep(Duration::from_micros(10));
        b.end_phase();
        b.end_tick();

        assert_eq!(b.tick_count(), 1);
        assert_eq!(b.overrun_count(), 1);
        assert!(b.utilization() > 1.0);
    }

    #[test]
    fn budget_multiple_phases_tracked() {
        let mut b = TickBudget::new(100_000_000);

        b.begin_tick();

        b.begin_phase("axis");
        std::thread::sleep(Duration::from_micros(50));
        b.end_phase();

        b.begin_phase("ffb");
        std::thread::sleep(Duration::from_micros(50));
        b.end_phase();

        b.begin_phase("bus");
        b.end_phase();

        b.end_tick();

        assert_eq!(b.phase_count(), 3);
        assert_eq!(b.phase_name(0), "axis");
        assert_eq!(b.phase_name(1), "ffb");
        assert_eq!(b.phase_name(2), "bus");
        assert_eq!(b.tick_count(), 1);
    }

    #[test]
    fn budget_phase_utilization() {
        // 10 ms budget
        let mut b = TickBudget::new(10_000_000);

        b.begin_tick();
        b.begin_phase("work");
        std::thread::sleep(Duration::from_millis(1));
        b.end_phase();
        b.end_tick();

        // Phase utilization should be roughly 0.1 (1ms out of 10ms)
        let u = b.phase_utilization(0);
        assert!(u > 0.01, "utilization too low: {u}");
        assert!(u < 0.9, "utilization too high: {u}");
    }

    #[test]
    fn budget_max_phases_enforced() {
        let mut b = TickBudget::new(100_000_000);
        b.begin_tick();

        // Register exactly MAX_PHASES phases
        static NAMES: [&str; MAX_PHASES] = ["a", "b", "c", "d", "e", "f", "g", "h"];
        for name in &NAMES {
            b.begin_phase(name);
            b.end_phase();
        }
        assert_eq!(b.phase_count(), MAX_PHASES);

        // Next phase should be silently ignored
        b.begin_phase("overflow");
        b.end_phase();
        assert_eq!(b.phase_count(), MAX_PHASES);

        b.end_tick();
    }

    #[test]
    fn budget_unclosed_phase_auto_closed_on_end_tick() {
        let mut b = TickBudget::new(100_000_000);

        b.begin_tick();
        b.begin_phase("oops");
        // Intentionally skip end_phase()
        b.end_tick();

        assert_eq!(b.tick_count(), 1);
        assert_eq!(b.phase_count(), 1);
        assert!(b.phases[0].invocations > 0);
    }

    #[test]
    fn budget_reset_clears_everything() {
        let mut b = TickBudget::new(100_000_000);

        b.begin_tick();
        b.begin_phase("x");
        b.end_phase();
        b.end_tick();
        assert!(b.tick_count() > 0);

        b.reset();
        assert_eq!(b.tick_count(), 0);
        assert_eq!(b.overrun_count(), 0);
        assert_eq!(b.phase_count(), 0);
        assert!((b.utilization() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn budget_repeated_phase_accumulates() {
        let mut b = TickBudget::new(100_000_000);

        for _ in 0..5 {
            b.begin_tick();
            b.begin_phase("axis");
            std::thread::sleep(Duration::from_micros(10));
            b.end_phase();
            b.end_tick();
        }

        assert_eq!(b.tick_count(), 5);
        assert_eq!(b.phase_count(), 1); // same phase reused
        assert!(b.phases[0].invocations == 5);
        assert!(b.phases[0].cumulative_ns > 0);
    }

    #[test]
    fn budget_overrun_count_across_ticks() {
        // 1 ns budget — guaranteed overrun
        let mut b = TickBudget::new(1);

        for _ in 0..10 {
            b.begin_tick();
            std::thread::sleep(Duration::from_micros(1));
            b.end_tick();
        }

        assert_eq!(b.tick_count(), 10);
        assert_eq!(b.overrun_count(), 10);
    }

    #[test]
    fn budget_end_tick_without_begin_is_noop() {
        let mut b = TickBudget::new(100_000_000);
        b.end_tick();
        assert_eq!(b.tick_count(), 0);
    }
}
