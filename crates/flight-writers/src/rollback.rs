//! Configuration rollback system

use crate::types::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Manages configuration rollbacks
pub struct RollbackManager {
    backup_dir: PathBuf,
}

/// Metadata for a backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub id: String,
    pub timestamp: u64,
    pub sim: SimulatorType,
    pub version: String,
    pub description: String,
    pub files: Vec<BackupFileInfo>,
}

/// Information about a backed up file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFileInfo {
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
    pub checksum: String,
}

impl RollbackManager {
    pub fn new<P: AsRef<Path>>(backup_dir: P) -> Self {
        Self {
            backup_dir: backup_dir.as_ref().to_path_buf(),
        }
    }

    /// Create a backup before applying changes
    pub async fn create_backup(
        &self,
        backup_id: &str,
        sim: SimulatorType,
        version: &str,
        description: &str,
        files_to_backup: &[PathBuf],
    ) -> Result<BackupMetadata> {
        info!("Creating backup {} for {} {}", backup_id, sim, version);

        let backup_path = self.backup_dir.join(backup_id);
        fs::create_dir_all(&backup_path)
            .context("Failed to create backup directory")?;

        let mut backup_files = Vec::new();

        for file_path in files_to_backup {
            if file_path.exists() {
                let backup_file_info = self.backup_single_file(file_path, &backup_path).await?;
                backup_files.push(backup_file_info);
            } else {
                debug!("Skipping non-existent file: {:?}", file_path);
            }
        }

        let metadata = BackupMetadata {
            id: backup_id.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            sim,
            version: version.to_string(),
            description: description.to_string(),
            files: backup_files,
        };

        // Save metadata
        let metadata_path = backup_path.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize backup metadata")?;
        fs::write(&metadata_path, metadata_json)
            .context("Failed to write backup metadata")?;

        info!("Created backup {} with {} files", backup_id, metadata.files.len());
        Ok(metadata)
    }

    /// Backup a single file
    async fn backup_single_file(
        &self,
        original_path: &Path,
        backup_dir: &Path,
    ) -> Result<BackupFileInfo> {
        let file_name = original_path
            .file_name()
            .context("Invalid file path")?
            .to_string_lossy();
        
        let _backup_file_path = backup_dir.join(&*file_name);
        
        // Create subdirectories if needed to preserve structure
        if let Some(parent) = original_path.parent() {
            let backup_subdir = backup_dir.join("files").join(parent);
            fs::create_dir_all(&backup_subdir)
                .context("Failed to create backup subdirectory")?;
        }

        // For simplicity, just use the filename in the backup directory
        let structured_backup_path = backup_dir.join("files").join(&*file_name);

        // Ensure parent directory exists
        if let Some(parent) = structured_backup_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create backup parent directory")?;
        }

        // Copy the file using read/write to avoid file locking issues
        let content = fs::read(original_path)
            .context("Failed to read original file")?;
        fs::write(&structured_backup_path, content)
            .context("Failed to write backup file")?;

        // Calculate checksum
        let checksum = self.calculate_file_checksum(original_path)?;

        debug!("Backed up {:?} -> {:?}", original_path, structured_backup_path);

        Ok(BackupFileInfo {
            original_path: original_path.to_path_buf(),
            backup_path: structured_backup_path,
            checksum,
        })
    }

    /// Calculate SHA-256 checksum of a file
    fn calculate_file_checksum(&self, file_path: &Path) -> Result<String> {
        use std::io::Read;
        
        let mut file = fs::File::open(file_path)
            .context("Failed to open file for checksum")?;
        
        let mut hasher = sha2::Sha256::new();
        let mut buffer = [0; 8192];
        
        loop {
            let bytes_read = file.read(&mut buffer)
                .context("Failed to read file for checksum")?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Rollback to a previous backup
    pub async fn rollback(&self, backup_id: &str) -> Result<RollbackResult> {
        info!("Rolling back to backup {}", backup_id);

        let backup_path = self.backup_dir.join(backup_id);
        if !backup_path.exists() {
            return Ok(RollbackResult {
                success: false,
                restored_files: vec![],
                errors: vec![format!("Backup {} not found", backup_id)],
            });
        }

        // Load backup metadata
        let metadata_path = backup_path.join("metadata.json");
        let metadata_content = fs::read_to_string(&metadata_path)
            .context("Failed to read backup metadata")?;
        
        let metadata: BackupMetadata = serde_json::from_str(&metadata_content)
            .context("Failed to parse backup metadata")?;

        let mut restored_files = Vec::new();
        let mut errors = Vec::new();

        // Restore each file
        for file_info in &metadata.files {
            match self.restore_single_file(file_info).await {
                Ok(()) => {
                    restored_files.push(file_info.original_path.clone());
                    debug!("Restored {:?}", file_info.original_path);
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to restore {:?}: {}",
                        file_info.original_path, e
                    ));
                }
            }
        }

        let success = errors.is_empty();
        
        if success {
            info!("Successfully rolled back {} files", restored_files.len());
        } else {
            warn!("Rollback completed with {} errors", errors.len());
        }

        Ok(RollbackResult {
            success,
            restored_files,
            errors,
        })
    }

    /// Restore a single file from backup
    async fn restore_single_file(&self, file_info: &BackupFileInfo) -> Result<()> {
        if !file_info.backup_path.exists() {
            anyhow::bail!("Backup file not found: {:?}", file_info.backup_path);
        }

        // Ensure target directory exists
        if let Some(parent) = file_info.original_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create target directory")?;
        }

        // Copy the backup file back to original location
        let backup_content = fs::read(&file_info.backup_path)
            .context("Failed to read backup file")?;
        fs::write(&file_info.original_path, backup_content)
            .context("Failed to restore file from backup")?;

        // Verify checksum if the original file still exists
        if file_info.original_path.exists() {
            let current_checksum = self.calculate_file_checksum(&file_info.original_path)?;
            if current_checksum != file_info.checksum {
                warn!(
                    "Checksum mismatch after restore for {:?}: expected {}, got {}",
                    file_info.original_path, file_info.checksum, current_checksum
                );
            }
        }

        Ok(())
    }

    /// List available backups
    pub async fn list_backups(&self) -> Result<Vec<BackupMetadata>> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let metadata_path = entry.path().join("metadata.json");
                if metadata_path.exists() {
                    match self.load_backup_metadata(&metadata_path).await {
                        Ok(metadata) => backups.push(metadata),
                        Err(e) => {
                            warn!("Failed to load backup metadata from {:?}: {}", metadata_path, e);
                        }
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }

    /// Load backup metadata from file
    async fn load_backup_metadata(&self, metadata_path: &Path) -> Result<BackupMetadata> {
        let content = fs::read_to_string(metadata_path)
            .context("Failed to read metadata file")?;
        
        serde_json::from_str(&content)
            .context("Failed to parse metadata JSON")
    }

    /// Delete old backups to free space
    pub async fn cleanup_old_backups(&self, keep_count: usize) -> Result<usize> {
        let backups = self.list_backups().await?;
        
        if backups.len() <= keep_count {
            return Ok(0);
        }

        let mut deleted_count = 0;
        
        // Delete oldest backups beyond the keep count
        for backup in backups.iter().skip(keep_count) {
            let backup_path = self.backup_dir.join(&backup.id);
            if backup_path.exists() {
                match fs::remove_dir_all(&backup_path) {
                    Ok(()) => {
                        deleted_count += 1;
                        info!("Deleted old backup: {}", backup.id);
                    }
                    Err(e) => {
                        warn!("Failed to delete backup {}: {}", backup.id, e);
                    }
                }
            }
        }

        info!("Cleaned up {} old backups", deleted_count);
        Ok(deleted_count)
    }

    /// Get backup information
    pub async fn get_backup_info(&self, backup_id: &str) -> Result<Option<BackupMetadata>> {
        let metadata_path = self.backup_dir.join(backup_id).join("metadata.json");
        
        if !metadata_path.exists() {
            return Ok(None);
        }

        let metadata = self.load_backup_metadata(&metadata_path).await?;
        Ok(Some(metadata))
    }

    /// Verify backup integrity
    pub async fn verify_backup(&self, backup_id: &str) -> Result<BackupVerificationResult> {
        let metadata = match self.get_backup_info(backup_id).await? {
            Some(metadata) => metadata,
            None => {
                return Ok(BackupVerificationResult {
                    backup_id: backup_id.to_string(),
                    valid: false,
                    errors: vec!["Backup not found".to_string()],
                    verified_files: 0,
                    total_files: 0,
                });
            }
        };

        let mut errors = Vec::new();
        let mut verified_files = 0;

        for file_info in &metadata.files {
            if !file_info.backup_path.exists() {
                errors.push(format!("Backup file missing: {:?}", file_info.backup_path));
                continue;
            }

            match self.calculate_file_checksum(&file_info.backup_path) {
                Ok(checksum) => {
                    if checksum == file_info.checksum {
                        verified_files += 1;
                    } else {
                        errors.push(format!(
                            "Checksum mismatch for {:?}: expected {}, got {}",
                            file_info.backup_path, file_info.checksum, checksum
                        ));
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to verify {:?}: {}",
                        file_info.backup_path, e
                    ));
                }
            }
        }

        Ok(BackupVerificationResult {
            backup_id: backup_id.to_string(),
            valid: errors.is_empty(),
            errors,
            verified_files,
            total_files: metadata.files.len(),
        })
    }
}

/// Result of backup verification
#[derive(Debug, Clone)]
pub struct BackupVerificationResult {
    pub backup_id: String,
    pub valid: bool,
    pub errors: Vec<String>,
    pub verified_files: usize,
    pub total_files: usize,
}

// Add sha2 dependency
use sha2::Digest;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_rollback_backup() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RollbackManager::new(temp_dir.path().join("backups"));

        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        {
            fs::write(&test_file, "original content").unwrap();
        }

        // Small delay to ensure file is closed
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Create backup
        let backup_id = "test_backup";
        let metadata = manager
            .create_backup(
                backup_id,
                SimulatorType::MSFS,
                "1.36.0",
                "Test backup",
                &[test_file.clone()],
            )
            .await
            .unwrap();

        assert_eq!(metadata.files.len(), 1);
        assert_eq!(metadata.id, backup_id);

        // Modify the original file
        {
            fs::write(&test_file, "modified content").unwrap();
        }

        // Small delay to ensure file is closed
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Rollback
        let result = manager.rollback(backup_id).await.unwrap();
        assert!(result.success);
        assert_eq!(result.restored_files.len(), 1);

        // Verify content was restored
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "original content");
    }

    #[tokio::test]
    async fn test_list_backups() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RollbackManager::new(temp_dir.path().join("backups"));

        // Create multiple backups
        let test_file = temp_dir.path().join("test.txt");
        {
            fs::write(&test_file, "content").unwrap();
        }

        // Small delay to ensure file is closed
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        for i in 1..=3 {
            let backup_id = format!("backup_{}", i);
            manager
                .create_backup(
                    &backup_id,
                    SimulatorType::MSFS,
                    "1.36.0",
                    &format!("Backup {}", i),
                    &[test_file.clone()],
                )
                .await
                .unwrap();
            
            // Small delay between backups to ensure different timestamps
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        let backups = manager.list_backups().await.unwrap();
        assert_eq!(backups.len(), 3);
        
        // Should be sorted by timestamp (newest first)
        assert_eq!(backups[0].id, "backup_3");
        assert_eq!(backups[1].id, "backup_2");
        assert_eq!(backups[2].id, "backup_1");
    }

    #[tokio::test]
    async fn test_backup_verification() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RollbackManager::new(temp_dir.path().join("backups"));

        let test_file = temp_dir.path().join("test.txt");
        {
            fs::write(&test_file, "test content").unwrap();
        }

        // Small delay to ensure file is closed
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let backup_id = "verify_test";
        manager
            .create_backup(
                backup_id,
                SimulatorType::MSFS,
                "1.36.0",
                "Verification test",
                &[test_file],
            )
            .await
            .unwrap();

        let verification = manager.verify_backup(backup_id).await.unwrap();
        assert!(verification.valid);
        assert_eq!(verification.verified_files, 1);
        assert_eq!(verification.total_files, 1);
        assert!(verification.errors.is_empty());
    }
}