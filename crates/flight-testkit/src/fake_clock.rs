// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Thread-safe, manually-advanceable clock for deterministic testing.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Microseconds per tick at the 250 Hz RT spine rate.
const US_PER_TICK: u64 = 4_000;

/// A thread-safe, manually-advanceable clock.
///
/// Time only moves forward when you tell it to, making tests fully
/// deterministic regardless of wall-clock speed.
///
/// Internally backed by `Arc<AtomicU64>`, so clones share the same time.
#[derive(Debug, Clone)]
pub struct FakeClock {
    time_us: Arc<AtomicU64>,
}

impl FakeClock {
    /// Create a new clock starting at time zero.
    pub fn new() -> Self {
        Self {
            time_us: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create a new clock starting at `initial_us` microseconds.
    pub fn starting_at(initial_us: u64) -> Self {
        Self {
            time_us: Arc::new(AtomicU64::new(initial_us)),
        }
    }

    /// Current time in microseconds.
    pub fn now(&self) -> u64 {
        self.time_us.load(Ordering::Acquire)
    }

    /// Current time as a [`Duration`].
    pub fn now_duration(&self) -> Duration {
        Duration::from_micros(self.now())
    }

    /// Advance the clock by `duration`.
    pub fn advance(&self, duration: Duration) {
        self.time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Release);
    }

    /// Advance the clock by exactly one 250 Hz period (4 ms).
    pub fn tick(&self) {
        self.time_us.fetch_add(US_PER_TICK, Ordering::Release);
    }

    /// Advance the clock by `n` ticks (each tick = 4 ms at 250 Hz).
    pub fn tick_n(&self, n: u32) {
        self.time_us
            .fetch_add(u64::from(n) * US_PER_TICK, Ordering::Release);
    }

    /// Reset the clock to zero.
    pub fn reset(&self) {
        self.time_us.store(0, Ordering::Release);
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn starts_at_zero() {
        let clock = FakeClock::new();
        assert_eq!(clock.now(), 0);
    }

    #[test]
    fn starts_at_custom_time() {
        let clock = FakeClock::starting_at(10_000);
        assert_eq!(clock.now(), 10_000);
    }

    #[test]
    fn advance_by_duration() {
        let clock = FakeClock::new();
        clock.advance(Duration::from_millis(10));
        assert_eq!(clock.now(), 10_000);
        clock.advance(Duration::from_micros(500));
        assert_eq!(clock.now(), 10_500);
    }

    #[test]
    fn tick_advances_4ms() {
        let clock = FakeClock::new();
        clock.tick();
        assert_eq!(clock.now(), 4_000);
    }

    #[test]
    fn tick_n_advances_correctly() {
        let clock = FakeClock::new();
        clock.tick_n(250); // 1 second
        assert_eq!(clock.now(), 1_000_000);
    }

    #[test]
    fn now_duration_returns_duration() {
        let clock = FakeClock::starting_at(5_000);
        assert_eq!(clock.now_duration(), Duration::from_micros(5_000));
    }

    #[test]
    fn reset_returns_to_zero() {
        let clock = FakeClock::new();
        clock.tick_n(100);
        assert!(clock.now() > 0);
        clock.reset();
        assert_eq!(clock.now(), 0);
    }

    #[test]
    fn clones_share_time() {
        let a = FakeClock::new();
        let b = a.clone();
        a.tick();
        assert_eq!(b.now(), 4_000);
    }

    #[test]
    fn thread_safe_concurrent_advance() {
        let clock = FakeClock::new();
        let mut handles = Vec::new();
        for _ in 0..4 {
            let c = clock.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..250 {
                    c.tick();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // 4 threads × 250 ticks × 4000 µs = 4_000_000 µs
        assert_eq!(clock.now(), 4_000_000);
    }
}
