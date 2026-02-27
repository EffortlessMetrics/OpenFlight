// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Audit log for configuration changes (REQ-686).
//!
//! Maintains an in-memory ring buffer of the last 1000 audit entries
//! and optionally appends each entry to a file for persistent storage.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of entries retained in the in-memory ring buffer.
const MAX_ENTRIES: usize = 1000;

/// Type of configuration change recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// A profile was created.
    ProfileCreated,
    /// A profile was updated.
    ProfileUpdated,
    /// A profile was deleted.
    ProfileDeleted,
    /// An axis mapping was changed.
    AxisMappingChanged,
    /// A general setting was modified.
    SettingChanged,
}

impl fmt::Display for ChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProfileCreated => write!(f, "ProfileCreated"),
            Self::ProfileUpdated => write!(f, "ProfileUpdated"),
            Self::ProfileDeleted => write!(f, "ProfileDeleted"),
            Self::AxisMappingChanged => write!(f, "AxisMappingChanged"),
            Self::SettingChanged => write!(f, "SettingChanged"),
        }
    }
}

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
    /// User or component that made the change.
    pub user: String,
    /// Category of change.
    pub change_type: ChangeType,
    /// Human-readable description of the change.
    pub description: String,
    /// Value before the change (if applicable).
    pub before_value: Option<String>,
    /// Value after the change (if applicable).
    pub after_value: Option<String>,
}

impl AuditEntry {
    /// Create a new audit entry with the current timestamp.
    pub fn new(
        user: impl Into<String>,
        change_type: ChangeType,
        description: impl Into<String>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            timestamp,
            user: user.into(),
            change_type,
            description: description.into(),
            before_value: None,
            after_value: None,
        }
    }

    /// Attach before/after values.
    #[must_use]
    pub fn with_values(
        mut self,
        before: impl Into<Option<String>>,
        after: impl Into<Option<String>>,
    ) -> Self {
        self.before_value = before.into();
        self.after_value = after.into();
        self
    }
}

/// In-memory ring-buffer audit log.
pub struct AuditLog {
    entries: Mutex<VecDeque<AuditEntry>>,
}

impl AuditLog {
    /// Create a new empty audit log.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(MAX_ENTRIES)),
        }
    }

    /// Record an audit entry. Drops the oldest entry when the buffer is full.
    pub fn log(&self, entry: AuditEntry) {
        let mut buf = self.entries.lock();
        if buf.len() >= MAX_ENTRIES {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    /// Return a snapshot of all entries currently in the buffer.
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().iter().cloned().collect()
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }

    /// Append all current entries to a file as newline-delimited JSON.
    pub fn write_to_file(&self, path: &Path) -> std::io::Result<()> {
        let entries = self.entries.lock();
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        for entry in entries.iter() {
            let line =
                serde_json::to_string(entry).map_err(|e| std::io::Error::other(e.to_string()))?;
            writeln!(file, "{line}")?;
        }
        Ok(())
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    #[test]
    fn log_entry_and_read() {
        let log = AuditLog::new();
        let entry = AuditEntry::new("admin", ChangeType::SettingChanged, "set volume to 80");
        log.log(entry);

        let entries = log.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].user, "admin");
        assert_eq!(entries[0].change_type, ChangeType::SettingChanged);
        assert_eq!(entries[0].description, "set volume to 80");
    }

    #[test]
    fn before_after_values() {
        let entry = AuditEntry::new("user1", ChangeType::ProfileUpdated, "update deadzone")
            .with_values(Some("0.05".to_string()), Some("0.10".to_string()));
        assert_eq!(entry.before_value.as_deref(), Some("0.05"));
        assert_eq!(entry.after_value.as_deref(), Some("0.10"));
    }

    #[test]
    fn overflow_ring_buffer() {
        let log = AuditLog::new();
        for i in 0..1_050 {
            log.log(AuditEntry::new(
                format!("user-{i}"),
                ChangeType::SettingChanged,
                format!("change #{i}"),
            ));
        }
        assert_eq!(log.len(), MAX_ENTRIES);
        let entries = log.entries();
        // Oldest surviving entry should be #50
        assert_eq!(entries[0].description, "change #50");
        assert_eq!(entries[MAX_ENTRIES - 1].description, "change #1049");
    }

    #[test]
    fn write_to_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");

        let log = AuditLog::new();
        log.log(AuditEntry::new(
            "admin",
            ChangeType::ProfileCreated,
            "created default",
        ));
        log.log(AuditEntry::new(
            "user2",
            ChangeType::AxisMappingChanged,
            "remapped yaw",
        ));
        log.write_to_file(&path).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let reader = std::io::BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
        assert_eq!(lines.len(), 2);

        let parsed: AuditEntry = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(parsed.user, "admin");
    }
}
