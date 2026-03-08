// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Records bus events with timestamps for test assertions and golden-file export.
//!
//! [`TraceRecorder`] captures [`BusEvent`]s alongside a deterministic or
//! wall-clock timestamp, then exposes helpers for asserting event order,
//! presence, and absence.

use flight_bus::routing::{BusEvent, EventKind, EventPayload, SourceType};
use serde::{Deserialize, Serialize};

/// A bus event together with the recorder-assigned timestamp (µs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedEvent {
    pub timestamp_us: u64,
    pub source_type: String,
    pub kind: String,
    pub source_id: u32,
    pub payload: PayloadSnapshot,
}

/// Serialisable snapshot of an [`EventPayload`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PayloadSnapshot {
    Axis { axis_id: u16, value: f64 },
    Button { button_id: u16, pressed: bool },
    Telemetry { field_id: u16, value: f64 },
    System { code: u16 },
    Empty,
}

impl From<EventPayload> for PayloadSnapshot {
    fn from(p: EventPayload) -> Self {
        match p {
            EventPayload::Axis { axis_id, value } => PayloadSnapshot::Axis { axis_id, value },
            EventPayload::Button {
                button_id,
                pressed,
            } => PayloadSnapshot::Button {
                button_id,
                pressed,
            },
            EventPayload::Telemetry { field_id, value } => {
                PayloadSnapshot::Telemetry { field_id, value }
            }
            EventPayload::System { code } => PayloadSnapshot::System { code },
            EventPayload::Empty => PayloadSnapshot::Empty,
        }
    }
}

fn source_type_str(s: SourceType) -> String {
    match s {
        SourceType::Device => "Device".to_owned(),
        SourceType::Simulator => "Simulator".to_owned(),
        SourceType::Internal => "Internal".to_owned(),
        SourceType::Any => "Any".to_owned(),
    }
}

fn event_kind_str(k: EventKind) -> String {
    match k {
        EventKind::AxisUpdate => "AxisUpdate".to_owned(),
        EventKind::ButtonPress => "ButtonPress".to_owned(),
        EventKind::ButtonRelease => "ButtonRelease".to_owned(),
        EventKind::TelemetryFrame => "TelemetryFrame".to_owned(),
        EventKind::SystemStatus => "SystemStatus".to_owned(),
    }
}

/// Records bus events for later inspection and assertion.
#[derive(Debug, Clone)]
pub struct TraceRecorder {
    events: Vec<TimestampedEvent>,
}

impl TraceRecorder {
    /// Create a new empty recorder.
    #[must_use]
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Record a bus event with an explicit timestamp.
    pub fn record(&mut self, event: &BusEvent) {
        self.events.push(TimestampedEvent {
            timestamp_us: event.timestamp_us,
            source_type: source_type_str(event.source_type),
            kind: event_kind_str(event.kind),
            source_id: event.source_id,
            payload: event.payload.into(),
        });
    }

    /// Record a bus event with a custom timestamp override.
    pub fn record_at(&mut self, timestamp_us: u64, event: &BusEvent) {
        self.events.push(TimestampedEvent {
            timestamp_us,
            source_type: source_type_str(event.source_type),
            kind: event_kind_str(event.kind),
            source_id: event.source_id,
            payload: event.payload.into(),
        });
    }

    /// Return all recorded events.
    #[must_use]
    pub fn events(&self) -> &[TimestampedEvent] {
        &self.events
    }

    /// Return the number of recorded events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if no events have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Assert that events match the given sequence of matchers in order.
    ///
    /// Each matcher is tested against the corresponding event. Extra events
    /// beyond the matcher list are ignored.
    ///
    /// # Panics
    ///
    /// Panics if there are fewer events than matchers, or if any matcher fails.
    pub fn assert_event_sequence(&self, matchers: &[EventMatcher]) {
        assert!(
            self.events.len() >= matchers.len(),
            "expected at least {} events, got {}",
            matchers.len(),
            self.events.len()
        );
        for (i, matcher) in matchers.iter().enumerate() {
            assert!(
                matcher.matches(&self.events[i]),
                "event[{i}] did not match: expected kind={:?}, got {:?}",
                matcher.kind,
                self.events[i]
            );
        }
    }

    /// Assert that no recorded event satisfies `predicate`.
    ///
    /// # Panics
    ///
    /// Panics if any event matches the predicate.
    pub fn assert_no_event<F>(&self, predicate: F)
    where
        F: Fn(&TimestampedEvent) -> bool,
    {
        for (i, evt) in self.events.iter().enumerate() {
            assert!(
                !predicate(evt),
                "unexpected event at index {i}: {evt:?}"
            );
        }
    }

    /// Export all recorded events to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.events)
    }
}

impl Default for TraceRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// A predicate for matching a [`TimestampedEvent`] by kind and optionally payload.
#[derive(Debug, Clone)]
pub struct EventMatcher {
    /// Required event kind string (e.g. `"AxisUpdate"`).
    pub kind: String,
    /// Optional payload matcher.
    pub payload: Option<PayloadSnapshot>,
}

impl EventMatcher {
    /// Create a matcher that requires an event of the given kind.
    #[must_use]
    pub fn kind(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            payload: None,
        }
    }

    /// Additionally require a specific payload.
    #[must_use]
    pub fn with_payload(mut self, payload: PayloadSnapshot) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Check whether a timestamped event matches this matcher.
    #[must_use]
    pub fn matches(&self, event: &TimestampedEvent) -> bool {
        if event.kind != self.kind {
            return false;
        }
        if let Some(ref expected) = self.payload {
            return &event.payload == expected;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::routing::{BusEvent, EventKind, EventPayload, EventPriority, SourceType};

    fn axis_event(axis_id: u16, value: f64, ts: u64) -> BusEvent {
        BusEvent::new(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Normal,
            ts,
            EventPayload::Axis { axis_id, value },
        )
    }

    fn button_event(button_id: u16, pressed: bool, ts: u64) -> BusEvent {
        BusEvent::new(
            SourceType::Device,
            1,
            EventKind::ButtonPress,
            EventPriority::Normal,
            ts,
            EventPayload::Button {
                button_id,
                pressed,
            },
        )
    }

    #[test]
    fn record_and_retrieve() {
        let mut rec = TraceRecorder::new();
        rec.record(&axis_event(0, 0.5, 1000));
        rec.record(&button_event(1, true, 2000));
        assert_eq!(rec.len(), 2);
        assert_eq!(rec.events()[0].kind, "AxisUpdate");
        assert_eq!(rec.events()[1].kind, "ButtonPress");
    }

    #[test]
    fn record_at_overrides_timestamp() {
        let mut rec = TraceRecorder::new();
        let event = axis_event(0, 1.0, 9999);
        rec.record_at(42, &event);
        assert_eq!(rec.events()[0].timestamp_us, 42);
    }

    #[test]
    fn is_empty_and_clear() {
        let mut rec = TraceRecorder::new();
        assert!(rec.is_empty());
        rec.record(&axis_event(0, 0.0, 0));
        assert!(!rec.is_empty());
        rec.clear();
        assert!(rec.is_empty());
    }

    #[test]
    fn assert_event_sequence_passes() {
        let mut rec = TraceRecorder::new();
        rec.record(&axis_event(0, 0.5, 1000));
        rec.record(&button_event(1, true, 2000));
        rec.assert_event_sequence(&[
            EventMatcher::kind("AxisUpdate"),
            EventMatcher::kind("ButtonPress"),
        ]);
    }

    #[test]
    #[should_panic(expected = "did not match")]
    fn assert_event_sequence_fails_on_mismatch() {
        let mut rec = TraceRecorder::new();
        rec.record(&axis_event(0, 0.5, 1000));
        rec.assert_event_sequence(&[EventMatcher::kind("ButtonPress")]);
    }

    #[test]
    #[should_panic(expected = "expected at least")]
    fn assert_event_sequence_fails_on_insufficient_events() {
        let rec = TraceRecorder::new();
        rec.assert_event_sequence(&[EventMatcher::kind("AxisUpdate")]);
    }

    #[test]
    fn assert_no_event_passes_when_none_match() {
        let mut rec = TraceRecorder::new();
        rec.record(&axis_event(0, 0.5, 1000));
        rec.assert_no_event(|e| e.kind == "SystemStatus");
    }

    #[test]
    #[should_panic(expected = "unexpected event")]
    fn assert_no_event_fails_when_matched() {
        let mut rec = TraceRecorder::new();
        rec.record(&axis_event(0, 0.5, 1000));
        rec.assert_no_event(|e| e.kind == "AxisUpdate");
    }

    #[test]
    fn event_matcher_with_payload() {
        let mut rec = TraceRecorder::new();
        rec.record(&axis_event(3, 0.75, 1000));
        rec.assert_event_sequence(&[EventMatcher::kind("AxisUpdate")
            .with_payload(PayloadSnapshot::Axis {
                axis_id: 3,
                value: 0.75,
            })]);
    }

    #[test]
    fn to_json_produces_valid_json() {
        let mut rec = TraceRecorder::new();
        rec.record(&axis_event(0, 1.0, 100));
        let json = rec.to_json().unwrap();
        let parsed: Vec<TimestampedEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn default_is_empty() {
        let rec = TraceRecorder::default();
        assert!(rec.is_empty());
    }

    #[test]
    fn payload_snapshot_from_all_variants() {
        let cases: Vec<(EventPayload, PayloadSnapshot)> = vec![
            (
                EventPayload::Axis {
                    axis_id: 1,
                    value: 0.5,
                },
                PayloadSnapshot::Axis {
                    axis_id: 1,
                    value: 0.5,
                },
            ),
            (
                EventPayload::Button {
                    button_id: 2,
                    pressed: true,
                },
                PayloadSnapshot::Button {
                    button_id: 2,
                    pressed: true,
                },
            ),
            (
                EventPayload::Telemetry {
                    field_id: 3,
                    value: 99.0,
                },
                PayloadSnapshot::Telemetry {
                    field_id: 3,
                    value: 99.0,
                },
            ),
            (
                EventPayload::System { code: 42 },
                PayloadSnapshot::System { code: 42 },
            ),
            (EventPayload::Empty, PayloadSnapshot::Empty),
        ];
        for (payload, expected) in cases {
            assert_eq!(PayloadSnapshot::from(payload), expected);
        }
    }
}
