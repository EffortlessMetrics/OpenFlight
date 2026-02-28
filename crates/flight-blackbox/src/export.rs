// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Export recorded blackbox data to JSON, CSV and compact binary formats.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::recorder::{
    AxisRecord, BlackboxRecorder, EventRecord, FfbRecord, RecordEntry, TelemetryRecord,
};

/// Errors that can occur during export.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Postcard serialization error: {0}")]
    Postcard(#[from] postcard::Error),
}

// ── Serializable mirror types ────────────────────────────────────────

/// JSON/binary-serializable axis record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxisRecordDto {
    pub axis_id: u16,
    pub raw: f64,
    pub processed: f64,
    pub timestamp_ns: u64,
}

/// JSON/binary-serializable event record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventRecordDto {
    pub timestamp_ns: u64,
    pub event_type: u16,
    pub source: String,
    pub data: Vec<u8>,
}

/// JSON/binary-serializable telemetry record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryRecordDto {
    pub timestamp_ns: u64,
    pub sim: String,
    pub snapshot: Vec<u8>,
}

/// JSON/binary-serializable FFB record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FfbRecordDto {
    pub timestamp_ns: u64,
    pub effect_type: u16,
    pub magnitude: f64,
}

/// A single exported entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExportEntry {
    Axis(AxisRecordDto),
    Event(EventRecordDto),
    Telemetry(TelemetryRecordDto),
    Ffb(FfbRecordDto),
}

/// Complete export document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecorderExportDoc {
    pub version: u32,
    pub entry_count: usize,
    pub entries: Vec<ExportEntry>,
}

impl RecorderExportDoc {
    pub const VERSION: u32 = 1;
}

// ── Conversion helpers ───────────────────────────────────────────────

fn axis_to_dto(a: &AxisRecord) -> AxisRecordDto {
    AxisRecordDto {
        axis_id: a.axis_id,
        raw: a.raw,
        processed: a.processed,
        timestamp_ns: a.timestamp_ns,
    }
}

fn event_to_dto(e: &EventRecord) -> EventRecordDto {
    EventRecordDto {
        timestamp_ns: e.timestamp_ns,
        event_type: e.event_type,
        source: e.source_str().to_string(),
        data: e.data_bytes().to_vec(),
    }
}

fn telemetry_to_dto(t: &TelemetryRecord) -> TelemetryRecordDto {
    TelemetryRecordDto {
        timestamp_ns: t.timestamp_ns,
        sim: t.sim_str().to_string(),
        snapshot: t.snapshot_bytes().to_vec(),
    }
}

fn ffb_to_dto(f: &FfbRecord) -> FfbRecordDto {
    FfbRecordDto {
        timestamp_ns: f.timestamp_ns,
        effect_type: f.effect_type,
        magnitude: f.magnitude,
    }
}

fn entry_to_export(entry: &RecordEntry) -> Option<ExportEntry> {
    match entry {
        RecordEntry::Axis(a) => Some(ExportEntry::Axis(axis_to_dto(a))),
        RecordEntry::Event(e) => Some(ExportEntry::Event(event_to_dto(e))),
        RecordEntry::Telemetry(t) => Some(ExportEntry::Telemetry(telemetry_to_dto(t))),
        RecordEntry::Ffb(f) => Some(ExportEntry::Ffb(ffb_to_dto(f))),
        RecordEntry::Empty => None,
    }
}

fn build_export_doc(recorder: &BlackboxRecorder) -> RecorderExportDoc {
    let entries: Vec<ExportEntry> = recorder.iter().filter_map(entry_to_export).collect();
    RecorderExportDoc {
        version: RecorderExportDoc::VERSION,
        entry_count: entries.len(),
        entries,
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// Export the recorder contents as pretty-printed JSON.
pub fn export_json(recorder: &BlackboxRecorder, path: &Path) -> Result<(), ExportError> {
    let doc = build_export_doc(recorder);
    let json = serde_json::to_string_pretty(&doc)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Export axis data as CSV (one row per axis sample).
///
/// Non-axis records are skipped. The CSV uses the header:
/// `timestamp_ns,axis_id,raw,processed`
pub fn export_csv(recorder: &BlackboxRecorder, path: &Path) -> Result<(), ExportError> {
    let mut out = std::fs::File::create(path)?;
    writeln!(out, "timestamp_ns,axis_id,raw,processed")?;
    for entry in recorder.iter() {
        if let RecordEntry::Axis(a) = entry {
            writeln!(
                out,
                "{},{},{},{}",
                a.timestamp_ns, a.axis_id, a.raw, a.processed
            )?;
        }
    }
    out.flush()?;
    Ok(())
}

/// Export recorder contents in a compact binary format (postcard).
pub fn export_binary(recorder: &BlackboxRecorder, path: &Path) -> Result<(), ExportError> {
    let doc = build_export_doc(recorder);
    let bytes = postcard::to_stdvec(&doc)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Human-readable summary of the current recording.
pub fn summary(recorder: &BlackboxRecorder) -> RecorderSummary {
    let mut axis_count: u64 = 0;
    let mut event_count: u64 = 0;
    let mut telemetry_count: u64 = 0;
    let mut ffb_count: u64 = 0;

    let mut min_ts = u64::MAX;
    let mut max_ts = 0u64;

    let mut axis_min = f64::INFINITY;
    let mut axis_max = f64::NEG_INFINITY;

    for entry in recorder.iter() {
        let ts = match entry {
            RecordEntry::Axis(a) => {
                axis_count += 1;
                if a.processed < axis_min {
                    axis_min = a.processed;
                }
                if a.processed > axis_max {
                    axis_max = a.processed;
                }
                a.timestamp_ns
            }
            RecordEntry::Event(e) => {
                event_count += 1;
                e.timestamp_ns
            }
            RecordEntry::Telemetry(t) => {
                telemetry_count += 1;
                t.timestamp_ns
            }
            RecordEntry::Ffb(f) => {
                ffb_count += 1;
                f.timestamp_ns
            }
            RecordEntry::Empty => continue,
        };
        if ts < min_ts {
            min_ts = ts;
        }
        if ts > max_ts {
            max_ts = ts;
        }
    }

    let duration_ns = if min_ts <= max_ts && min_ts != u64::MAX {
        max_ts - min_ts
    } else {
        0
    };

    RecorderSummary {
        total_entries: recorder.len() as u64,
        axis_count,
        event_count,
        telemetry_count,
        ffb_count,
        duration_ns,
        axis_range: if axis_count > 0 {
            Some((axis_min, axis_max))
        } else {
            None
        },
        overflow_count: recorder.overflow_count(),
    }
}

/// Summary statistics for a recording.
#[derive(Debug, Clone, PartialEq)]
pub struct RecorderSummary {
    pub total_entries: u64,
    pub axis_count: u64,
    pub event_count: u64,
    pub telemetry_count: u64,
    pub ffb_count: u64,
    /// Duration from first to last entry in nanoseconds.
    pub duration_ns: u64,
    /// (min, max) of processed axis values, if any axis data exists.
    pub axis_range: Option<(f64, f64)>,
    /// Number of entries lost to overflow.
    pub overflow_count: u64,
}

impl std::fmt::Display for RecorderSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Blackbox Recording Summary")?;
        writeln!(f, "=========================")?;
        writeln!(f, "Total entries : {}", self.total_entries)?;
        writeln!(f, "  Axis        : {}", self.axis_count)?;
        writeln!(f, "  Events      : {}", self.event_count)?;
        writeln!(f, "  Telemetry   : {}", self.telemetry_count)?;
        writeln!(f, "  FFB         : {}", self.ffb_count)?;
        let dur_s = self.duration_ns as f64 / 1_000_000_000.0;
        writeln!(f, "Duration      : {dur_s:.3} s")?;
        if let Some((lo, hi)) = self.axis_range {
            writeln!(f, "Axis range    : [{lo:.6}, {hi:.6}]")?;
        }
        if self.overflow_count > 0 {
            writeln!(f, "Overflows     : {}", self.overflow_count)?;
        }
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::RecorderConfig;

    fn make_recorder(cap: usize) -> BlackboxRecorder {
        BlackboxRecorder::new(RecorderConfig { capacity: cap })
    }

    #[test]
    fn export_json_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("test.json");

        let mut rec = make_recorder(64);
        rec.record_axis(1, 0.5, 0.75, 1000);
        rec.record_axis(2, -1.0, -0.5, 2000);

        export_json(&rec, &json_path).unwrap();

        let json_str = std::fs::read_to_string(&json_path).unwrap();
        let doc: RecorderExportDoc = serde_json::from_str(&json_str).unwrap();
        assert_eq!(doc.version, RecorderExportDoc::VERSION);
        assert_eq!(doc.entry_count, 2);
        match &doc.entries[0] {
            ExportEntry::Axis(a) => {
                assert_eq!(a.axis_id, 1);
                assert!((a.raw - 0.5).abs() < f64::EPSILON);
            }
            _ => panic!("expected Axis"),
        }
    }

    #[test]
    fn export_csv_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let csv_path = dir.path().join("test.csv");

        let mut rec = make_recorder(64);
        rec.record_axis(1, 0.5, 0.75, 1000);
        rec.record_axis(2, -1.0, -0.5, 2000);
        // Non-axis entries should be skipped in CSV
        rec.record_event(10, "panel", &[0x01]);

        export_csv(&rec, &csv_path).unwrap();

        let csv_str = std::fs::read_to_string(&csv_path).unwrap();
        let lines: Vec<&str> = csv_str.lines().collect();
        assert_eq!(lines[0], "timestamp_ns,axis_id,raw,processed");
        assert_eq!(lines.len(), 3); // header + 2 axis rows
        assert!(lines[1].starts_with("1000,1,"));
        assert!(lines[2].starts_with("2000,2,"));
    }

    #[test]
    fn export_binary_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("test.bin");

        let mut rec = make_recorder(64);
        rec.record_axis(1, 0.5, 0.75, 1000);
        rec.record_ffb(3, 0.9);

        export_binary(&rec, &bin_path).unwrap();

        let bytes = std::fs::read(&bin_path).unwrap();
        let doc: RecorderExportDoc = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(doc.entry_count, 2);
        assert!(matches!(&doc.entries[0], ExportEntry::Axis(_)));
        assert!(matches!(&doc.entries[1], ExportEntry::Ffb(_)));
    }

    #[test]
    fn summary_basic() {
        let mut rec = make_recorder(64);
        rec.record_axis(1, 0.1, 0.2, 1_000_000_000);
        rec.record_axis(2, 0.9, 0.95, 2_000_000_000);
        rec.record_event(1, "test", &[]);
        rec.record_ffb(1, 0.5);

        let s = summary(&rec);
        assert_eq!(s.total_entries, 4);
        assert_eq!(s.axis_count, 2);
        assert_eq!(s.event_count, 1);
        assert_eq!(s.ffb_count, 1);
        assert!(s.axis_range.is_some());
        let (lo, hi) = s.axis_range.unwrap();
        assert!((lo - 0.2).abs() < f64::EPSILON);
        assert!((hi - 0.95).abs() < f64::EPSILON);
        assert_eq!(s.overflow_count, 0);
    }

    #[test]
    fn summary_empty() {
        let rec = make_recorder(16);
        let s = summary(&rec);
        assert_eq!(s.total_entries, 0);
        assert_eq!(s.duration_ns, 0);
        assert!(s.axis_range.is_none());
    }

    #[test]
    fn summary_display() {
        let mut rec = make_recorder(16);
        rec.record_axis(1, 0.0, 0.0, 0);
        let s = summary(&rec);
        let text = format!("{s}");
        assert!(text.contains("Blackbox Recording Summary"));
        assert!(text.contains("Total entries"));
    }

    #[test]
    fn summary_with_overflow() {
        let mut rec = make_recorder(4);
        for i in 0..10u16 {
            rec.record_axis(i, 0.0, i as f64, (i as u64) * 1000);
        }
        let s = summary(&rec);
        assert_eq!(s.overflow_count, 6);
        assert_eq!(s.total_entries, 4);
    }

    #[test]
    fn json_csv_agree_on_axis_count() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("agree.json");
        let csv_path = dir.path().join("agree.csv");

        let mut rec = make_recorder(32);
        for i in 0..5 {
            rec.record_axis(i, i as f64, i as f64, i as u64 * 100);
        }
        rec.record_event(1, "x", &[]);

        export_json(&rec, &json_path).unwrap();
        export_csv(&rec, &csv_path).unwrap();

        let doc: RecorderExportDoc =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
        let csv_lines = std::fs::read_to_string(&csv_path).unwrap().lines().count() - 1; // minus header

        let json_axis = doc
            .entries
            .iter()
            .filter(|e| matches!(e, ExportEntry::Axis(_)))
            .count();
        assert_eq!(json_axis, csv_lines);
        assert_eq!(json_axis, 5);
    }
}
