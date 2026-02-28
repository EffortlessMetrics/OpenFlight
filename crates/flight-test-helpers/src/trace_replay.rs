// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Trace recording and replay engine for device input / telemetry streams.

/// Type of event captured in a trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventType {
    AxisInput,
    ButtonPress,
    ButtonRelease,
    TelemetryUpdate,
    SimEvent,
}

/// A single timestamped event in a trace recording.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceEvent {
    pub timestamp_us: u64,
    pub event_type: TraceEventType,
    pub data: Vec<f64>,
}

/// A named collection of trace events captured from a device.
#[derive(Debug, Clone)]
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

    /// Total duration in microseconds based on the latest event timestamp.
    pub fn duration(&self) -> u64 {
        self.duration_us
    }

    /// Number of events in the recording.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

/// Plays back a `TraceRecording` with time-based seeking and speed control.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recording() -> TraceRecording {
        let mut rec = TraceRecording::new("test_flight");
        rec.add_event(TraceEvent {
            timestamp_us: 1000,
            event_type: TraceEventType::AxisInput,
            data: vec![0.5],
        });
        rec.add_event(TraceEvent {
            timestamp_us: 2000,
            event_type: TraceEventType::ButtonPress,
            data: vec![1.0],
        });
        rec.add_event(TraceEvent {
            timestamp_us: 3000,
            event_type: TraceEventType::AxisInput,
            data: vec![0.75],
        });
        rec.add_event(TraceEvent {
            timestamp_us: 5000,
            event_type: TraceEventType::TelemetryUpdate,
            data: vec![100.0, 200.0],
        });
        rec
    }

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
    fn recording_device_id() {
        let mut rec = TraceRecording::new("dev_test");
        rec.device_id = Some("X52-001".to_owned());
        assert_eq!(rec.device_id.as_deref(), Some("X52-001"));
    }

    #[test]
    fn player_advance_to_zero_returns_nothing() {
        let mut player = TracePlayer::new(sample_recording());
        let events = player.advance_to(0);
        assert!(events.is_empty());
    }

    #[test]
    fn event_type_variants_distinct() {
        assert_ne!(TraceEventType::AxisInput, TraceEventType::ButtonPress);
        assert_ne!(TraceEventType::ButtonPress, TraceEventType::ButtonRelease);
        assert_ne!(TraceEventType::TelemetryUpdate, TraceEventType::SimEvent);
    }
}
