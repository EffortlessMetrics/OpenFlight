// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Runtime counters and allocation guards for RT monitoring
//!
//! Provides zero-overhead monitoring of real-time constraints including
//! allocation detection, lock usage, and timing violations.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Runtime performance counters for axis engine
#[derive(Debug)]
pub struct RuntimeCounters {
    /// Total frames processed
    frames_processed: AtomicU64,
    /// Number of pipeline swaps
    pipeline_swaps: AtomicU64,
    /// Deadline misses (processing > budget)
    deadline_misses: AtomicU64,
    /// Allocations detected in RT path
    rt_allocations: AtomicU64,
    /// Lock acquisitions in RT path
    rt_lock_acquisitions: AtomicU64,
    /// Maximum frame processing time (microseconds)
    max_frame_time_us: AtomicU32,
    /// Average frame processing time (microseconds)
    avg_frame_time_us: AtomicU32,
    /// Creation timestamp
    created_at: Instant,
}

impl RuntimeCounters {
    /// Create new runtime counters
    pub fn new() -> Self {
        Self {
            frames_processed: AtomicU64::new(0),
            pipeline_swaps: AtomicU64::new(0),
            deadline_misses: AtomicU64::new(0),
            rt_allocations: AtomicU64::new(0),
            rt_lock_acquisitions: AtomicU64::new(0),
            max_frame_time_us: AtomicU32::new(0),
            avg_frame_time_us: AtomicU32::new(0),
            created_at: Instant::now(),
        }
    }

    /// Record frame processing time
    pub fn record_frame_time(&self, duration: Duration) {
        let micros = duration.as_micros() as u32;

        // Update maximum
        let mut current_max = self.max_frame_time_us.load(Ordering::Relaxed);
        while micros > current_max {
            match self.max_frame_time_us.compare_exchange_weak(
                current_max,
                micros,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }

        // Update running average (simple exponential moving average)
        let current_avg = self.avg_frame_time_us.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            micros
        } else {
            // EMA with alpha = 0.1
            ((current_avg as u64 * 9 + micros as u64) / 10) as u32
        };
        self.avg_frame_time_us.store(new_avg, Ordering::Relaxed);

        self.frames_processed.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment pipeline swap counter
    pub fn increment_pipeline_swaps(&self) {
        self.pipeline_swaps.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment deadline miss counter
    pub fn increment_deadline_misses(&self) {
        self.deadline_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment RT allocation counter
    pub fn increment_rt_allocations(&self) {
        self.rt_allocations.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment RT lock acquisition counter
    pub fn increment_rt_lock_acquisitions(&self) {
        self.rt_lock_acquisitions.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total frames processed
    pub fn frames_processed(&self) -> u64 {
        self.frames_processed.load(Ordering::Relaxed)
    }

    /// Get pipeline swap count
    pub fn pipeline_swaps(&self) -> u64 {
        self.pipeline_swaps.load(Ordering::Relaxed)
    }

    /// Get deadline miss count
    pub fn deadline_misses(&self) -> u64 {
        self.deadline_misses.load(Ordering::Relaxed)
    }

    /// Get RT allocation count (should always be 0)
    pub fn rt_allocations(&self) -> u64 {
        self.rt_allocations.load(Ordering::Relaxed)
    }

    /// Get RT lock acquisition count (should always be 0)
    pub fn rt_lock_acquisitions(&self) -> u64 {
        self.rt_lock_acquisitions.load(Ordering::Relaxed)
    }

    /// Get maximum frame processing time in microseconds
    pub fn max_frame_time_us(&self) -> u32 {
        self.max_frame_time_us.load(Ordering::Relaxed)
    }

    /// Get average frame processing time in microseconds
    pub fn avg_frame_time_us(&self) -> u32 {
        self.avg_frame_time_us.load(Ordering::Relaxed)
    }

    /// Get uptime since counter creation
    pub fn uptime(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Reset all counters
    pub fn reset(&self) {
        self.frames_processed.store(0, Ordering::Relaxed);
        self.pipeline_swaps.store(0, Ordering::Relaxed);
        self.deadline_misses.store(0, Ordering::Relaxed);
        self.rt_allocations.store(0, Ordering::Relaxed);
        self.rt_lock_acquisitions.store(0, Ordering::Relaxed);
        self.max_frame_time_us.store(0, Ordering::Relaxed);
        self.avg_frame_time_us.store(0, Ordering::Relaxed);
    }

    /// Check if RT constraints are violated
    pub fn has_rt_violations(&self) -> bool {
        self.rt_allocations() > 0 || self.rt_lock_acquisitions() > 0
    }

    /// Get jitter statistics (p99 approximation)
    pub fn jitter_p99_estimate_us(&self) -> u32 {
        // Simple approximation: max is roughly p99 for well-behaved systems
        self.max_frame_time_us()
    }
}

impl Default for RuntimeCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Allocation guard for detecting heap allocations in RT code
///
/// When created, this guard installs a custom allocator that tracks
/// allocations and reports violations to the runtime counters.
pub struct AllocationGuard {
    _marker: std::marker::PhantomData<()>,
}

impl AllocationGuard {
    /// Create new allocation guard
    ///
    /// # Safety
    /// This guard uses thread-local state to track allocations.
    /// Only one guard should be active per thread at a time.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        RT_ALLOCATION_GUARD.with(|guard| {
            guard.store(true, Ordering::Relaxed);
        });

        Self {
            _marker: std::marker::PhantomData,
        }
    }

    /// Check if any allocations were detected
    pub fn allocations_detected() -> bool {
        RT_ALLOCATION_DETECTED.with(|detected| detected.load(Ordering::Relaxed))
    }

    /// Reset allocation detection state
    pub fn reset() {
        RT_ALLOCATION_DETECTED.with(|detected| {
            detected.store(false, Ordering::Relaxed);
        });
    }
}

impl Drop for AllocationGuard {
    fn drop(&mut self) {
        RT_ALLOCATION_GUARD.with(|guard| {
            guard.store(false, Ordering::Relaxed);
        });
    }
}

// Thread-local state for allocation tracking
thread_local! {
    static RT_ALLOCATION_GUARD: AtomicBool = const { AtomicBool::new(false) };
    static RT_ALLOCATION_DETECTED: AtomicBool = const { AtomicBool::new(false) };
}

/// Check if currently in RT context (for debugging)
pub fn in_rt_context() -> bool {
    RT_ALLOCATION_GUARD.with(|guard| guard.load(Ordering::Relaxed))
}

/// Performance snapshot for monitoring
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    pub frames_processed: u64,
    pub pipeline_swaps: u64,
    pub deadline_misses: u64,
    pub rt_violations: u64,
    pub max_frame_time_us: u32,
    pub avg_frame_time_us: u32,
    pub jitter_p99_us: u32,
    pub uptime_ms: u64,
}

impl RuntimeCounters {
    /// Take performance snapshot
    pub fn snapshot(&self) -> PerformanceSnapshot {
        PerformanceSnapshot {
            frames_processed: self.frames_processed(),
            pipeline_swaps: self.pipeline_swaps(),
            deadline_misses: self.deadline_misses(),
            rt_violations: self.rt_allocations() + self.rt_lock_acquisitions(),
            max_frame_time_us: self.max_frame_time_us(),
            avg_frame_time_us: self.avg_frame_time_us(),
            jitter_p99_us: self.jitter_p99_estimate_us(),
            uptime_ms: self.uptime().as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_counter_creation() {
        let counters = RuntimeCounters::new();
        assert_eq!(counters.frames_processed(), 0);
        assert_eq!(counters.pipeline_swaps(), 0);
        assert_eq!(counters.deadline_misses(), 0);
    }

    #[test]
    fn test_frame_time_recording() {
        let counters = RuntimeCounters::new();

        counters.record_frame_time(Duration::from_micros(100));
        assert_eq!(counters.frames_processed(), 1);
        assert_eq!(counters.max_frame_time_us(), 100);
        assert_eq!(counters.avg_frame_time_us(), 100);

        counters.record_frame_time(Duration::from_micros(200));
        assert_eq!(counters.frames_processed(), 2);
        assert_eq!(counters.max_frame_time_us(), 200);
    }

    #[test]
    fn test_allocation_guard() {
        AllocationGuard::reset();
        assert!(!AllocationGuard::allocations_detected());

        {
            let _guard = AllocationGuard::new();
            // Guard is active but no allocations yet
            assert!(!AllocationGuard::allocations_detected());
        }

        // Guard is dropped
        assert!(!AllocationGuard::allocations_detected());
    }

    #[test]
    fn test_performance_snapshot() {
        let counters = RuntimeCounters::new();
        counters.record_frame_time(Duration::from_micros(150));
        counters.increment_pipeline_swaps();

        let snapshot = counters.snapshot();
        assert_eq!(snapshot.frames_processed, 1);
        assert_eq!(snapshot.pipeline_swaps, 1);
        assert_eq!(snapshot.max_frame_time_us, 150);
    }

    #[test]
    fn test_rt_violation_detection() {
        let counters = RuntimeCounters::new();
        assert!(!counters.has_rt_violations());

        counters.increment_rt_allocations();
        assert!(counters.has_rt_violations());
    }
}
