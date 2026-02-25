// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! High-resolution monotonic clock using `mach_timebase_info` on macOS.
//!
//! On non-macOS platforms the clock falls back to `std::time::Instant`.

use std::time::Duration;

/// A monotonic clock suitable for the RT loop.
///
/// On macOS this currently delegates to `std::time::Instant`.
/// When `mach_absolute_time` / `mach_timebase_info` bindings are wired,
/// this will switch to the zero-syscall mach clock path.
/// On other platforms delegates to `std::time::Instant`.
#[derive(Debug, Clone)]
pub struct MacosClock {
    start: std::time::Instant,
}

impl MacosClock {
    /// Create and initialise the clock.
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    /// Elapsed time since the clock was created.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Current timestamp in nanoseconds (relative to clock creation).
    pub fn now_ns(&self) -> u64 {
        self.elapsed().as_nanos() as u64
    }
}

impl Default for MacosClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elapsed_non_zero() {
        let clk = MacosClock::new();
        std::thread::sleep(Duration::from_millis(1));
        assert!(clk.elapsed() >= Duration::from_millis(1));
    }

    #[test]
    fn test_now_ns_increases() {
        let clk = MacosClock::new();
        let t0 = clk.now_ns();
        std::thread::sleep(Duration::from_micros(100));
        let t1 = clk.now_ns();
        assert!(t1 >= t0);
    }
}
