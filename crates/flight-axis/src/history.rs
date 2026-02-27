// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis value history ring buffer.
//!
//! Records the last N axis values with timestamps for latency analysis
//! and replay. All memory is allocated at construction time.

/// A single history sample.
#[derive(Debug, Clone, Copy, Default)]
pub struct HistorySample {
    /// Axis value in [-1.0, 1.0].
    pub value: f32,
    /// Tick counter when this sample was recorded.
    pub tick: u64,
}

/// Fixed-capacity circular history buffer for axis values.
///
/// Uses a const generic for capacity so buffer memory is on the stack
/// or in a static, never on the heap (zero allocation on update).
#[derive(Debug, Clone)]
pub struct AxisHistory<const N: usize> {
    samples: [HistorySample; N],
    head: usize,
    count: usize,
    total_recorded: u64,
}

impl<const N: usize> AxisHistory<N> {
    pub const CAPACITY: usize = N;

    pub const fn new() -> Self {
        Self {
            samples: [HistorySample {
                value: 0.0,
                tick: 0,
            }; N],
            head: 0,
            count: 0,
            total_recorded: 0,
        }
    }

    /// Record a new sample. Overwrites oldest sample if full.
    pub fn push(&mut self, value: f32, tick: u64) {
        self.samples[self.head] = HistorySample { value, tick };
        self.head = (self.head + 1) % N;
        if self.count < N {
            self.count += 1;
        }
        self.total_recorded += 1;
    }

    /// Current number of recorded samples (≤ N).
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn is_full(&self) -> bool {
        self.count == N
    }

    pub fn total_recorded(&self) -> u64 {
        self.total_recorded
    }

    /// Get the most recent sample.
    pub fn latest(&self) -> Option<HistorySample> {
        if self.count == 0 {
            return None;
        }
        let idx = if self.head == 0 { N - 1 } else { self.head - 1 };
        Some(self.samples[idx])
    }

    /// Iterate samples from oldest to newest.
    pub fn iter_oldest_first(&self) -> impl Iterator<Item = &HistorySample> {
        let start = if self.count == N {
            self.head // head points to oldest when full
        } else {
            0
        };
        (0..self.count).map(move |i| &self.samples[(start + i) % N])
    }

    /// Take a snapshot of all samples as a Vec (for non-RT analysis).
    pub fn snapshot(&self) -> Vec<HistorySample> {
        self.iter_oldest_first().copied().collect()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.samples = [HistorySample {
            value: 0.0,
            tick: 0,
        }; N];
        self.head = 0;
        self.count = 0;
    }

    /// Compute min, max, mean of recorded values.
    pub fn stats(&self) -> Option<HistoryStats> {
        if self.count == 0 {
            return None;
        }
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        let mut sum = 0.0_f32;
        for s in self.iter_oldest_first() {
            if s.value < min {
                min = s.value;
            }
            if s.value > max {
                max = s.value;
            }
            sum += s.value;
        }
        Some(HistoryStats {
            min,
            max,
            mean: sum / self.count as f32,
            count: self.count,
        })
    }
}

impl<const N: usize> Default for AxisHistory<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistical summary of history buffer.
#[derive(Debug, Clone, Copy)]
pub struct HistoryStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub count: usize,
}

/// Type alias for a standard 256-sample history buffer.
pub type AxisHistory256 = AxisHistory<256>;
/// Type alias for a 64-sample history buffer (low memory).
pub type AxisHistory64 = AxisHistory<64>;
/// Type alias for a 1024-sample history buffer (high detail).
pub type AxisHistory1024 = AxisHistory<1024>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_history_empty() {
        let h = AxisHistory::<8>::new();
        assert_eq!(h.len(), 0);
        assert!(h.is_empty());
        assert!(h.latest().is_none());
    }

    #[test]
    fn test_push_increments_len() {
        let mut h = AxisHistory::<8>::new();
        h.push(0.1, 1);
        assert_eq!(h.len(), 1);
        h.push(0.2, 2);
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_push_beyond_capacity_wraps() {
        let mut h = AxisHistory::<4>::new();
        for i in 0..6 {
            h.push(i as f32 * 0.1, i as u64);
        }
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn test_latest_returns_most_recent() {
        let mut h = AxisHistory::<8>::new();
        h.push(0.1, 1);
        h.push(0.5, 2);
        h.push(0.9, 3);
        let latest = h.latest().unwrap();
        assert_eq!(latest.tick, 3);
        assert!((latest.value - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_total_recorded_exceeds_capacity() {
        let mut h = AxisHistory::<4>::new();
        for i in 0..10 {
            h.push(0.0, i as u64);
        }
        assert_eq!(h.total_recorded(), 10);
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn test_iter_oldest_first_order() {
        let mut h = AxisHistory::<8>::new();
        h.push(0.1, 10);
        h.push(0.2, 20);
        h.push(0.3, 30);
        let ticks: Vec<u64> = h.iter_oldest_first().map(|s| s.tick).collect();
        assert_eq!(ticks, vec![10, 20, 30]);
    }

    #[test]
    fn test_snapshot_returns_vec() {
        let mut h = AxisHistory::<8>::new();
        h.push(0.1, 1);
        h.push(0.2, 2);
        let snap = h.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].tick, 1);
        assert_eq!(snap[1].tick, 2);
    }

    #[test]
    fn test_clear_resets_buffer() {
        let mut h = AxisHistory::<8>::new();
        h.push(0.5, 1);
        h.push(0.6, 2);
        h.clear();
        assert_eq!(h.len(), 0);
        assert!(h.is_empty());
        assert!(h.latest().is_none());
    }

    #[test]
    fn test_stats_min_max_mean() {
        let mut h = AxisHistory::<8>::new();
        h.push(-1.0, 1);
        h.push(0.0, 2);
        h.push(1.0, 3);
        let stats = h.stats().unwrap();
        assert!((stats.min - (-1.0)).abs() < f32::EPSILON);
        assert!((stats.max - 1.0).abs() < f32::EPSILON);
        assert!((stats.mean - 0.0).abs() < f32::EPSILON);
        assert_eq!(stats.count, 3);
    }

    #[test]
    fn test_stats_none_when_empty() {
        let h = AxisHistory::<8>::new();
        assert!(h.stats().is_none());
    }

    #[test]
    fn test_is_full_when_at_capacity() {
        let mut h = AxisHistory::<4>::new();
        assert!(!h.is_full());
        for i in 0..4 {
            h.push(0.0, i as u64);
        }
        assert!(h.is_full());
    }

    #[test]
    fn test_history64_alias() {
        let mut h = AxisHistory64::new();
        assert_eq!(AxisHistory64::CAPACITY, 64);
        h.push(0.5, 1);
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_overwrite_order() {
        // Capacity of 3: push 4 values; the first (tick=1) should be gone.
        let mut h = AxisHistory::<3>::new();
        h.push(0.1, 1);
        h.push(0.2, 2);
        h.push(0.3, 3);
        h.push(0.4, 4);
        let ticks: Vec<u64> = h.iter_oldest_first().map(|s| s.tick).collect();
        // Oldest should now be tick=2, newest tick=4.
        assert_eq!(ticks, vec![2, 3, 4]);
        assert!(!ticks.contains(&1));
    }
}
