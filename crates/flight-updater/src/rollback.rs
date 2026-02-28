// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Rollback system for automatic recovery from failed updates

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use std::io;
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

// ═══════════════════════════════════════════════════════════════════════════
// Trait-based update rollback system with state machine and crash recovery
// ═══════════════════════════════════════════════════════════════════════════

/// Filesystem abstraction for testable update operations.
pub trait FileSystem: Clone {
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()>;
    fn append_file(&self, path: &Path, data: &[u8]) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
    fn is_dir(&self, path: &Path) -> bool;
}

/// Real filesystem implementation delegating to [`std::fs`].
#[derive(Clone)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        std::fs::write(path, data)
    }
    fn append_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(data)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            entries.push(entry?.path());
        }
        Ok(entries)
    }
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
}

/// Update lifecycle states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateState {
    Idle,
    Downloading,
    Verifying,
    Installing,
    RollingBack,
    Complete,
    Failed,
}

/// A single journal entry recording a state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub timestamp: u64,
    pub state: UpdateState,
    pub version: String,
    pub detail: String,
}

/// A downloaded artifact with its expected SHA-256 hash.
#[derive(Debug)]
pub struct ArtifactFile {
    pub path: PathBuf,
    pub expected_sha256: String,
}

/// Configuration for [`UpdateRollbackManager`].
#[derive(Debug)]
pub struct UpdateRollbackConfig {
    pub backup_dir: PathBuf,
    pub install_dir: PathBuf,
    pub journal_path: PathBuf,
    pub max_backups: usize,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Append-only log of update operations for crash recovery.
///
/// Records each state transition so that incomplete updates can be detected
/// and automatically rolled back on the next startup.
pub struct UpdateJournal<F: FileSystem> {
    journal_path: PathBuf,
    fs: F,
}

impl<F: FileSystem> UpdateJournal<F> {
    pub fn new(journal_path: PathBuf, fs: F) -> Self {
        Self { journal_path, fs }
    }

    /// Append a state transition entry to the journal.
    pub fn record(&self, state: &UpdateState, version: &str, detail: &str) -> crate::Result<()> {
        let entry = JournalEntry {
            timestamp: now_secs(),
            state: state.clone(),
            version: version.to_string(),
            detail: detail.to_string(),
        };
        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');
        self.fs
            .append_file(&self.journal_path, line.as_bytes())
            .map_err(|e| crate::UpdateError::Rollback(format!("Journal write failed: {e}")))?;
        Ok(())
    }

    /// Read all journal entries.
    pub fn entries(&self) -> crate::Result<Vec<JournalEntry>> {
        if !self.fs.exists(&self.journal_path) {
            return Ok(Vec::new());
        }
        let data = self
            .fs
            .read_file(&self.journal_path)
            .map_err(|e| crate::UpdateError::Rollback(format!("Journal read failed: {e}")))?;
        let text = String::from_utf8_lossy(&data);
        let mut entries = Vec::new();
        for line in text.lines() {
            if !line.trim().is_empty() {
                entries.push(serde_json::from_str(line)?);
            }
        }
        Ok(entries)
    }

    /// Check for an incomplete update (last state is not terminal).
    pub fn check_incomplete(&self) -> crate::Result<Option<JournalEntry>> {
        let entries = self.entries()?;
        match entries.last() {
            Some(last) => match last.state {
                UpdateState::Complete | UpdateState::Failed | UpdateState::Idle => Ok(None),
                _ => Ok(Some(last.clone())),
            },
            None => Ok(None),
        }
    }

    /// Clear the journal.
    pub fn clear(&self) -> crate::Result<()> {
        if self.fs.exists(&self.journal_path) {
            self.fs
                .write_file(&self.journal_path, b"")
                .map_err(|e| crate::UpdateError::Rollback(format!("Journal clear failed: {e}")))?;
        }
        Ok(())
    }
}

/// Manages the full update lifecycle with backup, verification, and rollback.
pub struct UpdateRollbackManager<F: FileSystem> {
    backup_dir: PathBuf,
    install_dir: PathBuf,
    max_backups: usize,
    state: UpdateState,
    journal: UpdateJournal<F>,
    fs: F,
}

impl<F: FileSystem> UpdateRollbackManager<F> {
    pub fn new(config: UpdateRollbackConfig, fs: F) -> crate::Result<Self> {
        let journal = UpdateJournal::new(config.journal_path, fs.clone());
        Ok(Self {
            backup_dir: config.backup_dir,
            install_dir: config.install_dir,
            max_backups: config.max_backups,
            state: UpdateState::Idle,
            journal,
            fs,
        })
    }

    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    pub fn journal(&self) -> &UpdateJournal<F> {
        &self.journal
    }

    fn transition(
        &mut self,
        new_state: UpdateState,
        version: &str,
        detail: &str,
    ) -> crate::Result<()> {
        self.journal.record(&new_state, version, detail)?;
        self.state = new_state;
        Ok(())
    }

    /// Copy current installation to a timestamped backup directory.
    pub fn backup_current_version(&self) -> crate::Result<PathBuf> {
        if !self.fs.exists(&self.install_dir) {
            return Err(crate::UpdateError::Rollback(
                "Install directory does not exist, cannot backup".to_string(),
            ));
        }
        self.fs
            .create_dir_all(&self.backup_dir)
            .map_err(|e| crate::UpdateError::Rollback(format!("Cannot create backup dir: {e}")))?;
        let backup_path = self.backup_dir.join(format!("backup_{}", now_secs()));
        self.copy_tree(&self.install_dir, &backup_path)?;
        Ok(backup_path)
    }

    /// Restore a backup to the installation directory.
    pub fn restore_backup(&self, backup_path: &Path) -> crate::Result<()> {
        if !self.fs.exists(backup_path) {
            return Err(crate::UpdateError::Rollback(format!(
                "Backup path does not exist: {}",
                backup_path.display()
            )));
        }
        if self.fs.exists(&self.install_dir) {
            self.fs.remove_dir_all(&self.install_dir).map_err(|e| {
                crate::UpdateError::Rollback(format!("Cannot remove install dir: {e}"))
            })?;
        }
        self.copy_tree(backup_path, &self.install_dir)
    }

    /// Check SHA-256 integrity of downloaded artifacts.
    pub fn verify_update_integrity(&self, artifacts: &[ArtifactFile]) -> crate::Result<()> {
        for artifact in artifacts {
            let data = self.fs.read_file(&artifact.path).map_err(|e| {
                crate::UpdateError::Rollback(format!(
                    "Cannot read artifact {}: {e}",
                    artifact.path.display()
                ))
            })?;
            let hash = hex::encode(Sha256::digest(&data));
            if hash != artifact.expected_sha256 {
                return Err(crate::UpdateError::Rollback(format!(
                    "SHA256 mismatch for {}: expected {}, got {}",
                    artifact.path.display(),
                    artifact.expected_sha256,
                    hash
                )));
            }
        }
        Ok(())
    }

    /// Run the full update state machine: verify → backup → install → complete.
    ///
    /// On verification failure the state moves directly to [`UpdateState::Failed`].
    /// On installation failure the manager performs a rollback before entering
    /// [`UpdateState::Failed`].
    pub fn apply_update(
        &mut self,
        artifacts: &[ArtifactFile],
        new_version: &str,
    ) -> crate::Result<()> {
        if self.state != UpdateState::Idle {
            return Err(crate::UpdateError::Rollback(format!(
                "Cannot start update from state {:?}",
                self.state
            )));
        }

        // Verify
        self.transition(
            UpdateState::Verifying,
            new_version,
            "Verifying artifact integrity",
        )?;
        if let Err(e) = self.verify_update_integrity(artifacts) {
            let _ = self.transition(UpdateState::Failed, new_version, &e.to_string());
            return Err(e);
        }

        // Install (backup first)
        self.transition(UpdateState::Installing, new_version, "Installing update")?;
        let backup_path = match self.backup_current_version() {
            Ok(p) => p,
            Err(e) => {
                let _ = self.transition(
                    UpdateState::Failed,
                    new_version,
                    &format!("Backup failed: {e}"),
                );
                return Err(e);
            }
        };

        if let Err(e) = self.install_artifacts(artifacts) {
            let _ = self.transition(
                UpdateState::RollingBack,
                new_version,
                &format!("Install failed: {e}"),
            );
            if let Err(re) = self.restore_backup(&backup_path) {
                let _ = self.transition(
                    UpdateState::Failed,
                    new_version,
                    &format!("Rollback also failed: {re}"),
                );
                return Err(crate::UpdateError::Rollback(format!(
                    "Install failed ({e}) and rollback failed ({re})"
                )));
            }
            let _ = self.transition(
                UpdateState::Failed,
                new_version,
                "Rolled back after install failure",
            );
            return Err(e);
        }

        self.transition(UpdateState::Complete, new_version, "Update complete")?;
        Ok(())
    }

    /// On startup, check the journal for incomplete updates and auto-rollback.
    pub fn recover_on_startup(&mut self) -> crate::Result<bool> {
        let incomplete = self.journal.check_incomplete()?;
        if let Some(entry) = incomplete {
            if self.fs.exists(&self.backup_dir) {
                let mut backups = self.fs.list_dir(&self.backup_dir).map_err(|e| {
                    crate::UpdateError::Rollback(format!("Cannot list backups: {e}"))
                })?;
                backups.sort();
                if let Some(latest) = backups.last().filter(|p| self.fs.is_dir(p)) {
                    self.transition(
                        UpdateState::RollingBack,
                        &entry.version,
                        "Auto-rollback on startup",
                    )?;
                    self.restore_backup(latest)?;
                    self.transition(UpdateState::Idle, &entry.version, "Recovery complete")?;
                    self.journal.clear()?;
                    return Ok(true);
                }
            }
            let _ = self.transition(
                UpdateState::Failed,
                &entry.version,
                "No backup found for recovery",
            );
            self.journal.clear()?;
            self.state = UpdateState::Idle;
            return Ok(false);
        }
        Ok(false)
    }

    /// Remove old backups, keeping at most `max_backups`.
    pub fn cleanup_backup(&self) -> crate::Result<()> {
        if !self.fs.exists(&self.backup_dir) {
            return Ok(());
        }
        let mut entries = self
            .fs
            .list_dir(&self.backup_dir)
            .map_err(|e| crate::UpdateError::Rollback(format!("Cannot list backup dir: {e}")))?;
        entries.retain(|p| self.fs.is_dir(p));
        entries.sort();
        if entries.len() > self.max_backups {
            let to_remove = entries.len() - self.max_backups;
            for path in entries.iter().take(to_remove) {
                self.fs.remove_dir_all(path).map_err(|e| {
                    crate::UpdateError::Rollback(format!(
                        "Cannot remove old backup {}: {e}",
                        path.display()
                    ))
                })?;
            }
        }
        Ok(())
    }

    fn install_artifacts(&self, artifacts: &[ArtifactFile]) -> crate::Result<()> {
        self.fs
            .create_dir_all(&self.install_dir)
            .map_err(|e| crate::UpdateError::Rollback(format!("Cannot create install dir: {e}")))?;
        for artifact in artifacts {
            let file_name = artifact.path.file_name().ok_or_else(|| {
                crate::UpdateError::Rollback(format!(
                    "Artifact path has no filename: {}",
                    artifact.path.display()
                ))
            })?;
            let data = self
                .fs
                .read_file(&artifact.path)
                .map_err(|e| crate::UpdateError::Rollback(format!("Cannot read artifact: {e}")))?;
            self.fs
                .write_file(&self.install_dir.join(file_name), &data)
                .map_err(|e| {
                    crate::UpdateError::Rollback(format!(
                        "Cannot write artifact to install dir: {e}"
                    ))
                })?;
        }
        Ok(())
    }

    fn copy_tree(&self, src: &Path, dst: &Path) -> crate::Result<()> {
        self.fs.create_dir_all(dst).map_err(|e| {
            crate::UpdateError::Rollback(format!("Cannot create dir {}: {e}", dst.display()))
        })?;
        let entries = self.fs.list_dir(src).map_err(|e| {
            crate::UpdateError::Rollback(format!("Cannot list dir {}: {e}", src.display()))
        })?;
        for entry in entries {
            let name = entry
                .file_name()
                .ok_or_else(|| crate::UpdateError::Rollback("Path has no filename".to_string()))?;
            let dest_entry = dst.join(name);
            if self.fs.is_dir(&entry) {
                self.copy_tree(&entry, &dest_entry)?;
            } else {
                let data = self.fs.read_file(&entry).map_err(|e| {
                    crate::UpdateError::Rollback(format!("Cannot read {}: {e}", entry.display()))
                })?;
                self.fs.write_file(&dest_entry, &data).map_err(|e| {
                    crate::UpdateError::Rollback(format!(
                        "Cannot write {}: {e}",
                        dest_entry.display()
                    ))
                })?;
            }
        }
        Ok(())
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

    // ═══════════════════════════════════════════════════════════════════════
    // MockFs and UpdateRollbackManager tests
    // ═══════════════════════════════════════════════════════════════════════

    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};
    use std::rc::Rc;

    #[derive(Clone)]
    struct MockFs {
        files: Rc<RefCell<HashMap<PathBuf, Vec<u8>>>>,
        dirs: Rc<RefCell<HashSet<PathBuf>>>,
        /// When `Some(n)`, the n-th `write_file` call (1-indexed) fails.
        write_fail_on_call: Rc<RefCell<Option<usize>>>,
        write_call_count: Rc<RefCell<usize>>,
    }

    impl MockFs {
        fn new_mock() -> Self {
            Self {
                files: Rc::new(RefCell::new(HashMap::new())),
                dirs: Rc::new(RefCell::new(HashSet::new())),
                write_fail_on_call: Rc::new(RefCell::new(None)),
                write_call_count: Rc::new(RefCell::new(0)),
            }
        }

        fn add_file(&self, path: &str, data: &[u8]) {
            let path = PathBuf::from(path);
            if let Some(parent) = path.parent() {
                self.ensure_parents(parent);
            }
            self.files.borrow_mut().insert(path, data.to_vec());
        }

        fn ensure_parents(&self, path: &Path) {
            let mut current = PathBuf::new();
            for component in path.components() {
                current.push(component);
                self.dirs.borrow_mut().insert(current.clone());
            }
        }

        fn get_file(&self, path: &str) -> Option<Vec<u8>> {
            self.files.borrow().get(&PathBuf::from(path)).cloned()
        }

        fn set_write_fail_on_call(&self, n: usize) {
            *self.write_fail_on_call.borrow_mut() = Some(n);
        }
    }

    impl FileSystem for MockFs {
        fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.files
                .borrow()
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.display().to_string()))
        }

        fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
            {
                let mut count = self.write_call_count.borrow_mut();
                *count += 1;
                if let Some(fail_on) = *self.write_fail_on_call.borrow() {
                    if *count == fail_on {
                        return Err(io::Error::other("Simulated write failure"));
                    }
                }
            }
            if let Some(parent) = path.parent() {
                self.ensure_parents(parent);
            }
            self.files
                .borrow_mut()
                .insert(path.to_path_buf(), data.to_vec());
            Ok(())
        }

        fn append_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
            if let Some(parent) = path.parent() {
                self.ensure_parents(parent);
            }
            self.files
                .borrow_mut()
                .entry(path.to_path_buf())
                .or_default()
                .extend_from_slice(data);
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            self.files
                .borrow_mut()
                .remove(path)
                .map(|_| ())
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not found"))
        }

        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.ensure_parents(path);
            Ok(())
        }

        fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
            let prefix = path.to_path_buf();
            self.files
                .borrow_mut()
                .retain(|k, _| !k.starts_with(&prefix));
            self.dirs.borrow_mut().retain(|k| !k.starts_with(&prefix));
            Ok(())
        }

        fn exists(&self, path: &Path) -> bool {
            self.files.borrow().contains_key(path) || self.dirs.borrow().contains(path)
        }

        fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
            if !self.is_dir(path) {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    path.display().to_string(),
                ));
            }
            let mut entries = HashSet::new();
            for key in self.files.borrow().keys() {
                if let Ok(suffix) = key.strip_prefix(path) {
                    if let Some(first) = suffix.components().next() {
                        entries.insert(path.join(first));
                    }
                }
            }
            for key in self.dirs.borrow().iter() {
                if let Ok(suffix) = key.strip_prefix(path) {
                    if let Some(first) = suffix.components().next() {
                        let entry = path.join(first);
                        if entry != *path {
                            entries.insert(entry);
                        }
                    }
                }
            }
            let mut result: Vec<_> = entries.into_iter().collect();
            result.sort();
            Ok(result)
        }

        fn is_dir(&self, path: &Path) -> bool {
            self.dirs.borrow().contains(path)
        }
    }

    fn sha256_hex(data: &[u8]) -> String {
        hex::encode(Sha256::digest(data))
    }

    fn make_config(base: &str) -> UpdateRollbackConfig {
        UpdateRollbackConfig {
            backup_dir: PathBuf::from(format!("{base}/backups")),
            install_dir: PathBuf::from(format!("{base}/install")),
            journal_path: PathBuf::from(format!("{base}/journal.log")),
            max_backups: 3,
        }
    }

    #[test]
    fn test_update_state_happy_path() {
        let fs = MockFs::new_mock();
        let config = make_config("t1");
        let mut mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        fs.add_file("t1/install/bin/app.exe", b"old-binary");

        let artifact_data = b"new-binary-v2";
        let hash = sha256_hex(artifact_data);
        fs.add_file("t1/downloads/app.exe", artifact_data);

        let artifacts = vec![ArtifactFile {
            path: PathBuf::from("t1/downloads/app.exe"),
            expected_sha256: hash,
        }];

        assert_eq!(mgr.state(), &UpdateState::Idle);
        mgr.apply_update(&artifacts, "2.0.0").unwrap();
        assert_eq!(mgr.state(), &UpdateState::Complete);
        assert_eq!(
            fs.get_file("t1/install/app.exe").unwrap(),
            artifact_data.to_vec()
        );
    }

    #[test]
    fn test_update_state_verification_failure() {
        let fs = MockFs::new_mock();
        let config = make_config("t2");
        let mut mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        fs.add_file("t2/install/bin/app.exe", b"old-binary");
        fs.add_file("t2/downloads/app.exe", b"new-binary");

        let artifacts = vec![ArtifactFile {
            path: PathBuf::from("t2/downloads/app.exe"),
            expected_sha256: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        }];

        let result = mgr.apply_update(&artifacts, "2.0.0");
        assert!(result.is_err());
        assert_eq!(mgr.state(), &UpdateState::Failed);
        // Original files must be untouched
        assert_eq!(
            fs.get_file("t2/install/bin/app.exe").unwrap(),
            b"old-binary"
        );
    }

    #[test]
    fn test_update_state_install_failure_triggers_rollback() {
        let fs = MockFs::new_mock();
        let config = make_config("t3");
        let mut mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        // Install dir with 2 files → backup creates 2 write_file calls
        fs.add_file("t3/install/a.bin", b"file-a");
        fs.add_file("t3/install/b.bin", b"file-b");

        let data = b"new-artifact";
        let hash = sha256_hex(data);
        fs.add_file("t3/downloads/artifact.bin", data);

        let artifacts = vec![ArtifactFile {
            path: PathBuf::from("t3/downloads/artifact.bin"),
            expected_sha256: hash,
        }];

        // Backup copies 2 files (write calls 1, 2). Install write is call 3.
        fs.set_write_fail_on_call(3);

        let result = mgr.apply_update(&artifacts, "2.0.0");
        assert!(result.is_err());
        assert_eq!(mgr.state(), &UpdateState::Failed);

        // After rollback, original files should be restored
        assert_eq!(fs.get_file("t3/install/a.bin").unwrap(), b"file-a");
        assert_eq!(fs.get_file("t3/install/b.bin").unwrap(), b"file-b");
    }

    #[test]
    fn test_journal_records_state_transitions() {
        let fs = MockFs::new_mock();
        let config = make_config("t4");
        let mut mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        fs.add_file("t4/install/app.exe", b"binary");

        let data = b"update-data";
        let hash = sha256_hex(data);
        fs.add_file("t4/downloads/app.exe", data);

        let artifacts = vec![ArtifactFile {
            path: PathBuf::from("t4/downloads/app.exe"),
            expected_sha256: hash,
        }];

        mgr.apply_update(&artifacts, "2.0.0").unwrap();

        let entries = mgr.journal().entries().unwrap();
        let states: Vec<_> = entries.iter().map(|e| e.state.clone()).collect();
        assert_eq!(
            states,
            vec![
                UpdateState::Verifying,
                UpdateState::Installing,
                UpdateState::Complete
            ]
        );
        assert!(entries.iter().all(|e| e.version == "2.0.0"));
    }

    #[test]
    fn test_journal_crash_recovery_restores_backup() {
        let fs = MockFs::new_mock();
        let config = make_config("t5");

        // Simulate a crashed update: journal shows Installing, backup exists
        let journal_line = serde_json::to_string(&JournalEntry {
            timestamp: 1000,
            state: UpdateState::Installing,
            version: "2.0.0".to_string(),
            detail: "Installing update".to_string(),
        })
        .unwrap()
            + "\n";
        fs.add_file("t5/journal.log", journal_line.as_bytes());
        fs.add_file("t5/backups/backup_0999/app.exe", b"v1-binary");
        fs.add_file("t5/install/app.exe", b"corrupted");

        let mut mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();
        let recovered = mgr.recover_on_startup().unwrap();

        assert!(recovered);
        assert_eq!(mgr.state(), &UpdateState::Idle);
        assert_eq!(fs.get_file("t5/install/app.exe").unwrap(), b"v1-binary");
    }

    #[test]
    fn test_journal_crash_recovery_no_incomplete() {
        let fs = MockFs::new_mock();
        let config = make_config("t6");

        let journal_line = serde_json::to_string(&JournalEntry {
            timestamp: 1000,
            state: UpdateState::Complete,
            version: "2.0.0".to_string(),
            detail: "done".to_string(),
        })
        .unwrap()
            + "\n";
        fs.add_file("t6/journal.log", journal_line.as_bytes());

        let mut mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();
        let recovered = mgr.recover_on_startup().unwrap();
        assert!(!recovered);
    }

    #[test]
    fn test_cleanup_backup_retention() {
        let fs = MockFs::new_mock();
        let mut config = make_config("t7");
        config.max_backups = 2;

        for i in 1..=4 {
            fs.add_file(
                &format!("t7/backups/backup_{i:04}/data.bin"),
                b"backup-data",
            );
        }

        let mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();
        mgr.cleanup_backup().unwrap();

        let remaining = fs.list_dir(Path::new("t7/backups")).unwrap();
        let remaining_dirs: Vec<_> = remaining.iter().filter(|p| fs.is_dir(p)).collect();
        assert_eq!(remaining_dirs.len(), 2);
        assert!(fs.exists(Path::new("t7/backups/backup_0003")));
        assert!(fs.exists(Path::new("t7/backups/backup_0004")));
        assert!(!fs.exists(Path::new("t7/backups/backup_0001")));
        assert!(!fs.exists(Path::new("t7/backups/backup_0002")));
    }

    #[test]
    fn test_verify_sha256_valid() {
        let fs = MockFs::new_mock();
        let config = make_config("t8");
        let mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        let data = b"hello world";
        let hash = sha256_hex(data);
        fs.add_file("t8/artifact.bin", data);

        let result = mgr.verify_update_integrity(&[ArtifactFile {
            path: PathBuf::from("t8/artifact.bin"),
            expected_sha256: hash,
        }]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_sha256_invalid() {
        let fs = MockFs::new_mock();
        let config = make_config("t9");
        let mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        fs.add_file("t9/artifact.bin", b"some data");

        let result = mgr.verify_update_integrity(&[ArtifactFile {
            path: PathBuf::from("t9/artifact.bin"),
            expected_sha256: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        }]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("SHA256 mismatch"));
    }

    #[test]
    fn test_backup_and_restore_round_trip() {
        let fs = MockFs::new_mock();
        let config = make_config("t10");
        let mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        fs.add_file("t10/install/bin/app.exe", b"my-binary");
        fs.add_file("t10/install/config/app.toml", b"[config]");

        let backup_path = mgr.backup_current_version().unwrap();
        assert!(fs.exists(&backup_path));

        // Destroy the install dir
        fs.remove_dir_all(Path::new("t10/install")).unwrap();
        assert!(!fs.exists(Path::new("t10/install/bin/app.exe")));

        // Restore
        mgr.restore_backup(&backup_path).unwrap();
        assert_eq!(
            fs.get_file("t10/install/bin/app.exe").unwrap(),
            b"my-binary"
        );
        assert_eq!(
            fs.get_file("t10/install/config/app.toml").unwrap(),
            b"[config]"
        );
    }

    #[test]
    fn test_apply_update_rejects_non_idle_state() {
        let fs = MockFs::new_mock();
        let config = make_config("t11");
        let mut mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        fs.add_file("t11/install/app.exe", b"binary");

        let data = b"update";
        let hash = sha256_hex(data);
        fs.add_file("t11/downloads/app.exe", data);

        let artifacts = vec![ArtifactFile {
            path: PathBuf::from("t11/downloads/app.exe"),
            expected_sha256: hash,
        }];

        mgr.apply_update(&artifacts, "2.0.0").unwrap();
        assert_eq!(mgr.state(), &UpdateState::Complete);

        // Second update must fail (not Idle)
        let result = mgr.apply_update(&artifacts, "3.0.0");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot start update")
        );
    }
}
