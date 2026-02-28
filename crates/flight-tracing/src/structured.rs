// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Structured flight events with typed context, fluent builder, and pluggable sinks.
//!
//! [`FlightEvent`] captures a timestamped, levelled log entry with domain-specific
//! [`EventContext`] fields (device, sim, axis, profile). Events are constructed via
//! [`EventBuilder`] and dispatched to one or more [`EventSink`] implementations.
//!
//! Two built-in sinks are provided:
//! - [`MemorySink`] — bounded ring buffer for live diagnostics
//! - [`FileSink`] — append-only JSON-lines writer with size-based rotation

use crate::log_rotation::{LogRotator, RotationConfig, RotationResult};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Core types ────────────────────────────────────────────────────────────

/// Severity level for a [`FlightEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for EventLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trace => f.write_str("TRACE"),
            Self::Debug => f.write_str("DEBUG"),
            Self::Info => f.write_str("INFO"),
            Self::Warn => f.write_str("WARN"),
            Self::Error => f.write_str("ERROR"),
        }
    }
}

/// Domain-specific context attached to a [`FlightEvent`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sim_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
}

/// A structured log event with common fields and typed context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightEvent {
    /// Nanoseconds since Unix epoch.
    pub timestamp_ns: u64,
    pub level: EventLevel,
    pub component: String,
    pub message: String,
    pub context: EventContext,
}

impl FlightEvent {
    /// Serialize this event as a single-line JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

// ── EventBuilder ──────────────────────────────────────────────────────────

/// Fluent builder for constructing [`FlightEvent`] instances.
pub struct EventBuilder {
    level: EventLevel,
    component: String,
    message: String,
    context: EventContext,
}

impl EventBuilder {
    /// Start building an event with the required triple.
    pub fn new(
        level: EventLevel,
        component: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            level,
            component: component.into(),
            message: message.into(),
            context: EventContext::default(),
        }
    }

    pub fn device_id(mut self, id: impl Into<String>) -> Self {
        self.context.device_id = Some(id.into());
        self
    }

    pub fn sim_name(mut self, name: impl Into<String>) -> Self {
        self.context.sim_name = Some(name.into());
        self
    }

    pub fn axis_name(mut self, name: impl Into<String>) -> Self {
        self.context.axis_name = Some(name.into());
        self
    }

    pub fn profile_name(mut self, name: impl Into<String>) -> Self {
        self.context.profile_name = Some(name.into());
        self
    }

    /// Consume the builder and produce a timestamped [`FlightEvent`].
    pub fn build(self) -> FlightEvent {
        FlightEvent {
            timestamp_ns: now_ns(),
            level: self.level,
            component: self.component,
            message: self.message,
            context: self.context,
        }
    }
}

// ── EventSink trait ───────────────────────────────────────────────────────

/// Output destination for structured events.
pub trait EventSink: Send {
    /// Accept a single event.  Implementations may buffer internally.
    fn send(&mut self, event: &FlightEvent) -> Result<(), SinkError>;

    /// Flush any buffered data to the underlying store.
    fn flush(&mut self) -> Result<(), SinkError>;
}

/// Errors produced by [`EventSink`] implementations.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

// ── MemorySink ────────────────────────────────────────────────────────────

/// Fixed-capacity ring buffer that retains the most recent events.
pub struct MemorySink {
    buf: Mutex<MemoryRing>,
}

struct MemoryRing {
    events: Vec<FlightEvent>,
    capacity: usize,
    /// Write cursor (next slot to overwrite).
    head: usize,
    /// Total events written (may exceed capacity).
    total: usize,
}

impl MemorySink {
    /// Create a sink that retains up to `capacity` events.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self {
            buf: Mutex::new(MemoryRing {
                events: Vec::with_capacity(capacity),
                capacity,
                head: 0,
                total: 0,
            }),
        }
    }

    /// Return a snapshot of all retained events in chronological order.
    pub fn snapshot(&self) -> Vec<FlightEvent> {
        let ring = self.buf.lock();
        if ring.total <= ring.capacity {
            // Haven't wrapped yet — events are in insertion order.
            return ring.events.clone();
        }
        // Wrapped: older half starts at `head`, newer half before it.
        let mut out = Vec::with_capacity(ring.capacity);
        out.extend_from_slice(&ring.events[ring.head..]);
        out.extend_from_slice(&ring.events[..ring.head]);
        out
    }

    /// Number of events currently retained.
    pub fn len(&self) -> usize {
        let ring = self.buf.lock();
        ring.events.len()
    }

    /// Whether the sink is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Total events ever received (including those evicted from the ring).
    pub fn total_received(&self) -> usize {
        self.buf.lock().total
    }

    /// Remove all events from the buffer.
    pub fn clear(&self) {
        let mut ring = self.buf.lock();
        ring.events.clear();
        ring.head = 0;
        ring.total = 0;
    }
}

impl EventSink for MemorySink {
    fn send(&mut self, event: &FlightEvent) -> Result<(), SinkError> {
        let mut ring = self.buf.lock();
        if ring.events.len() < ring.capacity {
            ring.events.push(event.clone());
        } else {
            let head = ring.head;
            ring.events[head] = event.clone();
        }
        ring.head = (ring.head + 1) % ring.capacity;
        ring.total += 1;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), SinkError> {
        Ok(()) // nothing to flush — all in memory
    }
}

// ── FileSink ──────────────────────────────────────────────────────────────

/// Append-only JSON-lines writer with size-based rotation.
pub struct FileSink {
    path: PathBuf,
    writer: Option<std::io::BufWriter<std::fs::File>>,
    rotator: LogRotator,
}

impl FileSink {
    /// Open (or create) the log file at `path` with the given rotation config.
    pub fn open(path: impl Into<PathBuf>, rotation: RotationConfig) -> Result<Self, SinkError> {
        let path = path.into();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let meta_len = file.metadata()?.len();
        let mut rotator = LogRotator::new(rotation);
        rotator.record_bytes(meta_len);
        Ok(Self {
            path,
            writer: Some(std::io::BufWriter::new(file)),
            rotator,
        })
    }

    /// Perform a rotation: rename current file and open a fresh one.
    fn rotate(&mut self) -> Result<(), SinkError> {
        // Flush + drop current writer so the OS unlocks the file.
        if let Some(ref mut w) = self.writer {
            w.flush()?;
        }
        self.writer = None;

        let result = self.rotator.rotate();
        if let RotationResult::Rotated { sequence } = result {
            let rotated = self.path.with_extension(format!("log.{sequence}"));
            std::fs::rename(&self.path, rotated)?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.writer = Some(std::io::BufWriter::new(file));
        Ok(())
    }

    /// Path to the active log file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl EventSink for FileSink {
    fn send(&mut self, event: &FlightEvent) -> Result<(), SinkError> {
        let line = serde_json::to_string(event)?;
        let bytes = line.len() as u64 + 1; // +1 for newline

        if self.rotator.should_rotate() {
            self.rotate()?;
        }

        if let Some(ref mut w) = self.writer {
            writeln!(w, "{line}")?;
        }
        self.rotator.record_bytes(bytes);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), SinkError> {
        if let Some(ref mut w) = self.writer {
            w.flush()?;
        }
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- FlightEvent construction & serialization --

    #[test]
    fn event_builder_minimal() {
        let ev = EventBuilder::new(EventLevel::Info, "axis", "tick processed").build();
        assert_eq!(ev.level, EventLevel::Info);
        assert_eq!(ev.component, "axis");
        assert_eq!(ev.message, "tick processed");
        assert!(ev.context.device_id.is_none());
        assert!(ev.timestamp_ns > 0);
    }

    #[test]
    fn event_builder_fluent_api() {
        let ev = EventBuilder::new(EventLevel::Warn, "hid", "write timeout")
            .device_id("0x1234")
            .sim_name("MSFS")
            .axis_name("pitch")
            .profile_name("f18")
            .build();

        assert_eq!(ev.context.device_id.as_deref(), Some("0x1234"));
        assert_eq!(ev.context.sim_name.as_deref(), Some("MSFS"));
        assert_eq!(ev.context.axis_name.as_deref(), Some("pitch"));
        assert_eq!(ev.context.profile_name.as_deref(), Some("f18"));
    }

    #[test]
    fn event_serialization_round_trip() {
        let ev = EventBuilder::new(EventLevel::Error, "ffb", "force clamp")
            .device_id("stick-1")
            .build();

        let json = ev.to_json().expect("serialization should succeed");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["level"], "Error");
        assert_eq!(parsed["component"], "ffb");
        assert_eq!(parsed["context"]["device_id"], "stick-1");
        // sim_name was not set — should be absent
        assert!(parsed["context"].get("sim_name").is_none());
    }

    #[test]
    fn event_json_deserializes_back() {
        let ev = EventBuilder::new(EventLevel::Debug, "bus", "publish")
            .sim_name("X-Plane")
            .build();
        let json = ev.to_json().unwrap();
        let recovered: FlightEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.component, "bus");
        assert_eq!(recovered.context.sim_name.as_deref(), Some("X-Plane"));
        assert_eq!(recovered.timestamp_ns, ev.timestamp_ns);
    }

    // -- EventLevel ordering --

    #[test]
    fn event_levels_ordered() {
        assert!(EventLevel::Trace < EventLevel::Debug);
        assert!(EventLevel::Debug < EventLevel::Info);
        assert!(EventLevel::Info < EventLevel::Warn);
        assert!(EventLevel::Warn < EventLevel::Error);
    }

    #[test]
    fn event_level_display() {
        assert_eq!(EventLevel::Trace.to_string(), "TRACE");
        assert_eq!(EventLevel::Error.to_string(), "ERROR");
    }

    // -- MemorySink --

    #[test]
    fn memory_sink_stores_up_to_capacity() {
        let mut sink = MemorySink::new(3);
        for i in 0..3 {
            let ev = EventBuilder::new(EventLevel::Info, "t", &format!("msg {i}")).build();
            sink.send(&ev).unwrap();
        }
        assert_eq!(sink.len(), 3);
        let snap = sink.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].message, "msg 0");
        assert_eq!(snap[2].message, "msg 2");
    }

    #[test]
    fn memory_sink_evicts_oldest_when_full() {
        let mut sink = MemorySink::new(3);
        for i in 0..5 {
            let ev = EventBuilder::new(EventLevel::Info, "t", &format!("msg {i}")).build();
            sink.send(&ev).unwrap();
        }
        assert_eq!(sink.len(), 3);
        assert_eq!(sink.total_received(), 5);
        let snap = sink.snapshot();
        // Should retain msg 2, 3, 4 (oldest two evicted)
        assert_eq!(snap[0].message, "msg 2");
        assert_eq!(snap[1].message, "msg 3");
        assert_eq!(snap[2].message, "msg 4");
    }

    #[test]
    fn memory_sink_clear_resets() {
        let mut sink = MemorySink::new(10);
        let ev = EventBuilder::new(EventLevel::Info, "t", "x").build();
        sink.send(&ev).unwrap();
        assert!(!sink.is_empty());
        sink.clear();
        assert!(sink.is_empty());
        assert_eq!(sink.total_received(), 0);
    }

    // -- FileSink --

    #[test]
    fn file_sink_writes_json_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let config = RotationConfig {
            max_file_size_bytes: 1_000_000,
            max_files: 5,
            compress_rotated: false,
        };
        let mut sink = FileSink::open(&path, config).unwrap();

        for i in 0..3 {
            let ev = EventBuilder::new(EventLevel::Info, "test", &format!("line {i}")).build();
            sink.send(&ev).unwrap();
        }
        sink.flush().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in &lines {
            let _: FlightEvent = serde_json::from_str(line).expect("each line must be valid JSON");
        }
    }

    #[test]
    fn file_sink_rotation_creates_numbered_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.log");
        let config = RotationConfig {
            max_file_size_bytes: 50, // tiny limit to force rotation
            max_files: 5,
            compress_rotated: false,
        };
        let mut sink = FileSink::open(&path, config).unwrap();

        // Write enough events to trigger at least one rotation
        for i in 0..10 {
            let ev = EventBuilder::new(EventLevel::Info, "t", &format!("event {i}")).build();
            sink.send(&ev).unwrap();
        }
        sink.flush().unwrap();

        // At least one rotated file should exist
        let rotated = dir.path().join("app.log.1");
        assert!(rotated.exists(), "rotated file should exist");
    }
}
