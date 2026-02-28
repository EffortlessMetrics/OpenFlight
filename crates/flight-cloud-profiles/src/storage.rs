// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Cloud storage backends for profile synchronization.
//!
//! Defines the [`CloudBackend`] trait and provides two implementations:
//!
//! - [`MockCloudBackend`] — in-memory backend for testing
//! - [`FileSystemBackend`] — local directory backend for testing and offline use

use crate::{CloudProfileError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Metadata about a profile stored in a cloud backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileMetadata {
    /// Unique profile identifier.
    pub id: String,
    /// Human-readable profile name.
    pub name: String,
    /// Unix timestamp of last modification.
    pub updated_at: u64,
    /// Size in bytes of the profile data.
    pub size: u64,
    /// SHA-256 checksum of the profile data.
    pub checksum: String,
}

/// Trait for cloud storage backends.
///
/// Implementations provide CRUD operations for profile data stored remotely
/// (or in a local surrogate for testing).
pub trait CloudBackend: Send + Sync {
    /// List metadata for all stored profiles.
    fn list_profiles(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<ProfileMetadata>>> + Send;

    /// Upload profile data with the given ID.
    fn upload(&self, id: &str, data: &[u8])
    -> impl std::future::Future<Output = Result<()>> + Send;

    /// Download profile data by ID.
    fn download(&self, id: &str) -> impl std::future::Future<Output = Result<Vec<u8>>> + Send;

    /// Delete a profile by ID.
    fn delete(&self, id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ── MockCloudBackend ─────────────────────────────────────────────────────────

/// In-memory cloud backend for testing.
#[derive(Debug)]
pub struct MockCloudBackend {
    store: Mutex<HashMap<String, (ProfileMetadata, Vec<u8>)>>,
}

impl MockCloudBackend {
    /// Create a new empty mock backend.
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MockCloudBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl CloudBackend for MockCloudBackend {
    async fn list_profiles(&self) -> Result<Vec<ProfileMetadata>> {
        let store = self
            .store
            .lock()
            .map_err(|_| CloudProfileError::InvalidArgument("lock poisoned".into()))?;
        Ok(store.values().map(|(meta, _)| meta.clone()).collect())
    }

    async fn upload(&self, id: &str, data: &[u8]) -> Result<()> {
        let checksum = format!("{:x}", Sha256::digest(data));
        let meta = ProfileMetadata {
            id: id.to_string(),
            name: id.to_string(),
            updated_at: now_secs(),
            size: data.len() as u64,
            checksum,
        };
        let mut store = self
            .store
            .lock()
            .map_err(|_| CloudProfileError::InvalidArgument("lock poisoned".into()))?;
        store.insert(id.to_string(), (meta, data.to_vec()));
        Ok(())
    }

    async fn download(&self, id: &str) -> Result<Vec<u8>> {
        let store = self
            .store
            .lock()
            .map_err(|_| CloudProfileError::InvalidArgument("lock poisoned".into()))?;
        store
            .get(id)
            .map(|(_, data)| data.clone())
            .ok_or_else(|| CloudProfileError::InvalidArgument(format!("profile not found: {id}")))
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| CloudProfileError::InvalidArgument("lock poisoned".into()))?;
        store
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| CloudProfileError::InvalidArgument(format!("profile not found: {id}")))
    }
}

// ── FileSystemBackend ────────────────────────────────────────────────────────

const FS_INDEX_FILE: &str = "fs_backend_index.json";

/// Local filesystem cloud backend for testing and offline use.
///
/// Stores profile data as files in a directory with a JSON metadata index.
#[derive(Debug, Clone)]
pub struct FileSystemBackend {
    dir: PathBuf,
}

impl FileSystemBackend {
    /// Create a backend rooted at the given directory.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Returns the root directory path.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn data_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.dat"))
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join(FS_INDEX_FILE)
    }

    async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        Ok(())
    }

    async fn load_index(&self) -> HashMap<String, ProfileMetadata> {
        let path = self.index_path();
        match tokio::fs::read_to_string(&path).await {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    async fn save_index(&self, index: &HashMap<String, ProfileMetadata>) -> Result<()> {
        self.ensure_dir().await?;
        let data = serde_json::to_string_pretty(index)?;
        tokio::fs::write(self.index_path(), data).await?;
        Ok(())
    }
}

impl CloudBackend for FileSystemBackend {
    async fn list_profiles(&self) -> Result<Vec<ProfileMetadata>> {
        Ok(self.load_index().await.into_values().collect())
    }

    async fn upload(&self, id: &str, data: &[u8]) -> Result<()> {
        self.ensure_dir().await?;
        let checksum = format!("{:x}", Sha256::digest(data));
        let meta = ProfileMetadata {
            id: id.to_string(),
            name: id.to_string(),
            updated_at: now_secs(),
            size: data.len() as u64,
            checksum,
        };

        tokio::fs::write(self.data_path(id), data).await?;

        let mut index = self.load_index().await;
        index.insert(id.to_string(), meta);
        self.save_index(&index).await
    }

    async fn download(&self, id: &str) -> Result<Vec<u8>> {
        let path = self.data_path(id);
        tokio::fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CloudProfileError::InvalidArgument(format!("profile not found: {id}"))
            } else {
                CloudProfileError::Cache(e)
            }
        })
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let path = self.data_path(id);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        let mut index = self.load_index().await;
        index.remove(id);
        self.save_index(&index).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── MockCloudBackend ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_mock_backend_upload_and_download() {
        let backend = MockCloudBackend::new();
        backend.upload("p1", b"profile data").await.unwrap();
        let data = backend.download("p1").await.unwrap();
        assert_eq!(data, b"profile data");
    }

    #[tokio::test]
    async fn test_mock_backend_list_profiles() {
        let backend = MockCloudBackend::new();
        backend.upload("p1", b"data1").await.unwrap();
        backend.upload("p2", b"data2").await.unwrap();
        let list = backend.list_profiles().await.unwrap();
        assert_eq!(list.len(), 2);
        let ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"p1"));
        assert!(ids.contains(&"p2"));
    }

    #[tokio::test]
    async fn test_mock_backend_delete() {
        let backend = MockCloudBackend::new();
        backend.upload("p1", b"data").await.unwrap();
        backend.delete("p1").await.unwrap();
        assert!(backend.download("p1").await.is_err());
        assert!(backend.list_profiles().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_mock_backend_download_missing() {
        let backend = MockCloudBackend::new();
        let result = backend.download("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_backend_upload_overwrites() {
        let backend = MockCloudBackend::new();
        backend.upload("p1", b"original").await.unwrap();
        backend.upload("p1", b"updated").await.unwrap();
        let data = backend.download("p1").await.unwrap();
        assert_eq!(data, b"updated");
        assert_eq!(backend.list_profiles().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_mock_backend_metadata_has_correct_checksum() {
        let backend = MockCloudBackend::new();
        backend.upload("p1", b"test data").await.unwrap();
        let list = backend.list_profiles().await.unwrap();
        let meta = list.iter().find(|m| m.id == "p1").unwrap();
        let expected = format!("{:x}", Sha256::digest(b"test data"));
        assert_eq!(meta.checksum, expected);
        assert_eq!(meta.size, 9);
    }

    #[tokio::test]
    async fn test_mock_backend_delete_missing_returns_error() {
        let backend = MockCloudBackend::new();
        assert!(backend.delete("nonexistent").await.is_err());
    }

    // ── FileSystemBackend ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_filesystem_backend_upload_and_download() {
        let tmp = tempfile_dir();
        let backend = FileSystemBackend::new(&tmp);
        backend.upload("p1", b"fs profile data").await.unwrap();
        let data = backend.download("p1").await.unwrap();
        assert_eq!(data, b"fs profile data");
        cleanup_dir(&tmp);
    }

    #[tokio::test]
    async fn test_filesystem_backend_list() {
        let tmp = tempfile_dir();
        let backend = FileSystemBackend::new(&tmp);
        backend.upload("a", b"data-a").await.unwrap();
        backend.upload("b", b"data-b").await.unwrap();
        let list = backend.list_profiles().await.unwrap();
        assert_eq!(list.len(), 2);
        cleanup_dir(&tmp);
    }

    #[tokio::test]
    async fn test_filesystem_backend_delete() {
        let tmp = tempfile_dir();
        let backend = FileSystemBackend::new(&tmp);
        backend.upload("p1", b"data").await.unwrap();
        backend.delete("p1").await.unwrap();
        assert!(backend.download("p1").await.is_err());
        assert!(backend.list_profiles().await.unwrap().is_empty());
        cleanup_dir(&tmp);
    }

    #[tokio::test]
    async fn test_filesystem_backend_download_missing() {
        let tmp = tempfile_dir();
        let backend = FileSystemBackend::new(&tmp);
        let result = backend.download("nonexistent").await;
        assert!(result.is_err());
        cleanup_dir(&tmp);
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join("flight-cloud-storage-test")
            .join(uuid_like());
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup_dir(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn uuid_like() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
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
