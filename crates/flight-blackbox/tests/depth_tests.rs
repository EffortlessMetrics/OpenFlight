// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-blackbox crate.
//!
//! Covers:
//! - Writer tests: binary format, file creation
//! - Reader tests: read-back ordering, timestamp validation
//! - Round-trip tests: write → read → compare with proptest
//! - Corruption resilience: truncated files, corrupted headers, partial writes
//! - Performance tests: 250Hz write throughput (no allocation on ring buffer)
//! - File format: snapshot tests for binary header stability
//! - Recorder ring-buffer: proptest for arbitrary events and capacity limits

use flight_blackbox::{
    BlackboxConfig, BlackboxReader, BlackboxRecord, BlackboxWriter, StreamType, FBB_ENDIAN_MARKER,
    FBB_FORMAT_VERSION, FBB_MAGIC,
    analysis::{AnomalyThresholds, Anomaly, axis_statistics, detect_anomalies, event_timeline},
    export::{RecorderExportDoc, export_binary, export_csv, export_json, summary},
    recorder::{
        BlackboxRecorder, RecordEntry, RecorderConfig, EVENT_DATA_MAX, EVENT_SOURCE_MAX,
        SIM_ID_MAX, SNAPSHOT_MAX,
    },
};
use proptest::prelude::*;
use std::io::Write;
use tempfile::tempdir;

// ── Helper ────────────────────────────────────────────────────────────────

fn small_recorder(cap: usize) -> BlackboxRecorder {
    BlackboxRecorder::new(RecorderConfig { capacity: cap })
}

fn test_config(dir: &std::path::Path) -> BlackboxConfig {
    BlackboxConfig {
        output_dir: dir.to_path_buf(),
        ..BlackboxConfig::default()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Writer tests — binary format, file creation
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn writer_creates_fbb_file() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v0.1".into())
        .await
        .unwrap();

    // Yield to let the spawned writer task create the file
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }

    assert!(path.exists(), "file should be created on start");
    assert!(
        path.extension().unwrap() == "fbb",
        "file should have .fbb extension"
    );

    writer.stop_recording().await.unwrap();
}

#[tokio::test]
async fn writer_file_starts_with_length_prefixed_header() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("DCS".into(), "F16".into(), "v1".into())
        .await
        .unwrap();
    writer.stop_recording().await.unwrap();

    let raw = std::fs::read(&path).unwrap();
    assert!(
        raw.len() >= 4,
        "file must contain at least the header length prefix"
    );
    let header_len = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
    assert!(
        raw.len() >= 4 + header_len,
        "file must contain the full header payload"
    );

    let header: flight_blackbox::BlackboxHeader =
        postcard::from_bytes(&raw[4..4 + header_len]).unwrap();
    assert_eq!(header.magic, *FBB_MAGIC);
    assert_eq!(header.endian_marker, FBB_ENDIAN_MARKER);
    assert_eq!(header.format_version, FBB_FORMAT_VERSION);
    assert_eq!(header.sim_id, "DCS");
    assert_eq!(header.aircraft_id, "F16");
}

#[tokio::test]
async fn writer_records_are_length_prefixed_postcard() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(1000, &[0xAA, 0xBB]).unwrap();

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let raw = std::fs::read(&path).unwrap();
    let header_len = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
    let record_start = 4 + header_len;
    assert!(
        raw.len() > record_start + 4,
        "should have at least one record"
    );

    let rec_len =
        u32::from_le_bytes(raw[record_start..record_start + 4].try_into().unwrap()) as usize;
    let rec_payload = &raw[record_start + 4..record_start + 4 + rec_len];
    let record: BlackboxRecord = postcard::from_bytes(rec_payload).unwrap();
    assert_eq!(record.stream_type, StreamType::AxisFrames);
    assert_eq!(record.data, &[0xAA, 0xBB]);
    assert_eq!(record.timestamp_ns, 1000);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Reader tests — read-back ordering, timestamp validation
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn reader_reads_all_records_in_order() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    let timestamps = [100u64, 200, 300, 400, 500];
    for &ts in &timestamps {
        writer.record_axis_frame(ts, &[0x01]).unwrap();
    }
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut read_timestamps = Vec::new();
    while let Some(rec) = reader.next_record().unwrap() {
        read_timestamps.push(rec.timestamp_ns);
    }

    assert_eq!(
        read_timestamps,
        timestamps.to_vec(),
        "timestamps must preserve order"
    );
}

#[tokio::test]
async fn reader_returns_none_after_last_record() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_event(1000, &[0xFF]).unwrap();
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    assert!(reader.next_record().unwrap().is_some());
    assert!(reader.next_record().unwrap().is_none());
    assert!(reader.next_record().unwrap().is_none());
}

#[tokio::test]
async fn reader_validates_header_fields() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let header = reader.header();
    assert_eq!(header.magic, *FBB_MAGIC);
    assert_eq!(header.format_version, FBB_FORMAT_VERSION);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Round-trip tests — write → read → compare
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn roundtrip_all_stream_types() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("XPLANE".into(), "B738".into(), "v2".into())
        .await
        .unwrap();

    let records = vec![
        (1000u64, StreamType::AxisFrames, vec![0x01, 0x02, 0x03]),
        (2000, StreamType::BusSnapshots, vec![0x10, 0x20]),
        (3000, StreamType::Events, vec![0xAA, 0xBB, 0xCC, 0xDD]),
        (4000, StreamType::AxisFrames, vec![0xFF]),
    ];

    for (ts, st, data) in &records {
        match st {
            StreamType::AxisFrames => writer.record_axis_frame(*ts, data).unwrap(),
            StreamType::BusSnapshots => writer.record_bus_snapshot(*ts, data).unwrap(),
            StreamType::Events => writer.record_event(*ts, data).unwrap(),
        }
    }

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    assert_eq!(reader.header().sim_id, "XPLANE");
    assert_eq!(reader.header().aircraft_id, "B738");

    for (expected_ts, expected_st, expected_data) in &records {
        let rec = reader.next_record().unwrap().expect("should have record");
        assert_eq!(rec.timestamp_ns, *expected_ts);
        assert_eq!(rec.stream_type, *expected_st);
        assert_eq!(rec.data, *expected_data);
    }
    assert!(reader.next_record().unwrap().is_none());
}

#[tokio::test]
async fn roundtrip_large_payload() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "A320".into(), "v1".into())
        .await
        .unwrap();

    let large_data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    writer.record_axis_frame(999, &large_data).unwrap();

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let rec = reader.next_record().unwrap().unwrap();
    assert_eq!(rec.data, large_data);
}

#[tokio::test]
async fn roundtrip_empty_payload() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_event(42, &[]).unwrap();

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let rec = reader.next_record().unwrap().unwrap();
    assert!(rec.data.is_empty());
    assert_eq!(rec.timestamp_ns, 42);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Corruption resilience — truncated files, corrupted headers
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn corrupted_header_too_short() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt.fbb");
    std::fs::write(&path, [0x00, 0x01]).unwrap();

    let result = BlackboxReader::open(&path);
    assert!(result.is_err(), "should fail on truncated header");
}

#[test]
fn corrupted_header_bad_length() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt_len.fbb");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&10000u32.to_le_bytes()).unwrap();
    file.write_all(&[0xDE; 10]).unwrap();

    let result = BlackboxReader::open(&path);
    assert!(
        result.is_err(),
        "should fail when payload shorter than length prefix"
    );
}

#[test]
fn corrupted_header_invalid_postcard() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt_postcard.fbb");
    let garbage = vec![0xFF; 64];
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&(garbage.len() as u32).to_le_bytes())
        .unwrap();
    file.write_all(&garbage).unwrap();

    let result = BlackboxReader::open(&path);
    assert!(result.is_err(), "should fail on invalid postcard header");
}

#[tokio::test]
async fn truncated_record_does_not_panic() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(1000, &[0x01, 0x02]).unwrap();
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let raw = std::fs::read(&path).unwrap();
    let truncated = &raw[..raw.len().saturating_sub(3)];
    std::fs::write(&path, truncated).unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    // Must not panic — any of Ok(None), Ok(Some(_)), Err(_) is acceptable
    let _result = reader.next_record();
}

#[test]
fn empty_fbb_file_fails_to_open() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.fbb");
    std::fs::write(&path, []).unwrap();

    let result = BlackboxReader::open(&path);
    assert!(result.is_err(), "empty file should fail to read header");
}

#[tokio::test]
async fn validate_rejects_wrong_magic() {
    let dir = tempdir().unwrap();
    let config = test_config(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();
    writer.stop_recording().await.unwrap();

    // Corrupt the magic bytes in the header
    let raw = std::fs::read(&path).unwrap();
    let header_len = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
    let header_payload = &raw[4..4 + header_len];
    let mut header: flight_blackbox::BlackboxHeader =
        postcard::from_bytes(header_payload).unwrap();
    header.magic = *b"XXXX";
    let new_payload = postcard::to_stdvec(&header).unwrap();
    let new_len = new_payload.len() as u32;

    // Rebuild file
    let mut new_file = Vec::new();
    new_file.extend_from_slice(&new_len.to_le_bytes());
    new_file.extend_from_slice(&new_payload);
    new_file.extend_from_slice(&raw[4 + header_len..]);
    std::fs::write(&path, &new_file).unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    let result = reader.validate();
    assert!(result.is_err(), "should reject invalid magic");
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Performance tests — ring buffer zero-alloc at 250Hz
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn ring_buffer_no_alloc_during_recording() {
    let mut rec = small_recorder(15_000);
    let cap_before = rec.capacity();

    for i in 0..15_000u64 {
        rec.record_axis(
            (i % 8) as u16,
            (i as f64) / 15_000.0,
            (i as f64) / 15_000.0,
            i * 4_000_000,
        );
    }

    assert_eq!(rec.capacity(), cap_before, "capacity must not change");
    assert_eq!(rec.len(), 15_000);
    assert_eq!(rec.total_written(), 15_000);
    assert_eq!(rec.overflow_count(), 0);
}

#[test]
fn ring_buffer_overflow_is_allocation_free() {
    let mut rec = small_recorder(1000);

    for i in 0..3000u64 {
        rec.record_axis(0, i as f64, i as f64, i * 4_000_000);
    }

    assert_eq!(rec.capacity(), 1000);
    assert_eq!(rec.len(), 1000);
    assert_eq!(rec.total_written(), 3000);
    assert_eq!(rec.overflow_count(), 2000);
}

#[test]
fn mixed_record_types_throughput() {
    let mut rec = small_recorder(10_000);
    let start = std::time::Instant::now();

    for i in 0..10_000u64 {
        match i % 4 {
            0 => rec.record_axis((i % 8) as u16, i as f64, i as f64, i * 4_000_000),
            1 => rec.record_event(i as u16, "perf-test", &[0x01, 0x02]),
            2 => rec.record_telemetry("MSFS", &[0xAA; 32]),
            3 => rec.record_ffb(i as u16, (i as f64) / 10_000.0),
            _ => unreachable!(),
        }
    }

    let elapsed = start.elapsed();
    assert_eq!(rec.len(), 10_000);
    assert!(
        elapsed.as_millis() < 500,
        "recording 10k mixed entries should complete in <500ms, took {}ms",
        elapsed.as_millis()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. File format — header binary stability
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn header_postcard_format_stability() {
    let header = flight_blackbox::BlackboxHeader {
        magic: *FBB_MAGIC,
        endian_marker: FBB_ENDIAN_MARKER,
        format_version: FBB_FORMAT_VERSION,
        app_version: "1.0.0".into(),
        timebase_ns: 1_700_000_000_000_000_000,
        sim_id: "MSFS".into(),
        aircraft_id: "C172".into(),
        recording_mode: "default".into(),
        start_timestamp: 0,
    };

    let bytes = postcard::to_stdvec(&header).unwrap();
    let recovered: flight_blackbox::BlackboxHeader = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(recovered.magic, *FBB_MAGIC);
    assert_eq!(recovered.endian_marker, FBB_ENDIAN_MARKER);
    assert_eq!(recovered.format_version, FBB_FORMAT_VERSION);
    assert_eq!(recovered.app_version, "1.0.0");
    assert_eq!(recovered.sim_id, "MSFS");
    assert_eq!(recovered.aircraft_id, "C172");
    assert_eq!(recovered.recording_mode, "default");
    assert_eq!(recovered.timebase_ns, 1_700_000_000_000_000_000);

    assert!(
        bytes.windows(4).any(|w| w == FBB_MAGIC),
        "FBB1 magic should be present in serialized header"
    );
}

#[test]
fn stream_type_repr_values_are_stable() {
    assert_eq!(StreamType::AxisFrames as u8, 0xA);
    assert_eq!(StreamType::BusSnapshots as u8, 0xB);
    assert_eq!(StreamType::Events as u8, 0xC);
}

#[test]
fn record_postcard_format_round_trip() {
    let record = BlackboxRecord {
        timestamp_ns: 42_000_000,
        stream_type: StreamType::Events,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };

    let bytes = postcard::to_stdvec(&record).unwrap();
    let recovered: BlackboxRecord = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(recovered.timestamp_ns, record.timestamp_ns);
    assert_eq!(recovered.stream_type, record.stream_type);
    assert_eq!(recovered.data, record.data);
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Export round-trip and analysis coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn export_json_binary_csv_consistency() {
    let dir = tempdir().unwrap();
    let mut rec = small_recorder(64);
    for i in 0..10u16 {
        rec.record_axis(i, i as f64, i as f64, (i as u64) * 4_000_000);
    }
    rec.record_event(1, "test", &[0x42]);

    let json_path = dir.path().join("out.json");
    let csv_path = dir.path().join("out.csv");
    let bin_path = dir.path().join("out.bin");

    export_json(&rec, &json_path).unwrap();
    export_csv(&rec, &csv_path).unwrap();
    export_binary(&rec, &bin_path).unwrap();

    let json_doc: RecorderExportDoc =
        serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
    assert_eq!(json_doc.entry_count, 11);

    let csv_lines = std::fs::read_to_string(&csv_path)
        .unwrap()
        .lines()
        .count();
    assert_eq!(csv_lines, 11); // header + 10 axis rows

    let bin_bytes = std::fs::read(&bin_path).unwrap();
    let bin_doc: RecorderExportDoc = postcard::from_bytes(&bin_bytes).unwrap();
    assert_eq!(bin_doc.entry_count, 11);
    assert_eq!(bin_doc, json_doc);
}

#[test]
fn summary_counts_all_record_types() {
    let mut rec = small_recorder(64);
    rec.record_axis(1, 0.5, 0.5, 1_000_000);
    rec.record_axis(2, -0.3, -0.3, 2_000_000);
    rec.record_event(10, "panel", &[0x01]);
    rec.record_telemetry("MSFS", &[0xAA]);
    rec.record_ffb(1, 0.8);

    let s = summary(&rec);
    assert_eq!(s.total_entries, 5);
    assert_eq!(s.axis_count, 2);
    assert_eq!(s.event_count, 1);
    assert_eq!(s.telemetry_count, 1);
    assert_eq!(s.ffb_count, 1);
    assert_eq!(s.overflow_count, 0);
}

#[test]
fn anomaly_detection_finds_saturation_and_disconnect() {
    let mut rec = small_recorder(64);
    rec.record_axis(1, 0.0, 0.5, 4_000_000);
    rec.record_axis(1, 0.0, 0.5, 8_000_000);
    rec.record_axis(1, 0.0, 1.0, 12_000_000);
    rec.record_axis(1, 0.0, 0.5, 100_000_000);

    let thresholds = AnomalyThresholds {
        max_gap_ns: 20_000_000,
        saturation_threshold: 0.999,
        max_jitter_ns: 500_000,
    };
    let anomalies = detect_anomalies(&rec, &thresholds);

    let has_saturation = anomalies
        .iter()
        .any(|a| matches!(a, Anomaly::Saturation { .. }));
    let has_disconnect = anomalies
        .iter()
        .any(|a| matches!(a, Anomaly::Disconnect { .. }));
    assert!(has_saturation, "should detect saturation at 1.0");
    assert!(has_disconnect, "should detect disconnect gap");
}

#[test]
fn axis_statistics_computed_correctly() {
    let mut rec = small_recorder(128);
    for i in 1..=100 {
        rec.record_axis(1, i as f64, i as f64, i as u64 * 1000);
    }

    let stats = axis_statistics(&rec, 1).unwrap();
    assert_eq!(stats.count, 100);
    assert!((stats.min - 1.0).abs() < f64::EPSILON);
    assert!((stats.max - 100.0).abs() < f64::EPSILON);
    assert!((stats.mean - 50.5).abs() < 0.01);
}

#[test]
fn event_timeline_sorted_and_excludes_axis() {
    let mut rec = small_recorder(32);
    rec.record_axis(1, 0.0, 0.0, 1000);
    rec.record_ffb(1, 0.5);
    rec.record_event(1, "hid", &[]);
    rec.record_telemetry("DCS", &[]);

    let tl = event_timeline(&rec);
    assert_eq!(tl.len(), 3, "axis should be excluded");
    for w in tl.windows(2) {
        assert!(w[0].timestamp_ns <= w[1].timestamp_ns, "must be sorted");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Recorder edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn recorder_capacity_one_works() {
    let mut rec = small_recorder(1);
    rec.record_axis(1, 0.5, 0.5, 100);
    assert_eq!(rec.len(), 1);
    rec.record_axis(2, 0.6, 0.6, 200);
    assert_eq!(rec.len(), 1);
    assert_eq!(rec.total_written(), 2);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Axis(a) => assert_eq!(a.axis_id, 2),
        _ => panic!("expected newest entry"),
    }
}

#[test]
fn recorder_clear_and_reuse() {
    let mut rec = small_recorder(16);
    for i in 0..10u16 {
        rec.record_axis(i, 0.0, 0.0, i as u64);
    }
    assert_eq!(rec.len(), 10);

    rec.clear();
    assert!(rec.is_empty());
    assert_eq!(rec.total_written(), 0);

    rec.record_axis(99, 1.0, 1.0, 999);
    assert_eq!(rec.len(), 1);
    assert_eq!(rec.total_written(), 1);
}

#[test]
fn event_data_truncation() {
    let mut rec = small_recorder(4);
    let long_data = vec![0xAB; EVENT_DATA_MAX + 50];
    rec.record_event(1, "src", &long_data);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Event(e) => {
            assert_eq!(e.data_len as usize, EVENT_DATA_MAX);
            assert_eq!(e.data_bytes().len(), EVENT_DATA_MAX);
        }
        _ => panic!("expected Event"),
    }
}

#[test]
fn telemetry_truncation() {
    let mut rec = small_recorder(4);
    let long_sim = "A".repeat(SIM_ID_MAX + 10);
    let long_snap = vec![0xCC; SNAPSHOT_MAX + 20];
    rec.record_telemetry(&long_sim, &long_snap);

    let snap = rec.snapshot();
    match &snap[0] {
        RecordEntry::Telemetry(t) => {
            assert_eq!(t.sim_len as usize, SIM_ID_MAX);
            assert_eq!(t.snapshot_len as usize, SNAPSHOT_MAX);
        }
        _ => panic!("expected Telemetry"),
    }
}

#[test]
fn iterator_exact_size_matches_len() {
    let mut rec = small_recorder(32);
    for i in 0..20u16 {
        rec.record_axis(i, 0.0, 0.0, i as u64);
    }
    let iter = rec.iter();
    assert_eq!(iter.len(), rec.len());
    assert_eq!(iter.count(), 20);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Proptest — arbitrary events round-trip
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_blackbox_record_roundtrip(
        timestamp_ns in any::<u64>(),
        stream_type in prop_oneof![
            Just(StreamType::AxisFrames),
            Just(StreamType::BusSnapshots),
            Just(StreamType::Events),
        ],
        data in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        let record = BlackboxRecord {
            timestamp_ns,
            stream_type,
            data: data.clone(),
        };
        let bytes = postcard::to_stdvec(&record).unwrap();
        let recovered: BlackboxRecord = postcard::from_bytes(&bytes).unwrap();
        prop_assert_eq!(recovered.timestamp_ns, timestamp_ns);
        prop_assert_eq!(recovered.stream_type, stream_type);
        prop_assert_eq!(recovered.data, data);
    }

    #[test]
    fn prop_recorder_capacity_respected(
        capacity in 1usize..500,
        num_writes in 0usize..2000,
    ) {
        let mut rec = BlackboxRecorder::new(RecorderConfig { capacity });
        for i in 0..num_writes {
            rec.record_axis(i as u16, i as f64, i as f64, i as u64);
        }
        prop_assert!(rec.len() <= capacity);
        prop_assert_eq!(rec.total_written(), num_writes as u64);
        if num_writes > capacity {
            prop_assert_eq!(rec.overflow_count(), (num_writes - capacity) as u64);
        } else {
            prop_assert_eq!(rec.overflow_count(), 0);
        }
    }

    #[test]
    fn prop_recorder_snapshot_order_is_chronological(
        capacity in 2usize..100,
        num_writes in 2usize..500,
    ) {
        let mut rec = BlackboxRecorder::new(RecorderConfig { capacity });
        for i in 0..num_writes {
            rec.record_axis(0, 0.0, 0.0, i as u64 * 1000);
        }
        let snap = rec.snapshot();
        for window in snap.windows(2) {
            let ts_a = match &window[0] {
                RecordEntry::Axis(a) => a.timestamp_ns,
                _ => continue,
            };
            let ts_b = match &window[1] {
                RecordEntry::Axis(a) => a.timestamp_ns,
                _ => continue,
            };
            prop_assert!(ts_b >= ts_a, "timestamps must be non-decreasing: {} >= {}", ts_b, ts_a);
        }
    }

    #[test]
    fn prop_axis_record_preserves_all_fields(
        axis_id in any::<u16>(),
        raw in any::<f64>(),
        processed in any::<f64>(),
        timestamp_ns in any::<u64>(),
    ) {
        let mut rec = small_recorder(4);
        rec.record_axis(axis_id, raw, processed, timestamp_ns);
        let snap = rec.snapshot();
        match &snap[0] {
            RecordEntry::Axis(a) => {
                prop_assert_eq!(a.axis_id, axis_id);
                prop_assert_eq!(a.timestamp_ns, timestamp_ns);
                if raw.is_nan() {
                    prop_assert!(a.raw.is_nan());
                } else {
                    prop_assert_eq!(a.raw, raw);
                }
                if processed.is_nan() {
                    prop_assert!(a.processed.is_nan());
                } else {
                    prop_assert_eq!(a.processed, processed);
                }
            }
            _ => prop_assert!(false, "expected Axis record"),
        }
    }

    #[test]
    fn prop_event_source_str_is_valid_utf8(
        source in "[a-z0-9_-]{0,32}",
        data in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let mut rec = small_recorder(4);
        rec.record_event(1, &source, &data);
        let snap = rec.snapshot();
        match &snap[0] {
            RecordEntry::Event(e) => {
                let recovered = e.source_str();
                prop_assert_eq!(recovered, &source[..source.len().min(EVENT_SOURCE_MAX)]);
            }
            _ => prop_assert!(false, "expected Event record"),
        }
    }
}
