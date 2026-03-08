// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Replay engine for blackbox recordings.
//!
//! Provides iterator-based playback of [`Record`] streams with time-scaling
//! and per-type filtering. All replay state is stack-allocated.

use crate::codec::{self, CodecError, Record};

/// Which record types to include during replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordFilter {
    pub axis_frames: bool,
    pub bus_events: bool,
    pub timing_marks: bool,
    pub annotations: bool,
}

impl RecordFilter {
    /// Accept all record types.
    pub fn all() -> Self {
        Self {
            axis_frames: true,
            bus_events: true,
            timing_marks: true,
            annotations: true,
        }
    }

    /// Accept only the specified type.
    pub fn only_axis_frames() -> Self {
        Self {
            axis_frames: true,
            bus_events: false,
            timing_marks: false,
            annotations: false,
        }
    }

    /// Accept only bus events.
    pub fn only_bus_events() -> Self {
        Self {
            axis_frames: false,
            bus_events: true,
            timing_marks: false,
            annotations: false,
        }
    }

    /// Accept only timing marks.
    pub fn only_timing_marks() -> Self {
        Self {
            axis_frames: false,
            bus_events: false,
            timing_marks: true,
            annotations: false,
        }
    }

    fn accepts(&self, record: &Record) -> bool {
        match record {
            Record::AxisFrame(_) => self.axis_frames,
            Record::BusEvent(_) => self.bus_events,
            Record::TimingMark(_) => self.timing_marks,
            Record::Annotation(_) => self.annotations,
        }
    }
}

impl Default for RecordFilter {
    fn default() -> Self {
        Self::all()
    }
}

/// Configuration for the replay engine.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Playback speed multiplier. 1.0 = real-time, 2.0 = double speed, etc.
    pub time_scale: f64,
    /// Record type filter.
    pub filter: RecordFilter,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            filter: RecordFilter::all(),
        }
    }
}

/// A replayed record with its adjusted timestamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayEntry {
    /// Original record.
    pub record: Record,
    /// Timestamp (ns) adjusted for time scaling, relative to the first record.
    pub scaled_time_ns: u64,
}

/// Replay engine that iterates over a buffer of encoded records.
///
/// Records are decoded lazily as the iterator advances. Time-scaled timestamps
/// are computed relative to the first record's original timestamp.
pub struct ReplayEngine<'a> {
    data: &'a [u8],
    offset: usize,
    config: ReplayConfig,
    base_ts: Option<u64>,
}

impl<'a> ReplayEngine<'a> {
    /// Create a replay engine over encoded record data.
    ///
    /// Returns an error if `time_scale` is non-finite or ≤ 0.
    pub fn new(data: &'a [u8], config: ReplayConfig) -> Result<Self, CodecError> {
        if !config.time_scale.is_finite() || config.time_scale <= 0.0 {
            return Err(CodecError::InvalidTimeScale);
        }
        Ok(Self {
            data,
            offset: 0,
            config,
            base_ts: None,
        })
    }

    /// Reset the engine to the beginning of the data.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.base_ts = None;
    }

    /// Remaining bytes in the input buffer.
    pub fn remaining_bytes(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }
}

impl Iterator for ReplayEngine<'_> {
    type Item = Result<ReplayEntry, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.offset >= self.data.len() {
                return None;
            }
            let (record, consumed) = match codec::decode(&self.data[self.offset..]) {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            self.offset += consumed;

            // Set base_ts from the first decoded record regardless of filter.
            let ts = record.timestamp_ns();
            self.base_ts.get_or_insert(ts);

            if !self.config.filter.accepts(&record) {
                continue;
            }

            let base = self.base_ts.unwrap();
            let delta = ts.saturating_sub(base);
            let scaled = (delta as f64 / self.config.time_scale) as u64;

            return Some(Ok(ReplayEntry {
                record,
                scaled_time_ns: scaled,
            }));
        }
    }
}

/// Convenience: encode a slice of records into a `Vec<u8>`.
///
/// This allocates — intended for test / setup, not the hot path.
pub fn encode_records(records: &[Record]) -> Vec<u8> {
    let mut out = Vec::with_capacity(records.len() * codec::MAX_ENCODED_SIZE);
    let mut buf = [0u8; codec::MAX_ENCODED_SIZE];
    for r in records {
        let n = codec::encode(r, &mut buf).expect("encode failed");
        out.extend_from_slice(&buf[..n]);
    }
    out
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{ANNOTATION_MAX, Annotation, AxisFrame, BusEvent, TimingMark};

    fn make_axis(ts: u64, id: u16) -> Record {
        Record::AxisFrame(AxisFrame {
            timestamp_ns: ts,
            axis_id: id,
            raw: 0.0,
            processed: 0.0,
        })
    }

    fn make_bus(ts: u64, code: u16) -> Record {
        Record::BusEvent(BusEvent {
            timestamp_ns: ts,
            event_code: code,
            payload: [0; 8],
            payload_len: 0,
        })
    }

    fn make_timing(ts: u64, seq: u32) -> Record {
        Record::TimingMark(TimingMark {
            timestamp_ns: ts,
            sequence: seq,
            delta_ns: 4000,
        })
    }

    fn make_annotation(ts: u64, text: &str) -> Record {
        let mut msg = [0u8; ANNOTATION_MAX];
        let len = text.len().min(ANNOTATION_MAX);
        msg[..len].copy_from_slice(&text.as_bytes()[..len]);
        Record::Annotation(Annotation {
            timestamp_ns: ts,
            msg,
            msg_len: len as u8,
        })
    }

    #[test]
    fn replay_produces_records_in_order() {
        let records = vec![
            make_axis(1_000_000, 1),
            make_axis(2_000_000, 2),
            make_axis(3_000_000, 3),
        ];
        let data = encode_records(&records);
        let engine = ReplayEngine::new(&data, ReplayConfig::default()).unwrap();
        let entries: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].scaled_time_ns, 0);
        assert_eq!(entries[1].scaled_time_ns, 1_000_000);
        assert_eq!(entries[2].scaled_time_ns, 2_000_000);
    }

    #[test]
    fn replay_time_scaling_2x() {
        let records = vec![
            make_axis(0, 1),
            make_axis(4_000_000, 2), // 4ms later
        ];
        let data = encode_records(&records);
        let config = ReplayConfig {
            time_scale: 2.0,
            ..Default::default()
        };
        let engine = ReplayEngine::new(&data, config).unwrap();
        let entries: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 2);
        // At 2x speed, 4ms becomes 2ms
        assert_eq!(entries[1].scaled_time_ns, 2_000_000);
    }

    #[test]
    fn replay_time_scaling_half() {
        let records = vec![make_axis(0, 1), make_axis(4_000_000, 2)];
        let data = encode_records(&records);
        let config = ReplayConfig {
            time_scale: 0.5,
            ..Default::default()
        };
        let engine = ReplayEngine::new(&data, config).unwrap();
        let entries: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 2);
        // At 0.5x speed, 4ms becomes 8ms
        assert_eq!(entries[1].scaled_time_ns, 8_000_000);
    }

    #[test]
    fn replay_filter_axis_only() {
        let records = vec![
            make_axis(1_000_000, 1),
            make_bus(2_000_000, 10),
            make_axis(3_000_000, 2),
            make_timing(4_000_000, 1),
        ];
        let data = encode_records(&records);
        let config = ReplayConfig {
            filter: RecordFilter::only_axis_frames(),
            ..Default::default()
        };
        let engine = ReplayEngine::new(&data, config).unwrap();
        let entries: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0].record, Record::AxisFrame(_)));
        assert!(matches!(entries[1].record, Record::AxisFrame(_)));
    }

    #[test]
    fn replay_filter_bus_events_only() {
        let records = vec![
            make_axis(1_000_000, 1),
            make_bus(2_000_000, 10),
            make_bus(3_000_000, 20),
        ];
        let data = encode_records(&records);
        let config = ReplayConfig {
            filter: RecordFilter::only_bus_events(),
            ..Default::default()
        };
        let engine = ReplayEngine::new(&data, config).unwrap();
        let entries: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0].record, Record::BusEvent(_)));
    }

    #[test]
    fn replay_filter_timing_marks_only() {
        let records = vec![
            make_timing(1_000_000, 1),
            make_axis(2_000_000, 1),
            make_timing(3_000_000, 2),
        ];
        let data = encode_records(&records);
        let config = ReplayConfig {
            filter: RecordFilter::only_timing_marks(),
            ..Default::default()
        };
        let engine = ReplayEngine::new(&data, config).unwrap();
        let entries: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0].record, Record::TimingMark(_)));
    }

    #[test]
    fn replay_empty_data() {
        let engine = ReplayEngine::new(&[], ReplayConfig::default()).unwrap();
        let entries: Vec<_> = engine.collect();
        assert!(entries.is_empty());
    }

    #[test]
    fn replay_mixed_types() {
        let records = vec![
            make_axis(1_000_000, 1),
            make_bus(2_000_000, 5),
            make_timing(3_000_000, 1),
            make_annotation(4_000_000, "test"),
        ];
        let data = encode_records(&records);
        let engine = ReplayEngine::new(&data, ReplayConfig::default()).unwrap();
        let entries: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 4);
        assert!(matches!(entries[0].record, Record::AxisFrame(_)));
        assert!(matches!(entries[1].record, Record::BusEvent(_)));
        assert!(matches!(entries[2].record, Record::TimingMark(_)));
        assert!(matches!(entries[3].record, Record::Annotation(_)));
    }

    #[test]
    fn replay_reset_replays_from_start() {
        let records = vec![make_axis(1_000_000, 1), make_axis(2_000_000, 2)];
        let data = encode_records(&records);
        let mut engine = ReplayEngine::new(&data, ReplayConfig::default()).unwrap();
        let first_pass: Vec<ReplayEntry> = engine.by_ref().map(|r| r.unwrap()).collect();
        assert_eq!(first_pass.len(), 2);

        engine.reset();
        let second_pass: Vec<ReplayEntry> = engine.map(|r| r.unwrap()).collect();
        assert_eq!(second_pass.len(), 2);
        assert_eq!(first_pass[0].record, second_pass[0].record);
    }

    #[test]
    fn replay_remaining_bytes() {
        let records = vec![make_axis(0, 1)];
        let data = encode_records(&records);
        let total = data.len();
        let mut engine = ReplayEngine::new(&data, ReplayConfig::default()).unwrap();
        assert_eq!(engine.remaining_bytes(), total);
        engine.next();
        assert_eq!(engine.remaining_bytes(), 0);
    }

    #[test]
    fn encode_records_helper() {
        let records = vec![make_axis(0, 1), make_bus(1000, 2)];
        let data = encode_records(&records);
        assert!(!data.is_empty());
        // Should be decodable
        let (r1, n1) = codec::decode(&data).unwrap();
        assert!(matches!(r1, Record::AxisFrame(_)));
        let (r2, _) = codec::decode(&data[n1..]).unwrap();
        assert!(matches!(r2, Record::BusEvent(_)));
    }
}
