// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Rollback system for automatic recovery from failed updates

use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};
use tokio::fs;

/// Version information for installed software
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionInfo {
    /// Semantic version string
    pub version: String,
    /// Build timestamp
    pub build_timestamp: u64,
    /// Git commit hash
    pub commit_hash: String,
    /// Channel this version came from
    pub channel: crate::Channel,
    /// Installation timestamp
    pub install_timestamp: u64,
    /// Installation path
    pub install_path: PathBuf,
    /// Backup path for rollback
    pub backup_path: Option<PathBuf>,
}

impl VersionInfo {
    /// Create new version info
    pub fn new(
        version: String,
        commit_hash: String,
        channel: crate::Channel,
        install_path: PathBuf,
    ) -> Self {
        Self {
            version,
            build_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            commit_hash,
            channel,
            install_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            install_path,
            backup_path: None,
        }
    }

    /// Check if this version is newer than another
    pub fn is_newer_than(&self, other: &VersionInfo) -> bool {
        // Simple timestamp comparison for now
        // In production, would use proper semver comparison
        self.build_timestamp > other.build_timestamp
    }
}

/// Rollback manager handles version tracking and automatic rollback
#[derive(Debug)]
pub struct RollbackManager {
    /// Directory where version history is stored
    versions_dir: PathBuf,
    /// Directory where backups are stored
    backups_dir: PathBuf,
    /// Maximum number of versions to keep
    max_versions: usize,
    /// Current version info
    current_version: Option<VersionInfo>,
    /// Version history (newest first)
    version_history: Vec<VersionInfo>,
}

impl RollbackManager {
    /// Create a new rollback manager
    pub fn new<P: AsRef<Path>>(base_dir: P, max_versions: usize) -> crate::Result<Self> {
        let base_dir = base_dir.as_ref();
        let versions_dir = base_dir.join("versions");
        let backups_dir = base_dir.join("backups");

        Ok(Self {
            versions_dir,
            backups_dir,
            max_versions,
            current_version: None,
            version_history: Vec::new(),
        })
    }

    /// Initialize the rollback manager by loading existing version history
    pub async fn initialize(&mut self) -> crate::Result<()> {
        // Create directories if they don't exist
        fs::create_dir_all(&self.versions_dir).await?;
        fs::create_dir_all(&self.backups_dir).await?;

        // Load version history
        self.load_version_history().await?;

        // Set current version to the newest one
        if let Some(version) = self.version_history.first() {
            self.current_version = Some(version.clone());
        }

        // Clean up old versions beyond max_versions
        self.cleanup_old_versions().await?;

        Ok(())
    }

    /// Record a new version installation
    pub async fn record_version(&mut self, version_info: VersionInfo) -> crate::Result<()> {
        tracing::info!("Recording new version: {}", version_info.version);

        // Create backup of current version if it exists
        if let Some(current) = &self.current_version {
            let backup_path = self.create_backup(current).await?;
            // Store the backup path on the version in history so rollback can find it.
            if let Some(entry) = self.version_history.iter_mut().find(|v| {
                v.version == current.version && v.install_timestamp == current.install_timestamp
            }) {
                entry.backup_path = Some(backup_path);
            }
        }

        // Add to history (newest first)
        self.version_history.insert(0, version_info.clone());
        self.current_version = Some(version_info);

        // Save updated history
        self.save_version_history().await?;

        // Clean up old versions
        self.cleanup_old_versions().await?;

        Ok(())
    }

    /// Perform automatic rollback to the previous version
    pub async fn rollback_to_previous(&mut self) -> crate::Result<VersionInfo> {
        if self.version_history.len() < 2 {
            return Err(crate::UpdateError::Rollback(
                "No previous version available for rollback".to_string(),
            ));
        }

        let current = self.version_history[0].clone();
        let previous = self.version_history[1].clone();

        tracing::warn!(
            "Rolling back from version {} to {}",
            current.version,
            previous.version
        );

        // Restore from backup
        self.restore_from_backup(&previous).await?;

        // Update current version
        self.current_version = Some(previous.clone());

        // Remove the failed version from history
        self.version_history.remove(0);
        self.save_version_history().await?;

        Ok(previous)
    }

    /// Check if rollback is needed based on startup crash detection
    pub async fn check_startup_crash(&self, startup_timeout_seconds: u64) -> bool {
        if let Some(current) = &self.current_version {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let time_since_install = now - current.install_timestamp;

            // If the version was installed recently and we're checking for crashes,
            // it might indicate a startup failure
            if time_since_install < startup_timeout_seconds {
                tracing::warn!(
                    "Potential startup crash detected for version {} (installed {} seconds ago)",
                    current.version,
                    time_since_install
                );
                return true;
            }
        }

        false
    }

    /// Get current version info
    pub fn current_version(&self) -> Option<&VersionInfo> {
        self.current_version.as_ref()
    }

    /// Get version history (newest first)
    pub fn version_history(&self) -> &[VersionInfo] {
        &self.version_history
    }

    /// Get available rollback targets (excluding current)
    pub fn rollback_targets(&self) -> Vec<&VersionInfo> {
        if self.version_history.len() > 1 {
            self.version_history[1..].iter().collect()
        } else {
            Vec::new()
        }
    }

    /// Create a backup of the specified version. Returns the backup path on success.
    async fn create_backup(&self, version: &VersionInfo) -> crate::Result<PathBuf> {
        let backup_name = format!("backup_{}_{}", version.version, version.install_timestamp);
        let backup_path = self.backups_dir.join(&backup_name);

        tracing::info!(
            "Creating backup: {} -> {}",
            version.install_path.display(),
            backup_path.display()
        );

        // Fail if the install path doesn't exist — a missing source directory means
        // we cannot create a valid backup, which would make a future rollback silently
        // restore nothing.
        if !version.install_path.exists() {
            return Err(crate::UpdateError::Rollback(format!(
                "Cannot create backup for version {} — install path does not exist: {}",
                version.version,
                version.install_path.display()
            )));
        }

        // Create backup directory
        fs::create_dir_all(&backup_path).await?;

        // Copy installation files to backup
        self.copy_directory(&version.install_path, &backup_path)
            .await?;

        Ok(backup_path)
    }

    /// Restore from backup
    async fn restore_from_backup(&self, version: &VersionInfo) -> crate::Result<()> {
        if let Some(backup_path) = &version.backup_path {
            tracing::info!(
                "Restoring from backup: {} -> {}",
                backup_path.display(),
                version.install_path.display()
            );

            // Remove current installation
            if version.install_path.exists() {
                fs::remove_dir_all(&version.install_path).await?;
            }

            // Restore from backup
            self.copy_directory(backup_path, &version.install_path)
                .await?;
        } else {
            return Err(crate::UpdateError::Rollback(format!(
                "No backup available for version {} — cannot restore files",
                version.version
            )));
        }

        Ok(())
    }

    /// Copy directory iteratively (converted from recursive to avoid async recursion)
    async fn copy_directory(&self, src: &Path, dst: &Path) -> crate::Result<()> {
        fs::create_dir_all(dst).await?;

        let mut dirs_to_process = vec![(src.to_path_buf(), dst.to_path_buf())];

        while let Some((current_src, current_dst)) = dirs_to_process.pop() {
            let mut entries = fs::read_dir(&current_src).await?;

            while let Some(entry) = entries.next_entry().await? {
                let src_path = entry.path();
                let dst_path = current_dst.join(entry.file_name());

                if src_path.is_dir() {
                    fs::create_dir_all(&dst_path).await?;
                    dirs_to_process.push((src_path, dst_path));
                } else {
                    fs::copy(&src_path, &dst_path).await?;
                }
            }
        }

        Ok(())
    }

    /// Load version history from disk
    async fn load_version_history(&mut self) -> crate::Result<()> {
        let history_file = self.versions_dir.join("history.json");

        if history_file.exists() {
            let content = fs::read_to_string(&history_file).await?;
            self.version_history = serde_json::from_str(&content)?;

            // Sort by install timestamp (newest first)
            self.version_history
                .sort_by(|a, b| b.install_timestamp.cmp(&a.install_timestamp));
        }

        Ok(())
    }

    /// Save version history to disk
    async fn save_version_history(&self) -> crate::Result<()> {
        let history_file = self.versions_dir.join("history.json");
        let content = serde_json::to_string_pretty(&self.version_history)?;
        fs::write(&history_file, content).await?;
        Ok(())
    }

    /// Clean up old versions beyond max_versions limit
    async fn cleanup_old_versions(&mut self) -> crate::Result<()> {
        if self.version_history.len() > self.max_versions {
            let to_remove = self.version_history.split_off(self.max_versions);

            for version in to_remove {
                tracing::info!("Cleaning up old version: {}", version.version);

                // Remove backup if it exists
                if let Some(backup_path) = &version.backup_path {
                    if backup_path.exists() {
                        fs::remove_dir_all(backup_path).await?;
                    }
                }
            }

            self.save_version_history().await?;
        }

        Ok(())
    }
}

/// Startup crash detector
#[derive(Debug)]
pub struct StartupCrashDetector {
    /// File to track startup attempts
    startup_file: PathBuf,
    /// Timeout for successful startup
    startup_timeout: std::time::Duration,
}

impl StartupCrashDetector {
    /// Create a new startup crash detector
    pub fn new<P: AsRef<Path>>(startup_file: P, startup_timeout: std::time::Duration) -> Self {
        Self {
            startup_file: startup_file.as_ref().to_path_buf(),
            startup_timeout,
        }
    }

    /// Mark startup attempt
    pub async fn mark_startup_attempt(&self) -> crate::Result<()> {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        fs::write(&self.startup_file, timestamp_ms.to_string()).await?;
        Ok(())
    }

    /// Mark successful startup
    pub async fn mark_startup_success(&self) -> crate::Result<()> {
        if self.startup_file.exists() {
            fs::remove_file(&self.startup_file).await?;
        }
        Ok(())
    }

    /// Check if previous startup crashed
    pub async fn check_previous_crash(&self) -> crate::Result<bool> {
        if !self.startup_file.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(&self.startup_file).await?;
        let startup_timestamp_ms: u128 = content.parse().map_err(|e| {
            crate::UpdateError::Rollback(format!("Invalid startup timestamp: {}", e))
        })?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let elapsed_ms = now_ms.saturating_sub(startup_timestamp_ms);
        let elapsed_ms_u64: u64 = elapsed_ms.try_into().unwrap_or(u64::MAX);
        let elapsed = std::time::Duration::from_millis(elapsed_ms_u64);

        if elapsed > self.startup_timeout {
            tracing::warn!("Previous startup crash detected (elapsed: {:?})", elapsed);
            return Ok(true);
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_rollback_manager_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = RollbackManager::new(temp_dir.path(), 3).unwrap();

        assert!(manager.initialize().await.is_ok());
        assert!(manager.current_version().is_none());
        assert_eq!(manager.version_history().len(), 0);
    }

    #[tokio::test]
    async fn test_version_recording() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = RollbackManager::new(temp_dir.path(), 3).unwrap();
        manager.initialize().await.unwrap();

        let version = VersionInfo::new(
            "1.0.0".to_string(),
            "abc123".to_string(),
            crate::Channel::Stable,
            temp_dir.path().join("install"),
        );

        assert!(manager.record_version(version.clone()).await.is_ok());
        assert_eq!(manager.current_version().unwrap().version, "1.0.0");
        assert_eq!(manager.version_history().len(), 1);
    }

    #[tokio::test]
    async fn test_startup_crash_detector() {
        let temp_dir = TempDir::new().unwrap();
        let startup_file = temp_dir.path().join("startup");
        let detector = StartupCrashDetector::new(&startup_file, std::time::Duration::from_secs(30));

        // No crash initially
        assert!(!detector.check_previous_crash().await.unwrap());

        // Mark startup attempt
        detector.mark_startup_attempt().await.unwrap();

        // Should not detect crash immediately
        assert!(!detector.check_previous_crash().await.unwrap());

        // Mark success
        detector.mark_startup_success().await.unwrap();
        assert!(!detector.check_previous_crash().await.unwrap());
    }

    /// Recording two versions and rolling back should return (and activate) the
    /// previous version, removing the failed one from history.
    #[tokio::test]
    async fn test_rollback_to_previous_selects_correct_version() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = RollbackManager::new(temp_dir.path(), 5).unwrap();
        manager.initialize().await.unwrap();

        // Create real install directories so create_backup succeeds.
        let install_v1 = temp_dir.path().join("install_v1");
        let install_v2 = temp_dir.path().join("install_v2");
        fs::create_dir_all(&install_v1).await.unwrap();
        fs::write(install_v1.join("bin.exe"), b"v1-binary")
            .await
            .unwrap();
        fs::create_dir_all(&install_v2).await.unwrap();
        fs::write(install_v2.join("bin.exe"), b"v2-binary")
            .await
            .unwrap();

        let v1 = VersionInfo::new(
            "1.0.0".to_string(),
            "abc123".to_string(),
            crate::Channel::Stable,
            install_v1,
        );
        manager.record_version(v1).await.unwrap();

        let v2 = VersionInfo::new(
            "1.1.0".to_string(),
            "def456".to_string(),
            crate::Channel::Stable,
            install_v2,
        );
        manager.record_version(v2).await.unwrap();

        assert_eq!(manager.current_version().unwrap().version, "1.1.0");
        assert_eq!(manager.version_history().len(), 2);

        // The previous version (index 1) must have a backup_path set by record_version.
        assert!(
            manager.version_history[1].backup_path.is_some(),
            "previous version must have a backup_path after create_backup"
        );

        let rolled_back = manager.rollback_to_previous().await.unwrap();
        assert_eq!(
            rolled_back.version, "1.0.0",
            "rollback must select the previous version"
        );
        assert_eq!(
            manager.current_version().unwrap().version,
            "1.0.0",
            "current version must be updated after rollback"
        );
        assert_eq!(
            manager.version_history().len(),
            1,
            "failed version must be removed from history"
        );
    }

    /// Requesting rollback when there is only one recorded version must return an error.
    #[tokio::test]
    async fn test_rollback_with_single_version_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = RollbackManager::new(temp_dir.path(), 5).unwrap();
        manager.initialize().await.unwrap();

        // First record_version has no current_version, so create_backup is not called.
        // The install path need not exist.
        let v1 = VersionInfo::new(
            "1.0.0".to_string(),
            "abc123".to_string(),
            crate::Channel::Stable,
            temp_dir.path().join("install"),
        );
        manager.record_version(v1).await.unwrap();

        let result = manager.rollback_to_previous().await;
        assert!(
            result.is_err(),
            "rollback with no previous version must fail"
        );
    }

    /// Rollback on an empty manager (no versions recorded) must also return an error.
    #[tokio::test]
    async fn test_rollback_with_no_versions_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = RollbackManager::new(temp_dir.path(), 5).unwrap();
        manager.initialize().await.unwrap();

        let result = manager.rollback_to_previous().await;
        assert!(result.is_err(), "rollback with no versions must fail");
    }

    /// check_startup_crash respects the timeout threshold:
    /// - A very large timeout treats a just-installed version as a potential crash.
    /// - A zero-second timeout never triggers (install always happened at or after "now").
    #[tokio::test]
    async fn test_crash_count_threshold_respected() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = RollbackManager::new(temp_dir.path(), 5).unwrap();
        manager.initialize().await.unwrap();

        // First record_version — no current_version, so create_backup is not called.
        let install_path = temp_dir.path().join("install");
        let version = VersionInfo::new(
            "1.0.0".to_string(),
            "abc123".to_string(),
            crate::Channel::Stable,
            install_path,
        );
        manager.record_version(version).await.unwrap();

        // Version was just installed → time_since_install < u64::MAX/2 → returns true
        assert!(
            manager.check_startup_crash(u64::MAX / 2).await,
            "very long timeout must trigger for a just-installed version"
        );

        // Zero-second timeout means time_since_install (≥0) is never < 0 → returns false
        assert!(
            !manager.check_startup_crash(0).await,
            "zero timeout must never trigger"
        );
    }

    /// StartupCrashDetector reports a crash once the startup file is older than the timeout.
    #[tokio::test]
    async fn test_startup_crash_detector_detects_stale_startup_file() {
        let temp_dir = TempDir::new().unwrap();
        let startup_file = temp_dir.path().join("startup");

        // Write a timestamp far in the past (Unix epoch)
        fs::write(&startup_file, "1000").await.unwrap();

        let detector = StartupCrashDetector::new(&startup_file, std::time::Duration::from_secs(30));

        // The timestamp is ancient → elapsed >> 30 s → crash detected
        assert!(
            detector.check_previous_crash().await.unwrap(),
            "stale startup file must be detected as a crash"
        );
    }

    /// restore_from_backup must return an error when backup_path is None.
    #[tokio::test]
    async fn test_restore_from_backup_errors_when_no_backup_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RollbackManager::new(temp_dir.path(), 5).unwrap();

        let version = VersionInfo {
            version: "1.0.0".to_string(),
            build_timestamp: 0,
            commit_hash: "abc".to_string(),
            channel: crate::Channel::Stable,
            install_timestamp: 0,
            install_path: temp_dir.path().join("install"),
            backup_path: None,
        };

        let result = manager.restore_from_backup(&version).await;
        assert!(
            result.is_err(),
            "restore_from_backup must fail when backup_path is None"
        );
    }

    /// create_backup must return an error when the install path doesn't exist.
    #[tokio::test]
    async fn test_create_backup_errors_when_install_path_missing() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = RollbackManager::new(temp_dir.path(), 5).unwrap();
        manager.initialize().await.unwrap();

        let version = VersionInfo::new(
            "1.0.0".to_string(),
            "abc123".to_string(),
            crate::Channel::Stable,
            temp_dir.path().join("nonexistent"),
        );

        let result = manager.create_backup(&version).await;
        assert!(
            result.is_err(),
            "create_backup must fail when install path does not exist"
        );
    }

    /// check_previous_crash must safely handle u128→u64 conversion without
    /// silent truncation. We verify the fix by checking that a timestamp of 0
    /// correctly produces a large elapsed duration that exceeds a normal timeout.
    #[tokio::test]
    async fn test_check_previous_crash_handles_large_timestamp_difference() {
        let temp_dir = TempDir::new().unwrap();
        let startup_file = temp_dir.path().join("startup");

        // Write a timestamp of 0 — elapsed time from epoch to now is ~55 years.
        fs::write(&startup_file, "0").await.unwrap();

        // Use a 1-second timeout; elapsed (~55 years) easily exceeds it.
        let detector = StartupCrashDetector::new(&startup_file, std::time::Duration::from_secs(1));

        let result = detector.check_previous_crash().await.unwrap();
        assert!(
            result,
            "timestamp 0 must produce a large elapsed duration that exceeds timeout"
        );
    }
}
