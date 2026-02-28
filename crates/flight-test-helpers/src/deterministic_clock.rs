// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! A deterministic clock for testing time-dependent code.

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

    /// Return the current time in microseconds.
    pub fn now_us(&self) -> u64 {
        self.current_time_us
    }

    /// Advance the clock by `us` microseconds.
    pub fn advance(&mut self, us: u64) {
        self.current_time_us += us;
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

#[cfg(test)]
mod tests {
    use super::DeterministicClock;

    #[test]
    fn initial_time() {
        let clock = DeterministicClock::new(1_000);
        assert_eq!(clock.now_us(), 1_000);
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
}
