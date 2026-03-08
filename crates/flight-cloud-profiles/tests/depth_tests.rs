// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive depth tests for cloud profile sync and sharing.
//!
//! Covers: models, serialization round-trips, conflict resolution edge cases,
//! version tracking & history, metadata handling, error handling,
//! storage backends, sanitization boundaries, and property-based tests.

use chrono::{TimeZone as _, Utc};
use flight_cloud_profiles::{
    CloudProfile, CloudProfileError,
    cache::{CacheEntry, CACHE_TTL_SECS, ProfileCache},
    models::{Page, ProfileSortOrder, PublishMeta, VoteDirection, VoteResult, ListFilter},
    sanitize::{sanitize_for_upload, validate_for_publish},
    storage::{CloudBackend, FileSystemBackend, MockCloudBackend, ProfileMetadata},
    sync::{
        ConflictResolution, ConflictStrategy, ProfileAction, ProfileSyncState, SyncConflict,
        SyncEngine, SyncPlan,
    },
    versioning::{ProfileVersion, VersionHistory, compute_version_hash},
    ProfileListing, ClientConfig, DEFAULT_API_BASE_URL, DEFAULT_TIMEOUT_SECS, DEFAULT_PAGE_SIZE,
};
use flight_profile::{AircraftId, AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use proptest::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ── helpers ──────────────────────────────────────────────────────────────────

fn state(id: &str, hash: &str, ts: u64) -> ProfileSyncState {
    ProfileSyncState {
        id: id.to_string(),
        version_hash: hash.to_string(),
        updated_at: ts,
    }
}

/// Alias for tests that use sync_state naming.
fn sync_state(id: &str, hash: &str, ts: u64) -> ProfileSyncState {
    state(id, hash, ts)
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

fn empty_profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn profile_with_axes_simple(pairs: &[(&str, f32, f32)]) -> Profile {
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

fn profile_with_axes(axes: HashMap<String, AxisConfig>) -> Profile {
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

fn axis(dz: Option<f32>, expo: Option<f32>) -> AxisConfig {
    AxisConfig {
        deadzone: dz,
        expo,
        slew_rate: None,
        detents: vec![],
        curve: None,
        filter: None,
    }
}

fn make_cloud_profile_with_data(id: &str, profile: Profile) -> CloudProfile {
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

fn make_cloud_profile(id: &str) -> CloudProfile {
    CloudProfile {
        id: id.to_string(),
        title: format!("Cloud {id}"),
        description: Some("test desc".to_string()),
        author_handle: "pilot42".to_string(),
        upvotes: 5,
        downvotes: 1,
        download_count: 42,
        published_at: Utc::now(),
        updated_at: Utc::now(),
        profile: empty_profile(),
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
        title: format!("Profile {id}"),
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

fn make_version_full(id: &str, ts: u64, author: &str, changes: &[&str]) -> ProfileVersion {
    ProfileVersion {
        version_id: id.to_string(),
        timestamp: ts,
        author: author.to_string(),
        changes: changes.iter().map(|s| s.to_string()).collect(),
    }
}

fn make_version(id: &str, ts: u64, changes: &[&str]) -> ProfileVersion {
    ProfileVersion {
        version_id: id.to_string(),
        timestamp: ts,
        author: "tester".to_string(),
        changes: changes.iter().map(|s| s.to_string()).collect(),
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
        .join("flight-cloud-depth-tests-v3")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. SERIALIZATION ROUND-TRIPS
// ═══════════════════════════════════════════════════════════════════════════════

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

#[test]
fn serialization_multi_axis_profile_round_trip() {
    let profile = profile_with_axes_simple(&[
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

#[tokio::test]
async fn serialization_upload_download_preserves_profile_data() {
    let backend = MockCloudBackend::new();
    let profile = profile_with_axes_simple(&[("pitch", 0.05, 0.3), ("roll", 0.1, 0.5)]);
    let data = serde_json::to_vec(&profile).unwrap();

    backend.upload("ser-test", &data).await.unwrap();
    let downloaded = backend.download("ser-test").await.unwrap();
    let restored: Profile = serde_json::from_slice(&downloaded).unwrap();

    assert_eq!(restored.axes.len(), 2);
    assert_eq!(restored.axes["pitch"].deadzone, Some(0.05));
    assert_eq!(restored.axes["roll"].expo, Some(0.5));
}

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

#[test]
fn serialization_publish_meta_null_description() {
    let meta = PublishMeta::new("Title Only");
    let body = serde_json::json!({"title": meta.title, "description": meta.description});
    assert!(body["description"].is_null());
}

#[test]
fn serialization_publish_meta_with_description() {
    let meta = PublishMeta::with_description("Full Meta", "Has a description");
    let body = serde_json::json!({"title": meta.title, "description": meta.description});
    assert_eq!(body["description"], "Has a description");
}

#[tokio::test]
async fn serialization_binary_data_through_backend() {
    let backend = MockCloudBackend::new();
    let binary_data: Vec<u8> = (0..=255).collect();
    backend.upload("binary-p", &binary_data).await.unwrap();
    let downloaded = backend.download("binary-p").await.unwrap();
    assert_eq!(downloaded, binary_data);
}

#[test]
fn listing_score_zero() {
    let l = make_listing("a", 0, 0, 0);
    assert_eq!(l.score(), 0);
}

#[test]
fn listing_score_large_positive() {
    let l = make_listing("a", u32::MAX, 0, 0);
    assert_eq!(l.score(), u32::MAX as i64);
}

#[test]
fn listing_score_large_negative() {
    let l = make_listing("a", 0, u32::MAX, 0);
    assert_eq!(l.score(), -(u32::MAX as i64));
}

#[test]
fn listing_score_symmetric_cancels() {
    let l = make_listing("a", 100, 100, 0);
    assert_eq!(l.score(), 0);
}

#[test]
fn listing_serde_round_trip_with_all_fields() {
    let l = ProfileListing {
        id: "full".to_string(),
        title: "Full Listing".to_string(),
        description: Some("A description".to_string()),
        sim: Some("dcs".to_string()),
        aircraft_icao: Some("F16".to_string()),
        author_handle: "ace".to_string(),
        upvotes: 999,
        downvotes: 1,
        download_count: 50_000,
        schema_version: "flight.profile/1".to_string(),
        published_at: Utc.with_ymd_and_hms(2024, 12, 25, 12, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
    };
    let json = serde_json::to_string(&l).unwrap();
    let back: ProfileListing = serde_json::from_str(&json).unwrap();
    assert_eq!(l, back);
}

#[test]
fn listing_serde_none_fields() {
    let l = ProfileListing {
        id: "min".to_string(),
        title: "Minimal".to_string(),
        description: None,
        sim: None,
        aircraft_icao: None,
        author_handle: "anon".to_string(),
        upvotes: 0,
        downvotes: 0,
        download_count: 0,
        schema_version: "flight.profile/1".to_string(),
        published_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
    };
    let json = serde_json::to_string(&l).unwrap();
    let back: ProfileListing = serde_json::from_str(&json).unwrap();
    assert_eq!(l, back);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. CONFLICT RESOLUTION / SYNC ENGINE PLANNING
// ═══════════════════════════════════════════════════════════════════════════════

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

#[test]
fn conflict_resolve_nonexistent_is_safe_noop() {
    let mut plan = SyncPlan::default();
    plan.resolve_conflict("ghost", ConflictResolution::UseLocal);
    plan.resolve_conflict("phantom", ConflictResolution::UseRemote);
    plan.resolve_conflict("void", ConflictResolution::Skip);
    assert!(plan.is_empty());
}

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

#[test]
fn sync_empty_both_sides() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let plan = engine.plan(&[], &[]);
    assert!(plan.is_empty());
}

#[test]
fn sync_local_only_uploads() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("a", "h1", 100)];
    let plan = engine.plan(&local, &[]);
    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].profile_id, "a");
}

#[test]
fn sync_remote_only_downloads() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let remote = vec![sync_state("b", "h2", 200)];
    let plan = engine.plan(&[], &remote);
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].profile_id, "b");
}

#[test]
fn sync_same_hash_no_action() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("x", "same", 100)];
    let remote = vec![sync_state("x", "same", 200)];
    assert!(engine.plan(&local, &remote).is_empty());
}

#[test]
fn sync_lww_local_newer() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("x", "l", 2000)];
    let remote = vec![sync_state("x", "r", 1000)];
    let plan = engine.plan(&local, &remote);
    assert_eq!(plan.uploads.len(), 1);
    assert!(plan.downloads.is_empty());
}

#[test]
fn sync_lww_remote_newer() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("x", "l", 1000)];
    let remote = vec![sync_state("x", "r", 2000)];
    let plan = engine.plan(&local, &remote);
    assert!(plan.uploads.is_empty());
    assert_eq!(plan.downloads.len(), 1);
}

#[test]
fn sync_lww_equal_timestamp_prefers_local() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    let local = vec![sync_state("x", "l", 1000)];
    let remote = vec![sync_state("x", "r", 1000)];
    let plan = engine.plan(&local, &remote);
    assert_eq!(plan.uploads.len(), 1);
}

#[test]
fn sync_manual_creates_conflict() {
    let engine = SyncEngine::with_strategy(MockCloudBackend::new(), ConflictStrategy::Manual);
    let local = vec![sync_state("x", "l", 1000)];
    let remote = vec![sync_state("x", "r", 2000)];
    let plan = engine.plan(&local, &remote);
    assert_eq!(plan.conflicts.len(), 1);
    assert!(plan.uploads.is_empty());
    assert!(plan.downloads.is_empty());
}

#[test]
fn sync_plan_resolve_use_local() {
    let mut plan = SyncPlan {
        conflicts: vec![SyncConflict {
            profile_id: "c".to_string(),
            local_version: sync_state("c", "lh", 100),
            remote_version: sync_state("c", "rh", 200),
        }],
        ..Default::default()
    };
    plan.resolve_conflict("c", ConflictResolution::UseLocal);
    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.uploads.len(), 1);
    assert_eq!(plan.uploads[0].version_hash, "lh");
}

#[test]
fn sync_plan_resolve_use_remote() {
    let mut plan = SyncPlan {
        conflicts: vec![SyncConflict {
            profile_id: "c".to_string(),
            local_version: sync_state("c", "lh", 100),
            remote_version: sync_state("c", "rh", 200),
        }],
        ..Default::default()
    };
    plan.resolve_conflict("c", ConflictResolution::UseRemote);
    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].version_hash, "rh");
}

#[test]
fn sync_plan_resolve_skip() {
    let mut plan = SyncPlan {
        conflicts: vec![SyncConflict {
            profile_id: "c".to_string(),
            local_version: sync_state("c", "lh", 100),
            remote_version: sync_state("c", "rh", 200),
        }],
        ..Default::default()
    };
    plan.resolve_conflict("c", ConflictResolution::Skip);
    assert!(plan.is_empty());
}

#[test]
fn sync_plan_resolve_nonexistent_noop() {
    let mut plan = SyncPlan::default();
    plan.resolve_conflict("nope", ConflictResolution::UseLocal);
    assert!(plan.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. VERSION TRACKING AND HISTORY
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn version_history_ordering() {
    let mut history = VersionHistory::new();
    history.push(make_version_full("v1", 1000, "alice", &["init"]));
    history.push(make_version_full("v2", 2000, "bob", &["update"]));
    history.push(make_version_full("v3", 3000, "charlie", &["fix"]));

    assert_eq!(history.len(), 3);
    assert_eq!(history.versions[0].version_id, "v1");
    assert_eq!(history.versions[1].version_id, "v2");
    assert_eq!(history.versions[2].version_id, "v3");
}

#[test]
fn version_history_current_and_previous() {
    let mut history = VersionHistory::new();
    assert!(history.current().is_none());
    assert!(history.previous().is_none());

    history.push(make_version_full("v1", 100, "a", &["one"]));
    assert_eq!(history.current().unwrap().version_id, "v1");
    assert!(history.previous().is_none());

    history.push(make_version_full("v2", 200, "b", &["two"]));
    assert_eq!(history.current().unwrap().version_id, "v2");
    assert_eq!(history.previous().unwrap().version_id, "v1");

    history.push(make_version_full("v3", 300, "c", &["three"]));
    assert_eq!(history.current().unwrap().version_id, "v3");
    assert_eq!(history.previous().unwrap().version_id, "v2");
}

#[test]
fn version_diff_aggregates_intermediate_changes() {
    let mut history = VersionHistory::new();
    history.push(make_version_full("v1", 100, "a", &["created"]));
    history.push(make_version_full("v2", 200, "b", &["added pitch"]));
    history.push(make_version_full("v3", 300, "a", &["adjusted roll", "fixed yaw"]));
    history.push(make_version_full("v4", 400, "c", &["final tune"]));

    let diff = history.diff("v1", "v4").unwrap();
    assert_eq!(diff.changes.len(), 4);
    assert!(diff.changes.contains(&"added pitch".to_string()));
    assert!(diff.changes.contains(&"adjusted roll".to_string()));
    assert!(diff.changes.contains(&"fixed yaw".to_string()));
    assert!(diff.changes.contains(&"final tune".to_string()));
}

#[test]
fn version_diff_same_version_empty() {
    let mut history = VersionHistory::new();
    history.push(make_version_full("v1", 100, "a", &["created"]));
    let diff = history.diff("v1", "v1").unwrap();
    assert!(diff.changes.is_empty());
    assert_eq!(diff.from, "v1");
    assert_eq!(diff.to, "v1");
}

#[test]
fn version_diff_missing_version_returns_none() {
    let mut history = VersionHistory::new();
    history.push(make_version_full("v1", 100, "a", &["init"]));
    assert!(history.diff("v1", "v99").is_none());
    assert!(history.diff("v99", "v1").is_none());
    assert!(history.diff("v99", "v100").is_none());
}

#[test]
fn version_history_serialization_complete() {
    let mut history = VersionHistory::new();
    history.push(make_version_full("v1", 1000, "alice", &["initial setup"]));
    history.push(make_version_full(
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

#[test]
fn version_history_get_by_id() {
    let mut history = VersionHistory::new();
    history.push(make_version_full("alpha", 100, "a", &["step 1"]));
    history.push(make_version_full("beta", 200, "b", &["step 2"]));
    history.push(make_version_full("gamma", 300, "c", &["step 3"]));

    assert_eq!(history.get("beta").unwrap().timestamp, 200);
    assert_eq!(history.get("gamma").unwrap().author, "c");
    assert!(history.get("delta").is_none());
}

#[test]
fn history_empty() {
    let h = VersionHistory::new();
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
    assert!(h.current().is_none());
    assert!(h.previous().is_none());
}

#[test]
fn history_single_version() {
    let mut h = VersionHistory::new();
    h.push(make_version("v1", 100, &["init"]));
    assert_eq!(h.len(), 1);
    assert_eq!(h.current().unwrap().version_id, "v1");
    assert!(h.previous().is_none());
}

#[test]
fn history_two_versions() {
    let mut h = VersionHistory::new();
    h.push(make_version("v1", 100, &["init"]));
    h.push(make_version("v2", 200, &["update"]));
    assert_eq!(h.current().unwrap().version_id, "v2");
    assert_eq!(h.previous().unwrap().version_id, "v1");
}

#[test]
fn history_diff_adjacent() {
    let mut h = VersionHistory::new();
    h.push(make_version("v1", 100, &["a"]));
    h.push(make_version("v2", 200, &["b"]));
    let d = h.diff("v1", "v2").unwrap();
    assert_eq!(d.changes, vec!["b".to_string()]);
}

#[test]
fn profile_version_serde_round_trip() {
    let v = make_version("v1", 100, &["a", "b"]);
    let json = serde_json::to_string(&v).unwrap();
    let back: ProfileVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. METADATA HANDLING
// ═══════════════════════════════════════════════════════════════════════════════

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

#[test]
fn metadata_description_preserves_content() {
    let desc = "A".repeat(500);
    let meta = PublishMeta::with_description("Tagged Profile", &desc);
    assert_eq!(meta.description.as_ref().unwrap().len(), 500);
}

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

#[test]
fn profile_metadata_serde_round_trip() {
    let m = ProfileMetadata {
        id: "pm1".to_string(),
        name: "Profile Meta".to_string(),
        updated_at: 1_700_000_000,
        size: 4096,
        checksum: "abc123".to_string(),
    };
    let json = serde_json::to_string(&m).unwrap();
    let back: ProfileMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(m, back);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. ERROR HANDLING
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn error_download_nonexistent_mock() {
    let backend = MockCloudBackend::new();
    let result = backend.download("does-not-exist").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn error_download_nonexistent_filesystem() {
    let tmp = tempfile_dir();
    let backend = FileSystemBackend::new(&tmp);
    let result = backend.download("ghost-profile").await;
    assert!(result.is_err());
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn error_delete_nonexistent_mock() {
    let backend = MockCloudBackend::new();
    let result = backend.delete("phantom").await;
    assert!(result.is_err());
}

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

#[test]
fn error_invalid_argument_wraps_reason() {
    let err = CloudProfileError::InvalidArgument("missing required field: title".to_string());
    assert!(err.to_string().contains("missing required field: title"));
}

#[test]
fn error_cache_io_wraps_std_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let err = CloudProfileError::Cache(io_err);
    assert!(err.to_string().contains("access denied"));
}

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
// 6. SANITIZATION / VALIDATION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn sanitize_normalizes_schema_from_any_version() {
    let mut profile = minimal_profile();
    profile.schema = "flight.profile/99".to_string();
    let sanitized = sanitize_for_upload(&profile);
    assert_eq!(sanitized.schema, PROFILE_SCHEMA_VERSION);
}

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

#[test]
fn sanitize_preserves_none_pof_overrides() {
    let profile = minimal_profile();
    let sanitized = sanitize_for_upload(&profile);
    assert!(sanitized.pof_overrides.is_none());
}

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

#[test]
fn validate_blank_title() {
    assert!(validate_for_publish(&empty_profile(), "").is_err());
}

#[test]
fn validate_whitespace_only_title() {
    assert!(validate_for_publish(&empty_profile(), "   \t\n").is_err());
}

#[test]
fn validate_title_exactly_100_chars() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.05), None));
    let p = profile_with_axes(axes);
    let title = "x".repeat(100);
    assert!(validate_for_publish(&p, &title).is_ok());
}

#[test]
fn validate_title_101_chars() {
    let title = "x".repeat(101);
    assert!(validate_for_publish(&empty_profile(), &title).is_err());
}

#[test]
fn validate_no_axes_no_overrides() {
    assert!(validate_for_publish(&empty_profile(), "Valid Title").is_err());
}

#[test]
fn validate_deadzone_at_boundary_zero() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.0), None));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "Good").is_ok());
}

#[test]
fn validate_deadzone_at_boundary_half() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.5), None));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "Good").is_ok());
}

#[test]
fn validate_deadzone_over_half() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.51), None));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "Bad").is_err());
}

#[test]
fn validate_deadzone_negative() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(-0.1), None));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "Bad").is_err());
}

#[test]
fn validate_expo_at_boundary_zero() {
    let mut axes = HashMap::new();
    axes.insert("roll".to_string(), axis(None, Some(0.0)));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "OK").is_ok());
}

#[test]
fn validate_expo_at_boundary_one() {
    let mut axes = HashMap::new();
    axes.insert("roll".to_string(), axis(None, Some(1.0)));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "OK").is_ok());
}

#[test]
fn validate_expo_over_one() {
    let mut axes = HashMap::new();
    axes.insert("roll".to_string(), axis(None, Some(1.01)));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "Bad").is_err());
}

#[test]
fn validate_expo_negative() {
    let mut axes = HashMap::new();
    axes.insert("roll".to_string(), axis(None, Some(-0.5)));
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "Bad").is_err());
}

#[test]
fn validate_multiple_axes_first_bad_fails() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.05), None));
    axes.insert("roll".to_string(), axis(Some(0.9), None)); // bad
    let p = profile_with_axes(axes);
    assert!(validate_for_publish(&p, "Mixed").is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. CACHE OPERATIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn cache_evict_targets_single_profile() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);

    for id in &["keep-1", "remove-me", "keep-2"] {
        let cp = make_cloud_profile_with_data(id, minimal_profile());
        cache.store(&cp).await.unwrap();
    }

    cache.evict("remove-me").await.unwrap();

    assert!(cache.get("keep-1").await.unwrap().is_some());
    assert!(cache.get("remove-me").await.unwrap().is_none());
    assert!(cache.get("keep-2").await.unwrap().is_some());
    assert_eq!(cache.list_cached().await.len(), 2);
    cleanup_dir(&tmp);
}

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

#[tokio::test]
async fn cache_miss_returns_none_not_error() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    let result = cache.get("never-stored").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn cache_zero_ttl_expires_all() {
    let tmp = tempfile_dir();
    let writer = ProfileCache::new(tmp.clone(), 3600);
    let reader = ProfileCache::new(tmp.clone(), 0);

    for i in 0..3 {
        let cp = make_cloud_profile_with_data(&format!("ttl-{i}"), minimal_profile());
        writer.store(&cp).await.unwrap();
    }

    // Writer sees all, reader sees none
    assert_eq!(writer.list_cached().await.len(), 3);
    assert_eq!(reader.list_cached().await.len(), 0);
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn cache_store_retrieve() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    let p = make_cloud_profile("store-1");
    cache.store(&p).await.unwrap();
    let got = cache.get("store-1").await.unwrap().unwrap();
    assert_eq!(got.id, "store-1");
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn cache_evict() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    cache.store(&make_cloud_profile("ev")).await.unwrap();
    cache.evict("ev").await.unwrap();
    assert!(cache.get("ev").await.unwrap().is_none());
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn cache_clear_removes_all() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    for i in 0..5 {
        cache
            .store(&make_cloud_profile(&format!("clr-{i}")))
            .await
            .unwrap();
    }
    cache.clear().await.unwrap();
    assert!(cache.list_cached().await.is_empty());
    cleanup_dir(&tmp);
}

#[test]
fn cache_entry_fresh_not_expired() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let e = CacheEntry {
        id: "x".to_string(),
        cached_at: now,
        title: "T".to_string(),
    };
    assert!(!e.is_expired(3600));
}

#[test]
fn cache_entry_epoch_expired() {
    let e = CacheEntry {
        id: "x".to_string(),
        cached_at: 0,
        title: "T".to_string(),
    };
    assert!(e.is_expired(1));
}

#[test]
fn cache_entry_boundary_ttl() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let e = CacheEntry {
        id: "x".to_string(),
        cached_at: now - 100,
        title: "T".to_string(),
    };
    assert!(!e.is_expired(200));
    assert!(e.is_expired(50));
    assert!(e.is_expired(100)); // exactly at boundary → expired
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. MODELS / CONFIG
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn cloud_profile_score() {
    let p = make_cloud_profile("cp1");
    assert_eq!(p.score(), 4); // 5 - 1
}

#[test]
fn list_filter_default_values() {
    let f = ListFilter::default();
    assert!(f.sim.is_none());
    assert!(f.aircraft_icao.is_none());
    assert!(f.query.is_none());
    assert_eq!(f.sort, ProfileSortOrder::TopRated);
    assert_eq!(f.page, 1);
    assert_eq!(f.per_page, DEFAULT_PAGE_SIZE);
}

#[test]
fn list_filter_serde_round_trip() {
    let f = ListFilter {
        sim: Some("xplane".to_string()),
        aircraft_icao: Some("B738".to_string()),
        query: Some("landing".to_string()),
        sort: ProfileSortOrder::Newest,
        page: 3,
        per_page: 10,
    };
    let json = serde_json::to_string(&f).unwrap();
    let back: ListFilter = serde_json::from_str(&json).unwrap();
    assert_eq!(back.sim.as_deref(), Some("xplane"));
    assert_eq!(back.page, 3);
}

#[test]
fn sort_order_display_all_variants() {
    assert_eq!(ProfileSortOrder::TopRated.to_string(), "top_rated");
    assert_eq!(ProfileSortOrder::Newest.to_string(), "newest");
    assert_eq!(ProfileSortOrder::MostDownloaded.to_string(), "most_downloaded");
}

#[test]
fn client_config_defaults() {
    let cfg = ClientConfig::default();
    assert_eq!(cfg.base_url, DEFAULT_API_BASE_URL);
    assert!(cfg.use_cache);
    assert_eq!(
        cfg.timeout,
        std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS)
    );
}

#[test]
fn constants_sanity() {
    assert!(!DEFAULT_API_BASE_URL.is_empty());
    assert!(DEFAULT_API_BASE_URL.starts_with("https://"));
    assert!(DEFAULT_TIMEOUT_SECS > 0);
    assert!(DEFAULT_PAGE_SIZE > 0 && DEFAULT_PAGE_SIZE <= 100);
    assert!(CACHE_TTL_SECS > 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. BACKEND DELEGATION
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn mock_backend_upload_download() {
    let b = MockCloudBackend::new();
    b.upload("p1", b"data").await.unwrap();
    assert_eq!(b.download("p1").await.unwrap(), b"data");
}

#[tokio::test]
async fn fs_backend_upload_download() {
    let tmp = tempfile_dir();
    let b = FileSystemBackend::new(&tmp);
    b.upload("f1", b"fs-data").await.unwrap();
    assert_eq!(b.download("f1").await.unwrap(), b"fs-data");
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn sync_engine_upload_download() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    engine.upload("se1", b"payload").await.unwrap();
    let data = engine.download("se1").await.unwrap();
    assert_eq!(data, b"payload");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10. PROPERTY-BASED TESTS
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
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

    #[test]
    fn prop_version_hash_deterministic(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let h1 = compute_version_hash(&data);
        let h2 = compute_version_hash(&data);
        prop_assert_eq!(h1.clone(), h2);
        prop_assert_eq!(h1.len(), 64);
    }

    #[test]
    fn prop_sanitize_idempotent(
        dz in 0.0f32..0.5f32,
        expo in 0.0f32..1.0f32,
    ) {
        let profile = profile_with_axes_simple(&[("axis", dz, expo)]);
        let once = sanitize_for_upload(&profile);
        let twice = sanitize_for_upload(&once);
        prop_assert_eq!(once.schema, twice.schema);
        prop_assert_eq!(once.sim, twice.sim);
        prop_assert_eq!(once.axes.len(), twice.axes.len());
    }

    #[test]
    fn prop_listing_score_is_upvotes_minus_downvotes(up in 0u32..10000, down in 0u32..10000) {
        let l = make_listing("p", up, down, 0);
        prop_assert_eq!(l.score(), up as i64 - down as i64);
    }

    #[test]
    fn prop_page_total_pages_covers_total(per_page in 1u32..200, total in 0u64..10000) {
        let p: Page<()> = Page { items: vec![], page: 1, per_page, total };
        let pages = p.total_pages();
        if total == 0 {
            prop_assert_eq!(pages, 0);
        } else {
            prop_assert!(pages * per_page as u64 >= total);
        }
    }

    #[test]
    fn prop_sync_local_only_all_uploaded(count in 1usize..20) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let local: Vec<_> = (0..count)
            .map(|i| sync_state(&format!("p{i}"), &format!("h{i}"), 100))
            .collect();
        let plan = engine.plan(&local, &[]);
        prop_assert_eq!(plan.uploads.len(), count);
        prop_assert!(plan.downloads.is_empty());
        prop_assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn prop_sync_remote_only_all_downloaded(count in 1usize..20) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let remote: Vec<_> = (0..count)
            .map(|i| sync_state(&format!("p{i}"), &format!("h{i}"), 100))
            .collect();
        let plan = engine.plan(&[], &remote);
        prop_assert!(plan.uploads.is_empty());
        prop_assert_eq!(plan.downloads.len(), count);
        prop_assert!(plan.conflicts.is_empty());
    }
}
