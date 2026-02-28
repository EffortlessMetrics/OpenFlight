// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Enhanced timer implementation for the RT scheduler.
//!
//! Provides:
//! - [`HighResTimer`] trait for platform-abstracted high-precision sleep
//! - [`TimerStats`] for zero-allocation tick and jitter tracking
//! - Platform implementations: Windows (`WaitableTimer`), Linux (`clock_nanosleep`),
//!   and a [`FallbackTimer`] using `thread::sleep` + busy-wait correction
//!
//! All hot-path code is zero-allocation per ADR-004.

use std::time::{Duration, Instant};

/// Number of fixed-size histogram buckets for jitter tracking.
pub const JITTER_BUCKETS: usize = 64;

/// Default bucket width: 10 µs per bucket → covers 0–640 µs of absolute jitter.
const DEFAULT_BUCKET_WIDTH_NS: u64 = 10_000;

// ---------------------------------------------------------------------------
// TimerStats — zero-allocation jitter / tick tracking
// ---------------------------------------------------------------------------

/// Zero-allocation jitter and tick statistics.
///
/// Uses a fixed-size histogram ([`JITTER_BUCKETS`] buckets) to track the
/// distribution of absolute jitter values without any heap allocation.
#[derive(Debug, Clone)]
pub struct TimerStats {
    tick_count: u64,
    missed_ticks: u64,
    /// `jitter_histogram[i]` = count of ticks with absolute jitter in
    /// `[i * bucket_width, (i+1) * bucket_width)`.
    /// The last bucket is the overflow bucket (≥ `(JITTER_BUCKETS-1) * bucket_width`).
    jitter_histogram: [u64; JITTER_BUCKETS],
    bucket_width_ns: u64,
    min_jitter_ns: i64,
    max_jitter_ns: i64,
    sum_jitter_abs_ns: u64,
    sum_jitter_squared_ns: u128,
}

impl TimerStats {
    /// Create with default bucket width (10 µs).
    pub fn new() -> Self {
        Self::with_bucket_width(DEFAULT_BUCKET_WIDTH_NS)
    }

    /// Create with a custom bucket width in nanoseconds.
    pub fn with_bucket_width(bucket_width_ns: u64) -> Self {
        Self {
            tick_count: 0,
            missed_ticks: 0,
            jitter_histogram: [0u64; JITTER_BUCKETS],
            bucket_width_ns,
            min_jitter_ns: i64::MAX,
            max_jitter_ns: i64::MIN,
            sum_jitter_abs_ns: 0,
            sum_jitter_squared_ns: 0,
        }
    }

    /// Record a single tick's jitter (**hot path — zero allocation**).
    #[inline]
    pub fn record_tick(&mut self, jitter_ns: i64, missed: bool) {
        self.tick_count += 1;
        if missed {
            self.missed_ticks += 1;
        }

        let abs_jitter = jitter_ns.unsigned_abs();
        self.sum_jitter_abs_ns = self.sum_jitter_abs_ns.wrapping_add(abs_jitter);
        self.sum_jitter_squared_ns = self
            .sum_jitter_squared_ns
            .wrapping_add((abs_jitter as u128) * (abs_jitter as u128));

        if jitter_ns < self.min_jitter_ns {
            self.min_jitter_ns = jitter_ns;
        }
        if jitter_ns > self.max_jitter_ns {
            self.max_jitter_ns = jitter_ns;
        }

        let bucket = if self.bucket_width_ns == 0 {
            0
        } else {
            let idx = (abs_jitter / self.bucket_width_ns) as usize;
            if idx >= JITTER_BUCKETS {
                JITTER_BUCKETS - 1
            } else {
                idx
            }
        };
        self.jitter_histogram[bucket] += 1;
    }

    /// Total number of ticks recorded.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Number of ticks flagged as missed.
    pub fn missed_ticks(&self) -> u64 {
        self.missed_ticks
    }

    /// Minimum signed jitter observed (nanoseconds). Returns 0 if no ticks recorded.
    pub fn min_jitter_ns(&self) -> i64 {
        if self.tick_count == 0 {
            0
        } else {
            self.min_jitter_ns
        }
    }

    /// Maximum signed jitter observed (nanoseconds). Returns 0 if no ticks recorded.
    pub fn max_jitter_ns(&self) -> i64 {
        if self.tick_count == 0 {
            0
        } else {
            self.max_jitter_ns
        }
    }

    /// Mean absolute jitter in nanoseconds.
    pub fn mean_jitter_abs_ns(&self) -> f64 {
        if self.tick_count == 0 {
            0.0
        } else {
            self.sum_jitter_abs_ns as f64 / self.tick_count as f64
        }
    }

    /// Reference to the fixed-size jitter histogram.
    pub fn jitter_histogram(&self) -> &[u64; JITTER_BUCKETS] {
        &self.jitter_histogram
    }

    /// Bucket width of the histogram in nanoseconds.
    pub fn bucket_width_ns(&self) -> u64 {
        self.bucket_width_ns
    }

    /// Reset all statistics to initial state.
    pub fn reset(&mut self) {
        let bw = self.bucket_width_ns;
        *self = Self::with_bucket_width(bw);
    }
}

impl Default for TimerStats {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HighResTimer trait
// ---------------------------------------------------------------------------

/// Platform-abstracted high-resolution timer.
///
/// Implementations **must** be zero-allocation on the hot path
/// ([`sleep_until`](HighResTimer::sleep_until)).
///
/// The timer automatically records jitter stats for every `sleep_until` call.
pub trait HighResTimer {
    /// Sleep until approximately `deadline`, using the best available
    /// platform mechanism, then busy-wait for the final microseconds.
    fn sleep_until(&mut self, deadline: Instant);

    /// Get accumulated timer statistics.
    fn stats(&self) -> &TimerStats;

    /// Reset accumulated statistics.
    fn reset_stats(&mut self);
}

// ---------------------------------------------------------------------------
// FallbackTimer — works on all platforms
// ---------------------------------------------------------------------------

/// Fallback timer using `thread::sleep` with busy-wait correction.
///
/// Subtracts a configurable busy-spin tail from the sleep duration so the
/// caller finishes with a tight spin-loop for precise wake-up.
pub struct FallbackTimer {
    busy_spin_ns: u64,
    stats: TimerStats,
}

impl FallbackTimer {
    /// Create with the given busy-spin tail duration in nanoseconds.
    pub fn new(busy_spin_ns: u64) -> Self {
        Self {
            busy_spin_ns,
            stats: TimerStats::new(),
        }
    }
}

impl HighResTimer for FallbackTimer {
    fn sleep_until(&mut self, deadline: Instant) {
        let now = Instant::now();
        if now >= deadline {
            let jitter_ns = (now - deadline).as_nanos() as i64;
            self.stats.record_tick(jitter_ns, jitter_ns > 500_000);
            return;
        }

        let remaining = deadline - now;
        let busy_dur = Duration::from_nanos(self.busy_spin_ns);
        if remaining > busy_dur {
            std::thread::sleep(remaining - busy_dur);
        }

        // Busy-wait for precise timing
        while Instant::now() < deadline {
            std::hint::spin_loop();
        }

        let actual = Instant::now();
        let jitter_ns = (actual - deadline).as_nanos() as i64;
        self.stats.record_tick(jitter_ns, false);
    }

    fn stats(&self) -> &TimerStats {
        &self.stats
    }

    fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

// ---------------------------------------------------------------------------
// SystemTimer — simple Instant-based timer
// ---------------------------------------------------------------------------

/// High-resolution timer using [`std::time::Instant`].
///
/// Provides `now_ns()` / `sleep_ns()` convenience in addition to
/// implementing [`HighResTimer`]. Uses a monotonic epoch captured at
/// construction time; `now_ns()` returns nanoseconds elapsed since then.
pub struct SystemTimer {
    epoch: Instant,
    busy_spin_ns: u64,
    stats: TimerStats,
}

impl SystemTimer {
    /// Create with the given busy-spin tail (nanoseconds).
    pub fn new(busy_spin_ns: u64) -> Self {
        Self {
            epoch: Instant::now(),
            busy_spin_ns,
            stats: TimerStats::new(),
        }
    }

    /// Nanoseconds elapsed since this timer was created.
    #[inline]
    pub fn now_ns(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    /// Sleep for `duration_ns` nanoseconds using the best available
    /// mechanism (kernel sleep + busy-wait tail).
    pub fn sleep_ns(&mut self, duration_ns: u64) {
        let deadline = Instant::now() + Duration::from_nanos(duration_ns);
        self.sleep_until(deadline);
    }
}

impl HighResTimer for SystemTimer {
    fn sleep_until(&mut self, deadline: Instant) {
        let now = Instant::now();
        if now >= deadline {
            let jitter_ns = (now - deadline).as_nanos() as i64;
            self.stats.record_tick(jitter_ns, jitter_ns > 500_000);
            return;
        }

        let remaining = deadline - now;
        let busy_dur = Duration::from_nanos(self.busy_spin_ns);
        if remaining > busy_dur {
            std::thread::sleep(remaining - busy_dur);
        }

        while Instant::now() < deadline {
            std::hint::spin_loop();
        }

        let actual = Instant::now();
        let jitter_ns = (actual - deadline).as_nanos() as i64;
        self.stats.record_tick(jitter_ns, false);
    }

    fn stats(&self) -> &TimerStats {
        &self.stats
    }

    fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

// ---------------------------------------------------------------------------
// MockTimer — deterministic timer for testing
// ---------------------------------------------------------------------------

/// Deterministic mock timer for testing.
///
/// Time only advances when explicitly called via [`advance_ns`](Self::advance_ns)
/// or [`sleep_ns`](Self::sleep_ns). This gives fully reproducible tests of
/// the scheduler pipeline without wall-clock dependencies.
///
/// Does **not** implement [`HighResTimer`] since that trait is bound to
/// real [`Instant`] deadlines. Use [`SystemTimer`] or [`FallbackTimer`] for
/// real-time execution.
pub struct MockTimer {
    current_ns: u64,
    stats: TimerStats,
}

impl MockTimer {
    /// Create a mock timer starting at time zero.
    pub fn new() -> Self {
        Self {
            current_ns: 0,
            stats: TimerStats::new(),
        }
    }

    /// Current virtual time in nanoseconds.
    #[inline]
    pub fn now_ns(&self) -> u64 {
        self.current_ns
    }

    /// Advance virtual time by `ns` nanoseconds.
    #[inline]
    pub fn advance_ns(&mut self, ns: u64) {
        self.current_ns += ns;
    }

    /// Set virtual time to an absolute value.
    pub fn set_ns(&mut self, ns: u64) {
        self.current_ns = ns;
    }

    /// Advance virtual time by `duration_ns` and record a zero-jitter tick.
    pub fn sleep_ns(&mut self, duration_ns: u64) {
        self.current_ns += duration_ns;
        self.stats.record_tick(0, false);
    }

    /// Get accumulated statistics.
    pub fn stats(&self) -> &TimerStats {
        &self.stats
    }

    /// Reset statistics (clock is NOT reset).
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

impl Default for MockTimer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Windows high-resolution timer
// ---------------------------------------------------------------------------

/// Windows timer using `WaitableTimer` / `timeBeginPeriod(1)` + busy-wait.
///
/// Delegates the OS-level sleep to [`crate::windows::platform_sleep`].
/// MMCSS thread priority should be configured separately via
/// [`crate::windows::WindowsRtThread`].
#[cfg(windows)]
pub struct WindowsHighResTimer {
    busy_spin_ns: u64,
    stats: TimerStats,
}

#[cfg(windows)]
impl WindowsHighResTimer {
    /// Create with the given busy-spin tail duration in nanoseconds.
    pub fn new(busy_spin_ns: u64) -> Self {
        Self {
            busy_spin_ns,
            stats: TimerStats::new(),
        }
    }
}

#[cfg(windows)]
impl HighResTimer for WindowsHighResTimer {
    fn sleep_until(&mut self, deadline: Instant) {
        let now = Instant::now();
        if now >= deadline {
            let jitter_ns = (now - deadline).as_nanos() as i64;
            self.stats.record_tick(jitter_ns, jitter_ns > 500_000);
            return;
        }

        let remaining = deadline - now;
        let busy_dur = Duration::from_nanos(self.busy_spin_ns);
        if remaining > busy_dur {
            crate::windows::platform_sleep(remaining - busy_dur);
        }

        while Instant::now() < deadline {
            std::hint::spin_loop();
        }

        let actual = Instant::now();
        let jitter_ns = (actual - deadline).as_nanos() as i64;
        self.stats.record_tick(jitter_ns, false);
    }

    fn stats(&self) -> &TimerStats {
        &self.stats
    }

    fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

// ---------------------------------------------------------------------------
// Linux high-resolution timer
// ---------------------------------------------------------------------------

/// Linux timer using `clock_nanosleep` with `CLOCK_MONOTONIC` + busy-wait.
///
/// Delegates the OS-level sleep to [`crate::unix::platform_sleep`].
/// RT thread priority should be configured separately via
/// [`crate::unix::LinuxRtThread`].
#[cfg(unix)]
pub struct LinuxHighResTimer {
    busy_spin_ns: u64,
    stats: TimerStats,
}

#[cfg(unix)]
impl LinuxHighResTimer {
    /// Create with the given busy-spin tail duration in nanoseconds.
    pub fn new(busy_spin_ns: u64) -> Self {
        Self {
            busy_spin_ns,
            stats: TimerStats::new(),
        }
    }
}

#[cfg(unix)]
impl HighResTimer for LinuxHighResTimer {
    fn sleep_until(&mut self, deadline: Instant) {
        let now = Instant::now();
        if now >= deadline {
            let jitter_ns = (now - deadline).as_nanos() as i64;
            self.stats.record_tick(jitter_ns, jitter_ns > 500_000);
            return;
        }

        let remaining = deadline - now;
        let busy_dur = Duration::from_nanos(self.busy_spin_ns);
        if remaining > busy_dur {
            crate::unix::platform_sleep(remaining - busy_dur);
        }

        while Instant::now() < deadline {
            std::hint::spin_loop();
        }

        let actual = Instant::now();
        let jitter_ns = (actual - deadline).as_nanos() as i64;
        self.stats.record_tick(jitter_ns, false);
    }

    fn stats(&self) -> &TimerStats {
        &self.stats
    }

    fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- TimerStats unit tests ------------------------------------------

    #[test]
    fn stats_empty_returns_zeros() {
        let s = TimerStats::new();
        assert_eq!(s.tick_count(), 0);
        assert_eq!(s.missed_ticks(), 0);
        assert_eq!(s.min_jitter_ns(), 0);
        assert_eq!(s.max_jitter_ns(), 0);
        assert!((s.mean_jitter_abs_ns() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_records_single_tick() {
        let mut s = TimerStats::new();
        s.record_tick(5_000, false); // 5µs jitter
        assert_eq!(s.tick_count(), 1);
        assert_eq!(s.missed_ticks(), 0);
        assert_eq!(s.min_jitter_ns(), 5_000);
        assert_eq!(s.max_jitter_ns(), 5_000);
    }

    #[test]
    fn stats_tracks_missed() {
        let mut s = TimerStats::new();
        s.record_tick(100_000, true);
        s.record_tick(200, false);
        assert_eq!(s.missed_ticks(), 1);
        assert_eq!(s.tick_count(), 2);
    }

    #[test]
    fn stats_histogram_bucketing() {
        let mut s = TimerStats::with_bucket_width(10_000); // 10µs buckets
        // 5µs → bucket 0
        s.record_tick(5_000, false);
        // 15µs → bucket 1
        s.record_tick(15_000, false);
        // -25µs → |25µs| → bucket 2
        s.record_tick(-25_000, false);
        // 999µs → bucket 63 (overflow, since 99 > 63)
        s.record_tick(999_000, false);

        let h = s.jitter_histogram();
        assert_eq!(h[0], 1);
        assert_eq!(h[1], 1);
        assert_eq!(h[2], 1);
        assert_eq!(h[JITTER_BUCKETS - 1], 1);
    }

    #[test]
    fn stats_min_max_with_negative_jitter() {
        let mut s = TimerStats::new();
        s.record_tick(-300, false);
        s.record_tick(500, false);
        s.record_tick(-100, false);
        assert_eq!(s.min_jitter_ns(), -300);
        assert_eq!(s.max_jitter_ns(), 500);
    }

    #[test]
    fn stats_mean_absolute_jitter() {
        let mut s = TimerStats::new();
        s.record_tick(100, false);
        s.record_tick(-200, false);
        s.record_tick(300, false);
        // mean abs = (100 + 200 + 300) / 3 = 200
        assert!((s.mean_jitter_abs_ns() - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_reset_clears_all() {
        let mut s = TimerStats::with_bucket_width(5_000);
        for i in 0..100 {
            s.record_tick(i * 100, i % 10 == 0);
        }
        assert!(s.tick_count() > 0);
        s.reset();
        assert_eq!(s.tick_count(), 0);
        assert_eq!(s.missed_ticks(), 0);
        assert_eq!(s.min_jitter_ns(), 0);
        assert_eq!(s.max_jitter_ns(), 0);
        assert_eq!(s.bucket_width_ns(), 5_000); // bucket width preserved
    }

    // -- FallbackTimer tests -------------------------------------------

    #[test]
    fn fallback_timer_sleep_accuracy() {
        let mut timer = FallbackTimer::new(50_000); // 50µs busy-spin
        let target_interval = Duration::from_millis(4); // 250 Hz
        let tolerance = if std::env::var_os("CI").is_some() {
            Duration::from_millis(10)
        } else {
            Duration::from_millis(5)
        };

        let start = Instant::now();
        let deadline = start + target_interval;
        timer.sleep_until(deadline);
        let elapsed = start.elapsed();

        // Should be close to 4ms (within tolerance)
        assert!(
            elapsed >= target_interval.saturating_sub(Duration::from_micros(500)),
            "woke too early: {elapsed:?}"
        );
        assert!(
            elapsed <= target_interval + tolerance,
            "woke too late: {elapsed:?}"
        );
    }

    #[test]
    fn fallback_timer_past_deadline_returns_immediately() {
        let mut timer = FallbackTimer::new(50_000);
        let past = Instant::now() - Duration::from_millis(10);
        let before = Instant::now();
        timer.sleep_until(past);
        let elapsed = before.elapsed();
        // Should return almost instantly
        assert!(elapsed < Duration::from_millis(1));
        // Should record a tick with positive jitter (missed)
        assert_eq!(timer.stats().tick_count(), 1);
    }

    #[test]
    fn fallback_timer_records_stats() {
        let mut timer = FallbackTimer::new(20_000);
        let now = Instant::now();

        // Sleep 3 intervals
        for i in 1..=3 {
            timer.sleep_until(now + Duration::from_millis(2 * i));
        }

        let stats = timer.stats();
        assert_eq!(stats.tick_count(), 3);
    }

    #[test]
    fn fallback_timer_reset_stats() {
        let mut timer = FallbackTimer::new(20_000);
        timer.sleep_until(Instant::now() + Duration::from_millis(1));
        assert!(timer.stats().tick_count() > 0);
        timer.reset_stats();
        assert_eq!(timer.stats().tick_count(), 0);
    }

    #[test]
    fn timer_measure_tick_intervals() {
        let mut timer = FallbackTimer::new(50_000);
        let period = Duration::from_millis(4);
        let tolerance = if std::env::var_os("CI").is_some() {
            Duration::from_millis(10)
        } else {
            Duration::from_millis(5)
        };

        let base = Instant::now();
        let mut prev = base;

        // Run 20 ticks and check intervals
        for i in 1..=20u64 {
            let deadline = base + period * i as u32;
            timer.sleep_until(deadline);
            let now = Instant::now();
            let interval = now - prev;

            assert!(
                interval >= period.saturating_sub(Duration::from_millis(2)),
                "tick {i}: interval too short: {interval:?}"
            );
            assert!(
                interval <= period + tolerance,
                "tick {i}: interval too long: {interval:?}"
            );
            prev = now;
        }
    }

    // -- SystemTimer tests --------------------------------------------------

    #[test]
    fn system_timer_now_ns_advances() {
        let timer = SystemTimer::new(20_000);
        let t1 = timer.now_ns();
        std::thread::sleep(Duration::from_millis(1));
        let t2 = timer.now_ns();
        assert!(t2 > t1, "now_ns should advance with real time");
    }

    #[test]
    fn system_timer_sleep_ns_records_stats() {
        let mut timer = SystemTimer::new(20_000);
        timer.sleep_ns(1_000_000); // 1ms
        assert_eq!(timer.stats().tick_count(), 1);
    }

    #[test]
    fn system_timer_implements_high_res_timer() {
        let mut timer = SystemTimer::new(20_000);
        let deadline = Instant::now() + Duration::from_millis(2);
        timer.sleep_until(deadline);
        assert_eq!(timer.stats().tick_count(), 1);
    }

    // -- MockTimer tests ----------------------------------------------------

    #[test]
    fn mock_timer_starts_at_zero() {
        let timer = MockTimer::new();
        assert_eq!(timer.now_ns(), 0);
    }

    #[test]
    fn mock_timer_advance() {
        let mut timer = MockTimer::new();
        timer.advance_ns(1_000_000);
        assert_eq!(timer.now_ns(), 1_000_000);
        timer.advance_ns(500_000);
        assert_eq!(timer.now_ns(), 1_500_000);
    }

    #[test]
    fn mock_timer_set_ns() {
        let mut timer = MockTimer::new();
        timer.set_ns(42_000_000);
        assert_eq!(timer.now_ns(), 42_000_000);
    }

    #[test]
    fn mock_timer_sleep_ns_advances_clock() {
        let mut timer = MockTimer::new();
        timer.sleep_ns(4_000_000);
        assert_eq!(timer.now_ns(), 4_000_000);
        timer.sleep_ns(4_000_000);
        assert_eq!(timer.now_ns(), 8_000_000);
    }

    #[test]
    fn mock_timer_sleep_ns_records_stats() {
        let mut timer = MockTimer::new();
        timer.sleep_ns(4_000_000);
        timer.sleep_ns(4_000_000);
        assert_eq!(timer.stats().tick_count(), 2);
    }

    #[test]
    fn mock_timer_deterministic_sequence() {
        let mut t1 = MockTimer::new();
        let mut t2 = MockTimer::new();
        for _ in 0..100 {
            t1.sleep_ns(4_000_000);
            t2.sleep_ns(4_000_000);
        }
        assert_eq!(t1.now_ns(), t2.now_ns());
        assert_eq!(t1.stats().tick_count(), t2.stats().tick_count());
    }

    #[test]
    fn mock_timer_reset_stats() {
        let mut timer = MockTimer::new();
        timer.sleep_ns(1_000_000);
        assert!(timer.stats().tick_count() > 0);
        timer.reset_stats();
        assert_eq!(timer.stats().tick_count(), 0);
        // Clock should NOT reset
        assert_eq!(timer.now_ns(), 1_000_000);
    }
}
