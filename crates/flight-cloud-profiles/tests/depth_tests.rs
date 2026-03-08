// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-cloud-profiles.
//!
//! Covers models, sanitization, versioning, sync engine, cache, and storage
//! with edge-case coverage, property-based (proptest) invariants, and serde
//! round-trip validation.

use flight_cloud_profiles::*;
use flight_cloud_profiles::cache::{CacheEntry, CACHE_TTL_SECS, ProfileCache};
use flight_cloud_profiles::models::Page;
use flight_cloud_profiles::storage::{CloudBackend, FileSystemBackend, MockCloudBackend};
use flight_cloud_profiles::sanitize::validate_for_publish;
use flight_cloud_profiles::sync::SyncEngine;
use flight_cloud_profiles::versioning::compute_version_hash;

use chrono::{TimeZone, Utc};
use flight_profile::{AircraftId, AxisConfig, Profile};
use proptest::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn empty_profile() -> Profile {
    Profile {
        schema: "flight.profile/1".to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    }
}

fn profile_with_axes(axes: HashMap<String, AxisConfig>) -> Profile {
    Profile {
        schema: "flight.profile/1".to_string(),
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

fn make_listing(id: &str, upvotes: u32, downvotes: u32, downloads: u64) -> ProfileListing {
    ProfileListing {
        id: id.to_string(),
        title: format!("Profile {id}"),
        description: None,
        sim: Some("msfs".to_string()),
        aircraft_icao: Some("C172".to_string()),
        author_handle: "anon".to_string(),
        upvotes,
        downvotes,
        download_count: downloads,
        schema_version: "flight.profile/1".to_string(),
        published_at: Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
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

fn sync_state(id: &str, hash: &str, ts: u64) -> ProfileSyncState {
    ProfileSyncState {
        id: id.to_string(),
        version_hash: hash.to_string(),
        updated_at: ts,
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
    let dir = std::env::temp_dir()
        .join("flight-cloud-depth-tests")
        .join(format!("{nanos:016x}-{seq:04x}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. ProfileListing — score, serde, edge cases
// ═══════════════════════════════════════════════════════════════════════════════

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
// 2. CloudProfile — score, serde
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn cloud_profile_score() {
    let p = make_cloud_profile("cp1");
    assert_eq!(p.score(), 4); // 5 - 1
}

#[test]
fn cloud_profile_score_zero_votes() {
    let mut p = make_cloud_profile("cp1");
    p.upvotes = 0;
    p.downvotes = 0;
    assert_eq!(p.score(), 0);
}

#[test]
fn cloud_profile_serde_round_trip() {
    let p = make_cloud_profile("rt");
    let json = serde_json::to_string(&p).unwrap();
    let back: CloudProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(p, back);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. ListFilter — defaults, serde
// ═══════════════════════════════════════════════════════════════════════════════

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
fn list_filter_deserialize_with_defaults() {
    let json = r#"{"sort":"newest"}"#;
    let f: ListFilter = serde_json::from_str(json).unwrap();
    assert_eq!(f.sort, ProfileSortOrder::Newest);
    assert_eq!(f.page, 1);
    assert_eq!(f.per_page, DEFAULT_PAGE_SIZE);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. ProfileSortOrder — display, serde
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn sort_order_display_all_variants() {
    assert_eq!(ProfileSortOrder::TopRated.to_string(), "top_rated");
    assert_eq!(ProfileSortOrder::Newest.to_string(), "newest");
    assert_eq!(ProfileSortOrder::MostDownloaded.to_string(), "most_downloaded");
}

#[test]
fn sort_order_serde_round_trip_all_variants() {
    for variant in [
        ProfileSortOrder::TopRated,
        ProfileSortOrder::Newest,
        ProfileSortOrder::MostDownloaded,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: ProfileSortOrder = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. PublishMeta
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn publish_meta_new_no_description() {
    let m = PublishMeta::new("Title Only");
    assert_eq!(m.title, "Title Only");
    assert!(m.description.is_none());
}

#[test]
fn publish_meta_with_description() {
    let m = PublishMeta::with_description("T", "D");
    assert_eq!(m.title, "T");
    assert_eq!(m.description.as_deref(), Some("D"));
}

#[test]
fn publish_meta_serde_round_trip() {
    let m = PublishMeta::with_description("Serde Test", "Round trip");
    let json = serde_json::to_string(&m).unwrap();
    let back: PublishMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(back.title, "Serde Test");
    assert_eq!(back.description.as_deref(), Some("Round trip"));
}

#[test]
fn publish_meta_empty_string_title() {
    let m = PublishMeta::new("");
    assert_eq!(m.title, "");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. VoteDirection / VoteResult
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn vote_direction_display() {
    assert_eq!(VoteDirection::Up.to_string(), "up");
    assert_eq!(VoteDirection::Down.to_string(), "down");
}

#[test]
fn vote_direction_serde_round_trip() {
    for dir in [VoteDirection::Up, VoteDirection::Down] {
        let json = serde_json::to_string(&dir).unwrap();
        let back: VoteDirection = serde_json::from_str(&json).unwrap();
        assert_eq!(dir, back);
    }
}

#[test]
fn vote_result_score_all_up() {
    let r = VoteResult {
        upvotes: 100,
        downvotes: 0,
        recorded: VoteDirection::Up,
    };
    assert_eq!(r.score(), 100);
}

#[test]
fn vote_result_score_all_down() {
    let r = VoteResult {
        upvotes: 0,
        downvotes: 50,
        recorded: VoteDirection::Down,
    };
    assert_eq!(r.score(), -50);
}

#[test]
fn vote_result_serde_round_trip() {
    let r = VoteResult {
        upvotes: 7,
        downvotes: 3,
        recorded: VoteDirection::Up,
    };
    let json = serde_json::to_string(&r).unwrap();
    let back: VoteResult = serde_json::from_str(&json).unwrap();
    assert_eq!(back.score(), 4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Page<T>
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn page_total_pages_exact_division() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 25,
        total: 75,
    };
    assert_eq!(p.total_pages(), 3);
}

#[test]
fn page_total_pages_with_remainder() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 25,
        total: 76,
    };
    assert_eq!(p.total_pages(), 4);
}

#[test]
fn page_total_pages_zero_items() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 25,
        total: 0,
    };
    assert_eq!(p.total_pages(), 0);
    assert!(!p.has_next_page());
}

#[test]
fn page_total_pages_zero_per_page() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 0,
        total: 100,
    };
    assert_eq!(p.total_pages(), 0);
}

#[test]
fn page_has_next_page_on_first() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 10,
        total: 25,
    };
    assert!(p.has_next_page());
}

#[test]
fn page_has_next_page_on_last() {
    let p: Page<()> = Page {
        items: vec![],
        page: 3,
        per_page: 10,
        total: 25,
    };
    assert!(!p.has_next_page());
}

#[test]
fn page_has_next_page_single_page() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 100,
        total: 5,
    };
    assert!(!p.has_next_page());
}

#[test]
fn page_serde_round_trip() {
    let p: Page<String> = Page {
        items: vec!["a".to_string(), "b".to_string()],
        page: 2,
        per_page: 10,
        total: 50,
    };
    let json = serde_json::to_string(&p).unwrap();
    let back: Page<String> = serde_json::from_str(&json).unwrap();
    assert_eq!(back.items, vec!["a", "b"]);
    assert_eq!(back.total, 50);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. sanitize_for_upload
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn sanitize_normalizes_schema() {
    let mut p = empty_profile();
    p.schema = "flight.profile/0".to_string();
    let s = sanitize_for_upload(&p);
    assert_eq!(s.schema, flight_profile::PROFILE_SCHEMA_VERSION);
}

#[test]
fn sanitize_lowercases_sim() {
    let p = Profile {
        sim: Some("MSFS".to_string()),
        ..empty_profile()
    };
    let s = sanitize_for_upload(&p);
    assert_eq!(s.sim.as_deref(), Some("msfs"));
}

#[test]
fn sanitize_preserves_none_sim() {
    let p = empty_profile();
    let s = sanitize_for_upload(&p);
    assert!(s.sim.is_none());
}

#[test]
fn sanitize_preserves_aircraft() {
    let p = Profile {
        aircraft: Some(AircraftId {
            icao: "B738".to_string(),
        }),
        ..empty_profile()
    };
    let s = sanitize_for_upload(&p);
    assert_eq!(s.aircraft.as_ref().unwrap().icao, "B738");
}

#[test]
fn sanitize_preserves_axes_values() {
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), axis(Some(0.05), Some(0.3)));
    axes.insert("roll".to_string(), axis(Some(0.10), None));
    let p = profile_with_axes(axes);
    let s = sanitize_for_upload(&p);
    assert_eq!(s.axes.len(), 2);
    assert_eq!(s.axes["pitch"].deadzone, Some(0.05));
    assert_eq!(s.axes["pitch"].expo, Some(0.3));
    assert_eq!(s.axes["roll"].deadzone, Some(0.10));
}

#[test]
fn sanitize_does_not_mutate_original() {
    let p = Profile {
        sim: Some("DCS".to_string()),
        ..empty_profile()
    };
    let _s = sanitize_for_upload(&p);
    assert_eq!(p.sim.as_deref(), Some("DCS")); // unchanged
}

#[test]
fn sanitize_empty_axes() {
    let p = empty_profile();
    let s = sanitize_for_upload(&p);
    assert!(s.axes.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. validate_for_publish
// ═══════════════════════════════════════════════════════════════════════════════

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
// 10. CacheEntry — expiry logic
// ═══════════════════════════════════════════════════════════════════════════════

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
fn cache_entry_ttl_zero_always_expired() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let e = CacheEntry {
        id: "x".to_string(),
        cached_at: now,
        title: "T".to_string(),
    };
    assert!(e.is_expired(0));
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
    assert!(e.is_expired(100)); // exactly at boundary → expired (>= check)
}

#[test]
fn cache_entry_serde_round_trip() {
    let e = CacheEntry {
        id: "rt".to_string(),
        cached_at: 1_700_000_000,
        title: "Round Trip".to_string(),
    };
    let json = serde_json::to_string(&e).unwrap();
    let back: CacheEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(e.id, back.id);
    assert_eq!(e.cached_at, back.cached_at);
    assert_eq!(e.title, back.title);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 11. ProfileCache — async integration
// ═══════════════════════════════════════════════════════════════════════════════

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
async fn cache_miss_returns_none() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    assert!(cache.get("missing").await.unwrap().is_none());
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
async fn cache_evict_nonexistent_ok() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    // evicting a non-existent entry should not error
    cache.evict("ghost").await.unwrap();
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

#[tokio::test]
async fn cache_expired_not_returned() {
    let tmp = tempfile_dir();
    let writer = ProfileCache::new(tmp.clone(), 3600);
    writer.store(&make_cloud_profile("exp")).await.unwrap();
    let reader = ProfileCache::new(tmp.clone(), 0); // TTL=0 → expired
    assert!(reader.get("exp").await.unwrap().is_none());
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn cache_store_overwrites_same_id() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    let mut p = make_cloud_profile("ow");
    p.title = "First".to_string();
    cache.store(&p).await.unwrap();
    p.title = "Second".to_string();
    cache.store(&p).await.unwrap();
    let got = cache.get("ow").await.unwrap().unwrap();
    assert_eq!(got.title, "Second");
    // Index should have exactly one entry
    let listed = cache.list_cached().await;
    assert_eq!(listed.len(), 1);
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn cache_list_only_fresh() {
    let tmp = tempfile_dir();
    let cache = ProfileCache::new(tmp.clone(), 3600);
    for i in 0..3 {
        cache
            .store(&make_cloud_profile(&format!("lst-{i}")))
            .await
            .unwrap();
    }
    let listed = cache.list_cached().await;
    assert_eq!(listed.len(), 3);
    cleanup_dir(&tmp);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 12. MockCloudBackend
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn mock_backend_upload_download() {
    let b = MockCloudBackend::new();
    b.upload("p1", b"data").await.unwrap();
    assert_eq!(b.download("p1").await.unwrap(), b"data");
}

#[tokio::test]
async fn mock_backend_overwrite() {
    let b = MockCloudBackend::new();
    b.upload("p1", b"v1").await.unwrap();
    b.upload("p1", b"v2").await.unwrap();
    assert_eq!(b.download("p1").await.unwrap(), b"v2");
    assert_eq!(b.list_profiles().await.unwrap().len(), 1);
}

#[tokio::test]
async fn mock_backend_delete() {
    let b = MockCloudBackend::new();
    b.upload("p1", b"d").await.unwrap();
    b.delete("p1").await.unwrap();
    assert!(b.download("p1").await.is_err());
}

#[tokio::test]
async fn mock_backend_delete_missing_errors() {
    let b = MockCloudBackend::new();
    assert!(b.delete("nope").await.is_err());
}

#[tokio::test]
async fn mock_backend_download_missing_errors() {
    let b = MockCloudBackend::new();
    assert!(b.download("nope").await.is_err());
}

#[tokio::test]
async fn mock_backend_empty_data() {
    let b = MockCloudBackend::new();
    b.upload("empty", b"").await.unwrap();
    assert_eq!(b.download("empty").await.unwrap(), b"");
    let meta = b.list_profiles().await.unwrap();
    assert_eq!(meta[0].size, 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 13. FileSystemBackend
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fs_backend_upload_download() {
    let tmp = tempfile_dir();
    let b = FileSystemBackend::new(&tmp);
    b.upload("f1", b"fs-data").await.unwrap();
    assert_eq!(b.download("f1").await.unwrap(), b"fs-data");
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn fs_backend_list() {
    let tmp = tempfile_dir();
    let b = FileSystemBackend::new(&tmp);
    b.upload("a", b"a").await.unwrap();
    b.upload("b", b"b").await.unwrap();
    assert_eq!(b.list_profiles().await.unwrap().len(), 2);
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn fs_backend_delete() {
    let tmp = tempfile_dir();
    let b = FileSystemBackend::new(&tmp);
    b.upload("del", b"x").await.unwrap();
    b.delete("del").await.unwrap();
    assert!(b.download("del").await.is_err());
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn fs_backend_download_missing() {
    let tmp = tempfile_dir();
    let b = FileSystemBackend::new(&tmp);
    assert!(b.download("missing").await.is_err());
    cleanup_dir(&tmp);
}

#[tokio::test]
async fn fs_backend_dir_accessor() {
    let tmp = tempfile_dir();
    let b = FileSystemBackend::new(&tmp);
    assert_eq!(b.dir(), tmp.as_path());
    cleanup_dir(&tmp);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 14. ProfileMetadata serde
// ═══════════════════════════════════════════════════════════════════════════════

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
// 15. SyncEngine — plan generation
// ═══════════════════════════════════════════════════════════════════════════════

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
// 16. SyncEngine — backend delegation
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn sync_engine_upload_download() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    engine.upload("se1", b"payload").await.unwrap();
    let data = engine.download("se1").await.unwrap();
    assert_eq!(data, b"payload");
}

#[tokio::test]
async fn sync_engine_backend_ref() {
    let engine = SyncEngine::new(MockCloudBackend::new());
    engine.backend().upload("ref", b"d").await.unwrap();
    let d = engine.backend().download("ref").await.unwrap();
    assert_eq!(d, b"d");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 17. Versioning — compute_version_hash
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn hash_deterministic() {
    let h1 = compute_version_hash(b"input");
    let h2 = compute_version_hash(b"input");
    assert_eq!(h1, h2);
}

#[test]
fn hash_different_inputs_differ() {
    assert_ne!(
        compute_version_hash(b"a"),
        compute_version_hash(b"b")
    );
}

#[test]
fn hash_length_is_64() {
    assert_eq!(compute_version_hash(b"any").len(), 64);
}

#[test]
fn hash_empty_input() {
    let h = compute_version_hash(b"");
    assert_eq!(h.len(), 64);
}

#[test]
fn hash_large_input() {
    let data = vec![0xAB_u8; 1_000_000];
    let h = compute_version_hash(&data);
    assert_eq!(h.len(), 64);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 18. VersionHistory
// ═══════════════════════════════════════════════════════════════════════════════

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
fn history_get_by_id() {
    let mut h = VersionHistory::new();
    h.push(make_version("v1", 100, &["a"]));
    h.push(make_version("v2", 200, &["b"]));
    assert!(h.get("v1").is_some());
    assert!(h.get("v2").is_some());
    assert!(h.get("v99").is_none());
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
fn history_diff_span_multiple() {
    let mut h = VersionHistory::new();
    h.push(make_version("v1", 100, &["a"]));
    h.push(make_version("v2", 200, &["b"]));
    h.push(make_version("v3", 300, &["c", "d"]));
    let d = h.diff("v1", "v3").unwrap();
    assert_eq!(d.changes.len(), 3);
}

#[test]
fn history_diff_same_version_empty() {
    let mut h = VersionHistory::new();
    h.push(make_version("v1", 100, &["a"]));
    let d = h.diff("v1", "v1").unwrap();
    assert!(d.changes.is_empty());
}

#[test]
fn history_diff_missing_version() {
    let h = VersionHistory::new();
    assert!(h.diff("v1", "v2").is_none());
}

#[test]
fn history_serde_round_trip() {
    let mut h = VersionHistory::new();
    h.push(make_version("v1", 100, &["init"]));
    h.push(make_version("v2", 200, &["change"]));
    let json = serde_json::to_string(&h).unwrap();
    let back: VersionHistory = serde_json::from_str(&json).unwrap();
    assert_eq!(h, back);
}

#[test]
fn profile_version_serde_round_trip() {
    let v = make_version("v1", 100, &["a", "b"]);
    let json = serde_json::to_string(&v).unwrap();
    let back: ProfileVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 19. ClientConfig defaults
// ═══════════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════════
// 20. Constants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn constants_sanity() {
    assert!(!DEFAULT_API_BASE_URL.is_empty());
    assert!(DEFAULT_API_BASE_URL.starts_with("https://"));
    assert!(DEFAULT_TIMEOUT_SECS > 0);
    assert!(DEFAULT_PAGE_SIZE > 0 && DEFAULT_PAGE_SIZE <= 100);
    assert!(CACHE_TTL_SECS > 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 21. Error type
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn error_display_api_error() {
    let e = CloudProfileError::ApiError {
        status: 404,
        message: "not found".to_string(),
    };
    let s = e.to_string();
    assert!(s.contains("404"));
    assert!(s.contains("not found"));
}

#[test]
fn error_display_invalid_argument() {
    let e = CloudProfileError::InvalidArgument("bad input".to_string());
    assert!(e.to_string().contains("bad input"));
}

#[test]
fn error_display_cache() {
    let e = CloudProfileError::Cache(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file gone",
    ));
    assert!(e.to_string().contains("Cache"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 22. ProfileSyncState serde
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn profile_sync_state_serde_round_trip() {
    let s = sync_state("id1", "hash1", 12345);
    let json = serde_json::to_string(&s).unwrap();
    let back: ProfileSyncState = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 23. Proptest — property-based invariants
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_listing_score_is_upvotes_minus_downvotes(up in 0u32..10000, down in 0u32..10000) {
        let l = make_listing("p", up, down, 0);
        prop_assert_eq!(l.score(), up as i64 - down as i64);
    }

    #[test]
    fn prop_cloud_profile_score_is_upvotes_minus_downvotes(up in 0u32..10000, down in 0u32..10000) {
        let mut p = make_cloud_profile("p");
        p.upvotes = up;
        p.downvotes = down;
        prop_assert_eq!(p.score(), up as i64 - down as i64);
    }

    #[test]
    fn prop_vote_result_score(up in 0u32..10000, down in 0u32..10000) {
        let r = VoteResult { upvotes: up, downvotes: down, recorded: VoteDirection::Up };
        prop_assert_eq!(r.score(), up as i64 - down as i64);
    }

    #[test]
    fn prop_page_total_pages_covers_total(per_page in 1u32..200, total in 0u64..10000) {
        let p: Page<()> = Page { items: vec![], page: 1, per_page, total };
        let pages = p.total_pages();
        if total == 0 {
            prop_assert_eq!(pages, 0);
        } else {
            prop_assert!(pages * per_page as u64 >= total);
            if pages > 0 {
                let prev_pages_coverage = (pages - 1) * per_page as u64;
                prop_assert!(prev_pages_coverage < total,
                    "previous page count should not cover total");
            }
        }
    }

    #[test]
    fn prop_compute_version_hash_deterministic(data in proptest::collection::vec(any::<u8>(), 0..1000)) {
        let h1 = compute_version_hash(&data);
        let h2 = compute_version_hash(&data);
        prop_assert_eq!(h1.len(), 64);
        prop_assert_eq!(h1, h2);
    }

    #[test]
    fn prop_sanitize_always_normalizes_schema(sim in "[a-z]{0,10}") {
        let p = Profile {
            schema: "old-version".to_string(),
            sim: if sim.is_empty() { None } else { Some(sim) },
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let s = sanitize_for_upload(&p);
        prop_assert_eq!(s.schema, flight_profile::PROFILE_SCHEMA_VERSION);
    }

    #[test]
    fn prop_sanitize_lowercases_sim(sim in "[A-Za-z]{1,20}") {
        let p = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some(sim.clone()),
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let s = sanitize_for_upload(&p);
        let expected = sim.to_ascii_lowercase();
        prop_assert_eq!(s.sim.as_deref(), Some(expected.as_str()));
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

    #[test]
    fn prop_sync_same_hash_no_actions(count in 1usize..20) {
        let engine = SyncEngine::new(MockCloudBackend::new());
        let states: Vec<_> = (0..count)
            .map(|i| sync_state(&format!("p{i}"), &format!("h{i}"), 100))
            .collect();
        let plan = engine.plan(&states, &states);
        prop_assert!(plan.is_empty());
    }

    #[test]
    fn prop_version_history_len_after_push(count in 0usize..50) {
        let mut h = VersionHistory::new();
        for i in 0..count {
            h.push(make_version(&format!("v{i}"), i as u64, &["c"]));
        }
        prop_assert_eq!(h.len(), count);
        prop_assert_eq!(h.is_empty(), count == 0);
    }

    #[test]
    fn prop_cache_entry_expired_iff_age_ge_ttl(age in 0u64..10000, ttl in 0u64..10000) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cached_at = now.saturating_sub(age);
        let e = CacheEntry {
            id: "x".to_string(),
            cached_at,
            title: "T".to_string(),
        };
        let expired = e.is_expired(ttl);
        // Entry should be expired when age >= ttl
        if age >= ttl {
            prop_assert!(expired, "age={age} >= ttl={ttl} should be expired");
        }
    }
}
