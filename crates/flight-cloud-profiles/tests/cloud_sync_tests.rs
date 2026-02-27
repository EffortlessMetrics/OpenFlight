// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for flight-cloud-profiles covering serialization,
//! pagination, sanitization, error handling, and proptest round-trips.

use chrono::TimeZone as _;
use chrono::Utc;
use flight_cloud_profiles::{
    CloudProfile, CloudProfileError, ListFilter, ProfileListing, ProfileSortOrder, PublishMeta,
    VoteDirection, VoteResult,
    models::Page,
    sanitize::{sanitize_for_upload, validate_for_publish},
};
use flight_profile::{AircraftId, AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use proptest::prelude::*;
use std::collections::HashMap;

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

fn profile_with_axis(name: &str, deadzone: f32, expo: f32) -> Profile {
    let mut axes = HashMap::new();
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

fn make_cloud_profile(id: &str, upvotes: u32, downvotes: u32) -> CloudProfile {
    CloudProfile {
        id: id.to_string(),
        title: "Test Profile".to_string(),
        description: None,
        author_handle: "pilot42".to_string(),
        upvotes,
        downvotes,
        download_count: 0,
        published_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        profile: minimal_profile(),
    }
}

fn make_listing(id: &str, upvotes: u32, downvotes: u32, download_count: u64) -> ProfileListing {
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

// ── Upload serialization ──────────────────────────────────────────────────────

/// The JSON body sent by `publish` must contain "title", "description", and "profile".
#[test]
fn publish_request_body_contains_title_description_and_nested_profile() {
    let profile = minimal_profile();
    let meta = PublishMeta::with_description("My C172 Profile", "Great default settings");

    // Mirror the body construction in CloudProfileClient::publish
    let body = serde_json::json!({
        "title": meta.title,
        "description": meta.description,
        "profile": &profile,
    });

    assert_eq!(body["title"], "My C172 Profile");
    assert_eq!(body["description"], "Great default settings");
    assert!(body["profile"].is_object(), "profile must be a JSON object");
    assert_eq!(body["profile"]["schema"], PROFILE_SCHEMA_VERSION);
}

#[test]
fn publish_meta_title_only_description_is_null_in_json() {
    let meta = PublishMeta::new("Title Only");
    let body = serde_json::json!({ "title": meta.title, "description": meta.description });
    assert_eq!(body["title"], "Title Only");
    assert!(body["description"].is_null());
}

// ── Download deserialization ──────────────────────────────────────────────────

#[test]
fn cloud_profile_round_trips_through_json() {
    let cp = make_cloud_profile("abc-123", 42, 3);
    let json = serde_json::to_string(&cp).unwrap();
    let back: CloudProfile = serde_json::from_str(&json).unwrap();

    assert_eq!(back.id, "abc-123");
    assert_eq!(back.upvotes, 42);
    assert_eq!(back.downvotes, 3);
    assert_eq!(back.score(), 39);
    assert_eq!(back.profile.sim.as_deref(), Some("msfs"));
    assert_eq!(back.profile.schema, PROFILE_SCHEMA_VERSION);
}

/// The client must not reject a profile that carries a schema version it does
/// not recognise — that is the server's responsibility.
#[test]
fn cloud_profile_with_legacy_schema_version_deserializes_successfully() {
    let json = r#"{
        "id": "legacy-001",
        "title": "Old Format",
        "description": null,
        "author_handle": "anon",
        "upvotes": 0,
        "downvotes": 0,
        "download_count": 0,
        "published_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "profile": {
            "schema": "flight.profile/0",
            "sim": null,
            "aircraft": null,
            "axes": {},
            "pof_overrides": null
        }
    }"#;
    let result: Result<CloudProfile, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "must accept profiles with unrecognised schema version"
    );
    assert_eq!(result.unwrap().profile.schema, "flight.profile/0");
}

// ── CloudProfile score ────────────────────────────────────────────────────────

#[test]
fn cloud_profile_score_positive() {
    assert_eq!(make_cloud_profile("id", 100, 10).score(), 90);
}

#[test]
fn cloud_profile_score_zero_when_balanced() {
    assert_eq!(make_cloud_profile("id", 5, 5).score(), 0);
}

#[test]
fn cloud_profile_score_negative() {
    assert_eq!(make_cloud_profile("id", 1, 10).score(), -9);
}

// ── Pagination ────────────────────────────────────────────────────────────────

#[test]
fn page_total_pages_exactly_divisible() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 25,
        total: 50,
    };
    assert_eq!(p.total_pages(), 2);
    assert!(p.has_next_page());
}

#[test]
fn page_total_pages_zero_items_means_zero_pages() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 25,
        total: 0,
    };
    assert_eq!(p.total_pages(), 0);
    assert!(!p.has_next_page());
}

/// `per_page == 0` must not panic (the guard in `total_pages` returns 0).
#[test]
fn page_zero_per_page_does_not_divide_by_zero() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 0,
        total: 100,
    };
    assert_eq!(p.total_pages(), 0);
}

#[test]
fn page_single_item_fits_on_one_page() {
    let p: Page<()> = Page {
        items: vec![],
        page: 1,
        per_page: 25,
        total: 1,
    };
    assert_eq!(p.total_pages(), 1);
    assert!(!p.has_next_page());
}

#[test]
fn page_on_last_page_has_no_next() {
    let p: Page<()> = Page {
        items: vec![],
        page: 4,
        per_page: 25,
        total: 100,
    };
    assert!(!p.has_next_page());
}

// ── Sanitize ──────────────────────────────────────────────────────────────────

#[test]
fn sanitize_for_upload_is_idempotent() {
    let profile = Profile {
        schema: "flight.profile/0".to_string(), // old version
        sim: Some("MSFS".to_string()),          // uppercase
        aircraft: Some(AircraftId {
            icao: "B738".to_string(),
        }),
        axes: {
            let mut m = HashMap::new();
            m.insert(
                "roll".to_string(),
                AxisConfig {
                    deadzone: Some(0.05),
                    expo: Some(0.3),
                    slew_rate: None,
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );
            m
        },
        pof_overrides: None,
    };

    let once = sanitize_for_upload(&profile);
    let twice = sanitize_for_upload(&once);

    assert_eq!(once.schema, twice.schema);
    assert_eq!(once.sim, twice.sim);
    assert_eq!(
        once.axes["roll"].deadzone, twice.axes["roll"].deadzone,
        "axis config should survive double sanitization unchanged"
    );
}

// ── validate_for_publish ──────────────────────────────────────────────────────

#[test]
fn validate_for_publish_accepts_exactly_100_char_title() {
    let title: String = "x".repeat(100);
    let profile = profile_with_axis("pitch", 0.05, 0.3);
    assert!(
        validate_for_publish(&profile, &title).is_ok(),
        "100-char title is at the boundary and must be accepted"
    );
}

#[test]
fn validate_for_publish_rejects_expo_above_one() {
    let profile = profile_with_axis("pitch", 0.05, 1.1);
    let err = validate_for_publish(&profile, "Good Title").unwrap_err();
    assert!(
        err.contains("expo"),
        "error should mention expo, got: {err}"
    );
}

#[test]
fn validate_for_publish_accepts_expo_at_boundaries() {
    // expo = 0.0 and 1.0 are both within [0.0, 1.0]
    assert!(validate_for_publish(&profile_with_axis("p", 0.05, 0.0), "T").is_ok());
    assert!(validate_for_publish(&profile_with_axis("p", 0.05, 1.0), "T").is_ok());
}

#[test]
fn validate_for_publish_accepts_zero_deadzone() {
    let profile = profile_with_axis("aileron", 0.0, 0.5);
    assert!(validate_for_publish(&profile, "Zero Deadzone").is_ok());
}

#[test]
fn validate_for_publish_accepts_max_deadzone() {
    let profile = profile_with_axis("rudder", 0.5, 0.0);
    assert!(
        validate_for_publish(&profile, "Max Deadzone").is_ok(),
        "deadzone=0.5 is the boundary and must be accepted"
    );
}

// ── Error display ─────────────────────────────────────────────────────────────

#[test]
fn error_api_error_display_includes_status_and_message() {
    let err = CloudProfileError::ApiError {
        status: 404,
        message: "profile not found".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("404"), "display should include status: {s}");
    assert!(
        s.contains("profile not found"),
        "display should include message: {s}"
    );
}

#[test]
fn error_invalid_argument_display_includes_reason() {
    let err = CloudProfileError::InvalidArgument("title required".to_string());
    let s = err.to_string();
    assert!(s.contains("title required"), "got: {s}");
}

#[test]
fn error_json_display_wraps_inner_message() {
    let inner: serde_json::Error = serde_json::from_str::<serde_json::Value>("???").unwrap_err();
    let err = CloudProfileError::Json(inner);
    let s = err.to_string();
    assert!(
        s.contains("JSON") || s.contains("json"),
        "should mention JSON: {s}"
    );
}

// ── VoteDirection ─────────────────────────────────────────────────────────────

#[test]
fn vote_direction_down_serializes_to_lowercase() {
    let json = serde_json::to_string(&VoteDirection::Down).unwrap();
    assert_eq!(json, r#""down""#);
    let back: VoteDirection = serde_json::from_str(&json).unwrap();
    assert_eq!(back, VoteDirection::Down);
}

#[test]
fn vote_direction_down_display() {
    assert_eq!(VoteDirection::Down.to_string(), "down");
}

#[test]
fn vote_result_score_computed_correctly() {
    let r = VoteResult {
        upvotes: 20,
        downvotes: 5,
        recorded: VoteDirection::Up,
    };
    assert_eq!(r.score(), 15);
}

// ── ListFilter ────────────────────────────────────────────────────────────────

#[test]
fn list_filter_with_all_fields_serializes_to_valid_json() {
    let filter = ListFilter {
        sim: Some("msfs".to_string()),
        aircraft_icao: Some("C172".to_string()),
        query: Some("cessna".to_string()),
        sort: ProfileSortOrder::Newest,
        page: 2,
        per_page: 10,
    };
    let json = serde_json::to_string(&filter).unwrap();
    assert!(json.contains("\"msfs\""), "sim missing: {json}");
    assert!(json.contains("\"C172\""), "icao missing: {json}");
    assert!(json.contains("newest"), "sort order missing: {json}");
}

#[test]
fn list_filter_default_first_page_no_filters() {
    let f = ListFilter::default();
    assert_eq!(f.page, 1);
    assert!(f.sim.is_none());
    assert!(f.query.is_none());
    assert!(f.aircraft_icao.is_none());
}

// ── proptest: ProfileListing round-trip ───────────────────────────────────────

proptest! {
    /// Any combination of vote counts and download counts must survive a
    /// JSON serialize → deserialize round-trip without data loss.
    #[test]
    fn profile_listing_json_round_trip(
        upvotes in 0u32..1_000_000u32,
        downvotes in 0u32..1_000_000u32,
        download_count in 0u64..10_000_000u64,
    ) {
        let listing = make_listing("test-id", upvotes, downvotes, download_count);
        let json = serde_json::to_string(&listing).expect("serialize");
        let back: ProfileListing = serde_json::from_str(&json).expect("deserialize");

        prop_assert_eq!(listing.upvotes, back.upvotes);
        prop_assert_eq!(listing.downvotes, back.downvotes);
        prop_assert_eq!(listing.download_count, back.download_count);
        prop_assert_eq!(listing.score(), back.score());
        prop_assert_eq!(listing.id, back.id);
    }
}
