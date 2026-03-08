// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fixed-bucket histogram with zero-allocation recording.
//!
//! [`FixedBucketHistogram`] uses pre-allocated atomic counters per bucket,
//! so [`observe`](FixedBucketHistogram::observe) never allocates on the heap.

use std::sync::atomic::{AtomicU64, Ordering};

/// Fixed-bucket histogram optimized for zero-allocation recording.
///
/// Bucket boundaries are set at construction time. Each [`observe`](Self::observe)
/// call performs only atomic increments — no heap allocation or locking.
pub struct FixedBucketHistogram {
    boundaries: Vec<f64>,
    /// Cumulative counts per bucket (one extra for +Inf).
    counts: Vec<AtomicU64>,
    total_count: AtomicU64,
    /// Sum stored as `f64` bits in an `AtomicU64`.
    sum: AtomicU64,
}

impl FixedBucketHistogram {
    /// Create a histogram with the given upper-bound bucket boundaries.
    ///
    /// Boundaries are sorted and deduplicated. An implicit `+Inf` bucket is
    /// always present.
    pub fn new(boundaries: &[f64]) -> Self {
        let mut sorted: Vec<f64> = boundaries
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted.dedup();

        let counts: Vec<AtomicU64> = (0..=sorted.len()).map(|_| AtomicU64::new(0)).collect();

        Self {
            boundaries: sorted,
            counts,
            total_count: AtomicU64::new(0),
            sum: AtomicU64::new(0.0f64.to_bits()),
        }
    }

    /// Record an observation. Non-finite values are silently dropped.
    ///
    /// Only performs atomic operations — no heap allocation.
    pub fn observe(&self, value: f64) {
        if !value.is_finite() {
            return;
        }
        self.total_count.fetch_add(1, Ordering::Relaxed);

        // Atomic f64 add via CAS loop.
        loop {
            let current = self.sum.load(Ordering::Relaxed);
            let new = f64::from_bits(current) + value;
            if self
                .sum
                .compare_exchange_weak(current, new.to_bits(), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        // Increment cumulative bucket counts.
        for (i, &boundary) in self.boundaries.iter().enumerate() {
            if value <= boundary {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        // +Inf bucket always incremented.
        self.counts[self.boundaries.len()].fetch_add(1, Ordering::Relaxed);
    }

    /// Estimate a percentile (0.0–1.0) using linear interpolation within
    /// bucket boundaries. Returns 0.0 for an empty histogram.
    pub fn percentile(&self, p: f64) -> f64 {
        let total = self.total_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let p = p.clamp(0.0, 1.0);
        let target = (p * total as f64).ceil().max(1.0) as u64;

        let mut prev_count = 0u64;
        let mut prev_bound = 0.0f64;

        for (i, &boundary) in self.boundaries.iter().enumerate() {
            let count = self.counts[i].load(Ordering::Relaxed);
            if count >= target {
                let bucket_span = count - prev_count;
                if bucket_span == 0 {
                    return boundary;
                }
                let rank_in_bucket = target - prev_count;
                let bucket_width = boundary - prev_bound;
                return prev_bound + bucket_width * (rank_in_bucket as f64 / bucket_span as f64);
            }
            prev_count = count;
            prev_bound = boundary;
        }

        prev_bound
    }

    /// Total number of observations recorded.
    pub fn count(&self) -> u64 {
        self.total_count.load(Ordering::Relaxed)
    }

    /// Sum of all observed values.
    pub fn sum(&self) -> f64 {
        f64::from_bits(self.sum.load(Ordering::Relaxed))
    }

    /// Return bucket upper bounds paired with their cumulative counts.
    ///
    /// The final entry has bound `f64::INFINITY` (the `+Inf` bucket).
    pub fn bucket_counts(&self) -> Vec<(f64, u64)> {
        let mut result = Vec::with_capacity(self.boundaries.len() + 1);
        for (i, &boundary) in self.boundaries.iter().enumerate() {
            result.push((boundary, self.counts[i].load(Ordering::Relaxed)));
        }
        result.push((
            f64::INFINITY,
            self.counts[self.boundaries.len()].load(Ordering::Relaxed),
        ));
        result
    }
}

/// Common latency bucket boundaries in milliseconds.
pub fn latency_buckets() -> Vec<f64> {
    vec![
        0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
    ]
}

/// Common data-size bucket boundaries in bytes.
pub fn size_buckets() -> Vec<f64> {
    vec![
        64.0, 256.0, 1024.0, 4096.0, 16384.0, 65536.0, 262144.0, 1048576.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_increments_count() {
        let h = FixedBucketHistogram::new(&[1.0, 5.0, 10.0]);
        h.observe(2.0);
        h.observe(7.0);
        assert_eq!(h.count(), 2);
    }

    #[test]
    fn observe_accumulates_sum() {
        let h = FixedBucketHistogram::new(&[10.0]);
        h.observe(3.0);
        h.observe(7.0);
        assert!((h.sum() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bucket_distribution() {
        let h = FixedBucketHistogram::new(&[1.0, 5.0, 10.0]);
        h.observe(0.5); // ≤1, ≤5, ≤10
        h.observe(3.0); // ≤5, ≤10
        h.observe(7.0); // ≤10
        h.observe(15.0); // only +Inf

        let buckets = h.bucket_counts();
        assert_eq!(buckets[0], (1.0, 1));
        assert_eq!(buckets[1], (5.0, 2));
        assert_eq!(buckets[2], (10.0, 3));
        assert_eq!(buckets[3].1, 4); // +Inf
    }

    #[test]
    fn percentile_empty_returns_zero() {
        let h = FixedBucketHistogram::new(&[1.0, 5.0, 10.0]);
        assert_eq!(h.percentile(0.5), 0.0);
    }

    #[test]
    fn percentile_estimation() {
        let h = FixedBucketHistogram::new(&[1.0, 5.0, 10.0]);
        for _ in 0..10 {
            h.observe(0.5);
        }
        let p50 = h.percentile(0.5);
        assert!(p50 <= 1.0, "p50={p50} should be ≤ 1.0");
        assert!(p50 > 0.0, "p50={p50} should be > 0.0");
    }

    #[test]
    fn percentile_monotonic() {
        let h = FixedBucketHistogram::new(&[1.0, 5.0, 10.0, 50.0]);
        for v in [0.5, 2.0, 7.0, 30.0] {
            h.observe(v);
        }
        let p25 = h.percentile(0.25);
        let p50 = h.percentile(0.50);
        let p75 = h.percentile(0.75);
        let p99 = h.percentile(0.99);
        assert!(p25 <= p50, "p25={p25} must be ≤ p50={p50}");
        assert!(p50 <= p75, "p50={p50} must be ≤ p75={p75}");
        assert!(p75 <= p99, "p75={p75} must be ≤ p99={p99}");
    }

    #[test]
    fn non_finite_values_ignored() {
        let h = FixedBucketHistogram::new(&[1.0]);
        h.observe(f64::NAN);
        h.observe(f64::INFINITY);
        h.observe(f64::NEG_INFINITY);
        assert_eq!(h.count(), 0);
        assert_eq!(h.sum(), 0.0);
    }

    #[test]
    fn latency_buckets_sorted() {
        let b = latency_buckets();
        for w in b.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn size_buckets_sorted() {
        let b = size_buckets();
        for w in b.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn empty_boundaries_still_has_inf_bucket() {
        let h = FixedBucketHistogram::new(&[]);
        h.observe(42.0);
        let buckets = h.bucket_counts();
        assert_eq!(buckets.len(), 1);
        assert!(buckets[0].0.is_infinite());
        assert_eq!(buckets[0].1, 1);
    }

    #[test]
    fn duplicate_boundaries_deduplicated() {
        let h = FixedBucketHistogram::new(&[5.0, 5.0, 10.0, 10.0]);
        let buckets = h.bucket_counts();
        // 2 unique boundaries + 1 Inf = 3
        assert_eq!(buckets.len(), 3);
    }
}
