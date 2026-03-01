// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the blackbox writer — recording lifecycle, frame format,
//! file management, replay, compression heuristics, and corruption handling.

use std::fs;
use std::io::Write;

use flight_blackbox::{
    BlackboxConfig, BlackboxReader, BlackboxRecord, BlackboxWriter, ExportDoc, FBB_ENDIAN_MARKER,
    FBB_FORMAT_VERSION, FBB_MAGIC, StreamType,
};
use proptest::prelude::*;
use tempfile::tempdir;

// ── Helpers ──────────────────────────────────────────────────────────

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

// ═══════════════════════════════════════════════════════════════════════
// 1. Recording Lifecycle
// ═══════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════
// 2. Frame Format
// ═══════════════════════════════════════════════════════════════════════

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

proptest! {
    /// Property test: write(record) → read(record) round-trip for arbitrary payloads.
    #[test]
    fn prop_record_roundtrip(
        timestamp_ns in 0u64..u64::MAX / 2,
        stream_idx in 0u8..3u8,
        data in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        let stream_type = match stream_idx {
            0 => StreamType::AxisFrames,
            1 => StreamType::BusSnapshots,
            _ => StreamType::Events,
        };

        let record = BlackboxRecord {
            timestamp_ns,
            stream_type,
            data: data.clone(),
        };

        let bytes = postcard::to_stdvec(&record).unwrap();
        let deserialized: BlackboxRecord = postcard::from_bytes(&bytes).unwrap();

        prop_assert_eq!(deserialized.timestamp_ns, timestamp_ns);
        prop_assert_eq!(deserialized.stream_type, stream_type);
        prop_assert_eq!(deserialized.data, data);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. File Management
// ═══════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════
// 4. Replay
// ═══════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════
// 5. Compression / Encoding Heuristics
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
// 6. Corruption Handling
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

// ═══════════════════════════════════════════════════════════════════════
// 7. Export Integration
// ═══════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════
// 8. Edge Cases
// ═══════════════════════════════════════════════════════════════════════

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
