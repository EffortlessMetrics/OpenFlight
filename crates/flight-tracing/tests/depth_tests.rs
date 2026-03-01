// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the tracing / diagnostics system.
//!
//! Covers structured logging, trace export, diagnostic bundles,
//! performance tracing, event correlation, and property-based tests.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use flight_tracing::correlation::{ChainCollector, CorrelatedEvent, CorrelationId, EventChain};
use flight_tracing::counters::PerfCounters;
use flight_tracing::events::{EventData, EventFilter, TraceEvent};
use flight_tracing::log_rotation::{LogRotator, RotationConfig, RotationResult};
use flight_tracing::spans::{
    self, FlightSpan, SpanCollector, AXIS_TICK, BUS_PUBLISH, FFB_COMPUTE, HID_READ,
};
use flight_tracing::structured::{
    EventBuilder, EventContext, EventLevel, EventSink, FileSink, FlightEvent, MemorySink,
};
use flight_tracing::structured_log::{
    JsonLogFormatter, LogEntry, LogEntryBuilder, LogLevel, LogValue,
};

use proptest::prelude::*;

// ═══════════════════════════════════════════════════════════════════════════
// 1. Structured logging
// ═══════════════════════════════════════════════════════════════════════════

mod structured_logging {
    use super::*;

    // -- Log events with structured fields --

    #[test]
    fn event_builder_all_context_fields_populated() {
        let ev = EventBuilder::new(EventLevel::Info, "axis", "curve applied")
            .device_id("js-0")
            .sim_name("MSFS")
            .axis_name("pitch")
            .profile_name("f18-carrier")
            .build();

        assert_eq!(ev.context.device_id.as_deref(), Some("js-0"));
        assert_eq!(ev.context.sim_name.as_deref(), Some("MSFS"));
        assert_eq!(ev.context.axis_name.as_deref(), Some("pitch"));
        assert_eq!(ev.context.profile_name.as_deref(), Some("f18-carrier"));
        assert!(ev.timestamp_ns > 0);
    }

    #[test]
    fn log_entry_with_multiple_typed_fields() {
        let entry = LogEntryBuilder::new(LogLevel::Info, "ffb", "force computed")
            .field("force_n", LogValue::Float(12.5))
            .field("device_id", LogValue::Int(42))
            .field("clamped", LogValue::Bool(false))
            .field("axis", LogValue::String("pitch".into()))
            .build();

        assert_eq!(entry.fields.len(), 4);
        assert!(matches!(
            entry.fields.get("force_n"),
            Some(LogValue::Float(f)) if (*f - 12.5).abs() < f64::EPSILON
        ));
        assert!(matches!(
            entry.fields.get("device_id"),
            Some(LogValue::Int(42))
        ));
        assert!(matches!(
            entry.fields.get("clamped"),
            Some(LogValue::Bool(false))
        ));
        assert!(matches!(
            entry.fields.get("axis"),
            Some(LogValue::String(s)) if s == "pitch"
        ));
    }

    #[test]
    fn structured_event_json_preserves_optional_none_fields() {
        let ev = EventBuilder::new(EventLevel::Debug, "bus", "publish").build();
        let json = ev.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // None fields must be absent (skip_serializing_if)
        assert!(parsed["context"].get("device_id").is_none());
        assert!(parsed["context"].get("sim_name").is_none());
        assert!(parsed["context"].get("axis_name").is_none());
        assert!(parsed["context"].get("profile_name").is_none());
    }

    // -- Log levels: trace, debug, info, warn, error --

    #[test]
    fn all_event_levels_round_trip_through_json() {
        let levels = [
            EventLevel::Trace,
            EventLevel::Debug,
            EventLevel::Info,
            EventLevel::Warn,
            EventLevel::Error,
        ];

        for level in levels {
            let ev = EventBuilder::new(level, "test", "msg").build();
            let json = ev.to_json().unwrap();
            let recovered: FlightEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered.level, level, "level {level:?} must round-trip");
        }
    }

    #[test]
    fn all_log_levels_produce_valid_json() {
        let levels = [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];

        for level in levels {
            let entry = LogEntryBuilder::new(level, "comp", "message").build();
            let json = JsonLogFormatter::format(&entry);
            let parsed: serde_json::Value =
                serde_json::from_str(&json).expect("each level must produce valid JSON");
            assert_eq!(parsed["level"], level.to_string());
        }
    }

    // -- Context propagation (span IDs) --

    #[test]
    fn span_and_trace_ids_propagate_through_json() {
        let entry = LogEntryBuilder::new(LogLevel::Info, "axis", "tick")
            .span_id("span-abc-123")
            .trace_id("trace-xyz-789")
            .build();

        let json = JsonLogFormatter::format(&entry);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["span_id"], "span-abc-123");
        assert_eq!(parsed["trace_id"], "trace-xyz-789");
    }

    #[test]
    fn multiple_entries_share_trace_id_with_distinct_span_ids() {
        let trace_id = "trace-pipeline-001";
        let entries: Vec<LogEntry> = ["hid", "axis", "sim"]
            .iter()
            .enumerate()
            .map(|(i, comp)| {
                LogEntryBuilder::new(LogLevel::Info, comp, "processing")
                    .trace_id(trace_id)
                    .span_id(&format!("span-{i}"))
                    .build()
            })
            .collect();

        let batch = JsonLogFormatter::format_batch(&entries);
        let lines: Vec<serde_json::Value> = batch
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        // All share the same trace ID
        for line in &lines {
            assert_eq!(line["trace_id"], trace_id);
        }
        // Span IDs are distinct
        let span_ids: HashSet<&str> = lines.iter().map(|l| l["span_id"].as_str().unwrap()).collect();
        assert_eq!(span_ids.len(), 3);
    }

    // -- Log filtering by level and target --

    #[test]
    fn event_filter_by_level_comparison() {
        // EventLevel implements Ord, so we can filter by minimum level
        let min_level = EventLevel::Warn;
        let events: Vec<FlightEvent> = [
            EventLevel::Trace,
            EventLevel::Debug,
            EventLevel::Info,
            EventLevel::Warn,
            EventLevel::Error,
        ]
        .iter()
        .map(|&level| EventBuilder::new(level, "test", "msg").build())
        .collect();

        let filtered: Vec<&FlightEvent> = events.iter().filter(|e| e.level >= min_level).collect();
        assert_eq!(filtered.len(), 2); // Warn + Error
        assert!(filtered.iter().all(|e| e.level >= EventLevel::Warn));
    }

    #[test]
    fn event_filter_by_component_target() {
        let events: Vec<FlightEvent> = ["hid", "axis", "ffb", "axis", "sim"]
            .iter()
            .map(|comp| EventBuilder::new(EventLevel::Info, *comp, "event").build())
            .collect();

        let axis_events: Vec<&FlightEvent> =
            events.iter().filter(|e| e.component == "axis").collect();
        assert_eq!(axis_events.len(), 2);
    }

    #[test]
    fn memory_sink_acts_as_level_filtered_buffer() {
        let mut sink = MemorySink::new(100);
        let min_level = EventLevel::Warn;

        for &level in &[
            EventLevel::Trace,
            EventLevel::Debug,
            EventLevel::Info,
            EventLevel::Warn,
            EventLevel::Error,
        ] {
            let ev = EventBuilder::new(level, "test", &format!("{level:?}")).build();
            if ev.level >= min_level {
                sink.send(&ev).unwrap();
            }
        }

        assert_eq!(sink.len(), 2);
        let snap = sink.snapshot();
        assert!(snap.iter().all(|e| e.level >= EventLevel::Warn));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Trace export
// ═══════════════════════════════════════════════════════════════════════════

mod trace_export {
    use super::*;

    // -- Export traces to file --

    #[test]
    fn file_sink_creates_and_writes_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace.log");
        let config = RotationConfig {
            max_file_size_bytes: 1_000_000,
            max_files: 5,
            compress_rotated: false,
        };

        let mut sink = FileSink::open(&path, config).unwrap();
        let ev = EventBuilder::new(EventLevel::Info, "axis", "tick 1").build();
        sink.send(&ev).unwrap();
        sink.flush().unwrap();

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty());
    }

    // -- JSON format output --

    #[test]
    fn file_sink_writes_valid_json_lines_with_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace.log");
        let config = RotationConfig {
            max_file_size_bytes: 1_000_000,
            max_files: 5,
            compress_rotated: false,
        };

        let mut sink = FileSink::open(&path, config).unwrap();
        let events: Vec<FlightEvent> = (0..5)
            .map(|i| {
                EventBuilder::new(EventLevel::Info, "axis", &format!("tick {i}"))
                    .device_id(&format!("dev-{i}"))
                    .sim_name("MSFS")
                    .build()
            })
            .collect();

        for ev in &events {
            sink.send(ev).unwrap();
        }
        sink.flush().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 5);

        for (i, line) in lines.iter().enumerate() {
            let parsed: FlightEvent = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("line {i} not valid FlightEvent JSON: {e}"));
            assert_eq!(parsed.component, "axis");
            assert_eq!(parsed.context.sim_name.as_deref(), Some("MSFS"));
        }
    }

    #[test]
    fn json_log_formatter_batch_produces_parseable_ndjson() {
        let entries: Vec<LogEntry> = (0..10)
            .map(|i| {
                LogEntryBuilder::new(LogLevel::Info, "test", &format!("event {i}"))
                    .field("index", LogValue::Int(i))
                    .build()
            })
            .collect();

        let batch = JsonLogFormatter::format_batch(&entries);
        let lines: Vec<&str> = batch.lines().collect();
        assert_eq!(lines.len(), 10);

        for line in &lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed["timestamp"].is_string());
            assert!(parsed["fields"]["index"].is_number());
        }
    }

    // -- Trace rotation (file size limits) --

    #[test]
    fn file_sink_rotates_when_size_exceeded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.log");
        let config = RotationConfig {
            max_file_size_bytes: 100, // Tiny limit
            max_files: 10,
            compress_rotated: false,
        };

        let mut sink = FileSink::open(&path, config).unwrap();
        for i in 0..20 {
            let ev = EventBuilder::new(EventLevel::Info, "test", &format!("event {i}")).build();
            sink.send(&ev).unwrap();
        }
        sink.flush().unwrap();

        // Rotated files should exist
        let rotated_1 = dir.path().join("app.log.1");
        assert!(rotated_1.exists(), "first rotated file must exist");

        // Current file should still be writable
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn log_rotator_respects_max_files_limit() {
        let mut rotator = LogRotator::new(RotationConfig {
            max_file_size_bytes: 10,
            max_files: 3,
            compress_rotated: false,
        });

        for _ in 0..5 {
            rotator.record_bytes(10);
            let result = rotator.rotate();
            if rotator.rotation_count() > 3 {
                // Should not happen — max_files caps it
                panic!("rotation_count exceeded max_files");
            }
            if result == RotationResult::MaxFilesReached {
                break;
            }
        }

        assert_eq!(rotator.rotation_count(), 3);
    }

    // -- Concurrent writes don't corrupt --

    #[test]
    fn concurrent_memory_sink_writes_preserve_event_count() {
        let sink = Arc::new(parking_lot::Mutex::new(MemorySink::new(10_000)));
        let num_threads = 4;
        let events_per_thread = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let sink = Arc::clone(&sink);
                thread::spawn(move || {
                    for i in 0..events_per_thread {
                        let ev = EventBuilder::new(
                            EventLevel::Info,
                            "test",
                            &format!("t{t}-e{i}"),
                        )
                        .build();
                        sink.lock().send(&ev).unwrap();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let guard = sink.lock();
        assert_eq!(guard.total_received(), num_threads * events_per_thread);
        assert_eq!(guard.len(), num_threads * events_per_thread);
    }

    #[test]
    fn concurrent_file_sink_writes_produce_valid_json_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("concurrent.log");
        let config = RotationConfig {
            max_file_size_bytes: 10_000_000,
            max_files: 5,
            compress_rotated: false,
        };

        // Write sequentially from multiple threads via mutex
        let sink = Arc::new(parking_lot::Mutex::new(FileSink::open(&path, config).unwrap()));
        let num_threads = 4;
        let events_per_thread = 50;

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let sink = Arc::clone(&sink);
                thread::spawn(move || {
                    for i in 0..events_per_thread {
                        let ev = EventBuilder::new(
                            EventLevel::Info,
                            &format!("thread-{t}"),
                            &format!("event-{i}"),
                        )
                        .build();
                        sink.lock().send(&ev).unwrap();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
        sink.lock().flush().unwrap();

        // Every line must be valid JSON
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), num_threads * events_per_thread);
        for (i, line) in lines.iter().enumerate() {
            serde_json::from_str::<FlightEvent>(line)
                .unwrap_or_else(|e| panic!("line {i} corrupted: {e}"));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Diagnostic bundles
// ═══════════════════════════════════════════════════════════════════════════

mod diagnostic_bundles {
    use super::*;

    /// Simulated diagnostic bundle that collects logs, config, system info.
    struct DiagnosticBundle {
        logs: Vec<FlightEvent>,
        config_json: String,
        system_info: HashMap<String, String>,
        recent_events: Vec<TraceEvent>,
    }

    impl DiagnosticBundle {
        fn collect(sink: &MemorySink, counters: &PerfCounters) -> Self {
            let snapshot = counters.snapshot();
            let mut system_info = HashMap::new();
            system_info.insert("os".into(), std::env::consts::OS.into());
            system_info.insert("arch".into(), std::env::consts::ARCH.into());
            system_info.insert(
                "total_ticks".into(),
                snapshot.total_ticks.to_string(),
            );

            Self {
                logs: sink.snapshot(),
                config_json: serde_json::json!({
                    "rt_tick_hz": 250,
                    "max_jitter_ns": 500_000,
                })
                .to_string(),
                system_info,
                recent_events: Vec::new(),
            }
        }

        fn to_json(&self) -> String {
            let logs_json: Vec<String> = self.logs.iter().map(|e| e.to_json().unwrap()).collect();
            serde_json::json!({
                "logs": logs_json,
                "config": self.config_json,
                "system_info": self.system_info,
                "recent_event_count": self.recent_events.len(),
            })
            .to_string()
        }

        fn redact_sensitive(&mut self, patterns: &[&str]) {
            for event in &mut self.logs {
                for pattern in patterns {
                    if event.message.contains(pattern) {
                        event.message = event.message.replace(pattern, "[REDACTED]");
                    }
                    if let Some(ref mut dev) = event.context.device_id {
                        if dev.contains(pattern) {
                            *dev = "[REDACTED]".into();
                        }
                    }
                }
            }
            for (_, v) in self.system_info.iter_mut() {
                for pattern in patterns {
                    if v.contains(pattern) {
                        *v = "[REDACTED]".into();
                    }
                }
            }
        }

        fn total_size_bytes(&self) -> usize {
            self.to_json().len()
        }
    }

    // -- Bundle collects: logs, config, system info, recent events --

    #[test]
    fn bundle_collects_logs_from_memory_sink() {
        let mut sink = MemorySink::new(100);
        for i in 0..5 {
            let ev = EventBuilder::new(EventLevel::Info, "axis", &format!("tick {i}")).build();
            sink.send(&ev).unwrap();
        }

        let counters = PerfCounters::new();
        let bundle = DiagnosticBundle::collect(&sink, &counters);

        assert_eq!(bundle.logs.len(), 5);
        assert!(!bundle.config_json.is_empty());
        assert!(bundle.system_info.contains_key("os"));
        assert!(bundle.system_info.contains_key("arch"));
    }

    #[test]
    fn bundle_collects_counter_snapshots() {
        let counters = PerfCounters::new();
        counters.record_event(&TraceEvent::tick_start(1));
        counters.record_event(&TraceEvent::tick_end(1, 4_000_000, 500));

        let sink = MemorySink::new(10);
        let bundle = DiagnosticBundle::collect(&sink, &counters);

        assert_eq!(
            bundle.system_info.get("total_ticks").unwrap(),
            "1"
        );
    }

    // -- Bundle format is valid --

    #[test]
    fn bundle_produces_valid_json() {
        let mut sink = MemorySink::new(100);
        let ev = EventBuilder::new(EventLevel::Warn, "ffb", "force clamp")
            .device_id("stick-1")
            .build();
        sink.send(&ev).unwrap();

        let counters = PerfCounters::new();
        let bundle = DiagnosticBundle::collect(&sink, &counters);
        let json = bundle.to_json();

        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("bundle JSON must be valid");
        assert!(parsed["logs"].is_array());
        assert!(parsed["system_info"].is_object());
    }

    // -- Sensitive data redaction --

    #[test]
    fn bundle_redacts_sensitive_patterns() {
        let mut sink = MemorySink::new(100);
        let ev = EventBuilder::new(EventLevel::Info, "auth", "token=secret123 loaded")
            .device_id("user-secret123")
            .build();
        sink.send(&ev).unwrap();

        let counters = PerfCounters::new();
        let mut bundle = DiagnosticBundle::collect(&sink, &counters);
        bundle.redact_sensitive(&["secret123"]);

        assert!(bundle.logs[0].message.contains("[REDACTED]"));
        assert!(!bundle.logs[0].message.contains("secret123"));
        assert_eq!(
            bundle.logs[0].context.device_id.as_deref(),
            Some("[REDACTED]")
        );
    }

    #[test]
    fn bundle_redaction_does_not_affect_non_matching_fields() {
        let mut sink = MemorySink::new(100);
        let ev = EventBuilder::new(EventLevel::Info, "axis", "normal event")
            .device_id("js-0")
            .build();
        sink.send(&ev).unwrap();

        let counters = PerfCounters::new();
        let mut bundle = DiagnosticBundle::collect(&sink, &counters);
        bundle.redact_sensitive(&["secret123"]);

        assert_eq!(bundle.logs[0].message, "normal event");
        assert_eq!(bundle.logs[0].context.device_id.as_deref(), Some("js-0"));
    }

    // -- Bundle size limits --

    #[test]
    fn bundle_size_is_bounded_by_sink_capacity() {
        let mut sink = MemorySink::new(10); // Small ring buffer
        for i in 0..100 {
            let ev = EventBuilder::new(EventLevel::Info, "test", &format!("event {i}")).build();
            sink.send(&ev).unwrap();
        }

        let counters = PerfCounters::new();
        let bundle = DiagnosticBundle::collect(&sink, &counters);

        // Ring buffer caps at 10
        assert_eq!(bundle.logs.len(), 10);
        // Size should be reasonable
        assert!(bundle.total_size_bytes() < 100_000, "bundle must be bounded");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Performance tracing
// ═══════════════════════════════════════════════════════════════════════════

mod performance_tracing {
    use super::*;

    // -- Span timing accuracy --

    #[test]
    fn span_measures_at_least_sleep_duration() {
        let collector = SpanCollector::new(1000);
        let span = collector.start_span(AXIS_TICK);
        thread::sleep(Duration::from_millis(5));
        let ns = span.finish();

        // Should be at least 4ms (conservative for CI)
        assert!(
            ns >= 4_000_000,
            "span should be at least 4ms, got {ns}ns"
        );
    }

    #[test]
    fn span_elapsed_ns_increases_over_time() {
        let span = FlightSpan::begin(HID_READ);
        let t1 = span.elapsed_ns();
        thread::sleep(Duration::from_millis(2));
        let t2 = span.elapsed_ns();
        assert!(t2 > t1, "elapsed must increase over time");
    }

    #[test]
    fn span_drop_records_into_collector() {
        let collector = SpanCollector::new(1000);
        {
            let _span = collector.start_span(BUS_PUBLISH);
            thread::sleep(Duration::from_millis(1));
        }
        let summary = spans::span_summary(&collector, BUS_PUBLISH).unwrap();
        assert_eq!(summary.count, 1);
        assert!(summary.min_ns > 0);
    }

    // -- Histogram collection for latency --

    #[test]
    fn span_collector_histogram_statistics() {
        let collector = SpanCollector::new(10_000);
        let durations = [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];

        for &d in &durations {
            collector.record(AXIS_TICK, d);
        }

        let summary = spans::span_summary(&collector, AXIS_TICK).unwrap();
        assert_eq!(summary.count, 10);
        assert_eq!(summary.min_ns, 100);
        assert_eq!(summary.max_ns, 1000);
        assert_eq!(summary.avg_ns, 550);
    }

    #[test]
    fn span_collector_p99_with_outliers() {
        let collector = SpanCollector::new(10_000);

        // 99 fast samples, 1 slow outlier
        for _ in 0..99 {
            collector.record(AXIS_TICK, 1_000);
        }
        collector.record(AXIS_TICK, 100_000);

        let summary = spans::span_summary(&collector, AXIS_TICK).unwrap();
        assert_eq!(summary.p99_ns, 100_000, "p99 should capture the outlier");
    }

    // -- Counter metrics --

    #[test]
    fn perf_counters_track_tick_count() {
        let counters = PerfCounters::new();
        for i in 0..100 {
            counters.record_event(&TraceEvent::tick_start(i));
        }
        let snap = counters.snapshot();
        assert_eq!(snap.total_ticks, 100);
    }

    #[test]
    fn perf_counters_track_deadline_misses() {
        let counters = PerfCounters::new();
        counters.record_event(&TraceEvent::tick_start(1)); // to avoid div-by-zero
        for i in 0..10 {
            counters.record_event(&TraceEvent::deadline_miss(i, 1_000_000));
        }
        let snap = counters.snapshot();
        assert_eq!(snap.deadline_misses, 10);
        assert!(snap.miss_rate > 0.0);
    }

    #[test]
    fn perf_counters_miss_rate_calculation() {
        let counters = PerfCounters::new();
        for i in 0..100 {
            counters.record_event(&TraceEvent::tick_start(i));
        }
        for i in 0..5 {
            counters.record_event(&TraceEvent::deadline_miss(i, 1_000_000));
        }
        let snap = counters.snapshot();
        assert!(
            (snap.miss_rate - 0.05).abs() < 0.001,
            "miss rate should be ~5%, got {}",
            snap.miss_rate
        );
    }

    // -- Gauge metrics (current values) --

    #[test]
    fn counter_snapshot_session_duration_increases() {
        let counters = PerfCounters::new();
        let snap1 = counters.snapshot();
        thread::sleep(Duration::from_millis(10));
        let snap2 = counters.snapshot();
        assert!(
            snap2.session_duration_ms >= snap1.session_duration_ms,
            "session duration must be non-decreasing"
        );
    }

    #[test]
    fn counter_snapshot_timestamp_is_current() {
        let counters = PerfCounters::new();
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let snap = counters.snapshot();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        assert!(snap.timestamp_ns >= before && snap.timestamp_ns <= after);
    }

    #[test]
    fn multiple_span_operations_tracked_independently() {
        let collector = SpanCollector::new(1000);
        collector.record(AXIS_TICK, 100);
        collector.record(AXIS_TICK, 200);
        collector.record(HID_READ, 500);
        collector.record(FFB_COMPUTE, 1000);
        collector.record(FFB_COMPUTE, 2000);
        collector.record(FFB_COMPUTE, 3000);

        let axis = spans::span_summary(&collector, AXIS_TICK).unwrap();
        let hid = spans::span_summary(&collector, HID_READ).unwrap();
        let ffb = spans::span_summary(&collector, FFB_COMPUTE).unwrap();

        assert_eq!(axis.count, 2);
        assert_eq!(hid.count, 1);
        assert_eq!(ffb.count, 3);
        assert_eq!(ffb.avg_ns, 2000);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Event correlation
// ═══════════════════════════════════════════════════════════════════════════

mod event_correlation {
    use super::*;

    fn make_event_at(component: &str, message: &str, timestamp_ns: u64) -> FlightEvent {
        FlightEvent {
            timestamp_ns,
            level: EventLevel::Info,
            component: component.to_owned(),
            message: message.to_owned(),
            context: EventContext::default(),
        }
    }

    // -- Events from same request share correlation ID --

    #[test]
    fn correlated_events_share_id() {
        let collector = ChainCollector::new(100);
        let id = CorrelationId::new();

        let ev1 = EventBuilder::new(EventLevel::Info, "hid", "input received").build();
        let ev2 = EventBuilder::new(EventLevel::Info, "axis", "processed").build();
        let ev3 = EventBuilder::new(EventLevel::Info, "sim", "output sent").build();

        collector.record(CorrelatedEvent::new(id, ev1));
        collector.record(CorrelatedEvent::new(id, ev2));
        collector.record(CorrelatedEvent::new(id, ev3));

        let chain = collector.get_chain(&id).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.correlation_id, id);
    }

    #[test]
    fn distinct_requests_get_distinct_correlation_ids() {
        let ids: Vec<CorrelationId> = (0..100).map(|_| CorrelationId::new()).collect();
        let unique: HashSet<u64> = ids.iter().map(|id| id.as_raw()).collect();
        assert_eq!(unique.len(), 100, "all correlation IDs must be unique");
    }

    // -- Cross-component tracing --

    #[test]
    fn cross_component_pipeline_trace() {
        let collector = ChainCollector::new(100);
        let id = CorrelationId::new();

        let components = ["hid", "axis", "bus", "ffb", "simconnect"];
        for (i, comp) in components.iter().enumerate() {
            let ev = make_event_at(comp, &format!("{comp} step"), (i as u64 + 1) * 1_000_000);
            collector.record(CorrelatedEvent::new(id, ev));
        }

        let chain = collector.get_chain(&id).unwrap();
        assert_eq!(chain.len(), 5);

        // Verify component ordering
        for (i, comp) in components.iter().enumerate() {
            assert_eq!(chain.events()[i].component, *comp);
        }
    }

    #[test]
    fn multiple_concurrent_chains_are_independent() {
        let collector = ChainCollector::new(100);
        let id_a = CorrelationId::new();
        let id_b = CorrelationId::new();

        // Interleave events from two chains
        collector.record(CorrelatedEvent::new(
            id_a,
            make_event_at("hid", "input A", 1_000_000),
        ));
        collector.record(CorrelatedEvent::new(
            id_b,
            make_event_at("hid", "input B", 1_100_000),
        ));
        collector.record(CorrelatedEvent::new(
            id_a,
            make_event_at("axis", "process A", 1_200_000),
        ));
        collector.record(CorrelatedEvent::new(
            id_b,
            make_event_at("axis", "process B", 1_300_000),
        ));

        let chain_a = collector.get_chain(&id_a).unwrap();
        let chain_b = collector.get_chain(&id_b).unwrap();

        assert_eq!(chain_a.len(), 2);
        assert_eq!(chain_b.len(), 2);
        assert!(chain_a.events().iter().all(|e| e.message.contains("A")));
        assert!(chain_b.events().iter().all(|e| e.message.contains("B")));
    }

    // -- Timeline reconstruction from events --

    #[test]
    fn timeline_reconstruction_with_duration() {
        let collector = ChainCollector::new(100);
        let id = CorrelationId::new();

        // Simulate a 2ms pipeline
        collector.record(CorrelatedEvent::new(
            id,
            make_event_at("hid", "read", 1_000_000),
        ));
        collector.record(CorrelatedEvent::new(
            id,
            make_event_at("axis", "process", 1_500_000),
        ));
        collector.record(CorrelatedEvent::new(
            id,
            make_event_at("sim", "output", 3_000_000),
        ));

        let chain = collector.take_chain(&id).unwrap();

        // Total pipeline latency
        let duration = chain.duration_ns().unwrap();
        assert_eq!(duration, 2_000_000, "pipeline should be 2ms");

        // Per-stage latencies
        let events = chain.events();
        let hid_to_axis = events[1].timestamp_ns - events[0].timestamp_ns;
        let axis_to_sim = events[2].timestamp_ns - events[1].timestamp_ns;
        assert_eq!(hid_to_axis, 500_000);
        assert_eq!(axis_to_sim, 1_500_000);
    }

    #[test]
    fn chain_serialization_preserves_event_order() {
        let id = CorrelationId::from_raw(42);
        let mut chain = EventChain::new(id);
        for i in 0..5 {
            chain.push(make_event_at("test", &format!("step-{i}"), i * 1_000));
        }

        let json = serde_json::to_string(&chain).unwrap();
        let recovered: EventChain = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered.len(), 5);
        for i in 0..5 {
            assert_eq!(recovered.events()[i].message, format!("step-{i}"));
            assert_eq!(recovered.events()[i].timestamp_ns, i as u64 * 1_000);
        }
    }

    #[test]
    fn collector_eviction_removes_oldest_chain() {
        let collector = ChainCollector::new(2);
        let id1 = CorrelationId::from_raw(100);
        let id2 = CorrelationId::from_raw(200);
        let id3 = CorrelationId::from_raw(300);

        collector.record(CorrelatedEvent::new(
            id1,
            make_event_at("a", "1", 1_000),
        ));
        collector.record(CorrelatedEvent::new(
            id2,
            make_event_at("b", "2", 2_000),
        ));
        // Adding id3 should evict id1 (smallest raw)
        collector.record(CorrelatedEvent::new(
            id3,
            make_event_at("c", "3", 3_000),
        ));

        assert!(collector.get_chain(&id1).is_none(), "id1 should be evicted");
        assert!(collector.get_chain(&id2).is_some());
        assert!(collector.get_chain(&id3).is_some());
        assert_eq!(collector.active_chains(), 2);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Property tests
// ═══════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;

    // -- All log events round-trip through serialization --

    proptest! {
        #[test]
        fn flight_event_round_trips_through_json(
            level_idx in 0u8..5,
            component in "[a-z]{1,16}",
            message in "[a-zA-Z0-9 _-]{0,64}",
        ) {
            let level = match level_idx {
                0 => EventLevel::Trace,
                1 => EventLevel::Debug,
                2 => EventLevel::Info,
                3 => EventLevel::Warn,
                _ => EventLevel::Error,
            };

            let ev = EventBuilder::new(level, &component, &message).build();
            let json = ev.to_json().unwrap();
            let recovered: FlightEvent = serde_json::from_str(&json).unwrap();

            prop_assert_eq!(recovered.level, ev.level);
            prop_assert_eq!(&recovered.component, &ev.component);
            prop_assert_eq!(&recovered.message, &ev.message);
            prop_assert_eq!(recovered.timestamp_ns, ev.timestamp_ns);
        }

        #[test]
        fn trace_event_all_variants_round_trip(
            tick in 0u64..1_000_000,
            duration in 0u64..10_000_000,
            jitter in -1_000_000i64..1_000_000,
            device_id in 0u32..0xFFFF,
            bytes in 0usize..1024,
        ) {
            // TickStart
            let ev = TraceEvent::tick_start(tick);
            let data = ev.to_json_bytes().unwrap();
            let recovered: TraceEvent = serde_json::from_slice(&data).unwrap();
            prop_assert_eq!(recovered.timestamp_ns, ev.timestamp_ns);

            // TickEnd
            let ev = TraceEvent::tick_end(tick, duration, jitter);
            let data = ev.to_json_bytes().unwrap();
            let recovered: TraceEvent = serde_json::from_slice(&data).unwrap();
            match recovered.data {
                EventData::TickEnd { tick_number, duration_ns, jitter_ns } => {
                    prop_assert_eq!(tick_number, tick);
                    prop_assert_eq!(duration_ns, duration);
                    prop_assert_eq!(jitter_ns, jitter);
                }
                _ => prop_assert!(false, "wrong variant"),
            }

            // HidWrite
            let ev = TraceEvent::hid_write(device_id, bytes, duration);
            let data = ev.to_json_bytes().unwrap();
            let recovered: TraceEvent = serde_json::from_slice(&data).unwrap();
            match recovered.data {
                EventData::HidWrite { device_id: d, bytes: b, duration_ns } => {
                    prop_assert_eq!(d, device_id);
                    prop_assert_eq!(b, bytes);
                    prop_assert_eq!(duration_ns, duration);
                }
                _ => prop_assert!(false, "wrong variant"),
            }
        }

        // -- Filtering is consistent (same filter → same output) --

        #[test]
        fn event_filter_is_deterministic(
            tick_enabled in proptest::bool::ANY,
            hid_enabled in proptest::bool::ANY,
            deadline_enabled in proptest::bool::ANY,
            writer_enabled in proptest::bool::ANY,
            custom_enabled in proptest::bool::ANY,
        ) {
            let filter = EventFilter {
                tick_events: tick_enabled,
                hid_events: hid_enabled,
                deadline_events: deadline_enabled,
                writer_events: writer_enabled,
                custom_events: custom_enabled,
            };

            let events = vec![
                TraceEvent::tick_start(1),
                TraceEvent::tick_end(1, 1_000, 0),
                TraceEvent::hid_write(0, 64, 100),
                TraceEvent::deadline_miss(1, 500),
                TraceEvent::writer_drop("s", 1),
                TraceEvent::custom("x", serde_json::json!(null)),
            ];

            // Two passes with the same filter must produce identical results
            let pass1: Vec<bool> = events.iter().map(|e| filter.should_trace(e)).collect();
            let pass2: Vec<bool> = events.iter().map(|e| filter.should_trace(e)).collect();
            prop_assert_eq!(&pass1, &pass2);
        }

        // -- Trace export is deterministic for same input --

        #[test]
        fn json_formatter_is_deterministic(
            component in "[a-z]{1,8}",
            message in "[a-zA-Z0-9 ]{0,32}",
            field_val in -1000i64..1000,
        ) {
            let entry = LogEntryBuilder::new(LogLevel::Info, &component, &message)
                .field("val", LogValue::Int(field_val))
                .build();

            let json1 = JsonLogFormatter::format(&entry);
            let json2 = JsonLogFormatter::format(&entry);
            prop_assert_eq!(&json1, &json2);
        }

        // -- CorrelationId uniqueness --

        #[test]
        fn correlation_ids_never_collide(_seed in 0u32..1000) {
            let a = CorrelationId::new();
            let b = CorrelationId::new();
            prop_assert_ne!(a.as_raw(), b.as_raw());
        }

        // -- EventChain serialization round-trip --

        #[test]
        fn event_chain_round_trips(
            num_events in 0usize..20,
            raw_id in 1u64..100_000,
        ) {
            let id = CorrelationId::from_raw(raw_id);
            let mut chain = EventChain::new(id);
            for i in 0..num_events {
                chain.push(FlightEvent {
                    timestamp_ns: i as u64 * 1_000,
                    level: EventLevel::Info,
                    component: "test".into(),
                    message: format!("event-{i}"),
                    context: EventContext::default(),
                });
            }

            let json = serde_json::to_string(&chain).unwrap();
            let recovered: EventChain = serde_json::from_str(&json).unwrap();

            prop_assert_eq!(recovered.len(), chain.len());
            prop_assert_eq!(recovered.correlation_id, chain.correlation_id);
        }

        // -- CounterSnapshot JSON round-trip --

        #[test]
        fn counter_snapshot_json_round_trip(
            ticks in 0u64..100_000,
            misses in 0u64..1000,
            _hid_writes in 0u64..10_000,
            _drops in 0u64..500,
        ) {
            let counters = PerfCounters::new();
            for i in 0..ticks.min(100) {
                counters.record_event(&TraceEvent::tick_start(i));
            }
            for i in 0..misses.min(10) {
                counters.record_event(&TraceEvent::deadline_miss(i, 1_000_000));
            }

            let snap = counters.snapshot();
            let json = snap.to_json().unwrap();
            let recovered: flight_tracing::CounterSnapshot =
                serde_json::from_str(&json).unwrap();

            prop_assert_eq!(recovered.total_ticks, snap.total_ticks);
            prop_assert_eq!(recovered.deadline_misses, snap.deadline_misses);
        }
    }
}
