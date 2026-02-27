// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis value histogram for statistical analysis.
//!
//! Records the frequency distribution of axis values to help detect
//! dead zones, saturation zones, and unusual input patterns.

/// Number of buckets in the histogram.
pub const HISTOGRAM_BUCKETS: usize = 100;

/// Axis value histogram with fixed N buckets.
///
/// Uses a fixed-size `[u32; 100]` array so that [`record`](AxisHistogram::record)
/// is zero-allocation and safe to call on the RT thread.  Methods that return
/// [`String`] (e.g. [`to_ascii`](AxisHistogram::to_ascii)) allocate on the heap
/// and must **not** be called from the RT spine.
#[derive(Debug, Clone, Copy)]
pub struct AxisHistogram {
    /// Count per bucket. Bucket `i` covers `[-1 + i·(2/N), -1 + (i+1)·(2/N))`.
    counts: [u32; HISTOGRAM_BUCKETS],
    total: u64,
}

impl AxisHistogram {
    /// Create a new empty histogram.
    pub const fn new() -> Self {
        Self {
            counts: [0; HISTOGRAM_BUCKETS],
            total: 0,
        }
    }

    /// Record one sample value. The value is clamped to `[-1, 1]` before bucketing.
    ///
    /// Zero allocation — safe to call on the RT thread.
    #[inline]
    pub fn record(&mut self, value: f32) {
        let clamped = value.clamp(-1.0, 1.0);
        let normalized = (clamped + 1.0) / 2.0;
        let bucket = (normalized * HISTOGRAM_BUCKETS as f32) as usize;
        let bucket = bucket.min(HISTOGRAM_BUCKETS - 1);
        self.counts[bucket] += 1;
        self.total += 1;
    }

    /// Total number of samples recorded so far.
    pub fn total_samples(&self) -> u64 {
        self.total
    }

    /// Raw count for bucket `i`. Returns `0` for out-of-range indices.
    pub fn bucket_count(&self, i: usize) -> u32 {
        self.counts.get(i).copied().unwrap_or(0)
    }

    /// Relative frequency (0.0–1.0) for bucket `i`.
    ///
    /// Returns `0.0` when no samples have been recorded.
    pub fn bucket_frequency(&self, i: usize) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.counts.get(i).copied().unwrap_or(0) as f32 / self.total as f32
    }

    /// Returns the bucket index for a given value (clamped to `[-1, 1]`).
    pub fn bucket_for_value(value: f32) -> usize {
        let clamped = value.clamp(-1.0, 1.0);
        let normalized = (clamped + 1.0) / 2.0;
        let bucket = (normalized * HISTOGRAM_BUCKETS as f32) as usize;
        bucket.min(HISTOGRAM_BUCKETS - 1)
    }

    /// Value range `(low, high)` for bucket `i`.
    ///
    /// Bucket `0` starts at `-1.0`; bucket `N-1` ends at `1.0`.
    pub fn bucket_range(i: usize) -> (f32, f32) {
        let step = 2.0 / HISTOGRAM_BUCKETS as f32;
        let low = -1.0 + i as f32 * step;
        let high = low + step;
        (low, high)
    }

    /// Reset all counts and totals to zero.
    pub fn reset(&mut self) {
        self.counts = [0; HISTOGRAM_BUCKETS];
        self.total = 0;
    }

    /// Index of the most-populated bucket, or `None` if no samples have been recorded.
    pub fn peak_bucket(&self) -> Option<usize> {
        if self.total == 0 {
            return None;
        }
        self.counts
            .iter()
            .enumerate()
            .max_by_key(|&(_, &c)| c)
            .map(|(i, _)| i)
    }

    /// Percentage of samples whose value fell in the centre dead zone (`|value| < 0.05`).
    pub fn center_deadzone_percent(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let mut count: u64 = 0;
        for i in 0..HISTOGRAM_BUCKETS {
            let (low, high) = Self::bucket_range(i);
            if low < 0.05 && high > -0.05 {
                count += self.counts[i] as u64;
            }
        }
        count as f32 / self.total as f32 * 100.0
    }

    /// Percentage of samples where the axis was saturated (`|value| > 0.95`).
    pub fn saturation_percent(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let mut count: u64 = 0;
        for i in 0..HISTOGRAM_BUCKETS {
            let (low, high) = Self::bucket_range(i);
            if high > 0.95 || low < -0.95 {
                count += self.counts[i] as u64;
            }
        }
        count as f32 / self.total as f32 * 100.0
    }

    /// Generate a multi-line ASCII bar chart showing 20 representative buckets
    /// (every 5th bucket).
    ///
    /// Each line has the form:
    /// ```text
    /// [-1.00,-0.90]: |########................................| 42 (4.2%)
    /// ```
    ///
    /// **Note:** This method allocates heap memory and must not be called from the RT thread.
    pub fn to_ascii(&self) -> String {
        const BAR_WIDTH: usize = 40;
        let max_count = self.counts.iter().copied().max().unwrap_or(0);
        let mut out = String::new();

        for step in 0..20usize {
            let i = step * 5;
            let (low, high) = Self::bucket_range(i);
            let count = self.counts[i];
            let freq = self.bucket_frequency(i);
            let filled = if max_count > 0 {
                (count as f32 / max_count as f32 * BAR_WIDTH as f32) as usize
            } else {
                0
            };
            let bar: String = (0..BAR_WIDTH)
                .map(|j| if j < filled { '#' } else { '.' })
                .collect();
            out.push_str(&format!(
                "[{:.2},{:.2}]: |{}| {} ({:.1}%)\n",
                low,
                high,
                bar,
                count,
                freq * 100.0
            ));
        }
        out
    }
}

impl Default for AxisHistogram {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_histogram_empty() {
        let h = AxisHistogram::new();
        assert_eq!(h.total_samples(), 0);
        for i in 0..HISTOGRAM_BUCKETS {
            assert_eq!(h.bucket_count(i), 0);
        }
    }

    #[test]
    fn test_record_center_value() {
        let mut h = AxisHistogram::new();
        h.record(0.0);
        // 0.0 → normalized 0.5 → bucket 50
        assert_eq!(h.bucket_count(50), 1);
        assert_eq!(h.total_samples(), 1);
    }

    #[test]
    fn test_record_max_value() {
        let mut h = AxisHistogram::new();
        h.record(1.0);
        assert_eq!(h.bucket_count(HISTOGRAM_BUCKETS - 1), 1);
        assert_eq!(h.total_samples(), 1);
    }

    #[test]
    fn test_record_min_value() {
        let mut h = AxisHistogram::new();
        h.record(-1.0);
        assert_eq!(h.bucket_count(0), 1);
        assert_eq!(h.total_samples(), 1);
    }

    #[test]
    fn test_total_samples_increments() {
        let mut h = AxisHistogram::new();
        h.record(0.0);
        h.record(0.5);
        h.record(-0.5);
        assert_eq!(h.total_samples(), 3);
    }

    #[test]
    fn test_frequency_sums_to_one() {
        let mut h = AxisHistogram::new();
        for i in 0..50 {
            // Values -1.0, -0.98, ..., -0.02 — one per bucket
            h.record(i as f32 * 0.02 - 1.0);
        }
        let sum: f32 = (0..HISTOGRAM_BUCKETS).map(|i| h.bucket_frequency(i)).sum();
        assert!(
            (sum - 1.0).abs() < 1e-3,
            "sum of frequencies was {sum}, expected ~1.0"
        );
    }

    #[test]
    fn test_reset_clears_all() {
        let mut h = AxisHistogram::new();
        h.record(0.0);
        h.record(1.0);
        h.reset();
        assert_eq!(h.total_samples(), 0);
        for i in 0..HISTOGRAM_BUCKETS {
            assert_eq!(
                h.bucket_count(i),
                0,
                "bucket {i} should be zero after reset"
            );
        }
    }

    #[test]
    fn test_peak_bucket_finds_most_active() {
        let mut h = AxisHistogram::new();
        for _ in 0..10 {
            h.record(0.0); // all go to bucket 50
        }
        h.record(0.5); // one sample in a different bucket
        assert_eq!(h.peak_bucket(), Some(50));
    }

    #[test]
    fn test_center_deadzone_percent() {
        let mut h = AxisHistogram::new();
        for _ in 0..100 {
            h.record(0.0); // well inside |value| < 0.05
        }
        let pct = h.center_deadzone_percent();
        assert!((pct - 100.0).abs() < 0.01, "expected ~100%, got {pct}");
    }

    #[test]
    fn test_saturation_percent() {
        let mut h = AxisHistogram::new();
        for _ in 0..100 {
            h.record(1.0); // |value| > 0.95
        }
        let pct = h.saturation_percent();
        assert!((pct - 100.0).abs() < 0.01, "expected ~100%, got {pct}");
    }

    #[test]
    fn test_out_of_range_clamped() {
        let mut h = AxisHistogram::new();
        h.record(1.5);
        h.record(-1.5);
        assert_eq!(
            h.bucket_count(HISTOGRAM_BUCKETS - 1),
            1,
            "1.5 should be clamped to last bucket"
        );
        assert_eq!(
            h.bucket_count(0),
            1,
            "-1.5 should be clamped to first bucket"
        );
        assert_eq!(h.total_samples(), 2);
    }

    #[test]
    fn test_bucket_range_coverage() {
        let (low0, _) = AxisHistogram::bucket_range(0);
        let (_, high_last) = AxisHistogram::bucket_range(HISTOGRAM_BUCKETS - 1);
        assert!(
            (low0 - (-1.0)).abs() < 1e-6,
            "first bucket should start at -1.0, got {low0}"
        );
        assert!(
            (high_last - 1.0).abs() < 1e-6,
            "last bucket should end at 1.0, got {high_last}"
        );
    }

    #[test]
    fn test_peak_bucket_empty_histogram() {
        let h = AxisHistogram::new();
        assert_eq!(h.peak_bucket(), None);
    }

    #[test]
    fn test_bucket_frequency_empty() {
        let h = AxisHistogram::new();
        assert_eq!(h.bucket_frequency(50), 0.0);
    }
}
