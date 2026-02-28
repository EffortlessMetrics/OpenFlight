// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Trace recording and replay engine for device input / telemetry streams.
//!
//! Provides [`TraceRecording`] for capturing timestamped events from devices and
//! simulators, [`TracePlayer`] for deterministic playback with speed control and
//! seeking, and [`TraceComparator`] for diffing two recordings with optional
//! floating-point tolerance.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::path::Path;

/// Source of a trace event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceSource {
    Device,
    Simulator,
}

impl fmt::Display for TraceSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Device => write!(f, "Device"),
            Self::Simulator => write!(f, "Simulator"),
        }
    }
}

/// Type of event captured in a trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceEventType {
    AxisInput,
    ButtonPress,
    ButtonRelease,
    TelemetryUpdate,
    SimEvent,
}

impl fmt::Display for TraceEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AxisInput => write!(f, "AxisInput"),
            Self::ButtonPress => write!(f, "ButtonPress"),
            Self::ButtonRelease => write!(f, "ButtonRelease"),
            Self::TelemetryUpdate => write!(f, "TelemetryUpdate"),
            Self::SimEvent => write!(f, "SimEvent"),
        }
    }
}

/// A single timestamped event in a trace recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Microseconds relative to the start of the recording.
    pub timestamp_us: u64,
    pub event_type: TraceEventType,
    pub source: TraceSource,
    pub data: Vec<f64>,
}

/// A named collection of trace events captured from a device or simulator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecording {
    pub name: String,
    pub events: Vec<TraceEvent>,
    pub duration_us: u64,
    pub device_id: Option<String>,
}

impl TraceRecording {
    /// Create a new empty recording with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            events: Vec::new(),
            duration_us: 0,
            device_id: None,
        }
    }

    /// Append an event and update the recording duration.
    pub fn add_event(&mut self, event: TraceEvent) {
        if event.timestamp_us > self.duration_us {
            self.duration_us = event.timestamp_us;
        }
        self.events.push(event);
    }

    /// Record an event — convenience alias for [`add_event`](Self::add_event).
    pub fn record_event(&mut self, event: TraceEvent) {
        self.add_event(event);
    }

    /// Total duration in microseconds based on the latest event timestamp.
    pub fn duration(&self) -> u64 {
        self.duration_us
    }

    /// Number of events in the recording.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Return all events matching the given type.
    pub fn events_of_type(&self, event_type: TraceEventType) -> Vec<&TraceEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .collect()
    }

    /// Serialize the recording to a JSON file.
    pub fn save_to_file(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Deserialize a recording from a JSON file.
    pub fn load_from_file(path: &Path) -> io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

/// An event paired with its computed playback delay.
#[derive(Debug, Clone)]
pub struct TimedEvent<'a> {
    pub event: &'a TraceEvent,
    /// Delay in microseconds since the previous event (adjusted for playback speed).
    pub delay_us: u64,
}

/// Iterator that yields [`TimedEvent`]s from a recording slice.
pub struct TracePlayIterator<'a> {
    events: &'a [TraceEvent],
    position: usize,
    speed: f64,
    last_timestamp_us: u64,
}

impl<'a> Iterator for TracePlayIterator<'a> {
    type Item = TimedEvent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.events.len() {
            return None;
        }
        let event = &self.events[self.position];
        let raw_delta = event.timestamp_us.saturating_sub(self.last_timestamp_us);
        let delay_us = if self.speed > 0.0 {
            (raw_delta as f64 / self.speed) as u64
        } else {
            0
        };
        self.last_timestamp_us = event.timestamp_us;
        self.position += 1;
        Some(TimedEvent { event, delay_us })
    }
}

/// Plays back a [`TraceRecording`] with time-based seeking and speed control.
#[derive(Debug)]
pub struct TracePlayer {
    recording: TraceRecording,
    position: usize,
    playback_speed: f64,
    current_time_us: u64,
}

impl TracePlayer {
    /// Create a new player positioned at the start of the recording.
    pub fn new(recording: TraceRecording) -> Self {
        Self {
            recording,
            position: 0,
            playback_speed: 1.0,
            current_time_us: 0,
        }
    }

    /// Advance playback to `timestamp_us` (scaled by playback speed) and return
    /// all events between the current position and the target time.
    pub fn advance_to(&mut self, timestamp_us: u64) -> Vec<&TraceEvent> {
        let effective_time = (timestamp_us as f64 * self.playback_speed) as u64;
        self.current_time_us = effective_time;

        let mut emitted = Vec::new();
        while self.position < self.recording.events.len() {
            let evt = &self.recording.events[self.position];
            if evt.timestamp_us <= effective_time {
                emitted.push(evt);
                self.position += 1;
            } else {
                break;
            }
        }
        emitted
    }

    /// Return an iterator over remaining events at the current playback speed.
    ///
    /// The iterator is a snapshot view — it does **not** advance the player's
    /// internal position.
    pub fn play(&self) -> TracePlayIterator<'_> {
        TracePlayIterator {
            events: &self.recording.events[self.position..],
            position: 0,
            speed: self.playback_speed,
            last_timestamp_us: self.current_time_us,
        }
    }

    /// Return an iterator over remaining events at the given speed `multiplier`.
    pub fn play_at_speed(&self, multiplier: f64) -> TracePlayIterator<'_> {
        TracePlayIterator {
            events: &self.recording.events[self.position..],
            position: 0,
            speed: multiplier,
            last_timestamp_us: self.current_time_us,
        }
    }

    /// Jump to the first event at or after `timestamp_us`.
    pub fn seek_to(&mut self, timestamp_us: u64) {
        self.position = self
            .recording
            .events
            .iter()
            .position(|e| e.timestamp_us >= timestamp_us)
            .unwrap_or(self.recording.events.len());
        self.current_time_us = timestamp_us;
    }

    /// Replay all remaining events, calling `callback` for each one.
    ///
    /// The callback receives `(event, delay_us)` where `delay_us` is the
    /// speed-adjusted gap since the previous event.  After this call the
    /// player is complete.
    pub fn with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&TraceEvent, u64),
    {
        let mut last_ts = self.current_time_us;
        while self.position < self.recording.events.len() {
            let event = &self.recording.events[self.position];
            let raw_delta = event.timestamp_us.saturating_sub(last_ts);
            let delay = if self.playback_speed > 0.0 {
                (raw_delta as f64 / self.playback_speed) as u64
            } else {
                0
            };
            callback(event, delay);
            last_ts = event.timestamp_us;
            self.position += 1;
        }
        self.current_time_us = self.recording.duration_us;
    }

    /// Reset to the beginning of the recording.
    pub fn reset(&mut self) {
        self.position = 0;
        self.current_time_us = 0;
    }

    /// `true` when all events have been consumed.
    pub fn is_complete(&self) -> bool {
        self.position >= self.recording.events.len()
    }

    /// Set the playback speed multiplier (e.g. 2.0 = double speed).
    pub fn set_speed(&mut self, multiplier: f64) {
        self.playback_speed = multiplier;
    }

    /// Number of events remaining to be played.
    pub fn remaining_events(&self) -> usize {
        self.recording.events.len().saturating_sub(self.position)
    }
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

/// A single mismatch found when comparing two trace recordings.
#[derive(Debug, Clone)]
pub enum TraceMismatch {
    /// Event types differ at the given index.
    TypeMismatch {
        index: usize,
        expected: TraceEventType,
        actual: TraceEventType,
    },
    /// Event sources differ at the given index.
    SourceMismatch {
        index: usize,
        expected: TraceSource,
        actual: TraceSource,
    },
    /// Event timestamps differ at the given index.
    TimingMismatch {
        index: usize,
        expected_us: u64,
        actual_us: u64,
    },
    /// Event data values differ beyond tolerance.
    DataMismatch {
        index: usize,
        expected: Vec<f64>,
        actual: Vec<f64>,
        max_diff: f64,
    },
    /// Data vectors have different lengths.
    DataLengthMismatch {
        index: usize,
        expected_len: usize,
        actual_len: usize,
    },
}

/// Result of comparing two trace recordings.
#[derive(Debug, Clone)]
pub struct TraceDiff {
    pub mismatches: Vec<TraceMismatch>,
    /// Number of events present in expected but missing from actual.
    pub missing_events: usize,
    /// Number of extra events in actual beyond expected.
    pub extra_events: usize,
}

impl TraceDiff {
    /// `true` when the traces are considered equivalent.
    pub fn is_match(&self) -> bool {
        self.mismatches.is_empty() && self.missing_events == 0 && self.extra_events == 0
    }

    /// Produce a human-readable diff report.
    pub fn report(&self) -> String {
        if self.is_match() {
            return "Traces match.".to_owned();
        }
        let mut lines = Vec::new();
        if self.missing_events > 0 {
            lines.push(format!("Missing events: {}", self.missing_events));
        }
        if self.extra_events > 0 {
            lines.push(format!("Extra events: {}", self.extra_events));
        }
        for m in &self.mismatches {
            match m {
                TraceMismatch::TypeMismatch {
                    index,
                    expected,
                    actual,
                } => {
                    lines.push(format!("[{index}] type: expected {expected}, got {actual}"));
                }
                TraceMismatch::SourceMismatch {
                    index,
                    expected,
                    actual,
                } => {
                    lines.push(format!(
                        "[{index}] source: expected {expected}, got {actual}"
                    ));
                }
                TraceMismatch::TimingMismatch {
                    index,
                    expected_us,
                    actual_us,
                } => {
                    lines.push(format!(
                        "[{index}] timing: expected {expected_us}\u{00b5}s, got {actual_us}\u{00b5}s"
                    ));
                }
                TraceMismatch::DataMismatch {
                    index,
                    expected,
                    actual,
                    max_diff,
                } => {
                    lines.push(format!(
                        "[{index}] data: expected {expected:?}, got {actual:?} (max diff: {max_diff:.6})"
                    ));
                }
                TraceMismatch::DataLengthMismatch {
                    index,
                    expected_len,
                    actual_len,
                } => {
                    lines.push(format!(
                        "[{index}] data length: expected {expected_len}, got {actual_len}"
                    ));
                }
            }
        }
        lines.join("\n")
    }
}

/// Compares two [`TraceRecording`]s, optionally with tolerance for
/// floating-point data values.
pub struct TraceComparator {
    tolerance: f64,
}

impl TraceComparator {
    /// Create a comparator that requires exact data matches.
    pub fn new() -> Self {
        Self { tolerance: 0.0 }
    }

    /// Create a comparator with the given tolerance for floating-point comparisons.
    pub fn within_tolerance(tolerance: f64) -> Self {
        Self { tolerance }
    }

    /// Compare two recordings and return the diff.
    pub fn compare(&self, expected: &TraceRecording, actual: &TraceRecording) -> TraceDiff {
        let common_len = expected.events.len().min(actual.events.len());
        let mut mismatches = Vec::new();

        for i in 0..common_len {
            let exp = &expected.events[i];
            let act = &actual.events[i];

            if exp.event_type != act.event_type {
                mismatches.push(TraceMismatch::TypeMismatch {
                    index: i,
                    expected: exp.event_type,
                    actual: act.event_type,
                });
            }

            if exp.source != act.source {
                mismatches.push(TraceMismatch::SourceMismatch {
                    index: i,
                    expected: exp.source,
                    actual: act.source,
                });
            }

            if exp.timestamp_us != act.timestamp_us {
                mismatches.push(TraceMismatch::TimingMismatch {
                    index: i,
                    expected_us: exp.timestamp_us,
                    actual_us: act.timestamp_us,
                });
            }

            if exp.data.len() != act.data.len() {
                mismatches.push(TraceMismatch::DataLengthMismatch {
                    index: i,
                    expected_len: exp.data.len(),
                    actual_len: act.data.len(),
                });
            } else {
                let max_diff = exp
                    .data
                    .iter()
                    .zip(act.data.iter())
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f64, f64::max);
                if max_diff > self.tolerance {
                    mismatches.push(TraceMismatch::DataMismatch {
                        index: i,
                        expected: exp.data.clone(),
                        actual: act.data.clone(),
                        max_diff,
                    });
                }
            }
        }

        let missing_events = expected.events.len().saturating_sub(actual.events.len());
        let extra_events = actual.events.len().saturating_sub(expected.events.len());

        TraceDiff {
            mismatches,
            missing_events,
            extra_events,
        }
    }
}

impl Default for TraceComparator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recording() -> TraceRecording {
        let mut rec = TraceRecording::new("test_flight");
        rec.add_event(TraceEvent {
            timestamp_us: 1000,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![0.5],
        });
        rec.add_event(TraceEvent {
            timestamp_us: 2000,
            event_type: TraceEventType::ButtonPress,
            source: TraceSource::Device,
            data: vec![1.0],
        });
        rec.add_event(TraceEvent {
            timestamp_us: 3000,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![0.75],
        });
        rec.add_event(TraceEvent {
            timestamp_us: 5000,
            event_type: TraceEventType::TelemetryUpdate,
            source: TraceSource::Simulator,
            data: vec![100.0, 200.0],
        });
        rec
    }

    // ---- Recording basics ------------------------------------------------

    #[test]
    fn recording_new_is_empty() {
        let rec = TraceRecording::new("empty");
        assert_eq!(rec.event_count(), 0);
        assert_eq!(rec.duration(), 0);
    }

    #[test]
    fn recording_tracks_duration() {
        let rec = sample_recording();
        assert_eq!(rec.duration(), 5000);
        assert_eq!(rec.event_count(), 4);
    }

    #[test]
    fn recording_device_id() {
        let mut rec = TraceRecording::new("dev_test");
        rec.device_id = Some("X52-001".to_owned());
        assert_eq!(rec.device_id.as_deref(), Some("X52-001"));
    }

    #[test]
    fn record_event_alias() {
        let mut rec = TraceRecording::new("alias");
        rec.record_event(TraceEvent {
            timestamp_us: 500,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![0.1],
        });
        assert_eq!(rec.event_count(), 1);
        assert_eq!(rec.duration(), 500);
    }

    #[test]
    fn events_of_type_filters_correctly() {
        let rec = sample_recording();
        let axis = rec.events_of_type(TraceEventType::AxisInput);
        assert_eq!(axis.len(), 2);
        assert!(
            axis.iter()
                .all(|e| e.event_type == TraceEventType::AxisInput)
        );

        let telem = rec.events_of_type(TraceEventType::TelemetryUpdate);
        assert_eq!(telem.len(), 1);

        let sim = rec.events_of_type(TraceEventType::SimEvent);
        assert!(sim.is_empty());
    }

    // ---- Player: advance_to (existing) -----------------------------------

    #[test]
    fn player_advance_returns_events_up_to_time() {
        let mut player = TracePlayer::new(sample_recording());
        let events = player.advance_to(2000);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].timestamp_us, 1000);
        assert_eq!(events[1].timestamp_us, 2000);
    }

    #[test]
    fn player_advance_incremental() {
        let mut player = TracePlayer::new(sample_recording());
        let first = player.advance_to(1500);
        assert_eq!(first.len(), 1);
        let second = player.advance_to(3000);
        assert_eq!(second.len(), 2);
    }

    #[test]
    fn player_is_complete_after_all_events() {
        let mut player = TracePlayer::new(sample_recording());
        assert!(!player.is_complete());
        player.advance_to(10_000);
        assert!(player.is_complete());
    }

    #[test]
    fn player_remaining_events() {
        let mut player = TracePlayer::new(sample_recording());
        assert_eq!(player.remaining_events(), 4);
        player.advance_to(2000);
        assert_eq!(player.remaining_events(), 2);
    }

    #[test]
    fn player_reset() {
        let mut player = TracePlayer::new(sample_recording());
        player.advance_to(5000);
        assert!(player.is_complete());
        player.reset();
        assert!(!player.is_complete());
        assert_eq!(player.remaining_events(), 4);
    }

    #[test]
    fn player_double_speed() {
        let mut player = TracePlayer::new(sample_recording());
        player.set_speed(2.0);
        // Advancing to 1500 at 2x means effective time = 3000
        let events = player.advance_to(1500);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn player_half_speed() {
        let mut player = TracePlayer::new(sample_recording());
        player.set_speed(0.5);
        // Advancing to 4000 at 0.5x means effective time = 2000
        let events = player.advance_to(4000);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn player_advance_to_zero_returns_nothing() {
        let mut player = TracePlayer::new(sample_recording());
        let events = player.advance_to(0);
        assert!(events.is_empty());
    }

    // ---- Player: play / play_at_speed ------------------------------------

    #[test]
    fn play_yields_all_events_with_delays() {
        let player = TracePlayer::new(sample_recording());
        let timed: Vec<_> = player.play().collect();
        assert_eq!(timed.len(), 4);
        // First event delay is from time 0 → 1000
        assert_eq!(timed[0].delay_us, 1000);
        // Second: 1000 → 2000
        assert_eq!(timed[1].delay_us, 1000);
        // Third: 2000 → 3000
        assert_eq!(timed[2].delay_us, 1000);
        // Fourth: 3000 → 5000
        assert_eq!(timed[3].delay_us, 2000);
    }

    #[test]
    fn play_after_seek_yields_remaining() {
        let mut player = TracePlayer::new(sample_recording());
        player.seek_to(3000);
        let timed: Vec<_> = player.play().collect();
        assert_eq!(timed.len(), 2);
        assert_eq!(timed[0].event.timestamp_us, 3000);
        assert_eq!(timed[1].event.timestamp_us, 5000);
    }

    #[test]
    fn play_at_speed_adjusts_delays() {
        let player = TracePlayer::new(sample_recording());
        let timed: Vec<_> = player.play_at_speed(2.0).collect();
        assert_eq!(timed.len(), 4);
        // At 2x speed, delays are halved.
        assert_eq!(timed[0].delay_us, 500);
        assert_eq!(timed[1].delay_us, 500);
        assert_eq!(timed[2].delay_us, 500);
        assert_eq!(timed[3].delay_us, 1000);
    }

    #[test]
    fn play_at_half_speed_doubles_delays() {
        let player = TracePlayer::new(sample_recording());
        let timed: Vec<_> = player.play_at_speed(0.5).collect();
        assert_eq!(timed[0].delay_us, 2000);
        assert_eq!(timed[1].delay_us, 2000);
    }

    // ---- Player: seek_to -------------------------------------------------

    #[test]
    fn seek_to_middle() {
        let mut player = TracePlayer::new(sample_recording());
        player.seek_to(2500);
        assert_eq!(player.remaining_events(), 2);
        let events = player.advance_to(5000);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].timestamp_us, 3000);
    }

    #[test]
    fn seek_to_exact_event_time() {
        let mut player = TracePlayer::new(sample_recording());
        player.seek_to(2000);
        assert_eq!(player.remaining_events(), 3);
    }

    #[test]
    fn seek_past_end() {
        let mut player = TracePlayer::new(sample_recording());
        player.seek_to(99_999);
        assert!(player.is_complete());
        assert_eq!(player.remaining_events(), 0);
    }

    #[test]
    fn seek_and_resume_play() {
        let mut player = TracePlayer::new(sample_recording());
        player.seek_to(3000);
        let timed: Vec<_> = player.play().collect();
        assert_eq!(timed.len(), 2);
        // Delay of first event after seek: 3000 - 3000 = 0
        assert_eq!(timed[0].delay_us, 0);
        // Second: 5000 - 3000 = 2000
        assert_eq!(timed[1].delay_us, 2000);
    }

    // ---- Player: with_callback -------------------------------------------

    #[test]
    fn with_callback_visits_all_events() {
        let mut player = TracePlayer::new(sample_recording());
        let mut visited = Vec::new();
        player.with_callback(|evt, delay| {
            visited.push((evt.timestamp_us, delay));
        });
        assert!(player.is_complete());
        assert_eq!(visited.len(), 4);
        assert_eq!(visited[0], (1000, 1000));
        assert_eq!(visited[1], (2000, 1000));
        assert_eq!(visited[2], (3000, 1000));
        assert_eq!(visited[3], (5000, 2000));
    }

    #[test]
    fn with_callback_respects_speed() {
        let mut player = TracePlayer::new(sample_recording());
        player.set_speed(2.0);
        let mut delays = Vec::new();
        player.with_callback(|_, delay| {
            delays.push(delay);
        });
        assert_eq!(delays, vec![500, 500, 500, 1000]);
    }

    #[test]
    fn with_callback_after_seek() {
        let mut player = TracePlayer::new(sample_recording());
        player.seek_to(3000);
        let mut count = 0;
        player.with_callback(|_, _| count += 1);
        assert_eq!(count, 2);
    }

    // ---- JSON serialization round-trip -----------------------------------

    #[test]
    fn json_round_trip_in_memory() {
        let rec = sample_recording();
        let json = serde_json::to_string(&rec).unwrap();
        let loaded: TraceRecording = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, rec.name);
        assert_eq!(loaded.events.len(), rec.events.len());
        assert_eq!(loaded.duration_us, rec.duration_us);
        for (a, b) in loaded.events.iter().zip(rec.events.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn save_and_load_file_round_trip() {
        let rec = sample_recording();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace.json");
        rec.save_to_file(&path).unwrap();
        let loaded = TraceRecording::load_from_file(&path).unwrap();
        assert_eq!(loaded.name, rec.name);
        assert_eq!(loaded.event_count(), rec.event_count());
        assert_eq!(loaded.duration(), rec.duration());
        for (a, b) in loaded.events.iter().zip(rec.events.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn load_from_missing_file_returns_error() {
        let result = TraceRecording::load_from_file(Path::new("nonexistent_file.json"));
        assert!(result.is_err());
    }

    // ---- TraceComparator -------------------------------------------------

    #[test]
    fn compare_identical_traces() {
        let a = sample_recording();
        let b = sample_recording();
        let diff = TraceComparator::new().compare(&a, &b);
        assert!(diff.is_match());
        assert_eq!(diff.report(), "Traces match.");
    }

    #[test]
    fn compare_detects_type_mismatch() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[0].event_type = TraceEventType::SimEvent;

        let diff = TraceComparator::new().compare(&a, &b);
        assert!(!diff.is_match());
        assert!(matches!(
            diff.mismatches[0],
            TraceMismatch::TypeMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn compare_detects_source_mismatch() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[0].source = TraceSource::Simulator;

        let diff = TraceComparator::new().compare(&a, &b);
        assert!(!diff.is_match());
        assert!(matches!(
            diff.mismatches[0],
            TraceMismatch::SourceMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn compare_detects_timing_mismatch() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[1].timestamp_us = 9999;

        let diff = TraceComparator::new().compare(&a, &b);
        assert!(!diff.is_match());
        assert!(matches!(
            diff.mismatches[0],
            TraceMismatch::TimingMismatch { index: 1, .. }
        ));
    }

    #[test]
    fn compare_detects_data_mismatch_exact() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[0].data = vec![0.6];

        let diff = TraceComparator::new().compare(&a, &b);
        assert!(!diff.is_match());
        assert!(matches!(
            diff.mismatches[0],
            TraceMismatch::DataMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn compare_within_tolerance_passes() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[0].data = vec![0.509]; // diff = 0.009

        let diff = TraceComparator::within_tolerance(0.01).compare(&a, &b);
        assert!(diff.is_match());
    }

    #[test]
    fn compare_within_tolerance_fails() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[0].data = vec![0.6]; // diff = 0.1

        let diff = TraceComparator::within_tolerance(0.01).compare(&a, &b);
        assert!(!diff.is_match());
    }

    #[test]
    fn compare_detects_data_length_mismatch() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[3].data = vec![100.0]; // was [100.0, 200.0]

        let diff = TraceComparator::new().compare(&a, &b);
        assert!(!diff.is_match());
        assert!(matches!(
            diff.mismatches[0],
            TraceMismatch::DataLengthMismatch { index: 3, .. }
        ));
    }

    #[test]
    fn compare_detects_missing_events() {
        let a = sample_recording(); // 4 events
        let mut b = TraceRecording::new("short");
        b.add_event(a.events[0].clone());
        b.add_event(a.events[1].clone());

        let diff = TraceComparator::new().compare(&a, &b);
        assert!(!diff.is_match());
        assert_eq!(diff.missing_events, 2);
        assert_eq!(diff.extra_events, 0);
    }

    #[test]
    fn compare_detects_extra_events() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.add_event(TraceEvent {
            timestamp_us: 6000,
            event_type: TraceEventType::SimEvent,
            source: TraceSource::Simulator,
            data: vec![42.0],
        });

        let diff = TraceComparator::new().compare(&a, &b);
        assert!(!diff.is_match());
        assert_eq!(diff.extra_events, 1);
        assert_eq!(diff.missing_events, 0);
    }

    #[test]
    fn compare_report_contains_details() {
        let a = sample_recording();
        let mut b = sample_recording();
        b.events[0].event_type = TraceEventType::SimEvent;
        b.events[2].data = vec![0.99];

        let diff = TraceComparator::new().compare(&a, &b);
        let report = diff.report();
        assert!(report.contains("[0] type:"));
        assert!(report.contains("[2] data:"));
    }

    // ---- Edge cases ------------------------------------------------------

    #[test]
    fn empty_trace_round_trip() {
        let rec = TraceRecording::new("empty");
        assert_eq!(rec.event_count(), 0);
        assert_eq!(rec.duration(), 0);
        assert!(rec.events_of_type(TraceEventType::AxisInput).is_empty());

        let json = serde_json::to_string(&rec).unwrap();
        let loaded: TraceRecording = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.event_count(), 0);
    }

    #[test]
    fn single_event_trace() {
        let mut rec = TraceRecording::new("single");
        rec.record_event(TraceEvent {
            timestamp_us: 42,
            event_type: TraceEventType::ButtonRelease,
            source: TraceSource::Device,
            data: vec![0.0],
        });
        assert_eq!(rec.event_count(), 1);
        assert_eq!(rec.duration(), 42);

        let player = TracePlayer::new(rec);
        let timed: Vec<_> = player.play().collect();
        assert_eq!(timed.len(), 1);
        assert_eq!(timed[0].delay_us, 42);
    }

    #[test]
    fn empty_trace_player() {
        let rec = TraceRecording::new("empty");
        let player = TracePlayer::new(rec);
        assert!(player.is_complete());
        assert_eq!(player.remaining_events(), 0);
        let timed: Vec<_> = player.play().collect();
        assert!(timed.is_empty());
    }

    #[test]
    fn compare_two_empty_traces() {
        let a = TraceRecording::new("a");
        let b = TraceRecording::new("b");
        let diff = TraceComparator::new().compare(&a, &b);
        assert!(diff.is_match());
    }

    #[test]
    fn event_type_variants_distinct() {
        assert_ne!(TraceEventType::AxisInput, TraceEventType::ButtonPress);
        assert_ne!(TraceEventType::ButtonPress, TraceEventType::ButtonRelease);
        assert_ne!(TraceEventType::TelemetryUpdate, TraceEventType::SimEvent);
    }

    #[test]
    fn trace_source_display() {
        assert_eq!(format!("{}", TraceSource::Device), "Device");
        assert_eq!(format!("{}", TraceSource::Simulator), "Simulator");
    }

    #[test]
    fn trace_event_type_display() {
        assert_eq!(format!("{}", TraceEventType::AxisInput), "AxisInput");
        assert_eq!(format!("{}", TraceEventType::SimEvent), "SimEvent");
    }

    #[test]
    fn record_and_replay_full_round_trip() {
        // Record
        let mut rec = TraceRecording::new("round_trip");
        rec.device_id = Some("HOTAS-01".to_owned());
        for i in 0..10 {
            rec.record_event(TraceEvent {
                timestamp_us: i * 4000,
                event_type: TraceEventType::AxisInput,
                source: TraceSource::Device,
                data: vec![i as f64 / 10.0],
            });
        }

        // Save & load
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("round_trip.json");
        rec.save_to_file(&path).unwrap();
        let loaded = TraceRecording::load_from_file(&path).unwrap();

        // Compare
        let diff = TraceComparator::new().compare(&rec, &loaded);
        assert!(diff.is_match());

        // Replay
        let mut player = TracePlayer::new(loaded);
        let mut collected = Vec::new();
        player.with_callback(|evt, _| collected.push(evt.data[0]));
        assert!(player.is_complete());
        assert_eq!(collected.len(), 10);
        assert!((collected[5] - 0.5).abs() < f64::EPSILON);
    }
}
