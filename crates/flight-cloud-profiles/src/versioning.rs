// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile version tracking and history management.
//!
//! Provides hash-based versioning for profiles, allowing callers to track
//! changes over time and compute diffs between versions.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Compute a SHA-256 hash of the given data, returned as a lowercase hex string.
pub fn compute_version_hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

/// A single version in a profile's history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileVersion {
    /// Hash-based version identifier.
    pub version_id: String,
    /// Unix timestamp when this version was created.
    pub timestamp: u64,
    /// Author who created this version.
    pub author: String,
    /// Description of changes in this version.
    pub changes: Vec<String>,
}

/// Tracks the version history of a profile.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VersionHistory {
    /// Ordered list of versions (oldest first).
    pub versions: Vec<ProfileVersion>,
}

/// Diff between two profile versions.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionDiff {
    /// Version ID of the starting point.
    pub from: String,
    /// Version ID of the ending point.
    pub to: String,
    /// Aggregate changes between the two versions.
    pub changes: Vec<String>,
}

impl VersionHistory {
    /// Create a new empty version history.
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
        }
    }

    /// Returns the most recent version, if any.
    pub fn current(&self) -> Option<&ProfileVersion> {
        self.versions.last()
    }

    /// Returns the version before the current one, if any.
    pub fn previous(&self) -> Option<&ProfileVersion> {
        if self.versions.len() >= 2 {
            self.versions.get(self.versions.len() - 2)
        } else {
            None
        }
    }

    /// Add a new version to the history.
    pub fn push(&mut self, version: ProfileVersion) {
        self.versions.push(version);
    }

    /// Compute the diff between two versions identified by their version IDs.
    ///
    /// Returns `None` if either version ID is not found. The diff includes all
    /// changes from versions after `from_id` up to and including `to_id`.
    pub fn diff(&self, from_id: &str, to_id: &str) -> Option<VersionDiff> {
        let from_idx = self.versions.iter().position(|v| v.version_id == from_id)?;
        let to_idx = self.versions.iter().position(|v| v.version_id == to_id)?;

        let (start, end) = if from_idx < to_idx {
            (from_idx + 1, to_idx + 1)
        } else {
            (to_idx + 1, from_idx + 1)
        };

        let changes: Vec<String> = self.versions[start..end]
            .iter()
            .flat_map(|v| v.changes.clone())
            .collect();

        Some(VersionDiff {
            from: from_id.to_string(),
            to: to_id.to_string(),
            changes,
        })
    }

    /// Get a version by its ID.
    pub fn get(&self, version_id: &str) -> Option<&ProfileVersion> {
        self.versions.iter().find(|v| v.version_id == version_id)
    }

    /// Number of versions in the history.
    pub fn len(&self) -> usize {
        self.versions.len()
    }

    /// Returns true if there are no versions.
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_version(id: &str, ts: u64, author: &str, changes: &[&str]) -> ProfileVersion {
        ProfileVersion {
            version_id: id.to_string(),
            timestamp: ts,
            author: author.to_string(),
            changes: changes.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_compute_version_hash() {
        let hash = compute_version_hash(b"hello world");
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64, "SHA-256 produces 64 hex chars");
        assert_eq!(hash, compute_version_hash(b"hello world"));
    }

    #[test]
    fn test_compute_version_hash_different_inputs() {
        let h1 = compute_version_hash(b"input a");
        let h2 = compute_version_hash(b"input b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_version_history_empty() {
        let history = VersionHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.current().is_none());
        assert!(history.previous().is_none());
    }

    #[test]
    fn test_version_history_single_version() {
        let mut history = VersionHistory::new();
        history.push(make_version("v1", 1000, "alice", &["initial"]));

        assert_eq!(history.len(), 1);
        assert!(!history.is_empty());
        assert_eq!(history.current().unwrap().version_id, "v1");
        assert!(history.previous().is_none());
    }

    #[test]
    fn test_version_history_current_and_previous() {
        let mut history = VersionHistory::new();
        history.push(make_version("v1", 1000, "alice", &["initial"]));
        history.push(make_version("v2", 2000, "bob", &["update curves"]));
        history.push(make_version("v3", 3000, "alice", &["fix deadzone"]));

        assert_eq!(history.current().unwrap().version_id, "v3");
        assert_eq!(history.previous().unwrap().version_id, "v2");
    }

    #[test]
    fn test_version_history_get_by_id() {
        let mut history = VersionHistory::new();
        history.push(make_version("v1", 1000, "alice", &["initial"]));
        history.push(make_version("v2", 2000, "bob", &["update"]));

        assert_eq!(history.get("v1").unwrap().author, "alice");
        assert_eq!(history.get("v2").unwrap().author, "bob");
        assert!(history.get("v99").is_none());
    }

    #[test]
    fn test_version_history_diff() {
        let mut history = VersionHistory::new();
        history.push(make_version("v1", 1000, "alice", &["initial setup"]));
        history.push(make_version("v2", 2000, "bob", &["changed deadzone"]));
        history.push(make_version(
            "v3",
            3000,
            "alice",
            &["added expo", "tweaked curve"],
        ));

        let diff = history.diff("v1", "v3").unwrap();
        assert_eq!(diff.from, "v1");
        assert_eq!(diff.to, "v3");
        assert_eq!(diff.changes.len(), 3);
        assert!(diff.changes.contains(&"changed deadzone".to_string()));
        assert!(diff.changes.contains(&"added expo".to_string()));
        assert!(diff.changes.contains(&"tweaked curve".to_string()));
    }

    #[test]
    fn test_version_history_diff_adjacent_versions() {
        let mut history = VersionHistory::new();
        history.push(make_version("v1", 1000, "alice", &["initial"]));
        history.push(make_version("v2", 2000, "bob", &["update"]));

        let diff = history.diff("v1", "v2").unwrap();
        assert_eq!(diff.changes, vec!["update".to_string()]);
    }

    #[test]
    fn test_version_history_diff_not_found() {
        let history = VersionHistory::new();
        assert!(history.diff("v1", "v2").is_none());
    }

    #[test]
    fn test_version_history_diff_same_version() {
        let mut history = VersionHistory::new();
        history.push(make_version("v1", 1000, "alice", &["initial"]));

        let diff = history.diff("v1", "v1").unwrap();
        assert!(diff.changes.is_empty());
    }

    #[test]
    fn test_profile_version_serialization_round_trip() {
        let version = make_version("v1", 1000, "alice", &["created profile", "set deadzone"]);
        let json = serde_json::to_string(&version).unwrap();
        let back: ProfileVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(version, back);
    }

    #[test]
    fn test_version_history_serialization_round_trip() {
        let mut history = VersionHistory::new();
        history.push(make_version("v1", 1000, "alice", &["initial"]));
        history.push(make_version("v2", 2000, "bob", &["update"]));

        let json = serde_json::to_string(&history).unwrap();
        let back: VersionHistory = serde_json::from_str(&json).unwrap();
        assert_eq!(history, back);
    }
}
