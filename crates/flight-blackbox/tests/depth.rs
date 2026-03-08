// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the blackbox (flight data recording) system.
//!
//! Covers recording, playback, storage, RT safety, and integration scenarios.

use std::path::Path;
use std::time::Duration;

use flight_blackbox::recorder::{
    BlackboxRecorder, RecordEntry, RecorderConfig, EVENT_DATA_MAX, EVENT_SOURCE_MAX, SIM_ID_MAX,
    SNAPSHOT_MAX,
};
use flight_blackbox::{
    BlackboxConfig, BlackboxHeader, BlackboxReader, BlackboxRecord, BlackboxWriter, ExportDoc,
    FBB_ENDIAN_MARKER, FBB_FORMAT_VERSION, FBB_MAGIC, StreamType,
};

/// Helper: create a [`BlackboxConfig`] with `output_dir` pointing at the given path.
fn config_in(dir: &Path) -> BlackboxConfig {
    BlackboxConfig {
        output_dir: dir.to_path_buf(),
        ..BlackboxConfig::default()
    }
}

/// Helper: create a [`BlackboxConfig`] with a custom buffer size.
fn config_in_with_buffer(dir: &Path, buffer_size: usize) -> BlackboxConfig {
    BlackboxConfig {
        output_dir: dir.to_path_buf(),
        buffer_size,
        ..BlackboxConfig::default()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §1  Recording (8 tests)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn recording_start_stop_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    writer
        .start("v1".into(), "MSFS".into(), "C172".into(), "test".into())
        .await
        .unwrap();
    writer.stop().await.unwrap();
}

#[tokio::test]
async fn recording_stop_without_start_errors() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let err = writer.stop().await.unwrap_err();
    assert!(
        err.to_string().contains("not started"),
        "expected NotStarted error, got: {err}"
    );
}

#[tokio::test]
async fn recording_double_start_errors() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    writer
        .start("v1".into(), "SIM".into(), "A320".into(), "m".into())
        .await
        .unwrap();
    let err = writer
        .start("v1".into(), "SIM".into(), "A320".into(), "m".into())
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("already started")
            || err.to_string().contains("Already"),
        "expected AlreadyStarted, got: {err}"
    );
    writer.stop().await.unwrap();
}

#[tokio::test]
async fn recording_captures_250hz_frames() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Simulate 250 frames (1 second at 250Hz)
    for i in 0..250u64 {
        let ts = i * 4_000_000; // 4ms intervals
        writer.record_axis_frame(ts, &[i as u8]).unwrap();
    }

    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut count = 0u64;
    while reader.next_record().unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 250, "should capture all 250 frames");
}

#[tokio::test]
async fn recording_variable_rate_channels() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // 250Hz axis frames, 60Hz bus snapshots, sporadic events
    for i in 0..250u64 {
        let ts = i * 4_000_000;
        writer.record_axis_frame(ts, &[0xAA]).unwrap();
    }
    for i in 0..60u64 {
        let ts = i * 16_666_667;
        writer.record_bus_snapshot(ts, &[0xBB]).unwrap();
    }
    for i in 0..5u64 {
        writer.record_event(i * 200_000_000, &[0xCC]).unwrap();
    }

    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut axis = 0u64;
    let mut bus = 0u64;
    let mut events = 0u64;
    while let Some(rec) = reader.next_record().unwrap() {
        match rec.stream_type {
            StreamType::AxisFrames => axis += 1,
            StreamType::BusSnapshots => bus += 1,
            StreamType::Events => events += 1,
        }
    }
    assert_eq!(axis, 250);
    assert_eq!(bus, 60);
    assert_eq!(events, 5);
}

#[tokio::test]
async fn recording_buffer_overflow_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in_with_buffer(dir.path(), 128);

    let mut writer = BlackboxWriter::new(config);
    writer
        .start("v1".into(), "MSFS".into(), "C172".into(), "test".into())
        .await
        .unwrap();

    // Flood the channel without yielding to the writer task
    let mut overflow_count = 0u64;
    for i in 0..50_000u64 {
        if writer.record_axis_frame(i, &[0xFF; 64]).is_err() {
            overflow_count += 1;
        }
    }
    assert!(overflow_count > 0, "should have triggered buffer overflow");

    // Drain and stop gracefully
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    writer.stop().await.unwrap();
}

#[tokio::test]
async fn recording_timestamps_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    let timestamps = [100_000u64, 200_000, 999_999_999, u64::MAX / 2];
    for &ts in &timestamps {
        writer.record_axis_frame(ts, &[0x01]).unwrap();
    }

    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    for &expected_ts in &timestamps {
        let rec = reader.next_record().unwrap().unwrap();
        assert_eq!(rec.timestamp_ns, expected_ts);
    }
}

#[tokio::test]
async fn recording_metadata_header_fields() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("DCS".into(), "FA18".into(), "2.1.0".into())
        .await
        .unwrap();

    writer.record_axis_frame(1, &[0x00]).unwrap();
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let h = reader.header();
    assert_eq!(h.magic, *FBB_MAGIC);
    assert_eq!(h.endian_marker, FBB_ENDIAN_MARKER);
    assert_eq!(h.format_version, FBB_FORMAT_VERSION);
    assert_eq!(h.sim_id, "DCS");
    assert_eq!(h.aircraft_id, "FA18");
    assert_eq!(h.app_version, "2.1.0");
    assert_eq!(h.recording_mode, "default");
    assert!(h.timebase_ns > 0, "timebase should be set");
    assert!(h.start_timestamp > 0, "start_timestamp should be set");
}

// ═══════════════════════════════════════════════════════════════════════
// §2  Playback (6 tests)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn playback_reads_all_frames_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    for i in 0..20u64 {
        writer.record_axis_frame(i * 1000, &[i as u8]).unwrap();
    }
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    let mut prev_ts = 0u64;
    let mut count = 0;
    while let Some(rec) = reader.next_record().unwrap() {
        assert!(
            rec.timestamp_ns >= prev_ts,
            "timestamps should be non-decreasing"
        );
        prev_ts = rec.timestamp_ns;
        count += 1;
    }
    assert_eq!(count, 20);
}

#[tokio::test]
async fn playback_data_integrity() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Write records with distinct payloads
    let payloads: Vec<Vec<u8>> = (0..10u8).map(|i| vec![i; (i as usize) + 1]).collect();
    for (i, payload) in payloads.iter().enumerate() {
        writer
            .record_axis_frame(i as u64 * 1000, payload)
            .unwrap();
    }

    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    for expected_payload in &payloads {
        let rec = reader.next_record().unwrap().unwrap();
        assert_eq!(&rec.data, expected_payload);
    }
}

#[tokio::test]
async fn playback_stream_type_filtering() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(1000, &[0xAA]).unwrap();
    writer.record_bus_snapshot(2000, &[0xBB]).unwrap();
    writer.record_event(3000, &[0xCC]).unwrap();
    writer.record_axis_frame(4000, &[0xDD]).unwrap();

    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    // Read all and filter by stream type
    let mut records = Vec::new();
    while let Some(rec) = reader.next_record().unwrap() {
        records.push(rec);
    }

    let axis_only: Vec<_> = records
        .iter()
        .filter(|r| r.stream_type == StreamType::AxisFrames)
        .collect();
    assert_eq!(axis_only.len(), 2);
    assert_eq!(axis_only[0].data, vec![0xAA]);
    assert_eq!(axis_only[1].data, vec![0xDD]);

    let bus_only: Vec<_> = records
        .iter()
        .filter(|r| r.stream_type == StreamType::BusSnapshots)
        .collect();
    assert_eq!(bus_only.len(), 1);
}

#[tokio::test]
async fn playback_empty_recording() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    assert!(reader.next_record().unwrap().is_none());
}

#[tokio::test]
async fn playback_large_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    let big_payload = vec![0xAB; 4096];
    writer.record_axis_frame(1000, &big_payload).unwrap();

    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let rec = reader.next_record().unwrap().unwrap();
    assert_eq!(rec.data.len(), 4096);
    assert!(rec.data.iter().all(|&b| b == 0xAB));
}

#[tokio::test]
async fn playback_export_roundtrip_counts() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("XPLANE".into(), "B738".into(), "v2".into())
        .await
        .unwrap();

    for i in 0..10u64 {
        writer.record_axis_frame(i * 1000, &[0x01]).unwrap();
    }
    for i in 0..3u64 {
        writer
            .record_bus_snapshot(i * 10_000, &[0x02])
            .unwrap();
    }
    writer.record_event(50_000, &[0x03]).unwrap();

    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(false).unwrap();

    assert_eq!(doc.export_version, ExportDoc::VERSION);
    assert_eq!(doc.summary.axis_frames, 10);
    assert_eq!(doc.summary.bus_snapshots, 3);
    assert_eq!(doc.summary.events, 1);
    assert_eq!(doc.summary.total_records, 14);
    assert_eq!(doc.records.len(), 14);
}

// ═══════════════════════════════════════════════════════════════════════
// §3  Storage (6 tests)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn storage_file_format_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    let test_data: Vec<(u64, StreamType, Vec<u8>)> = vec![
        (1000, StreamType::AxisFrames, vec![0xDE, 0xAD]),
        (2000, StreamType::BusSnapshots, vec![0xBE, 0xEF]),
        (3000, StreamType::Events, vec![0xCA, 0xFE]),
    ];

    for (ts, st, data) in &test_data {
        match st {
            StreamType::AxisFrames => writer.record_axis_frame(*ts, data).unwrap(),
            StreamType::BusSnapshots => writer.record_bus_snapshot(*ts, data).unwrap(),
            StreamType::Events => writer.record_event(*ts, data).unwrap(),
        }
    }

    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    // Verify the file exists and read it back
    assert!(path.exists());
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();

    for (expected_ts, expected_st, expected_data) in &test_data {
        let rec = reader.next_record().unwrap().unwrap();
        assert_eq!(rec.timestamp_ns, *expected_ts);
        assert_eq!(rec.stream_type, *expected_st);
        assert_eq!(&rec.data, expected_data);
    }
    assert!(reader.next_record().unwrap().is_none());
}

#[tokio::test]
async fn storage_file_naming_convention() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let filename = path.file_name().unwrap().to_str().unwrap();
    assert!(filename.starts_with("flight_"), "filename should start with flight_");
    assert!(filename.ends_with(".fbb"), "filename should end with .fbb");
}

#[tokio::test]
async fn storage_output_dir_created_automatically() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("sub").join("dir");
    let config = config_in(&nested);

    let mut writer = BlackboxWriter::new(config);
    writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();
    assert!(nested.exists(), "output directory should be created");
}

#[test]
fn storage_header_serialization_roundtrip() {
    let header = BlackboxHeader {
        magic: *FBB_MAGIC,
        endian_marker: FBB_ENDIAN_MARKER,
        format_version: FBB_FORMAT_VERSION,
        app_version: "1.2.3".to_string(),
        timebase_ns: 1_700_000_000_000_000_000,
        sim_id: "DCS".to_string(),
        aircraft_id: "FA18".to_string(),
        recording_mode: "replay".to_string(),
        start_timestamp: 42_000_000,
    };

    let bytes = postcard::to_stdvec(&header).unwrap();
    let decoded: BlackboxHeader = postcard::from_bytes(&bytes).unwrap();

    assert_eq!(decoded.magic, header.magic);
    assert_eq!(decoded.endian_marker, header.endian_marker);
    assert_eq!(decoded.format_version, header.format_version);
    assert_eq!(decoded.app_version, header.app_version);
    assert_eq!(decoded.timebase_ns, header.timebase_ns);
    assert_eq!(decoded.sim_id, header.sim_id);
    assert_eq!(decoded.aircraft_id, header.aircraft_id);
    assert_eq!(decoded.recording_mode, header.recording_mode);
    assert_eq!(decoded.start_timestamp, header.start_timestamp);
}

#[test]
fn storage_record_serialization_all_stream_types() {
    for stream_type in [
        StreamType::AxisFrames,
        StreamType::BusSnapshots,
        StreamType::Events,
    ] {
        let record = BlackboxRecord {
            timestamp_ns: 123_456_789,
            stream_type,
            data: vec![0x01, 0x02, 0x03],
        };
        let bytes = postcard::to_stdvec(&record).unwrap();
        let decoded: BlackboxRecord = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.timestamp_ns, record.timestamp_ns);
        assert_eq!(decoded.stream_type, record.stream_type);
        assert_eq!(decoded.data, record.data);
    }
}

#[test]
fn storage_reader_rejects_nonexistent_file() {
    let result = BlackboxReader::open("nonexistent_file_abc123.fbb");
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════
// §4  RT Safety (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn rt_no_allocation_during_axis_recording() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 1024 });
    let cap_before = rec.capacity();

    // Fill beyond capacity — buffer must not reallocate
    for i in 0..5000u16 {
        rec.record_axis(i, i as f64 / 5000.0, i as f64 / 5000.0, i as u64 * 4_000_000);
    }

    assert_eq!(
        rec.capacity(),
        cap_before,
        "capacity must not change (no reallocation)"
    );
    assert_eq!(rec.len(), 1024);
    assert_eq!(rec.total_written(), 5000);
}

#[test]
fn rt_no_allocation_during_event_recording() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 256 });
    let cap_before = rec.capacity();

    for i in 0..1000u16 {
        rec.record_event(i, "rt-source", &[0xAA; 16]);
    }

    assert_eq!(
        rec.capacity(),
        cap_before,
        "capacity must not change during event recording"
    );
}

#[test]
fn rt_no_allocation_during_mixed_recording() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 512 });
    let cap_before = rec.capacity();

    for i in 0..2000u16 {
        match i % 4 {
            0 => rec.record_axis(i, 0.5, 0.5, i as u64 * 1000),
            1 => rec.record_event(i, "src", &[0x01]),
            2 => rec.record_telemetry("MSFS", &[0x02, 0x03]),
            _ => rec.record_ffb(i, 0.7),
        }
    }

    assert_eq!(rec.capacity(), cap_before, "no reallocation under mixed load");
    assert_eq!(rec.len(), 512);
    assert_eq!(rec.total_written(), 2000);
}

#[test]
fn rt_ring_buffer_overflow_preserves_newest() {
    let cap = 8;
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: cap });

    for i in 0..100u16 {
        rec.record_axis(i, i as f64, i as f64, i as u64 * 1000);
    }

    assert_eq!(rec.len(), cap);
    assert_eq!(rec.overflow_count(), 92);

    // Verify only the newest entries remain
    let entries = rec.snapshot();
    for (idx, entry) in entries.iter().enumerate() {
        match entry {
            RecordEntry::Axis(a) => {
                assert_eq!(a.axis_id, (92 + idx) as u16);
            }
            _ => panic!("expected Axis entry"),
        }
    }
}

#[test]
fn rt_atomic_state_clear_and_rerecord() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 64 });

    // First recording session
    for i in 0..30u16 {
        rec.record_axis(i, 0.0, 0.0, i as u64);
    }
    assert_eq!(rec.len(), 30);
    assert_eq!(rec.total_written(), 30);

    // Clear state atomically
    rec.clear();
    assert!(rec.is_empty());
    assert_eq!(rec.len(), 0);
    assert_eq!(rec.total_written(), 0);
    assert_eq!(rec.overflow_count(), 0);

    // Second recording session — buffer is reusable
    for i in 0..10u16 {
        rec.record_ffb(i, 0.5);
    }
    assert_eq!(rec.len(), 10);

    let entries = rec.snapshot();
    assert!(matches!(entries[0], RecordEntry::Ffb(_)));
}

// ═══════════════════════════════════════════════════════════════════════
// §5  Integration (5 tests)
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn integration_full_pipeline_record_export() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    // Simulate a short flight segment
    for i in 0..100u64 {
        writer
            .record_axis_frame(i * 4_000_000, &(i as u32).to_le_bytes())
            .unwrap();
        if i % 4 == 0 {
            writer
                .record_bus_snapshot(i * 4_000_000, &[0x01, 0x02])
                .unwrap();
        }
        if i == 50 {
            writer
                .record_event(i * 4_000_000, &[0xEE])
                .unwrap();
        }
    }

    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    // Read back and verify pipeline integrity
    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(false).unwrap();

    assert_eq!(doc.summary.axis_frames, 100);
    assert_eq!(doc.summary.bus_snapshots, 25); // every 4th frame
    assert_eq!(doc.summary.events, 1);
    assert_eq!(doc.summary.total_records, 126);
    assert_eq!(doc.header.sim_id, "MSFS");
    assert_eq!(doc.header.aircraft_id, "C172");
}

#[tokio::test]
async fn integration_sanitized_export_redacts_aircraft() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("DCS".into(), "SECRET_PLANE".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(1000, &[0x01]).unwrap();
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(true).unwrap();

    assert_eq!(doc.header.aircraft_id, "[REDACTED]");
    assert_eq!(doc.header.sim_id, "DCS");
}

#[tokio::test]
async fn integration_export_json_serializable() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let mut writer = BlackboxWriter::new(config);
    let path = writer
        .start_recording("MSFS".into(), "C172".into(), "v1".into())
        .await
        .unwrap();

    writer.record_axis_frame(1000, &[0xDE, 0xAD]).unwrap();
    writer.record_bus_snapshot(2000, &[0xBE]).unwrap();

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    writer.stop_recording().await.unwrap();

    let mut reader = BlackboxReader::open(&path).unwrap();
    reader.validate().unwrap();
    let doc = reader.export(false).unwrap();

    let json = serde_json::to_string_pretty(&doc).unwrap();
    assert!(json.contains("\"export_version\""));
    assert!(json.contains("\"summary\""));
    assert!(json.contains("\"axis_frames\""));

    // Deserialize back and verify
    let roundtrip: ExportDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.summary.total_records, doc.summary.total_records);
}

#[test]
fn integration_recorder_export_json_roundtrip() {
    use flight_blackbox::export::{export_json, RecorderExportDoc};

    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("test_export.json");

    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 128 });
    for i in 0..50u16 {
        rec.record_axis(i, i as f64, i as f64 * 0.5, i as u64 * 4_000_000);
    }
    rec.record_event(10, "panel-switch", &[0x01, 0x02]);
    rec.record_telemetry("DCS", &[0xAA, 0xBB, 0xCC]);
    rec.record_ffb(5, 0.65);

    export_json(&rec, &json_path).unwrap();

    let json_str = std::fs::read_to_string(&json_path).unwrap();
    let doc: RecorderExportDoc = serde_json::from_str(&json_str).unwrap();

    assert_eq!(doc.version, RecorderExportDoc::VERSION);
    assert_eq!(doc.entry_count, 53); // 50 axis + 1 event + 1 telemetry + 1 ffb
}

#[test]
fn integration_recorder_export_binary_roundtrip() {
    use flight_blackbox::export::{export_binary, RecorderExportDoc};

    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_export.bin");

    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 64 });
    for i in 0..20u16 {
        rec.record_axis(i, i as f64, i as f64, i as u64 * 1000);
    }
    rec.record_ffb(1, 0.99);

    export_binary(&rec, &bin_path).unwrap();

    let bytes = std::fs::read(&bin_path).unwrap();
    let doc: RecorderExportDoc = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(doc.entry_count, 21);
}

// ═══════════════════════════════════════════════════════════════════════
// §6  Additional depth: recorder ring-buffer edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn recorder_capacity_one() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 1 });
    assert_eq!(rec.capacity(), 1);

    rec.record_axis(1, 0.5, 0.5, 1000);
    assert_eq!(rec.len(), 1);

    rec.record_axis(2, 0.6, 0.6, 2000);
    assert_eq!(rec.len(), 1);
    assert_eq!(rec.overflow_count(), 1);

    let entries = rec.snapshot();
    match &entries[0] {
        RecordEntry::Axis(a) => assert_eq!(a.axis_id, 2),
        _ => panic!("expected Axis"),
    }
}

#[test]
fn recorder_iterator_chronological_after_wrap() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 4 });

    // Write 7 entries to wrap around
    for i in 0..7u16 {
        rec.record_axis(i, 0.0, 0.0, (i as u64 + 1) * 1000);
    }

    let entries = rec.snapshot();
    assert_eq!(entries.len(), 4);

    // Should be entries 3,4,5,6 (axis_ids)
    let ids: Vec<u16> = entries
        .iter()
        .map(|e| match e {
            RecordEntry::Axis(a) => a.axis_id,
            _ => panic!("expected Axis"),
        })
        .collect();
    assert_eq!(ids, vec![3, 4, 5, 6]);
}

#[test]
fn recorder_event_data_truncation() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 4 });

    // Source longer than EVENT_SOURCE_MAX
    let long_source = "x".repeat(EVENT_SOURCE_MAX + 100);
    // Data longer than EVENT_DATA_MAX
    let long_data = vec![0xFF; EVENT_DATA_MAX + 100];

    rec.record_event(1, &long_source, &long_data);

    let entries = rec.snapshot();
    match &entries[0] {
        RecordEntry::Event(e) => {
            assert_eq!(e.source_len as usize, EVENT_SOURCE_MAX);
            assert_eq!(e.data_len as usize, EVENT_DATA_MAX);
            assert_eq!(e.source_str().len(), EVENT_SOURCE_MAX);
            assert_eq!(e.data_bytes().len(), EVENT_DATA_MAX);
        }
        _ => panic!("expected Event"),
    }
}

#[test]
fn recorder_telemetry_truncation() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 4 });

    let long_sim = "S".repeat(SIM_ID_MAX + 50);
    let long_snap = vec![0xAA; SNAPSHOT_MAX + 50];

    rec.record_telemetry(&long_sim, &long_snap);

    let entries = rec.snapshot();
    match &entries[0] {
        RecordEntry::Telemetry(t) => {
            assert_eq!(t.sim_len as usize, SIM_ID_MAX);
            assert_eq!(t.snapshot_len as usize, SNAPSHOT_MAX);
        }
        _ => panic!("expected Telemetry"),
    }
}

#[test]
fn recorder_snapshot_is_independent_copy() {
    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 16 });
    rec.record_axis(1, 0.5, 0.5, 1000);

    let snap1 = rec.snapshot();
    assert_eq!(snap1.len(), 1);

    // Record more — snapshot should not change
    rec.record_axis(2, 0.6, 0.6, 2000);
    assert_eq!(snap1.len(), 1); // unchanged

    let snap2 = rec.snapshot();
    assert_eq!(snap2.len(), 2);
}

#[test]
fn recorder_debug_format() {
    let rec = BlackboxRecorder::new(RecorderConfig { capacity: 32 });
    let debug = format!("{rec:?}");
    assert!(debug.contains("BlackboxRecorder"));
    assert!(debug.contains("capacity"));
    assert!(debug.contains("32"));
}

// ═══════════════════════════════════════════════════════════════════════
// §7  Additional depth: analysis module
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn analysis_statistics_multiple_axes() {
    use flight_blackbox::analysis::axis_statistics;

    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 256 });

    // Axis 0: values 0..10
    for i in 0..10 {
        rec.record_axis(0, 0.0, i as f64, i as u64 * 1000);
    }
    // Axis 1: values 100..110
    for i in 0..10 {
        rec.record_axis(1, 0.0, (100 + i) as f64, i as u64 * 1000);
    }

    let stats0 = axis_statistics(&rec, 0).unwrap();
    assert_eq!(stats0.count, 10);
    assert!((stats0.min - 0.0).abs() < f64::EPSILON);
    assert!((stats0.max - 9.0).abs() < f64::EPSILON);

    let stats1 = axis_statistics(&rec, 1).unwrap();
    assert_eq!(stats1.count, 10);
    assert!((stats1.min - 100.0).abs() < f64::EPSILON);
    assert!((stats1.max - 109.0).abs() < f64::EPSILON);
}

#[test]
fn analysis_anomaly_clean_steady_state() {
    use flight_blackbox::analysis::{AnomalyThresholds, detect_anomalies};

    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 512 });

    // Perfectly steady 250Hz signal, value=0.5
    let cadence = 4_000_000u64;
    for i in 0..200 {
        rec.record_axis(0, 0.5, 0.5, i * cadence);
    }

    let thresholds = AnomalyThresholds {
        max_jitter_ns: 500_000,
        saturation_threshold: 0.999,
        max_gap_ns: 20_000_000,
    };
    let anomalies = detect_anomalies(&rec, &thresholds);
    assert!(
        anomalies.is_empty(),
        "clean 250Hz signal should produce no anomalies"
    );
}

#[test]
fn analysis_event_timeline_ordering() {
    use flight_blackbox::analysis::event_timeline;

    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 64 });

    // Record several events — they'll get monotonic timestamps
    rec.record_event(1, "a", &[]);
    rec.record_ffb(2, 0.5);
    rec.record_telemetry("MSFS", &[0x01]);
    rec.record_event(3, "b", &[]);

    let tl = event_timeline(&rec);
    assert_eq!(tl.len(), 4);

    // Verify chronological ordering
    for window in tl.windows(2) {
        assert!(window[0].timestamp_ns <= window[1].timestamp_ns);
    }
}

#[test]
fn analysis_summary_with_overflow() {
    use flight_blackbox::export::summary;

    let mut rec = BlackboxRecorder::new(RecorderConfig { capacity: 8 });
    for i in 0..20u16 {
        rec.record_axis(i, 0.0, i as f64, i as u64 * 1000);
    }

    let s = summary(&rec);
    assert_eq!(s.total_entries, 8);
    assert_eq!(s.overflow_count, 12);
    assert_eq!(s.axis_count, 8);
}

// ═══════════════════════════════════════════════════════════════════════
// §8  Stream type coverage
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn stream_type_index_mapping() {
    assert_eq!(StreamType::AxisFrames.to_index(), 0);
    assert_eq!(StreamType::BusSnapshots.to_index(), 1);
    assert_eq!(StreamType::Events.to_index(), 2);
}

#[test]
fn stream_type_serialization_roundtrip() {
    for st in [
        StreamType::AxisFrames,
        StreamType::BusSnapshots,
        StreamType::Events,
    ] {
        let bytes = postcard::to_stdvec(&st).unwrap();
        let decoded: StreamType = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, st);
    }
}

#[test]
fn config_defaults_are_sensible() {
    let cfg = BlackboxConfig::default();
    assert!(cfg.max_file_size_mb >= 10, "should allow at least 10MB files");
    assert!(
        cfg.max_recording_duration >= Duration::from_secs(60),
        "should allow at least 60s recordings"
    );
    assert!(!cfg.enable_compression, "compression should be off by default");
    assert!(cfg.buffer_size > 0, "buffer must be nonzero");
}
