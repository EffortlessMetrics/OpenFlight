//! Delta update system for efficient binary patches

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        self.files.insert(file_delta.target_path.clone(), file_delta);
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
                match op {
                    DeltaOperation::Insert { data } => size += data.len() as u64,
                    _ => {} // Copy and Delete don't add to patch size
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
    work_dir: PathBuf,
    /// Temporary directory for intermediate files
    temp_dir: PathBuf,
}

impl DeltaApplier {
    /// Create a new delta applier
    pub fn new<P: AsRef<Path>>(work_dir: P) -> crate::Result<Self> {
        let work_dir = work_dir.as_ref().to_path_buf();
        let temp_dir = work_dir.join("temp");
        
        Ok(Self {
            work_dir,
            temp_dir,
        })
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
        for (target_path, file_delta) in &patch.files {
            self.apply_file_delta(file_delta, source_dir, target_dir).await?;
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
                return Err(crate::UpdateError::DeltaPatch(
                    format!("Source file not found: {}", file_delta.source_path)
                ));
            }
            
            let content = fs::read(&source_file).await?;
            let hash = self.calculate_hash(&content);
            
            if hash != file_delta.source_hash {
                return Err(crate::UpdateError::DeltaPatch(
                    format!(
                        "Source file hash mismatch for {}: expected {}, got {}",
                        file_delta.source_path,
                        file_delta.source_hash,
                        hash
                    )
                ));
            }
        }
        
        Ok(())
    }
    
    /// Apply delta operations to a single file
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
        let mut source_pos = 0usize;
        let mut target_content = Vec::new();
        
        // Apply operations in sequence
        for operation in &file_delta.operations {
            match operation {
                DeltaOperation::Copy { src_offset, length } => {
                    let start = *src_offset as usize;
                    let end = start + (*length as usize);
                    
                    if end > source_content.len() {
                        return Err(crate::UpdateError::DeltaPatch(
                            format!("Copy operation exceeds source file bounds")
                        ));
                    }
                    
                    target_content.extend_from_slice(&source_content[start..end]);
                    source_pos = end;
                }
                DeltaOperation::Insert { data } => {
                    target_content.extend_from_slice(data);
                }
                DeltaOperation::Delete { length } => {
                    source_pos += *length as usize;
                }
            }
        }
        
        // Write target file
        fs::write(&target_file, &target_content).await?;
        
        // Verify target hash
        let target_hash = self.calculate_hash(&target_content);
        if target_hash != file_delta.target_hash {
            return Err(crate::UpdateError::DeltaPatch(
                format!(
                    "Target file hash mismatch for {}: expected {}, got {}",
                    file_delta.target_path,
                    file_delta.target_hash,
                    target_hash
                )
            ));
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
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;
        
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)
            .map_err(|e| crate::UpdateError::DeltaPatch(format!("Compression failed: {}", e)))?;
        
        encoder.finish()
            .map_err(|e| crate::UpdateError::DeltaPatch(format!("Compression finish failed: {}", e)))
    }
    
    /// Decompress patch data
    pub fn decompress_patch_data(compressed: &[u8]) -> crate::Result<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;
        
        let mut decoder = GzDecoder::new(compressed);
        let mut decompressed = Vec::new();
        
        decoder.read_to_end(&mut decompressed)
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
                let file_delta = self.create_file_delta(
                    source_path,
                    target_path,
                    rel_path,
                ).await?;
                
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
    
    /// Recursively scan directory
    async fn scan_directory_recursive(
        &self,
        base_dir: &Path,
        current_dir: &Path,
        files: &mut HashMap<String, PathBuf>,
    ) -> crate::Result<()> {
        let mut entries = fs::read_dir(current_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() {
                self.scan_directory_recursive(base_dir, &path, files).await?;
            } else {
                let rel_path = path.strip_prefix(base_dir)
                    .map_err(|e| crate::UpdateError::DeltaPatch(
                        format!("Failed to create relative path: {}", e)
                    ))?
                    .to_string_lossy()
                    .to_string();
                
                files.insert(rel_path, path);
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
        assert!(applier.apply_patch(&patch, &source_dir, &target_dir).await.is_ok());
        
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
}