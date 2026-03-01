// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive depth tests for cloud profile sync and sharing.
//!
//! Covers: serialization round-trips, conflict resolution edge cases,
//! version tracking & history, metadata handling, error handling,
//! storage backends, sanitization boundaries, and property-based tests.
//! (50+ tests total)

use chrono::{TimeZone as _, Utc};
use flight_cloud_profiles::{
    CloudProfile, CloudProfileError,
    cache::ProfileCache,
    models::{Page, ProfileSortOrder, PublishMeta, VoteDirection, VoteResult},
    sanitize::{sanitize_for_upload, validate_for_publish},
    storage::{CloudBackend, FileSystemBackend, MockCloudBackend, ProfileMetadata},
    sync::{
        ConflictResolution, ConflictStrategy, ProfileAction, ProfileSyncState, SyncConflict,
        SyncEngine, SyncPlan,
    },
    versioning::{ProfileVersion, VersionDiff, VersionHistory, compute_version_hash},
    ProfileListing,
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

fn make_listing(
    id: &str,
    upvotes: u32,
    downvotes: u32,
    download_count: u64,
) -> ProfileListing {
    ProfileListing {
        id: id.to_string(),
        title: "Test".to_string(),
        description: None,
        sim: Some("msfs".to_string()),
        aircraft_icao: Some("C172".to_string()),
        author_handle: "pilot42".to_string(),
        upvotes,
        downvotes,
        download_count,
        schema_version: PROFILE_SCHEMA_VERSION.to_string(),
        published_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
    }
}

fn make_version(id: &str, ts: u64, author: &str, changes: &[&str]) -> ProfileVersion {
    ProfileVersion {
        version_id: id.to_string(),
        timestamp: ts,
        author: author.to_string(),
        changes: changes.iter().map(|s| s.to_string()).collect(),
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
    let name = format!("depth-{nanos:016x}-{seq:04x}-{tid:?}").replace(['(', ')', ' '], "_");
    let dir = std::env::temp_dir()
        .join("flight-cloud-depth-tests-v3")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. PROFILE UPLOAD / DOWNLOAD SERIALIZATION (10 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// CloudProfile with all optional fields populated round-trips through JSON.
#[test]
fn serialization_cloud_profile_full_fields_round_trip() {
    let mut axes = HashMap::new();
    axes.insert(
        "elevator".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.4),
            slew_rate: Some(2.5),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let cp = CloudProfile {
        id: "full-001".to_string(),
        title: "Full Featured Profile".to_string(),
        description: Some("A complete profile with all optional fields".to_string()),
        author_handle: "expert_pilot".to_string(),
        upvotes: 1234,
        downvotes: 56,
        download_count: 99999,
        published_at: Utc.with_ymd_and_hms(2024, 6, 15, 12, 30, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 3, 20, 8, 0, 0).unwrap(),
        profile: Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("dcs".to_string()),
            aircraft: Some(AircraftId {
                icao: "F16C".to_string(),
            }),
            axes,
            pof_overrides: None,
        },
    };

    let json = serde_json::to_string_pretty(&cp).unwrap();
    let restored: CloudProfile = serde_json::from_str(&json).unwrap();

    assert_eq!(cp, restored);
    assert_eq!(restored.score(), 1178);
    assert_eq!(restored.profile.axes["elevator"].slew_rate, Some(2.5));
}

/// CloudProfile with no description and no aircraft still round-trips.
#[test]
fn serialization_cloud_profile_minimal_optional_fields() {
    let cp = CloudProfile {
        id: "minimal-001".to_string(),
        title: "Bare Minimum".to_string(),
        description: None,
        author_handle: "anon".to_string(),
        upvotes: 0,
        downvotes: 0,
        download_count: 0,
        published_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        profile: Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        },
    };

    let json = serde_json::to_string(&cp).unwrap();
    let restored: CloudProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(cp, restored);
    assert!(restored.description.is_none());
    assert!(restored.profile.sim.is_none());
}

/// ProfileListing preserves all metadata through JSON.
#[test]
fn serialization_listing_preserves_all_metadata() {
    let listing = ProfileListing {
        id: "list-meta-001".to_string(),
        title: "Detailed Listing".to_string(),
        description: Some("With description".to_string()),
        sim: Some("xplane".to_string()),
        aircraft_icao: Some("B738".to_string()),
        author_handle: "captain_sim".to_string(),
        upvotes: 500,
        downvotes: 25,
        download_count: 10000,
        schema_version: "flight.profile/1".to_string(),
        published_at: Utc.with_ymd_and_hms(2024, 12, 25, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 2, 14, 12, 0, 0).unwrap(),
    };

    let json = serde_json::to_string(&listing).unwrap();
    let restored: ProfileListing = serde_json::from_str(&json).unwrap();

    assert_eq!(listing, restored);
    assert_eq!(restored.sim.as_deref(), Some("xplane"));
    assert_eq!(restored.aircraft_icao.as_deref(), Some("B738"));
    assert_eq!(restored.score(), 475);
}

/// Multiple axes with different configs survive serialization.
#[test]
fn serialization_multi_axis_profile_round_trip() {
    let profile = profile_with_axes(&[
        ("pitch", 0.05, 0.3),
        ("roll", 0.08, 0.5),
        ("yaw", 0.15, 0.2),
        ("throttle", 0.0, 0.0),
    ]);

    let json = serde_json::to_string(&profile).unwrap();
    let restored: Profile = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.axes.len(), 4);
    assert_eq!(restored.axes["pitch"].deadzone, Some(0.05));
    assert_eq!(restored.axes["roll"].expo, Some(0.5));
    assert_eq!(restored.axes["yaw"].deadzone, Some(0.15));
    assert_eq!(restored.axes["throttle"].expo, Some(0.0));
}

/// Profile data uploaded to mock backend can be downloaded and deserialized.
#[tokio::test]
async fn serialization_upload_download_preserves_profile_data() {
    let backend = MockCloudBackend::new();
    let profile = profile_with_axes(&[("pitch", 0.05, 0.3), ("roll", 0.1, 0.5)]);
    let data = serde_json::to_vec(&profile).unwrap();

    backend.upload("ser-test", &data).await.unwrap();
    let downloaded = backend.download("ser-test").await.unwrap();
    let restored: Profile = serde_json::from_slice(&downloaded).unwrap();

    assert_eq!(restored.axes.len(), 2);
    assert_eq!(restored.axes["pitch"].deadzone, Some(0.05));
    assert_eq!(restored.axes["roll"].expo, Some(0.5));
}

/// Page<ProfileListing> serializes with pagination metadata intact.
#[test]
fn serialization_page_wrapper_round_trip() {
    let page: Page<ProfileListing> = Page {
        items: vec![
            make_listing("a", 10, 1, 100),
            make_listing("b", 20, 2, 200),
        ],
        page: 2,
        per_page: 25,
        total: 52,
    };

    let json = serde_json::to_string(&page).unwrap();
    let restored: Page<ProfileListing> = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.items.len(), 2);
    assert_eq!(restored.page, 2);
    assert_eq!(restored.per_page, 25);
    assert_eq!(restored.total, 52);
    assert_eq!(restored.total_pages(), 3);
    assert!(restored.has_next_page());
}

/// VoteResult round-trips correctly.
#[test]
fn serialization_vote_result_round_trip() {
    let result = VoteResult {
        upvotes: 42,
        downvotes: 7,
        recorded: VoteDirection::Down,
    };
    let json = serde_json::to_string(&result).unwrap();
    let restored: VoteResult = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.upvotes, 42);
    assert_eq!(restored.downvotes, 7);
    assert_eq!(restored.recorded, VoteDirection::Down);
    assert_eq!(restored.score(), 35);
}

/// PublishMeta serializes description as null when absent.
#[test]
fn serialization_publish_meta_null_description() {
    let meta = PublishMeta::new("Title Only");
    let body = serde_json::json!({"title": meta.title, "description": meta.description});
    assert!(body["description"].is_null());
}

/// PublishMeta serializes description as string when present.
#[test]
fn serialization_publish_meta_with_description() {
    let meta = PublishMeta::with_description("Full Meta", "Has a description");
    let body = serde_json::json!({"title": meta.title, "description": meta.description});
    assert_eq!(body["description"], "Has a description");
}

/// Binary data (non-UTF-8) can be stored and retrieved from backend.
#[tokio::test]
async fn serialization_binary_data_through_backend() {
    let backend = MockCloudBackend::new();
    let binary_data: Vec<u8> = (0..=255).collect();
    backend.upload("binary-p", &binary_data).await.unwrap();
    let downloaded = backend.download("binary-p").await.unwrap();
    assert_eq!(downloaded, binary_data);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. CONFLICT RESOLUTION (10 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// LWW: local strictly newer than remote causes upload.
#[test]
fn conflict_lww_local_newer_uploads() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let plan = engine.plan(
        &[state("p", "local-v2", 5000)],
        &[state("p", "remote-v1", 3000)],
    );

    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].version_hash, "local-v2");
    assert!(plan.downloads.is_empty());
}

/// LWW: remote strictly newer than local causes download.
#[test]
fn conflict_lww_remote_newer_downloads() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let plan = engine.plan(
        &[state("p", "local-v1", 3000)],
        &[state("p", "remote-v2", 5000)],
    );

    assert!(plan.uploads.is_empty());
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].version_hash, "remote-v2");
}

/// LWW: equal timestamps tie-break to local (upload).
#[test]
fn conflict_lww_equal_timestamps_prefer_local() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let plan = engine.plan(
        &[state("p", "local-v", 1000)],
        &[state("p", "remote-v", 1000)],
    );

    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].version_hash, "local-v");
}

/// Manual strategy: conflict captures both version states completely.
#[test]
fn conflict_manual_captures_full_state() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    let plan = engine.plan(
        &[state("p", "local-hash-abc", 100)],
        &[state("p", "remote-hash-xyz", 200)],
    );

    assert_eq!(plan.conflicts.len(), 1);
    let c = &plan.conflicts[0];
    assert_eq!(c.local_version.id, "p");
    assert_eq!(c.local_version.version_hash, "local-hash-abc");
    assert_eq!(c.local_version.updated_at, 100);
    assert_eq!(c.remote_version.version_hash, "remote-hash-xyz");
    assert_eq!(c.remote_version.updated_at, 200);
}

/// Resolving multiple conflicts in sequence works correctly.
#[test]
fn conflict_resolve_multiple_in_sequence() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    let local = vec![
        state("a", "la", 1),
        state("b", "lb", 2),
        state("c", "lc", 3),
    ];
    let remote = vec![
        state("a", "ra", 4),
        state("b", "rb", 5),
        state("c", "rc", 6),
    ];
    let mut plan = engine.plan(&local, &remote);
    assert_eq!(plan.conflicts.len(), 3);

    plan.resolve_conflict("a", ConflictResolution::UseLocal);
    plan.resolve_conflict("b", ConflictResolution::UseRemote);
    plan.resolve_conflict("c", ConflictResolution::Skip);

    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].profile_id, "a");
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].profile_id, "b");
}

/// Resolving a nonexistent conflict is a no-op and doesn't panic.
#[test]
fn conflict_resolve_nonexistent_is_safe_noop() {
    let mut plan = SyncPlan::default();
    plan.resolve_conflict("ghost", ConflictResolution::UseLocal);
    plan.resolve_conflict("phantom", ConflictResolution::UseRemote);
    plan.resolve_conflict("void", ConflictResolution::Skip);
    assert!(plan.is_empty());
}

/// Resolving the same conflict ID twice: the second call is a no-op.
#[test]
fn conflict_double_resolve_is_noop() {
    let mut plan = SyncPlan {
        uploads: vec![],
        downloads: vec![],
        conflicts: vec![SyncConflict {
            profile_id: "double".to_string(),
            local_version: state("double", "lv", 1),
            remote_version: state("double", "rv", 2),
        }],
    };

    plan.resolve_conflict("double", ConflictResolution::UseLocal);
    assert_eq!(plan.uploads.len(), 1);

    // Second resolve on same ID: no additional action
    plan.resolve_conflict("double", ConflictResolution::UseRemote);
    assert_eq!(plan.uploads.len(), 1);
    assert!(plan.downloads.is_empty());
}

/// Mixed plan: some profiles conflict, others are new on either side.
#[test]
fn conflict_mixed_plan_with_new_and_conflicting() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    let local = vec![
        state("new-local", "nl", 100),
        state("conflict-1", "cl1", 200),
        state("in-sync", "same", 300),
    ];
    let remote = vec![
        state("new-remote", "nr", 400),
        state("conflict-1", "cr1", 500),
        state("in-sync", "same", 300),
    ];
    let plan = engine.plan(&local, &remote);

    assert_eq!(plan.uploads.len(), 1); // new-local
    assert_eq!(plan.downloads.len(), 1); // new-remote
    assert_eq!(plan.conflicts.len(), 1); // conflict-1
    assert_eq!(plan.conflicts[0].profile_id, "conflict-1");
}

/// Sync plan is_empty correctly reports non-empty for uploads only.
#[test]
fn conflict_sync_plan_is_empty_checks_all_fields() {
    let mut plan = SyncPlan::default();
    assert!(plan.is_empty());

    plan.uploads.push(ProfileAction {
        profile_id: "x".to_string(),
        version_hash: "h".to_string(),
    });
    assert!(!plan.is_empty());

    plan.uploads.clear();
    plan.downloads.push(ProfileAction {
        profile_id: "y".to_string(),
        version_hash: "h".to_string(),
    });
    assert!(!plan.is_empty());

    plan.downloads.clear();
    plan.conflicts.push(SyncConflict {
        profile_id: "z".to_string(),
        local_version: state("z", "lv", 1),
        remote_version: state("z", "rv", 2),
    });
    assert!(!plan.is_empty());
}

/// Large-scale sync: 200 profiles with interleaved conflicts, uploads, downloads.
#[test]
fn conflict_large_scale_sync_plan() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);

    let mut local = Vec::new();
    let mut remote = Vec::new();

    // 50 local-only, 50 remote-only, 50 in-sync, 50 conflicting
    for i in 0..50 {
        local.push(state(&format!("local-only-{i}"), &format!("lh-{i}"), 100));
    }
    for i in 0..50 {
        remote.push(state(&format!("remote-only-{i}"), &format!("rh-{i}"), 200));
    }
    for i in 0..50 {
        let hash = format!("sync-hash-{i}");
        local.push(state(&format!("sync-{i}"), &hash, 300));
        remote.push(state(&format!("sync-{i}"), &hash, 300));
    }
    for i in 0..50 {
        local.push(state(&format!("conflict-{i}"), &format!("cl-{i}"), 400));
        remote.push(state(&format!("conflict-{i}"), &format!("cr-{i}"), 500));
    }

    let plan = engine.plan(&local, &remote);
    assert_eq!(plan.uploads.len(), 50);
    assert_eq!(plan.downloads.len(), 50);
    assert_eq!(plan.conflicts.len(), 50);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. VERSION TRACKING AND HISTORY (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Version history tracks versions in insertion order.
#[test]
fn version_history_ordering() {
    let mut history = VersionHistory::new();
    history.push(make_version("v1", 1000, "alice", &["init"]));
    history.push(make_version("v2", 2000, "bob", &["update"]));
    history.push(make_version("v3", 3000, "charlie", &["fix"]));

    assert_eq!(history.len(), 3);
    assert_eq!(history.versions[0].version_id, "v1");
    assert_eq!(history.versions[1].version_id, "v2");
    assert_eq!(history.versions[2].version_id, "v3");
}

/// current() and previous() point to correct entries.
#[test]
fn version_history_current_and_previous() {
    let mut history = VersionHistory::new();
    assert!(history.current().is_none());
    assert!(history.previous().is_none());

    history.push(make_version("v1", 100, "a", &["one"]));
    assert_eq!(history.current().unwrap().version_id, "v1");
    assert!(history.previous().is_none());

    history.push(make_version("v2", 200, "b", &["two"]));
    assert_eq!(history.current().unwrap().version_id, "v2");
    assert_eq!(history.previous().unwrap().version_id, "v1");

    history.push(make_version("v3", 300, "c", &["three"]));
    assert_eq!(history.current().unwrap().version_id, "v3");
    assert_eq!(history.previous().unwrap().version_id, "v2");
}

/// Diff between non-adjacent versions aggregates all intermediate changes.
#[test]
fn version_diff_aggregates_intermediate_changes() {
    let mut history = VersionHistory::new();
    history.push(make_version("v1", 100, "a", &["created"]));
    history.push(make_version("v2", 200, "b", &["added pitch"]));
    history.push(make_version("v3", 300, "a", &["adjusted roll", "fixed yaw"]));
    history.push(make_version("v4", 400, "c", &["final tune"]));

    let diff = history.diff("v1", "v4").unwrap();
    assert_eq!(diff.changes.len(), 4);
    assert!(diff.changes.contains(&"added pitch".to_string()));
    assert!(diff.changes.contains(&"adjusted roll".to_string()));
    assert!(diff.changes.contains(&"fixed yaw".to_string()));
    assert!(diff.changes.contains(&"final tune".to_string()));
}

/// Diff of same version returns empty changes.
#[test]
fn version_diff_same_version_empty() {
    let mut history = VersionHistory::new();
    history.push(make_version("v1", 100, "a", &["created"]));
    let diff = history.diff("v1", "v1").unwrap();
    assert!(diff.changes.is_empty());
    assert_eq!(diff.from, "v1");
    assert_eq!(diff.to, "v1");
}

/// Diff with nonexistent version returns None.
#[test]
fn version_diff_missing_version_returns_none() {
    let mut history = VersionHistory::new();
    history.push(make_version("v1", 100, "a", &["init"]));
    assert!(history.diff("v1", "v99").is_none());
    assert!(history.diff("v99", "v1").is_none());
    assert!(history.diff("v99", "v100").is_none());
}

/// Version history serialization round-trip preserves all fields.
#[test]
fn version_history_serialization_complete() {
    let mut history = VersionHistory::new();
    history.push(make_version("v1", 1000, "alice", &["initial setup"]));
    history.push(make_version(
        "v2",
        2000,
        "bob",
        &["deadzone update", "expo tweak"],
    ));

    let json = serde_json::to_string(&history).unwrap();
    let restored: VersionHistory = serde_json::from_str(&json).unwrap();

    assert_eq!(history, restored);
    assert_eq!(restored.versions[1].changes.len(), 2);
    assert_eq!(restored.versions[1].author, "bob");
}

/// compute_version_hash produces consistent 64-char hex strings.
#[test]
fn version_hash_format_and_consistency() {
    let h = compute_version_hash(b"test payload");
    assert_eq!(h.len(), 64);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));

    // Same input always produces same hash
    assert_eq!(h, compute_version_hash(b"test payload"));

    // Empty input has a valid hash too
    let empty_hash = compute_version_hash(b"");
    assert_eq!(empty_hash.len(), 64);
    assert_ne!(empty_hash, h);
}

/// get() retrieves a specific version by ID.
#[test]
fn version_history_get_by_id() {
    let mut history = VersionHistory::new();
    history.push(make_version("alpha", 100, "a", &["step 1"]));
    history.push(make_version("beta", 200, "b", &["step 2"]));
    history.push(make_version("gamma", 300, "c", &["step 3"]));

    assert_eq!(history.get("beta").unwrap().timestamp, 200);
    assert_eq!(history.get("gamma").unwrap().author, "c");
    assert!(history.get("delta").is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. METADATA HANDLING (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// ProfileMetadata correctly tracks size and checksum after upload.
#[tokio::test]
async fn metadata_backend_tracks_size_and_checksum() {
    let backend = MockCloudBackend::new();
    let data = b"profile content for checksum test";
    backend.upload("meta-p", data).await.unwrap();

    let list = backend.list_profiles().await.unwrap();
    let meta = list.iter().find(|m| m.id == "meta-p").unwrap();

    assert_eq!(meta.size, data.len() as u64);
    assert_eq!(meta.checksum, compute_version_hash(data));
    assert_eq!(meta.name, "meta-p");
}

/// Timestamps in ProfileListing reflect published and updated times.
#[test]
fn metadata_listing_timestamps() {
    let listing = ProfileListing {
        id: "ts-001".to_string(),
        title: "Timestamp Test".to_string(),
        description: None,
        sim: None,
        aircraft_icao: None,
        author_handle: "timer".to_string(),
        upvotes: 0,
        downvotes: 0,
        download_count: 0,
        schema_version: PROFILE_SCHEMA_VERSION.to_string(),
        published_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 7, 15, 23, 59, 59).unwrap(),
    };

    let json = serde_json::to_string(&listing).unwrap();
    assert!(json.contains("2024-01-01"));
    assert!(json.contains("2025-07-15"));
}

/// Tags via description field: max 500 chars per PublishMeta contract.
#[test]
fn metadata_description_preserves_content() {
    let desc = "A".repeat(500);
    let meta = PublishMeta::with_description("Tagged Profile", &desc);
    assert_eq!(meta.description.as_ref().unwrap().len(), 500);
}

/// ProfileSortOrder variants all have distinct Display values.
#[test]
fn metadata_sort_order_display_distinct() {
    let values = [
        ProfileSortOrder::TopRated,
        ProfileSortOrder::Newest,
        ProfileSortOrder::MostDownloaded,
    ];
    let display_strings: Vec<String> = values.iter().map(|v| v.to_string()).collect();

    // All must be unique
    for (i, a) in display_strings.iter().enumerate() {
        for (j, b) in display_strings.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "sort orders {i} and {j} have same display");
            }
        }
    }
}

/// FileSystemBackend metadata index tracks multiple profiles.
#[tokio::test]
async fn metadata_filesystem_index_tracks_multiple() {
    let tmp = tempfile_dir();
    let backend = FileSystemBackend::new(&tmp);

    for i in 0..5 {
        backend
            .upload(&format!("p-{i}"), format!("data-{i}").as_bytes())
            .await
            .unwrap();
    }

    let list = backend.list_profiles().await.unwrap();
    assert_eq!(list.len(), 5);

    let ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
    for i in 0..5 {
        assert!(ids.contains(&format!("p-{i}").as_str()));
    }
    cleanup_dir(&tmp);
}

/// ProfileMetadata checksum changes when data is overwritten.
#[tokio::test]
async fn metadata_checksum_updates_on_overwrite() {
    let backend = MockCloudBackend::new();
    backend.upload("chk-p", b"version 1").await.unwrap();
    let meta1 = backend
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.id == "chk-p")
        .unwrap();

    backend.upload("chk-p", b"version 2").await.unwrap();
    let meta2 = backend
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.id == "chk-p")
        .unwrap();

    assert_ne!(meta1.checksum, meta2.checksum);
    assert_eq!(meta2.size, b"version 2".len() as u64);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. ERROR HANDLING (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Download of nonexistent profile from MockCloudBackend returns error.
#[tokio::test]
async fn error_download_nonexistent_mock() {
    let backend = MockCloudBackend::new();
    let result = backend.download("does-not-exist").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

/// Download of nonexistent profile from FileSystemBackend returns error.
#[tokio::test]
async fn error_download_nonexistent_filesystem() {
    let tmp = tempfile_dir();
    let backend = FileSystemBackend::new(&tmp);
    let result = backend.download("ghost-profile").await;
    assert!(result.is_err());
    cleanup_dir(&tmp);
}

/// Delete of nonexistent profile from MockCloudBackend returns error.
#[tokio::test]
async fn error_delete_nonexistent_mock() {
    let backend = MockCloudBackend::new();
    let result = backend.delete("phantom").await;
    assert!(result.is_err());
}

/// ApiError with various status codes are distinguishable.
#[test]
fn error_api_error_status_codes_distinguishable() {
    let codes = [400, 401, 403, 404, 429, 500, 502, 503];
    let errors: Vec<CloudProfileError> = codes
        .iter()
        .map(|&code| CloudProfileError::ApiError {
            status: code,
            message: format!("error {code}"),
        })
        .collect();

    for (i, err) in errors.iter().enumerate() {
        let display = err.to_string();
        assert!(display.contains(&codes[i].to_string()));
    }
}

/// InvalidArgument error wraps the reason string.
#[test]
fn error_invalid_argument_wraps_reason() {
    let err = CloudProfileError::InvalidArgument("missing required field: title".to_string());
    assert!(err.to_string().contains("missing required field: title"));
}

/// Cache I/O error wraps std::io::Error.
#[test]
fn error_cache_io_wraps_std_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let err = CloudProfileError::Cache(io_err);
    assert!(err.to_string().contains("access denied"));
}

/// Validate rejects profile with negative deadzone (below 0.0).
#[test]
fn error_validate_rejects_negative_deadzone() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(-0.1),
            expo: Some(0.3),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    let err = validate_for_publish(&profile, "Bad Deadzone").unwrap_err();
    assert!(err.contains("deadzone"));
}

/// Validate rejects profile with expo > 1.0.
#[test]
fn error_validate_rejects_expo_out_of_range() {
    let mut axes = HashMap::new();
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(1.5),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    let err = validate_for_publish(&profile, "Bad Expo").unwrap_err();
    assert!(err.contains("expo"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. SANITIZATION EDGE CASES (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Sanitize normalizes schema version from any input.
#[test]
fn sanitize_normalizes_schema_from_any_version() {
    let mut profile = minimal_profile();
    profile.schema = "flight.profile/99".to_string();
    let sanitized = sanitize_for_upload(&profile);
    assert_eq!(sanitized.schema, PROFILE_SCHEMA_VERSION);
}

/// Sanitize lowercases all sim identifiers.
#[test]
fn sanitize_lowercases_sim_identifiers() {
    for sim in &["MSFS", "XPlane", "DCS", "MiXeD"] {
        let mut profile = minimal_profile();
        profile.sim = Some(sim.to_string());
        let sanitized = sanitize_for_upload(&profile);
        assert_eq!(
            sanitized.sim.as_deref().unwrap(),
            sim.to_ascii_lowercase(),
            "sim '{}' should be lowercased",
            sim
        );
    }
}

/// Sanitize preserves pof_overrides if present (pass-through).
#[test]
fn sanitize_preserves_none_pof_overrides() {
    let profile = minimal_profile();
    let sanitized = sanitize_for_upload(&profile);
    assert!(sanitized.pof_overrides.is_none());
}

/// Sanitize is non-destructive: original profile is unchanged.
#[test]
fn sanitize_does_not_mutate_original() {
    let profile = Profile {
        schema: "OLD_SCHEMA".to_string(),
        sim: Some("UPPER".to_string()),
        aircraft: Some(AircraftId {
            icao: "A320".to_string(),
        }),
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let _ = sanitize_for_upload(&profile);
    assert_eq!(profile.schema, "OLD_SCHEMA");
    assert_eq!(profile.sim.as_deref(), Some("UPPER"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. CACHE OPERATIONS (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Cache evict removes only the targeted profile.
#[tokio::test]
async fn cache_evict_targets_single_profile() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);

    for id in &["keep-1", "remove-me", "keep-2"] {
        let cp = make_cloud_profile(id, minimal_profile());
        cache.store(&cp).await.unwrap();
    }

    cache.evict("remove-me").await.unwrap();

    assert!(cache.get("keep-1").await.unwrap().is_some());
    assert!(cache.get("remove-me").await.unwrap().is_none());
    assert!(cache.get("keep-2").await.unwrap().is_some());
    assert_eq!(cache.list_cached().await.len(), 2);
    cleanup_dir(&tmp);
}

/// Cache store overwrites existing entry without duplication.
#[tokio::test]
async fn cache_store_overwrites_existing() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);

    let cp1 = CloudProfile {
        id: "overwrite-test".to_string(),
        title: "Version 1".to_string(),
        description: None,
        author_handle: "a".to_string(),
        upvotes: 0,
        downvotes: 0,
        download_count: 0,
        published_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        profile: minimal_profile(),
    };
    cache.store(&cp1).await.unwrap();

    let cp2 = CloudProfile {
        title: "Version 2".to_string(),
        ..cp1.clone()
    };
    cache.store(&cp2).await.unwrap();

    let listed = cache.list_cached().await;
    assert_eq!(listed.len(), 1, "should not duplicate entries");

    let got = cache.get("overwrite-test").await.unwrap().unwrap();
    assert_eq!(got.title, "Version 2");
    cleanup_dir(&tmp);
}

/// Cache miss on never-stored ID returns None (not error).
#[tokio::test]
async fn cache_miss_returns_none_not_error() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    let result = cache.get("never-stored").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    cleanup_dir(&tmp);
}

/// Cache with zero TTL immediately expires all entries.
#[tokio::test]
async fn cache_zero_ttl_expires_all() {
    let tmp = tempfile_dir();
    let writer = ProfileCache::new(tmp.clone(), 3600);
    let reader = ProfileCache::new(tmp.clone(), 0);

    for i in 0..3 {
        let cp = make_cloud_profile(&format!("ttl-{i}"), minimal_profile());
        writer.store(&cp).await.unwrap();
    }

    // Writer sees all, reader sees none
    assert_eq!(writer.list_cached().await.len(), 3);
    assert_eq!(reader.list_cached().await.len(), 0);
    cleanup_dir(&tmp);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. PROPERTY-BASED TESTS (10 tests)
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// Same hash means in-sync regardless of timestamps.
    #[test]
    fn prop_same_hash_always_in_sync(
        id in "[a-z]{1,8}",
        hash in "[a-f0-9]{8,16}",
        ts_a in 1u64..1_000_000u64,
        ts_b in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let plan = engine.plan(
            &[state(&id, &hash, ts_a)],
            &[state(&id, &hash, ts_b)],
        );
        prop_assert!(plan.is_empty());
    }

    /// Different hashes always produce an action (never no-op).
    #[test]
    fn prop_different_hash_always_produces_action(
        id in "[a-z]{1,8}",
        hash_a in "[a-f0-9]{8}",
        hash_b in "[g-z0-9]{8}",
        ts_a in 1u64..1_000_000u64,
        ts_b in 1u64..1_000_000u64,
    ) {
        prop_assume!(hash_a != hash_b);
        let engine = SyncEngine::new(MockCloudBackend::new());
        let plan = engine.plan(
            &[state(&id, &hash_a, ts_a)],
            &[state(&id, &hash_b, ts_b)],
        );
        prop_assert!(!plan.is_empty());
    }

    /// LWW always picks the hash from the side with the higher timestamp.
    #[test]
    fn prop_lww_picks_higher_timestamp(
        ts_local in 1u64..1_000_000u64,
        ts_remote in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let plan = engine.plan(
            &[state("p", "lh", ts_local)],
            &[state("p", "rh", ts_remote)],
        );

        if ts_local >= ts_remote {
            prop_assert_eq!(plan.uploads.len(), 1);
            prop_assert_eq!(&plan.uploads[0].version_hash, "lh");
        } else {
            prop_assert_eq!(plan.downloads.len(), 1);
            prop_assert_eq!(&plan.downloads[0].version_hash, "rh");
        }
    }

    /// Manual strategy always produces a conflict (never auto-resolves).
    #[test]
    fn prop_manual_always_conflicts(
        hash_a in "[a-f0-9]{8}",
        hash_b in "[g-z0-9]{8}",
        ts_a in 1u64..1_000_000u64,
        ts_b in 1u64..1_000_000u64,
    ) {
        prop_assume!(hash_a != hash_b);
        let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
        let plan = engine.plan(
            &[state("p", &hash_a, ts_a)],
            &[state("p", &hash_b, ts_b)],
        );
        prop_assert_eq!(plan.conflicts.len(), 1);
        prop_assert!(plan.uploads.is_empty());
        prop_assert!(plan.downloads.is_empty());
    }

    /// Version hash is deterministic for arbitrary byte sequences.
    #[test]
    fn prop_version_hash_deterministic(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let h1 = compute_version_hash(&data);
        let h2 = compute_version_hash(&data);
        prop_assert_eq!(h1.clone(), h2);
        prop_assert_eq!(h1.len(), 64);
    }

    /// Version hash never collides for different data (probabilistic).
    #[test]
    fn prop_version_hash_no_collision(
        a in proptest::collection::vec(any::<u8>(), 1..256),
        b in proptest::collection::vec(any::<u8>(), 1..256),
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(compute_version_hash(&a), compute_version_hash(&b));
    }

    /// Sanitize is idempotent for any valid deadzone and expo.
    #[test]
    fn prop_sanitize_idempotent(
        dz in 0.0f32..0.5f32,
        expo in 0.0f32..1.0f32,
    ) {
        let profile = profile_with_axes(&[("axis", dz, expo)]);
        let once = sanitize_for_upload(&profile);
        let twice = sanitize_for_upload(&once);
        prop_assert_eq!(once.schema, twice.schema);
        prop_assert_eq!(once.sim, twice.sim);
        prop_assert_eq!(once.axes.len(), twice.axes.len());
    }

    /// Local-only profiles always become uploads.
    #[test]
    fn prop_local_only_becomes_upload(
        id in "[a-z]{1,8}",
        hash in "[a-f0-9]{8,16}",
        ts in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let plan = engine.plan(&[state(&id, &hash, ts)], &[]);
        prop_assert_eq!(plan.uploads.len(), 1);
        prop_assert!(plan.downloads.is_empty());
        prop_assert!(plan.conflicts.is_empty());
    }

    /// Remote-only profiles always become downloads.
    #[test]
    fn prop_remote_only_becomes_download(
        id in "[a-z]{1,8}",
        hash in "[a-f0-9]{8,16}",
        ts in 1u64..1_000_000u64,
    ) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let plan = engine.plan(&[], &[state(&id, &hash, ts)]);
        prop_assert!(plan.uploads.is_empty());
        prop_assert_eq!(plan.downloads.len(), 1);
        prop_assert!(plan.conflicts.is_empty());
    }

    /// ProfileListing score is always upvotes − downvotes.
    #[test]
    fn prop_listing_score_is_upvotes_minus_downvotes(
        upvotes in 0u32..1_000_000u32,
        downvotes in 0u32..1_000_000u32,
    ) {
        let listing = make_listing("score-test", upvotes, downvotes, 0);
        prop_assert_eq!(listing.score(), upvotes as i64 - downvotes as i64);
    }
}
