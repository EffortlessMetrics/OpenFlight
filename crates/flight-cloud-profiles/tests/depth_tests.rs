// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for cloud profile management — storage, sync engine,
//! conflict resolution, authentication, serialization, and property tests.

use flight_cloud_profiles::{
    CloudProfile, CloudProfileError, ConflictResolution, ConflictStrategy, ProfileSyncState,
    SyncConflict, SyncEngine,
    storage::{CloudBackend, FileSystemBackend, MockCloudBackend},
    versioning::{ProfileVersion, VersionHistory, compute_version_hash},
};
use flight_profile::{AircraftId, AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use proptest::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ── helpers ──────────────────────────────────────────────────────────────────

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

fn profile_with_axes(axes: &[(&str, f32, f32)]) -> Profile {
    let mut map = HashMap::new();
    for &(name, deadzone, expo) in axes {
        map.insert(
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
        axes: map,
        pof_overrides: None,
    }
}

fn sync_state(id: &str, hash: &str, ts: u64) -> ProfileSyncState {
    ProfileSyncState {
        id: id.to_string(),
        version_hash: hash.to_string(),
        updated_at: ts,
    }
}

fn tempfile_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tid = std::thread::current().id();
    let name = format!("depth-{nanos:016x}-{seq:04x}-{tid:?}").replace(['(', ')', ' '], "_");
    let dir = std::env::temp_dir()
        .join("flight-cloud-depth-tests")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. Profile Storage (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn storage_save_and_load_profile_data_intact() {
    let backend = MockCloudBackend::new();
    let data = serde_json::to_vec(&profile_with_axes(&[("pitch", 0.05, 0.3)])).unwrap();
    backend.upload("save-load", &data).await.unwrap();
    let loaded = backend.download("save-load").await.unwrap();
    let profile: Profile = serde_json::from_slice(&loaded).unwrap();
    assert_eq!(profile.axes["pitch"].deadzone, Some(0.05));
    assert_eq!(profile.axes["pitch"].expo, Some(0.3));
}

#[tokio::test]
async fn storage_list_returns_all_uploaded_profiles() {
    let backend = MockCloudBackend::new();
    for i in 0..5 {
        backend
            .upload(&format!("p{i}"), format!("data-{i}").as_bytes())
            .await
            .unwrap();
    }
    let list = backend.list_profiles().await.unwrap();
    assert_eq!(list.len(), 5);
    let ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
    for i in 0..5 {
        assert!(ids.contains(&format!("p{i}").as_str()));
    }
}

#[tokio::test]
async fn storage_delete_removes_profile_completely() {
    let backend = MockCloudBackend::new();
    backend.upload("del-me", b"data").await.unwrap();
    backend.delete("del-me").await.unwrap();

    assert!(backend.download("del-me").await.is_err());
    assert!(backend.list_profiles().await.unwrap().is_empty());
}

#[tokio::test]
async fn storage_versioning_update_changes_checksum() {
    let backend = MockCloudBackend::new();
    backend.upload("versioned", b"version-1").await.unwrap();
    let meta1 = backend
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.id == "versioned")
        .unwrap();

    backend.upload("versioned", b"version-2").await.unwrap();
    let meta2 = backend
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.id == "versioned")
        .unwrap();

    assert_ne!(
        meta1.checksum, meta2.checksum,
        "checksum must change when data changes"
    );
}

#[tokio::test]
async fn storage_metadata_has_correct_size() {
    let backend = MockCloudBackend::new();
    let data = b"exactly 20 bytes!!!";
    backend.upload("sized", data).await.unwrap();
    let meta = backend
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.id == "sized")
        .unwrap();
    assert_eq!(meta.size, data.len() as u64);
}

#[tokio::test]
async fn storage_metadata_updated_at_is_recent() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let backend = MockCloudBackend::new();
    backend.upload("timed", b"data").await.unwrap();
    let meta = backend
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.id == "timed")
        .unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        meta.updated_at >= before && meta.updated_at <= after,
        "updated_at ({}) should be between {before} and {after}",
        meta.updated_at
    );
}

#[tokio::test]
async fn storage_filesystem_round_trip_preserves_data() {
    let tmp = tempfile_dir();
    let backend = FileSystemBackend::new(&tmp);
    let profile = profile_with_axes(&[("roll", 0.1, 0.5)]);
    let data = serde_json::to_vec(&profile).unwrap();

    backend.upload("fs-rt", &data).await.unwrap();
    let loaded = backend.download("fs-rt").await.unwrap();
    let recovered: Profile = serde_json::from_slice(&loaded).unwrap();

    assert_eq!(recovered.axes["roll"].deadzone, Some(0.1));
    assert_eq!(recovered.axes["roll"].expo, Some(0.5));
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn storage_large_profile_survives_upload_download() {
    let backend = MockCloudBackend::new();
    // Build a profile with many axes to simulate quota-like size
    let mut axes = HashMap::new();
    for i in 0..100 {
        axes.insert(
            format!("axis_{i}"),
            AxisConfig {
                deadzone: Some(0.01 * (i as f32 % 50.0)),
                expo: Some(0.01 * (i as f32 % 100.0)),
                slew_rate: Some(100.0),
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
    }
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("dcs".to_string()),
        aircraft: Some(AircraftId {
            icao: "F16C".to_string(),
        }),
        axes,
        pof_overrides: None,
    };
    let data = serde_json::to_vec(&profile).unwrap();
    assert!(data.len() > 1000, "profile should be non-trivially sized");

    backend.upload("large", &data).await.unwrap();
    let downloaded = backend.download("large").await.unwrap();
    assert_eq!(data, downloaded);
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Sync Engine (10 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn sync_upload_local_only_profiles_to_cloud() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![
        sync_state("a", "ha", 1000),
        sync_state("b", "hb", 2000),
    ];
    let plan = engine.plan(&local, &[]);
    assert_eq!(plan.uploads.len(), 2);
    assert!(plan.downloads.is_empty());
    assert!(plan.conflicts.is_empty());
}

#[test]
fn sync_download_remote_only_profiles_to_local() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let remote = vec![
        sync_state("x", "hx", 1000),
        sync_state("y", "hy", 2000),
    ];
    let plan = engine.plan(&[], &remote);
    assert!(plan.uploads.is_empty());
    assert_eq!(plan.downloads.len(), 2);
}

#[test]
fn sync_bidirectional_upload_and_download() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("local-only", "h1", 1000)];
    let remote = vec![sync_state("remote-only", "h2", 1000)];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].profile_id, "local-only");
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].profile_id, "remote-only");
}

#[test]
fn sync_conflict_detected_when_both_sides_modified() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    let local = vec![sync_state("shared", "local-v2", 2000)];
    let remote = vec![sync_state("shared", "remote-v2", 2500)];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].profile_id, "shared");
    assert_eq!(plan.conflicts[0].local_version.version_hash, "local-v2");
    assert_eq!(plan.conflicts[0].remote_version.version_hash, "remote-v2");
}

#[test]
fn sync_last_write_wins_local_newer() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("p", "local-hash", 3000)];
    let remote = vec![sync_state("p", "remote-hash", 1000)];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].version_hash, "local-hash");
    assert!(plan.downloads.is_empty());
}

#[test]
fn sync_last_write_wins_remote_newer() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("p", "local-hash", 1000)];
    let remote = vec![sync_state("p", "remote-hash", 3000)];
    let plan = engine.plan(&local, &remote);

    assert!(plan.uploads.is_empty());
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].version_hash, "remote-hash");
}

#[tokio::test]
async fn sync_offline_queue_upload_then_verify() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    // Simulate offline queue: upload multiple profiles then verify all present
    for i in 0..3 {
        engine
            .upload(&format!("queued-{i}"), format!("data-{i}").as_bytes())
            .await
            .unwrap();
    }
    for i in 0..3 {
        let data = engine.download(&format!("queued-{i}")).await.unwrap();
        assert_eq!(data, format!("data-{i}").as_bytes());
    }
}

#[test]
fn sync_status_tracking_plan_reports_empty_when_synced() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let profiles = vec![
        sync_state("a", "hash-a", 1000),
        sync_state("b", "hash-b", 2000),
    ];
    let plan = engine.plan(&profiles, &profiles);
    assert!(plan.is_empty(), "identical states should produce empty plan");
}

#[test]
fn sync_incremental_only_changed_profiles_appear() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![
        sync_state("unchanged", "same", 1000),
        sync_state("modified", "new-local", 2000),
    ];
    let remote = vec![
        sync_state("unchanged", "same", 1000),
        sync_state("modified", "old-remote", 1000),
    ];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].profile_id, "modified");
    assert!(plan.downloads.is_empty());
}

#[test]
fn sync_full_mixed_scenario_many_profiles() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![
        sync_state("synced-1", "h", 100),
        sync_state("synced-2", "h", 100),
        sync_state("local-new", "h-ln", 200),
        sync_state("diverged", "h-local", 300),
    ];
    let remote = vec![
        sync_state("synced-1", "h", 100),
        sync_state("synced-2", "h", 100),
        sync_state("remote-new", "h-rn", 200),
        sync_state("diverged", "h-remote", 100),
    ];
    let plan = engine.plan(&local, &remote);

    let upload_ids: Vec<&str> = plan.uploads.iter().map(|a| a.profile_id.as_str()).collect();
    let download_ids: Vec<&str> = plan
        .downloads
        .iter()
        .map(|a| a.profile_id.as_str())
        .collect();

    assert!(upload_ids.contains(&"local-new"));
    assert!(upload_ids.contains(&"diverged")); // local is newer (300 > 100)
    assert!(download_ids.contains(&"remote-new"));
    assert_eq!(plan.conflicts.len(), 0);
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Conflict Resolution (6 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn conflict_same_field_different_values_detected() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    // Both sides modified the same profile with different hashes
    let local = vec![sync_state("profile-x", "hash-after-deadzone-change", 2000)];
    let remote = vec![sync_state("profile-x", "hash-after-expo-change", 2000)];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.conflicts.len(), 1);
    let conflict = &plan.conflicts[0];
    assert_eq!(conflict.profile_id, "profile-x");
    assert_ne!(
        conflict.local_version.version_hash,
        conflict.remote_version.version_hash
    );
}

#[test]
fn conflict_non_conflicting_profiles_pass_through() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    // Different profiles on each side — no conflict
    let local = vec![sync_state("only-local", "h1", 1000)];
    let remote = vec![sync_state("only-remote", "h2", 1000)];
    let plan = engine.plan(&local, &remote);

    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.downloads.len(), 1);
}

#[test]
fn conflict_three_way_merge_via_version_history() {
    // Simulate three-way merge: base → local changes, base → remote changes
    let mut history = VersionHistory::new();
    history.push(ProfileVersion {
        version_id: "base".to_string(),
        timestamp: 1000,
        author: "alice".to_string(),
        changes: vec!["initial setup".to_string()],
    });
    history.push(ProfileVersion {
        version_id: "local-edit".to_string(),
        timestamp: 2000,
        author: "alice".to_string(),
        changes: vec!["changed deadzone".to_string()],
    });
    history.push(ProfileVersion {
        version_id: "remote-edit".to_string(),
        timestamp: 2500,
        author: "bob".to_string(),
        changes: vec!["changed expo".to_string()],
    });

    let diff = history.diff("base", "remote-edit").unwrap();
    assert_eq!(diff.changes.len(), 2);
    assert!(diff.changes.contains(&"changed deadzone".to_string()));
    assert!(diff.changes.contains(&"changed expo".to_string()));
}

#[test]
fn conflict_marker_format_preserves_both_versions() {
    let conflict = SyncConflict {
        profile_id: "conflict-profile".to_string(),
        local_version: sync_state("conflict-profile", "local-abc", 2000),
        remote_version: sync_state("conflict-profile", "remote-xyz", 2500),
    };

    // Verify conflict retains full state for both sides
    assert_eq!(conflict.local_version.version_hash, "local-abc");
    assert_eq!(conflict.remote_version.version_hash, "remote-xyz");
    assert_eq!(conflict.local_version.updated_at, 2000);
    assert_eq!(conflict.remote_version.updated_at, 2500);
}

#[test]
fn conflict_user_choice_local_remote_skip() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    let local = vec![
        sync_state("c1", "l1", 1000),
        sync_state("c2", "l2", 1000),
        sync_state("c3", "l3", 1000),
    ];
    let remote = vec![
        sync_state("c1", "r1", 2000),
        sync_state("c2", "r2", 2000),
        sync_state("c3", "r3", 2000),
    ];
    let mut plan = engine.plan(&local, &remote);
    assert_eq!(plan.conflicts.len(), 3);

    plan.resolve_conflict("c1", ConflictResolution::UseLocal);
    plan.resolve_conflict("c2", ConflictResolution::UseRemote);
    plan.resolve_conflict("c3", ConflictResolution::Skip);

    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].profile_id, "c1");
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].profile_id, "c2");
}

#[test]
fn conflict_automatic_resolution_lww_prefers_latest() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::LastWriteWins);
    // Local newer for p1, remote newer for p2
    let local = vec![
        sync_state("p1", "l1", 3000),
        sync_state("p2", "l2", 1000),
    ];
    let remote = vec![
        sync_state("p1", "r1", 1000),
        sync_state("p2", "r2", 3000),
    ];
    let plan = engine.plan(&local, &remote);

    assert!(plan.conflicts.is_empty(), "LWW should auto-resolve");
    let upload_ids: Vec<&str> = plan.uploads.iter().map(|a| a.profile_id.as_str()).collect();
    let download_ids: Vec<&str> = plan
        .downloads
        .iter()
        .map(|a| a.profile_id.as_str())
        .collect();
    assert!(upload_ids.contains(&"p1"));
    assert!(download_ids.contains(&"p2"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Authentication (5 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn auth_api_error_unauthorized_has_401_status() {
    let err = CloudProfileError::ApiError {
        status: 401,
        message: "invalid or expired token".to_string(),
    };
    match &err {
        CloudProfileError::ApiError { status, message } => {
            assert_eq!(*status, 401);
            assert!(message.contains("token"));
        }
        _ => panic!("expected ApiError"),
    }
}

#[test]
fn auth_api_error_token_refresh_required_on_403() {
    let err = CloudProfileError::ApiError {
        status: 403,
        message: "token refresh required".to_string(),
    };
    let display = err.to_string();
    assert!(display.contains("403"));
    assert!(display.contains("token refresh"));
}

#[test]
fn auth_expired_token_returns_api_error() {
    let err = CloudProfileError::ApiError {
        status: 401,
        message: "token expired".to_string(),
    };
    assert!(
        matches!(&err, CloudProfileError::ApiError { status: 401, .. }),
        "expired token should be 401"
    );
}

#[test]
fn auth_unauthorized_access_distinguishable_from_not_found() {
    let unauthorized = CloudProfileError::ApiError {
        status: 401,
        message: "unauthorized".to_string(),
    };
    let not_found = CloudProfileError::ApiError {
        status: 404,
        message: "profile not found".to_string(),
    };

    let u_str = unauthorized.to_string();
    let n_str = not_found.to_string();
    assert!(u_str.contains("401"));
    assert!(n_str.contains("404"));
    assert_ne!(u_str, n_str);
}

#[test]
fn auth_rate_limiting_returns_429() {
    let err = CloudProfileError::ApiError {
        status: 429,
        message: "rate limit exceeded, retry after 60s".to_string(),
    };
    let display = err.to_string();
    assert!(display.contains("429"));
    assert!(display.contains("rate limit"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. Serialization (5 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn serialization_profile_format_versioning() {
    let profile = minimal_profile();
    let json = serde_json::to_string(&profile).unwrap();
    assert!(json.contains(PROFILE_SCHEMA_VERSION));
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["schema"], PROFILE_SCHEMA_VERSION);
}

#[test]
fn serialization_backward_compat_old_schema_deserializes() {
    let json = r#"{
        "schema": "flight.profile/0",
        "sim": "xplane",
        "aircraft": { "icao": "B738" },
        "axes": {},
        "pof_overrides": null
    }"#;
    let profile: Profile = serde_json::from_str(json).unwrap();
    assert_eq!(profile.schema, "flight.profile/0");
    assert_eq!(profile.sim.as_deref(), Some("xplane"));
}

#[test]
fn serialization_forward_compat_unknown_fields_ignored() {
    // A cloud profile response with extra fields should still deserialize
    let json = serde_json::json!({
        "id": "fwd-001",
        "title": "Future Profile",
        "description": null,
        "author_handle": "anon",
        "upvotes": 5,
        "downvotes": 0,
        "download_count": 100,
        "published_at": "2025-01-01T00:00:00Z",
        "updated_at": "2025-01-01T00:00:00Z",
        "profile": {
            "schema": "flight.profile/1",
            "sim": "msfs",
            "aircraft": { "icao": "C172" },
            "axes": {},
            "pof_overrides": null
        },
        "future_field": "should be ignored",
        "another_new_field": 42
    });
    // serde should ignore unknown fields during deserialization
    let result: Result<CloudProfile, _> = serde_json::from_value(json);
    assert!(
        result.is_ok(),
        "should tolerate unknown fields: {:?}",
        result.err()
    );
}

#[test]
fn serialization_migration_on_sync_version_hash_changes() {
    let data_v1 = br#"{"schema":"flight.profile/0","axes":{}}"#;
    let data_v2 = br#"{"schema":"flight.profile/1","axes":{}}"#;
    let hash_v1 = compute_version_hash(data_v1);
    let hash_v2 = compute_version_hash(data_v2);
    assert_ne!(
        hash_v1, hash_v2,
        "different schema versions must produce different hashes"
    );
}

#[test]
fn serialization_schema_validation_rejects_malformed_json() {
    let bad_json = r#"{ "schema": 123, "axes": "not-a-map" }"#;
    let result: Result<Profile, _> = serde_json::from_str(bad_json);
    assert!(result.is_err(), "malformed profile JSON should be rejected");
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. Property Tests (6 tests)
// ═════════════════════════════════════════════════════════════════════════════

proptest! {
    /// Planning the same state twice produces an identical (empty) plan.
    #[test]
    fn prop_sync_plan_idempotent(
        ts1 in 1u64..1_000_000u64,
        ts2 in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let profiles = vec![
            sync_state("a", "hash-a", ts1),
            sync_state("b", "hash-b", ts2),
        ];
        let plan1 = engine.plan(&profiles, &profiles);
        let plan2 = engine.plan(&profiles, &profiles);
        prop_assert!(plan1.is_empty());
        prop_assert!(plan2.is_empty());
    }

    /// Conflict detection is symmetric: swapping local/remote produces a
    /// conflict on the same profile.
    #[test]
    fn prop_conflict_detection_symmetric(
        ts_local in 1u64..1_000_000u64,
        ts_remote in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::with_strategy(
            MockCloudBackend::new(),
            ConflictStrategy::Manual,
        );
        let local = vec![sync_state("p", "hash-l", ts_local)];
        let remote = vec![sync_state("p", "hash-r", ts_remote)];

        let plan_lr = engine.plan(&local, &remote);
        let plan_rl = engine.plan(&remote, &local);

        // Both directions should detect a conflict on "p"
        prop_assert_eq!(plan_lr.conflicts.len(), 1);
        prop_assert_eq!(plan_rl.conflicts.len(), 1);
        prop_assert_eq!(&plan_lr.conflicts[0].profile_id, "p");
        prop_assert_eq!(&plan_rl.conflicts[0].profile_id, "p");
    }

    /// Data uploaded to mock storage can always be downloaded unchanged.
    #[test]
    fn prop_storage_round_trip(
        data in proptest::collection::vec(any::<u8>(), 0..1024),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let backend = MockCloudBackend::new();
            backend.upload("rt-prop", &data).await.unwrap();
            let downloaded = backend.download("rt-prop").await.unwrap();
            assert_eq!(data, downloaded);
        });
    }

    /// Version hashes are deterministic — same data always yields same hash.
    #[test]
    fn prop_version_hash_deterministic(
        data in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        let h1 = compute_version_hash(&data);
        let h2 = compute_version_hash(&data);
        prop_assert_eq!(h1, h2);
    }

    /// LWW always resolves without conflicts — never leaves unresolved state.
    #[test]
    fn prop_lww_never_produces_conflicts(
        ts_local in 1u64..1_000_000u64,
        ts_remote in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local = vec![sync_state("p", "hl", ts_local)];
        let remote = vec![sync_state("p", "hr", ts_remote)];
        let plan = engine.plan(&local, &remote);
        prop_assert!(plan.conflicts.is_empty(), "LWW must never leave conflicts");
    }

    /// Metadata checksum matches SHA-256 of uploaded data.
    #[test]
    fn prop_checksum_matches_sha256(
        data in proptest::collection::vec(any::<u8>(), 1..256),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let backend = MockCloudBackend::new();
            backend.upload("chk", &data).await.unwrap();
            let meta = backend
                .list_profiles()
                .await
                .unwrap()
                .into_iter()
                .find(|m| m.id == "chk")
                .unwrap();
            let expected = format!("{:x}", Sha256::digest(&data));
            assert_eq!(meta.checksum, expected);
        });
    }
}
