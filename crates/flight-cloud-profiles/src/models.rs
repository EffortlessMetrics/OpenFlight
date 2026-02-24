// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! API data transfer objects for the cloud profile repository.

use chrono::{DateTime, Utc};
use flight_profile::Profile;
use serde::{Deserialize, Serialize};

/// Compact listing entry returned by the `GET /profiles` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileListing {
    /// Stable opaque profile identifier.
    pub id: String,
    /// Human-readable title set by the publisher.
    pub title: String,
    /// Optional short description.
    pub description: Option<String>,
    /// Target simulator slug (e.g. `"msfs"`, `"xplane"`, `"dcs"`).
    pub sim: Option<String>,
    /// Target aircraft ICAO type code.
    pub aircraft_icao: Option<String>,
    /// Anonymous publisher handle (never a real username or email).
    pub author_handle: String,
    /// Total upvotes received.
    pub upvotes: u32,
    /// Total downvotes received.
    pub downvotes: u32,
    /// Number of times this profile has been downloaded.
    pub download_count: u64,
    /// Profile schema version (e.g. `"flight.profile/1"`).
    pub schema_version: String,
    /// RFC-3339 publication timestamp.
    pub published_at: DateTime<Utc>,
    /// RFC-3339 timestamp of last update.
    pub updated_at: DateTime<Utc>,
}

impl ProfileListing {
    /// Net vote score (upvotes − downvotes).
    pub fn score(&self) -> i64 {
        self.upvotes as i64 - self.downvotes as i64
    }
}

/// Full profile detail returned by the `GET /profiles/{id}` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloudProfile {
    /// Stable opaque profile identifier.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
    /// Anonymous publisher handle.
    pub author_handle: String,
    /// Total upvotes.
    pub upvotes: u32,
    /// Total downvotes.
    pub downvotes: u32,
    /// Download count.
    pub download_count: u64,
    /// RFC-3339 publication timestamp.
    pub published_at: DateTime<Utc>,
    /// RFC-3339 last-update timestamp.
    pub updated_at: DateTime<Utc>,
    /// The actual profile data.
    pub profile: Profile,
}

impl CloudProfile {
    /// Net vote score.
    pub fn score(&self) -> i64 {
        self.upvotes as i64 - self.downvotes as i64
    }
}

/// Filter parameters for the `GET /profiles` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilter {
    /// Restrict results to a specific simulator (e.g. `"msfs"`).
    pub sim: Option<String>,
    /// Restrict results to a specific aircraft ICAO code.
    pub aircraft_icao: Option<String>,
    /// Free-text search across title and description.
    pub query: Option<String>,
    /// Sort order.
    #[serde(default)]
    pub sort: ProfileSortOrder,
    /// Page number (1-based).
    #[serde(default = "default_page")]
    pub page: u32,
    /// Profiles per page.
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

impl Default for ListFilter {
    fn default() -> Self {
        Self {
            sim: None,
            aircraft_icao: None,
            query: None,
            sort: ProfileSortOrder::default(),
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    crate::DEFAULT_PAGE_SIZE
}

/// Available sort orders for profile listings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSortOrder {
    /// Highest net score first (default).
    #[default]
    TopRated,
    /// Most recently published first.
    Newest,
    /// Most downloads first.
    MostDownloaded,
}

impl std::fmt::Display for ProfileSortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TopRated => f.write_str("top_rated"),
            Self::Newest => f.write_str("newest"),
            Self::MostDownloaded => f.write_str("most_downloaded"),
        }
    }
}

/// Metadata supplied when publishing a profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishMeta {
    /// Required: human-readable profile title.
    pub title: String,
    /// Optional short description (max 500 chars).
    pub description: Option<String>,
}

impl PublishMeta {
    /// Create publish metadata with just a title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
        }
    }

    /// Create publish metadata with title and description.
    pub fn with_description(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: Some(description.into()),
        }
    }
}

/// Direction of a vote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoteDirection {
    /// Positive vote.
    Up,
    /// Negative vote.
    Down,
}

impl std::fmt::Display for VoteDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Up => f.write_str("up"),
            Self::Down => f.write_str("down"),
        }
    }
}

/// Response body from a vote operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResult {
    /// Updated upvote count.
    pub upvotes: u32,
    /// Updated downvote count.
    pub downvotes: u32,
    /// The direction that was recorded.
    pub recorded: VoteDirection,
}

impl VoteResult {
    /// Net score after this vote.
    pub fn score(&self) -> i64 {
        self.upvotes as i64 - self.downvotes as i64
    }
}

/// Pagination wrapper used in list responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    /// Items on this page.
    pub items: Vec<T>,
    /// Current page number (1-based).
    pub page: u32,
    /// Items per page.
    pub per_page: u32,
    /// Total number of items across all pages.
    pub total: u64,
}

impl<T> Page<T> {
    /// Total number of pages.
    pub fn total_pages(&self) -> u64 {
        if self.per_page == 0 {
            0
        } else {
            self.total.div_ceil(self.per_page as u64)
        }
    }

    /// Returns `true` if there are more pages after this one.
    pub fn has_next_page(&self) -> bool {
        (self.page as u64) < self.total_pages()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use flight_profile::{AircraftId, Profile};
    use std::collections::HashMap;

    fn make_listing(upvotes: u32, downvotes: u32) -> ProfileListing {
        ProfileListing {
            id: "abc123".to_string(),
            title: "Test Profile".to_string(),
            description: None,
            sim: Some("msfs".to_string()),
            aircraft_icao: Some("C172".to_string()),
            author_handle: "pilot42".to_string(),
            upvotes,
            downvotes,
            download_count: 100,
            schema_version: "flight.profile/1".to_string(),
            published_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        }
    }

    #[test]
    fn test_listing_score_positive() {
        let l = make_listing(10, 2);
        assert_eq!(l.score(), 8);
    }

    #[test]
    fn test_listing_score_negative() {
        let l = make_listing(1, 5);
        assert_eq!(l.score(), -4);
    }

    #[test]
    fn test_listing_round_trip_json() {
        let l = make_listing(5, 1);
        let json = serde_json::to_string(&l).unwrap();
        let back: ProfileListing = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn test_sort_order_defaults_to_top_rated() {
        let filter = ListFilter::default();
        assert_eq!(filter.sort, ProfileSortOrder::TopRated);
        assert_eq!(filter.page, 1);
        assert_eq!(filter.per_page, crate::DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn test_sort_order_display() {
        assert_eq!(ProfileSortOrder::TopRated.to_string(), "top_rated");
        assert_eq!(ProfileSortOrder::Newest.to_string(), "newest");
        assert_eq!(ProfileSortOrder::MostDownloaded.to_string(), "most_downloaded");
    }

    #[test]
    fn test_vote_direction_round_trip() {
        let json = serde_json::to_string(&VoteDirection::Up).unwrap();
        assert_eq!(json, r#""up""#);
        let back: VoteDirection = serde_json::from_str(&json).unwrap();
        assert_eq!(back, VoteDirection::Up);
    }

    #[test]
    fn test_vote_result_score() {
        let r = VoteResult { upvotes: 10, downvotes: 3, recorded: VoteDirection::Up };
        assert_eq!(r.score(), 7);
    }

    #[test]
    fn test_page_total_pages() {
        let p: Page<()> = Page { items: vec![], page: 1, per_page: 25, total: 60 };
        assert_eq!(p.total_pages(), 3);
        assert!(p.has_next_page());
    }

    #[test]
    fn test_page_last_page_no_next() {
        let p: Page<()> = Page { items: vec![], page: 3, per_page: 25, total: 60 };
        assert!(!p.has_next_page());
    }

    #[test]
    fn test_publish_meta_new() {
        let m = PublishMeta::new("My Profile");
        assert_eq!(m.title, "My Profile");
        assert!(m.description.is_none());
    }

    #[test]
    fn test_publish_meta_with_description() {
        let m = PublishMeta::with_description("Title", "Desc");
        assert_eq!(m.description.as_deref(), Some("Desc"));
    }
}
