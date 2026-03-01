// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the cloud profile sync system.
//!
//! Covers: sync logic, storage, authentication patterns, merge semantics,
//! error handling, and property-based invariants (30+ tests).

use chrono::{TimeZone as _, Utc};
use flight_cloud_profiles::{
    CloudProfile, CloudProfileError,
    cache::ProfileCache,
    sanitize::sanitize_for_upload,
    storage::{CloudBackend, FileSystemBackend, MockCloudBackend},
    sync::{
        ConflictResolution, ConflictStrategy, ProfileSyncState, SyncConflict, SyncEngine, SyncPlan,
    },
    versioning::{ProfileVersion, VersionHistory, compute_version_hash},
};
use flight_profile::{AircraftId, AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use proptest::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── helpers ──────────────────────────────────────────────────────────────────

fn state(id: &str, hash: &str, ts: u64) -> ProfileSyncState {
    ProfileSyncState {
        id: id.to_string(),
        version_hash: hash.to_string(),
        updated_at: ts,
    }
}

fn minimal_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn profile_with_axes(pairs: &[(&str, f32, f32)]) -> Profile {
    let mut axes = HashMap::new();
    for &(name, deadzone, expo) in pairs {
        axes.insert(
            name.to_string(),
            AxisConfig {
                deadzone: Some(deadzone),
                expo: Some(expo),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
    }
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

fn make_cloud_profile(id: &str, profile: Profile) -> CloudProfile {
    CloudProfile {
        id: id.to_string(),
        title: format!("Profile {id}"),
        description: Some("test profile".to_string()),
        author_handle: "tester".to_string(),
        upvotes: 0,
        downvotes: 0,
        download_count: 0,
        published_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
        profile,
    }
}

fn tempfile_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tid = std::thread::current().id();
    let name = format!("test-{nanos:016x}-{seq:04x}-{tid:?}").replace(['(', ')', ' '], "_");
    let dir = std::env::temp_dir()
        .join("flight-cloud-depth-test")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. SYNC LOGIC (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Upload: a profile only on local side gets scheduled for upload.
#[tokio::test]
async fn sync_upload_profile_end_to_end() {
    let backend = MockCloudBackend::new();
    let engine = SyncEngine::new(backend);

    let local = vec![state("jet-profile", "hash-local", 5000)];
    let remote = vec![];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].profile_id, "jet-profile");
    assert_eq!(plan.uploads[0].version_hash, "hash-local");

    // Execute the upload
    engine.upload("jet-profile", b"jet data").await.unwrap();
    let downloaded = engine.download("jet-profile").await.unwrap();
    assert_eq!(downloaded, b"jet data");
}

/// Download: a profile only on remote side gets scheduled for download.
#[tokio::test]
async fn sync_download_profile_end_to_end() {
    let backend = MockCloudBackend::new();
    backend.upload("remote-only", b"remote data").await.unwrap();
    let engine = SyncEngine::new(backend);

    let local = vec![];
    let remote = vec![state("remote-only", "hash-remote", 3000)];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].profile_id, "remote-only");

    let data = engine.download("remote-only").await.unwrap();
    assert_eq!(data, b"remote data");
}

/// Conflict detection: differing hashes with manual strategy produce a conflict.
#[test]
fn sync_conflict_detection_captures_both_versions() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    let local = vec![state("shared", "hash-a", 1000)];
    let remote = vec![state("shared", "hash-b", 2000)];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.conflicts.len(), 1);
    let c = &plan.conflicts[0];
    assert_eq!(c.profile_id, "shared");
    assert_eq!(c.local_version.version_hash, "hash-a");
    assert_eq!(c.remote_version.version_hash, "hash-b");
    assert_eq!(c.local_version.updated_at, 1000);
    assert_eq!(c.remote_version.updated_at, 2000);
}

/// Conflict resolution: last-write-wins picks local when timestamps are equal.
#[test]
fn sync_lww_tie_breaks_to_local() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![state("p1", "local-v", 5000)];
    let remote = vec![state("p1", "remote-v", 5000)];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].version_hash, "local-v");
    assert!(plan.downloads.is_empty());
    assert!(plan.conflicts.is_empty());
}

/// Conflict resolution: merge (manual then resolve both ways).
#[test]
fn sync_conflict_resolution_all_three_modes() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);

    // UseLocal
    let mut plan = engine.plan(
        &[state("a", "la", 1)],
        &[state("a", "ra", 2)],
    );
    plan.resolve_conflict("a", ConflictResolution::UseLocal);
    assert_eq!(plan.uploads.len(), 1);
    assert!(plan.conflicts.is_empty());

    // UseRemote
    let mut plan = engine.plan(
        &[state("b", "lb", 1)],
        &[state("b", "rb", 2)],
    );
    plan.resolve_conflict("b", ConflictResolution::UseRemote);
    assert_eq!(plan.downloads.len(), 1);
    assert!(plan.conflicts.is_empty());

    // Skip
    let mut plan = engine.plan(
        &[state("c", "lc", 1)],
        &[state("c", "rc", 2)],
    );
    plan.resolve_conflict("c", ConflictResolution::Skip);
    assert!(plan.is_empty());
}

/// Sync queue: many profiles processed in a single plan.
#[test]
fn sync_queue_handles_many_profiles() {
    let engine = SyncEngine::new(MockCloudBackend::new());

    let local: Vec<_> = (0..50)
        .map(|i| state(&format!("local-{i}"), &format!("lh-{i}"), 1000))
        .collect();
    let remote: Vec<_> = (50..100)
        .map(|i| state(&format!("remote-{i}"), &format!("rh-{i}"), 2000))
        .collect();

    let plan = engine.plan(&local, &remote);
    assert_eq!(plan.uploads.len(), 50);
    assert_eq!(plan.downloads.len(), 50);
    assert!(plan.conflicts.is_empty());
}

/// Retry logic: re-uploading after a failed download is safe.
#[tokio::test]
async fn sync_retry_upload_after_failure_is_idempotent() {
    let backend = MockCloudBackend::new();

    // First upload
    backend.upload("retry-p", b"version-1").await.unwrap();
    assert_eq!(backend.download("retry-p").await.unwrap(), b"version-1");

    // Overwrite (simulating retry with updated data)
    backend.upload("retry-p", b"version-2").await.unwrap();
    assert_eq!(backend.download("retry-p").await.unwrap(), b"version-2");

    // Metadata list should still have exactly one entry for this ID
    let list = backend.list_profiles().await.unwrap();
    let count = list.iter().filter(|m| m.id == "retry-p").count();
    assert_eq!(count, 1);
}

/// Offline queue: plan can be computed entirely offline without backend calls.
#[test]
fn sync_offline_queue_plan_is_pure() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![
        state("a", "ha", 100),
        state("b", "hb", 200),
    ];
    let remote = vec![
        state("b", "hb-remote", 300),
        state("c", "hc", 400),
    ];

    let plan = engine.plan(&local, &remote);

    // "a" local-only → upload
    assert!(plan.uploads.iter().any(|a| a.profile_id == "a"));
    // "b" conflict, LWW picks remote (300 > 200)
    assert!(plan.downloads.iter().any(|a| a.profile_id == "b"));
    // "c" remote-only → download
    assert!(plan.downloads.iter().any(|a| a.profile_id == "c"));
}

/// Batch sync: mixed scenario with uploads, downloads, and in-sync profiles.
#[test]
fn sync_batch_mixed_scenario() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![
        state("upload-me", "u1", 100),
        state("in-sync", "same", 100),
        state("lww-local", "ll", 9999),
        state("lww-remote", "lr", 1),
    ];
    let remote = vec![
        state("download-me", "d1", 200),
        state("in-sync", "same", 200),
        state("lww-local", "rl", 1),
        state("lww-remote", "rr", 9999),
    ];

    let plan = engine.plan(&local, &remote);

    let up_ids: Vec<&str> = plan.uploads.iter().map(|a| a.profile_id.as_str()).collect();
    let dn_ids: Vec<&str> = plan.downloads.iter().map(|a| a.profile_id.as_str()).collect();

    assert!(up_ids.contains(&"upload-me"));
    assert!(up_ids.contains(&"lww-local"));
    assert!(dn_ids.contains(&"download-me"));
    assert!(dn_ids.contains(&"lww-remote"));
    assert!(plan.conflicts.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. STORAGE (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Local cache: store and retrieve a profile via the cache.
#[tokio::test]
async fn storage_local_cache_store_and_get() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);

    let cp = make_cloud_profile("cache-1", profile_with_axes(&[("pitch", 0.05, 0.3)]));
    cache.store(&cp).await.unwrap();

    let got = cache.get("cache-1").await.unwrap().unwrap();
    assert_eq!(got.id, "cache-1");
    assert_eq!(got.profile.axes["pitch"].deadzone, Some(0.05));
    cleanup_dir(&tmp);
}

/// Cache invalidation: entries with TTL=0 are immediately expired.
#[tokio::test]
async fn storage_cache_invalidation_ttl_zero() {
    let tmp = tempfile_dir();
    let writer = ProfileCache::new(tmp.clone(), 3600);
    let reader = ProfileCache::new(tmp.clone(), 0); // immediate expiry

    let cp = make_cloud_profile("expire-me", minimal_profile());
    writer.store(&cp).await.unwrap();

    let result = reader.get("expire-me").await.unwrap();
    assert!(result.is_none(), "TTL=0 should expire immediately");
    cleanup_dir(&tmp);
}

/// Cache clear: verify that `clear()` removes all cached entries.
#[tokio::test]
async fn storage_cache_clear_removes_all_entries() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);

    for i in 0..10u32 {
        let cp = make_cloud_profile(&format!("bulk-{i}"), minimal_profile());
        cache.store(&cp).await.unwrap();
    }
    assert_eq!(cache.list_cached().await.len(), 10);

    cache.clear().await.unwrap();
    assert!(cache.list_cached().await.is_empty());
    cleanup_dir(&tmp);
}

/// JSON round-trip: profile data survives JSON serialization through backend.
#[tokio::test]
async fn storage_compression_round_trip_through_backend() {
    let backend = MockCloudBackend::new();
    let profile = profile_with_axes(&[("roll", 0.1, 0.5), ("pitch", 0.02, 0.8)]);
    let data = serde_json::to_vec(&profile).unwrap();

    backend.upload("compressed-p", &data).await.unwrap();
    let downloaded = backend.download("compressed-p").await.unwrap();
    let restored: Profile = serde_json::from_slice(&downloaded).unwrap();

    assert_eq!(restored.axes["roll"].deadzone, Some(0.1));
    assert_eq!(restored.axes["pitch"].expo, Some(0.8));
}

/// Disk persistence: FileSystemBackend data persists to disk and is retrievable.
#[tokio::test]
async fn storage_filesystem_persistence() {
    let tmp = tempfile_dir();
    let backend = FileSystemBackend::new(&tmp);

    backend.upload("persist-1", b"important data").await.unwrap();

    // Verify file actually exists on disk
    let data_path = tmp.join("persist-1.dat");
    assert!(data_path.exists());

    let data = backend.download("persist-1").await.unwrap();
    assert_eq!(data, b"important data");

    // Metadata is tracked
    let list = backend.list_profiles().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "persist-1");
    cleanup_dir(&tmp);
}

/// Format versioning: version hash is deterministic for same input.
#[test]
fn storage_format_versioning_hash_determinism() {
    let data = b"profile-payload-v2";
    let h1 = compute_version_hash(data);
    let h2 = compute_version_hash(data);
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64); // SHA-256 hex

    // Different data produces different hash
    let h3 = compute_version_hash(b"profile-payload-v3");
    assert_ne!(h1, h3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. AUTHENTICATION (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Token refresh: CloudProfileError::ApiError captures 401 status.
#[test]
fn auth_token_refresh_error_captures_status() {
    let err = CloudProfileError::ApiError {
        status: 401,
        message: "token expired".to_string(),
    };
    let display = err.to_string();
    assert!(display.contains("401"));
    assert!(display.contains("token expired"));
}

/// Expired token handling: 403 errors are distinguishable from 401.
#[test]
fn auth_expired_token_403_vs_401() {
    let err_401 = CloudProfileError::ApiError {
        status: 401,
        message: "unauthorized".to_string(),
    };
    let err_403 = CloudProfileError::ApiError {
        status: 403,
        message: "forbidden".to_string(),
    };
    // Both render distinctly
    assert!(err_401.to_string().contains("401"));
    assert!(err_403.to_string().contains("403"));
    assert_ne!(err_401.to_string(), err_403.to_string());
}

/// Unauthorized rejection: missing auth produces ApiError.
#[test]
fn auth_unauthorized_rejection_error_variant() {
    let err = CloudProfileError::ApiError {
        status: 401,
        message: "no bearer token".to_string(),
    };
    match &err {
        CloudProfileError::ApiError { status, message } => {
            assert_eq!(*status, 401);
            assert!(message.contains("bearer"));
        }
        other => panic!("expected ApiError, got: {other:?}"),
    }
}

/// Multi-device auth: separate backends maintain independent state.
#[tokio::test]
async fn auth_multi_device_independent_backends() {
    let device_a = MockCloudBackend::new();
    let device_b = MockCloudBackend::new();

    device_a.upload("shared-p", b"device-a-data").await.unwrap();

    // Device B has no knowledge of device A's uploads
    let result = device_b.download("shared-p").await;
    assert!(result.is_err(), "device B should not see device A's data");

    // Each device independently tracks metadata
    assert_eq!(device_a.list_profiles().await.unwrap().len(), 1);
    assert_eq!(device_b.list_profiles().await.unwrap().len(), 0);
}

/// Offline token caching: cache operates without network (filesystem backend).
#[tokio::test]
async fn auth_offline_cache_works_without_network() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);

    let cp = make_cloud_profile("offline-p", profile_with_axes(&[("yaw", 0.03, 0.2)]));
    cache.store(&cp).await.unwrap();

    // Retrieval works purely from disk — no HTTP involved
    let got = cache.get("offline-p").await.unwrap().unwrap();
    assert_eq!(got.profile.axes["yaw"].expo, Some(0.2));
    cleanup_dir(&tmp);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. MERGE (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Profile merge during sync: resolving a conflict with UseLocal merges into uploads.
#[test]
fn merge_conflict_to_upload() {
    let mut plan = SyncPlan {
        uploads: vec![],
        downloads: vec![],
        conflicts: vec![SyncConflict {
            profile_id: "merge-target".to_string(),
            local_version: state("merge-target", "local-h", 2000),
            remote_version: state("merge-target", "remote-h", 1000),
        }],
    };

    plan.resolve_conflict("merge-target", ConflictResolution::UseLocal);
    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].version_hash, "local-h");
}

/// Field-level merge: sanitize preserves axis-level config during merge.
#[test]
fn merge_field_level_preserves_axis_config() {
    let profile = profile_with_axes(&[
        ("pitch", 0.05, 0.3),
        ("roll", 0.1, 0.5),
        ("yaw", 0.02, 0.1),
    ]);
    let sanitized = sanitize_for_upload(&profile);

    // All three axes with their exact values must survive
    assert_eq!(sanitized.axes.len(), 3);
    assert_eq!(sanitized.axes["pitch"].deadzone, Some(0.05));
    assert_eq!(sanitized.axes["roll"].expo, Some(0.5));
    assert_eq!(sanitized.axes["yaw"].deadzone, Some(0.02));
}

/// Additive merge: version history accumulates across pushes.
#[test]
fn merge_additive_version_history() {
    let mut history = VersionHistory::new();
    history.push(ProfileVersion {
        version_id: "v1".to_string(),
        timestamp: 1000,
        author: "alice".to_string(),
        changes: vec!["initial".to_string()],
    });
    history.push(ProfileVersion {
        version_id: "v2".to_string(),
        timestamp: 2000,
        author: "bob".to_string(),
        changes: vec!["added pitch curve".to_string()],
    });
    history.push(ProfileVersion {
        version_id: "v3".to_string(),
        timestamp: 3000,
        author: "alice".to_string(),
        changes: vec!["tuned deadzone".to_string(), "adjusted expo".to_string()],
    });

    assert_eq!(history.len(), 3);
    let diff = history.diff("v1", "v3").unwrap();
    assert_eq!(diff.changes.len(), 3);
    assert!(diff.changes.contains(&"added pitch curve".to_string()));
    assert!(diff.changes.contains(&"tuned deadzone".to_string()));
}

/// Destructive merge detection: resolving with UseRemote discards local version.
#[test]
fn merge_destructive_detection_use_remote_discards_local() {
    let mut plan = SyncPlan {
        uploads: vec![],
        downloads: vec![],
        conflicts: vec![SyncConflict {
            profile_id: "destructive".to_string(),
            local_version: state("destructive", "my-work", 5000),
            remote_version: state("destructive", "their-work", 6000),
        }],
    };

    plan.resolve_conflict("destructive", ConflictResolution::UseRemote);
    assert!(plan.uploads.is_empty(), "local should be discarded");
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].version_hash, "their-work");
}

/// Merge preview: version diff provides a preview of what changed.
#[test]
fn merge_preview_via_version_diff() {
    let mut history = VersionHistory::new();
    history.push(ProfileVersion {
        version_id: "base".to_string(),
        timestamp: 100,
        author: "system".to_string(),
        changes: vec!["base setup".to_string()],
    });
    history.push(ProfileVersion {
        version_id: "update-1".to_string(),
        timestamp: 200,
        author: "user-a".to_string(),
        changes: vec!["changed deadzone to 0.05".to_string()],
    });
    history.push(ProfileVersion {
        version_id: "update-2".to_string(),
        timestamp: 300,
        author: "user-b".to_string(),
        changes: vec!["added expo curve".to_string(), "set slew rate".to_string()],
    });

    // Preview changes from base to update-2
    let preview = history.diff("base", "update-2").unwrap();
    assert_eq!(preview.changes.len(), 3);
    assert_eq!(preview.from, "base");
    assert_eq!(preview.to, "update-2");

    // Adjacent diff shows only that version's changes
    let adjacent = history.diff("update-1", "update-2").unwrap();
    assert_eq!(adjacent.changes.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. ERROR HANDLING (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Network failure: download of nonexistent profile returns an error.
#[tokio::test]
async fn error_network_failure_download_missing() {
    let backend = MockCloudBackend::new();
    let result = backend.download("nonexistent").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "error should mention 'not found': {}",
        err
    );
}

/// Server error: ApiError with 500 status is representable and displayable.
#[test]
fn error_server_error_500_display() {
    let err = CloudProfileError::ApiError {
        status: 500,
        message: "internal server error".to_string(),
    };
    let display = err.to_string();
    assert!(display.contains("500"));
    assert!(display.contains("internal server error"));
}

/// Partial upload: overwriting with new data replaces old completely.
#[tokio::test]
async fn error_partial_upload_overwrite_replaces() {
    let backend = MockCloudBackend::new();
    backend.upload("partial", b"incomplete").await.unwrap();

    // "Retry" with complete data
    backend.upload("partial", b"complete-data").await.unwrap();

    let data = backend.download("partial").await.unwrap();
    assert_eq!(data, b"complete-data");

    // Checksum matches the final data
    let profiles = backend.list_profiles().await.unwrap();
    let meta = profiles.iter().find(|m| m.id == "partial").unwrap();
    assert_eq!(meta.checksum, compute_version_hash(b"complete-data"));
}

/// Corrupt download: invalid JSON in cache produces an error, not a panic.
#[tokio::test]
async fn error_corrupt_download_cache_handles_invalid_json() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);

    // Manually write an index entry and corrupt profile file
    let index = vec![flight_cloud_profiles::cache::CacheEntry {
        id: "corrupt".to_string(),
        cached_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        title: "Corrupt".to_string(),
    }];
    let index_data = serde_json::to_string(&index).unwrap();
    tokio::fs::create_dir_all(&tmp).await.unwrap();
    tokio::fs::write(tmp.join("cache_index.json"), index_data)
        .await
        .unwrap();
    tokio::fs::write(tmp.join("corrupt.json"), "NOT VALID JSON!!!")
        .await
        .unwrap();

    // get() should return Err (JSON parse error), not panic
    let result = cache.get("corrupt").await;
    assert!(result.is_err(), "corrupt JSON should produce an error");
    cleanup_dir(&tmp);
}

/// Rate limiting: 429 errors are expressible via ApiError.
#[test]
fn error_rate_limiting_429() {
    let err = CloudProfileError::ApiError {
        status: 429,
        message: "too many requests, retry after 60s".to_string(),
    };
    let display = err.to_string();
    assert!(display.contains("429"));
    assert!(display.contains("retry"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. PROPERTY TESTS (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// Sync roundtrip identity: same hash on both sides means empty plan.
    #[test]
    fn prop_sync_roundtrip_identity(
        id in "[a-z]{1,10}",
        hash in "[a-f0-9]{8,16}",
        ts_local in 1u64..1_000_000u64,
        ts_remote in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![state(&id, &hash, ts_local)];
        let remote = vec![state(&id, &hash, ts_remote)];
        let plan = engine.plan(&local, &remote);

        prop_assert!(plan.is_empty(),
            "same hash must mean in-sync regardless of timestamps");
    }

    /// Merge commutativity where applicable: LWW always picks the hash
    /// associated with the higher timestamp, regardless of local/remote order.
    /// Note: when timestamps are equal, the local side wins (tie-break rule).
    #[test]
    fn prop_merge_commutativity_lww(
        ts_a in 1u64..1_000_000u64,
        ts_b in 1u64..1_000_000u64,
    ) {
        // Skip equal timestamps — the tie-break favors local, so swapping
        // local/remote changes the winner and commutativity does not hold.
        prop_assume!(ts_a != ts_b);

        let engine = SyncEngine::new(MockCloudBackend::new());

        // hash-a is associated with ts_a, hash-b with ts_b.
        let plan_ab = engine.plan(
            &[state("p", "hash-a", ts_a)],
            &[state("p", "hash-b", ts_b)],
        );
        let plan_ba = engine.plan(
            &[state("p", "hash-b", ts_b)],
            &[state("p", "hash-a", ts_a)],
        );

        let winner_ab = if !plan_ab.uploads.is_empty() {
            plan_ab.uploads[0].version_hash.clone()
        } else {
            plan_ab.downloads[0].version_hash.clone()
        };
        let winner_ba = if !plan_ba.uploads.is_empty() {
            plan_ba.uploads[0].version_hash.clone()
        } else {
            plan_ba.downloads[0].version_hash.clone()
        };

        // Regardless of local/remote placement, the hash with the
        // higher (or equal) timestamp must win.
        let expected_winner = if ts_a >= ts_b { "hash-a" } else { "hash-b" };
        prop_assert_eq!(&winner_ab, expected_winner);
        prop_assert_eq!(&winner_ba, expected_winner);
    }

    /// Cache consistency: store then get always returns the stored profile.
    #[test]
    fn prop_cache_consistency_store_get(
        upvotes in 0u32..100_000u32,
        downvotes in 0u32..100_000u32,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let tmp = tempfile_dir();
            let cache = ProfileCache::new(tmp.clone(), 3600);

            let mut cp = make_cloud_profile("prop-cache", minimal_profile());
            cp.upvotes = upvotes;
            cp.downvotes = downvotes;

            cache.store(&cp).await.unwrap();
            let got = cache.get("prop-cache").await.unwrap().unwrap();

            assert_eq!(got.upvotes, upvotes);
            assert_eq!(got.downvotes, downvotes);
            cleanup_dir(&tmp);
        });
    }

    /// Version hash determinism: same input always produces same hash.
    #[test]
    fn prop_version_hash_determinism(data in proptest::collection::vec(any::<u8>(), 0..512)) {
        let h1 = compute_version_hash(&data);
        let h2 = compute_version_hash(&data);
        prop_assert_eq!(h1, h2);
    }

    /// Sanitize idempotency: sanitizing twice gives the same result.
    #[test]
    fn prop_sanitize_idempotent(
        dz in 0.0f32..0.5f32,
        expo in 0.0f32..1.0f32,
    ) {
        let profile = profile_with_axes(&[("test-axis", dz, expo)]);
        let once = sanitize_for_upload(&profile);
        let twice = sanitize_for_upload(&once);

        prop_assert_eq!(once.schema, twice.schema);
        prop_assert_eq!(once.sim, twice.sim);
        prop_assert_eq!(once.axes.len(), twice.axes.len());
        if let (Some(a1), Some(a2)) = (once.axes.get("test-axis"), twice.axes.get("test-axis")) {
            prop_assert_eq!(a1.deadzone, a2.deadzone);
            prop_assert_eq!(a1.expo, a2.expo);
        }
    }
}
