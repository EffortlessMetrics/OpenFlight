#![cfg_attr(
    test,
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_parens,
        dead_code
    )
)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Real-time scheduler for Flight Hub
//!
//! Provides precise timing for the 250Hz axis processing loop with
//! platform-specific optimizations for Windows and Linux.
//!
//! Features:
//! - ABS (Absolute) scheduling with PLL phase correction
//! - Bounded SPSC rings with drop-tail policy
//! - Jitter measurement and monitoring
//! - Platform-specific high-precision timing

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub mod metrics;
pub mod pll;
pub mod ring;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub use metrics::{JitterMetrics, TimingStats};
pub use pll::Pll;
pub use ring::{RingStats, SpscRing};

#[cfg(test)]
mod tests;

/// Real-time scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Target frequency in Hz
    pub frequency_hz: u32,
    /// Busy-spin tail duration in microseconds (50-80μs recommended)
    pub busy_spin_us: u32,
    /// PLL gain for phase correction (0.001 = 0.1%/s)
    pub pll_gain: f64,
    /// Enable jitter measurement
    pub measure_jitter: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 250,
            busy_spin_us: 65, // 50-80μs range
            pll_gain: 0.001,  // 0.1%/s phase correction
            measure_jitter: true,
        }
    }
}

/// Real-time scheduler with PLL and jitter measurement
pub struct Scheduler {
    config: SchedulerConfig,
    period_ns: u64,
    next_tick: Instant,
    pll: Pll,
    metrics: Option<JitterMetrics>,
    tick_count: AtomicU64,
    missed_ticks: AtomicU64,
}

impl Scheduler {
    /// Create new scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        let period_ns = 1_000_000_000 / config.frequency_hz as u64;
        let pll = Pll::new(config.pll_gain, period_ns as f64);
        let metrics = if config.measure_jitter {
            Some(JitterMetrics::new(config.frequency_hz))
        } else {
            None
        };

        Self {
            config,
            period_ns,
            next_tick: Instant::now(),
            pll,
            metrics,
            tick_count: AtomicU64::new(0),
            missed_ticks: AtomicU64::new(0),
        }
    }

    /// Wait for next tick with PLL correction
    pub fn wait_for_tick(&mut self) -> TickResult {
        let tick_start = Instant::now();
        let tick_number = self.tick_count.fetch_add(1, Ordering::Relaxed);

        // Check if we missed the deadline
        let missed = tick_start >= self.next_tick + Duration::from_nanos(self.period_ns / 2);
        if missed {
            self.missed_ticks.fetch_add(1, Ordering::Relaxed);
        }

        // Sleep until close to target time
        if tick_start < self.next_tick {
            let sleep_duration = self.next_tick - tick_start;
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

        let actual_tick = Instant::now();

        // Update PLL with timing error
        let error_ns = if actual_tick >= self.next_tick {
            (actual_tick - self.next_tick).as_nanos() as i64
        } else {
            -((self.next_tick - actual_tick).as_nanos() as i64)
        };

        let corrected_period = self.pll.update(error_ns as f64);

        // Schedule next tick with PLL correction
        self.next_tick += Duration::from_nanos(corrected_period as u64);

        // Update jitter metrics
        if let Some(ref mut metrics) = self.metrics {
            metrics.record_tick(actual_tick, error_ns);
        }

        TickResult {
            tick_number,
            timestamp: actual_tick,
            error_ns,
            missed,
        }
    }

    /// Get current timing statistics
    pub fn get_stats(&self) -> TimingStats {
        let total_ticks = self.tick_count.load(Ordering::Relaxed);
        let missed_ticks = self.missed_ticks.load(Ordering::Relaxed);

        let jitter_stats = self.metrics.as_ref().map(|m| m.get_stats());

        TimingStats {
            total_ticks,
            missed_ticks,
            miss_rate: if total_ticks > 0 {
                missed_ticks as f64 / total_ticks as f64
            } else {
                0.0
            },
            jitter_stats,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.tick_count.store(0, Ordering::Relaxed);
        self.missed_ticks.store(0, Ordering::Relaxed);
        if let Some(ref mut metrics) = self.metrics {
            metrics.reset();
        }
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

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Test PLL phase correction logic
        #[test]
        fn prop_pll_convergence(
            error_ns in -1_000_000i64..1_000_000i64, // +/- 1ms error
            base_period in 3_000_000f64..5_000_000f64 // 3-5ms period
        ) {
            let mut pll = Pll::new(0.001, base_period);
            let corrected = pll.update(error_ns as f64);

            // Correction should oppose the error
            if error_ns > 0 {
                // If we are late (positive error), period should decrease to catch up
                prop_assert!(corrected < base_period);
            } else if error_ns < 0 {
                // If we are early (negative error), period should increase to slow down
                prop_assert!(corrected > base_period);
            } else {
                prop_assert!((corrected - base_period).abs() < 1e-9);
            }

            // Correction should be bounded (prevent extreme swings)
            let max_change = base_period * 0.1; // 10% max change usually
            prop_assert!((corrected - base_period).abs() <= max_change * 2.0); // Rough check
        }

        // Test SchedulerConfig validity
        #[test]
        fn prop_scheduler_config_validity(
            frequency_hz in 1u32..1000,
            busy_spin_us in 0u32..1000
        ) {
            let config = SchedulerConfig {
                frequency_hz,
                busy_spin_us,
                pll_gain: 0.001,
                measure_jitter: true,
            };

            let scheduler = Scheduler::new(config);
            // Just verifying construction doesn't panic and values are derived correctly
            prop_assert!(scheduler.period_ns > 0);
        }
    }
}

/// Result of a scheduler tick
#[derive(Debug, Clone)]
pub struct TickResult {
    /// Tick sequence number
    pub tick_number: u64,
    /// Actual timestamp when tick occurred
    pub timestamp: Instant,
    /// Timing error in nanoseconds (positive = late, negative = early)
    pub error_ns: i64,
    /// Whether this tick was considered missed (>1.5x period late)
    pub missed: bool,
}
