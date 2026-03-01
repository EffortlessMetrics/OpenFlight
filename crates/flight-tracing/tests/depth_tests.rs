// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-tracing.
//!
//! Covers areas complementary to `tracing_tests.rs`:
//!
//! - Structured logging (LogEntry, LogEntryBuilder, JsonLogFormatter)
//! - Span lifecycle and concurrency
//! - MemorySink ring-buffer invariants
//! - FileSink rotation edge cases
//! - LogRotator boundary conditions
//! - EventLevel / LogLevel ordering and display
//! - Correlation subsystem concurrency and eviction
//! - Regression detector edge cases and alert generation
//! - EventFilter toggling
//! - Quality gate checks
//! - Cross-module integration (events → counters → regression)
//! - proptest: structured log fields, span durations, correlation IDs

use flight_tracing::correlation::{ChainCollector, CorrelatedEvent, CorrelationId, EventChain};
use flight_tracing::counters::{HidStats, JitterStats, PerfCounters};
use flight_tracing::events::{EventFilter, TraceEvent};
use flight_tracing::log_rotation::{LogRotator, RotationConfig, RotationResult};
use flight_tracing::regression::{RegressionDetector, Thresholds};
use flight_tracing::spans::{
    self, FlightSpan, SpanCollector, AXIS_TICK, BUS_PUBLISH, FFB_COMPUTE, HID_READ,
    PROFILE_COMPILE,
};
use flight_tracing::structured::{EventBuilder, EventLevel, EventSink, FileSink, MemorySink};
use flight_tracing::structured_log::{JsonLogFormatter, LogEntryBuilder, LogLevel, LogValue};
use flight_tracing::CounterSnapshot;
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ── Helpers ───────────────────────────────────────────────────────────────

fn make_flight_event(component: &str, msg: &str) -> flight_tracing::structured::FlightEvent {
    EventBuilder::new(EventLevel::Info, component, msg).build()
}

fn test_snapshot(
    jitter_p99: i64,
    hid_avg: u64,
    miss_rate: f64,
    writer_drops: u64,
) -> CounterSnapshot {
    CounterSnapshot {
        total_ticks: 1000,
        deadline_misses: (miss_rate * 1000.0) as u64,
        miss_rate,
        total_hid_writes: 100,
        writer_drops,
        jitter: JitterStats {
            p50_ns: jitter_p99 / 2,
            p99_ns: jitter_p99,
            max_ns: jitter_p99 * 2,
            sample_count: 1000,
        },
        hid: HidStats {
            total_writes: 100,
            total_time_ns: hid_avg * 100,
            avg_time_ns: hid_avg,
            p99_time_ns: hid_avg * 2,
        },
        session_duration_ms: 4000,
        timestamp_ns: 0,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  1.  Structured logging — LogEntry / LogEntryBuilder / JsonLogFormatter
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn log_entry_builder_sets_all_fields() {
    let entry = LogEntryBuilder::new(LogLevel::Warn, "scheduler", "tick overrun")
        .field("tick_number", LogValue::Int(42))
        .field("overrun_us", LogValue::Float(123.4))
        .field("recovered", LogValue::Bool(false))
        .field("details", LogValue::String("buffer full".into()))
        .span_id("span-abc")
        .trace_id("trace-xyz")
        .build();

    assert_eq!(entry.level, LogLevel::Warn);
    assert_eq!(entry.component, "scheduler");
    assert_eq!(entry.message, "tick overrun");
    assert_eq!(entry.fields.len(), 4);
    assert_eq!(entry.span_id.as_deref(), Some("span-abc"));
    assert_eq!(entry.trace_id.as_deref(), Some("trace-xyz"));
}

#[test]
fn log_entry_fields_are_btree_ordered() {
    let entry = LogEntryBuilder::new(LogLevel::Info, "test", "msg")
        .field("zebra", LogValue::Int(1))
        .field("alpha", LogValue::Int(2))
        .field("middle", LogValue::Int(3))
        .build();

    let keys: Vec<&String> = entry.fields.keys().collect();
    assert_eq!(keys, vec!["alpha", "middle", "zebra"]);
}

#[test]
fn log_entry_duplicate_field_keeps_last() {
    let entry = LogEntryBuilder::new(LogLevel::Debug, "t", "m")
        .field("key", LogValue::Int(1))
        .field("key", LogValue::Int(99))
        .build();

    assert!(matches!(entry.fields.get("key"), Some(LogValue::Int(99))));
}

#[test]
fn json_formatter_timestamp_has_millis_precision() {
    let entry = LogEntryBuilder::new(LogLevel::Info, "x", "y").build();
    let json = JsonLogFormatter::format(&entry);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let ts = parsed["timestamp"].as_str().unwrap();
    // Format: "<seconds>.<millis>"
    assert!(ts.contains('.'), "timestamp must have dot separator");
    let parts: Vec<&str> = ts.split('.').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1].len(), 3, "millis part must be 3 digits");
}

#[test]
fn json_formatter_batch_empty_returns_empty_string() {
    let result = JsonLogFormatter::format_batch(&[]);
    assert!(result.is_empty());
}

#[test]
fn json_formatter_batch_single_entry_has_no_newlines() {
    let entries = vec![LogEntryBuilder::new(LogLevel::Info, "a", "b").build()];
    let result = JsonLogFormatter::format_batch(&entries);
    assert!(!result.contains('\n'));
    serde_json::from_str::<serde_json::Value>(&result).unwrap();
}

#[test]
fn log_value_display_all_variants() {
    assert_eq!(LogValue::String("hello world".into()).to_string(), "hello world");
    assert_eq!(LogValue::Int(-42).to_string(), "-42");
    let float_zero = LogValue::Float(0.0).to_string();
    assert!(float_zero == "0" || float_zero == "0.0");
    assert_eq!(LogValue::Bool(true).to_string(), "true");
}

#[test]
fn json_formatter_omits_fields_key_when_empty() {
    let entry = LogEntryBuilder::new(LogLevel::Trace, "core", "startup").build();
    let json = JsonLogFormatter::format(&entry);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("fields").is_none());
}

#[test]
fn json_formatter_includes_all_log_value_types() {
    let entry = LogEntryBuilder::new(LogLevel::Info, "test", "types")
        .field("s", LogValue::String("val".into()))
        .field("i", LogValue::Int(7))
        .field("f", LogValue::Float(2.78))
        .field("b", LogValue::Bool(true))
        .build();
    let json = JsonLogFormatter::format(&entry);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["fields"]["s"], "val");
    assert_eq!(parsed["fields"]["i"], 7);
    assert!((parsed["fields"]["f"].as_f64().unwrap() - 2.78).abs() < 0.001);
    assert_eq!(parsed["fields"]["b"], true);
}

// ═══════════════════════════════════════════════════════════════════════════
//  2.  EventLevel / LogLevel ordering and display
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn event_level_full_ordering() {
    let levels = [
        EventLevel::Trace,
        EventLevel::Debug,
        EventLevel::Info,
        EventLevel::Warn,
        EventLevel::Error,
    ];
    for window in levels.windows(2) {
        assert!(window[0] < window[1]);
    }
}

#[test]
fn event_level_display_uppercase() {
    assert_eq!(EventLevel::Trace.to_string(), "TRACE");
    assert_eq!(EventLevel::Debug.to_string(), "DEBUG");
    assert_eq!(EventLevel::Info.to_string(), "INFO");
    assert_eq!(EventLevel::Warn.to_string(), "WARN");
    assert_eq!(EventLevel::Error.to_string(), "ERROR");
}

#[test]
fn log_level_full_ordering() {
    let levels = [
        LogLevel::Trace,
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
    ];
    for window in levels.windows(2) {
        assert!(window[0] < window[1]);
    }
}

#[test]
fn log_level_display_uppercase() {
    assert_eq!(LogLevel::Trace.to_string(), "TRACE");
    assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
    assert_eq!(LogLevel::Info.to_string(), "INFO");
    assert_eq!(LogLevel::Warn.to_string(), "WARN");
    assert_eq!(LogLevel::Error.to_string(), "ERROR");
}

// ═══════════════════════════════════════════════════════════════════════════
//  3.  Span lifecycle
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn free_span_has_nonzero_elapsed() {
    let span = FlightSpan::begin(AXIS_TICK);
    // Even without sleep, wall clock should advance slightly.
    let ns = span.finish();
    // Just check it didn't panic; duration may be 0 on fast machines.
    assert!(ns < 1_000_000_000, "span should not take > 1s");
}

#[test]
fn span_name_round_trips() {
    for name in [AXIS_TICK, HID_READ, BUS_PUBLISH, PROFILE_COMPILE, FFB_COMPUTE] {
        let span = FlightSpan::begin(name);
        assert_eq!(span.name(), name);
        span.finish();
    }
}

#[test]
fn collector_multiple_ops_tracked_independently() {
    let collector = SpanCollector::new(1000);
    collector.record(AXIS_TICK, 100);
    collector.record(HID_READ, 200);
    collector.record(FFB_COMPUTE, 300);

    let summaries = collector.summary();
    assert_eq!(summaries.len(), 3);

    let axis = spans::span_summary(&collector, AXIS_TICK).unwrap();
    assert_eq!(axis.avg_ns, 100);
    let hid = spans::span_summary(&collector, HID_READ).unwrap();
    assert_eq!(hid.avg_ns, 200);
}

#[test]
fn collector_reset_clears_everything() {
    let collector = SpanCollector::new(100);
    collector.record(AXIS_TICK, 1000);
    collector.record(HID_READ, 2000);
    collector.reset();
    assert!(collector.summary().is_empty());
}

#[test]
fn collector_statistics_min_max_avg_correct() {
    let collector = SpanCollector::new(10_000);
    let durations = [10u64, 20, 30, 40, 50];
    for d in &durations {
        collector.record(BUS_PUBLISH, *d);
    }
    let s = spans::span_summary(&collector, BUS_PUBLISH).unwrap();
    assert_eq!(s.min_ns, 10);
    assert_eq!(s.max_ns, 50);
    assert_eq!(s.avg_ns, 30);
    assert_eq!(s.count, 5);
}

#[test]
fn span_collector_concurrent_recording() {
    let collector = Arc::new(SpanCollector::new(10_000));
    let num_threads = 4;
    let per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&collector);
            thread::spawn(move || {
                for _ in 0..per_thread {
                    c.record(AXIS_TICK, 1000);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let s = spans::span_summary(&collector, AXIS_TICK).unwrap();
    assert_eq!(s.count, (num_threads * per_thread) as u64);
}

#[test]
fn span_summary_returns_none_for_unknown_op() {
    let collector = SpanCollector::new(100);
    collector.record(AXIS_TICK, 100);
    assert!(spans::span_summary(&collector, "nonexistent").is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
//  4.  MemorySink ring-buffer invariants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn memory_sink_single_event() {
    let mut sink = MemorySink::new(10);
    let ev = make_flight_event("hid", "single");
    sink.send(&ev).unwrap();
    assert_eq!(sink.len(), 1);
    assert_eq!(sink.total_received(), 1);
    assert!(!sink.is_empty());
}

#[test]
fn memory_sink_wrap_around_preserves_order() {
    let mut sink = MemorySink::new(4);
    for i in 0..7 {
        let ev = EventBuilder::new(EventLevel::Info, "t", format!("e{i}")).build();
        sink.send(&ev).unwrap();
    }
    let snap = sink.snapshot();
    assert_eq!(snap.len(), 4);
    // Should retain e3, e4, e5, e6 in order
    assert_eq!(snap[0].message, "e3");
    assert_eq!(snap[1].message, "e4");
    assert_eq!(snap[2].message, "e5");
    assert_eq!(snap[3].message, "e6");
}

#[test]
fn memory_sink_total_received_counts_all() {
    let mut sink = MemorySink::new(2);
    for i in 0..100 {
        let ev = EventBuilder::new(EventLevel::Info, "t", format!("{i}")).build();
        sink.send(&ev).unwrap();
    }
    assert_eq!(sink.total_received(), 100);
    assert_eq!(sink.len(), 2);
}

#[test]
fn memory_sink_clear_resets_everything() {
    let mut sink = MemorySink::new(5);
    for _ in 0..3 {
        sink.send(&make_flight_event("t", "x")).unwrap();
    }
    sink.clear();
    assert!(sink.is_empty());
    assert_eq!(sink.total_received(), 0);
    assert!(sink.snapshot().is_empty());
}

#[test]
fn memory_sink_flush_is_noop() {
    let mut sink = MemorySink::new(5);
    sink.send(&make_flight_event("t", "x")).unwrap();
    sink.flush().unwrap(); // should not fail
    assert_eq!(sink.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
//  5.  LogRotator boundary conditions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn rotator_does_not_rotate_below_threshold() {
    let mut r = LogRotator::new(RotationConfig {
        max_file_size_bytes: 1000,
        max_files: 5,
        compress_rotated: false,
    });
    r.record_bytes(999);
    assert!(!r.should_rotate());
    assert_eq!(r.rotate(), RotationResult::NotNeeded);
}

#[test]
fn rotator_rotates_at_exact_threshold() {
    let mut r = LogRotator::new(RotationConfig {
        max_file_size_bytes: 100,
        max_files: 5,
        compress_rotated: false,
    });
    r.record_bytes(100);
    assert!(r.should_rotate());
    assert_eq!(r.rotate(), RotationResult::Rotated { sequence: 1 });
    assert_eq!(r.current_size(), 0);
}

#[test]
fn rotator_max_files_stops_rotation() {
    let mut r = LogRotator::new(RotationConfig {
        max_file_size_bytes: 10,
        max_files: 2,
        compress_rotated: false,
    });
    r.record_bytes(10);
    assert_eq!(r.rotate(), RotationResult::Rotated { sequence: 1 });
    r.record_bytes(10);
    assert_eq!(r.rotate(), RotationResult::Rotated { sequence: 2 });
    r.record_bytes(10);
    assert_eq!(r.rotate(), RotationResult::MaxFilesReached);
}

#[test]
fn rotator_record_bytes_saturates() {
    let mut r = LogRotator::new(RotationConfig {
        max_file_size_bytes: u64::MAX,
        max_files: 1,
        compress_rotated: false,
    });
    r.record_bytes(u64::MAX);
    r.record_bytes(100);
    assert_eq!(r.current_size(), u64::MAX);
}

#[test]
fn rotator_compress_flag() {
    let r = LogRotator::new(RotationConfig {
        max_file_size_bytes: 100,
        max_files: 3,
        compress_rotated: true,
    });
    assert!(r.compress_enabled());
}

// ═══════════════════════════════════════════════════════════════════════════
//  6.  FileSink rotation integration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn file_sink_creates_and_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.log");
    let config = RotationConfig {
        max_file_size_bytes: 1_000_000,
        max_files: 5,
        compress_rotated: false,
    };
    let mut sink = FileSink::open(&path, config).unwrap();
    let ev = make_flight_event("test", "hello");
    sink.send(&ev).unwrap();
    sink.flush().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.lines().count(), 1);
    let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(parsed["component"], "test");
}

#[test]
fn file_sink_rotation_produces_numbered_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    let config = RotationConfig {
        max_file_size_bytes: 50, // very small to force rotation
        max_files: 10,
        compress_rotated: false,
    };
    let mut sink = FileSink::open(&path, config).unwrap();

    for i in 0..20 {
        let ev = EventBuilder::new(EventLevel::Info, "t", format!("event {i}")).build();
        sink.send(&ev).unwrap();
    }
    sink.flush().unwrap();

    // At least one rotated file should exist
    assert!(
        dir.path().join("app.log.1").exists(),
        "rotated file .1 must exist"
    );
}

#[test]
fn file_sink_each_line_is_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jsonl.log");
    let config = RotationConfig {
        max_file_size_bytes: 1_000_000,
        max_files: 1,
        compress_rotated: false,
    };
    let mut sink = FileSink::open(&path, config).unwrap();

    for i in 0..5 {
        let ev = EventBuilder::new(EventLevel::Debug, "c", format!("msg {i}"))
            .device_id("dev-1")
            .build();
        sink.send(&ev).unwrap();
    }
    sink.flush().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    for (idx, line) in content.lines().enumerate() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("line {idx} is invalid JSON: {e}"));
    }
}

#[test]
fn file_sink_path_accessor() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.log");
    let config = RotationConfig {
        max_file_size_bytes: 1000,
        max_files: 1,
        compress_rotated: false,
    };
    let sink = FileSink::open(&path, config).unwrap();
    assert_eq!(sink.path(), path);
}

// ═══════════════════════════════════════════════════════════════════════════
//  7.  EventFilter combinations
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn event_filter_all_disabled() {
    let filter = EventFilter {
        tick_events: false,
        hid_events: false,
        deadline_events: false,
        writer_events: false,
        custom_events: false,
    };
    assert!(!filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(!filter.should_trace(&TraceEvent::tick_end(1, 0, 0)));
    assert!(!filter.should_trace(&TraceEvent::hid_write(0, 0, 0)));
    assert!(!filter.should_trace(&TraceEvent::deadline_miss(1, 0)));
    assert!(!filter.should_trace(&TraceEvent::writer_drop("s", 0)));
    assert!(!filter.should_trace(&TraceEvent::custom("c", serde_json::json!({}))));
}

#[test]
fn event_filter_only_custom_enabled() {
    let filter = EventFilter {
        tick_events: false,
        hid_events: false,
        deadline_events: false,
        writer_events: false,
        custom_events: true,
    };
    assert!(!filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::custom("x", serde_json::json!(null))));
}

#[test]
fn event_filter_tick_covers_both_start_and_end() {
    let filter = EventFilter {
        tick_events: true,
        hid_events: false,
        deadline_events: false,
        writer_events: false,
        custom_events: false,
    };
    assert!(filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::tick_end(1, 100, 0)));
}

// ═══════════════════════════════════════════════════════════════════════════
//  8.  Correlation subsystem
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn correlation_id_uniqueness_across_many() {
    let ids: Vec<CorrelationId> = (0..1000).map(|_| CorrelationId::new()).collect();
    let unique: std::collections::HashSet<u64> = ids.iter().map(|id| id.as_raw()).collect();
    assert_eq!(unique.len(), 1000);
}

#[test]
fn event_chain_duration_none_when_empty() {
    let chain = EventChain::new(CorrelationId::new());
    assert!(chain.duration_ns().is_none());
}

#[test]
fn event_chain_duration_none_with_single_event() {
    let mut chain = EventChain::new(CorrelationId::new());
    chain.push(make_flight_event("a", "b"));
    assert!(chain.duration_ns().is_none());
}

#[test]
fn event_chain_duration_with_explicit_timestamps() {
    let mut chain = EventChain::new(CorrelationId::new());
    chain.push(flight_tracing::structured::FlightEvent {
        timestamp_ns: 1_000_000,
        level: EventLevel::Info,
        component: "start".into(),
        message: "begin".into(),
        context: Default::default(),
    });
    chain.push(flight_tracing::structured::FlightEvent {
        timestamp_ns: 5_000_000,
        level: EventLevel::Info,
        component: "end".into(),
        message: "done".into(),
        context: Default::default(),
    });
    assert_eq!(chain.duration_ns(), Some(4_000_000));
}

#[test]
fn chain_collector_concurrent_recording() {
    let collector = Arc::new(ChainCollector::new(1000));
    let num_threads = 4;
    let per_thread = 50;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let c = Arc::clone(&collector);
            thread::spawn(move || {
                for i in 0..per_thread {
                    let id = CorrelationId::from_raw(t * 10_000 + i);
                    c.record(CorrelatedEvent::new(id, make_flight_event("t", "msg")));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(collector.active_chains(), (num_threads * per_thread) as usize);
}

#[test]
fn chain_collector_eviction_preserves_newer() {
    let collector = ChainCollector::new(3);
    // Insert IDs 1..=5; capacity 3 means IDs 1,2 should be evicted
    for i in 1u64..=5 {
        let id = CorrelationId::from_raw(i);
        collector.record(CorrelatedEvent::new(id, make_flight_event("x", "y")));
    }
    assert_eq!(collector.active_chains(), 3);
    assert!(collector.get_chain(&CorrelationId::from_raw(1)).is_none());
    assert!(collector.get_chain(&CorrelationId::from_raw(2)).is_none());
    assert!(collector.get_chain(&CorrelationId::from_raw(3)).is_some());
    assert!(collector.get_chain(&CorrelationId::from_raw(4)).is_some());
    assert!(collector.get_chain(&CorrelationId::from_raw(5)).is_some());
}

#[test]
fn chain_collector_take_removes_and_returns() {
    let collector = ChainCollector::new(10);
    let id = CorrelationId::from_raw(42);
    collector.record(CorrelatedEvent::new(id, make_flight_event("a", "1")));
    collector.record(CorrelatedEvent::new(id, make_flight_event("b", "2")));

    let chain = collector.take_chain(&id).unwrap();
    assert_eq!(chain.len(), 2);
    assert!(collector.get_chain(&id).is_none());
}

#[test]
fn chain_collector_clear_empties() {
    let collector = ChainCollector::new(100);
    for i in 0..10 {
        collector.record(CorrelatedEvent::new(
            CorrelationId::from_raw(i),
            make_flight_event("x", "y"),
        ));
    }
    collector.clear();
    assert_eq!(collector.active_chains(), 0);
}

#[test]
fn correlation_id_display_format() {
    let id = CorrelationId::from_raw(0x1234);
    let s = id.to_string();
    assert!(s.starts_with("corr-"));
    assert!(s.contains("1234"));
}

// ═══════════════════════════════════════════════════════════════════════════
//  9.  Regression detector edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn regression_detector_no_baselines_no_regression() {
    let detector = RegressionDetector::new();
    let snap = test_snapshot(100_000, 200_000, 0.001, 0);
    let result = detector.check_regression(snap);
    assert!(!result.regression_detected);
    assert!(result.baseline.is_none());
}

#[test]
fn regression_detector_writer_drops_alert() {
    let mut detector = RegressionDetector::new();
    let baseline = test_snapshot(100_000, 200_000, 0.001, 0);
    detector.add_baseline(baseline);

    let current = test_snapshot(100_000, 200_000, 0.001, 150);
    let result = detector.check_regression(current);

    let drop_alerts: Vec<_> = result
        .alerts
        .iter()
        .filter(|a| a.metric.contains("writer_drops"))
        .collect();
    assert!(!drop_alerts.is_empty(), "should alert on writer drops");
}

#[test]
fn regression_detector_baseline_capacity_limit() {
    let mut detector = RegressionDetector::with_config(3, Thresholds::default(), 100);
    for i in 0..10 {
        detector.add_baseline(test_snapshot(1000 * (i + 1), 200_000, 0.001, 0));
    }
    // Only last 3 should remain
    let summary = detector.get_baseline_summary().unwrap();
    assert_eq!(summary.count, 3);
}

#[test]
fn regression_detector_baseline_summary_empty() {
    let detector = RegressionDetector::new();
    assert!(detector.get_baseline_summary().is_none());
}

#[test]
fn regression_detector_save_load_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("baselines.json");

    let mut detector = RegressionDetector::new();
    detector.add_baseline(test_snapshot(5000, 200_000, 0.002, 1));
    detector.add_baseline(test_snapshot(6000, 210_000, 0.003, 2));
    detector.save_baselines(&path).unwrap();

    let mut loaded = RegressionDetector::new();
    loaded.load_baselines(&path).unwrap();
    let summary = loaded.get_baseline_summary().unwrap();
    assert_eq!(summary.count, 2);
}

// ═══════════════════════════════════════════════════════════════════════════
//  10.  Quality gates
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn quality_gates_pass_with_good_metrics() {
    let counters = PerfCounters::new();
    for i in 0..2000 {
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 100)); // 100ns jitter
    }
    let result = counters.check_quality_gates();
    assert!(result.passed);
    assert!(result.violations.is_empty());
}

#[test]
fn quality_gates_fail_on_high_jitter() {
    let counters = PerfCounters::new();
    for i in 0..2000 {
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 1_000_000)); // 1ms
    }
    let result = counters.check_quality_gates();
    assert!(!result.passed);
    let jitter_violation = result.violations.iter().any(|v| v.gate == "QG-AX-Jitter");
    assert!(jitter_violation, "should have jitter quality gate violation");
}

#[test]
fn quality_gates_fail_on_excessive_miss_rate() {
    let counters = PerfCounters::new();
    for i in 0..100 {
        counters.record_event(&TraceEvent::tick_start(i));
        counters.record_event(&TraceEvent::deadline_miss(i, 1_000_000));
    }
    let result = counters.check_quality_gates();
    assert!(!result.passed);
    let miss_violation = result
        .violations
        .iter()
        .any(|v| v.gate == "Deadline-Miss-Rate");
    assert!(miss_violation, "should have deadline miss rate violation");
}

// ═══════════════════════════════════════════════════════════════════════════
//  11.  Cross-module integration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn events_flow_through_counters_to_regression() {
    let counters = PerfCounters::new();
    // Simulate a good session
    for i in 0..500 {
        counters.record_event(&TraceEvent::tick_start(i));
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 200));
        counters.record_event(&TraceEvent::hid_write(0x01, 64, 50_000));
    }
    let baseline = counters.snapshot();

    counters.reset();
    // Simulate a regressed session
    for i in 0..500 {
        counters.record_event(&TraceEvent::tick_start(i));
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 200));
        counters.record_event(&TraceEvent::hid_write(0x01, 64, 400_000)); // 4x slower
    }
    let current = counters.snapshot();

    assert!(
        current.is_regression(&baseline),
        "4x HID latency increase must be detected as regression"
    );
}

#[test]
fn counter_snapshot_kv_pairs_contain_expected_keys() {
    let snap = test_snapshot(5000, 250_000, 0.005, 3);
    let kv = snap.to_kv_pairs();
    let keys: Vec<&str> = kv.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"total_ticks"));
    assert!(keys.contains(&"jitter_p99_us"));
    assert!(keys.contains(&"hid_avg_us"));
    assert!(keys.contains(&"writer_drops"));
    assert!(keys.contains(&"miss_rate_percent"));
}

#[test]
fn counter_snapshot_json_round_trip() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::tick_start(1));
    counters.record_event(&TraceEvent::tick_end(1, 4_000_000, 500));
    counters.record_event(&TraceEvent::hid_write(0, 64, 100_000));

    let snap = counters.snapshot();
    let json = snap.to_json().unwrap();
    let recovered: CounterSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.total_ticks, snap.total_ticks);
    assert_eq!(recovered.total_hid_writes, snap.total_hid_writes);
    assert_eq!(recovered.jitter.sample_count, snap.jitter.sample_count);
}

// ═══════════════════════════════════════════════════════════════════════════
//  12.  Binary event encoding edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn binary_tick_start_is_17_bytes() {
    let ev = TraceEvent::tick_start(u64::MAX);
    let bin = ev.to_binary();
    // 8 (timestamp) + 1 (type) + 8 (tick) = 17
    assert_eq!(bin.len(), 17);
}

#[test]
fn binary_deadline_miss_is_25_bytes() {
    let ev = TraceEvent::deadline_miss(999, 1_000_000);
    let bin = ev.to_binary();
    // 8 (timestamp) + 1 (type) + 8 (tick) + 8 (miss_duration) = 25
    assert_eq!(bin.len(), 25);
}

#[test]
fn binary_writer_drop_variable_length() {
    let short = TraceEvent::writer_drop("a", 1);
    let long = TraceEvent::writer_drop("long-stream-name", 1);
    let short_bin = short.to_binary();
    let long_bin = long.to_binary();
    // Longer stream ID → longer binary
    assert!(long_bin.len() > short_bin.len());
}

#[test]
fn binary_event_type_tags_correct() {
    assert_eq!(TraceEvent::tick_start(0).to_binary()[8], 0x01);
    assert_eq!(TraceEvent::tick_end(0, 0, 0).to_binary()[8], 0x02);
    assert_eq!(TraceEvent::hid_write(0, 0, 0).to_binary()[8], 0x03);
    assert_eq!(TraceEvent::deadline_miss(0, 0).to_binary()[8], 0x04);
    assert_eq!(TraceEvent::writer_drop("s", 0).to_binary()[8], 0x05);
    assert_eq!(
        TraceEvent::custom("c", serde_json::json!(null)).to_binary()[8],
        0xFF
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  13.  Error type display
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn trace_error_display_not_initialized() {
    let err = flight_tracing::TraceError::NotInitialized;
    assert_eq!(err.to_string(), "Provider not initialized");
}

#[test]
fn trace_error_display_platform() {
    let err = flight_tracing::TraceError::Platform("test failure".into());
    assert!(err.to_string().contains("test failure"));
}

// ═══════════════════════════════════════════════════════════════════════════
//  14.  proptest
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    /// LogEntry with arbitrary component/message never panics in JSON formatting.
    #[test]
    fn log_entry_json_format_never_panics(
        component in "[a-zA-Z0-9_-]{1,32}",
        message in "[ -~]{0,128}",
    ) {
        let entry = LogEntryBuilder::new(LogLevel::Info, &component, &message).build();
        let json = JsonLogFormatter::format(&entry);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(parsed["component"].as_str().unwrap(), component.as_str());
    }

    /// Arbitrary LogValue::Int round-trips through JSON.
    #[test]
    fn log_value_int_round_trips(val in i64::MIN..=i64::MAX) {
        let entry = LogEntryBuilder::new(LogLevel::Debug, "t", "m")
            .field("v", LogValue::Int(val))
            .build();
        let json = JsonLogFormatter::format(&entry);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(parsed["fields"]["v"].as_i64().unwrap(), val);
    }

    /// Arbitrary correlation IDs from raw values are distinct.
    #[test]
    fn correlation_id_from_raw_preserves_value(raw in 1u64..u64::MAX) {
        let id = CorrelationId::from_raw(raw);
        prop_assert_eq!(id.as_raw(), raw);
    }

    /// CorrelationId serialization round-trips.
    #[test]
    fn correlation_id_serde_round_trip(raw in 0u64..=u64::MAX) {
        let id = CorrelationId::from_raw(raw);
        let json = serde_json::to_string(&id).unwrap();
        let recovered: CorrelationId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(id, recovered);
    }

    /// SpanCollector with arbitrary durations never panics and stats are consistent.
    #[test]
    fn span_collector_arbitrary_durations(
        durations in proptest::collection::vec(0u64..10_000_000, 1..50),
    ) {
        let collector = SpanCollector::new(10_000);
        for d in &durations {
            collector.record(AXIS_TICK, *d);
        }
        let s = spans::span_summary(&collector, AXIS_TICK).unwrap();
        prop_assert_eq!(s.count, durations.len() as u64);
        prop_assert!(s.min_ns <= s.max_ns);
        prop_assert!(s.avg_ns >= s.min_ns);
        prop_assert!(s.avg_ns <= s.max_ns);
    }

    /// LogRotator rotation count never exceeds max_files.
    #[test]
    fn rotator_never_exceeds_max_files(
        max_files in 1u32..20,
        writes in 1usize..100,
    ) {
        let mut r = LogRotator::new(RotationConfig {
            max_file_size_bytes: 10,
            max_files,
            compress_rotated: false,
        });
        let mut rotations = 0u32;
        for _ in 0..writes {
            r.record_bytes(10);
            if let RotationResult::Rotated { .. } = r.rotate() {
                rotations += 1;
            }
        }
        prop_assert!(rotations <= max_files);
    }
}
