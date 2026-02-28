// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Community Cloud Profile Repository
//!
//! Provides a client for the Flight Hub community profile repository,
//! allowing users to browse, download, publish, and vote on shared
//! flight control profiles.
//!
//! # Overview
//!
//! - **Browse** profiles filtered by simulator and aircraft
//! - **Download** a community profile and apply it locally
//! - **Publish** a sanitized local profile to the community repository
//! - **Vote** on profiles (thumbs up / thumbs down)
//! - **Offline cache** — profiles are cached locally for use without connectivity
//!
//! # Default API endpoint
//!
//! The default base URL is [`DEFAULT_API_BASE_URL`]. Override via
//! [`ClientConfig::base_url`] for self-hosted or staging environments.
//!
//! # Example
//!
//! ```no_run
//! use flight_cloud_profiles::{CloudProfileClient, ClientConfig, ListFilter};
//!
//! # tokio_test::block_on(async {
//! let client = CloudProfileClient::new(ClientConfig::default())?;
//! let profiles = client.list(ListFilter::default()).await?;
//! for p in &profiles {
//!     println!("{} — {} votes", p.id, p.upvotes);
//! }
//! # Ok::<(), flight_cloud_profiles::CloudProfileError>(())
//! # });
//! ```

pub mod cache;
pub mod client;
pub mod models;
pub mod sanitize;
pub mod storage;
pub mod sync;
pub mod versioning;

pub use cache::ProfileCache;
pub use client::{ClientConfig, CloudProfileClient};
pub use flight_profile::Profile;
pub use models::{
    CloudProfile, ListFilter, ProfileListing, ProfileSortOrder, PublishMeta, VoteDirection,
    VoteResult,
};
pub use sanitize::sanitize_for_upload;
pub use storage::{CloudBackend, FileSystemBackend, MockCloudBackend, ProfileMetadata};
pub use sync::{
    ConflictResolution, ConflictStrategy, ProfileAction, ProfileSyncState, SyncConflict,
    SyncEngine, SyncPlan,
};
pub use versioning::{ProfileVersion, VersionDiff, VersionHistory, compute_version_hash};

/// Default community profile repository API base URL.
pub const DEFAULT_API_BASE_URL: &str = "https://profiles.flighthub.io/api/v1";

/// Default request timeout in seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Default maximum number of profiles returned per page.
pub const DEFAULT_PAGE_SIZE: u32 = 25;

/// Top-level error type.
#[derive(Debug, thiserror::Error)]
pub enum CloudProfileError {
    /// HTTP transport error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// The server returned an error status.
    #[error("Server error {status}: {message}")]
    ApiError { status: u16, message: String },

    /// Serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Local cache I/O error.
    #[error("Cache I/O error: {0}")]
    Cache(#[from] std::io::Error),

    /// A required field was missing or invalid.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, CloudProfileError>;
