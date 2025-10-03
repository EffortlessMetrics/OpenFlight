// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Trace event definitions and serialization
//!
//! Defines the structured events that are emitted by the Flight Hub tracing system.
//! Events are designed to be lightweight and contain only essential timing data.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Core trace event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Event timestamp (nanoseconds since Unix epoch)
    pub timestamp_ns: u64,
    /// Event data payload
    pub data: EventData,
}

/// Event data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventData {
    /// RT loop tick started
    TickStart {
        tick_number: u64,
    },
    
    /// RT loop tick completed
    TickEnd {
        tick_number: u64,
        duration_ns: u64,
        jitter_ns: i64,
    },
    
    /// HID write operation
    HidWrite {
        device_id: u32,
        bytes: usize,
        duration_ns: u64,
    },
    
    /// Deadline miss detected
    DeadlineMiss {
        tick_number: u64,
        miss_duration_ns: u64,
    },
    
    /// Writer buffer drop
    WriterDrop {
        stream_id: String,
        dropped_count: u64,
    },
    
    /// Custom event for extensibility
    Custom {
        name: String,
        data: serde_json::Value,
    },
}

impl TraceEvent {
    /// Create tick start event
    pub fn tick_start(tick_number: u64) -> Self {
        Self {
            timestamp_ns: current_timestamp_ns(),
            data: EventData::TickStart { tick_number },
        }
    }
    
    /// Create tick end event
    pub fn tick_end(tick_number: u64, duration_ns: u64, jitter_ns: i64) -> Self {
        Self {
            timestamp_ns: current_timestamp_ns(),
            data: EventData::TickEnd {
                tick_number,
                duration_ns,
                jitter_ns,
            },
        }
    }
    
    /// Create HID write event
    pub fn hid_write(device_id: u32, bytes: usize, duration_ns: u64) -> Self {
        Self {
            timestamp_ns: current_timestamp_ns(),
            data: EventData::HidWrite {
                device_id,
                bytes,
                duration_ns,
            },
        }
    }
    
    /// Create deadline miss event
    pub fn deadline_miss(tick_number: u64, miss_duration_ns: u64) -> Self {
        Self {
            timestamp_ns: current_timestamp_ns(),
            data: EventData::DeadlineMiss {
                tick_number,
                miss_duration_ns,
            },
        }
    }
    
    /// Create writer drop event
    pub fn writer_drop(stream_id: impl Into<String>, dropped_count: u64) -> Self {
        Self {
            timestamp_ns: current_timestamp_ns(),
            data: EventData::WriterDrop {
                stream_id: stream_id.into(),
                dropped_count,
            },
        }
    }
    
    /// Create custom event
    pub fn custom(name: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            timestamp_ns: current_timestamp_ns(),
            data: EventData::Custom {
                name: name.into(),
                data,
            },
        }
    }
    
    /// Get event type as string
    pub fn event_type(&self) -> &'static str {
        match &self.data {
            EventData::TickStart { .. } => "TickStart",
            EventData::TickEnd { .. } => "TickEnd",
            EventData::HidWrite { .. } => "HidWrite",
            EventData::DeadlineMiss { .. } => "DeadlineMiss",
            EventData::WriterDrop { .. } => "WriterDrop",
            EventData::Custom { .. } => "Custom",
        }
    }
    
    /// Serialize event to JSON bytes
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    
    /// Serialize event to compact binary format
    pub fn to_binary(&self) -> Vec<u8> {
        // Simple binary format for high-performance logging
        let mut buf = Vec::with_capacity(64);
        
        // Timestamp (8 bytes)
        buf.extend_from_slice(&self.timestamp_ns.to_le_bytes());
        
        // Event type and data
        match &self.data {
            EventData::TickStart { tick_number } => {
                buf.push(0x01); // TickStart type
                buf.extend_from_slice(&tick_number.to_le_bytes());
            }
            EventData::TickEnd { tick_number, duration_ns, jitter_ns } => {
                buf.push(0x02); // TickEnd type
                buf.extend_from_slice(&tick_number.to_le_bytes());
                buf.extend_from_slice(&duration_ns.to_le_bytes());
                buf.extend_from_slice(&jitter_ns.to_le_bytes());
            }
            EventData::HidWrite { device_id, bytes, duration_ns } => {
                buf.push(0x03); // HidWrite type
                buf.extend_from_slice(&device_id.to_le_bytes());
                buf.extend_from_slice(&(*bytes as u32).to_le_bytes());
                buf.extend_from_slice(&duration_ns.to_le_bytes());
            }
            EventData::DeadlineMiss { tick_number, miss_duration_ns } => {
                buf.push(0x04); // DeadlineMiss type
                buf.extend_from_slice(&tick_number.to_le_bytes());
                buf.extend_from_slice(&miss_duration_ns.to_le_bytes());
            }
            EventData::WriterDrop { stream_id, dropped_count } => {
                buf.push(0x05); // WriterDrop type
                let id_bytes = stream_id.as_bytes();
                buf.push(id_bytes.len() as u8);
                buf.extend_from_slice(id_bytes);
                buf.extend_from_slice(&dropped_count.to_le_bytes());
            }
            EventData::Custom { name, data } => {
                buf.push(0xFF); // Custom type
                let json_bytes = serde_json::to_vec(data).unwrap_or_default();
                let name_bytes = name.as_bytes();
                buf.push(name_bytes.len() as u8);
                buf.extend_from_slice(name_bytes);
                buf.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
                buf.extend_from_slice(&json_bytes);
            }
        }
        
        buf
    }
}

/// Get current timestamp in nanoseconds since Unix epoch
fn current_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Event filtering for performance
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// Enable tick events
    pub tick_events: bool,
    /// Enable HID write events
    pub hid_events: bool,
    /// Enable deadline miss events
    pub deadline_events: bool,
    /// Enable writer drop events
    pub writer_events: bool,
    /// Enable custom events
    pub custom_events: bool,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self {
            tick_events: true,
            hid_events: true,
            deadline_events: true,
            writer_events: true,
            custom_events: false, // Disabled by default for performance
        }
    }
}

impl EventFilter {
    /// Check if event should be traced
    pub fn should_trace(&self, event: &TraceEvent) -> bool {
        match &event.data {
            EventData::TickStart { .. } | EventData::TickEnd { .. } => self.tick_events,
            EventData::HidWrite { .. } => self.hid_events,
            EventData::DeadlineMiss { .. } => self.deadline_events,
            EventData::WriterDrop { .. } => self.writer_events,
            EventData::Custom { .. } => self.custom_events,
        }
    }
    
    /// Create filter for CI performance testing (minimal events)
    pub fn ci_minimal() -> Self {
        Self {
            tick_events: false,
            hid_events: true,
            deadline_events: true,
            writer_events: true,
            custom_events: false,
        }
    }
    
    /// Create filter for development (all events)
    pub fn development() -> Self {
        Self {
            tick_events: true,
            hid_events: true,
            deadline_events: true,
            writer_events: true,
            custom_events: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let tick_start = TraceEvent::tick_start(42);
        assert_eq!(tick_start.event_type(), "TickStart");
        
        let tick_end = TraceEvent::tick_end(42, 1000000, 500);
        assert_eq!(tick_end.event_type(), "TickEnd");
        
        let hid_write = TraceEvent::hid_write(0x1234, 64, 250000);
        assert_eq!(hid_write.event_type(), "HidWrite");
        
        let deadline_miss = TraceEvent::deadline_miss(43, 2000000);
        assert_eq!(deadline_miss.event_type(), "DeadlineMiss");
        
        let writer_drop = TraceEvent::writer_drop("axis", 5);
        assert_eq!(writer_drop.event_type(), "WriterDrop");
    }

    #[test]
    fn test_event_serialization() {
        let event = TraceEvent::tick_end(100, 4000000, 1500);
        
        // JSON serialization
        let json_bytes = event.to_json_bytes().unwrap();
        let deserialized: TraceEvent = serde_json::from_slice(&json_bytes).unwrap();
        assert_eq!(event.timestamp_ns, deserialized.timestamp_ns);
        
        // Binary serialization
        let binary = event.to_binary();
        assert!(!binary.is_empty());
        assert_eq!(binary[0..8], event.timestamp_ns.to_le_bytes());
        assert_eq!(binary[8], 0x02); // TickEnd type
    }

    #[test]
    fn test_event_filter() {
        let filter = EventFilter::default();
        
        let tick_event = TraceEvent::tick_start(1);
        let hid_event = TraceEvent::hid_write(0x1234, 64, 250000);
        let custom_event = TraceEvent::custom("test", serde_json::json!({"key": "value"}));
        
        assert!(filter.should_trace(&tick_event));
        assert!(filter.should_trace(&hid_event));
        assert!(!filter.should_trace(&custom_event)); // Custom disabled by default
        
        let ci_filter = EventFilter::ci_minimal();
        assert!(!ci_filter.should_trace(&tick_event)); // Ticks disabled for CI
        assert!(ci_filter.should_trace(&hid_event));
    }

    #[test]
    fn test_binary_format_size() {
        // Verify binary format is compact
        let tick_end = TraceEvent::tick_end(100, 4000000, 1500);
        let binary = tick_end.to_binary();
        
        // Should be: 8 (timestamp) + 1 (type) + 8 (tick) + 8 (duration) + 8 (jitter) = 33 bytes
        assert_eq!(binary.len(), 33);
        
        let hid_write = TraceEvent::hid_write(0x1234, 64, 250000);
        let binary = hid_write.to_binary();
        
        // Should be: 8 (timestamp) + 1 (type) + 4 (device) + 4 (bytes) + 8 (duration) = 25 bytes
        assert_eq!(binary.len(), 25);
    }
}