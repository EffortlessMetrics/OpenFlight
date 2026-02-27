// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Custom snapshot testing framework for golden-file comparisons.

use std::collections::HashMap;

/// Format of a stored snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Json,
    Yaml,
    PlainText,
}

/// Metadata attached to every snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotMetadata {
    pub created_at: String,
    pub format: SnapshotFormat,
}

/// A named snapshot with content and metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub content: String,
    pub metadata: SnapshotMetadata,
}

/// Result of verifying a snapshot against expected content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotResult {
    /// Content matches the stored snapshot.
    Match,
    /// Content differs from the stored snapshot.
    Mismatch { expected: String, actual: String },
    /// No snapshot with this name exists yet.
    New,
}

/// In-memory snapshot store for golden-file testing.
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    snapshots: HashMap<String, String>,
}

impl SnapshotStore {
    /// Create an empty snapshot store.
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
        }
    }

    /// Record a new snapshot. Overwrites any existing snapshot with the same name.
    pub fn record(&mut self, name: impl Into<String>, content: impl Into<String>) {
        self.snapshots.insert(name.into(), content.into());
    }

    /// Verify content against a stored snapshot.
    pub fn verify(&self, name: &str, content: &str) -> SnapshotResult {
        match self.snapshots.get(name) {
            None => SnapshotResult::New,
            Some(expected) if expected == content => SnapshotResult::Match,
            Some(expected) => SnapshotResult::Mismatch {
                expected: expected.clone(),
                actual: content.to_owned(),
            },
        }
    }

    /// Update an existing snapshot (or insert if absent).
    pub fn update(&mut self, name: impl Into<String>, content: impl Into<String>) {
        self.snapshots.insert(name.into(), content.into());
    }

    /// Return all snapshot names in arbitrary order.
    pub fn all_names(&self) -> Vec<&str> {
        self.snapshots.keys().map(String::as_str).collect()
    }

    /// Check whether a snapshot with the given name exists.
    pub fn has(&self, name: &str) -> bool {
        self.snapshots.contains_key(name)
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_store_is_empty() {
        let store = SnapshotStore::new();
        assert!(store.all_names().is_empty());
    }

    #[test]
    fn record_and_verify_match() {
        let mut store = SnapshotStore::new();
        store.record("axis_curve", "[[0,0],[0.5,0.5],[1,1]]");
        assert_eq!(
            store.verify("axis_curve", "[[0,0],[0.5,0.5],[1,1]]"),
            SnapshotResult::Match
        );
    }

    #[test]
    fn verify_returns_new_for_unknown() {
        let store = SnapshotStore::new();
        assert_eq!(store.verify("missing", "data"), SnapshotResult::New);
    }

    #[test]
    fn verify_detects_mismatch() {
        let mut store = SnapshotStore::new();
        store.record("out", "expected");
        let result = store.verify("out", "actual");
        assert_eq!(
            result,
            SnapshotResult::Mismatch {
                expected: "expected".to_owned(),
                actual: "actual".to_owned(),
            }
        );
    }

    #[test]
    fn update_overwrites_existing() {
        let mut store = SnapshotStore::new();
        store.record("snap", "v1");
        store.update("snap", "v2");
        assert_eq!(store.verify("snap", "v2"), SnapshotResult::Match);
    }

    #[test]
    fn update_inserts_when_absent() {
        let mut store = SnapshotStore::new();
        store.update("new_snap", "content");
        assert!(store.has("new_snap"));
    }

    #[test]
    fn has_returns_false_for_missing() {
        let store = SnapshotStore::new();
        assert!(!store.has("nope"));
    }

    #[test]
    fn has_returns_true_after_record() {
        let mut store = SnapshotStore::new();
        store.record("present", "data");
        assert!(store.has("present"));
    }

    #[test]
    fn all_names_returns_recorded() {
        let mut store = SnapshotStore::new();
        store.record("a", "1");
        store.record("b", "2");
        let mut names = store.all_names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn record_overwrites_previous() {
        let mut store = SnapshotStore::new();
        store.record("x", "old");
        store.record("x", "new");
        assert_eq!(store.verify("x", "new"), SnapshotResult::Match);
    }

    #[test]
    fn snapshot_metadata_round_trip() {
        let snap = Snapshot {
            name: "test".to_owned(),
            content: "{}".to_owned(),
            metadata: SnapshotMetadata {
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                format: SnapshotFormat::Json,
            },
        };
        assert_eq!(snap.metadata.format, SnapshotFormat::Json);
        assert_eq!(snap.name, "test");
    }

    #[test]
    fn snapshot_format_variants() {
        assert_ne!(SnapshotFormat::Json, SnapshotFormat::Yaml);
        assert_ne!(SnapshotFormat::Yaml, SnapshotFormat::PlainText);
        assert_ne!(SnapshotFormat::PlainText, SnapshotFormat::Json);
    }

    #[test]
    fn default_store_is_empty() {
        let store = SnapshotStore::default();
        assert!(store.all_names().is_empty());
        assert!(!store.has("anything"));
    }
}
