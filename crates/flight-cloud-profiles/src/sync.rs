// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile synchronization engine.
//!
//! Compares local and remote profile states to produce a [`SyncPlan`]
//! describing which profiles to upload, download, or flag as conflicts.
//! Supports last-write-wins and manual conflict resolution strategies.

use crate::Result;
use crate::storage::CloudBackend;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State of a profile for sync comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSyncState {
    /// Profile identifier.
    pub id: String,
    /// Hash-based version identifier.
    pub version_hash: String,
    /// Unix timestamp of last modification.
    pub updated_at: u64,
}

/// An action to perform on a profile during synchronization.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileAction {
    /// Profile identifier.
    pub profile_id: String,
    /// Version hash of the profile to sync.
    pub version_hash: String,
}

/// A conflict between local and remote profile versions.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncConflict {
    /// Profile identifier.
    pub profile_id: String,
    /// Local version state.
    pub local_version: ProfileSyncState,
    /// Remote version state.
    pub remote_version: ProfileSyncState,
}

/// How to resolve a sync conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Use the local version (upload to remote).
    UseLocal,
    /// Use the remote version (download to local).
    UseRemote,
    /// Skip this profile (no action).
    Skip,
}

/// Strategy for automatic conflict resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictStrategy {
    /// Automatically resolve by picking the version with the latest timestamp.
    #[default]
    LastWriteWins,
    /// Leave conflicts unresolved for manual intervention.
    Manual,
}

/// Result of sync planning — describes what actions to take.
#[derive(Debug, Clone, Default)]
pub struct SyncPlan {
    /// Profiles to upload (local → remote).
    pub uploads: Vec<ProfileAction>,
    /// Profiles to download (remote → local).
    pub downloads: Vec<ProfileAction>,
    /// Conflicts requiring resolution.
    pub conflicts: Vec<SyncConflict>,
}

impl SyncPlan {
    /// Returns `true` if no actions are needed.
    pub fn is_empty(&self) -> bool {
        self.uploads.is_empty() && self.downloads.is_empty() && self.conflicts.is_empty()
    }

    /// Resolve a conflict for the given profile ID.
    ///
    /// Moves the conflict into the appropriate action list based on the
    /// resolution, or drops it if [`ConflictResolution::Skip`] is chosen.
    pub fn resolve_conflict(&mut self, profile_id: &str, resolution: ConflictResolution) {
        let idx = self
            .conflicts
            .iter()
            .position(|c| c.profile_id == profile_id);
        if let Some(idx) = idx {
            let conflict = self.conflicts.remove(idx);
            match resolution {
                ConflictResolution::UseLocal => {
                    self.uploads.push(ProfileAction {
                        profile_id: conflict.profile_id,
                        version_hash: conflict.local_version.version_hash,
                    });
                }
                ConflictResolution::UseRemote => {
                    self.downloads.push(ProfileAction {
                        profile_id: conflict.profile_id,
                        version_hash: conflict.remote_version.version_hash,
                    });
                }
                ConflictResolution::Skip => {}
            }
        }
    }
}

/// Engine for synchronizing profiles between local and remote storage.
pub struct SyncEngine<B: CloudBackend> {
    backend: B,
    strategy: ConflictStrategy,
}

impl<B: CloudBackend> SyncEngine<B> {
    /// Create a new sync engine with the default strategy (last-write-wins).
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            strategy: ConflictStrategy::default(),
        }
    }

    /// Create a sync engine with a specific conflict strategy.
    pub fn with_strategy(backend: B, strategy: ConflictStrategy) -> Self {
        Self { backend, strategy }
    }

    /// Returns a reference to the underlying backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Generate a sync plan by comparing local and remote profile states.
    ///
    /// Profiles present only locally are scheduled for upload. Profiles present
    /// only remotely are scheduled for download. Profiles that differ are either
    /// auto-resolved (last-write-wins) or flagged as conflicts (manual).
    pub fn plan(&self, local: &[ProfileSyncState], remote: &[ProfileSyncState]) -> SyncPlan {
        let mut plan = SyncPlan::default();

        let remote_map: HashMap<&str, &ProfileSyncState> =
            remote.iter().map(|r| (r.id.as_str(), r)).collect();
        let local_map: HashMap<&str, &ProfileSyncState> =
            local.iter().map(|l| (l.id.as_str(), l)).collect();

        for local_state in local {
            match remote_map.get(local_state.id.as_str()) {
                None => {
                    plan.uploads.push(ProfileAction {
                        profile_id: local_state.id.clone(),
                        version_hash: local_state.version_hash.clone(),
                    });
                }
                Some(remote_state) => {
                    if local_state.version_hash == remote_state.version_hash {
                        continue;
                    }
                    match self.strategy {
                        ConflictStrategy::LastWriteWins => {
                            if local_state.updated_at >= remote_state.updated_at {
                                plan.uploads.push(ProfileAction {
                                    profile_id: local_state.id.clone(),
                                    version_hash: local_state.version_hash.clone(),
                                });
                            } else {
                                plan.downloads.push(ProfileAction {
                                    profile_id: local_state.id.clone(),
                                    version_hash: remote_state.version_hash.clone(),
                                });
                            }
                        }
                        ConflictStrategy::Manual => {
                            plan.conflicts.push(SyncConflict {
                                profile_id: local_state.id.clone(),
                                local_version: local_state.clone(),
                                remote_version: (*remote_state).clone(),
                            });
                        }
                    }
                }
            }
        }

        for remote_state in remote {
            if !local_map.contains_key(remote_state.id.as_str()) {
                plan.downloads.push(ProfileAction {
                    profile_id: remote_state.id.clone(),
                    version_hash: remote_state.version_hash.clone(),
                });
            }
        }

        plan
    }

    /// Upload profile data to the backend.
    pub async fn upload(&self, id: &str, data: &[u8]) -> Result<()> {
        self.backend.upload(id, data).await
    }

    /// Download profile data from the backend.
    pub async fn download(&self, id: &str) -> Result<Vec<u8>> {
        self.backend.download(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MockCloudBackend;

    fn state(id: &str, hash: &str, ts: u64) -> ProfileSyncState {
        ProfileSyncState {
            id: id.to_string(),
            version_hash: hash.to_string(),
            updated_at: ts,
        }
    }

    // ── SyncPlan basics ─────────────────────────────────────────────────────

    #[test]
    fn test_sync_plan_empty() {
        let plan = SyncPlan::default();
        assert!(plan.is_empty());
    }

    // ── No-conflict scenarios ───────────────────────────────────────────────

    #[test]
    fn test_sync_upload_only() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state("p1", "hash-a", 1000)];
        let remote = vec![];
        let plan = engine.plan(&local, &remote);

        assert_eq!(plan.uploads.len(), 1);
        assert_eq!(plan.uploads[0].profile_id, "p1");
        assert!(plan.downloads.is_empty());
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn test_sync_download_only() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![];
        let remote = vec![state("p1", "hash-a", 1000)];
        let plan = engine.plan(&local, &remote);

        assert!(plan.uploads.is_empty());
        assert_eq!(plan.downloads.len(), 1);
        assert_eq!(plan.downloads[0].profile_id, "p1");
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn test_sync_bidirectional_no_overlap() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state("p1", "hash-a", 1000)];
        let remote = vec![state("p2", "hash-b", 2000)];
        let plan = engine.plan(&local, &remote);

        assert_eq!(plan.uploads.len(), 1);
        assert_eq!(plan.uploads[0].profile_id, "p1");
        assert_eq!(plan.downloads.len(), 1);
        assert_eq!(plan.downloads[0].profile_id, "p2");
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn test_sync_already_in_sync() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state("p1", "same-hash", 1000)];
        let remote = vec![state("p1", "same-hash", 1000)];
        let plan = engine.plan(&local, &remote);

        assert!(plan.is_empty());
    }

    #[test]
    fn test_sync_same_hash_different_timestamps_still_in_sync() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state("p1", "same-hash", 1000)];
        let remote = vec![state("p1", "same-hash", 2000)];
        let plan = engine.plan(&local, &remote);

        assert!(
            plan.is_empty(),
            "same hash means in sync regardless of timestamp"
        );
    }

    // ── Last-write-wins strategy ────────────────────────────────────────────

    #[test]
    fn test_sync_lww_local_newer_uploads() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state("p1", "local-hash", 2000)];
        let remote = vec![state("p1", "remote-hash", 1000)];
        let plan = engine.plan(&local, &remote);

        assert_eq!(plan.uploads.len(), 1);
        assert_eq!(plan.uploads[0].version_hash, "local-hash");
        assert!(plan.downloads.is_empty());
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn test_sync_lww_remote_newer_downloads() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state("p1", "local-hash", 1000)];
        let remote = vec![state("p1", "remote-hash", 2000)];
        let plan = engine.plan(&local, &remote);

        assert!(plan.uploads.is_empty());
        assert_eq!(plan.downloads.len(), 1);
        assert_eq!(plan.downloads[0].version_hash, "remote-hash");
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn test_sync_lww_equal_timestamp_prefers_local() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state("p1", "local-hash", 1000)];
        let remote = vec![state("p1", "remote-hash", 1000)];
        let plan = engine.plan(&local, &remote);

        assert_eq!(plan.uploads.len(), 1, "equal timestamp should prefer local");
        assert!(plan.downloads.is_empty());
    }

    // ── Manual conflict detection ───────────────────────────────────────────

    #[test]
    fn test_sync_manual_conflict_detection() {
        let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
        let local = vec![state("p1", "local-hash", 1000)];
        let remote = vec![state("p1", "remote-hash", 2000)];
        let plan = engine.plan(&local, &remote);

        assert!(plan.uploads.is_empty());
        assert!(plan.downloads.is_empty());
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].profile_id, "p1");
        assert_eq!(plan.conflicts[0].local_version.version_hash, "local-hash");
        assert_eq!(plan.conflicts[0].remote_version.version_hash, "remote-hash");
    }

    // ── Conflict resolution ─────────────────────────────────────────────────

    #[test]
    fn test_conflict_resolution_use_local() {
        let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
        let local = vec![state("p1", "local-hash", 1000)];
        let remote = vec![state("p1", "remote-hash", 2000)];
        let mut plan = engine.plan(&local, &remote);

        plan.resolve_conflict("p1", ConflictResolution::UseLocal);
        assert!(plan.conflicts.is_empty());
        assert_eq!(plan.uploads.len(), 1);
        assert_eq!(plan.uploads[0].version_hash, "local-hash");
    }

    #[test]
    fn test_conflict_resolution_use_remote() {
        let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
        let local = vec![state("p1", "local-hash", 1000)];
        let remote = vec![state("p1", "remote-hash", 2000)];
        let mut plan = engine.plan(&local, &remote);

        plan.resolve_conflict("p1", ConflictResolution::UseRemote);
        assert!(plan.conflicts.is_empty());
        assert_eq!(plan.downloads.len(), 1);
        assert_eq!(plan.downloads[0].version_hash, "remote-hash");
    }

    #[test]
    fn test_conflict_resolution_skip() {
        let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
        let local = vec![state("p1", "local-hash", 1000)];
        let remote = vec![state("p1", "remote-hash", 2000)];
        let mut plan = engine.plan(&local, &remote);

        plan.resolve_conflict("p1", ConflictResolution::Skip);
        assert!(plan.conflicts.is_empty());
        assert!(plan.uploads.is_empty());
        assert!(plan.downloads.is_empty());
    }

    #[test]
    fn test_conflict_resolution_nonexistent_is_noop() {
        let mut plan = SyncPlan::default();
        plan.resolve_conflict("nonexistent", ConflictResolution::UseLocal);
        assert!(plan.is_empty());
    }

    // ── Mixed scenario ──────────────────────────────────────────────────────

    #[test]
    fn test_sync_mixed_scenario() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![
            state("local-only", "hash-1", 1000),
            state("in-sync", "same-hash", 1000),
            state("local-newer", "hash-new", 2000),
            state("remote-newer", "hash-old", 1000),
        ];
        let remote = vec![
            state("remote-only", "hash-2", 1000),
            state("in-sync", "same-hash", 1000),
            state("local-newer", "hash-old-r", 1000),
            state("remote-newer", "hash-new-r", 2000),
        ];
        let plan = engine.plan(&local, &remote);

        assert_eq!(plan.uploads.len(), 2);
        assert_eq!(plan.downloads.len(), 2);
        assert!(plan.conflicts.is_empty());

        let upload_ids: Vec<&str> = plan.uploads.iter().map(|a| a.profile_id.as_str()).collect();
        assert!(upload_ids.contains(&"local-only"));
        assert!(upload_ids.contains(&"local-newer"));

        let download_ids: Vec<&str> = plan
            .downloads
            .iter()
            .map(|a| a.profile_id.as_str())
            .collect();
        assert!(download_ids.contains(&"remote-only"));
        assert!(download_ids.contains(&"remote-newer"));
    }

    // ── Backend delegation ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sync_engine_upload_via_backend() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        engine.upload("p1", b"profile data").await.unwrap();
        let data = engine.backend().download("p1").await.unwrap();
        assert_eq!(data, b"profile data");
    }

    #[tokio::test]
    async fn test_sync_engine_download_via_backend() {
        let engine = SyncEngine::new(MockCloudBackend::new());
        engine
            .backend()
            .upload("p1", b"profile data")
            .await
            .unwrap();
        let data = engine.download("p1").await.unwrap();
        assert_eq!(data, b"profile data");
    }
}
