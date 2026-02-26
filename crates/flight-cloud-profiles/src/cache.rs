// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Local disk cache for downloaded cloud profiles.
//!
//! Profiles are stored as JSON files under the platform cache directory:
//!
//! - **Windows**: `%LOCALAPPDATA%\flight-hub\cloud-profiles\`
//! - **Linux/macOS**: `~/.cache/flight-hub/cloud-profiles/`
//!
//! Each profile is stored as `{id}.json`. A lightweight `cache_index.json`
//! tracks metadata (fetch time, profile size) for cache expiry decisions.
//!
//! Cache entries expire after [`CACHE_TTL_SECS`] seconds by default.

use crate::{CloudProfile, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default time-to-live for cached profile entries (1 hour).
pub const CACHE_TTL_SECS: u64 = 3600;

/// Filename for the cache metadata index.
const INDEX_FILE: &str = "cache_index.json";

/// Per-entry metadata stored in the cache index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Profile ID.
    pub id: String,
    /// Unix timestamp when this entry was cached.
    pub cached_at: u64,
    /// Profile title (for display without loading the full file).
    pub title: String,
}

impl CacheEntry {
    /// Returns `true` if this entry is older than `ttl_secs` seconds.
    pub fn is_expired(&self, ttl_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now.saturating_sub(self.cached_at) >= ttl_secs
    }
}

/// Local cache for cloud profiles.
///
/// # Example
///
/// ```no_run
/// use flight_cloud_profiles::cache::ProfileCache;
///
/// # tokio_test::block_on(async {
/// let cache = ProfileCache::default_dir()?;
/// // Store a profile
/// // cache.store(&cloud_profile).await?;
/// // let profile = cache.get("abc123").await?;
/// # Ok::<(), flight_cloud_profiles::CloudProfileError>(())
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct ProfileCache {
    dir: PathBuf,
    ttl_secs: u64,
}

impl ProfileCache {
    /// Create a cache rooted at `dir` with a custom TTL.
    pub fn new(dir: PathBuf, ttl_secs: u64) -> Self {
        Self { dir, ttl_secs }
    }

    /// Create a cache using the platform-default cache directory.
    ///
    /// Returns an error if the directory cannot be created.
    pub fn default_dir() -> Result<Self> {
        let dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flight-hub")
            .join("cloud-profiles");
        Ok(Self::new(dir, CACHE_TTL_SECS))
    }

    /// Returns the path to the cache directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Ensure the cache directory exists.
    async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        Ok(())
    }

    /// Path to the JSON file for `id`.
    fn profile_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Path to the index file.
    fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
    }

    /// Load the cache index, returning an empty index if the file is absent.
    async fn load_index(&self) -> Vec<CacheEntry> {
        let path = self.index_path();
        match tokio::fs::read_to_string(&path).await {
            Ok(data) => serde_json::from_str::<Vec<CacheEntry>>(&data).unwrap_or_default(),
            Err(_) => vec![],
        }
    }

    /// Persist the cache index.
    async fn save_index(&self, entries: &[CacheEntry]) -> Result<()> {
        self.ensure_dir().await?;
        let data = serde_json::to_string_pretty(entries)?;
        tokio::fs::write(self.index_path(), data).await?;
        Ok(())
    }

    /// Store a downloaded profile in the cache.
    pub async fn store(&self, profile: &CloudProfile) -> Result<()> {
        self.ensure_dir().await?;
        // Write profile JSON
        let data = serde_json::to_string_pretty(profile)?;
        tokio::fs::write(self.profile_path(&profile.id), data).await?;
        // Update index
        let mut index = self.load_index().await;
        index.retain(|e| e.id != profile.id);
        index.push(CacheEntry {
            id: profile.id.clone(),
            title: profile.title.clone(),
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        });
        self.save_index(&index).await
    }

    /// Retrieve a profile by ID.
    ///
    /// Returns `None` if the entry is absent or has expired.
    pub async fn get(&self, id: &str) -> Result<Option<CloudProfile>> {
        let index = self.load_index().await;
        let entry = match index.iter().find(|e| e.id == id) {
            Some(e) => e,
            None => return Ok(None),
        };
        if entry.is_expired(self.ttl_secs) {
            return Ok(None);
        }
        let path = self.profile_path(id);
        match tokio::fs::read_to_string(&path).await {
            Ok(data) => {
                let profile: CloudProfile = serde_json::from_str(&data)?;
                Ok(Some(profile))
            }
            Err(_) => Ok(None),
        }
    }

    /// List all non-expired entries in the index.
    pub async fn list_cached(&self) -> Vec<CacheEntry> {
        self.load_index()
            .await
            .into_iter()
            .filter(|e| !e.is_expired(self.ttl_secs))
            .collect()
    }

    /// Remove a specific profile from the cache.
    pub async fn evict(&self, id: &str) -> Result<()> {
        let path = self.profile_path(id);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        let mut index = self.load_index().await;
        index.retain(|e| e.id != id);
        self.save_index(&index).await
    }

    /// Clear all cached profiles.
    pub async fn clear(&self) -> Result<()> {
        let index = self.load_index().await;
        for entry in &index {
            let path = self.profile_path(&entry.id);
            if path.exists() {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }
        self.save_index(&[]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_not_expired_when_fresh() {
        let entry = CacheEntry {
            id: "x".to_string(),
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            title: "T".to_string(),
        };
        assert!(!entry.is_expired(CACHE_TTL_SECS));
    }

    #[test]
    fn test_cache_entry_expired_when_old() {
        let entry = CacheEntry {
            id: "x".to_string(),
            cached_at: 0, // epoch — definitely old
            title: "T".to_string(),
        };
        assert!(entry.is_expired(CACHE_TTL_SECS));
    }

    #[test]
    fn test_cache_entry_not_expired_within_ttl() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let entry = CacheEntry {
            id: "x".to_string(),
            cached_at: now - 100, // 100s ago
            title: "T".to_string(),
        };
        assert!(!entry.is_expired(200)); // TTL = 200s
        assert!(entry.is_expired(50)); // TTL = 50s
    }

    #[tokio::test]
    async fn test_cache_store_and_retrieve() {
        use chrono::Utc;
        use flight_profile::{AircraftId, Profile};
        use std::collections::HashMap;

        let tmp = tempfile_dir();
        let cache = ProfileCache::new(tmp.clone(), 3600);

        let profile = CloudProfile {
            id: "test-001".to_string(),
            title: "MSFS C172".to_string(),
            description: Some("Good defaults".to_string()),
            author_handle: "pilot42".to_string(),
            upvotes: 10,
            downvotes: 1,
            download_count: 50,
            published_at: Utc::now(),
            updated_at: Utc::now(),
            profile: Profile {
                schema: "flight.profile/1".to_string(),
                sim: Some("msfs".to_string()),
                aircraft: Some(AircraftId {
                    icao: "C172".to_string(),
                }),
                axes: HashMap::new(),
                pof_overrides: None,
            },
        };

        cache.store(&profile).await.expect("store should succeed");
        let retrieved = cache.get("test-001").await.expect("get should succeed");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "MSFS C172");

        cleanup_dir(&tmp);
    }

    #[tokio::test]
    async fn test_cache_miss_returns_none() {
        let tmp = tempfile_dir();
        let cache = ProfileCache::new(tmp.clone(), 3600);
        let result = cache.get("nonexistent").await.expect("should not error");
        assert!(result.is_none());
        cleanup_dir(&tmp);
    }

    #[tokio::test]
    async fn test_cache_evict_removes_entry() {
        use chrono::Utc;
        use flight_profile::{AircraftId, Profile};
        use std::collections::HashMap;

        let tmp = tempfile_dir();
        let cache = ProfileCache::new(tmp.clone(), 3600);

        let profile = CloudProfile {
            id: "evict-test".to_string(),
            title: "To Evict".to_string(),
            description: None,
            author_handle: "anon".to_string(),
            upvotes: 0,
            downvotes: 0,
            download_count: 0,
            published_at: Utc::now(),
            updated_at: Utc::now(),
            profile: Profile {
                schema: "flight.profile/1".to_string(),
                sim: None,
                aircraft: None,
                axes: HashMap::new(),
                pof_overrides: None,
            },
        };

        cache.store(&profile).await.unwrap();
        cache.evict("evict-test").await.unwrap();
        let result = cache.get("evict-test").await.unwrap();
        assert!(result.is_none());
        cleanup_dir(&tmp);
    }

    #[tokio::test]
    async fn test_cache_expired_entry_not_returned() {
        use chrono::Utc;
        use flight_profile::{AircraftId, Profile};
        use std::collections::HashMap;

        let tmp = tempfile_dir();
        let cache_write = ProfileCache::new(tmp.clone(), 3600);

        let profile = CloudProfile {
            id: "exp-test".to_string(),
            title: "Expired".to_string(),
            description: None,
            author_handle: "anon".to_string(),
            upvotes: 0,
            downvotes: 0,
            download_count: 0,
            published_at: Utc::now(),
            updated_at: Utc::now(),
            profile: Profile {
                schema: "flight.profile/1".to_string(),
                sim: None,
                aircraft: None,
                axes: HashMap::new(),
                pof_overrides: None,
            },
        };

        cache_write.store(&profile).await.unwrap();

        // Create a reader cache with TTL=0 so the entry is immediately expired
        let cache_read = ProfileCache::new(tmp.clone(), 0);
        let result = cache_read.get("exp-test").await.unwrap();
        assert!(result.is_none(), "expired entry should not be returned");
        cleanup_dir(&tmp);
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join("flight-cloud-profiles-test")
            .join(uuid_like());
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup_dir(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn uuid_like() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tid = std::thread::current().id();
        format!("test-{nanos:016x}-{seq:04x}-{tid:?}").replace(['(', ')', ' '], "_")
    }
}
