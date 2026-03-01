// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Lightweight, allocation-free span timing recorder.
//!
//! [`SpanRecorder`] pre-allocates a fixed array of span tracking slots at
//! construction time and never allocates again.  Each span type is identified
//! by a `u16` name ID and has its own fixed-size ring buffer of recent
//! durations used for statistics (min/max/mean/p99).
//!
//! Thread-safety is achieved through atomics — no locks on the hot path.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// ── Configuration ─────────────────────────────────────────────────────────

/// Maximum distinct span types tracked by a single [`SpanRecorder`].
pub const MAX_SPAN_TYPES: usize = 64;

/// Ring buffer size per span type for percentile estimation.
pub const SAMPLES_PER_SPAN: usize = 256;

// ── SpanToken ─────────────────────────────────────────────────────────────

/// Opaque token returned by [`SpanRecorder::start_span`].
///
/// Dropping the token does **not** record the span — call
/// [`SpanRecorder::end_span`] explicitly.
#[derive(Debug)]
pub struct SpanToken {
    name_id: u16,
    start: Instant,
}

// ── SpanStats ─────────────────────────────────────────────────────────────

/// Computed statistics for a single span type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanStats {
    pub count: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub mean_ns: u64,
    pub p99_ns: u64,
}

// ── Per-slot state ────────────────────────────────────────────────────────

struct SpanSlot {
    /// Total number of recorded spans (monotonically increasing).
    count: AtomicU64,
    /// Running sum for mean calculation (may wrap, but only used with count).
    sum_ns: AtomicU64,
    /// Observed minimum duration.
    min_ns: AtomicU64,
    /// Observed maximum duration.
    max_ns: AtomicU64,
    /// Fixed-size ring buffer of recent durations.
    samples: Box<[AtomicU64; SAMPLES_PER_SPAN]>,
    /// Write cursor into `samples`.
    cursor: AtomicU64,
}

impl SpanSlot {
    fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            sum_ns: AtomicU64::new(0),
            min_ns: AtomicU64::new(u64::MAX),
            max_ns: AtomicU64::new(0),
            samples: Box::new(std::array::from_fn(|_| AtomicU64::new(0))),
            cursor: AtomicU64::new(0),
        }
    }

    fn record(&self, duration_ns: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum_ns.fetch_add(duration_ns, Ordering::Relaxed);

        // Update min (CAS loop).
        let mut current_min = self.min_ns.load(Ordering::Relaxed);
        while duration_ns < current_min {
            match self.min_ns.compare_exchange_weak(
                current_min,
                duration_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_min = actual,
            }
        }

        // Update max (CAS loop).
        let mut current_max = self.max_ns.load(Ordering::Relaxed);
        while duration_ns > current_max {
            match self.max_ns.compare_exchange_weak(
                current_max,
                duration_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }

        // Ring-buffer write.
        let idx = self.cursor.fetch_add(1, Ordering::Relaxed) as usize % SAMPLES_PER_SPAN;
        self.samples[idx].store(duration_ns, Ordering::Relaxed);
    }

    fn stats(&self) -> SpanStats {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return SpanStats {
                count: 0,
                min_ns: 0,
                max_ns: 0,
                mean_ns: 0,
                p99_ns: 0,
            };
        }

        let sum = self.sum_ns.load(Ordering::Relaxed);
        let min_raw = self.min_ns.load(Ordering::Relaxed);
        let max_ns = self.max_ns.load(Ordering::Relaxed);
        let min_ns = if min_raw == u64::MAX { 0 } else { min_raw };
        let mean_ns = sum / count;

        // Collect valid samples for p99 using a stack-allocated buffer.
        let sample_count = (count as usize).min(SAMPLES_PER_SPAN);
        let mut buf = [0u64; SAMPLES_PER_SPAN];
        // Read the most recent `sample_count` entries.
        let cursor = self.cursor.load(Ordering::Relaxed) as usize;
        for (i, slot) in buf[..sample_count].iter_mut().enumerate() {
            // Walk backwards from cursor so the newest samples are always included.
            let ring_idx = (cursor.wrapping_sub(1).wrapping_sub(i)) % SAMPLES_PER_SPAN;
            *slot = self.samples[ring_idx].load(Ordering::Relaxed);
        }
        buf[..sample_count].sort_unstable();
        let p99_idx = ((sample_count * 99) / 100).min(sample_count.saturating_sub(1));
        let p99_ns = buf[p99_idx];

        SpanStats {
            count,
            min_ns,
            max_ns,
            mean_ns,
            p99_ns,
        }
    }
}

// ── SpanRecorder ──────────────────────────────────────────────────────────

/// Pre-allocated, allocation-free span timing recorder.
///
/// Supports up to [`MAX_SPAN_TYPES`] distinct span name IDs (0 ..
/// `MAX_SPAN_TYPES - 1`).  All operations after construction are lock-free
/// and allocation-free.
pub struct SpanRecorder {
    slots: Box<[SpanSlot]>,
}

impl SpanRecorder {
    /// Create a new recorder.  Allocates all internal buffers up front.
    pub fn new() -> Self {
        let slots: Vec<SpanSlot> = (0..MAX_SPAN_TYPES).map(|_| SpanSlot::new()).collect();
        Self {
            slots: slots.into_boxed_slice(),
        }
    }

    /// Begin timing a span identified by `name_id`.
    ///
    /// # Panics
    ///
    /// Panics if `name_id >= MAX_SPAN_TYPES`.
    pub fn start_span(&self, name_id: u16) -> SpanToken {
        assert!(
            (name_id as usize) < MAX_SPAN_TYPES,
            "name_id {name_id} exceeds MAX_SPAN_TYPES ({MAX_SPAN_TYPES})"
        );
        SpanToken {
            name_id,
            start: Instant::now(),
        }
    }

    /// Record the duration of a previously started span.
    pub fn end_span(&self, token: SpanToken) {
        let duration_ns = token.start.elapsed().as_nanos() as u64;
        self.slots[token.name_id as usize].record(duration_ns);
    }

    /// Record a pre-measured duration for a span type (useful for testing).
    pub fn record_duration(&self, name_id: u16, duration_ns: u64) {
        assert!(
            (name_id as usize) < MAX_SPAN_TYPES,
            "name_id {name_id} exceeds MAX_SPAN_TYPES ({MAX_SPAN_TYPES})"
        );
        self.slots[name_id as usize].record(duration_ns);
    }

    /// Compute statistics for a given span type.
    pub fn stats(&self, name_id: u16) -> SpanStats {
        assert!(
            (name_id as usize) < MAX_SPAN_TYPES,
            "name_id {name_id} exceeds MAX_SPAN_TYPES ({MAX_SPAN_TYPES})"
        );
        self.slots[name_id as usize].stats()
    }
}

impl Default for SpanRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: All interior mutation is through atomics.
unsafe impl Send for SpanRecorder {}
unsafe impl Sync for SpanRecorder {}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    const AXIS_SPAN: u16 = 0;
    const HID_SPAN: u16 = 1;

    #[test]
    fn start_end_records_nonzero_duration() {
        let rec = SpanRecorder::new();
        let token = rec.start_span(AXIS_SPAN);
        thread::sleep(std::time::Duration::from_millis(1));
        rec.end_span(token);

        let s = rec.stats(AXIS_SPAN);
        assert_eq!(s.count, 1);
        assert!(s.min_ns > 0);
        assert!(s.max_ns > 0);
    }

    #[test]
    fn record_duration_accumulates() {
        let rec = SpanRecorder::new();
        rec.record_duration(AXIS_SPAN, 100);
        rec.record_duration(AXIS_SPAN, 200);
        rec.record_duration(AXIS_SPAN, 300);

        let s = rec.stats(AXIS_SPAN);
        assert_eq!(s.count, 3);
        assert_eq!(s.min_ns, 100);
        assert_eq!(s.max_ns, 300);
        assert_eq!(s.mean_ns, 200);
    }

    #[test]
    fn stats_for_unused_span_returns_zeros() {
        let rec = SpanRecorder::new();
        let s = rec.stats(10);
        assert_eq!(s.count, 0);
        assert_eq!(s.min_ns, 0);
        assert_eq!(s.max_ns, 0);
        assert_eq!(s.mean_ns, 0);
        assert_eq!(s.p99_ns, 0);
    }

    #[test]
    fn independent_span_types() {
        let rec = SpanRecorder::new();
        rec.record_duration(AXIS_SPAN, 100);
        rec.record_duration(HID_SPAN, 999);

        assert_eq!(rec.stats(AXIS_SPAN).count, 1);
        assert_eq!(rec.stats(AXIS_SPAN).mean_ns, 100);
        assert_eq!(rec.stats(HID_SPAN).count, 1);
        assert_eq!(rec.stats(HID_SPAN).mean_ns, 999);
    }

    #[test]
    fn p99_with_outlier() {
        let rec = SpanRecorder::new();
        // 99 samples at 1000, 1 at 9999
        for _ in 0..99 {
            rec.record_duration(AXIS_SPAN, 1_000);
        }
        rec.record_duration(AXIS_SPAN, 9_999);

        let s = rec.stats(AXIS_SPAN);
        assert_eq!(s.count, 100);
        assert_eq!(s.min_ns, 1_000);
        assert_eq!(s.max_ns, 9_999);
        assert_eq!(s.p99_ns, 9_999);
    }

    #[test]
    fn ring_buffer_wraps_correctly() {
        let rec = SpanRecorder::new();
        // Fill more than the ring buffer capacity
        for i in 0..(SAMPLES_PER_SPAN as u64 * 2) {
            rec.record_duration(AXIS_SPAN, 1000 + i);
        }
        let s = rec.stats(AXIS_SPAN);
        assert_eq!(s.count, (SAMPLES_PER_SPAN * 2) as u64);
        // Min should be the very first sample (1000) which is still tracked
        // in the atomic min_ns field even after ring wrap.
        assert_eq!(s.min_ns, 1000);
    }

    #[test]
    fn concurrent_recording() {
        let rec = Arc::new(SpanRecorder::new());
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let rec = Arc::clone(&rec);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        rec.record_duration(AXIS_SPAN, 500);
                    }
                })
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }

        let s = rec.stats(AXIS_SPAN);
        assert_eq!(s.count, 4000);
        assert_eq!(s.min_ns, 500);
        assert_eq!(s.max_ns, 500);
    }

    #[test]
    fn single_sample_stats() {
        let rec = SpanRecorder::new();
        rec.record_duration(AXIS_SPAN, 42);
        let s = rec.stats(AXIS_SPAN);
        assert_eq!(s.count, 1);
        assert_eq!(s.min_ns, 42);
        assert_eq!(s.max_ns, 42);
        assert_eq!(s.mean_ns, 42);
        assert_eq!(s.p99_ns, 42);
    }

    #[test]
    fn mean_calculation_accuracy() {
        let rec = SpanRecorder::new();
        let values = [100u64, 200, 300, 400, 500];
        for &v in &values {
            rec.record_duration(AXIS_SPAN, v);
        }
        let s = rec.stats(AXIS_SPAN);
        assert_eq!(s.mean_ns, 300); // (100+200+300+400+500) / 5
    }

    #[test]
    #[should_panic(expected = "exceeds MAX_SPAN_TYPES")]
    fn out_of_range_name_id_panics() {
        let rec = SpanRecorder::new();
        rec.start_span(MAX_SPAN_TYPES as u16);
    }

    #[test]
    fn default_trait_works() {
        let rec = SpanRecorder::default();
        rec.record_duration(0, 100);
        assert_eq!(rec.stats(0).count, 1);
    }
}
