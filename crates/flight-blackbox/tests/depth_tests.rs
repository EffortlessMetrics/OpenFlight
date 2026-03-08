// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the flight-blackbox crate.
//!
//! Covers record types, serialization round-trips, recording lifecycle,
//! frame format, file management, replay, ring buffer semantics,
//! compression heuristics, corruption handling, analysis utilities,
//! export formats, and property-based invariants.

use std::fs;
use std::io::Write;

use flight_blackbox::analysis::{self, Anomaly, AnomalyThresholds};
use flight_blackbox::export::{
    self, AxisRecordDto, EventRecordDto, ExportEntry, FfbRecordDto, RecorderExportDoc,
    TelemetryRecordDto,
};
use flight_blackbox::recorder::{
    BlackboxRecorder, RecordEntry, RecorderConfig, EVENT_DATA_MAX, EVENT_SOURCE_MAX, SIM_ID_MAX,
    SNAPSHOT_MAX,
};
use flight_blackbox::{
    BlackboxConfig, BlackboxError, BlackboxFooter, BlackboxHeader, BlackboxReader, BlackboxRecord,
    BlackboxWriter, ExportDoc, IndexEntry, StreamType, FBB_ENDIAN_MARKER, FBB_FORMAT_VERSION,
    FBB_MAGIC,
};

use proptest::prelude::*;
use tempfile::tempdir;

// ── Helpers ──────────────────────────────────────────────────────────

fn small_recorder(cap: usize) -> BlackboxRecorder {
    BlackboxRecorder::new(RecorderConfig { capacity: cap })
}

fn test_config(dir: &std::path::Path) -> BlackboxConfig {
    BlackboxConfig {
        output_dir: dir.to_path_buf(),
        ..BlackboxConfig::default()
    }
}

/// Yield to the tokio runtime so the async writer task can drain the channel.
async fn drain_writer() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

/// Write an FBB file using BlackboxWriter and return its path.
async fn write_test_file(
    dir: &std::path::Path,
    records: &[(u64, StreamType, Vec<u8>)],
) -> std::path::PathBuf {
    let config = test_config(dir);
    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "test_v1".into())
        .await
        .unwrap();

    for (ts, st, data) in records {
        let record = BlackboxRecord {
            timestamp_ns: *ts,
            stream_type: *st,
            data: data.clone(),
        };
        writer.submit(record).unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();
    path
}

// ═════════════════════════════════════════════════════════════════════
// 1. Record types & field access
// ═════════════════════════════════════════════════════════════════════

#[test]
fn axis_record_field_access() {
    let mut rec = small_recorder(4);
    rec.record_axis(5, -0.75, 0.8, 42_000);
    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Axis(a) => {
            assert_eq!(a.axis_id, 5);
            assert!((a.raw - (-0.75)).abs() < f64::EPSILON);
            assert!((a.processed - 0.8).abs() < f64::EPSILON);
            assert_eq!(a.timestamp_ns, 42_000);
        }
        _ => panic!("expected Axis"),
    }
}

#[test]
fn event_record_source_and_data_access() {
    let mut rec = small_recorder(4);
    rec.record_event(7, "hid-panel", &[0xAA, 0xBB]);
    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Event(e) => {
            assert_eq!(e.event_type, 7);
            assert_eq!(e.source_str(), "hid-panel");
            assert_eq!(e.data_bytes(), &[0xAA, 0xBB]);
        }
        _ => panic!("expected Event"),
    }
}

#[test]
fn event_record_empty_source_and_data() {
    let mut rec = small_recorder(4);
    rec.record_event(0, "", &[]);
    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Event(e) => {
            assert_eq!(e.source_str(), "");
            assert!(e.data_bytes().is_empty());
        }
        _ => panic!("expected Event"),
    }
}

#[test]
fn telemetry_record_field_access() {
    let mut rec = small_recorder(4);
    rec.record_telemetry("DCS", &[0xFF]);
    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Telemetry(t) => {
            assert_eq!(t.sim_str(), "DCS");
            assert_eq!(t.snapshot_bytes(), &[0xFF]);
        }
        _ => panic!("expected Telemetry"),
    }
}

#[test]
fn ffb_record_field_access() {
    let mut rec = small_recorder(4);
    rec.record_ffb(3, 0.42);
    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Ffb(f) => {
            assert_eq!(f.effect_type, 3);
            assert!((f.magnitude - 0.42).abs() < f64::EPSILON);
        }
        _ => panic!("expected Ffb"),
    }
}

#[test]
fn record_entry_variant_discrimination() {
    let mut rec = small_recorder(8);
    rec.record_axis(0, 0.0, 0.0, 0);
    rec.record_event(0, "", &[]);
    rec.record_telemetry("X", &[]);
    rec.record_ffb(0, 0.0);

    let snap = rec.snapshot();
    assert!(matches!(snap[0], RecordEntry::Axis(_)));
    assert!(matches!(snap[1], RecordEntry::Event(_)));
    assert!(matches!(snap[2], RecordEntry::Telemetry(_)));
    assert!(matches!(snap[3], RecordEntry::Ffb(_)));
}

#[test]
fn stream_type_repr_values() {
    assert_eq!(StreamType::AxisFrames as u8, 0xA);
    assert_eq!(StreamType::BusSnapshots as u8, 0xB);
    assert_eq!(StreamType::Events as u8, 0xC);
}

#[test]
fn stream_type_to_index_mapping() {
    assert_eq!(StreamType::AxisFrames.to_index(), 0);
    assert_eq!(StreamType::BusSnapshots.to_index(), 1);
    assert_eq!(StreamType::Events.to_index(), 2);
}

// ═════════════════════════════════════════════════════════════════════
// 2. Binary encoding / decoding round-trips
// ═════════════════════════════════════════════════════════════════════

#[test]
fn blackbox_header_postcard_roundtrip() {
    let header = BlackboxHeader {
        magic: *FBB_MAGIC,
        endian_marker: FBB_ENDIAN_MARKER,
        format_version: FBB_FORMAT_VERSION,
        app_version: "1.0.0".into(),
        timebase_ns: 1_700_000_000_000_000_000,
        sim_id: "MSFS".into(),
        aircraft_id: "C172".into(),
        recording_mode: "default".into(),
        start_timestamp: 12345,
    };
    let bytes = postcard::to_stdvec(&header).unwrap();
    let decoded: BlackboxHeader = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.magic, *FBB_MAGIC);
    assert_eq!(decoded.sim_id, "MSFS");
    assert_eq!(decoded.start_timestamp, 12345);
}

#[test]
fn blackbox_footer_postcard_roundtrip() {
    let footer = BlackboxFooter {
        end_timestamp: 99999,
        total_entries: [10, 20, 30],
        index_offset: 4096,
        index_len: 128,
        index_count: 5,
        crc32c: 0xDEADBEEF,
    };
    let bytes = postcard::to_stdvec(&footer).unwrap();
    let decoded: BlackboxFooter = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.end_timestamp, 99999);
    assert_eq!(decoded.total_entries, [10, 20, 30]);
    assert_eq!(decoded.crc32c, 0xDEADBEEF);
}

#[test]
fn index_entry_postcard_roundtrip() {
    let entry = IndexEntry {
        timestamp_ns: 1_000_000,
        file_offset: 8192,
        stream_counts: [100, 50, 7],
    };
    let bytes = postcard::to_stdvec(&entry).unwrap();
    let decoded: IndexEntry = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.timestamp_ns, 1_000_000);
    assert_eq!(decoded.file_offset, 8192);
    assert_eq!(decoded.stream_counts, [100, 50, 7]);
}

#[test]
fn blackbox_record_postcard_roundtrip_all_stream_types() {
    for st in [
        StreamType::AxisFrames,
        StreamType::BusSnapshots,
        StreamType::Events,
    ] {
        let record = BlackboxRecord {
            timestamp_ns: 42,
            stream_type: st,
            data: vec![0x01, 0x02, 0x03],
        };
        let bytes = postcard::to_stdvec(&record).unwrap();
        let decoded: BlackboxRecord = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.stream_type, st);
        assert_eq!(decoded.data, vec![0x01, 0x02, 0x03]);
        assert_eq!(decoded.timestamp_ns, 42);
    }
}

#[test]
fn blackbox_record_empty_data_roundtrip() {
    let record = BlackboxRecord {
        timestamp_ns: 0,
        stream_type: StreamType::Events,
        data: vec![],
    };
    let bytes = postcard::to_stdvec(&record).unwrap();
    let decoded: BlackboxRecord = postcard::from_bytes(&bytes).unwrap();
    assert!(decoded.data.is_empty());
}

#[test]
fn recorder_export_doc_postcard_roundtrip() {
    let doc = RecorderExportDoc {
        version: RecorderExportDoc::VERSION,
        entry_count: 2,
        entries: vec![
            ExportEntry::Axis(AxisRecordDto {
                axis_id: 1,
                raw: 0.5,
                processed: 0.6,
                timestamp_ns: 100,
            }),
            ExportEntry::Ffb(FfbRecordDto {
                timestamp_ns: 200,
                effect_type: 3,
                magnitude: 0.9,
            }),
        ],
    };
    let bytes = postcard::to_stdvec(&doc).unwrap();
    let decoded: RecorderExportDoc = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.version, RecorderExportDoc::VERSION);
    assert_eq!(decoded.entry_count, 2);
    assert_eq!(decoded.entries.len(), 2);
}

#[test]
fn export_entry_all_variants_roundtrip() {
    let entries = vec![
        ExportEntry::Axis(AxisRecordDto {
            axis_id: 1,
            raw: 0.1,
            processed: 0.2,
            timestamp_ns: 10,
        }),
        ExportEntry::Event(EventRecordDto {
            timestamp_ns: 20,
            event_type: 5,
            source: "test".into(),
            data: vec![0xAA],
        }),
        ExportEntry::Telemetry(TelemetryRecordDto {
            timestamp_ns: 30,
            sim: "DCS".into(),
            snapshot: vec![0x01, 0x02],
        }),
        ExportEntry::Ffb(FfbRecordDto {
            timestamp_ns: 40,
            effect_type: 2,
            magnitude: 0.7,
        }),
    ];
    for entry in &entries {
        let bytes = postcard::to_stdvec(entry).unwrap();
        let decoded: ExportEntry = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(&decoded, entry);
    }
}

// ═════════════════════════════════════════════════════════════════════
// 3. Recording Lifecycle
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn lifecycle_start_write_stop_file_exists() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(1000, &[0x01, 0x02]).unwrap();
    writer.record_axis_frame(2000, &[0x03, 0x04]).unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    assert!(path.exists(), "recording file must exist after stop");
    assert!(
        fs::metadata(&path).unwrap().len() > 0,
        "recording file must not be empty"
    );
}

#[tokio::test]
async fn lifecycle_multiple_recordings_separate_files() {
    let dir = tempdir().unwrap();

    let mut writer1 = BlackboxWriter::new(test_config(dir.path()));
    let path1 = writer1
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();
    writer1.record_axis_frame(100, &[0xAA]).unwrap();
    drain_writer().await;
    writer1.stop_recording().await.unwrap();

    // Small delay to ensure distinct timestamps in filenames
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let mut writer2 = BlackboxWriter::new(test_config(dir.path()));
    let path2 = writer2
        .start_recording("DCS".into(), "F18".into(), "v1".into())
        .await
        .unwrap();
    writer2.record_event(200, &[0xBB]).unwrap();
    drain_writer().await;
    writer2.stop_recording().await.unwrap();

    assert_ne!(
        path1, path2,
        "separate recordings must produce separate files"
    );
    assert!(path1.exists());
    assert!(path2.exists());
}

#[tokio::test]
async fn lifecycle_recording_during_active_simulation_data() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("XPLANE".into(), "B738".into(), "v1".into())
        .await
        .unwrap();

    // Simulate a burst of mixed data as during an active flight
    for i in 0u64..50 {
        let ts = i * 4_000_000; // 4ms intervals (250Hz)
        let axis_data: Vec<u8> = (i as f32).to_le_bytes().to_vec();
        writer.record_axis_frame(ts, &axis_data).unwrap();

        if i % 4 == 0 {
            writer
                .record_bus_snapshot(ts, &[0x01, 0x02, 0x03])
                .unwrap();
        }
        if i % 25 == 0 {
            writer.record_event(ts, b"profile_change").unwrap();
        }
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Verify all records are readable
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut count = 0u64;
    while reader.next_record().unwrap().is_some() {
        count += 1;
    }
    // 50 axis + 13 bus (0,4,8,...,48) + 2 events (0,25)
    assert_eq!(count, 65, "all submitted records must be retrievable");
}

#[tokio::test]
async fn lifecycle_graceful_stop_flushes_pending_records() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Write a batch of records
    for i in 0..10 {
        writer
            .record_axis_frame(i * 1000, &[i as u8])
            .unwrap();
    }
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // All records should be present after graceful stop
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let mut count = 0;
    while reader.next_record().unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 10);
}

#[tokio::test]
async fn write_empty_recording_read_header() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let h = reader.header();
    assert_eq!(h.magic, *FBB_MAGIC);
    assert_eq!(h.sim_id, "MSFS");
    assert_eq!(h.aircraft_id, "C172");
    assert!(reader.next_record().unwrap().is_none());
}

// ═════════════════════════════════════════════════════════════════════
// 4. Frame Format
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn format_header_fields_correct() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("DCS".into(), "FA18".into(), "v2.1.0".into())
        .await
        .unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let header = reader.header();

    assert_eq!(header.magic, *FBB_MAGIC);
    assert_eq!(header.endian_marker, FBB_ENDIAN_MARKER);
    assert_eq!(header.format_version, FBB_FORMAT_VERSION);
    assert_eq!(header.app_version, "v2.1.0");
    assert_eq!(header.sim_id, "DCS");
    assert_eq!(header.aircraft_id, "FA18");
    assert!(header.timebase_ns > 0, "timebase must be set");
}

#[tokio::test]
async fn format_axis_values_stored_as_raw_bytes() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Store f32 axis values as raw bytes
    let axis_value: f32 = 0.75;
    let axis_bytes = axis_value.to_le_bytes();
    writer.record_axis_frame(1000, &axis_bytes).unwrap();

    let neg_value: f32 = -0.5;
    let neg_bytes = neg_value.to_le_bytes();
    writer.record_axis_frame(2000, &neg_bytes).unwrap();

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let rec1 = reader.next_record().unwrap().unwrap();
    assert_eq!(rec1.stream_type, StreamType::AxisFrames);
    let recovered: f32 = f32::from_le_bytes(rec1.data[..4].try_into().unwrap());
    assert!((recovered - 0.75).abs() < f32::EPSILON);

    let rec2 = reader.next_record().unwrap().unwrap();
    let recovered2: f32 = f32::from_le_bytes(rec2.data[..4].try_into().unwrap());
    assert!((recovered2 - (-0.5)).abs() < f32::EPSILON);
}

#[tokio::test]
async fn format_button_states_as_bitfield() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Encode button states as a bitfield: buttons 0,2,7 pressed
    let button_state: u8 = (1 << 0) | (1 << 2) | (1 << 7);
    writer.record_event(5000, &[button_state]).unwrap();

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let rec = reader.next_record().unwrap().unwrap();
    assert_eq!(rec.stream_type, StreamType::Events);
    let recovered = rec.data[0];
    assert!(recovered & (1 << 0) != 0, "button 0 should be pressed");
    assert!(recovered & (1 << 2) != 0, "button 2 should be pressed");
    assert!(recovered & (1 << 7) != 0, "button 7 should be pressed");
    assert!(recovered & (1 << 1) == 0, "button 1 should not be pressed");
}

#[tokio::test]
async fn format_metadata_aircraft_sim_profile() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("XPLANE".into(), "B777".into(), "v3.0".into())
        .await
        .unwrap();

    // Store metadata as a JSON event payload
    let metadata = br#"{"profile":"IFR","phase":"cruise"}"#;
    writer.record_event(100, metadata).unwrap();

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let header = reader.header();
    assert_eq!(header.sim_id, "XPLANE");
    assert_eq!(header.aircraft_id, "B777");

    let rec = reader.next_record().unwrap().unwrap();
    let payload = std::str::from_utf8(&rec.data).unwrap();
    assert!(payload.contains("\"profile\":\"IFR\""));
}

#[tokio::test]
async fn format_timestamp_and_sequence_preserved() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    let timestamps: Vec<u64> = (0..20).map(|i| i * 4_000_000).collect();
    for (i, &ts) in timestamps.iter().enumerate() {
        writer
            .record_axis_frame(ts, &[i as u8])
            .unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    for (i, &expected_ts) in timestamps.iter().enumerate() {
        let rec = reader.next_record().unwrap().unwrap();
        assert_eq!(
            rec.timestamp_ns, expected_ts,
            "timestamp mismatch at frame {i}"
        );
        assert_eq!(rec.data, vec![i as u8], "data mismatch at frame {i}");
    }
    assert!(reader.next_record().unwrap().is_none());
}

// ═════════════════════════════════════════════════════════════════════
// 5. File Management
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn file_directory_creation_if_missing() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("deeply").join("nested").join("dir");
    assert!(!nested.exists());

    let config = BlackboxConfig {
        output_dir: nested.clone(),
        ..BlackboxConfig::default()
    };

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(100, &[0x01]).unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    assert!(
        nested.exists(),
        "output directory must be created automatically"
    );
    assert!(
        path.exists(),
        "recording file must exist in created directory"
    );
}

#[tokio::test]
async fn file_naming_convention() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let filename = path.file_name().unwrap().to_str().unwrap();
    assert!(
        filename.starts_with("flight_"),
        "filename must start with 'flight_'"
    );
    assert!(filename.ends_with(".fbb"), "filename must end with '.fbb'");
}

#[tokio::test]
async fn file_fbb_extension() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("DCS".into(), "A10".into(), "v1".into())
        .await
        .unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    assert_eq!(
        path.extension().unwrap().to_str().unwrap(),
        "fbb",
        "recording must use .fbb extension"
    );
}

#[tokio::test]
async fn start_recording_returns_fbb_path() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "A320".into(), "v2".into())
        .await
        .unwrap();

    assert!(
        path.extension().unwrap() == "fbb",
        "output should be .fbb file"
    );
    assert!(path.starts_with(dir.path()));

    drain_writer().await;
    writer.stop_recording().await.unwrap();
}

// ═════════════════════════════════════════════════════════════════════
// 6. Replay / Reading
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn replay_load_recording_iterate_frames() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    let frame_count = 30;
    for i in 0..frame_count {
        let ts = i as u64 * 4_000_000;
        writer
            .record_axis_frame(ts, &(i as f32).to_le_bytes())
            .unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Replay: iterate all frames
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut replayed = Vec::new();
    while let Some(rec) = reader.next_record().unwrap() {
        replayed.push(rec);
    }

    assert_eq!(replayed.len(), frame_count);
    for (i, rec) in replayed.iter().enumerate() {
        let expected_ts = i as u64 * 4_000_000;
        assert_eq!(rec.timestamp_ns, expected_ts);
        assert_eq!(rec.stream_type, StreamType::AxisFrames);
    }
}

#[tokio::test]
async fn replay_seek_to_timestamp_via_scan() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Write 100 frames at 4ms intervals
    for i in 0u64..100 {
        let ts = i * 4_000_000;
        writer.record_axis_frame(ts, &[i as u8]).unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Seek to a specific timestamp by scanning
    let target_ts: u64 = 50 * 4_000_000; // seek to frame 50
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut found = None;
    while let Some(rec) = reader.next_record().unwrap() {
        if rec.timestamp_ns >= target_ts {
            found = Some(rec);
            break;
        }
    }

    let frame = found.expect("should find frame at target timestamp");
    assert_eq!(frame.timestamp_ns, target_ts);
    assert_eq!(frame.data, vec![50u8]);
}

#[tokio::test]
async fn replay_playback_speed_simulation() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    let cadence_ns: u64 = 4_000_000; // 4ms
    for i in 0u64..10 {
        writer
            .record_axis_frame(i * cadence_ns, &[i as u8])
            .unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Load all frames
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let mut frames = Vec::new();
    while let Some(rec) = reader.next_record().unwrap() {
        frames.push(rec);
    }

    // Verify playback speed calculations (1x, 2x, 0.5x)
    for &speed in &[1.0f64, 2.0, 0.5] {
        for window in frames.windows(2) {
            let real_delta = window[1].timestamp_ns - window[0].timestamp_ns;
            let playback_delta = (real_delta as f64 / speed) as u64;

            if speed > 1.0 {
                assert!(
                    playback_delta < real_delta,
                    "faster playback means shorter intervals"
                );
            } else if speed < 1.0 {
                assert!(
                    playback_delta > real_delta,
                    "slower playback means longer intervals"
                );
            } else {
                assert_eq!(playback_delta, real_delta, "1x speed preserves intervals");
            }
        }
    }
}

#[tokio::test]
async fn replay_forward_and_backward_iteration() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    for i in 0u64..20 {
        writer
            .record_axis_frame(i * 4_000_000, &[i as u8])
            .unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Forward pass
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let mut forward_frames = Vec::new();
    while let Some(rec) = reader.next_record().unwrap() {
        forward_frames.push(rec);
    }

    assert_eq!(forward_frames.len(), 20);

    // Verify forward order
    for window in forward_frames.windows(2) {
        assert!(
            window[0].timestamp_ns <= window[1].timestamp_ns,
            "forward iteration must be chronological"
        );
    }

    // Backward iteration via reverse
    let backward_frames: Vec<_> = forward_frames.iter().rev().collect();
    for window in backward_frames.windows(2) {
        assert!(
            window[0].timestamp_ns >= window[1].timestamp_ns,
            "backward iteration must be reverse-chronological"
        );
    }
}

#[tokio::test]
async fn write_single_record_read_back() {
    let dir = tempdir().unwrap();
    let path = write_test_file(
        dir.path(),
        &[(1000, StreamType::AxisFrames, vec![0xDE, 0xAD])],
    )
    .await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let rec = reader.next_record().unwrap().unwrap();
    assert_eq!(rec.timestamp_ns, 1000);
    assert_eq!(rec.stream_type, StreamType::AxisFrames);
    assert_eq!(rec.data, vec![0xDE, 0xAD]);
    assert!(reader.next_record().unwrap().is_none());
}

#[tokio::test]
async fn write_multiple_stream_types_read_all() {
    let dir = tempdir().unwrap();
    let records = vec![
        (100, StreamType::AxisFrames, vec![0x01]),
        (200, StreamType::BusSnapshots, vec![0x02]),
        (300, StreamType::Events, vec![0x03]),
        (400, StreamType::AxisFrames, vec![0x04]),
    ];
    let path = write_test_file(dir.path(), &records).await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut read_records = Vec::new();
    while let Some(rec) = reader.next_record().unwrap() {
        read_records.push(rec);
    }
    assert_eq!(read_records.len(), 4);
    assert_eq!(read_records[0].stream_type, StreamType::AxisFrames);
    assert_eq!(read_records[1].stream_type, StreamType::BusSnapshots);
    assert_eq!(read_records[2].stream_type, StreamType::Events);
    assert_eq!(read_records[3].stream_type, StreamType::AxisFrames);
}

#[tokio::test]
async fn write_many_records_preserves_order() {
    let dir = tempdir().unwrap();
    let records: Vec<_> = (0..50)
        .map(|i| {
            (
                i as u64 * 1000,
                StreamType::AxisFrames,
                vec![(i & 0xFF) as u8],
            )
        })
        .collect();
    let path = write_test_file(dir.path(), &records).await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut prev_ts = 0u64;
    let mut count = 0;
    while let Some(rec) = reader.next_record().unwrap() {
        assert!(
            rec.timestamp_ns >= prev_ts,
            "records must be in timestamp order"
        );
        prev_ts = rec.timestamp_ns;
        count += 1;
    }
    assert_eq!(count, 50);
}

#[tokio::test]
async fn reader_validates_magic() {
    let dir = tempdir().unwrap();
    let path = write_test_file(dir.path(), &[]).await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    assert_eq!(reader.header().magic, *FBB_MAGIC);
}

#[tokio::test]
async fn reader_validates_endian_marker() {
    let dir = tempdir().unwrap();
    let path = write_test_file(dir.path(), &[]).await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    assert_eq!(reader.header().endian_marker, FBB_ENDIAN_MARKER);
}

#[tokio::test]
async fn reader_validates_format_version() {
    let dir = tempdir().unwrap();
    let path = write_test_file(dir.path(), &[]).await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    assert_eq!(reader.header().format_version, FBB_FORMAT_VERSION);
}

// ═════════════════════════════════════════════════════════════════════
// 7. Ring buffer semantics
// ═════════════════════════════════════════════════════════════════════

#[test]
fn ring_buffer_fill_to_exact_capacity() {
    let mut rec = small_recorder(5);
    for i in 0..5 {
        rec.record_axis(i, 0.0, 0.0, i as u64);
    }
    assert_eq!(rec.len(), 5);
    assert_eq!(rec.total_written(), 5);
    assert_eq!(rec.overflow_count(), 0);
}

#[test]
fn ring_buffer_overflow_preserves_newest() {
    let mut rec = small_recorder(3);
    for i in 0..7u16 {
        rec.record_axis(i, i as f64, i as f64, i as u64 * 100);
    }
    assert_eq!(rec.len(), 3);
    assert_eq!(rec.total_written(), 7);
    assert_eq!(rec.overflow_count(), 4);

    let ids: Vec<u16> = rec
        .snapshot()
        .iter()
        .map(|e| match e {
            RecordEntry::Axis(a) => a.axis_id,
            _ => panic!("expected Axis"),
        })
        .collect();
    assert_eq!(ids, vec![4, 5, 6]);
}

#[test]
fn ring_buffer_massive_overflow() {
    let mut rec = small_recorder(2);
    for i in 0..100_000u64 {
        rec.record_axis(0, 0.0, i as f64, i);
    }
    assert_eq!(rec.len(), 2);
    assert_eq!(rec.overflow_count(), 99_998);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Axis(a) => assert_eq!(a.timestamp_ns, 99_998),
        _ => panic!("expected Axis"),
    }
    match &snap[1] {
        RecordEntry::Axis(a) => assert_eq!(a.timestamp_ns, 99_999),
        _ => panic!("expected Axis"),
    }
}

#[test]
fn ring_buffer_clear_resets_all_state() {
    let mut rec = small_recorder(8);
    for i in 0..10 {
        rec.record_axis(i, 0.0, 0.0, i as u64);
    }
    assert!(!rec.is_empty());

    rec.clear();
    assert!(rec.is_empty());
    assert_eq!(rec.len(), 0);
    assert_eq!(rec.total_written(), 0);
    assert_eq!(rec.overflow_count(), 0);
    assert_eq!(rec.capacity(), 8);
    assert!(rec.snapshot().is_empty());
}

#[test]
fn ring_buffer_zero_capacity_clamped() {
    let rec = BlackboxRecorder::new(RecorderConfig { capacity: 0 });
    assert_eq!(rec.capacity(), 1);
}

#[test]
fn ring_buffer_iterator_exact_size() {
    let mut rec = small_recorder(10);
    for i in 0..7 {
        rec.record_axis(i, 0.0, 0.0, 0);
    }
    let iter = rec.iter();
    assert_eq!(iter.len(), 7);
    assert_eq!(iter.size_hint(), (7, Some(7)));
}

#[test]
fn ring_buffer_mixed_types_preserved() {
    let mut rec = small_recorder(32);
    rec.record_axis(1, 0.5, 0.6, 100);
    rec.record_event(10, "panel", &[0x01]);
    rec.record_telemetry("DCS", &[0xAA, 0xBB]);
    rec.record_ffb(3, 0.9);

    let snap = rec.snapshot();
    assert_eq!(snap.len(), 4);
    assert!(matches!(snap[0], RecordEntry::Axis(_)));
    assert!(matches!(snap[1], RecordEntry::Event(_)));
    assert!(matches!(snap[2], RecordEntry::Telemetry(_)));
    assert!(matches!(snap[3], RecordEntry::Ffb(_)));
}

#[test]
fn ring_buffer_snapshot_chronological_after_wrap() {
    let mut rec = small_recorder(4);
    for i in 0..6u16 {
        rec.record_axis(i, 0.0, 0.0, i as u64 * 1000);
    }

    let snap = rec.snapshot();
    let timestamps: Vec<u64> = snap
        .iter()
        .map(|e| match e {
            RecordEntry::Axis(a) => a.timestamp_ns,
            _ => panic!("expected Axis"),
        })
        .collect();
    for window in timestamps.windows(2) {
        assert!(window[0] <= window[1], "snapshot must be chronological");
    }
}

#[test]
fn ring_buffer_no_reallocation() {
    let mut rec = small_recorder(64);
    let cap = rec.capacity();
    for i in 0..500u16 {
        rec.record_axis(i, 0.0, 0.0, i as u64);
    }
    assert_eq!(rec.capacity(), cap, "capacity must not change");
    assert_eq!(rec.len(), 64);
}

// ═════════════════════════════════════════════════════════════════════
// 8. Timestamp ordering and precision
// ═════════════════════════════════════════════════════════════════════

#[test]
fn to_ns_from_ms_conversions() {
    assert_eq!(flight_blackbox::to_ns_from_ms(0), 0);
    assert_eq!(flight_blackbox::to_ns_from_ms(1), 1_000_000);
    assert_eq!(flight_blackbox::to_ns_from_ms(1000), 1_000_000_000);
}

#[test]
fn to_ns_from_ms_saturates() {
    assert_eq!(flight_blackbox::to_ns_from_ms(u64::MAX), u64::MAX);
}

#[test]
fn axis_record_preserves_exact_timestamp() {
    let mut rec = small_recorder(8);
    let ts = 123_456_789_012_345u64;
    rec.record_axis(1, 0.0, 0.0, ts);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Axis(a) => assert_eq!(a.timestamp_ns, ts),
        _ => panic!("expected Axis"),
    }
}

#[test]
fn monotonic_timestamps_non_decreasing() {
    let mut rec = small_recorder(32);
    for _ in 0..10 {
        rec.record_event(1, "test", &[]);
    }

    let snap = rec.snapshot();
    let timestamps: Vec<u64> = snap
        .iter()
        .map(|e| match e {
            RecordEntry::Event(ev) => ev.timestamp_ns,
            _ => panic!("expected Event"),
        })
        .collect();

    for window in timestamps.windows(2) {
        assert!(
            window[0] <= window[1],
            "monotonic timestamps must not decrease"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 9. Compression / Encoding Heuristics
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn compression_delta_encoding_for_axis_values() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Write slowly changing axis values (high delta-encoding potential)
    let values: Vec<f32> = (0..100).map(|i| 0.5 + (i as f32) * 0.001).collect();
    for (i, &val) in values.iter().enumerate() {
        writer
            .record_axis_frame(i as u64 * 4_000_000, &val.to_le_bytes())
            .unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Compute delta encoding efficiency: consecutive deltas should be small
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let mut prev: Option<f32> = None;
    let mut deltas = Vec::new();

    while let Some(rec) = reader.next_record().unwrap() {
        let val = f32::from_le_bytes(rec.data[..4].try_into().unwrap());
        if let Some(p) = prev {
            deltas.push((val - p).abs());
        }
        prev = Some(val);
    }

    // All deltas should be approximately 0.001
    for &d in &deltas {
        assert!(
            d < 0.01,
            "delta {d} exceeds expected range for slowly changing data"
        );
    }
}

#[tokio::test]
async fn compression_rle_for_button_states() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Simulate button state: long runs of same state (RLE-friendly)
    let states: Vec<u8> = std::iter::repeat(0x05)
        .take(50)
        .chain(std::iter::repeat(0x07).take(50))
        .collect();

    for (i, &state) in states.iter().enumerate() {
        writer
            .record_event(i as u64 * 4_000_000, &[state])
            .unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Read back and verify RLE-compressible pattern
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut read_states = Vec::new();
    while let Some(rec) = reader.next_record().unwrap() {
        read_states.push(rec.data[0]);
    }

    assert_eq!(read_states.len(), 100);

    // Count runs: should be exactly 2 runs for RLE
    let mut run_count = 1;
    for w in read_states.windows(2) {
        if w[0] != w[1] {
            run_count += 1;
        }
    }
    assert_eq!(
        run_count, 2,
        "data should have exactly 2 runs (RLE-friendly)"
    );
}

#[tokio::test]
async fn compression_ratio_within_expected_range() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Write enough data to get a meaningful file size measurement
    let total_raw_bytes: usize = 200 * 4; // 200 f32 values
    for i in 0u64..200 {
        let val = (i as f32 * 0.01).sin();
        writer
            .record_axis_frame(i * 4_000_000, &val.to_le_bytes())
            .unwrap();
    }

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let file_size = fs::metadata(&path).unwrap().len() as usize;
    // File includes header + length prefixes + serialization overhead,
    // so it will be larger than raw payload. Verify it's within a
    // reasonable range (< 20x the raw payload).
    assert!(
        file_size < total_raw_bytes * 20,
        "file size {file_size} exceeds 20x raw payload {total_raw_bytes}"
    );
    assert!(
        file_size > total_raw_bytes,
        "file size {file_size} should exceed raw payload {total_raw_bytes} due to framing"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 10. Corruption Handling
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn corruption_truncated_file_reads_available() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    for i in 0u64..10 {
        writer.record_axis_frame(i * 1000, &[i as u8]).unwrap();
    }
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let full_size = fs::metadata(&path).unwrap().len();

    // Truncate the file to remove some records but keep the header
    let truncated_size = full_size * 2 / 3;
    let data = fs::read(&path).unwrap();
    let truncated_path = dir.path().join("truncated.fbb");
    fs::write(&truncated_path, &data[..truncated_size as usize]).unwrap();

    // Should be able to read partial data
    let mut reader = BlackboxReader::open(&truncated_path).unwrap();
    reader.validate().unwrap();

    let mut count = 0;
    loop {
        match reader.next_record() {
            Ok(Some(_)) => count += 1,
            Ok(None) | Err(_) => break,
        }
    }

    assert!(
        count > 0,
        "truncated file should yield at least some records"
    );
    assert!(
        count < 10,
        "truncated file should yield fewer records than the original"
    );
}

#[tokio::test]
async fn corruption_corrupted_frame_detected() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Write enough records with larger payloads so the file is big enough
    // to corrupt record data without touching the header.
    for i in 0u64..20 {
        writer
            .record_axis_frame(i * 1000, &[i as u8; 32])
            .unwrap();
    }
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    // Read the original to confirm all 20 records are readable
    let mut orig_reader = BlackboxReader::open(&path).unwrap();
    orig_reader.validate().unwrap();
    let mut orig_count = 0;
    while orig_reader.next_record().unwrap().is_some() {
        orig_count += 1;
    }
    assert_eq!(orig_count, 20);

    // Corrupt bytes well past the header (last third of the file)
    let data = fs::read(&path).unwrap();
    let corrupt_start = data.len() * 2 / 3;
    let mut corrupted = data.clone();
    for byte in corrupted[corrupt_start..corrupt_start + 8].iter_mut() {
        *byte ^= 0xFF;
    }
    let corrupted_path = dir.path().join("corrupted.fbb");
    fs::write(&corrupted_path, &corrupted).unwrap();

    // Reader should encounter an error or fewer records
    let mut reader = BlackboxReader::open(&corrupted_path).unwrap();
    reader.validate().unwrap();

    let mut good_records = 0;
    let mut hit_error = false;
    loop {
        match reader.next_record() {
            Ok(Some(_)) => good_records += 1,
            Ok(None) => break,
            Err(_) => {
                hit_error = true;
                break;
            }
        }
    }

    // Corruption should manifest as either fewer records or an error
    assert!(
        good_records < 20 || hit_error,
        "corruption must be detected: got {good_records} good records, error={hit_error}"
    );
}

#[tokio::test]
async fn corruption_invalid_header_clear_error() {
    let dir = tempdir().unwrap();
    let bad_path = dir.path().join("bad_header.fbb");

    // Write garbage that won't deserialize as a valid header
    let mut f = fs::File::create(&bad_path).unwrap();
    let garbage_len: u32 = 8;
    f.write_all(&garbage_len.to_le_bytes()).unwrap();
    f.write_all(&[0xFF; 8]).unwrap();
    f.flush().unwrap();
    drop(f);

    let result = BlackboxReader::open(&bad_path);
    assert!(result.is_err(), "invalid header must produce a clear error");

    let err_msg = format!("{}", result.err().unwrap());
    assert!(!err_msg.is_empty(), "error message must not be empty");
}

#[tokio::test]
async fn corruption_zero_length_file_produces_error() {
    let dir = tempdir().unwrap();
    let empty_path = dir.path().join("empty.fbb");
    fs::write(&empty_path, b"").unwrap();

    let result = BlackboxReader::open(&empty_path);
    assert!(
        result.is_err(),
        "zero-length file must produce an error on open"
    );
}

#[tokio::test]
async fn corruption_header_only_no_records() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();
    // No records written
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    // Should return None immediately (no records)
    assert!(reader.next_record().unwrap().is_none());
}

#[test]
fn reader_rejects_empty_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.fbb");
    std::fs::write(&path, b"").unwrap();
    assert!(BlackboxReader::open(&path).is_err());
}

#[test]
fn reader_rejects_truncated_length_prefix() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("short.fbb");
    std::fs::write(&path, [0x01, 0x02]).unwrap();
    assert!(BlackboxReader::open(&path).is_err());
}

#[test]
fn reader_rejects_truncated_header_payload() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("trunc_header.fbb");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(&1000u32.to_le_bytes()).unwrap();
    f.write_all(&[0xAA; 4]).unwrap();
    f.flush().unwrap();
    assert!(BlackboxReader::open(&path).is_err());
}

#[test]
fn reader_rejects_corrupt_postcard_payload() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt.fbb");
    let garbage = vec![0xFF; 64];
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(&(garbage.len() as u32).to_le_bytes()).unwrap();
    f.write_all(&garbage).unwrap();
    f.flush().unwrap();
    assert!(BlackboxReader::open(&path).is_err());
}

#[tokio::test]
async fn reader_handles_truncated_record_payload() {
    let dir = tempdir().unwrap();
    let path = write_test_file(
        dir.path(),
        &[(100, StreamType::AxisFrames, vec![0x01, 0x02])],
    )
    .await;

    // Truncate the file to corrupt the last record
    let metadata = std::fs::metadata(&path).unwrap();
    let truncated_len = metadata.len() - 2;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap();
    file.set_len(truncated_len).unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    let result = reader.next_record();
    // Should either error or return None — not panic
    assert!(result.is_err() || result.unwrap().is_none());
}

// ═════════════════════════════════════════════════════════════════════
// 11. Error handling
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn lifecycle_double_start_returns_error() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    writer
        .start("v1".into(), "MSFS".into(), "C172".into(), "test".into())
        .await
        .unwrap();

    let result = writer
        .start("v1".into(), "MSFS".into(), "C172".into(), "test".into())
        .await;
    assert!(result.is_err(), "double start must return an error");

    writer.stop().await.unwrap();
}

#[tokio::test]
async fn lifecycle_stop_without_start_returns_error() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let result = writer.stop().await;
    assert!(result.is_err(), "stop without start must return an error");
}

#[test]
fn blackbox_error_display_io() {
    let err = BlackboxError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
    let msg = format!("{err}");
    assert!(msg.contains("IO error"));
}

#[test]
fn blackbox_error_display_already_started() {
    let err = BlackboxError::AlreadyStarted;
    assert_eq!(format!("{err}"), "Writer already started");
}

#[test]
fn blackbox_error_display_not_started() {
    let err = BlackboxError::NotStarted;
    assert_eq!(format!("{err}"), "Writer not started");
}

#[test]
fn blackbox_error_display_buffer_overflow() {
    let err = BlackboxError::BufferOverflow;
    assert!(format!("{err}").contains("overflow"));
}

#[test]
fn blackbox_error_display_corruption() {
    let err = BlackboxError::CorruptionDetected {
        expected: 0x12345678,
        actual: 0xDEADBEEF,
    };
    let msg = format!("{err}");
    assert!(msg.contains("12345678"));
    assert!(msg.contains("deadbeef"));
}

#[test]
fn blackbox_config_defaults_are_sane() {
    let cfg = BlackboxConfig::default();
    assert!(cfg.max_file_size_mb > 0);
    assert!(!cfg.enable_compression);
    assert!(cfg.buffer_size > 0);
    assert!(cfg.max_recording_duration.as_secs() >= 60);
}

#[test]
fn blackbox_writer_not_running_before_start() {
    let writer = BlackboxWriter::new(BlackboxConfig::default());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(async {
        let mut w = writer;
        w.stop().await
    });
    assert!(result.is_err());
}

// ═════════════════════════════════════════════════════════════════════
// 12. Analysis module depth tests
// ═════════════════════════════════════════════════════════════════════

#[test]
fn anomaly_detection_clean_data_no_anomalies() {
    let mut rec = small_recorder(128);
    let cadence = 4_000_000u64;
    for i in 0..100 {
        rec.record_axis(1, 0.0, 0.5, i * cadence);
    }
    let thresholds = AnomalyThresholds {
        max_jitter_ns: 1_000_000,
        saturation_threshold: 0.999,
        max_gap_ns: 20_000_000,
    };
    let anomalies = analysis::detect_anomalies(&rec, &thresholds);
    assert!(anomalies.is_empty());
}

#[test]
fn anomaly_detection_saturation_at_boundary() {
    let mut rec = small_recorder(32);
    rec.record_axis(1, 0.0, 0.999, 1_000_000);
    rec.record_axis(1, 0.0, 1.0, 2_000_000);
    rec.record_axis(1, 0.0, 0.5, 3_000_000);

    let thresholds = AnomalyThresholds {
        saturation_threshold: 0.999,
        max_jitter_ns: u64::MAX,
        max_gap_ns: u64::MAX,
    };
    let anomalies = analysis::detect_anomalies(&rec, &thresholds);
    let sats: Vec<_> = anomalies
        .iter()
        .filter(|a| matches!(a, Anomaly::Saturation { .. }))
        .collect();
    assert_eq!(sats.len(), 2);
}

#[test]
fn anomaly_detection_disconnect_large_gap() {
    let mut rec = small_recorder(32);
    rec.record_axis(1, 0.0, 0.0, 1_000_000);
    rec.record_axis(1, 0.0, 0.0, 2_000_000);
    rec.record_axis(1, 0.0, 0.0, 52_000_000);

    let thresholds = AnomalyThresholds {
        max_gap_ns: 20_000_000,
        saturation_threshold: 2.0,
        max_jitter_ns: u64::MAX,
    };
    let anomalies = analysis::detect_anomalies(&rec, &thresholds);
    let discs: Vec<_> = anomalies
        .iter()
        .filter(|a| matches!(a, Anomaly::Disconnect { .. }))
        .collect();
    assert_eq!(discs.len(), 1);
}

#[test]
fn axis_statistics_known_distribution() {
    let mut rec = small_recorder(64);
    let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
    for (i, v) in values.iter().enumerate() {
        rec.record_axis(1, *v, *v, (i as u64) * 1000);
    }

    let stats = analysis::axis_statistics(&rec, 1).unwrap();
    assert_eq!(stats.count, 10);
    assert!((stats.min - 1.0).abs() < f64::EPSILON);
    assert!((stats.max - 10.0).abs() < f64::EPSILON);
    assert!((stats.mean - 5.5).abs() < 0.001);
    assert!((stats.p99 - 10.0).abs() < f64::EPSILON);
}

#[test]
fn axis_statistics_returns_none_for_missing_axis() {
    let rec = small_recorder(8);
    assert!(analysis::axis_statistics(&rec, 99).is_none());
}

#[test]
fn event_timeline_sorted_and_excludes_axis() {
    let mut rec = small_recorder(32);
    rec.record_axis(1, 0.0, 0.0, 1000);
    rec.record_ffb(1, 0.5);
    rec.record_event(1, "test", &[]);
    rec.record_telemetry("MSFS", &[0x01]);

    let tl = analysis::event_timeline(&rec);
    assert_eq!(tl.len(), 3);
    for window in tl.windows(2) {
        assert!(window[0].timestamp_ns <= window[1].timestamp_ns);
    }
}

#[test]
fn event_timeline_empty_recorder() {
    let rec = small_recorder(8);
    assert!(analysis::event_timeline(&rec).is_empty());
}

// ═════════════════════════════════════════════════════════════════════
// 13. Export module depth tests
// ═════════════════════════════════════════════════════════════════════

#[test]
fn export_json_writes_valid_json() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("out.json");
    let mut rec = small_recorder(16);
    rec.record_axis(1, 0.5, 0.6, 1000);
    rec.record_event(2, "src", &[0xAA]);

    export::export_json(&rec, &path).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["version"], RecorderExportDoc::VERSION);
    assert_eq!(json["entry_count"], 2);
}

#[test]
fn export_csv_header_and_axis_only() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("out.csv");
    let mut rec = small_recorder(16);
    rec.record_axis(1, 0.5, 0.6, 1000);
    rec.record_event(2, "src", &[0xAA]);
    rec.record_axis(2, -1.0, -0.5, 2000);

    export::export_csv(&rec, &path).unwrap();
    let csv = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = csv.lines().collect();
    assert_eq!(lines[0], "timestamp_ns,axis_id,raw,processed");
    assert_eq!(lines.len(), 3);
}

#[test]
fn export_binary_roundtrip_with_all_types() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("out.bin");
    let mut rec = small_recorder(16);
    rec.record_axis(1, 0.5, 0.6, 100);
    rec.record_event(2, "src", &[0xAA]);
    rec.record_telemetry("DCS", &[0x01]);
    rec.record_ffb(3, 0.9);

    export::export_binary(&rec, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let doc: RecorderExportDoc = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(doc.entry_count, 4);
    assert!(matches!(doc.entries[0], ExportEntry::Axis(_)));
    assert!(matches!(doc.entries[1], ExportEntry::Event(_)));
    assert!(matches!(doc.entries[2], ExportEntry::Telemetry(_)));
    assert!(matches!(doc.entries[3], ExportEntry::Ffb(_)));
}

#[test]
fn summary_counts_all_types() {
    let mut rec = small_recorder(32);
    rec.record_axis(1, 0.0, 0.0, 100);
    rec.record_axis(2, 0.0, 0.0, 200);
    rec.record_event(1, "x", &[]);
    rec.record_telemetry("MSFS", &[]);
    rec.record_ffb(1, 0.5);

    let s = export::summary(&rec);
    assert_eq!(s.total_entries, 5);
    assert_eq!(s.axis_count, 2);
    assert_eq!(s.event_count, 1);
    assert_eq!(s.telemetry_count, 1);
    assert_eq!(s.ffb_count, 1);
}

#[test]
fn summary_display_format() {
    let mut rec = small_recorder(8);
    rec.record_axis(1, 0.0, 0.5, 1_000_000_000);
    rec.record_axis(1, 0.0, 0.9, 2_000_000_000);

    let s = export::summary(&rec);
    let text = format!("{s}");
    assert!(text.contains("Blackbox Recording Summary"));
    assert!(text.contains("Total entries"));
    assert!(text.contains("Axis range"));
}

#[test]
fn summary_with_overflow() {
    let mut rec = small_recorder(4);
    for i in 0..10u16 {
        rec.record_axis(i, 0.0, i as f64, (i as u64) * 1000);
    }
    let s = export::summary(&rec);
    assert_eq!(s.overflow_count, 6);
    assert_eq!(s.total_entries, 4);
}

// ═════════════════════════════════════════════════════════════════════
// 14. Export Integration
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn export_full_roundtrip_with_all_stream_types() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("DCS".into(), "SU27".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(100, &[0x01, 0x02]).unwrap();
    writer.record_axis_frame(200, &[0x03, 0x04]).unwrap();
    writer.record_bus_snapshot(300, &[0x10]).unwrap();
    writer.record_event(400, b"gear_down").unwrap();
    writer.record_event(500, b"flaps_30").unwrap();

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let doc = reader.export(false).unwrap();
    assert_eq!(doc.export_version, ExportDoc::VERSION);
    assert_eq!(doc.summary.axis_frames, 2);
    assert_eq!(doc.summary.bus_snapshots, 1);
    assert_eq!(doc.summary.events, 2);
    assert_eq!(doc.summary.total_records, 5);
    assert_eq!(doc.header.sim_id, "DCS");
    assert_eq!(doc.header.aircraft_id, "SU27");
}

#[tokio::test]
async fn export_sanitized_redacts_aircraft() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "SECRET_AIRCRAFT".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(100, &[0x01]).unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let doc = reader.export(true).unwrap();
    assert_eq!(
        doc.header.aircraft_id, "[REDACTED]",
        "sanitized export must redact aircraft_id"
    );
}

#[tokio::test]
async fn export_json_serializable() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(100, &[0xAB]).unwrap();
    writer.record_event(200, &[0xCD]).unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let doc = reader.export(false).unwrap();
    let json = serde_json::to_string_pretty(&doc).unwrap();

    assert!(json.contains("\"export_version\""));
    assert!(json.contains("\"records\""));
    assert!(json.contains("\"summary\""));

    // Deserialize back
    let roundtrip: ExportDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.summary.total_records, 2);
}

// ═════════════════════════════════════════════════════════════════════
// 15. Export doc (FBB file-level) depth tests
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn export_doc_sanitized_redacts_aircraft() {
    let dir = tempdir().unwrap();
    let path = write_test_file(
        dir.path(),
        &[(100, StreamType::AxisFrames, vec![0x01])],
    )
    .await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(true).unwrap();
    assert_eq!(doc.header.aircraft_id, "[REDACTED]");
}

#[tokio::test]
async fn export_doc_unsanitized_preserves_aircraft() {
    let dir = tempdir().unwrap();
    let path = write_test_file(
        dir.path(),
        &[(100, StreamType::AxisFrames, vec![0x01])],
    )
    .await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(false).unwrap();
    assert_eq!(doc.header.aircraft_id, "C172");
}

#[tokio::test]
async fn export_doc_summary_counts_match() {
    let dir = tempdir().unwrap();
    let records = vec![
        (100, StreamType::AxisFrames, vec![0x01]),
        (200, StreamType::AxisFrames, vec![0x02]),
        (300, StreamType::BusSnapshots, vec![0x03]),
        (400, StreamType::Events, vec![0x04]),
        (500, StreamType::Events, vec![0x05]),
        (600, StreamType::Events, vec![0x06]),
    ];
    let path = write_test_file(dir.path(), &records).await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(false).unwrap();
    assert_eq!(doc.summary.axis_frames, 2);
    assert_eq!(doc.summary.bus_snapshots, 1);
    assert_eq!(doc.summary.events, 3);
    assert_eq!(doc.summary.total_records, 6);
    assert_eq!(doc.records.len(), 6);
}

#[tokio::test]
async fn export_doc_version_is_correct() {
    let dir = tempdir().unwrap();
    let path = write_test_file(dir.path(), &[]).await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(false).unwrap();
    assert_eq!(doc.export_version, ExportDoc::VERSION);
}

#[tokio::test]
async fn export_doc_json_serializable() {
    let dir = tempdir().unwrap();
    let path = write_test_file(
        dir.path(),
        &[(100, StreamType::Events, vec![0xFF])],
    )
    .await;

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(false).unwrap();

    let json = serde_json::to_string(&doc).unwrap();
    assert!(json.contains("\"export_version\""));
    let round_tripped: ExportDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(round_tripped.summary.total_records, 1);
}

// ═════════════════════════════════════════════════════════════════════
// 16. Edge Cases
// ═════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn edge_large_payload() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Write a large payload (64KB)
    let large_data = vec![0xABu8; 65536];
    writer.record_axis_frame(1000, &large_data).unwrap();

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let rec = reader.next_record().unwrap().unwrap();
    assert_eq!(rec.data.len(), 65536);
    assert!(rec.data.iter().all(|&b| b == 0xAB));
}

#[tokio::test]
async fn edge_empty_payload() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(1000, &[]).unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let rec = reader.next_record().unwrap().unwrap();
    assert!(rec.data.is_empty(), "empty payload should round-trip");
}

#[tokio::test]
async fn edge_max_timestamp() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(u64::MAX, &[0x01]).unwrap();
    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let rec = reader.next_record().unwrap().unwrap();
    assert_eq!(rec.timestamp_ns, u64::MAX);
}

#[tokio::test]
async fn edge_all_stream_types_in_single_recording() {
    let dir = tempdir().unwrap();
    let mut writer = BlackboxWriter::new(test_config(dir.path()));

    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(100, &[0x01]).unwrap();
    writer.record_bus_snapshot(200, &[0x02]).unwrap();
    writer.record_event(300, &[0x03]).unwrap();

    drain_writer().await;
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let r1 = reader.next_record().unwrap().unwrap();
    assert_eq!(r1.stream_type, StreamType::AxisFrames);
    let r2 = reader.next_record().unwrap().unwrap();
    assert_eq!(r2.stream_type, StreamType::BusSnapshots);
    let r3 = reader.next_record().unwrap().unwrap();
    assert_eq!(r3.stream_type, StreamType::Events);
    assert!(reader.next_record().unwrap().is_none());
}

// ═════════════════════════════════════════════════════════════════════
// 17. Truncation and boundary tests
// ═════════════════════════════════════════════════════════════════════

#[test]
fn event_source_truncation_at_max() {
    let mut rec = small_recorder(4);
    let long_source: String = "x".repeat(EVENT_SOURCE_MAX + 50);
    rec.record_event(1, &long_source, &[]);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Event(e) => {
            assert_eq!(e.source_len as usize, EVENT_SOURCE_MAX);
        }
        _ => panic!("expected Event"),
    }
}

#[test]
fn event_data_truncation_at_max() {
    let mut rec = small_recorder(4);
    let long_data: Vec<u8> = vec![0xAB; EVENT_DATA_MAX + 50];
    rec.record_event(1, "src", &long_data);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Event(e) => {
            assert_eq!(e.data_len as usize, EVENT_DATA_MAX);
            assert_eq!(e.data_bytes(), &vec![0xAB; EVENT_DATA_MAX]);
        }
        _ => panic!("expected Event"),
    }
}

#[test]
fn telemetry_sim_truncation_at_max() {
    let mut rec = small_recorder(4);
    let long_sim: String = "S".repeat(SIM_ID_MAX + 20);
    rec.record_telemetry(&long_sim, &[]);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Telemetry(t) => {
            assert_eq!(t.sim_len as usize, SIM_ID_MAX);
        }
        _ => panic!("expected Telemetry"),
    }
}

#[test]
fn telemetry_snapshot_truncation_at_max() {
    let mut rec = small_recorder(4);
    let long_snap: Vec<u8> = vec![0xCC; SNAPSHOT_MAX + 20];
    rec.record_telemetry("X", &long_snap);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Telemetry(t) => {
            assert_eq!(t.snapshot_len as usize, SNAPSHOT_MAX);
        }
        _ => panic!("expected Telemetry"),
    }
}

// ═════════════════════════════════════════════════════════════════════
// 18. Property-based tests (proptest)
// ═════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_ring_buffer_len_is_min_of_written_and_capacity(
        cap in 1usize..256,
        n in 0usize..1000
    ) {
        let mut rec = small_recorder(cap);
        for i in 0..n {
            rec.record_axis(0, 0.0, 0.0, i as u64);
        }
        prop_assert_eq!(rec.len(), n.min(cap));
    }

    #[test]
    fn prop_overflow_count_correct(
        cap in 1usize..256,
        n in 0usize..1000
    ) {
        let mut rec = small_recorder(cap);
        for i in 0..n {
            rec.record_axis(0, 0.0, 0.0, i as u64);
        }
        let expected_overflow = if n > cap { (n - cap) as u64 } else { 0 };
        prop_assert_eq!(rec.overflow_count(), expected_overflow);
    }

    #[test]
    fn prop_total_written_matches_insertions(
        cap in 1usize..128,
        n in 0usize..500
    ) {
        let mut rec = small_recorder(cap);
        for i in 0..n {
            rec.record_axis(0, 0.0, 0.0, i as u64);
        }
        prop_assert_eq!(rec.total_written(), n as u64);
    }

    #[test]
    fn prop_snapshot_preserves_newest(
        cap in 1usize..64,
        n in 1usize..500
    ) {
        let mut rec = small_recorder(cap);
        for i in 0..n {
            rec.record_axis(i as u16, 0.0, 0.0, i as u64 * 100);
        }
        let snap = rec.snapshot();
        if let RecordEntry::Axis(a) = snap.last().unwrap() {
            prop_assert_eq!(a.timestamp_ns, (n - 1) as u64 * 100);
        } else {
            prop_assert!(false, "expected Axis");
        }
    }

    #[test]
    fn prop_blackbox_record_roundtrip(
        ts in any::<u64>(),
        st_idx in 0u8..3,
        data_len in 0usize..256
    ) {
        let st = match st_idx {
            0 => StreamType::AxisFrames,
            1 => StreamType::BusSnapshots,
            _ => StreamType::Events,
        };
        let data: Vec<u8> = (0..data_len).map(|i| (i & 0xFF) as u8).collect();
        let record = BlackboxRecord {
            timestamp_ns: ts,
            stream_type: st,
            data: data.clone(),
        };
        let bytes = postcard::to_stdvec(&record).unwrap();
        let decoded: BlackboxRecord = postcard::from_bytes(&bytes).unwrap();
        prop_assert_eq!(decoded.timestamp_ns, ts);
        prop_assert_eq!(decoded.stream_type, st);
        prop_assert_eq!(decoded.data, data);
    }

    #[test]
    fn prop_index_entry_roundtrip(
        ts in any::<u64>(),
        offset in any::<u64>(),
        c0 in any::<u32>(),
        c1 in any::<u32>(),
        c2 in any::<u32>()
    ) {
        let entry = IndexEntry {
            timestamp_ns: ts,
            file_offset: offset,
            stream_counts: [c0, c1, c2],
        };
        let bytes = postcard::to_stdvec(&entry).unwrap();
        let decoded: IndexEntry = postcard::from_bytes(&bytes).unwrap();
        prop_assert_eq!(decoded.timestamp_ns, ts);
        prop_assert_eq!(decoded.file_offset, offset);
        prop_assert_eq!(decoded.stream_counts, [c0, c1, c2]);
    }

    #[test]
    fn prop_axis_dto_roundtrip(
        axis_id in any::<u16>(),
        raw in any::<f64>(),
        processed in any::<f64>(),
        ts in any::<u64>()
    ) {
        let dto = AxisRecordDto { axis_id, raw, processed, timestamp_ns: ts };
        let bytes = postcard::to_stdvec(&dto).unwrap();
        let decoded: AxisRecordDto = postcard::from_bytes(&bytes).unwrap();
        prop_assert_eq!(decoded.axis_id, axis_id);
        prop_assert_eq!(decoded.timestamp_ns, ts);
        prop_assert_eq!(decoded.raw.to_bits(), raw.to_bits());
        prop_assert_eq!(decoded.processed.to_bits(), processed.to_bits());
    }

    #[test]
    fn prop_to_ns_from_ms_monotonic(a in 0u64..u64::MAX / 1_000_000, b in 0u64..u64::MAX / 1_000_000) {
        let na = flight_blackbox::to_ns_from_ms(a);
        let nb = flight_blackbox::to_ns_from_ms(b);
        if a <= b {
            prop_assert!(na <= nb, "to_ns_from_ms should be monotonic");
        } else {
            prop_assert!(na >= nb, "to_ns_from_ms should be monotonic");
        }
    }

    #[test]
    fn prop_clear_always_empties(cap in 1usize..64, n in 0usize..200) {
        let mut rec = small_recorder(cap);
        for i in 0..n {
            rec.record_axis(0, 0.0, 0.0, i as u64);
        }
        rec.clear();
        prop_assert!(rec.is_empty());
        prop_assert_eq!(rec.len(), 0);
        prop_assert_eq!(rec.total_written(), 0);
    }
}
