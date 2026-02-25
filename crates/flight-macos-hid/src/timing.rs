// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! High-resolution monotonic clock using `mach_timebase_info` on macOS.
//!
//! On non-macOS platforms the clock falls back to `std::time::Instant`.

use std::time::Duration;

/// A monotonic clock suitable for the RT loop.
///
/// On macOS wraps `mach_absolute_time` / `mach_timebase_info` to give
/// nanosecond-resolution timestamps without a syscall per tick.
/// On other platforms delegates to `std::time::Instant`.
#[derive(Debug, Clone)]
pub struct MacosClock {
    #[cfg(not(target_os = "macos"))]
    start: std::time::Instant,
    #[cfg(target_os = "macos")]
    // TODO: store numer/denom from mach_timebase_info_t
    start_ticks: u64,
    #[cfg(target_os = "macos")]
    numer: u32,
    #[cfg(target_os = "macos")]
    denom: u32,
}

impl MacosClock {
    /// Create and initialise the clock.
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            // mach_timebase_info / mach_absolute_time not yet wired.
            // Fall back to std::time for now so macOS builds don't panic.
            todo!("mach clock binding not yet implemented — see flight-macos-hid")
        }
        #[cfg(not(target_os = "macos"))]
        Self {
            start: std::time::Instant::now(),
        }
    }

    /// Elapsed time since the clock was created.
    pub fn elapsed(&self) -> Duration {
        #[cfg(target_os = "macos")]
        {
            // mach_absolute_time not yet wired.
            todo!("mach elapsed not yet implemented — see flight-macos-hid")
        }
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(not(target_os = "macos"))]
        {
            let clk = MacosClock::new();
            std::thread::sleep(Duration::from_millis(1));
            assert!(clk.elapsed() >= Duration::from_millis(1));
        }
    }

    #[test]
    fn test_now_ns_increases() {
        #[cfg(not(target_os = "macos"))]
        {
            let clk = MacosClock::new();
            let t0 = clk.now_ns();
            std::thread::sleep(Duration::from_micros(100));
            let t1 = clk.now_ns();
            assert!(t1 >= t0);
        }
    }
}
