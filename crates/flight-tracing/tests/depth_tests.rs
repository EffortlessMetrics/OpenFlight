// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-tracing crate.
//!
//! Covers:
//! - Span creation: create/enter/exit spans, verify timing
//! - Event recording: record events with fields, verify serialization
//! - Filter tests: level filtering (trace/debug/info/warn/error)
//! - Subscriber tests: custom sinks receive expected events
//! - Performance: event recording overhead measurement
//! - Integration: tracing + structured events + correlation pipeline
//! - Proptest: arbitrary inputs never panic

use flight_tracing::{
    ChainCollector, CorrelatedEvent, CorrelationId, EventChain, SpanCollector,
    counters::{CounterSnapshot, HidStats, JitterStats, PerfCounters},
    events::{EventData, EventFilter, TraceEvent},
    regression::RegressionDetector,
    spans::{self, FlightSpan, span_summary},
    structured::{
        EventBuilder, EventContext, EventLevel, EventSink, FileSink, FlightEvent, MemorySink,
    },
    structured_log::{JsonLogFormatter, LogEntryBuilder, LogLevel, LogValue},
    log_rotation::RotationConfig,
};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════
// 1. Span creation — create/enter/exit spans, verify timing
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn span_begin_records_nonzero_elapsed() {
    let span = FlightSpan::begin(spans::AXIS_TICK);
    thread::sleep(Duration::from_millis(1));
    let ns = span.elapsed_ns();
    assert!(ns > 0, "elapsed must be > 0 after sleep");
}

#[test]
fn span_finish_returns_positive_duration() {
    let span = FlightSpan::begin(spans::HID_READ);
    thread::sleep(Duration::from_millis(2));
    let ns = span.finish();
    assert!(ns >= 1_000_000, "finish should return ≥1ms, got {ns}ns");
}

#[test]
fn span_drop_auto_records_to_collector() {
    let collector = SpanCollector::new(1000);
    {
        let _span = collector.start_span(spans::FFB_COMPUTE);
        thread::sleep(Duration::from_millis(1));
    }
    let s = span_summary(&collector, spans::FFB_COMPUTE).expect("summary should exist");
    assert_eq!(s.count, 1);
    assert!(s.min_ns > 0);
}

#[test]
fn multiple_spans_same_operation_accumulate() {
    let collector = SpanCollector::new(1000);
    for _ in 0..10 {
        let _span = collector.start_span(spans::BUS_PUBLISH);
    }
    let s = span_summary(&collector, spans::BUS_PUBLISH).unwrap();
    assert_eq!(s.count, 10);
}

#[test]
fn different_operations_tracked_independently() {
    let collector = SpanCollector::new(1000);
    collector.record(spans::AXIS_TICK, 100);
    collector.record(spans::HID_READ, 200);
    collector.record(spans::PROFILE_COMPILE, 300);

    assert_eq!(collector.summary().len(), 3);
    assert_eq!(
        span_summary(&collector, spans::AXIS_TICK).unwrap().min_ns,
        100
    );
    assert_eq!(
        span_summary(&collector, spans::HID_READ).unwrap().min_ns,
        200
    );
    assert_eq!(
        span_summary(&collector, spans::PROFILE_COMPILE)
            .unwrap()
            .min_ns,
        300
    );
}

#[test]
fn collector_reset_clears_all_spans() {
    let collector = SpanCollector::new(1000);
    collector.record(spans::AXIS_TICK, 100);
    collector.record(spans::HID_READ, 200);
    collector.reset();
    assert!(collector.summary().is_empty());
}

#[test]
fn span_statistics_min_max_avg_p99() {
    let collector = SpanCollector::new(10_000);
    // 99 fast samples, 1 slow
    for _ in 0..99 {
        collector.record(spans::AXIS_TICK, 100);
    }
    collector.record(spans::AXIS_TICK, 10_000);

    let s = span_summary(&collector, spans::AXIS_TICK).unwrap();
    assert_eq!(s.count, 100);
    assert_eq!(s.min_ns, 100);
    assert_eq!(s.max_ns, 10_000);
    assert_eq!(s.p99_ns, 10_000);
    // avg = (99*100 + 10000) / 100 = 199
    assert_eq!(s.avg_ns, 199);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Event recording — record events with fields, verify serialization
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn trace_event_all_variants_serialize_to_json() {
    let events = vec![
        TraceEvent::tick_start(1),
        TraceEvent::tick_end(1, 4_000_000, -500),
        TraceEvent::hid_write(0x1234, 64, 100_000),
        TraceEvent::deadline_miss(42, 2_000_000),
        TraceEvent::writer_drop("axis-stream", 7),
        TraceEvent::custom("test-event", serde_json::json!({"key": "val"})),
    ];

    for ev in &events {
        let json = ev.to_json_bytes().unwrap();
        let recovered: TraceEvent = serde_json::from_slice(&json).unwrap();
        assert_eq!(recovered.timestamp_ns, ev.timestamp_ns);
        assert_eq!(recovered.event_type(), ev.event_type());
    }
}

#[test]
fn trace_event_binary_format_tick_start() {
    let ev = TraceEvent::tick_start(42);
    let binary = ev.to_binary();
    // 8 (timestamp) + 1 (type=0x01) + 8 (tick_number) = 17
    assert_eq!(binary.len(), 17);
    assert_eq!(binary[8], 0x01);
    let tick = u64::from_le_bytes(binary[9..17].try_into().unwrap());
    assert_eq!(tick, 42);
}

#[test]
fn trace_event_binary_format_deadline_miss() {
    let ev = TraceEvent::deadline_miss(99, 5_000_000);
    let binary = ev.to_binary();
    // 8 (timestamp) + 1 (type=0x04) + 8 (tick) + 8 (miss_duration) = 25
    assert_eq!(binary.len(), 25);
    assert_eq!(binary[8], 0x04);
}

#[test]
fn trace_event_binary_format_writer_drop() {
    let ev = TraceEvent::writer_drop("ax", 10);
    let binary = ev.to_binary();
    // 8 (timestamp) + 1 (type=0x05) + 1 (len=2) + 2 (stream_id) + 8 (count) = 20
    assert_eq!(binary.len(), 20);
    assert_eq!(binary[8], 0x05);
    assert_eq!(binary[9], 2); // stream_id length
}

#[test]
fn structured_event_builder_all_context_fields() {
    let ev = EventBuilder::new(EventLevel::Warn, "ffb", "force clamp")
        .device_id("stick-1")
        .sim_name("MSFS")
        .axis_name("pitch")
        .profile_name("f18_carrier")
        .build();

    assert_eq!(ev.level, EventLevel::Warn);
    assert_eq!(ev.component, "ffb");
    assert_eq!(ev.message, "force clamp");
    assert_eq!(ev.context.device_id.as_deref(), Some("stick-1"));
    assert_eq!(ev.context.sim_name.as_deref(), Some("MSFS"));
    assert_eq!(ev.context.axis_name.as_deref(), Some("pitch"));
    assert_eq!(ev.context.profile_name.as_deref(), Some("f18_carrier"));
    assert!(ev.timestamp_ns > 0);
}

#[test]
fn structured_event_json_round_trip() {
    let ev = EventBuilder::new(EventLevel::Error, "hid", "device lost")
        .device_id("js-2")
        .build();
    let json = ev.to_json().unwrap();
    let recovered: FlightEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.component, "hid");
    assert_eq!(recovered.level, EventLevel::Error);
    assert_eq!(recovered.context.device_id.as_deref(), Some("js-2"));
    assert_eq!(recovered.timestamp_ns, ev.timestamp_ns);
}

#[test]
fn structured_event_omits_none_context_fields() {
    let ev = EventBuilder::new(EventLevel::Info, "bus", "publish").build();
    let json = ev.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["context"].get("device_id").is_none());
    assert!(parsed["context"].get("sim_name").is_none());
    assert!(parsed["context"].get("axis_name").is_none());
    assert!(parsed["context"].get("profile_name").is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Filter tests — level filtering (trace/debug/info/warn/error)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn event_level_ordering() {
    assert!(EventLevel::Trace < EventLevel::Debug);
    assert!(EventLevel::Debug < EventLevel::Info);
    assert!(EventLevel::Info < EventLevel::Warn);
    assert!(EventLevel::Warn < EventLevel::Error);
}

#[test]
fn event_filter_default_disables_custom() {
    let filter = EventFilter::default();
    assert!(filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::hid_write(0, 64, 100)));
    assert!(filter.should_trace(&TraceEvent::deadline_miss(1, 100)));
    assert!(filter.should_trace(&TraceEvent::writer_drop("s", 1)));
    assert!(!filter.should_trace(&TraceEvent::custom(
        "x",
        serde_json::json!(null)
    )));
}

#[test]
fn event_filter_development_enables_all() {
    let filter = EventFilter::development();
    assert!(filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::custom(
        "x",
        serde_json::json!(null)
    )));
}

#[test]
fn event_filter_ci_minimal_disables_ticks() {
    let filter = EventFilter::ci_minimal();
    assert!(!filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(!filter.should_trace(&TraceEvent::tick_end(1, 100, 0)));
    assert!(filter.should_trace(&TraceEvent::hid_write(0, 0, 0)));
    assert!(filter.should_trace(&TraceEvent::deadline_miss(1, 0)));
}

#[test]
fn event_filter_selective_toggle() {
    let filter = EventFilter {
        tick_events: false,
        hid_events: false,
        deadline_events: true,
        writer_events: true,
        custom_events: false,
    };
    assert!(!filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(!filter.should_trace(&TraceEvent::hid_write(0, 0, 0)));
    assert!(filter.should_trace(&TraceEvent::deadline_miss(1, 0)));
    assert!(filter.should_trace(&TraceEvent::writer_drop("s", 1)));
}

#[test]
fn structured_log_level_filtering() {
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

    let min_level = EventLevel::Warn;
    let filtered: Vec<_> = events.iter().filter(|e| e.level >= min_level).collect();
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].level, EventLevel::Warn);
    assert_eq!(filtered[1].level, EventLevel::Error);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Subscriber / sink tests — custom sinks receive expected events
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn memory_sink_stores_events() {
    let mut sink = MemorySink::new(10);
    for i in 0..5 {
        let ev = EventBuilder::new(EventLevel::Info, "test", format!("msg {i}")).build();
        sink.send(&ev).unwrap();
    }
    assert_eq!(sink.len(), 5);
    let snap = sink.snapshot();
    assert_eq!(snap[0].message, "msg 0");
    assert_eq!(snap[4].message, "msg 4");
}

#[test]
fn memory_sink_ring_buffer_eviction() {
    let mut sink = MemorySink::new(3);
    for i in 0..7 {
        let ev = EventBuilder::new(EventLevel::Info, "t", format!("e{i}")).build();
        sink.send(&ev).unwrap();
    }
    assert_eq!(sink.len(), 3);
    assert_eq!(sink.total_received(), 7);
    let snap = sink.snapshot();
    assert_eq!(snap[0].message, "e4");
    assert_eq!(snap[1].message, "e5");
    assert_eq!(snap[2].message, "e6");
}

#[test]
fn memory_sink_clear_resets_everything() {
    let mut sink = MemorySink::new(10);
    let ev = EventBuilder::new(EventLevel::Info, "t", "x").build();
    sink.send(&ev).unwrap();
    sink.clear();
    assert!(sink.is_empty());
    assert_eq!(sink.total_received(), 0);
}

#[test]
fn file_sink_writes_valid_json_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.log");
    let config = RotationConfig {
        max_file_size_bytes: 1_000_000,
        max_files: 5,
        compress_rotated: false,
    };
    let mut sink = FileSink::open(&path, config).unwrap();

    for i in 0..5 {
        let ev = EventBuilder::new(EventLevel::Info, "test", format!("line {i}")).build();
        sink.send(&ev).unwrap();
    }
    sink.flush().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 5);
    for line in &lines {
        let _: FlightEvent = serde_json::from_str(line).expect("each line must be valid JSON");
    }
}

#[test]
fn file_sink_rotation_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rot.log");
    let config = RotationConfig {
        max_file_size_bytes: 50,
        max_files: 5,
        compress_rotated: false,
    };
    let mut sink = FileSink::open(&path, config).unwrap();

    for i in 0..20 {
        let ev = EventBuilder::new(EventLevel::Info, "t", format!("event {i}")).build();
        sink.send(&ev).unwrap();
    }
    sink.flush().unwrap();

    let rotated = dir.path().join("rot.log.1");
    assert!(rotated.exists(), "rotated file should exist");
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Performance — event recording overhead measurement
// ═══════════════════════════════════════════════════════════════════════════

// Run with: cargo test -p flight-tracing -- --ignored (benchmark suite)
#[test]
#[ignore]
fn perf_counters_throughput() {
    let counters = PerfCounters::new();
    let start = std::time::Instant::now();

    for i in 0..100_000u64 {
        counters.record_event(&TraceEvent::tick_start(i));
    }

    let elapsed = start.elapsed();
    let snap = counters.snapshot();
    assert_eq!(snap.total_ticks, 100_000);
    assert!(
        elapsed.as_millis() < 1000,
        "100k tick_start events should complete in <1s, took {}ms",
        elapsed.as_millis()
    );
}

// Run with: cargo test -p flight-tracing -- --ignored (benchmark suite)
#[test]
#[ignore]
fn span_collector_throughput() {
    let collector = SpanCollector::new(10_000);
    let start = std::time::Instant::now();

    for i in 0..50_000u64 {
        collector.record(spans::AXIS_TICK, i % 10_000);
    }

    let elapsed = start.elapsed();
    let s = span_summary(&collector, spans::AXIS_TICK).unwrap();
    assert_eq!(s.count, 50_000);
    assert!(
        elapsed.as_millis() < 1000,
        "50k span records should complete in <1s, took {}ms",
        elapsed.as_millis()
    );
}

// Run with: cargo test -p flight-tracing -- --ignored (benchmark suite)
#[test]
#[ignore]
fn structured_event_creation_overhead() {
    let start = std::time::Instant::now();

    for _ in 0..10_000 {
        let _ev = EventBuilder::new(EventLevel::Info, "axis", "tick")
            .device_id("js-0")
            .sim_name("MSFS")
            .build();
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "10k structured events should be created in <500ms, took {}ms",
        elapsed.as_millis()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Integration — tracing components together
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn correlation_chain_end_to_end() {
    let collector = ChainCollector::new(100);
    let id = CorrelationId::new();

    let events = [
        ("hid", "joystick input", 1_000_000u64),
        ("axis", "curve applied", 1_200_000),
        ("simconnect", "value sent", 1_800_000),
    ];

    for (component, message, timestamp_ns) in &events {
        let ev = FlightEvent {
            timestamp_ns: *timestamp_ns,
            level: EventLevel::Info,
            component: component.to_string(),
            message: message.to_string(),
            context: EventContext::default(),
        };
        collector.record(CorrelatedEvent::new(id, ev));
    }

    let chain = collector.take_chain(&id).unwrap();
    assert_eq!(chain.len(), 3);
    assert_eq!(chain.duration_ns(), Some(800_000));
    assert_eq!(chain.events()[0].component, "hid");
    assert_eq!(chain.events()[2].component, "simconnect");
}

#[test]
fn correlation_chain_eviction_at_capacity() {
    let collector = ChainCollector::new(2);
    let id1 = CorrelationId::from_raw(10);
    let id2 = CorrelationId::from_raw(20);
    let id3 = CorrelationId::from_raw(30);

    let ev = || FlightEvent {
        timestamp_ns: 0,
        level: EventLevel::Info,
        component: "x".into(),
        message: "y".into(),
        context: EventContext::default(),
    };

    collector.record(CorrelatedEvent::new(id1, ev()));
    collector.record(CorrelatedEvent::new(id2, ev()));
    collector.record(CorrelatedEvent::new(id3, ev()));

    assert_eq!(collector.active_chains(), 2);
    assert!(
        collector.get_chain(&id1).is_none(),
        "oldest should be evicted"
    );
    assert!(collector.get_chain(&id2).is_some());
    assert!(collector.get_chain(&id3).is_some());
}

#[test]
fn perf_counters_to_regression_detector_pipeline() {
    let counters = PerfCounters::new();

    // Generate baseline data
    for i in 0..2000u64 {
        counters.record_event(&TraceEvent::tick_start(i));
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 100)); // 100ns jitter
        counters.record_event(&TraceEvent::hid_write(0x1234, 64, 50_000)); // 50μs
    }

    let baseline = counters.snapshot();
    let mut detector = RegressionDetector::new();
    detector.add_baseline(baseline);

    // Simulate good performance
    counters.reset();
    for i in 0..2000u64 {
        counters.record_event(&TraceEvent::tick_start(i));
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 110)); // 110ns, ~10% increase
        counters.record_event(&TraceEvent::hid_write(0x1234, 64, 55_000));
    }

    let result = detector.check_regression(counters.snapshot());
    assert!(
        !result.regression_detected,
        "small increase should not trigger regression"
    );
}

#[test]
fn perf_counters_quality_gate_pass() {
    let counters = PerfCounters::new();
    for i in 0..2000u64 {
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 100));
    }
    let result = counters.check_quality_gates();
    assert!(result.passed, "100ns jitter should pass quality gate");
}

#[test]
fn perf_counters_quality_gate_fail() {
    let counters = PerfCounters::new();
    for i in 0..2000u64 {
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, 1_000_000)); // 1ms jitter
    }
    let result = counters.check_quality_gates();
    assert!(!result.passed, "1ms jitter should fail quality gate");
    assert!(!result.violations.is_empty());
}

#[test]
fn structured_log_json_formatter_with_all_field_types() {
    let entry = LogEntryBuilder::new(LogLevel::Info, "axis", "tick processed")
        .field("axis_id", LogValue::Int(3))
        .field("value", LogValue::Float(0.75))
        .field("saturated", LogValue::Bool(false))
        .field("device", LogValue::String("js-0".into()))
        .span_id("span-42")
        .trace_id("trace-7")
        .build();

    let json = JsonLogFormatter::format(&entry);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["level"], "INFO");
    assert_eq!(parsed["component"], "axis");
    assert_eq!(parsed["fields"]["axis_id"], 3);
    assert_eq!(parsed["fields"]["value"], 0.75);
    assert_eq!(parsed["fields"]["saturated"], false);
    assert_eq!(parsed["fields"]["device"], "js-0");
    assert_eq!(parsed["span_id"], "span-42");
    assert_eq!(parsed["trace_id"], "trace-7");
}

#[test]
fn structured_log_batch_format() {
    let entries: Vec<_> = (0..5)
        .map(|i| LogEntryBuilder::new(LogLevel::Debug, "test", &format!("msg {i}")).build())
        .collect();
    let batch = JsonLogFormatter::format_batch(&entries);
    let lines: Vec<&str> = batch.lines().collect();
    assert_eq!(lines.len(), 5);
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line).expect("each line must be valid JSON");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Concurrency tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn concurrent_perf_counter_writes() {
    let counters = Arc::new(PerfCounters::new());
    let num_threads = 4u64;
    let per_thread = 1000u64;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let c = Arc::clone(&counters);
            thread::spawn(move || {
                for i in 0..per_thread {
                    c.record_event(&TraceEvent::tick_start(t * 10_000 + i));
                    c.record_event(&TraceEvent::hid_write(t as u32, 64, 100_000));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let snap = counters.snapshot();
    assert_eq!(snap.total_ticks, num_threads * per_thread);
    assert_eq!(snap.total_hid_writes, num_threads * per_thread);
}

#[test]
fn concurrent_span_collector() {
    let collector = Arc::new(SpanCollector::new(10_000));
    let num_threads = 4;
    let per_thread = 500;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&collector);
            thread::spawn(move || {
                for i in 0..per_thread {
                    c.record(spans::AXIS_TICK, i as u64 * 100);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let s = span_summary(&collector, spans::AXIS_TICK).unwrap();
    assert_eq!(s.count, (num_threads * per_thread) as u64);
}

#[test]
fn concurrent_chain_collector() {
    let collector = Arc::new(ChainCollector::new(1000));
    let num_threads = 4;
    let per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&collector);
            thread::spawn(move || {
                for _ in 0..per_thread {
                    let id = CorrelationId::new();
                    let ev = FlightEvent {
                        timestamp_ns: 0,
                        level: EventLevel::Info,
                        component: "t".into(),
                        message: "m".into(),
                        context: EventContext::default(),
                    };
                    c.record(CorrelatedEvent::new(id, ev));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert!(
        collector.active_chains() <= 1000,
        "should respect capacity limit"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Regression detector edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn regression_detector_no_baseline_no_regression() {
    let detector = RegressionDetector::new();
    let snap = CounterSnapshot {
        total_ticks: 1000,
        deadline_misses: 0,
        miss_rate: 0.0,
        total_hid_writes: 100,
        writer_drops: 0,
        jitter: JitterStats {
            p50_ns: 100,
            p99_ns: 200,
            max_ns: 300,
            sample_count: 1000,
        },
        hid: HidStats {
            total_writes: 100,
            total_time_ns: 10_000_000,
            avg_time_ns: 100_000,
            p99_time_ns: 200_000,
        },
        session_duration_ms: 4000,
        timestamp_ns: 0,
    };
    let result = detector.check_regression(snap);
    assert!(
        !result.regression_detected,
        "no regression without baselines for good data"
    );
}

#[test]
fn regression_detector_baseline_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("baselines.json");

    let mut detector = RegressionDetector::new();
    for i in 0..3 {
        detector.add_baseline(CounterSnapshot {
            total_ticks: 1000,
            deadline_misses: 0,
            miss_rate: 0.0,
            total_hid_writes: 100,
            writer_drops: 0,
            jitter: JitterStats {
                p50_ns: 100 + i * 10,
                p99_ns: 200 + i * 10,
                max_ns: 300,
                sample_count: 1000,
            },
            hid: HidStats {
                total_writes: 100,
                total_time_ns: 10_000_000,
                avg_time_ns: 100_000,
                p99_time_ns: 200_000,
            },
            session_duration_ms: 4000,
            timestamp_ns: 0,
        });
    }

    detector.save_baselines(&path).unwrap();

    let mut loaded = RegressionDetector::new();
    loaded.load_baselines(&path).unwrap();
    let summary = loaded.get_baseline_summary().unwrap();
    assert_eq!(summary.count, 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Proptest — arbitrary inputs never panic
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_trace_event_json_round_trip(
        tick in any::<u64>(),
        duration in any::<u64>(),
        jitter in i64::MIN..i64::MAX,
    ) {
        let ev = TraceEvent::tick_end(tick, duration, jitter);
        let bytes = ev.to_json_bytes().unwrap();
        let decoded: TraceEvent = serde_json::from_slice(&bytes).unwrap();
        prop_assert_eq!(decoded.timestamp_ns, ev.timestamp_ns);
        match decoded.data {
            EventData::TickEnd { tick_number, duration_ns, jitter_ns } => {
                prop_assert_eq!(tick_number, tick);
                prop_assert_eq!(duration_ns, duration);
                prop_assert_eq!(jitter_ns, jitter);
            }
            _ => prop_assert!(false, "wrong variant"),
        }
    }

    #[test]
    fn prop_hid_write_binary_is_25_bytes(
        device_id in any::<u32>(),
        bytes in 0usize..65535,
        duration in any::<u64>(),
    ) {
        let ev = TraceEvent::hid_write(device_id, bytes, duration);
        let binary = ev.to_binary();
        prop_assert_eq!(binary.len(), 25);
    }

    #[test]
    fn prop_tick_end_binary_is_33_bytes(
        tick in any::<u64>(),
        duration in any::<u64>(),
        jitter in any::<i64>(),
    ) {
        let ev = TraceEvent::tick_end(tick, duration, jitter);
        let binary = ev.to_binary();
        prop_assert_eq!(binary.len(), 33);
    }

    #[test]
    fn prop_structured_event_never_panics(
        level in prop_oneof![
            Just(EventLevel::Trace),
            Just(EventLevel::Debug),
            Just(EventLevel::Info),
            Just(EventLevel::Warn),
            Just(EventLevel::Error),
        ],
        component in "[a-z_]{1,20}",
        message in "[ -~]{0,100}",
    ) {
        let ev = EventBuilder::new(level, &component, &message).build();
        let json = ev.to_json().unwrap();
        let _: FlightEvent = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn prop_correlation_id_unique(a in any::<u64>(), b in any::<u64>()) {
        prop_assume!(a != b);
        let id_a = CorrelationId::from_raw(a);
        let id_b = CorrelationId::from_raw(b);
        prop_assert_ne!(id_a, id_b);
    }

    #[test]
    fn prop_writer_drop_records_count(
        stream in "[a-z]{1,16}",
        count in 0u64..100_000,
    ) {
        let counters = PerfCounters::new();
        counters.record_event(&TraceEvent::writer_drop(stream, count));
        let snap = counters.snapshot();
        prop_assert_eq!(snap.writer_drops, count);
    }

    #[test]
    fn prop_event_chain_duration_consistent(
        ts1 in 0u64..1_000_000_000,
        ts2 in 0u64..1_000_000_000,
    ) {
        let mut chain = EventChain::new(CorrelationId::from_raw(1));
        let (first, second) = if ts1 <= ts2 { (ts1, ts2) } else { (ts2, ts1) };
        chain.push(FlightEvent {
            timestamp_ns: first,
            level: EventLevel::Info,
            component: "a".into(),
            message: "b".into(),
            context: EventContext::default(),
        });
        chain.push(FlightEvent {
            timestamp_ns: second,
            level: EventLevel::Info,
            component: "c".into(),
            message: "d".into(),
            context: EventContext::default(),
        });
        let dur = chain.duration_ns().unwrap();
        prop_assert_eq!(dur, second - first);
    }

    #[test]
    fn prop_log_entry_json_is_valid(
        component in "[a-z]{1,10}",
        message in "[ -~]{0,50}",
        int_val in any::<i64>(),
    ) {
        let entry = LogEntryBuilder::new(LogLevel::Info, &component, &message)
            .field("val", LogValue::Int(int_val))
            .build();
        let json = JsonLogFormatter::format(&entry);
        let _: serde_json::Value = serde_json::from_str(&json).expect("must be valid JSON");
    }
}
