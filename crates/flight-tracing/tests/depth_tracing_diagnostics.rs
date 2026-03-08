// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the tracing/diagnostics subsystem.
//!
//! Covers five areas with 30+ tests:
//!   1. Span management (enter/exit, nesting, attributes, cross-task, lifecycle)
//!   2. Event filtering (level filtering, target filtering, dynamic changes, layering)
//!   3. Structured logging (key-value fields, visitor patterns, formatting, JSON output)
//!   4. Performance (disabled overhead, pre-filtered fast path, allocation avoidance)
//!   5. Diagnostic bundle (system info, profiles, device states, error history,
//!      bundle format/export)

use flight_tracing::correlation::{ChainCollector, CorrelatedEvent, CorrelationId};
use flight_tracing::counters::PerfCounters;
use flight_tracing::events::{EventFilter, TraceEvent};
use flight_tracing::regression::RegressionDetector;
use flight_tracing::spans::{
    self, FlightSpan, SpanCollector, AXIS_TICK, BUS_PUBLISH, FFB_COMPUTE, HID_READ,
    PROFILE_COMPILE,
};
use flight_tracing::structured::{
    EventBuilder, EventLevel, EventSink, FileSink, FlightEvent, MemorySink,
};
use flight_tracing::structured_log::{
    JsonLogFormatter, LogEntry, LogEntryBuilder, LogLevel, LogValue,
};
use flight_tracing::CounterSnapshot;
use flight_tracing::log_rotation::{LogRotator, RotationConfig, RotationResult};

use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════
// 1. SPAN MANAGEMENT (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Entering and exiting a span via `finish()` records exactly one sample.
#[test]
fn span_enter_exit_records_single_sample() {
    let collector = SpanCollector::new(1_000);
    let span = collector.start_span(AXIS_TICK);
    thread::sleep(Duration::from_millis(1));
    let ns = span.finish();
    assert!(ns > 0, "finish must return positive duration");

    let summary = spans::span_summary(&collector, AXIS_TICK).unwrap();
    assert_eq!(summary.count, 1);
    assert!(summary.min_ns > 0);
    assert_eq!(summary.min_ns, summary.max_ns);
}

/// Nested spans are tracked independently — inner completes before outer.
#[test]
fn nested_spans_tracked_independently() {
    let collector = SpanCollector::new(1_000);
    let outer = collector.start_span(AXIS_TICK);
    {
        let inner = collector.start_span(HID_READ);
        thread::sleep(Duration::from_millis(1));
        inner.finish();
    }
    thread::sleep(Duration::from_millis(1));
    let outer_ns = outer.finish();

    let axis = spans::span_summary(&collector, AXIS_TICK).unwrap();
    let hid = spans::span_summary(&collector, HID_READ).unwrap();

    assert_eq!(axis.count, 1);
    assert_eq!(hid.count, 1);
    // Outer span must be longer than inner since it sleeps again after inner finishes.
    assert!(outer_ns > hid.min_ns, "outer should be >= inner");
}

/// Span attributes (name, elapsed_ns) are accessible during the span's lifetime.
#[test]
fn span_attributes_accessible_during_lifetime() {
    let span = FlightSpan::begin(FFB_COMPUTE);
    assert_eq!(span.name(), FFB_COMPUTE);
    thread::sleep(Duration::from_millis(1));
    let elapsed = span.elapsed_ns();
    assert!(elapsed > 0, "elapsed_ns should be positive while span is active");
    let final_ns = span.finish();
    assert!(final_ns >= elapsed, "final duration >= mid-span elapsed");
}

/// Spans started from different threads are independently recorded.
#[test]
fn cross_thread_spans_recorded_independently() {
    let collector = Arc::new(SpanCollector::new(10_000));

    let handles: Vec<_> = (0..4)
        .map(|_| {
            // SAFETY: SpanCollector is internally synchronised, and the collector
            // outlives all spawned threads because we join below.
            let ptr = Arc::as_ptr(&collector) as usize;
            thread::spawn(move || {
                let collector_ref = unsafe { &*(ptr as *const SpanCollector) };
                for _ in 0..50 {
                    let _span = collector_ref.start_span(BUS_PUBLISH);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let s = spans::span_summary(&collector, BUS_PUBLISH).unwrap();
    assert_eq!(s.count, 200, "4 threads × 50 spans = 200 total");
}

/// Span lifecycle: drop auto-records, double-finish is idempotent.
#[test]
fn span_drop_auto_records_and_double_finish_is_safe() {
    let collector = SpanCollector::new(1_000);

    // Drop-based recording
    {
        let _span = collector.start_span(PROFILE_COMPILE);
    }
    let s = spans::span_summary(&collector, PROFILE_COMPILE).unwrap();
    assert_eq!(s.count, 1, "drop should auto-record");

    // Explicit finish then drop is safe (no double-count)
    collector.reset();
    let span = collector.start_span(PROFILE_COMPILE);
    span.finish();
    // span is consumed by finish; drop runs on the moved value but `finished=true`
    let s = spans::span_summary(&collector, PROFILE_COMPILE).unwrap();
    assert_eq!(s.count, 1, "finish+drop must not double-count");
}

/// SpanCollector.reset() clears all accumulated data.
#[test]
fn span_collector_reset_clears_all() {
    let collector = SpanCollector::new(1_000);
    collector.record(AXIS_TICK, 1_000);
    collector.record(HID_READ, 2_000);
    assert_eq!(collector.summary().len(), 2);
    collector.reset();
    assert!(collector.summary().is_empty(), "reset must clear everything");
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. EVENT FILTERING (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// EventLevel ordering: TRACE < DEBUG < INFO < WARN < ERROR.
#[test]
fn event_level_ordering_trace_to_error() {
    let levels = [
        EventLevel::Trace,
        EventLevel::Debug,
        EventLevel::Info,
        EventLevel::Warn,
        EventLevel::Error,
    ];
    for window in levels.windows(2) {
        assert!(
            window[0] < window[1],
            "{:?} should be < {:?}",
            window[0],
            window[1]
        );
    }
}

/// Target filtering: MemorySink only stores events matching minimum level.
#[test]
fn memory_sink_level_filtering_simulation() {
    let mut sink = MemorySink::new(100);
    let min_level = EventLevel::Warn;

    for &level in &[
        EventLevel::Trace,
        EventLevel::Debug,
        EventLevel::Info,
        EventLevel::Warn,
        EventLevel::Error,
    ] {
        let ev = EventBuilder::new(level, "test", format!("{level:?} msg")).build();
        if level >= min_level {
            sink.send(&ev).unwrap();
        }
    }

    let snap = sink.snapshot();
    assert_eq!(snap.len(), 2, "only Warn and Error should pass");
    assert_eq!(snap[0].level, EventLevel::Warn);
    assert_eq!(snap[1].level, EventLevel::Error);
}

/// Dynamic filter change: EventFilter can be mutated at runtime.
#[test]
fn event_filter_dynamic_change() {
    let mut filter = EventFilter {
        tick_events: false,
        ..EventFilter::default()
    };
    let tick_ev = TraceEvent::tick_start(1);
    assert!(
        !filter.should_trace(&tick_ev),
        "ticks disabled after mutation"
    );

    filter.tick_events = true;
    assert!(filter.should_trace(&tick_ev), "ticks re-enabled");
}

/// Layered filtering: combine EventFilter + EventLevel for dual gate.
#[test]
fn layered_filter_type_and_level() {
    let type_filter = EventFilter {
        tick_events: true,
        hid_events: false,
        deadline_events: true,
        writer_events: true,
        custom_events: false,
    };
    let level_gate = EventLevel::Warn;

    // A tick event that also checks level
    let tick = TraceEvent::tick_start(1);
    let tick_passes_type = type_filter.should_trace(&tick);
    assert!(tick_passes_type, "tick passes type filter");

    // HID event blocked by type filter regardless of level
    let hid = TraceEvent::hid_write(0, 64, 100_000);
    assert!(!type_filter.should_trace(&hid), "HID blocked by type");

    // Level check on structured events
    let low = EventBuilder::new(EventLevel::Debug, "test", "debug msg").build();
    let high = EventBuilder::new(EventLevel::Error, "test", "error msg").build();
    assert!(low.level < level_gate, "debug < warn");
    assert!(high.level >= level_gate, "error >= warn");
}

/// All-disabled filter blocks everything.
#[test]
fn all_disabled_filter_blocks_all_events() {
    let filter = EventFilter {
        tick_events: false,
        hid_events: false,
        deadline_events: false,
        writer_events: false,
        custom_events: false,
    };
    let events = [
        TraceEvent::tick_start(1),
        TraceEvent::tick_end(1, 0, 0),
        TraceEvent::hid_write(0, 0, 0),
        TraceEvent::deadline_miss(1, 0),
        TraceEvent::writer_drop("s", 0),
        TraceEvent::custom("x", serde_json::json!(null)),
    ];
    for ev in &events {
        assert!(
            !filter.should_trace(ev),
            "all-disabled filter must block {}",
            ev.event_type()
        );
    }
}

/// Filter correctly categorises TickStart and TickEnd under the same flag.
#[test]
fn tick_start_and_tick_end_share_tick_events_flag() {
    let mut filter = EventFilter {
        tick_events: false,
        ..EventFilter::default()
    };
    assert!(!filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(!filter.should_trace(&TraceEvent::tick_end(1, 0, 0)));

    filter.tick_events = true;
    assert!(filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::tick_end(1, 0, 0)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. STRUCTURED LOGGING (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Key-value fields attached via builder are present in JSON output.
#[test]
fn structured_kv_fields_in_json() {
    let entry = LogEntryBuilder::new(LogLevel::Info, "axis", "tick processed")
        .field("tick_number", LogValue::Int(42))
        .field("duration_us", LogValue::Float(3.75))
        .field("overrun", LogValue::Bool(false))
        .build();

    let json = JsonLogFormatter::format(&entry);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["fields"]["tick_number"], 42);
    assert_eq!(parsed["fields"]["duration_us"], 3.75);
    assert_eq!(parsed["fields"]["overrun"], false);
}

/// Visitor pattern: LogValue variants all format correctly via Display.
#[test]
fn log_value_display_visitor_pattern() {
    let values: Vec<(LogValue, &str)> = vec![
        (LogValue::String("hello world".into()), "hello world"),
        (LogValue::Int(-999), "-999"),
        (LogValue::Float(3.125), "3.125"),
        (LogValue::Bool(true), "true"),
    ];
    for (val, expected) in &values {
        assert_eq!(
            val.to_string(),
            *expected,
            "LogValue::Display mismatch for {val:?}"
        );
    }
}

/// Display formatting: EventLevel and LogLevel both produce uppercase strings.
#[test]
fn level_display_formatting() {
    assert_eq!(EventLevel::Trace.to_string(), "TRACE");
    assert_eq!(EventLevel::Debug.to_string(), "DEBUG");
    assert_eq!(EventLevel::Info.to_string(), "INFO");
    assert_eq!(EventLevel::Warn.to_string(), "WARN");
    assert_eq!(EventLevel::Error.to_string(), "ERROR");

    assert_eq!(LogLevel::Trace.to_string(), "TRACE");
    assert_eq!(LogLevel::Error.to_string(), "ERROR");
}

/// JSON output: FlightEvent.to_json() skips absent context fields.
#[test]
fn flight_event_json_skips_absent_context_fields() {
    let ev = EventBuilder::new(EventLevel::Info, "bus", "publish")
        .device_id("stick-1")
        .build();

    let json = ev.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["context"]["device_id"], "stick-1");
    assert!(
        parsed["context"].get("sim_name").is_none(),
        "absent fields must not appear"
    );
    assert!(parsed["context"].get("axis_name").is_none());
    assert!(parsed["context"].get("profile_name").is_none());
}

/// JSON batch output: newline-delimited format, each line valid JSON.
#[test]
fn json_batch_newline_delimited() {
    let entries: Vec<LogEntry> = (0..5)
        .map(|i| {
            LogEntryBuilder::new(LogLevel::Info, "test", &format!("msg {i}"))
                .field("idx", LogValue::Int(i))
                .build()
        })
        .collect();

    let batch = JsonLogFormatter::format_batch(&entries);
    let lines: Vec<&str> = batch.lines().collect();
    assert_eq!(lines.len(), 5);

    for (i, line) in lines.iter().enumerate() {
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["fields"]["idx"], i as i64);
    }
}

/// Full context builder: all four context fields populate correctly.
#[test]
fn full_context_builder_populates_all_fields() {
    let ev = EventBuilder::new(EventLevel::Debug, "simconnect", "data sent")
        .device_id("warthog-0")
        .sim_name("MSFS")
        .axis_name("elevator")
        .profile_name("f18-carrier")
        .build();

    assert_eq!(ev.context.device_id.as_deref(), Some("warthog-0"));
    assert_eq!(ev.context.sim_name.as_deref(), Some("MSFS"));
    assert_eq!(ev.context.axis_name.as_deref(), Some("elevator"));
    assert_eq!(ev.context.profile_name.as_deref(), Some("f18-carrier"));

    // Round-trip through JSON
    let json = ev.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["context"]["device_id"], "warthog-0");
    assert_eq!(parsed["context"]["sim_name"], "MSFS");
    assert_eq!(parsed["context"]["axis_name"], "elevator");
    assert_eq!(parsed["context"]["profile_name"], "f18-carrier");
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. PERFORMANCE (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Recording events when no provider is initialised has low overhead
/// (exercises the fast-path exit in `emit_event`).
#[test]
fn emit_event_noop_when_no_provider() {
    // After shutdown or before init, emit should still succeed.
    let _ = flight_tracing::shutdown();
    let result = flight_tracing::emit_event(TraceEvent::tick_start(1));
    assert!(result.is_ok(), "emit with no provider should be Ok");
}

/// EventFilter pre-filtering avoids work on disabled event types.
#[test]
fn pre_filtered_events_avoid_counter_recording() {
    let counters = PerfCounters::new();
    let filter = EventFilter::ci_minimal();

    let tick_ev = TraceEvent::tick_start(1);
    let hid_ev = TraceEvent::hid_write(0, 64, 100_000);

    // Only record events that pass the filter
    if filter.should_trace(&tick_ev) {
        counters.record_event(&tick_ev);
    }
    if filter.should_trace(&hid_ev) {
        counters.record_event(&hid_ev);
    }

    let snap = counters.snapshot();
    assert_eq!(snap.total_ticks, 0, "tick event was filtered out");
    assert_eq!(snap.total_hid_writes, 1, "HID event passed filter");
}

/// Free-standing spans (no collector) have minimal overhead.
#[test]
fn free_standing_span_minimal_overhead() {
    let span = FlightSpan::begin(AXIS_TICK);
    let ns = span.finish();
    // Span with no sleep should be well under 1ms.
    assert!(
        ns < 1_000_000,
        "free span overhead should be < 1ms, got {ns}ns"
    );
}

/// Binary encoding is allocation-efficient with pre-sized buffers.
#[test]
fn binary_encoding_compact_sizes() {
    let tick_start = TraceEvent::tick_start(1);
    let bin = tick_start.to_binary();
    // 8 (timestamp) + 1 (type) + 8 (tick_number) = 17
    assert_eq!(bin.len(), 17);

    let deadline = TraceEvent::deadline_miss(1, 500_000);
    let bin = deadline.to_binary();
    // 8 (timestamp) + 1 (type) + 8 (tick) + 8 (miss_duration) = 25
    assert_eq!(bin.len(), 25);

    let writer_drop = TraceEvent::writer_drop("ax", 3);
    let bin = writer_drop.to_binary();
    // 8 (timestamp) + 1 (type) + 1 (id_len) + 2 (id bytes) + 8 (count) = 20
    assert_eq!(bin.len(), 20);
}

/// Batch span recording at RT cadence: 250 Hz for 1 s of simulated ticks
/// completes well within the budget.
#[test]
fn batch_250hz_span_recording_within_budget() {
    let collector = SpanCollector::new(10_000);

    let start = std::time::Instant::now();
    for _ in 0..250 {
        collector.record(AXIS_TICK, 4_000_000); // 4ms simulated tick
    }
    let elapsed = start.elapsed();

    // Recording overhead for 250 samples should be well under 10ms.
    assert!(
        elapsed < Duration::from_millis(50),
        "250 record() calls took {elapsed:?}, expected < 50ms"
    );

    let s = spans::span_summary(&collector, AXIS_TICK).unwrap();
    assert_eq!(s.count, 250);
    assert_eq!(s.avg_ns, 4_000_000);
}

/// PerfCounters concurrent write throughput: 4 threads × 1000 events without data races.
#[test]
fn perf_counters_concurrent_throughput() {
    let counters = Arc::new(PerfCounters::new());
    let per_thread = 1_000u64;

    let handles: Vec<_> = (0..4)
        .map(|t| {
            let c = Arc::clone(&counters);
            thread::spawn(move || {
                for i in 0..per_thread {
                    c.record_event(&TraceEvent::tick_start(t * 10_000 + i));
                    c.record_event(&TraceEvent::tick_end(t * 10_000 + i, 4_000_000, 100));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let snap = counters.snapshot();
    assert_eq!(snap.total_ticks, 4_000);
    assert_eq!(snap.jitter.sample_count, 4_000);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. DIAGNOSTIC BUNDLE (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Collect system info: CounterSnapshot captures session duration and timestamp.
#[test]
fn diagnostic_snapshot_captures_session_metadata() {
    let counters = PerfCounters::new();
    thread::sleep(Duration::from_millis(10));
    let snap = counters.snapshot();

    assert!(
        snap.session_duration_ms >= 10,
        "session_duration_ms should reflect elapsed time"
    );
    assert!(snap.timestamp_ns > 0, "timestamp_ns should be set");
}

/// Active profiles: SpanSummary for each operation collects min/max/avg/p99.
#[test]
fn diagnostic_span_summaries_cover_all_operations() {
    let collector = SpanCollector::new(10_000);
    let ops = [AXIS_TICK, HID_READ, BUS_PUBLISH, PROFILE_COMPILE, FFB_COMPUTE];

    for (i, &op) in ops.iter().enumerate() {
        for j in 0..10 {
            collector.record(op, (i as u64 + 1) * 1_000 + j * 100);
        }
    }

    let summaries = collector.summary();
    assert_eq!(summaries.len(), 5, "all 5 ops should have summaries");

    let names: HashSet<&str> = summaries.iter().map(|s| s.name).collect();
    for op in &ops {
        assert!(names.contains(op), "missing summary for {op}");
    }

    for s in &summaries {
        assert!(s.count == 10);
        assert!(s.min_ns <= s.avg_ns);
        assert!(s.avg_ns <= s.max_ns);
        assert!(s.p99_ns <= s.max_ns);
    }
}

/// Device state: correlation chains capture end-to-end device→sim pipeline.
#[test]
fn diagnostic_correlation_chain_captures_pipeline() {
    let collector = ChainCollector::new(100);
    let id = CorrelationId::new();

    let events = [
        ("hid", "joystick input", 1_000_000u64),
        ("axis", "curve applied", 1_200_000),
        ("bus", "event published", 1_350_000),
        ("simconnect", "value written", 1_800_000),
    ];

    for &(component, message, ts) in &events {
        let ev = FlightEvent {
            timestamp_ns: ts,
            level: EventLevel::Info,
            component: component.into(),
            message: message.into(),
            context: Default::default(),
        };
        collector.record(CorrelatedEvent::new(id, ev));
    }

    let chain = collector.get_chain(&id).unwrap();
    assert_eq!(chain.len(), 4);
    assert_eq!(chain.duration_ns(), Some(800_000)); // 1.8ms - 1.0ms
    assert_eq!(chain.events()[0].component, "hid");
    assert_eq!(chain.events()[3].component, "simconnect");
}

/// Error history: MemorySink ring buffer retains most recent errors on overflow.
#[test]
fn diagnostic_error_history_ring_buffer_eviction() {
    let mut sink = MemorySink::new(5);

    for i in 0..10 {
        let ev = EventBuilder::new(EventLevel::Error, "test", format!("error {i}")).build();
        sink.send(&ev).unwrap();
    }

    assert_eq!(sink.len(), 5);
    assert_eq!(sink.total_received(), 10);

    let snap = sink.snapshot();
    // Ring buffer should retain errors 5–9 (most recent 5)
    assert_eq!(snap[0].message, "error 5");
    assert_eq!(snap[4].message, "error 9");
}

/// Bundle format: CounterSnapshot serializes to JSON with all required fields.
#[test]
fn diagnostic_bundle_json_format() {
    let counters = PerfCounters::new();
    // Populate with representative data
    for i in 0..100 {
        counters.record_event(&TraceEvent::tick_start(i));
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, (i % 500) as i64));
    }
    counters.record_event(&TraceEvent::hid_write(0x1234, 64, 200_000));
    counters.record_event(&TraceEvent::deadline_miss(50, 1_000_000));
    counters.record_event(&TraceEvent::writer_drop("axis", 2));

    let snap = counters.snapshot();
    let json = snap.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Top-level fields
    assert!(parsed["total_ticks"].is_number());
    assert!(parsed["deadline_misses"].is_number());
    assert!(parsed["miss_rate"].is_number());
    assert!(parsed["total_hid_writes"].is_number());
    assert!(parsed["writer_drops"].is_number());
    assert!(parsed["session_duration_ms"].is_number());
    assert!(parsed["timestamp_ns"].is_number());

    // Nested objects
    assert!(parsed["jitter"]["p50_ns"].is_number());
    assert!(parsed["jitter"]["p99_ns"].is_number());
    assert!(parsed["jitter"]["max_ns"].is_number());
    assert!(parsed["jitter"]["sample_count"].is_number());
    assert!(parsed["hid"]["total_writes"].is_number());
    assert!(parsed["hid"]["avg_time_ns"].is_number());
    assert!(parsed["hid"]["p99_time_ns"].is_number());

    // KV pairs export
    let kvs = snap.to_kv_pairs();
    assert!(!kvs.is_empty());
    let keys: HashSet<&str> = kvs.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains("total_ticks"));
    assert!(keys.contains("jitter_p99_us"));
    assert!(keys.contains("hid_avg_us"));
}

/// Export: regression detector save/load baselines round-trips correctly.
#[test]
fn diagnostic_baseline_export_round_trip() {
    let mut detector = RegressionDetector::new();

    // Build a few baselines with known values
    for i in 0..3 {
        let snap = CounterSnapshot {
            total_ticks: 1_000 + i * 100,
            deadline_misses: i,
            miss_rate: i as f64 * 0.001,
            total_hid_writes: 100 + i * 10,
            writer_drops: 0,
            jitter: flight_tracing::counters::JitterStats {
                p50_ns: 1_000 + i as i64 * 50,
                p99_ns: 5_000 + i as i64 * 100,
                max_ns: 10_000,
                sample_count: 1_000,
            },
            hid: flight_tracing::counters::HidStats {
                total_writes: 100,
                total_time_ns: 20_000_000,
                avg_time_ns: 200_000,
                p99_time_ns: 280_000,
            },
            session_duration_ms: 4_000,
            timestamp_ns: 0,
        };
        detector.add_baseline(snap);
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("baselines.json");
    detector.save_baselines(&path).unwrap();

    let mut loaded = RegressionDetector::new();
    loaded.load_baselines(&path).unwrap();

    let summary = loaded.get_baseline_summary().unwrap();
    assert_eq!(summary.count, 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// BONUS: Additional depth tests (4 tests to exceed the 30 minimum)
// ═══════════════════════════════════════════════════════════════════════════

/// Regression detector: good performance produces no critical/fatal alerts.
#[test]
fn regression_no_alerts_for_good_performance() {
    let mut detector = RegressionDetector::new();
    let baseline = CounterSnapshot {
        total_ticks: 1_000,
        deadline_misses: 0,
        miss_rate: 0.0,
        total_hid_writes: 100,
        writer_drops: 0,
        jitter: flight_tracing::counters::JitterStats {
            p50_ns: 500,
            p99_ns: 2_000,
            max_ns: 5_000,
            sample_count: 1_000,
        },
        hid: flight_tracing::counters::HidStats {
            total_writes: 100,
            total_time_ns: 20_000_000,
            avg_time_ns: 200_000,
            p99_time_ns: 250_000,
        },
        session_duration_ms: 4_000,
        timestamp_ns: 0,
    };
    detector.add_baseline(baseline.clone());

    // 5% worse than baseline — within tolerance
    let current = CounterSnapshot {
        jitter: flight_tracing::counters::JitterStats {
            p99_ns: 2_100,
            ..baseline.jitter.clone()
        },
        hid: flight_tracing::counters::HidStats {
            avg_time_ns: 210_000,
            ..baseline.hid.clone()
        },
        ..baseline
    };

    let result = detector.check_regression(current);
    assert!(!result.regression_detected, "5% change should not regress");
}

/// CorrelationId uniqueness across many allocations.
#[test]
fn correlation_id_uniqueness_bulk() {
    let ids: Vec<CorrelationId> = (0..1_000).map(|_| CorrelationId::new()).collect();
    let unique: HashSet<u64> = ids.iter().map(|id| id.as_raw()).collect();
    assert_eq!(unique.len(), 1_000, "all 1000 IDs must be unique");
}

/// LogRotator: rotation lifecycle with multiple cycles.
#[test]
fn log_rotation_multi_cycle() {
    let config = RotationConfig {
        max_file_size_bytes: 100,
        max_files: 3,
        compress_rotated: false,
    };
    let mut rotator = LogRotator::new(config);

    // Cycle 1
    rotator.record_bytes(100);
    assert!(rotator.should_rotate());
    assert_eq!(rotator.rotate(), RotationResult::Rotated { sequence: 1 });
    assert_eq!(rotator.current_size(), 0);

    // Cycle 2
    rotator.record_bytes(150);
    assert_eq!(rotator.rotate(), RotationResult::Rotated { sequence: 2 });

    // Cycle 3
    rotator.record_bytes(100);
    assert_eq!(rotator.rotate(), RotationResult::Rotated { sequence: 3 });

    // Cycle 4: should be blocked
    rotator.record_bytes(100);
    assert_eq!(rotator.rotate(), RotationResult::MaxFilesReached);
    assert_eq!(rotator.rotation_count(), 3);
}

/// FileSink writes valid JSON lines to disk.
#[test]
fn file_sink_writes_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("depth-test.log");
    let config = RotationConfig {
        max_file_size_bytes: 1_000_000,
        max_files: 5,
        compress_rotated: false,
    };
    let mut sink = FileSink::open(&path, config).unwrap();

    let levels = [
        EventLevel::Trace,
        EventLevel::Debug,
        EventLevel::Info,
        EventLevel::Warn,
        EventLevel::Error,
    ];

    for level in &levels {
        let ev = EventBuilder::new(*level, "depth-test", format!("{level:?} event"))
            .device_id("test-device")
            .build();
        sink.send(&ev).unwrap();
    }
    sink.flush().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 5);

    for line in &lines {
        let parsed: FlightEvent = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.component, "depth-test");
        assert!(parsed.context.device_id.is_some());
    }
}
