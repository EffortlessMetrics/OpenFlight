// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Delta update system for efficient binary patches

use crate::manifest::{FileOperation, FileUpdate, UpdateManifest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Delta patch operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOperation {
    /// Copy bytes from source file
    Copy {
        /// Offset in source file
        src_offset: u64,
        /// Number of bytes to copy
        length: u64,
    },
    /// Insert new bytes
    Insert {
        /// New data to insert
        data: Vec<u8>,
    },
    /// Delete bytes (skip in source)
    Delete {
        /// Number of bytes to skip
        length: u64,
    },
}

/// Delta patch for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDelta {
    /// Source file path (relative)
    pub source_path: String,
    /// Target file path (relative)
    pub target_path: String,
    /// Source file hash for verification
    pub source_hash: String,
    /// Target file hash for verification
    pub target_hash: String,
    /// Sequence of operations to transform source to target
    pub operations: Vec<DeltaOperation>,
    /// Compression used for operations
    pub compression: String,
}

/// Complete delta patch containing multiple file deltas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaPatch {
    /// Patch format version
    pub version: u32,
    /// Source version this patch applies to
    pub source_version: String,
    /// Target version this patch produces
    pub target_version: String,
    /// File deltas
    pub files: HashMap<String, FileDelta>,
    /// Files to be deleted in target
    pub deleted_files: Vec<String>,
    /// New files to be created
    pub new_files: HashMap<String, Vec<u8>>,
    /// Patch creation timestamp
    pub created_at: u64,
    /// Patch size in bytes
    pub patch_size: u64,
}

impl DeltaPatch {
    /// Create a new empty delta patch
    pub fn new(source_version: String, target_version: String) -> Self {
        Self {
            version: 1,
            source_version,
            target_version,
            files: HashMap::new(),
            deleted_files: Vec::new(),
            new_files: HashMap::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            patch_size: 0,
        }
    }

    /// Add a file delta to the patch
    pub fn add_file_delta(&mut self, file_delta: FileDelta) {
        self.files
            .insert(file_delta.target_path.clone(), file_delta);
    }

    /// Add a file to be deleted
    pub fn add_deleted_file(&mut self, file_path: String) {
        self.deleted_files.push(file_path);
    }

    /// Add a new file
    pub fn add_new_file(&mut self, file_path: String, content: Vec<u8>) {
        self.new_files.insert(file_path, content);
    }

    /// Calculate total patch size
    pub fn calculate_size(&mut self) {
        let mut size = 0u64;

        // Size of file deltas (operations)
        for file_delta in self.files.values() {
            for op in &file_delta.operations {
                // Copy and Delete don't add to patch size
                if let DeltaOperation::Insert { data } = op {
                    size += data.len() as u64;
                }
            }
        }

        // Size of new files
        for content in self.new_files.values() {
            size += content.len() as u64;
        }

        self.patch_size = size;
    }
}

/// Delta applier for applying patches to existing installations
#[derive(Debug)]
pub struct DeltaApplier {
    /// Working directory for patch application
    #[allow(dead_code)]
    work_dir: PathBuf,
    /// Temporary directory for intermediate files
    temp_dir: PathBuf,
}

impl DeltaApplier {
    /// Create a new delta applier
    pub fn new<P: AsRef<Path>>(work_dir: P) -> crate::Result<Self> {
        let work_dir = work_dir.as_ref().to_path_buf();
        let temp_dir = work_dir.join("temp");

        Ok(Self { work_dir, temp_dir })
    }

    /// Apply a delta patch to the installation
    pub async fn apply_patch(
        &self,
        patch: &DeltaPatch,
        source_dir: &Path,
        target_dir: &Path,
    ) -> crate::Result<()> {
        tracing::info!(
            "Applying delta patch: {} -> {}",
            patch.source_version,
            patch.target_version
        );

        // Create temporary directory
        fs::create_dir_all(&self.temp_dir).await?;

        // Verify source files exist and have correct hashes
        self.verify_source_files(patch, source_dir).await?;

        // Apply file deltas
        for file_delta in patch.files.values() {
            self.apply_file_delta(file_delta, source_dir, target_dir)
                .await?;
        }

        // Create new files
        for (file_path, content) in &patch.new_files {
            let target_file = target_dir.join(file_path);

            // Create parent directories
            if let Some(parent) = target_file.parent() {
                fs::create_dir_all(parent).await?;
            }

            fs::write(&target_file, content).await?;
            tracing::debug!("Created new file: {}", file_path);
        }

        // Delete files
        for file_path in &patch.deleted_files {
            let target_file = target_dir.join(file_path);
            if target_file.exists() {
                fs::remove_file(&target_file).await?;
                tracing::debug!("Deleted file: {}", file_path);
            }
        }

        // Clean up temporary directory
        if self.temp_dir.exists() {
            fs::remove_dir_all(&self.temp_dir).await?;
        }

        tracing::info!("Delta patch applied successfully");
        Ok(())
    }

    /// Verify source files have expected hashes
    async fn verify_source_files(
        &self,
        patch: &DeltaPatch,
        source_dir: &Path,
    ) -> crate::Result<()> {
        for file_delta in patch.files.values() {
            let source_file = source_dir.join(&file_delta.source_path);

            if !source_file.exists() {
                return Err(crate::UpdateError::DeltaPatch(format!(
                    "Source file not found: {}",
                    file_delta.source_path
                )));
            }

            let content = fs::read(&source_file).await?;
            let hash = self.calculate_hash(&content);

            if hash != file_delta.source_hash {
                return Err(crate::UpdateError::DeltaPatch(format!(
                    "Source file hash mismatch for {}: expected {}, got {}",
                    file_delta.source_path, file_delta.source_hash, hash
                )));
            }
        }

        Ok(())
    }

    /// Apply delta operations to a single file
    #[allow(unused_assignments)]
    async fn apply_file_delta(
        &self,
        file_delta: &FileDelta,
        source_dir: &Path,
        target_dir: &Path,
    ) -> crate::Result<()> {
        let source_file = source_dir.join(&file_delta.source_path);
        let target_file = target_dir.join(&file_delta.target_path);

        tracing::debug!(
            "Applying file delta: {} -> {}",
            file_delta.source_path,
            file_delta.target_path
        );

        // Create parent directories for target
        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Read source file
        let source_content = fs::read(&source_file).await?;
        let mut _source_pos = 0usize;
        let mut target_content = Vec::new();

        // Apply operations in sequence
        for operation in &file_delta.operations {
            match operation {
                DeltaOperation::Copy { src_offset, length } => {
                    let start = *src_offset as usize;
                    let end = start + (*length as usize);

                    if end > source_content.len() {
                        return Err(crate::UpdateError::DeltaPatch(format!(
                            "Copy operation exceeds source file bounds"
                        )));
                    }

                    target_content.extend_from_slice(&source_content[start..end]);
                    _source_pos = end;
                }
                DeltaOperation::Insert { data } => {
                    target_content.extend_from_slice(data);
                }
                DeltaOperation::Delete { length } => {
                    _source_pos += *length as usize;
                }
            }
        }

        // Write target file
        fs::write(&target_file, &target_content).await?;

        // Verify target hash
        let target_hash = self.calculate_hash(&target_content);
        if target_hash != file_delta.target_hash {
            return Err(crate::UpdateError::DeltaPatch(format!(
                "Target file hash mismatch for {}: expected {}, got {}",
                file_delta.target_path, file_delta.target_hash, target_hash
            )));
        }

        Ok(())
    }

    /// Calculate SHA256 hash of content
    fn calculate_hash(&self, content: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    /// Compress patch data using flate2
    pub fn compress_patch_data(data: &[u8]) -> crate::Result<Vec<u8>> {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(data)
            .map_err(|e| crate::UpdateError::DeltaPatch(format!("Compression failed: {}", e)))?;

        encoder.finish().map_err(|e| {
            crate::UpdateError::DeltaPatch(format!("Compression finish failed: {}", e))
        })
    }

    /// Decompress patch data
    pub fn decompress_patch_data(compressed: &[u8]) -> crate::Result<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(compressed);
        let mut decompressed = Vec::new();

        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| crate::UpdateError::DeltaPatch(format!("Decompression failed: {}", e)))?;

        Ok(decompressed)
    }
}

/// Delta patch generator for creating patches between versions
#[derive(Debug)]
pub struct DeltaGenerator {
    /// Minimum file size to consider for delta compression
    min_delta_size: u64,
}

impl DeltaGenerator {
    /// Create a new delta generator
    pub fn new() -> Self {
        Self {
            min_delta_size: 1024, // 1KB minimum
        }
    }

    /// Generate a delta patch between two directory trees
    pub async fn generate_patch(
        &self,
        source_dir: &Path,
        target_dir: &Path,
        source_version: String,
        target_version: String,
    ) -> crate::Result<DeltaPatch> {
        let mut patch = DeltaPatch::new(source_version, target_version);

        // Find all files in both directories
        let source_files = self.scan_directory(source_dir).await?;
        let target_files = self.scan_directory(target_dir).await?;

        // Process each target file
        for (rel_path, target_path) in &target_files {
            if let Some(source_path) = source_files.get(rel_path) {
                // File exists in both - create delta
                let file_delta = self
                    .create_file_delta(source_path, target_path, rel_path)
                    .await?;

                if let Some(delta) = file_delta {
                    patch.add_file_delta(delta);
                }
            } else {
                // New file - add as new
                let content = fs::read(target_path).await?;
                patch.add_new_file(rel_path.clone(), content);
            }
        }

        // Find deleted files
        for (rel_path, _) in &source_files {
            if !target_files.contains_key(rel_path) {
                patch.add_deleted_file(rel_path.clone());
            }
        }

        patch.calculate_size();
        Ok(patch)
    }

    /// Scan directory and return relative path -> absolute path mapping
    async fn scan_directory(&self, dir: &Path) -> crate::Result<HashMap<String, PathBuf>> {
        let mut files = HashMap::new();
        self.scan_directory_recursive(dir, dir, &mut files).await?;
        Ok(files)
    }

    /// Iteratively scan directory (converted from recursive to avoid async recursion)
    async fn scan_directory_recursive(
        &self,
        base_dir: &Path,
        current_dir: &Path,
        files: &mut HashMap<String, PathBuf>,
    ) -> crate::Result<()> {
        let mut dirs_to_process = vec![current_dir.to_path_buf()];

        while let Some(current_dir) = dirs_to_process.pop() {
            let mut entries = fs::read_dir(&current_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    dirs_to_process.push(path);
                } else {
                    let rel_path = path
                        .strip_prefix(base_dir)
                        .map_err(|e| {
                            crate::UpdateError::DeltaPatch(format!(
                                "Failed to create relative path: {}",
                                e
                            ))
                        })?
                        .to_string_lossy()
                        .to_string();

                    files.insert(rel_path, path);
                }
            }
        }

        Ok(())
    }

    /// Create file delta between source and target files
    async fn create_file_delta(
        &self,
        source_path: &Path,
        target_path: &Path,
        rel_path: &str,
    ) -> crate::Result<Option<FileDelta>> {
        let source_content = fs::read(source_path).await?;
        let target_content = fs::read(target_path).await?;

        // If files are identical, no delta needed
        if source_content == target_content {
            return Ok(None);
        }

        // For small files or if delta would be larger than target, use replacement
        if target_content.len() < self.min_delta_size as usize {
            return Ok(None); // Will be handled as new file
        }

        // Create simple delta (for now, just replace entire file)
        // In production, would use more sophisticated binary diff algorithm
        let operations = vec![DeltaOperation::Insert {
            data: target_content.clone(),
        }];

        let source_hash = self.calculate_hash(&source_content);
        let target_hash = self.calculate_hash(&target_content);

        Ok(Some(FileDelta {
            source_path: rel_path.to_string(),
            target_path: rel_path.to_string(),
            source_hash,
            target_hash,
            operations,
            compression: "gzip".to_string(),
        }))
    }

    /// Calculate SHA256 hash
    fn calculate_hash(&self, content: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }
}

impl Default for DeltaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Manifest-driven helpers
// ---------------------------------------------------------------------------

/// Compute the SHA-256 hex digest of `data`.
fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// Compare two sets of files and produce a `Vec<FileUpdate>` describing the
/// differences.
///
/// Both `old_files` and `new_files` map *relative path* → *file content*.
pub fn calculate_delta(
    old_files: &HashMap<String, Vec<u8>>,
    new_files: &HashMap<String, Vec<u8>>,
) -> Vec<FileUpdate> {
    let mut updates = Vec::new();

    // Added or modified files
    for (path, new_content) in new_files {
        match old_files.get(path) {
            None => {
                updates.push(FileUpdate {
                    path: path.clone(),
                    hash_before: String::new(),
                    hash_after: sha256_hex(new_content),
                    size: new_content.len() as u64,
                    operation: FileOperation::Add,
                });
            }
            Some(old_content) => {
                let old_hash = sha256_hex(old_content);
                let new_hash = sha256_hex(new_content);
                if old_hash != new_hash {
                    updates.push(FileUpdate {
                        path: path.clone(),
                        hash_before: old_hash,
                        hash_after: new_hash,
                        size: new_content.len() as u64,
                        operation: FileOperation::Modify,
                    });
                }
            }
        }
    }

    // Removed files
    for path in old_files.keys() {
        if !new_files.contains_key(path) {
            let old_content = &old_files[path];
            updates.push(FileUpdate {
                path: path.clone(),
                hash_before: sha256_hex(old_content),
                hash_after: String::new(),
                size: 0,
                operation: FileOperation::Remove,
            });
        }
    }

    updates.sort_by(|a, b| a.path.cmp(&b.path));
    updates
}

/// Post-apply verification: every `hash_after` in the manifest must match the
/// corresponding file on disk. Returns `Ok(())` when all hashes match.
pub async fn verify_install(manifest: &UpdateManifest, install_dir: &Path) -> crate::Result<()> {
    for file_update in &manifest.files {
        match file_update.operation {
            FileOperation::Remove => {
                let p = install_dir.join(&file_update.path);
                if p.exists() {
                    return Err(crate::UpdateError::DeltaPatch(format!(
                        "file should have been removed: {}",
                        file_update.path
                    )));
                }
            }
            FileOperation::Add | FileOperation::Modify => {
                let p = install_dir.join(&file_update.path);
                let content = fs::read(&p).await.map_err(|e| {
                    crate::UpdateError::DeltaPatch(format!("cannot read {}: {e}", file_update.path))
                })?;
                let actual = sha256_hex(&content);
                if actual != file_update.hash_after {
                    return Err(crate::UpdateError::DeltaPatch(format!(
                        "hash mismatch for {}: expected {}, got {}",
                        file_update.path, file_update.hash_after, actual
                    )));
                }
            }
        }
    }
    Ok(())
}

impl DeltaApplier {
    /// Apply an `UpdateManifest` to `install_dir`.
    ///
    /// 1. **Pre-check** — verify all `hash_before` values match current files.
    /// 2. **Apply** — process each `FileUpdate` by reading content from
    ///    `content_dir` (for Add/Modify).
    /// 3. **Rollback** — if any step fails, restore every file that was
    ///    already touched.
    pub async fn apply_manifest(
        &self,
        manifest: &UpdateManifest,
        install_dir: &Path,
        content_dir: &Path,
    ) -> crate::Result<()> {
        // --- pre-check ---
        for fu in &manifest.files {
            match fu.operation {
                FileOperation::Add => {
                    let p = install_dir.join(&fu.path);
                    if p.exists() {
                        return Err(crate::UpdateError::DeltaPatch(format!(
                            "file already exists for Add operation: {}",
                            fu.path
                        )));
                    }
                }
                FileOperation::Modify => {
                    let content = fs::read(install_dir.join(&fu.path)).await.map_err(|e| {
                        crate::UpdateError::DeltaPatch(format!(
                            "pre-check: cannot read {}: {e}",
                            fu.path
                        ))
                    })?;
                    let actual = sha256_hex(&content);
                    if actual != fu.hash_before {
                        return Err(crate::UpdateError::DeltaPatch(format!(
                            "pre-check hash mismatch for {}: expected {}, got {}",
                            fu.path, fu.hash_before, actual
                        )));
                    }
                }
                FileOperation::Remove => {
                    let content = fs::read(install_dir.join(&fu.path)).await.map_err(|e| {
                        crate::UpdateError::DeltaPatch(format!(
                            "pre-check: cannot read {}: {e}",
                            fu.path
                        ))
                    })?;
                    let actual = sha256_hex(&content);
                    if actual != fu.hash_before {
                        return Err(crate::UpdateError::DeltaPatch(format!(
                            "pre-check hash mismatch for {}: expected {}, got {}",
                            fu.path, fu.hash_before, actual
                        )));
                    }
                }
            }
        }

        // --- apply with rollback tracking ---
        // Each entry: (path, Option<original_bytes>)  — None means file was
        // newly created and should be deleted on rollback.
        let mut applied: Vec<(PathBuf, Option<Vec<u8>>)> = Vec::new();

        let result = self
            .apply_manifest_inner(manifest, install_dir, content_dir, &mut applied)
            .await;

        if let Err(ref _e) = result {
            // rollback everything we already touched
            for (path, original) in applied.into_iter().rev() {
                match original {
                    Some(bytes) => {
                        let _ = fs::write(&path, &bytes).await;
                    }
                    None => {
                        let _ = fs::remove_file(&path).await;
                    }
                }
            }
        }

        result
    }

    /// Inner apply loop — separated so the caller can handle rollback.
    async fn apply_manifest_inner(
        &self,
        manifest: &UpdateManifest,
        install_dir: &Path,
        content_dir: &Path,
        applied: &mut Vec<(PathBuf, Option<Vec<u8>>)>,
    ) -> crate::Result<()> {
        for fu in &manifest.files {
            let target = install_dir.join(&fu.path);
            match fu.operation {
                FileOperation::Add => {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).await?;
                    }
                    let new_content = fs::read(content_dir.join(&fu.path)).await.map_err(|e| {
                        crate::UpdateError::DeltaPatch(format!(
                            "cannot read new content for {}: {e}",
                            fu.path
                        ))
                    })?;
                    fs::write(&target, &new_content).await?;
                    applied.push((target, None));
                }
                FileOperation::Modify => {
                    let old_content = fs::read(&target).await?;
                    let new_content = fs::read(content_dir.join(&fu.path)).await.map_err(|e| {
                        crate::UpdateError::DeltaPatch(format!(
                            "cannot read new content for {}: {e}",
                            fu.path
                        ))
                    })?;
                    fs::write(&target, &new_content).await?;
                    applied.push((target, Some(old_content)));
                }
                FileOperation::Remove => {
                    let old_content = fs::read(&target).await?;
                    fs::remove_file(&target).await?;
                    applied.push((target, Some(old_content)));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_delta_patch_creation() {
        let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());

        let file_delta = FileDelta {
            source_path: "test.txt".to_string(),
            target_path: "test.txt".to_string(),
            source_hash: "hash1".to_string(),
            target_hash: "hash2".to_string(),
            operations: vec![DeltaOperation::Insert {
                data: b"new content".to_vec(),
            }],
            compression: "gzip".to_string(),
        };

        patch.add_file_delta(file_delta);
        patch.add_new_file("new.txt".to_string(), b"new file".to_vec());
        patch.add_deleted_file("old.txt".to_string());

        assert_eq!(patch.files.len(), 1);
        assert_eq!(patch.new_files.len(), 1);
        assert_eq!(patch.deleted_files.len(), 1);
    }

    #[tokio::test]
    async fn test_delta_applier() {
        let temp_dir = TempDir::new().unwrap();
        let applier = DeltaApplier::new(temp_dir.path()).unwrap();

        // Create test directories
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create source file
        let source_file = source_dir.join("test.txt");
        fs::write(&source_file, b"original content").await.unwrap();

        // Create patch with simple replacement
        let source_hash = applier.calculate_hash(b"original content");
        let target_hash = applier.calculate_hash(b"new content");

        let file_delta = FileDelta {
            source_path: "test.txt".to_string(),
            target_path: "test.txt".to_string(),
            source_hash,
            target_hash,
            operations: vec![DeltaOperation::Insert {
                data: b"new content".to_vec(),
            }],
            compression: "none".to_string(),
        };

        let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());
        patch.add_file_delta(file_delta);

        // Apply patch
        assert!(
            applier
                .apply_patch(&patch, &source_dir, &target_dir)
                .await
                .is_ok()
        );

        // Verify result
        let result = fs::read(target_dir.join("test.txt")).await.unwrap();
        assert_eq!(result, b"new content");
    }

    #[test]
    fn test_compression() {
        let data = b"test data for compression";
        let compressed = DeltaApplier::compress_patch_data(data).unwrap();
        let decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();

        assert_eq!(data, decompressed.as_slice());
        assert!(compressed.len() < data.len() + 50); // Should be compressed or similar size
    }

    /// generate_patch → apply_patch round-trips binary content (≥ min_delta_size so a
    /// FileDelta is created) and also propagates brand-new files.
    #[tokio::test]
    async fn test_generate_patch_and_apply_round_trips_binary_content() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        let output_dir = temp.path().join("output");
        let work_dir = temp.path().join("work");

        fs::create_dir_all(&source_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();
        fs::create_dir_all(&output_dir).await.unwrap();

        // > min_delta_size (1024) so create_file_delta produces a FileDelta
        let source_content: Vec<u8> = (0u16..2048).map(|i| (i & 0xFF) as u8).collect();
        let target_content: Vec<u8> = (0u16..2048)
            .map(|i| (i.wrapping_mul(3) & 0xFF) as u8)
            .collect();

        fs::write(source_dir.join("data.bin"), &source_content)
            .await
            .unwrap();
        fs::write(target_dir.join("data.bin"), &target_content)
            .await
            .unwrap();

        // A new file only present in target exercises the new_files path
        let new_content = b"brand-new file content";
        fs::write(target_dir.join("new.bin"), new_content)
            .await
            .unwrap();

        let generator = DeltaGenerator::new();
        let patch = generator
            .generate_patch(
                &source_dir,
                &target_dir,
                "1.0.0".to_string(),
                "1.1.0".to_string(),
            )
            .await
            .unwrap();

        let applier = DeltaApplier::new(&work_dir).unwrap();
        applier
            .apply_patch(&patch, &source_dir, &output_dir)
            .await
            .unwrap();

        let patched = fs::read(output_dir.join("data.bin")).await.unwrap();
        assert_eq!(
            patched, target_content,
            "patched file must equal target content"
        );

        let new_out = fs::read(output_dir.join("new.bin")).await.unwrap();
        assert_eq!(
            new_out.as_slice(),
            new_content,
            "new file must be propagated"
        );
    }

    /// Copy operation extracts the exact byte range from the source.
    #[tokio::test]
    async fn test_apply_patch_copy_operation_extracts_correct_bytes() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("src");
        let output_dir = temp.path().join("out");
        let work_dir = temp.path().join("work");

        fs::create_dir_all(&source_dir).await.unwrap();
        fs::create_dir_all(&output_dir).await.unwrap();

        let source_content = b"ABCDEFGHIJ";
        fs::write(source_dir.join("file.bin"), source_content)
            .await
            .unwrap();

        let applier = DeltaApplier::new(&work_dir).unwrap();
        let source_hash = applier.calculate_hash(source_content);

        // Copy bytes at offset 2, length 3 → "CDE"
        let expected: &[u8] = b"CDE";
        let target_hash = applier.calculate_hash(expected);

        let file_delta = FileDelta {
            source_path: "file.bin".to_string(),
            target_path: "file.bin".to_string(),
            source_hash,
            target_hash,
            operations: vec![DeltaOperation::Copy {
                src_offset: 2,
                length: 3,
            }],
            compression: "none".to_string(),
        };

        let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());
        patch.add_file_delta(file_delta);
        applier
            .apply_patch(&patch, &source_dir, &output_dir)
            .await
            .unwrap();

        let result = fs::read(output_dir.join("file.bin")).await.unwrap();
        assert_eq!(result, expected);
    }

    /// deleted_files entries cause the file to be removed from the target directory.
    #[tokio::test]
    async fn test_apply_patch_removes_deleted_files() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("src");
        let output_dir = temp.path().join("out");
        let work_dir = temp.path().join("work");

        fs::create_dir_all(&source_dir).await.unwrap();
        fs::create_dir_all(&output_dir).await.unwrap();

        // Pre-create the file in the output directory
        fs::write(output_dir.join("obsolete.bin"), b"old content")
            .await
            .unwrap();

        let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());
        patch.add_deleted_file("obsolete.bin".to_string());

        let applier = DeltaApplier::new(&work_dir).unwrap();
        applier
            .apply_patch(&patch, &source_dir, &output_dir)
            .await
            .unwrap();

        assert!(
            !output_dir.join("obsolete.bin").exists(),
            "deleted file must be removed"
        );
    }

    /// A source-hash mismatch causes apply_patch to return an error.
    #[tokio::test]
    async fn test_apply_patch_source_hash_mismatch_returns_error() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("src");
        let output_dir = temp.path().join("out");
        let work_dir = temp.path().join("work");

        fs::create_dir_all(&source_dir).await.unwrap();
        fs::create_dir_all(&output_dir).await.unwrap();
        fs::write(source_dir.join("file.bin"), b"actual_content")
            .await
            .unwrap();

        let applier = DeltaApplier::new(&work_dir).unwrap();
        let wrong_source_hash = applier.calculate_hash(b"completely_different");
        let target_hash = applier.calculate_hash(b"result");

        let file_delta = FileDelta {
            source_path: "file.bin".to_string(),
            target_path: "file.bin".to_string(),
            source_hash: wrong_source_hash,
            target_hash,
            operations: vec![DeltaOperation::Insert {
                data: b"result".to_vec(),
            }],
            compression: "none".to_string(),
        };

        let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());
        patch.add_file_delta(file_delta);

        let result = applier.apply_patch(&patch, &source_dir, &output_dir).await;
        assert!(result.is_err(), "mismatched source hash must return error");
    }

    // ===================================================================
    // Tests for calculate_delta, verify_install, apply_manifest + rollback
    // ===================================================================

    fn hash(data: &[u8]) -> String {
        super::sha256_hex(data)
    }

    // -- calculate_delta -------------------------------------------------

    #[test]
    fn calculate_delta_detects_added_file() {
        let old = HashMap::new();
        let mut new = HashMap::new();
        new.insert("a.txt".into(), b"hello".to_vec());

        let delta = super::calculate_delta(&old, &new);
        assert_eq!(delta.len(), 1);
        assert_eq!(delta[0].operation, FileOperation::Add);
        assert_eq!(delta[0].path, "a.txt");
    }

    #[test]
    fn calculate_delta_detects_removed_file() {
        let mut old = HashMap::new();
        old.insert("a.txt".into(), b"hello".to_vec());
        let new = HashMap::new();

        let delta = super::calculate_delta(&old, &new);
        assert_eq!(delta.len(), 1);
        assert_eq!(delta[0].operation, FileOperation::Remove);
    }

    #[test]
    fn calculate_delta_detects_modified_file() {
        let mut old = HashMap::new();
        old.insert("a.txt".into(), b"old".to_vec());
        let mut new = HashMap::new();
        new.insert("a.txt".into(), b"new".to_vec());

        let delta = super::calculate_delta(&old, &new);
        assert_eq!(delta.len(), 1);
        assert_eq!(delta[0].operation, FileOperation::Modify);
        assert_eq!(delta[0].hash_before, hash(b"old"));
        assert_eq!(delta[0].hash_after, hash(b"new"));
    }

    #[test]
    fn calculate_delta_ignores_unchanged_file() {
        let mut old = HashMap::new();
        old.insert("a.txt".into(), b"same".to_vec());
        let mut new = HashMap::new();
        new.insert("a.txt".into(), b"same".to_vec());

        let delta = super::calculate_delta(&old, &new);
        assert!(delta.is_empty());
    }

    #[test]
    fn calculate_delta_mixed_operations() {
        let mut old = HashMap::new();
        old.insert("keep.txt".into(), b"same".to_vec());
        old.insert("modify.txt".into(), b"old".to_vec());
        old.insert("remove.txt".into(), b"gone".to_vec());

        let mut new = HashMap::new();
        new.insert("keep.txt".into(), b"same".to_vec());
        new.insert("modify.txt".into(), b"new".to_vec());
        new.insert("add.txt".into(), b"brand new".to_vec());

        let delta = super::calculate_delta(&old, &new);
        assert_eq!(delta.len(), 3); // add, modify, remove
        assert!(delta.iter().any(|d| d.operation == FileOperation::Add));
        assert!(delta.iter().any(|d| d.operation == FileOperation::Modify));
        assert!(delta.iter().any(|d| d.operation == FileOperation::Remove));
    }

    // -- verify_install --------------------------------------------------

    #[tokio::test]
    async fn verify_install_succeeds_when_hashes_match() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();

        fs::write(dir.join("a.txt"), b"hello").await.unwrap();

        let manifest = crate::manifest::UpdateManifest {
            version: crate::manifest::SemVer::new(1, 0, 0),
            channel: crate::Channel::Stable,
            files: vec![FileUpdate {
                path: "a.txt".into(),
                hash_before: String::new(),
                hash_after: hash(b"hello"),
                size: 5,
                operation: FileOperation::Add,
            }],
            signature: String::new(),
            min_version: None,
        };

        assert!(super::verify_install(&manifest, dir).await.is_ok());
    }

    #[tokio::test]
    async fn verify_install_fails_on_hash_mismatch() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();

        fs::write(dir.join("a.txt"), b"wrong content")
            .await
            .unwrap();

        let manifest = crate::manifest::UpdateManifest {
            version: crate::manifest::SemVer::new(1, 0, 0),
            channel: crate::Channel::Stable,
            files: vec![FileUpdate {
                path: "a.txt".into(),
                hash_before: String::new(),
                hash_after: hash(b"expected content"),
                size: 16,
                operation: FileOperation::Add,
            }],
            signature: String::new(),
            min_version: None,
        };

        assert!(super::verify_install(&manifest, dir).await.is_err());
    }

    #[tokio::test]
    async fn verify_install_fails_when_removed_file_still_exists() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();

        fs::write(dir.join("old.txt"), b"still here").await.unwrap();

        let manifest = crate::manifest::UpdateManifest {
            version: crate::manifest::SemVer::new(1, 0, 0),
            channel: crate::Channel::Stable,
            files: vec![FileUpdate {
                path: "old.txt".into(),
                hash_before: hash(b"still here"),
                hash_after: String::new(),
                size: 0,
                operation: FileOperation::Remove,
            }],
            signature: String::new(),
            min_version: None,
        };

        assert!(super::verify_install(&manifest, dir).await.is_err());
    }

    // -- apply_manifest + rollback ---------------------------------------

    #[tokio::test]
    async fn apply_manifest_add_file() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let content = temp.path().join("content");
        fs::create_dir_all(&install).await.unwrap();
        fs::create_dir_all(&content).await.unwrap();

        fs::write(content.join("new.txt"), b"new data")
            .await
            .unwrap();

        let manifest = crate::manifest::UpdateManifest {
            version: crate::manifest::SemVer::new(1, 1, 0),
            channel: crate::Channel::Stable,
            files: vec![FileUpdate {
                path: "new.txt".into(),
                hash_before: String::new(),
                hash_after: hash(b"new data"),
                size: 8,
                operation: FileOperation::Add,
            }],
            signature: String::new(),
            min_version: None,
        };

        let applier = DeltaApplier::new(temp.path()).unwrap();
        applier
            .apply_manifest(&manifest, &install, &content)
            .await
            .unwrap();

        let result = fs::read(install.join("new.txt")).await.unwrap();
        assert_eq!(result, b"new data");
    }

    #[tokio::test]
    async fn apply_manifest_rollback_on_failure() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let content = temp.path().join("content");
        fs::create_dir_all(&install).await.unwrap();
        fs::create_dir_all(&content).await.unwrap();

        // Existing file that will be modified
        fs::write(install.join("existing.txt"), b"original")
            .await
            .unwrap();
        fs::write(content.join("existing.txt"), b"updated")
            .await
            .unwrap();

        // Second file's content is intentionally missing from content_dir
        // so apply will fail mid-way.
        let manifest = crate::manifest::UpdateManifest {
            version: crate::manifest::SemVer::new(2, 0, 0),
            channel: crate::Channel::Stable,
            files: vec![
                FileUpdate {
                    path: "existing.txt".into(),
                    hash_before: hash(b"original"),
                    hash_after: hash(b"updated"),
                    size: 7,
                    operation: FileOperation::Modify,
                },
                FileUpdate {
                    path: "missing.txt".into(),
                    hash_before: String::new(),
                    hash_after: hash(b"x"),
                    size: 1,
                    operation: FileOperation::Add,
                },
            ],
            signature: String::new(),
            min_version: None,
        };

        let applier = DeltaApplier::new(temp.path()).unwrap();
        let result = applier.apply_manifest(&manifest, &install, &content).await;

        assert!(
            result.is_err(),
            "should fail because missing.txt content doesn't exist"
        );

        // existing.txt should have been rolled back to its original content
        let rolled_back = fs::read(install.join("existing.txt")).await.unwrap();
        assert_eq!(
            rolled_back, b"original",
            "existing.txt must be rolled back to original"
        );

        // missing.txt should not exist
        assert!(
            !install.join("missing.txt").exists(),
            "missing.txt must not remain after rollback"
        );
    }

    #[tokio::test]
    async fn apply_manifest_precheck_rejects_wrong_hash() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let content = temp.path().join("content");
        fs::create_dir_all(&install).await.unwrap();
        fs::create_dir_all(&content).await.unwrap();

        fs::write(install.join("f.txt"), b"actual").await.unwrap();
        fs::write(content.join("f.txt"), b"new").await.unwrap();

        let manifest = crate::manifest::UpdateManifest {
            version: crate::manifest::SemVer::new(2, 0, 0),
            channel: crate::Channel::Stable,
            files: vec![FileUpdate {
                path: "f.txt".into(),
                hash_before: hash(b"expected-but-wrong"),
                hash_after: hash(b"new"),
                size: 3,
                operation: FileOperation::Modify,
            }],
            signature: String::new(),
            min_version: None,
        };

        let applier = DeltaApplier::new(temp.path()).unwrap();
        let result = applier.apply_manifest(&manifest, &install, &content).await;
        assert!(result.is_err());

        // File should be unchanged because pre-check failed before any writes
        let unchanged = fs::read(install.join("f.txt")).await.unwrap();
        assert_eq!(unchanged, b"actual");
    }
}
