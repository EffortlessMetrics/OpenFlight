//! Real-time scheduler for Flight Hub
//!
//! Provides precise timing for the 250Hz axis processing loop with
//! platform-specific optimizations for Windows and Linux.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::time::{Duration, Instant};

/// Real-time scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Target frequency in Hz
    pub frequency_hz: u32,
    /// Busy-spin tail duration in microseconds
    pub busy_spin_us: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 250,
            busy_spin_us: 65, // 50-80μs range
        }
    }
}

/// Real-time scheduler
pub struct Scheduler {
    config: SchedulerConfig,
    period_ns: u64,
    next_tick: Instant,
}

impl Scheduler {
    /// Create new scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        let period_ns = 1_000_000_000 / config.frequency_hz as u64;

        Self {
            config,
            period_ns,
            next_tick: Instant::now(),
        }
    }

    /// Wait for next tick
    pub fn wait_for_tick(&mut self) -> Instant {
        let now = Instant::now();

        if now < self.next_tick {
            let sleep_duration = self.next_tick - now;
            let busy_spin_duration = Duration::from_micros(self.config.busy_spin_us as u64);

            if sleep_duration > busy_spin_duration {
                let sleep_time = sleep_duration - busy_spin_duration;
                self.platform_sleep(sleep_time);
            }

            // Busy-spin for precise timing
            while Instant::now() < self.next_tick {
                std::hint::spin_loop();
            }
        }

        let tick_time = Instant::now();
        self.next_tick += Duration::from_nanos(self.period_ns);

        tick_time
    }

    #[cfg(windows)]
    fn platform_sleep(&self, duration: Duration) {
        windows::platform_sleep(duration);
    }

    #[cfg(unix)]
    fn platform_sleep(&self, duration: Duration) {
        unix::platform_sleep(duration);
    }
}
