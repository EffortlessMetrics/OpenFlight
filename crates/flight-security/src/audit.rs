// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! High-level audit event types for security forensics (REQ-934).
//!
//! Extends the lower-level [`crate::audit_log`] module with domain-specific
//! event types and serialization support.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Domain-level security audit events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityAuditEvent {
    PluginLoaded {
        name: String,
        version: String,
    },
    PluginUnloaded {
        name: String,
    },
    ConfigChanged {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        old_value: Option<String>,
        new_value: String,
    },
    DeviceConnected {
        device_id: String,
        device_type: String,
    },
    DeviceDisconnected {
        device_id: String,
    },
    AuthAttempt {
        user_id: String,
        success: bool,
    },
    PermissionDenied {
        actor: String,
        capability: String,
    },
    UpdateVerified {
        version: String,
        checksum: String,
    },
    UpdateFailed {
        version: String,
        reason: String,
    },
    ServiceStarted,
    ServiceStopped,
}

/// A timestamped audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Seconds since UNIX epoch.
    pub timestamp_secs: u64,
    /// The event.
    pub event: SecurityAuditEvent,
    /// Free-form details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl AuditRecord {
    /// Create a record stamped with the current time.
    pub fn now(event: SecurityAuditEvent, details: Option<String>) -> Self {
        let timestamp_secs = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            timestamp_secs,
            event,
            details,
        }
    }
}

/// Append-only audit log that can be serialized for forensics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditLog {
    records: std::collections::VecDeque<AuditRecord>,
    max_records: usize,
}

impl SecurityAuditLog {
    /// Create a new log with the given capacity limit.
    pub fn new(max_records: usize) -> Self {
        Self {
            records: std::collections::VecDeque::with_capacity(max_records.min(4096)),
            max_records,
        }
    }

    /// Append an event. Does nothing when `max_records` is zero.
    pub fn append(&mut self, event: SecurityAuditEvent, details: Option<String>) {
        if self.max_records == 0 {
            return;
        }
        if self.records.len() >= self.max_records {
            self.records.pop_front();
        }
        self.records.push_back(AuditRecord::now(event, details));
    }

    /// All records.
    pub fn records(&self) -> &std::collections::VecDeque<AuditRecord> {
        &self.records
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Filter records by event type predicate.
    pub fn filter<F: Fn(&SecurityAuditEvent) -> bool>(&self, pred: F) -> Vec<&AuditRecord> {
        self.records.iter().filter(|r| pred(&r.event)).collect()
    }

    /// Serialize the log to a JSON string.
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a log from a JSON string.
    pub fn from_json(json: &str) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Clear all records.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Append and retrieve ---

    #[test]
    fn test_append_single_event() {
        let mut log = SecurityAuditLog::new(100);
        log.append(SecurityAuditEvent::ServiceStarted, None);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn test_append_preserves_order() {
        let mut log = SecurityAuditLog::new(100);
        log.append(SecurityAuditEvent::ServiceStarted, None);
        log.append(SecurityAuditEvent::ServiceStopped, None);
        assert_eq!(log.records()[0].event, SecurityAuditEvent::ServiceStarted);
        assert_eq!(log.records()[1].event, SecurityAuditEvent::ServiceStopped);
    }

    #[test]
    fn test_append_rotates_when_full() {
        let mut log = SecurityAuditLog::new(2);
        log.append(SecurityAuditEvent::ServiceStarted, Some("first".into()));
        log.append(
            SecurityAuditEvent::PluginLoaded {
                name: "a".into(),
                version: "1".into(),
            },
            None,
        );
        log.append(SecurityAuditEvent::ServiceStopped, Some("third".into()));
        assert_eq!(log.len(), 2);
        // first event should be evicted
        assert!(matches!(
            log.records()[0].event,
            SecurityAuditEvent::PluginLoaded { .. }
        ));
    }

    // --- Filter ---

    #[test]
    fn test_filter_by_event_type() {
        let mut log = SecurityAuditLog::new(100);
        log.append(SecurityAuditEvent::ServiceStarted, None);
        log.append(
            SecurityAuditEvent::AuthAttempt {
                user_id: "u1".into(),
                success: true,
            },
            None,
        );
        log.append(
            SecurityAuditEvent::AuthAttempt {
                user_id: "u2".into(),
                success: false,
            },
            None,
        );
        let auth_events = log.filter(|e| matches!(e, SecurityAuditEvent::AuthAttempt { .. }));
        assert_eq!(auth_events.len(), 2);
    }

    #[test]
    fn test_filter_no_match() {
        let mut log = SecurityAuditLog::new(100);
        log.append(SecurityAuditEvent::ServiceStarted, None);
        let results = log.filter(|e| matches!(e, SecurityAuditEvent::ServiceStopped));
        assert!(results.is_empty());
    }

    // --- JSON serialization ---

    #[test]
    fn test_json_round_trip() {
        let mut log = SecurityAuditLog::new(100);
        log.append(
            SecurityAuditEvent::PluginLoaded {
                name: "test-plugin".into(),
                version: "2.0".into(),
            },
            Some("loaded successfully".into()),
        );
        log.append(
            SecurityAuditEvent::ConfigChanged {
                key: "axis.deadzone".into(),
                old_value: Some("0.05".into()),
                new_value: "0.10".into(),
            },
            None,
        );

        let json = log.to_json().unwrap();
        let restored = SecurityAuditLog::from_json(&json).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.records()[0].event, log.records()[0].event);
        assert_eq!(restored.records()[1].event, log.records()[1].event);
    }

    #[test]
    fn test_empty_log_json_round_trip() {
        let log = SecurityAuditLog::new(10);
        let json = log.to_json().unwrap();
        let restored = SecurityAuditLog::from_json(&json).unwrap();
        assert!(restored.is_empty());
    }

    #[test]
    fn test_json_contains_event_fields() {
        let mut log = SecurityAuditLog::new(10);
        log.append(
            SecurityAuditEvent::DeviceConnected {
                device_id: "dev-42".into(),
                device_type: "joystick".into(),
            },
            None,
        );
        let json = log.to_json().unwrap();
        assert!(json.contains("dev-42"));
        assert!(json.contains("joystick"));
    }

    // --- Clear ---

    #[test]
    fn test_clear() {
        let mut log = SecurityAuditLog::new(100);
        log.append(SecurityAuditEvent::ServiceStarted, None);
        log.append(SecurityAuditEvent::ServiceStopped, None);
        assert_eq!(log.len(), 2);
        log.clear();
        assert!(log.is_empty());
    }

    // --- Timestamp ---

    #[test]
    fn test_record_has_nonzero_timestamp() {
        let record = AuditRecord::now(SecurityAuditEvent::ServiceStarted, None);
        assert!(record.timestamp_secs > 0);
    }

    // --- All event variants serialize ---

    #[test]
    fn test_all_event_variants_serialize() {
        let events = vec![
            SecurityAuditEvent::PluginLoaded {
                name: "p".into(),
                version: "1".into(),
            },
            SecurityAuditEvent::PluginUnloaded { name: "p".into() },
            SecurityAuditEvent::ConfigChanged {
                key: "k".into(),
                old_value: None,
                new_value: "v".into(),
            },
            SecurityAuditEvent::DeviceConnected {
                device_id: "d".into(),
                device_type: "t".into(),
            },
            SecurityAuditEvent::DeviceDisconnected {
                device_id: "d".into(),
            },
            SecurityAuditEvent::AuthAttempt {
                user_id: "u".into(),
                success: true,
            },
            SecurityAuditEvent::PermissionDenied {
                actor: "a".into(),
                capability: "c".into(),
            },
            SecurityAuditEvent::UpdateVerified {
                version: "1.0".into(),
                checksum: "abc".into(),
            },
            SecurityAuditEvent::UpdateFailed {
                version: "1.0".into(),
                reason: "bad sig".into(),
            },
            SecurityAuditEvent::ServiceStarted,
            SecurityAuditEvent::ServiceStopped,
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let restored: SecurityAuditEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, restored);
        }
    }

    // --- Details field ---

    #[test]
    fn test_record_with_details() {
        let record = AuditRecord::now(
            SecurityAuditEvent::ServiceStarted,
            Some("boot sequence initiated".into()),
        );
        assert_eq!(record.details.as_deref(), Some("boot sequence initiated"));
    }

    #[test]
    fn test_record_without_details() {
        let record = AuditRecord::now(SecurityAuditEvent::ServiceStopped, None);
        assert!(record.details.is_none());
    }
}
