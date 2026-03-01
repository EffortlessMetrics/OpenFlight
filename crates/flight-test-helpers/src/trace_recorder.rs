// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Lightweight trace recorder for capturing and replaying timestamped events.
//!
//! Unlike the full [`super::trace_replay`] module which targets device/sim trace
//! streams with diffing and speed control, this module provides a simpler
//! record-then-replay primitive suited for golden-test workflows.

use serde::{Deserialize, Serialize};

/// A single recorded trace entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEntry {
    /// Timestamp in microseconds relative to recorder start.
    pub timestamp_us: u64,
    /// A short tag identifying the event kind (e.g. "axis", "button", "connect").
    pub tag: String,
    /// Arbitrary payload values.
    pub values: Vec<f64>,
}

/// Records timestamped events and supports replay and serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecorder {
    name: String,
    entries: Vec<TraceEntry>,
}

impl TraceRecorder {
    /// Create a new empty recorder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
        }
    }

    /// Record a new event.
    pub fn record(&mut self, timestamp_us: u64, tag: impl Into<String>, values: Vec<f64>) {
        self.entries.push(TraceEntry {
            timestamp_us,
            tag: tag.into(),
            values,
        });
    }

    /// Return the recorder name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return all recorded entries.
    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    /// Return the number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if no entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the duration of the recording (last timestamp minus first, or 0).
    pub fn duration_us(&self) -> u64 {
        match (self.entries.first(), self.entries.last()) {
            (Some(first), Some(last)) => last.timestamp_us.saturating_sub(first.timestamp_us),
            _ => 0,
        }
    }

    /// Filter entries by tag.
    pub fn entries_with_tag(&self, tag: &str) -> Vec<&TraceEntry> {
        self.entries.iter().filter(|e| e.tag == tag).collect()
    }

    /// Create a replay iterator over the recorded entries.
    pub fn replay(&self) -> TraceReplayIter<'_> {
        TraceReplayIter {
            entries: &self.entries,
            position: 0,
        }
    }

    /// Serialize the recording to a JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a recording from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Iterator for replaying a recorded trace.
#[derive(Debug)]
pub struct TraceReplayIter<'a> {
    entries: &'a [TraceEntry],
    position: usize,
}

impl<'a> Iterator for TraceReplayIter<'a> {
    type Item = &'a TraceEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.entries.len() {
            let entry = &self.entries[self.position];
            self.position += 1;
            Some(entry)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.entries.len() - self.position;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for TraceReplayIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_recorder_is_empty() {
        let rec = TraceRecorder::new("test");
        assert!(rec.is_empty());
        assert_eq!(rec.len(), 0);
        assert_eq!(rec.name(), "test");
        assert_eq!(rec.duration_us(), 0);
    }

    #[test]
    fn record_and_retrieve() {
        let mut rec = TraceRecorder::new("axes");
        rec.record(0, "axis", vec![0.0, 0.0]);
        rec.record(4000, "axis", vec![0.5, -0.5]);
        rec.record(8000, "button", vec![1.0]);

        assert_eq!(rec.len(), 3);
        assert!(!rec.is_empty());
        assert_eq!(rec.entries()[0].tag, "axis");
        assert_eq!(rec.entries()[2].tag, "button");
    }

    #[test]
    fn duration_calculation() {
        let mut rec = TraceRecorder::new("dur");
        rec.record(1000, "a", vec![]);
        rec.record(5000, "b", vec![]);
        rec.record(10_000, "c", vec![]);
        assert_eq!(rec.duration_us(), 9000);
    }

    #[test]
    fn entries_with_tag_filter() {
        let mut rec = TraceRecorder::new("filter");
        rec.record(0, "axis", vec![0.0]);
        rec.record(1000, "button", vec![1.0]);
        rec.record(2000, "axis", vec![0.5]);

        let axes = rec.entries_with_tag("axis");
        assert_eq!(axes.len(), 2);
        let buttons = rec.entries_with_tag("button");
        assert_eq!(buttons.len(), 1);
        let empty = rec.entries_with_tag("connect");
        assert!(empty.is_empty());
    }

    #[test]
    fn replay_iterator() {
        let mut rec = TraceRecorder::new("replay");
        rec.record(0, "a", vec![1.0]);
        rec.record(100, "b", vec![2.0]);
        rec.record(200, "c", vec![3.0]);

        let mut iter = rec.replay();
        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next().unwrap().tag, "a");
        assert_eq!(iter.next().unwrap().tag, "b");
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next().unwrap().tag, "c");
        assert!(iter.next().is_none());
    }

    #[test]
    fn json_round_trip() {
        let mut rec = TraceRecorder::new("golden");
        rec.record(0, "axis", vec![0.0, 0.5]);
        rec.record(4000, "button", vec![1.0]);

        let json = rec.to_json().unwrap();
        let restored = TraceRecorder::from_json(&json).unwrap();

        assert_eq!(restored.name(), "golden");
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.entries()[0].tag, "axis");
        assert!((restored.entries()[0].values[1] - 0.5).abs() < f64::EPSILON);
        assert_eq!(restored.entries()[1].timestamp_us, 4000);
    }

    #[test]
    fn json_deserialization_preserves_values() {
        let json = r#"{
            "name": "test",
            "entries": [
                {"timestamp_us": 0, "tag": "x", "values": [1.0, 2.0, 3.0]}
            ]
        }"#;
        let rec = TraceRecorder::from_json(json).unwrap();
        assert_eq!(rec.entries()[0].values, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn clear_removes_entries() {
        let mut rec = TraceRecorder::new("clear");
        rec.record(0, "a", vec![]);
        rec.record(1000, "b", vec![]);
        assert_eq!(rec.len(), 2);
        rec.clear();
        assert!(rec.is_empty());
        assert_eq!(rec.duration_us(), 0);
    }

    #[test]
    fn trace_entry_clone_and_eq() {
        let entry = TraceEntry {
            timestamp_us: 100,
            tag: "axis".to_owned(),
            values: vec![0.5],
        };
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
    }

    #[test]
    fn empty_replay_iterator() {
        let rec = TraceRecorder::new("empty");
        let mut iter = rec.replay();
        assert_eq!(iter.len(), 0);
        assert!(iter.next().is_none());
    }

    #[test]
    fn replay_collects_all() {
        let mut rec = TraceRecorder::new("collect");
        for i in 0..5 {
            rec.record(i * 1000, "tick", vec![i as f64]);
        }
        let replayed: Vec<_> = rec.replay().collect();
        assert_eq!(replayed.len(), 5);
        assert!((replayed[4].values[0] - 4.0).abs() < f64::EPSILON);
    }
}
