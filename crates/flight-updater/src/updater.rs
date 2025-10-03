// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Main update manager coordinating all update operations

use crate::{
    channels::{Channel, ChannelManager},
    delta::{DeltaApplier, DeltaPatch},
    rollback::{RollbackManager, StartupCrashDetector, VersionInfo},
    signature::{SignatureManifest, SignatureVerifier},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;

/// Update configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Current installation directory
    pub install_dir: PathBuf,
    /// Update data directory
    pub update_dir: PathBuf,
    /// Current version
    pub current_version: String,
    /// Current channel
    pub channel: Channel,
    /// Automatic update checking
    pub auto_check: bool,
    /// Automatic update installation
    pub auto_install: bool,
    /// Maximum versions to keep for rollback
    pub max_rollback_versions: usize,
    /// Startup crash detection timeout
    pub startup_timeout_seconds: u64,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            install_dir: PathBuf::from("."),
            update_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("flight-hub")
                .join("updates"),
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            channel: Channel::Stable,
            auto_check: true,
            auto_install: false,
            max_rollback_versions: 3,
            startup_timeout_seconds: 60,
        }
    }
}

/// Update result information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    /// Whether an update was applied
    pub updated: bool,
    /// Previous version (if updated)
    pub previous_version: Option<String>,
    /// New version (if updated)
    pub new_version: Option<String>,
    /// Whether rollback occurred
    pub rollback_occurred: bool,
    /// Update channel used
    pub channel: Channel,
    /// Update size in bytes
    pub update_size: u64,
    /// Time taken for update
    pub duration_seconds: u64,
}

/// Available update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableUpdate {
    /// Version string
    pub version: String,
    /// Channel this update is from
    pub channel: Channel,
    /// Update size in bytes
    pub size: u64,
    /// Release notes URL
    pub release_notes_url: Option<String>,
    /// Whether this is a security update
    pub is_security_update: bool,
    /// Update priority (1-5, 5 being critical)
    pub priority: u8,
}

/// Main update manager
#[derive(Debug)]
pub struct UpdateManager {
    config: UpdateConfig,
    channel_manager: ChannelManager,
    rollback_manager: RollbackManager,
    crash_detector: StartupCrashDetector,
    delta_applier: DeltaApplier,
    http_client: reqwest::Client,
}

impl UpdateManager {
    /// Create a new update manager
    pub async fn new(config: UpdateConfig) -> crate::Result<Self> {
        // Create update directory
        fs::create_dir_all(&config.update_dir).await?;
        
        // Initialize rollback manager
        let mut rollback_manager = RollbackManager::new(
            &config.update_dir,
            config.max_rollback_versions,
        )?;
        rollback_manager.initialize().await?;
        
        // Initialize crash detector
        let startup_file = config.update_dir.join("startup_check");
        let crash_detector = StartupCrashDetector::new(
            startup_file,
            Duration::from_secs(config.startup_timeout_seconds),
        );
        
        // Initialize delta applier
        let delta_applier = DeltaApplier::new(&config.update_dir)?;
        
        // Create HTTP client with timeout
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(format!("flight-hub-updater/{}", config.current_version))
            .build()?;
        
        let mut channel_manager = ChannelManager::new();
        channel_manager.switch_channel(config.channel)?;
        
        Ok(Self {
            config,
            channel_manager,
            rollback_manager,
            crash_detector,
            delta_applier,
            http_client,
        })
    }
    
    /// Initialize update manager and handle startup crash detection
    pub async fn initialize(&mut self) -> crate::Result<Option<UpdateResult>> {
        tracing::info!("Initializing update manager");
        
        // Mark startup attempt
        self.crash_detector.mark_startup_attempt().await?;
        
        // Check for previous startup crash
        if self.crash_detector.check_previous_crash().await? {
            tracing::warn!("Previous startup crash detected, attempting rollback");
            return self.handle_startup_crash().await.map(Some);
        }
        
        // Record current version if not already recorded
        if let Some(current) = self.rollback_manager.current_version() {
            if current.version != self.config.current_version {
                self.record_current_version().await?;
            }
        } else {
            self.record_current_version().await?;
        }
        
        Ok(None)
    }
    
    /// Mark successful startup (call after application is fully initialized)
    pub async fn mark_startup_success(&self) -> crate::Result<()> {
        self.crash_detector.mark_startup_success().await
    }
    
    /// Check for available updates
    pub async fn check_for_updates(&self) -> crate::Result<Option<AvailableUpdate>> {
        let channel_config = self.channel_manager
            .get_config(self.config.channel)
            .ok_or_else(|| crate::UpdateError::ChannelNotFound(
                self.config.channel.to_string()
            ))?;
        
        tracing::info!("Checking for updates on {} channel", self.config.channel);
        
        // Construct update check URL
        let check_url = format!(
            "{}/check?current_version={}&channel={}",
            channel_config.update_url,
            self.config.current_version,
            self.config.channel
        );
        
        // Make HTTP request
        let response = self.http_client
            .get(&check_url)
            .send()
            .await?;
        
        if response.status().is_success() {
            let update_info: Option<AvailableUpdate> = response.json().await?;
            
            if let Some(ref update) = update_info {
                tracing::info!(
                    "Update available: {} -> {} ({})",
                    self.config.current_version,
                    update.version,
                    update.channel
                );
            } else {
                tracing::info!("No updates available");
            }
            
            Ok(update_info)
        } else {
            tracing::warn!("Update check failed: {}", response.status());
            Ok(None)
        }
    }
    
    /// Download and apply an update
    pub async fn apply_update(&mut self, update: &AvailableUpdate) -> crate::Result<UpdateResult> {
        let start_time = std::time::Instant::now();
        
        tracing::info!("Applying update to version {}", update.version);
        
        // Download update package
        let update_package = self.download_update(update).await?;
        
        // Verify signatures
        self.verify_update_package(&update_package).await?;
        
        // Apply delta patch
        let temp_install_dir = self.config.update_dir.join("temp_install");
        if temp_install_dir.exists() {
            fs::remove_dir_all(&temp_install_dir).await?;
        }
        fs::create_dir_all(&temp_install_dir).await?;
        
        // Copy current installation to temp directory
        self.copy_directory(&self.config.install_dir, &temp_install_dir).await?;
        
        // Apply patch to temp directory
        self.delta_applier.apply_patch(
            &update_package.delta_patch,
            &self.config.install_dir,
            &temp_install_dir,
        ).await?;
        
        // Record new version before replacing files
        let new_version_info = VersionInfo::new(
            update.version.clone(),
            "unknown".to_string(), // Would be filled from update metadata
            update.channel,
            self.config.install_dir.clone(),
        );
        
        self.rollback_manager.record_version(new_version_info).await?;
        
        // Replace installation atomically
        let backup_dir = self.config.update_dir.join("current_backup");
        if backup_dir.exists() {
            fs::remove_dir_all(&backup_dir).await?;
        }
        
        // Move current installation to backup
        fs::rename(&self.config.install_dir, &backup_dir).await?;
        
        // Move new installation to final location
        fs::rename(&temp_install_dir, &self.config.install_dir).await?;
        
        // Clean up backup
        fs::remove_dir_all(&backup_dir).await?;
        
        let duration = start_time.elapsed();
        
        Ok(UpdateResult {
            updated: true,
            previous_version: Some(self.config.current_version.clone()),
            new_version: Some(update.version.clone()),
            rollback_occurred: false,
            channel: update.channel,
            update_size: update.size,
            duration_seconds: duration.as_secs(),
        })
    }
    
    /// Perform manual rollback to previous version
    pub async fn rollback_to_previous(&mut self) -> crate::Result<UpdateResult> {
        tracing::info!("Performing manual rollback");
        
        let start_time = std::time::Instant::now();
        let previous_version = self.rollback_manager.rollback_to_previous().await?;
        let duration = start_time.elapsed();
        
        Ok(UpdateResult {
            updated: true,
            previous_version: Some(self.config.current_version.clone()),
            new_version: Some(previous_version.version),
            rollback_occurred: true,
            channel: previous_version.channel,
            update_size: 0,
            duration_seconds: duration.as_secs(),
        })
    }
    
    /// Get rollback targets
    pub fn get_rollback_targets(&self) -> Vec<&VersionInfo> {
        self.rollback_manager.rollback_targets()
    }
    
    /// Switch update channel
    pub async fn switch_channel(&mut self, channel: Channel) -> crate::Result<()> {
        self.channel_manager.switch_channel(channel)?;
        self.config.channel = channel;
        Ok(())
    }
    
    /// Handle startup crash by rolling back
    async fn handle_startup_crash(&mut self) -> crate::Result<UpdateResult> {
        tracing::error!("Handling startup crash with automatic rollback");
        
        let start_time = std::time::Instant::now();
        
        // Attempt rollback
        match self.rollback_manager.rollback_to_previous().await {
            Ok(previous_version) => {
                // Clear startup check file
                let _ = self.crash_detector.mark_startup_success().await;
                
                let duration = start_time.elapsed();
                
                Ok(UpdateResult {
                    updated: true,
                    previous_version: Some(self.config.current_version.clone()),
                    new_version: Some(previous_version.version),
                    rollback_occurred: true,
                    channel: previous_version.channel,
                    update_size: 0,
                    duration_seconds: duration.as_secs(),
                })
            }
            Err(e) => {
                tracing::error!("Rollback failed: {}", e);
                Err(e)
            }
        }
    }
    
    /// Record current version in rollback manager
    async fn record_current_version(&mut self) -> crate::Result<()> {
        let version_info = VersionInfo::new(
            self.config.current_version.clone(),
            "unknown".to_string(),
            self.config.channel,
            self.config.install_dir.clone(),
        );
        
        self.rollback_manager.record_version(version_info).await
    }
    
    /// Download update package
    async fn download_update(&self, update: &AvailableUpdate) -> crate::Result<UpdatePackage> {
        let channel_config = self.channel_manager
            .get_config(update.channel)
            .unwrap();
        
        let download_url = format!(
            "{}/download/{}/{}",
            channel_config.update_url,
            update.channel,
            update.version
        );
        
        tracing::info!("Downloading update from: {}", download_url);
        
        let response = self.http_client
            .get(&download_url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(crate::UpdateError::Network(
                reqwest::Error::from(response.error_for_status().unwrap_err())
            ));
        }
        
        let package_data = response.bytes().await?;
        
        // Parse update package (simplified - would use proper format)
        let package: UpdatePackage = serde_json::from_slice(&package_data)?;
        
        Ok(package)
    }
    
    /// Verify update package signatures
    async fn verify_update_package(&self, package: &UpdatePackage) -> crate::Result<()> {
        let channel_config = self.channel_manager
            .get_config(self.config.channel)
            .unwrap();
        
        let verifier = SignatureVerifier::new(&channel_config.public_key)?;
        
        // Verify package signature
        let package_data = serde_json::to_vec(&package.delta_patch)?;
        
        if !verifier.verify_content(&package_data, &package.signature).await? {
            return Err(crate::UpdateError::InvalidSignature(
                "Package signature verification failed".to_string()
            ));
        }
        
        // Verify signature timestamp
        if !verifier.verify_timestamp(&package.signature, 24) {
            return Err(crate::UpdateError::InvalidSignature(
                "Package signature is too old".to_string()
            ));
        }
        
        tracing::info!("Update package signature verified successfully");
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
}

/// Update package structure
#[derive(Debug, Serialize, Deserialize)]
struct UpdatePackage {
    /// Delta patch to apply
    delta_patch: DeltaPatch,
    /// Package signature
    signature: crate::signature::UpdateSignature,
    /// Signature manifest for all files
    manifest: SignatureManifest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_update_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = UpdateConfig {
            install_dir: temp_dir.path().join("install"),
            update_dir: temp_dir.path().join("updates"),
            ..Default::default()
        };
        
        let manager = UpdateManager::new(config).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_startup_crash_detection() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = UpdateConfig {
            install_dir: temp_dir.path().join("install"),
            update_dir: temp_dir.path().join("updates"),
            startup_timeout_seconds: 1, // Very short for testing
            ..Default::default()
        };
        
        let mut manager = UpdateManager::new(config).await.unwrap();
        
        // Initialize should mark startup attempt
        let result = manager.initialize().await.unwrap();
        assert!(result.is_none()); // No crash on first run
        
        // Mark success
        manager.mark_startup_success().await.unwrap();
    }

    #[test]
    fn test_update_result_serialization() {
        let result = UpdateResult {
            updated: true,
            previous_version: Some("1.0.0".to_string()),
            new_version: Some("1.1.0".to_string()),
            rollback_occurred: false,
            channel: Channel::Stable,
            update_size: 1024,
            duration_seconds: 30,
        };
        
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UpdateResult = serde_json::from_str(&json).unwrap();
        
        assert_eq!(result.updated, deserialized.updated);
        assert_eq!(result.previous_version, deserialized.previous_version);
        assert_eq!(result.new_version, deserialized.new_version);
    }
}