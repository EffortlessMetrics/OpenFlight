// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Thread-safe deterministic clock for testing timing-sensitive code.
//!
//! Advances only when explicitly told to, making tests fully reproducible.
//! Backed by `Arc<AtomicU64>` so it can be shared across threads.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Microseconds per tick at the 250 Hz RT spine rate.
const US_PER_TICK: u64 = 4_000;

/// A thread-safe deterministic clock.
///
/// Time only moves forward via explicit calls to [`advance`](Self::advance),
/// [`advance_ticks`](Self::advance_ticks), or [`set_time`](Self::set_time).
/// Cloning produces a handle that shares the same underlying time.
#[derive(Debug, Clone)]
pub struct DeterministicClock {
    time_us: Arc<AtomicU64>,
}

impl DeterministicClock {
    /// Create a new clock starting at zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            time_us: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a new clock starting at `initial` microseconds.
    #[must_use]
    pub fn with_initial(initial_us: u64) -> Self {
        Self {
            time_us: Arc::new(AtomicU64::new(initial_us)),
        }
    }

    /// Return the current time in microseconds.
    #[must_use]
    pub fn now_us(&self) -> u64 {
        self.time_us.load(Ordering::Acquire)
    }

    /// Return the current time as a [`Duration`].
    #[must_use]
    pub fn now(&self) -> Duration {
        Duration::from_micros(self.now_us())
    }

    /// Advance the clock by `duration`.
    pub fn advance(&self, duration: Duration) {
        self.time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Release);
    }

    /// Advance the clock by `us` microseconds.
    pub fn advance_us(&self, us: u64) {
        self.time_us.fetch_add(us, Ordering::Release);
    }

    /// Advance by `ticks` RT spine ticks (each tick = 4 000 µs at 250 Hz).
    pub fn advance_ticks(&self, ticks: u32) {
        self.time_us
            .fetch_add(u64::from(ticks) * US_PER_TICK, Ordering::Release);
    }

    /// Set the clock to an absolute time in microseconds.
    pub fn set_time(&self, time_us: u64) {
        self.time_us.store(time_us, Ordering::Release);
    }

    /// Set the clock to an absolute [`Duration`].
    pub fn set_time_duration(&self, time: Duration) {
        self.set_time(time.as_micros() as u64);
    }

    /// Reset the clock to zero.
    pub fn reset(&self) {
        self.time_us.store(0, Ordering::Release);
    }
}

impl Default for DeterministicClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn new_starts_at_zero() {
        let clock = DeterministicClock::new();
        assert_eq!(clock.now_us(), 0);
    }

    #[test]
    fn with_initial_starts_at_value() {
        let clock = DeterministicClock::with_initial(5_000);
        assert_eq!(clock.now_us(), 5_000);
    }

    #[test]
    fn advance_by_duration() {
        let clock = DeterministicClock::new();
        clock.advance(Duration::from_millis(10));
        assert_eq!(clock.now_us(), 10_000);
        assert_eq!(clock.now(), Duration::from_millis(10));
    }

    #[test]
    fn advance_us() {
        let clock = DeterministicClock::new();
        clock.advance_us(500);
        clock.advance_us(1_500);
        assert_eq!(clock.now_us(), 2_000);
    }

    #[test]
    fn advance_ticks_250hz() {
        let clock = DeterministicClock::new();
        clock.advance_ticks(1);
        assert_eq!(clock.now_us(), 4_000);
        clock.advance_ticks(250);
        assert_eq!(clock.now_us(), 4_000 + 1_000_000);
    }

    #[test]
    fn set_time_absolute() {
        let clock = DeterministicClock::new();
        clock.advance_us(1_000);
        clock.set_time(50_000);
        assert_eq!(clock.now_us(), 50_000);
    }

    #[test]
    fn set_time_duration() {
        let clock = DeterministicClock::new();
        clock.set_time_duration(Duration::from_secs(1));
        assert_eq!(clock.now_us(), 1_000_000);
    }

    #[test]
    fn reset_zeroes_clock() {
        let clock = DeterministicClock::with_initial(50_000);
        clock.advance_us(10_000);
        clock.reset();
        assert_eq!(clock.now_us(), 0);
    }

    #[test]
    fn clone_shares_state() {
        let clock = DeterministicClock::new();
        let clone = clock.clone();
        clock.advance_us(1_000);
        assert_eq!(clone.now_us(), 1_000);
    }

    #[test]
    fn thread_safe_concurrent_advance() {
        let clock = DeterministicClock::new();
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let c = clock.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        c.advance_us(1);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(clock.now_us(), 400);
    }

    #[test]
    fn default_starts_at_zero() {
        let clock = DeterministicClock::default();
        assert_eq!(clock.now_us(), 0);
    }
}
