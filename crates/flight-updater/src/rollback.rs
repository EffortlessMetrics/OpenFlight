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
            self.create_backup(current).await?;
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
                "No previous version available for rollback".to_string()
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
    
    /// Create a backup of the specified version
    async fn create_backup(&self, version: &VersionInfo) -> crate::Result<()> {
        let backup_name = format!("backup_{}_{}", version.version, version.install_timestamp);
        let backup_path = self.backups_dir.join(&backup_name);
        
        tracing::info!("Creating backup: {} -> {}", 
                      version.install_path.display(), 
                      backup_path.display());
        
        // Create backup directory
        fs::create_dir_all(&backup_path).await?;
        
        // Copy installation files to backup
        self.copy_directory(&version.install_path, &backup_path).await?;
        
        Ok(())
    }
    
    /// Restore from backup
    async fn restore_from_backup(&self, version: &VersionInfo) -> crate::Result<()> {
        if let Some(backup_path) = &version.backup_path {
            tracing::info!("Restoring from backup: {} -> {}", 
                          backup_path.display(), 
                          version.install_path.display());
            
            // Remove current installation
            if version.install_path.exists() {
                fs::remove_dir_all(&version.install_path).await?;
            }
            
            // Restore from backup
            self.copy_directory(backup_path, &version.install_path).await?;
        } else {
            return Err(crate::UpdateError::Rollback(
                format!("No backup available for version {}", version.version)
            ));
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
            self.version_history.sort_by(|a, b| b.install_timestamp.cmp(&a.install_timestamp));
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
    pub fn new<P: AsRef<Path>>(
        startup_file: P,
        startup_timeout: std::time::Duration,
    ) -> Self {
        Self {
            startup_file: startup_file.as_ref().to_path_buf(),
            startup_timeout,
        }
    }
    
    /// Mark startup attempt
    pub async fn mark_startup_attempt(&self) -> crate::Result<()> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        fs::write(&self.startup_file, timestamp.to_string()).await?;
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
        let startup_timestamp: u64 = content.parse()
            .map_err(|e| crate::UpdateError::Rollback(
                format!("Invalid startup timestamp: {}", e)
            ))?;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let elapsed = std::time::Duration::from_secs(now - startup_timestamp);
        
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
        let detector = StartupCrashDetector::new(
            &startup_file,
            std::time::Duration::from_secs(30),
        );
        
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
}