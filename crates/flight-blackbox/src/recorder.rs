// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Zero-allocation ring-buffer flight recorder.
//!
//! All [`BlackboxRecorder`] write methods are allocation-free on the hot path.
//! The backing buffer is pre-allocated at construction time and old entries are
//! silently overwritten when the buffer is full.

use std::fmt;

/// Default recording duration in seconds at 250 Hz.
const DEFAULT_DURATION_SECS: usize = 60;
/// Default tick rate in Hz.
const DEFAULT_TICK_RATE: usize = 250;

/// Maximum length of an event source tag stored inline.
pub const EVENT_SOURCE_MAX: usize = 32;
/// Maximum length of inline event payload bytes.
pub const EVENT_DATA_MAX: usize = 64;
/// Maximum length of an inline simulator identifier.
pub const SIM_ID_MAX: usize = 16;
/// Maximum length of inline telemetry snapshot bytes.
pub const SNAPSHOT_MAX: usize = 128;

// ── Record types ─────────────────────────────────────────────────────

/// Types of entries stored in the ring buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordEntry {
    /// A single axis sample.
    Axis(AxisRecord),
    /// A system event.
    Event(EventRecord),
    /// A telemetry snapshot from a sim adapter.
    Telemetry(TelemetryRecord),
    /// A force-feedback output sample.
    Ffb(FfbRecord),
    /// Unused slot (sentinel for empty pre-allocated entries).
    Empty,
}

/// Captured axis sample (zero-alloc, Copy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisRecord {
    pub axis_id: u16,
    pub raw: f64,
    pub processed: f64,
    pub timestamp_ns: u64,
}

/// Captured system event (fixed-size, zero-alloc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventRecord {
    pub timestamp_ns: u64,
    pub event_type: u16,
    /// Inline source tag — only `source_len` bytes are meaningful.
    pub source: [u8; EVENT_SOURCE_MAX],
    pub source_len: u8,
    /// Inline payload — only `data_len` bytes are meaningful.
    pub data: [u8; EVENT_DATA_MAX],
    pub data_len: u8,
}

impl EventRecord {
    /// View the source bytes as a UTF-8 string (best-effort).
    pub fn source_str(&self) -> &str {
        std::str::from_utf8(&self.source[..self.source_len as usize]).unwrap_or("<invalid>")
    }

    /// View the data bytes as a slice.
    pub fn data_bytes(&self) -> &[u8] {
        &self.data[..self.data_len as usize]
    }
}

/// Captured telemetry frame (fixed-size, zero-alloc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TelemetryRecord {
    pub timestamp_ns: u64,
    /// Inline sim identifier — only `sim_len` bytes are meaningful.
    pub sim: [u8; SIM_ID_MAX],
    pub sim_len: u8,
    /// Inline snapshot payload — only `snapshot_len` bytes are meaningful.
    pub snapshot: [u8; SNAPSHOT_MAX],
    pub snapshot_len: u8,
}

impl TelemetryRecord {
    /// View the sim identifier as a string.
    pub fn sim_str(&self) -> &str {
        std::str::from_utf8(&self.sim[..self.sim_len as usize]).unwrap_or("<invalid>")
    }

    /// View the snapshot payload.
    pub fn snapshot_bytes(&self) -> &[u8] {
        &self.snapshot[..self.snapshot_len as usize]
    }
}

/// Captured force-feedback output sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FfbRecord {
    pub timestamp_ns: u64,
    pub effect_type: u16,
    pub magnitude: f64,
}

// ── Ring buffer ──────────────────────────────────────────────────────

/// Configuration for [`BlackboxRecorder`].
#[derive(Debug, Clone)]
pub struct RecorderConfig {
    /// Number of slots to pre-allocate.
    pub capacity: usize,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_DURATION_SECS * DEFAULT_TICK_RATE,
        }
    }
}

/// A fixed-size, pre-allocated ring-buffer flight recorder.
///
/// All `record_*` methods execute in O(1) time with **zero heap allocation**.
/// When the buffer is full the oldest entry is silently overwritten.
pub struct BlackboxRecorder {
    buf: Vec<RecordEntry>,
    /// Total capacity (length of `buf`).
    capacity: usize,
    /// Write cursor — always wraps modulo `capacity`.
    write_pos: usize,
    /// Total number of entries ever written (may exceed `capacity`).
    total_written: u64,
}

impl fmt::Debug for BlackboxRecorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlackboxRecorder")
            .field("capacity", &self.capacity)
            .field("len", &self.len())
            .field("total_written", &self.total_written)
            .finish()
    }
}

impl BlackboxRecorder {
    /// Create a new recorder with the given configuration.
    ///
    /// The entire ring buffer is allocated up front.
    pub fn new(config: RecorderConfig) -> Self {
        let capacity = config.capacity.max(1);
        let mut buf = Vec::with_capacity(capacity);
        buf.resize(capacity, RecordEntry::Empty);
        Self {
            buf,
            capacity,
            write_pos: 0,
            total_written: 0,
        }
    }

    /// Create a recorder with the default 60-second / 250 Hz capacity.
    pub fn with_defaults() -> Self {
        Self::new(RecorderConfig::default())
    }

    // ── Recording (zero-alloc) ───────────────────────────────────────

    /// Record an axis sample. Zero allocation.
    #[inline]
    pub fn record_axis(&mut self, axis_id: u16, raw: f64, processed: f64, timestamp_ns: u64) {
        self.push(RecordEntry::Axis(AxisRecord {
            axis_id,
            raw,
            processed,
            timestamp_ns,
        }));
    }

    /// Record a system event. Zero allocation.
    ///
    /// `source` and `data` are truncated to [`EVENT_SOURCE_MAX`] and
    /// [`EVENT_DATA_MAX`] respectively.
    #[inline]
    pub fn record_event(&mut self, event_type: u16, source: &str, data: &[u8]) {
        let mut src_buf = [0u8; EVENT_SOURCE_MAX];
        let src_len = source.len().min(EVENT_SOURCE_MAX);
        src_buf[..src_len].copy_from_slice(&source.as_bytes()[..src_len]);

        let mut data_buf = [0u8; EVENT_DATA_MAX];
        let data_len = data.len().min(EVENT_DATA_MAX);
        data_buf[..data_len].copy_from_slice(&data[..data_len]);

        let now = crate::time::monotonic_now_ns();
        self.push(RecordEntry::Event(EventRecord {
            timestamp_ns: now,
            event_type,
            source: src_buf,
            source_len: src_len as u8,
            data: data_buf,
            data_len: data_len as u8,
        }));
    }

    /// Record a telemetry frame. Zero allocation.
    ///
    /// `sim` is truncated to [`SIM_ID_MAX`]; `snapshot` to [`SNAPSHOT_MAX`].
    #[inline]
    pub fn record_telemetry(&mut self, sim: &str, snapshot: &[u8]) {
        let mut sim_buf = [0u8; SIM_ID_MAX];
        let sim_len = sim.len().min(SIM_ID_MAX);
        sim_buf[..sim_len].copy_from_slice(&sim.as_bytes()[..sim_len]);

        let mut snap_buf = [0u8; SNAPSHOT_MAX];
        let snap_len = snapshot.len().min(SNAPSHOT_MAX);
        snap_buf[..snap_len].copy_from_slice(&snapshot[..snap_len]);

        let now = crate::time::monotonic_now_ns();
        self.push(RecordEntry::Telemetry(TelemetryRecord {
            timestamp_ns: now,
            sim: sim_buf,
            sim_len: sim_len as u8,
            snapshot: snap_buf,
            snapshot_len: snap_len as u8,
        }));
    }

    /// Record a force-feedback output. Zero allocation.
    #[inline]
    pub fn record_ffb(&mut self, effect_type: u16, magnitude: f64) {
        let now = crate::time::monotonic_now_ns();
        self.push(RecordEntry::Ffb(FfbRecord {
            timestamp_ns: now,
            effect_type,
            magnitude,
        }));
    }

    // ── Query / introspection ────────────────────────────────────────

    /// Number of valid (non-empty) entries currently in the buffer.
    pub fn len(&self) -> usize {
        (self.total_written as usize).min(self.capacity)
    }

    /// Returns `true` when no entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.total_written == 0
    }

    /// Pre-allocated capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Total entries written since creation (including overwritten ones).
    pub fn total_written(&self) -> u64 {
        self.total_written
    }

    /// Number of entries that were overwritten due to overflow.
    pub fn overflow_count(&self) -> u64 {
        self.total_written.saturating_sub(self.capacity as u64)
    }

    /// Iterate over all valid entries in chronological order (oldest first).
    pub fn iter(&self) -> RecorderIter<'_> {
        let len = self.len();
        let start = if self.total_written as usize > self.capacity {
            self.write_pos
        } else {
            0
        };
        RecorderIter {
            buf: &self.buf,
            capacity: self.capacity,
            pos: start,
            remaining: len,
        }
    }

    /// Return a snapshot of all valid entries as a `Vec` (oldest first).
    ///
    /// This allocates — intended for export / analysis, not the hot path.
    pub fn snapshot(&self) -> Vec<RecordEntry> {
        self.iter().copied().collect()
    }

    /// Clear all recorded data (capacity is retained).
    pub fn clear(&mut self) {
        for slot in &mut self.buf {
            *slot = RecordEntry::Empty;
        }
        self.write_pos = 0;
        self.total_written = 0;
    }

    // ── Internal ─────────────────────────────────────────────────────

    /// Push a record into the ring buffer. O(1), zero allocation.
    #[inline(always)]
    fn push(&mut self, entry: RecordEntry) {
        // SAFETY: write_pos is always < capacity.
        self.buf[self.write_pos] = entry;
        self.write_pos = (self.write_pos + 1) % self.capacity;
        self.total_written += 1;
    }
}

// ── Iterator ─────────────────────────────────────────────────────────

/// Iterator over recorder entries in chronological order.
pub struct RecorderIter<'a> {
    buf: &'a [RecordEntry],
    capacity: usize,
    pos: usize,
    remaining: usize,
}

impl<'a> Iterator for RecorderIter<'a> {
    type Item = &'a RecordEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let entry = &self.buf[self.pos];
        self.pos = (self.pos + 1) % self.capacity;
        self.remaining -= 1;
        Some(entry)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for RecorderIter<'_> {}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn small_recorder(cap: usize) -> BlackboxRecorder {
        BlackboxRecorder::new(RecorderConfig { capacity: cap })
    }

    #[test]
    fn new_recorder_is_empty() {
        let rec = small_recorder(16);
        assert!(rec.is_empty());
        assert_eq!(rec.len(), 0);
        assert_eq!(rec.capacity(), 16);
        assert_eq!(rec.total_written(), 0);
        assert_eq!(rec.overflow_count(), 0);
    }

    #[test]
    fn record_and_retrieve_axis() {
        let mut rec = small_recorder(64);
        rec.record_axis(1, 0.5, 0.75, 1000);
        rec.record_axis(2, -1.0, -0.9, 2000);

        assert_eq!(rec.len(), 2);
        assert_eq!(rec.total_written(), 2);

        let entries: Vec<_> = rec.snapshot();
        assert_eq!(entries.len(), 2);

        match entries[0] {
            RecordEntry::Axis(a) => {
                assert_eq!(a.axis_id, 1);
                assert!((a.raw - 0.5).abs() < f64::EPSILON);
                assert!((a.processed - 0.75).abs() < f64::EPSILON);
                assert_eq!(a.timestamp_ns, 1000);
            }
            _ => panic!("expected Axis"),
        }
        match entries[1] {
            RecordEntry::Axis(a) => {
                assert_eq!(a.axis_id, 2);
                assert!((a.raw - (-1.0)).abs() < f64::EPSILON);
            }
            _ => panic!("expected Axis"),
        }
    }

    #[test]
    fn record_event_stores_correctly() {
        let mut rec = small_recorder(8);
        rec.record_event(42, "hid-device", &[0xDE, 0xAD]);

        let entries = rec.snapshot();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            RecordEntry::Event(e) => {
                assert_eq!(e.event_type, 42);
                assert_eq!(e.source_str(), "hid-device");
                assert_eq!(e.data_bytes(), &[0xDE, 0xAD]);
            }
            _ => panic!("expected Event"),
        }
    }

    #[test]
    fn record_telemetry_stores_correctly() {
        let mut rec = small_recorder(8);
        let snap_data = [0x01, 0x02, 0x03];
        rec.record_telemetry("MSFS", &snap_data);

        let entries = rec.snapshot();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            RecordEntry::Telemetry(t) => {
                assert_eq!(t.sim_str(), "MSFS");
                assert_eq!(t.snapshot_bytes(), &snap_data);
            }
            _ => panic!("expected Telemetry"),
        }
    }

    #[test]
    fn record_ffb_stores_correctly() {
        let mut rec = small_recorder(8);
        rec.record_ffb(7, 0.85);

        let entries = rec.snapshot();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            RecordEntry::Ffb(f) => {
                assert_eq!(f.effect_type, 7);
                assert!((f.magnitude - 0.85).abs() < f64::EPSILON);
            }
            _ => panic!("expected Ffb"),
        }
    }

    #[test]
    fn ring_buffer_overflow_drops_oldest() {
        let mut rec = small_recorder(4);

        // Write 6 entries into a buffer of capacity 4
        for i in 0u16..6 {
            rec.record_axis(i, i as f64, i as f64, (i as u64) * 1000);
        }

        assert_eq!(rec.len(), 4);
        assert_eq!(rec.total_written(), 6);
        assert_eq!(rec.overflow_count(), 2);

        let entries = rec.snapshot();
        // Should contain entries 2,3,4,5 (oldest 0,1 dropped)
        let ids: Vec<u16> = entries
            .iter()
            .map(|e| match e {
                RecordEntry::Axis(a) => a.axis_id,
                _ => panic!("expected Axis"),
            })
            .collect();
        assert_eq!(ids, vec![2, 3, 4, 5]);
    }

    #[test]
    fn ring_buffer_exact_capacity_fill() {
        let mut rec = small_recorder(3);
        rec.record_axis(0, 0.0, 0.0, 100);
        rec.record_axis(1, 1.0, 1.0, 200);
        rec.record_axis(2, 2.0, 2.0, 300);

        assert_eq!(rec.len(), 3);
        assert_eq!(rec.overflow_count(), 0);

        // One more overflows
        rec.record_axis(3, 3.0, 3.0, 400);
        assert_eq!(rec.len(), 3);
        assert_eq!(rec.overflow_count(), 1);

        let ids: Vec<u16> = rec
            .snapshot()
            .iter()
            .map(|e| match e {
                RecordEntry::Axis(a) => a.axis_id,
                _ => panic!("expected Axis"),
            })
            .collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn clear_resets_state() {
        let mut rec = small_recorder(8);
        rec.record_axis(1, 0.0, 0.0, 100);
        rec.record_axis(2, 0.0, 0.0, 200);
        assert_eq!(rec.len(), 2);

        rec.clear();
        assert!(rec.is_empty());
        assert_eq!(rec.len(), 0);
        assert_eq!(rec.total_written(), 0);
        assert_eq!(rec.capacity(), 8);
    }

    #[test]
    fn iterator_exact_size() {
        let mut rec = small_recorder(8);
        for i in 0..5 {
            rec.record_axis(i, 0.0, 0.0, i as u64);
        }
        let iter = rec.iter();
        assert_eq!(iter.len(), 5);
    }

    #[test]
    fn event_source_truncation() {
        let mut rec = small_recorder(4);
        let long_source = "a]".repeat(EVENT_SOURCE_MAX + 10);
        rec.record_event(1, &long_source, &[]);

        let entries = rec.snapshot();
        match &entries[0] {
            RecordEntry::Event(e) => {
                assert_eq!(e.source_len as usize, EVENT_SOURCE_MAX);
            }
            _ => panic!("expected Event"),
        }
    }

    #[test]
    fn default_config_capacity() {
        let cfg = RecorderConfig::default();
        assert_eq!(cfg.capacity, DEFAULT_DURATION_SECS * DEFAULT_TICK_RATE);
    }

    #[test]
    fn zero_capacity_clamped_to_one() {
        let rec = BlackboxRecorder::new(RecorderConfig { capacity: 0 });
        assert_eq!(rec.capacity(), 1);
    }

    #[test]
    fn zero_alloc_on_hot_path() {
        // Verify that recording does not allocate by measuring heap usage
        // indirectly: we fill the buffer, then record more entries and
        // confirm the capacity did not change (no reallocation).
        let mut rec = small_recorder(64);
        let cap_before = rec.buf.capacity();

        for i in 0..200u16 {
            rec.record_axis(i, i as f64, i as f64, i as u64);
        }

        assert_eq!(rec.buf.capacity(), cap_before, "buffer must not reallocate");
        assert_eq!(rec.capacity(), 64);
        assert_eq!(rec.len(), 64);
        assert_eq!(rec.total_written(), 200);
    }

    #[test]
    fn mixed_record_types_in_buffer() {
        let mut rec = small_recorder(16);
        rec.record_axis(1, 0.5, 0.6, 100);
        rec.record_event(10, "panel", &[0x01]);
        rec.record_telemetry("DCS", &[0xAA, 0xBB]);
        rec.record_ffb(3, 0.9);

        assert_eq!(rec.len(), 4);
        let entries = rec.snapshot();
        assert!(matches!(entries[0], RecordEntry::Axis(_)));
        assert!(matches!(entries[1], RecordEntry::Event(_)));
        assert!(matches!(entries[2], RecordEntry::Telemetry(_)));
        assert!(matches!(entries[3], RecordEntry::Ffb(_)));
    }

    #[test]
    fn large_overflow_preserves_newest() {
        let mut rec = small_recorder(4);
        for i in 0..10_000u16 {
            rec.record_axis(i, 0.0, 0.0, i as u64);
        }
        assert_eq!(rec.len(), 4);
        assert_eq!(rec.overflow_count(), 9996);

        let ids: Vec<u16> = rec
            .snapshot()
            .iter()
            .map(|e| match e {
                RecordEntry::Axis(a) => a.axis_id,
                _ => panic!("expected Axis"),
            })
            .collect();
        assert_eq!(ids, vec![9996, 9997, 9998, 9999]);
    }
}
