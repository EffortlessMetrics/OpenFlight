// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Telemetry recording and replay for testing (REQ-714).
//!
//! Provides [`TelemetryRecording`] for capturing bus events to JSON-lines
//! files, and [`ReplayIterator`] for deterministic playback at configurable
//! speed with optional looping.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// A single recorded telemetry event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryRecord {
    /// Microsecond timestamp relative to recording start.
    pub timestamp_us: u64,
    /// Semantic event type label.
    pub event_type: String,
    /// Opaque payload bytes.
    pub payload: Vec<u8>,
}

/// An ordered collection of telemetry records.
#[derive(Debug, Clone, Default)]
pub struct TelemetryRecording {
    records: Vec<TelemetryRecord>,
}

impl TelemetryRecording {
    /// Create an empty recording.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Append a record.
    pub fn add_record(&mut self, record: TelemetryRecord) {
        self.records.push(record);
    }

    /// Persist the recording as JSON-lines to `path`.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        for record in &self.records {
            let line = serde_json::to_string(record)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            writeln!(writer, "{line}")?;
        }
        writer.flush()?;
        Ok(())
    }

    /// Load a recording from a JSON-lines file.
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: TelemetryRecord = serde_json::from_str(&line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            records.push(record);
        }
        Ok(Self { records })
    }

    /// Borrow the underlying record slice.
    pub fn records(&self) -> &[TelemetryRecord] {
        &self.records
    }

    /// Total duration in microseconds (last − first timestamp, or 0 if empty/single).
    pub fn duration_us(&self) -> u64 {
        if self.records.len() < 2 {
            return 0;
        }
        self.records.last().unwrap().timestamp_us - self.records.first().unwrap().timestamp_us
    }
}

/// Replay configuration.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Playback speed multiplier (e.g. 2.0 = double speed).
    pub speed_multiplier: f32,
    /// If `true`, replay wraps around after the last record.
    pub loop_enabled: bool,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            speed_multiplier: 1.0,
            loop_enabled: false,
        }
    }
}

/// Iterator that yields [`TelemetryRecord`] references together with the
/// adjusted delay (in microseconds) that should elapse before that record
/// is delivered.
pub struct ReplayIterator<'a> {
    recording: &'a TelemetryRecording,
    config: ReplayConfig,
    index: usize,
}

impl<'a> ReplayIterator<'a> {
    /// Create a new replay iterator over `recording` with the given config.
    pub fn new(recording: &'a TelemetryRecording, config: ReplayConfig) -> Self {
        Self {
            recording,
            config,
            index: 0,
        }
    }
}

impl<'a> Iterator for ReplayIterator<'a> {
    /// `(delay_us, record)` — delay is the *adjusted* inter-record gap.
    type Item = (u64, &'a TelemetryRecord);

    fn next(&mut self) -> Option<Self::Item> {
        let records = self.recording.records();
        if records.is_empty() {
            return None;
        }

        if self.index >= records.len() {
            if self.config.loop_enabled {
                self.index = 0;
            } else {
                return None;
            }
        }

        let record = &records[self.index];
        let delay_us = if self.index == 0 {
            0
        } else {
            let prev = &records[self.index - 1];
            let raw = record.timestamp_us.saturating_sub(prev.timestamp_us);
            let multiplier = if self.config.speed_multiplier > 0.0 {
                self.config.speed_multiplier
            } else {
                1.0
            };
            (raw as f32 / multiplier) as u64
        };

        self.index += 1;
        Some((delay_us, record))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_records() -> Vec<TelemetryRecord> {
        vec![
            TelemetryRecord {
                timestamp_us: 1000,
                event_type: "axis".into(),
                payload: vec![1, 2, 3],
            },
            TelemetryRecord {
                timestamp_us: 2000,
                event_type: "button".into(),
                payload: vec![4, 5],
            },
            TelemetryRecord {
                timestamp_us: 4000,
                event_type: "axis".into(),
                payload: vec![6],
            },
        ]
    }

    #[test]
    fn test_record_and_playback() {
        let mut rec = TelemetryRecording::new();
        for r in sample_records() {
            rec.add_record(r);
        }
        let iter = ReplayIterator::new(&rec, ReplayConfig::default());
        let items: Vec<_> = iter.collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].1.event_type, "axis");
        assert_eq!(items[1].1.event_type, "button");
    }

    #[test]
    fn test_save_load_roundtrip() {
        let mut rec = TelemetryRecording::new();
        for r in sample_records() {
            rec.add_record(r);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        rec.save_to_file(&path).unwrap();

        let loaded = TelemetryRecording::load_from_file(&path).unwrap();
        assert_eq!(loaded.records(), rec.records());
    }

    #[test]
    fn test_duration_calculation() {
        let mut rec = TelemetryRecording::new();
        for r in sample_records() {
            rec.add_record(r);
        }
        // last (4000) - first (1000) = 3000
        assert_eq!(rec.duration_us(), 3000);
    }

    #[test]
    fn test_empty_recording() {
        let rec = TelemetryRecording::new();
        assert_eq!(rec.duration_us(), 0);
        assert!(rec.records().is_empty());
    }

    #[test]
    fn test_speed_multiplier() {
        let mut rec = TelemetryRecording::new();
        for r in sample_records() {
            rec.add_record(r);
        }
        let config = ReplayConfig {
            speed_multiplier: 2.0,
            loop_enabled: false,
        };
        let items: Vec<_> = ReplayIterator::new(&rec, config).collect();
        // Original gaps: 0, 1000, 2000  → at 2× speed: 0, 500, 1000
        assert_eq!(items[0].0, 0);
        assert_eq!(items[1].0, 500);
        assert_eq!(items[2].0, 1000);
    }

    #[test]
    fn test_loop_mode() {
        let mut rec = TelemetryRecording::new();
        for r in sample_records() {
            rec.add_record(r);
        }
        let config = ReplayConfig {
            speed_multiplier: 1.0,
            loop_enabled: true,
        };
        let mut iter = ReplayIterator::new(&rec, config);
        // Consume first pass (3 records)
        for _ in 0..3 {
            assert!(iter.next().is_some());
        }
        // Should loop: next record is the first again
        let looped = iter.next().unwrap();
        assert_eq!(looped.1.event_type, "axis");
        assert_eq!(looped.0, 0); // first record of new loop has 0 delay
    }
}
