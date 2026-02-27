// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive integration tests for the flight-tracing crate.
//!
//! Exercises the public API at arm's length — no access to private fields.
//! Tests are grouped by the capability they verify:
//!
//! 1.  `TraceEvent` stores all fields of each variant correctly
//! 2.  Span timestamps are non-zero (wall-clock populated)
//! 3.  Span durations are positive
//! 4.  Nested / multiple spans each increment the counter independently
//! 5.  Jitter buffer respects its 10 000-sample cap (oldest sample dropped)
//! 6.  `CounterSnapshot::to_json` and `TraceEvent::to_json_bytes` produce
//!     valid, round-trippable JSON
//! 7.  `EventFilter` lets each event type be enabled/disabled individually
//! 8.  Concurrent writes to an isolated `PerfCounters` are race-free
//! 9.  `PerfCounters::reset` zeroes every counter and clears all samples
//! 10. `proptest`: arbitrary UTF-8 names/stream IDs never panic;
//!     binary encoding of `TickEnd` and `HidWrite` always hits the expected
//!     fixed size.

use flight_tracing::{
    CounterSnapshot,
    counters::PerfCounters,
    events::{EventData, EventFilter, TraceEvent},
};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ── 1. TraceEvent stores all fields of each variant correctly ─────────────

#[test]
fn tick_start_stores_tick_number() {
    let ev = TraceEvent::tick_start(99);
    assert_eq!(ev.event_type(), "TickStart");
    match ev.data {
        EventData::TickStart { tick_number } => assert_eq!(tick_number, 99),
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn tick_end_stores_all_three_fields() {
    let ev = TraceEvent::tick_end(5, 4_000_000, -750);
    assert_eq!(ev.event_type(), "TickEnd");
    match ev.data {
        EventData::TickEnd {
            tick_number,
            duration_ns,
            jitter_ns,
        } => {
            assert_eq!(tick_number, 5);
            assert_eq!(duration_ns, 4_000_000);
            assert_eq!(jitter_ns, -750);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn hid_write_stores_device_bytes_and_duration() {
    let ev = TraceEvent::hid_write(0xDEAD, 128, 99_000);
    assert_eq!(ev.event_type(), "HidWrite");
    match ev.data {
        EventData::HidWrite {
            device_id,
            bytes,
            duration_ns,
        } => {
            assert_eq!(device_id, 0xDEAD);
            assert_eq!(bytes, 128);
            assert_eq!(duration_ns, 99_000);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn deadline_miss_stores_tick_and_miss_duration() {
    let ev = TraceEvent::deadline_miss(42, 3_000_000);
    assert_eq!(ev.event_type(), "DeadlineMiss");
    match ev.data {
        EventData::DeadlineMiss {
            tick_number,
            miss_duration_ns,
        } => {
            assert_eq!(tick_number, 42);
            assert_eq!(miss_duration_ns, 3_000_000);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn writer_drop_stores_stream_id_and_count() {
    let ev = TraceEvent::writer_drop("pitch-axis", 7);
    assert_eq!(ev.event_type(), "WriterDrop");
    match ev.data {
        EventData::WriterDrop {
            stream_id,
            dropped_count,
        } => {
            assert_eq!(stream_id, "pitch-axis");
            assert_eq!(dropped_count, 7);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn custom_event_stores_name_and_json_value() {
    let data = serde_json::json!({"key": "value", "num": 42});
    let ev = TraceEvent::custom("my-event", data.clone());
    assert_eq!(ev.event_type(), "Custom");
    match &ev.data {
        EventData::Custom { name, data: d } => {
            assert_eq!(name, "my-event");
            assert_eq!(d["key"], "value");
            assert_eq!(d["num"], 42);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

// ── 2. Span timestamps are non-zero ───────────────────────────────────────

#[test]
fn trace_event_timestamp_is_nonzero() {
    // Every event is stamped at construction time with nanoseconds since epoch.
    for ev in [
        TraceEvent::tick_start(0),
        TraceEvent::tick_end(0, 0, 0),
        TraceEvent::hid_write(0, 0, 0),
        TraceEvent::deadline_miss(0, 0),
        TraceEvent::writer_drop("s", 0),
    ] {
        assert!(
            ev.timestamp_ns > 0,
            "timestamp should be > 0 for {}",
            ev.event_type()
        );
    }
}

#[test]
fn consecutive_events_have_nondecreasing_timestamps() {
    let a = TraceEvent::tick_start(1);
    let b = TraceEvent::tick_end(1, 1_000_000, 0);
    assert!(
        b.timestamp_ns >= a.timestamp_ns,
        "timestamps must be non-decreasing"
    );
}

// ── 3. Span durations are positive ────────────────────────────────────────

#[test]
fn tick_end_with_nonzero_duration_records_correctly() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::tick_start(1));
    counters.record_event(&TraceEvent::tick_end(1, 4_000_000, 500));

    let snap = counters.snapshot();
    assert_eq!(snap.total_ticks, 1);
    assert_eq!(snap.jitter.sample_count, 1);
    assert_eq!(snap.jitter.p50_ns, 500);
}

#[test]
fn hid_write_with_nonzero_duration_records_avg_latency() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::hid_write(0x1234, 64, 50_000));
    let snap = counters.snapshot();
    assert_eq!(snap.total_hid_writes, 1);
    assert_eq!(snap.hid.avg_time_ns, 50_000);
    assert!(snap.hid.avg_time_ns > 0, "average latency must be positive");
}

// ── 4. Nested / multiple spans each increment the counter ─────────────────

#[test]
fn two_consecutive_tick_spans_each_increment_counter() {
    let counters = PerfCounters::new();

    counters.record_event(&TraceEvent::tick_start(1));
    counters.record_event(&TraceEvent::tick_end(1, 4_000_000, 100));

    counters.record_event(&TraceEvent::tick_start(2));
    counters.record_event(&TraceEvent::tick_end(2, 4_100_000, 200));

    let snap = counters.snapshot();
    assert_eq!(
        snap.total_ticks, 2,
        "each span should increment the tick counter"
    );
    assert_eq!(
        snap.jitter.sample_count, 2,
        "each TickEnd adds a jitter sample"
    );
}

#[test]
fn overlapping_hid_writes_accumulate_independently() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::hid_write(0x01, 64, 100_000));
    counters.record_event(&TraceEvent::hid_write(0x02, 32, 200_000));
    counters.record_event(&TraceEvent::hid_write(0x03, 16, 150_000));

    let snap = counters.snapshot();
    assert_eq!(snap.total_hid_writes, 3);
    let expected_avg = (100_000 + 200_000 + 150_000) / 3;
    assert_eq!(snap.hid.avg_time_ns, expected_avg);
}

// ── 5. Jitter buffer respects its 10 000-sample cap ───────────────────────

#[test]
fn jitter_buffer_caps_at_10000_samples() {
    let counters = PerfCounters::new();
    // Record 12 000 samples — 2 000 more than the 10 000-sample internal cap.
    for i in 0u64..12_000 {
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, (i % 1000) as i64));
    }
    let snap = counters.snapshot();
    assert!(
        snap.jitter.sample_count <= 10_000,
        "jitter buffer must be capped at 10 000; got {}",
        snap.jitter.sample_count
    );
}

#[test]
fn hid_latency_buffer_caps_and_p99_is_still_populated() {
    let counters = PerfCounters::new();
    for i in 0u64..11_000 {
        counters.record_event(&TraceEvent::hid_write(0, 64, 100_000 + i));
    }
    let snap = counters.snapshot();
    // The unbounded atomic counter must reflect every event.
    assert_eq!(snap.total_hid_writes, 11_000);
    // p99 is computed over the capped sample window — still non-zero.
    assert!(snap.hid.p99_time_ns > 0);
}

// ── 6. JSON export produces valid, round-trippable output ─────────────────

#[test]
fn counter_snapshot_to_json_contains_all_top_level_fields() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::tick_start(1));
    counters.record_event(&TraceEvent::tick_end(1, 4_000_000, 500));
    counters.record_event(&TraceEvent::hid_write(0x1234, 64, 200_000));
    counters.record_event(&TraceEvent::deadline_miss(1, 1_000_000));
    counters.record_event(&TraceEvent::writer_drop("axis", 3));

    let snap = counters.snapshot();
    let json = snap.to_json().expect("to_json should not fail");

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should produce valid JSON");
    assert_eq!(parsed["total_ticks"], 1);
    assert_eq!(parsed["total_hid_writes"], 1);
    assert_eq!(parsed["deadline_misses"], 1);
    assert_eq!(parsed["writer_drops"], 3);
    assert!(
        parsed["jitter"].is_object(),
        "jitter sub-object must be present"
    );
    assert!(parsed["hid"].is_object(), "hid sub-object must be present");
}

#[test]
fn counter_snapshot_to_json_round_trips_via_serde() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::tick_end(1, 4_000_000, 1_500));
    let snap = counters.snapshot();
    let json = snap.to_json().unwrap();
    let recovered: CounterSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.total_ticks, snap.total_ticks);
    assert_eq!(recovered.jitter.p50_ns, snap.jitter.p50_ns);
}

#[test]
fn trace_event_to_json_bytes_round_trips_tick_end() {
    let ev = TraceEvent::tick_end(77, 3_900_000, 250);
    let bytes = ev.to_json_bytes().expect("serialization should not fail");
    let decoded: TraceEvent =
        serde_json::from_slice(&bytes).expect("deserialization should not fail");
    assert_eq!(decoded.timestamp_ns, ev.timestamp_ns);
    assert_eq!(decoded.event_type(), "TickEnd");
}

#[test]
fn hid_write_json_round_trip_preserves_all_fields() {
    let ev = TraceEvent::hid_write(0x5678, 64, 125_000);
    let bytes = ev.to_json_bytes().unwrap();
    let decoded: TraceEvent = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded.event_type(), "HidWrite");
    match decoded.data {
        EventData::HidWrite {
            device_id,
            bytes: b,
            duration_ns,
        } => {
            assert_eq!(device_id, 0x5678);
            assert_eq!(b, 64);
            assert_eq!(duration_ns, 125_000);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

// ── 7. EventFilter enables/disables each event type independently ─────────

#[test]
fn default_filter_passes_all_system_events_but_not_custom() {
    let filter = EventFilter::default();
    assert!(filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::tick_end(1, 1_000_000, 0)));
    assert!(filter.should_trace(&TraceEvent::hid_write(0, 64, 100_000)));
    assert!(filter.should_trace(&TraceEvent::deadline_miss(1, 1_000_000)));
    assert!(filter.should_trace(&TraceEvent::writer_drop("axis", 1)));
    assert!(
        !filter.should_trace(&TraceEvent::custom("test", serde_json::json!({}))),
        "custom events must be disabled by default"
    );
}

#[test]
fn development_filter_enables_all_event_types() {
    let filter = EventFilter::development();
    assert!(filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::hid_write(0, 0, 0)));
    assert!(
        filter.should_trace(&TraceEvent::custom("any", serde_json::json!({}))),
        "custom events must be enabled in development filter"
    );
}

#[test]
fn ci_minimal_filter_disables_tick_events_only() {
    let filter = EventFilter::ci_minimal();
    assert!(
        !filter.should_trace(&TraceEvent::tick_start(1)),
        "tick start must be disabled in ci_minimal"
    );
    assert!(
        !filter.should_trace(&TraceEvent::tick_end(1, 0, 0)),
        "tick end must be disabled in ci_minimal"
    );
    // Non-tick events remain enabled.
    assert!(filter.should_trace(&TraceEvent::hid_write(0, 0, 0)));
    assert!(filter.should_trace(&TraceEvent::deadline_miss(1, 0)));
    assert!(filter.should_trace(&TraceEvent::writer_drop("axis", 0)));
}

#[test]
fn custom_filter_with_only_hid_enabled() {
    let filter = EventFilter {
        tick_events: false,
        hid_events: true,
        deadline_events: false,
        writer_events: false,
        custom_events: false,
    };
    assert!(!filter.should_trace(&TraceEvent::tick_start(1)));
    assert!(filter.should_trace(&TraceEvent::hid_write(0, 0, 0)));
    assert!(!filter.should_trace(&TraceEvent::deadline_miss(1, 0)));
    assert!(!filter.should_trace(&TraceEvent::writer_drop("s", 0)));
    assert!(!filter.should_trace(&TraceEvent::custom("x", serde_json::json!(null))));
}

// ── 8. Concurrent writes are race-free ────────────────────────────────────

#[test]
fn concurrent_tick_and_hid_writes_produce_correct_totals() {
    let counters = Arc::new(PerfCounters::new());
    let num_threads: u64 = 8;
    let events_per_thread: u64 = 500;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let c = Arc::clone(&counters);
            thread::spawn(move || {
                for i in 0..events_per_thread {
                    c.record_event(&TraceEvent::tick_start(t * 10_000 + i));
                    c.record_event(&TraceEvent::hid_write(t as u32, 64, 100_000 + i));
                    c.record_event(&TraceEvent::deadline_miss(t * 10_000 + i, 1_000_000));
                    c.record_event(&TraceEvent::writer_drop("axis", 1));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("worker thread must not panic");
    }

    let snap = counters.snapshot();
    let expected = num_threads * events_per_thread;
    assert_eq!(
        snap.total_ticks, expected,
        "tick count mismatch under concurrency"
    );
    assert_eq!(snap.total_hid_writes, expected, "HID write count mismatch");
    assert_eq!(
        snap.deadline_misses, expected,
        "deadline miss count mismatch"
    );
    assert_eq!(snap.writer_drops, expected, "writer drop count mismatch");
}

#[test]
fn concurrent_jitter_samples_dont_deadlock_or_corrupt() {
    let counters = Arc::new(PerfCounters::new());

    let handles: Vec<_> = (0..4)
        .map(|t| {
            let c = Arc::clone(&counters);
            thread::spawn(move || {
                for i in 0u64..250 {
                    c.record_event(&TraceEvent::tick_end(t * 1000 + i, 4_000_000, i as i64));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let snap = counters.snapshot();
    // 4 threads × 250 TickEnd events = 1 000 total jitter samples.
    assert_eq!(snap.jitter.sample_count, 1000);
    assert!(snap.jitter.p99_ns >= 0, "p99 must be non-negative");
}

// ── 9. PerfCounters::reset zeroes every counter ───────────────────────────

#[test]
fn reset_zeroes_all_atomic_counters() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::tick_start(1));
    counters.record_event(&TraceEvent::hid_write(0, 64, 100_000));
    counters.record_event(&TraceEvent::deadline_miss(1, 500_000));
    counters.record_event(&TraceEvent::writer_drop("axis", 3));

    counters.reset();

    let snap = counters.snapshot();
    assert_eq!(snap.total_ticks, 0, "ticks should be 0 after reset");
    assert_eq!(
        snap.total_hid_writes, 0,
        "HID writes should be 0 after reset"
    );
    assert_eq!(
        snap.deadline_misses, 0,
        "deadline misses should be 0 after reset"
    );
    assert_eq!(snap.writer_drops, 0, "writer drops should be 0 after reset");
    assert_eq!(snap.miss_rate, 0.0, "miss rate should be 0.0 after reset");
}

#[test]
fn reset_clears_jitter_and_hid_samples() {
    let counters = PerfCounters::new();
    for i in 0..100 {
        counters.record_event(&TraceEvent::tick_end(i, 4_000_000, i as i64));
        counters.record_event(&TraceEvent::hid_write(0, 64, 100_000 + i));
    }

    let before = counters.snapshot();
    assert_eq!(before.jitter.sample_count, 100);

    counters.reset();

    let after = counters.snapshot();
    assert_eq!(
        after.jitter.sample_count, 0,
        "jitter samples should be cleared after reset"
    );
    assert_eq!(
        after.hid.p99_time_ns, 0,
        "HID p99 should be 0 after reset (no samples)"
    );
}

#[test]
fn can_record_again_after_reset() {
    let counters = PerfCounters::new();
    counters.record_event(&TraceEvent::tick_start(1));
    counters.reset();
    counters.record_event(&TraceEvent::tick_start(2));

    let snap = counters.snapshot();
    assert_eq!(
        snap.total_ticks, 1,
        "should count only the post-reset event"
    );
}

// ── 10. Proptest: arbitrary input never panics; binary sizes are invariant ─

proptest! {
    /// Any valid UTF-8 string can be a custom event name without panicking.
    #[test]
    fn custom_event_with_arbitrary_name_never_panics(
        name in "[\\u{0020}-\\u{007E}\\u{00A0}-\\u{07FF}]{0,64}",
        value in ".*",
    ) {
        let ev = TraceEvent::custom(name, serde_json::json!({"v": value}));
        let bytes = ev.to_json_bytes().unwrap();
        prop_assert!(!bytes.is_empty());
    }

    /// Any UTF-8 stream ID and drop count are accepted by WriterDrop.
    #[test]
    fn writer_drop_with_arbitrary_stream_id_stores_count(
        stream_id in "[\\u{0020}-\\u{007E}]{0,128}",
        count in 0u64..1_000_000,
    ) {
        let counters = PerfCounters::new();
        counters.record_event(&TraceEvent::writer_drop(stream_id, count));
        let snap = counters.snapshot();
        prop_assert_eq!(snap.writer_drops, count);
    }

    /// TickEnd binary encoding is always exactly 33 bytes.
    #[test]
    fn tick_end_binary_is_always_33_bytes(
        tick in 0u64..u64::MAX,
        duration in 0u64..u64::MAX,
        jitter in i64::MIN..i64::MAX,
    ) {
        let ev = TraceEvent::tick_end(tick, duration, jitter);
        let binary = ev.to_binary();
        // 8 (timestamp) + 1 (type) + 8 (tick) + 8 (duration) + 8 (jitter) = 33
        prop_assert_eq!(binary.len(), 33);
    }

    /// HidWrite binary encoding is always exactly 25 bytes.
    #[test]
    fn hid_write_binary_is_always_25_bytes(
        device_id in 0u32..u32::MAX,
        bytes in 0usize..65535usize,
        duration in 0u64..u64::MAX,
    ) {
        let ev = TraceEvent::hid_write(device_id, bytes, duration);
        let binary = ev.to_binary();
        // 8 (timestamp) + 1 (type) + 4 (device) + 4 (bytes as u32) + 8 (duration) = 25
        prop_assert_eq!(binary.len(), 25);
    }

    /// `to_json_bytes` followed by `from_slice` always recovers the same data.
    #[test]
    fn tick_start_json_round_trip_preserves_tick_number(tick in 0u64..u64::MAX) {
        let ev = TraceEvent::tick_start(tick);
        let bytes = ev.to_json_bytes().unwrap();
        let decoded: TraceEvent = serde_json::from_slice(&bytes).unwrap();
        prop_assert_eq!(decoded.timestamp_ns, ev.timestamp_ns);
        match decoded.data {
            EventData::TickStart { tick_number } => prop_assert_eq!(tick_number, tick),
            other => prop_assert!(false, "wrong variant: {other:?}"),
        }
    }
}
