// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Circular-buffer jitter tracker with exact p99 (ADR-004 zero-allocation).
//!
//! [`JitterTracker`] maintains running min/max/mean/stddev statistics and a
//! 256-entry circular buffer that enables **exact** p99 computation (unlike
//! the normal-approximation in [`crate::pll::JitterStats`]).
//!
//! The struct is [`Copy`] and fully stack-allocated — no heap allocation on
//! the hot path.

/// Size of the circular buffer used for exact percentile computation.
pub const RING_SIZE: usize = 256;

/// Circular-buffer jitter tracker with exact percentile support.
///
/// All ~2 KiB of state live on the stack (256 × 8 bytes for the ring plus
/// scalars). [`record`](Self::record) is O(1); [`p99_ns`](Self::p99_ns)
/// is O(n log n) where n ≤ 256 (sorts a stack copy of the ring).
///
/// # Zero-allocation guarantee
///
/// All fields are fixed-size scalars or inline arrays. No method performs
/// heap allocation.
#[derive(Debug, Clone, Copy)]
pub struct JitterTracker {
    min_ns: u64,
    max_ns: u64,
    sum_ns: u64,
    sum_sq_ns: u128,
    count: u64,
    recent: [u64; RING_SIZE],
    recent_idx: u8,
}

impl JitterTracker {
    /// Create an empty tracker.
    pub const fn new() -> Self {
        Self {
            min_ns: u64::MAX,
            max_ns: 0,
            sum_ns: 0,
            sum_sq_ns: 0,
            count: 0,
            recent: [0; RING_SIZE],
            recent_idx: 0,
        }
    }

    /// Record a jitter sample in nanoseconds (**hot path — zero allocation**).
    #[inline]
    pub fn record(&mut self, jitter_ns: u64) {
        self.count += 1;
        self.sum_ns += jitter_ns;
        self.sum_sq_ns += jitter_ns as u128 * jitter_ns as u128;
        if jitter_ns < self.min_ns {
            self.min_ns = jitter_ns;
        }
        if jitter_ns > self.max_ns {
            self.max_ns = jitter_ns;
        }
        self.recent[self.recent_idx as usize] = jitter_ns;
        self.recent_idx = self.recent_idx.wrapping_add(1);
    }

    /// Mean jitter in nanoseconds.
    pub fn mean_ns(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.sum_ns / self.count
        }
    }

    /// Population standard deviation in nanoseconds.
    pub fn stddev_ns(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let mean = self.sum_ns as f64 / self.count as f64;
        let mean_sq = self.sum_sq_ns as f64 / self.count as f64;
        let variance = mean_sq - mean * mean;
        if variance < 0.0 { 0.0 } else { variance.sqrt() }
    }

    /// Exact p99 from the circular buffer.
    ///
    /// Sorts a **stack-local copy** of the ring buffer (~2 KiB), so this is
    /// O(256 log 256) ≈ O(2048) with no heap allocation.
    pub fn p99_ns(&self) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let len = self.count.min(RING_SIZE as u64) as usize;
        let mut buf = self.recent;
        buf[..len].sort_unstable();
        let idx = (len * 99) / 100;
        buf[idx.min(len.saturating_sub(1))]
    }

    /// Maximum jitter observed (nanoseconds). Returns 0 if empty.
    pub fn max_ns(&self) -> u64 {
        if self.count == 0 { 0 } else { self.max_ns }
    }

    /// Minimum jitter observed (nanoseconds). Returns 0 if empty.
    pub fn min_ns(&self) -> u64 {
        if self.count == 0 { 0 } else { self.min_ns }
    }

    /// Number of samples recorded.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset all statistics and the circular buffer.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for JitterTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_tracker_empty_returns_zeros() {
        let t = JitterTracker::new();
        assert_eq!(t.count(), 0);
        assert_eq!(t.min_ns(), 0);
        assert_eq!(t.max_ns(), 0);
        assert_eq!(t.mean_ns(), 0);
        assert_eq!(t.p99_ns(), 0);
        assert!(t.stddev_ns().abs() < f64::EPSILON);
    }

    #[test]
    fn jitter_tracker_single_record() {
        let mut t = JitterTracker::new();
        t.record(5_000);
        assert_eq!(t.count(), 1);
        assert_eq!(t.min_ns(), 5_000);
        assert_eq!(t.max_ns(), 5_000);
        assert_eq!(t.mean_ns(), 5_000);
        assert_eq!(t.p99_ns(), 5_000);
    }

    #[test]
    fn jitter_tracker_known_mean() {
        let mut t = JitterTracker::new();
        t.record(100);
        t.record(200);
        t.record(300);
        assert_eq!(t.mean_ns(), 200);
        assert_eq!(t.min_ns(), 100);
        assert_eq!(t.max_ns(), 300);
    }

    #[test]
    fn jitter_tracker_known_stddev() {
        let mut t = JitterTracker::new();
        // Values: 2, 4, 4, 4, 5, 5, 7, 9 → mean=5, var=4, stddev=2
        for &v in &[2u64, 4, 4, 4, 5, 5, 7, 9] {
            t.record(v);
        }
        assert_eq!(t.mean_ns(), 5);
        assert!(
            (t.stddev_ns() - 2.0).abs() < 0.01,
            "expected stddev ~2.0, got {}",
            t.stddev_ns()
        );
    }

    #[test]
    fn jitter_tracker_p99_exact_from_sorted_buffer() {
        let mut t = JitterTracker::new();
        // Record 100 values: 1..=100
        for v in 1..=100u64 {
            t.record(v);
        }
        // (100 * 99) / 100 = 99 → buf[99] = 100
        let p99 = t.p99_ns();
        assert_eq!(p99, 100, "p99 of 1..=100 should be 100, got {p99}");
    }

    #[test]
    fn jitter_tracker_p99_larger_dataset() {
        let mut t = JitterTracker::new();
        for v in 0..256u64 {
            t.record(v);
        }
        // (256 * 99) / 100 = 253 → sorted[253] = 253
        let p99 = t.p99_ns();
        assert!(
            p99 >= 250 && p99 <= 255,
            "p99 of 0..255 should be ~253, got {p99}"
        );
    }

    #[test]
    fn jitter_tracker_circular_buffer_wraps() {
        let mut t = JitterTracker::new();
        // Fill buffer with low values
        for _ in 0..256 {
            t.record(10);
        }
        // Overwrite with high values
        for _ in 0..256 {
            t.record(1000);
        }
        // p99 should reflect the recent (high) values
        assert_eq!(t.p99_ns(), 1000);
        assert_eq!(t.count(), 512);
    }

    #[test]
    fn jitter_tracker_all_zeros() {
        let mut t = JitterTracker::new();
        for _ in 0..100 {
            t.record(0);
        }
        assert_eq!(t.mean_ns(), 0);
        assert_eq!(t.p99_ns(), 0);
        assert_eq!(t.min_ns(), 0);
        assert_eq!(t.max_ns(), 0);
        assert!(t.stddev_ns() < f64::EPSILON);
    }

    #[test]
    fn jitter_tracker_reset_clears_all() {
        let mut t = JitterTracker::new();
        for i in 0..100 {
            t.record(i * 1000);
        }
        assert!(t.count() > 0);
        t.reset();
        assert_eq!(t.count(), 0);
        assert_eq!(t.min_ns(), 0);
        assert_eq!(t.max_ns(), 0);
        assert_eq!(t.mean_ns(), 0);
        assert_eq!(t.p99_ns(), 0);
    }

    #[test]
    fn jitter_tracker_is_copy() {
        let mut t = JitterTracker::new();
        t.record(100);
        t.record(200);
        let t2 = t; // Copy
        let _ = t; // original still usable
        assert_eq!(t2.count(), t.count());
        assert_eq!(t2.mean_ns(), t.mean_ns());
    }

    #[test]
    fn jitter_tracker_max_ns_tracked() {
        let mut t = JitterTracker::new();
        t.record(10);
        t.record(50);
        t.record(30);
        assert_eq!(t.max_ns(), 50);
    }

    #[test]
    fn jitter_tracker_min_ns_tracked() {
        let mut t = JitterTracker::new();
        t.record(50);
        t.record(10);
        t.record(30);
        assert_eq!(t.min_ns(), 10);
    }
}
