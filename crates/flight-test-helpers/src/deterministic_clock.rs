// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! A deterministic clock for testing time-dependent code.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Microseconds per tick at the 250 Hz RT spine rate.
const US_PER_TICK: u64 = 4_000;

/// A deterministic clock for testing time-dependent code.
///
/// Advances only when explicitly told to, making tests fully reproducible.
#[derive(Debug, Clone)]
pub struct DeterministicClock {
    current_time_us: u64,
}

impl DeterministicClock {
    /// Create a new clock starting at `initial_time_us` microseconds.
    pub fn new(initial_time_us: u64) -> Self {
        Self {
            current_time_us: initial_time_us,
        }
    }

    /// Create a new clock starting at zero.
    pub fn zero() -> Self {
        Self::new(0)
    }

    /// Return the current time in microseconds.
    pub fn now_us(&self) -> u64 {
        self.current_time_us
    }

    /// Return the current time as a [`Duration`].
    pub fn now(&self) -> Duration {
        Duration::from_micros(self.current_time_us)
    }

    /// Advance the clock by `us` microseconds.
    pub fn advance(&mut self, us: u64) {
        self.current_time_us += us;
    }

    /// Advance the clock by a [`Duration`].
    pub fn advance_duration(&mut self, duration: Duration) {
        self.current_time_us += u64::try_from(duration.as_micros()).expect("duration too large");
    }

    /// Advance the clock by `ms` milliseconds.
    pub fn advance_ms(&mut self, ms: u64) {
        self.current_time_us += ms * 1_000;
    }

    /// Advance the clock by `ticks` RT spine ticks (each tick = 4 000 µs at 250 Hz).
    pub fn advance_ticks(&mut self, ticks: u32) {
        self.current_time_us += u64::from(ticks) * US_PER_TICK;
    }

    /// Reset the clock to zero.
    pub fn reset(&mut self) {
        self.current_time_us = 0;
    }
}

// ---------------------------------------------------------------------------
// SharedClock — thread-safe wrapper
// ---------------------------------------------------------------------------

/// A thread-safe wrapper around [`DeterministicClock`] for use across threads.
///
/// Internally uses `Arc<Mutex<DeterministicClock>>` so all clones share the
/// same time source.
#[derive(Debug, Clone)]
pub struct SharedClock {
    inner: Arc<Mutex<DeterministicClock>>,
}

impl SharedClock {
    /// Create a new shared clock starting at zero.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(DeterministicClock::zero())),
        }
    }

    /// Create a new shared clock starting at `initial_time_us` microseconds.
    pub fn with_initial(initial_time_us: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DeterministicClock::new(initial_time_us))),
        }
    }

    /// Return the current time in microseconds.
    pub fn now_us(&self) -> u64 {
        self.inner.lock().expect("clock lock poisoned").now_us()
    }

    /// Return the current time as a [`Duration`].
    pub fn now(&self) -> Duration {
        self.inner.lock().expect("clock lock poisoned").now()
    }

    /// Advance the clock by `us` microseconds.
    pub fn advance(&self, us: u64) {
        self.inner.lock().expect("clock lock poisoned").advance(us);
    }

    /// Advance the clock by a [`Duration`].
    pub fn advance_duration(&self, duration: Duration) {
        self.inner
            .lock()
            .expect("clock lock poisoned")
            .advance_duration(duration);
    }

    /// Advance the clock by `ticks` RT spine ticks.
    pub fn advance_ticks(&self, ticks: u32) {
        self.inner
            .lock()
            .expect("clock lock poisoned")
            .advance_ticks(ticks);
    }

    /// Reset the clock to zero.
    pub fn reset(&self) {
        self.inner.lock().expect("clock lock poisoned").reset();
    }
}

impl Default for SharedClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn initial_time() {
        let clock = DeterministicClock::new(1_000);
        assert_eq!(clock.now_us(), 1_000);
    }

    #[test]
    fn zero_constructor() {
        let clock = DeterministicClock::zero();
        assert_eq!(clock.now_us(), 0);
    }

    #[test]
    fn advance_microseconds() {
        let mut clock = DeterministicClock::new(0);
        clock.advance(500);
        assert_eq!(clock.now_us(), 500);
        clock.advance(1_500);
        assert_eq!(clock.now_us(), 2_000);
    }

    #[test]
    fn advance_milliseconds() {
        let mut clock = DeterministicClock::new(0);
        clock.advance_ms(4);
        assert_eq!(clock.now_us(), 4_000);
    }

    #[test]
    fn advance_ticks_250hz() {
        let mut clock = DeterministicClock::new(0);
        clock.advance_ticks(1);
        assert_eq!(clock.now_us(), 4_000);
        clock.advance_ticks(250);
        // 1 second = 250 ticks * 4000 µs
        assert_eq!(clock.now_us(), 4_000 + 1_000_000);
    }

    #[test]
    fn reset_zeroes_clock() {
        let mut clock = DeterministicClock::new(50_000);
        clock.advance(10_000);
        clock.reset();
        assert_eq!(clock.now_us(), 0);
    }

    #[test]
    fn now_returns_duration() {
        let mut clock = DeterministicClock::zero();
        clock.advance_ms(100);
        assert_eq!(clock.now(), Duration::from_millis(100));
    }

    #[test]
    fn advance_duration_works() {
        let mut clock = DeterministicClock::zero();
        clock.advance_duration(Duration::from_millis(50));
        assert_eq!(clock.now_us(), 50_000);
        clock.advance_duration(Duration::from_micros(500));
        assert_eq!(clock.now_us(), 50_500);
    }

    #[test]
    fn time_ordering_preserved() {
        let mut clock = DeterministicClock::zero();
        let t0 = clock.now();
        clock.advance(1);
        let t1 = clock.now();
        clock.advance(1);
        let t2 = clock.now();
        assert!(t0 < t1);
        assert!(t1 < t2);
    }

    // --- SharedClock tests ---

    #[test]
    fn shared_clock_basic() {
        let clock = SharedClock::new();
        assert_eq!(clock.now_us(), 0);
        clock.advance(1000);
        assert_eq!(clock.now_us(), 1000);
    }

    #[test]
    fn shared_clock_with_initial() {
        let clock = SharedClock::with_initial(5_000);
        assert_eq!(clock.now_us(), 5_000);
    }

    #[test]
    fn shared_clock_clones_share_time() {
        let clock1 = SharedClock::new();
        let clock2 = clock1.clone();
        clock1.advance(1000);
        assert_eq!(clock2.now_us(), 1000);
    }

    #[test]
    fn shared_clock_thread_safety() {
        let clock = SharedClock::new();
        let clock_clone = clock.clone();

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                clock_clone.advance(1);
            }
        });

        for _ in 0..100 {
            clock.advance(1);
        }

        handle.join().unwrap();
        assert_eq!(clock.now_us(), 200);
    }

    #[test]
    fn shared_clock_advance_ticks() {
        let clock = SharedClock::new();
        clock.advance_ticks(10);
        assert_eq!(clock.now_us(), 40_000);
    }

    #[test]
    fn shared_clock_advance_duration() {
        let clock = SharedClock::new();
        clock.advance_duration(Duration::from_secs(1));
        assert_eq!(clock.now_us(), 1_000_000);
        assert_eq!(clock.now(), Duration::from_secs(1));
    }

    #[test]
    fn shared_clock_reset() {
        let clock = SharedClock::new();
        clock.advance(10_000);
        clock.reset();
        assert_eq!(clock.now_us(), 0);
    }

    #[test]
    fn shared_clock_default() {
        let clock = SharedClock::default();
        assert_eq!(clock.now_us(), 0);
    }
}
