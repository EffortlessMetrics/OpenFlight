//! File diff operations for applying configuration changes

use crate::types::*;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Applies writer configurations to simulator files
pub struct WriterApplier {
    backup_dir: PathBuf,
}

impl WriterApplier {
    pub fn new<P: AsRef<Path>>(backup_dir: P) -> Self {
        Self {
            backup_dir: backup_dir.as_ref().to_path_buf(),
        }
    }

    /// Apply a complete writer configuration
    pub async fn apply(&self, config: &WriterConfig) -> Result<ApplyResult> {
        info!(
            "Applying writer config for {} version {}",
            config.sim, config.version
        );

        let backup_id = self.generate_backup_id();
        let mut modified_files = Vec::new();
        let mut errors = Vec::new();

        // Create backup directory for this operation
        let backup_path = self.backup_dir.join(&backup_id);
        fs::create_dir_all(&backup_path)
            .context("Failed to create backup directory")?;

        // Apply each diff
        for diff in &config.diffs {
            match self.apply_diff(diff, &backup_path).await {
                Ok(file_path) => {
                    modified_files.push(file_path);
                }
                Err(e) => {
                    errors.push(format!("Failed to apply diff to {:?}: {}", diff.file, e));
                }
            }
        }

        let success = errors.is_empty();
        
        if success {
            info!("Successfully applied {} diffs", modified_files.len());
        } else {
            warn!("Applied with {} errors", errors.len());
        }

        Ok(ApplyResult {
            success,
            modified_files,
            backup_id,
            errors,
        })
    }

    /// Apply a single file diff
    async fn apply_diff(&self, diff: &FileDiff, backup_path: &Path) -> Result<PathBuf> {
        debug!("Applying diff to {:?}", diff.file);

        // Create backup if requested
        if diff.backup && diff.file.exists() {
            self.create_backup(&diff.file, backup_path)
                .context("Failed to create backup")?;
        }

        // Apply the operation
        match &diff.operation {
            DiffOperation::Replace { content } => {
                self.replace_file(&diff.file, content).await
            }
            DiffOperation::IniSection { section, changes } => {
                self.modify_ini_section(&diff.file, section, changes).await
            }
            DiffOperation::JsonPatch { patches } => {
                self.apply_json_patches(&diff.file, patches).await
            }
            DiffOperation::LineReplace { pattern, replacement, regex } => {
                self.replace_lines(&diff.file, pattern, replacement, *regex).await
            }
        }?;

        Ok(diff.file.clone())
    }

    /// Replace entire file content
    async fn replace_file(&self, file_path: &Path, content: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create parent directory")?;
        }

        fs::write(file_path, content)
            .context("Failed to write file")?;

        debug!("Replaced file {:?}", file_path);
        Ok(())
    }

    /// Modify INI-style file section
    async fn modify_ini_section(
        &self,
        file_path: &Path,
        section: &str,
        changes: &HashMap<String, String>,
    ) -> Result<()> {
        let content = if file_path.exists() {
            fs::read_to_string(file_path)
                .context("Failed to read existing file")?
        } else {
            String::new()
        };

        let modified_content = self.apply_ini_changes(&content, section, changes)?;

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create parent directory")?;
        }

        fs::write(file_path, modified_content)
            .context("Failed to write modified file")?;

        debug!("Modified INI section [{}] in {:?}", section, file_path);
        Ok(())
    }

    /// Apply changes to INI-style content
    fn apply_ini_changes(
        &self,
        content: &str,
        target_section: &str,
        changes: &HashMap<String, String>,
    ) -> Result<String> {
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let mut in_target_section = false;
        let mut section_found = false;
        let mut applied_changes = HashMap::new();

        // Find and modify existing keys in the target section
        for line in &mut lines {
            let trimmed = line.trim();
            
            // Check for section headers
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let section_name = &trimmed[1..trimmed.len()-1];
                in_target_section = section_name == target_section;
                if in_target_section {
                    section_found = true;
                }
                continue;
            }

            // If we're in the target section and this is a key=value line
            if in_target_section && trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    if let Some(new_value) = changes.get(key) {
                        *line = format!("{}={}", key, new_value);
                        applied_changes.insert(key.to_string(), new_value.clone());
                    }
                }
            }
        }

        // If section wasn't found, add it
        if !section_found {
            lines.push(String::new());
            lines.push(format!("[{}]", target_section));
        }

        // Add any changes that weren't applied (new keys)
        for (key, value) in changes {
            if !applied_changes.contains_key(key) {
                // Find the end of the target section to add new keys
                let mut insert_index = lines.len();
                let mut in_section = false;
                
                for (i, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('[') && trimmed.ends_with(']') {
                        let section_name = &trimmed[1..trimmed.len()-1];
                        if section_name == target_section {
                            in_section = true;
                        } else if in_section {
                            insert_index = i;
                            break;
                        }
                    }
                }

                lines.insert(insert_index, format!("{}={}", key, value));
            }
        }

        Ok(lines.join("\n"))
    }

    /// Apply JSON patches to a file
    async fn apply_json_patches(&self, file_path: &Path, patches: &[JsonPatchOp]) -> Result<()> {
        let content = if file_path.exists() {
            fs::read_to_string(file_path)
                .context("Failed to read existing JSON file")?
        } else {
            "{}".to_string()
        };

        let mut json: Value = serde_json::from_str(&content)
            .context("Failed to parse JSON")?;

        // Apply each patch
        for patch in patches {
            self.apply_json_patch(&mut json, patch)
                .context("Failed to apply JSON patch")?;
        }

        let modified_content = serde_json::to_string_pretty(&json)
            .context("Failed to serialize JSON")?;

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create parent directory")?;
        }

        fs::write(file_path, modified_content)
            .context("Failed to write modified JSON file")?;

        debug!("Applied {} JSON patches to {:?}", patches.len(), file_path);
        Ok(())
    }

    /// Apply a single JSON patch operation
    fn apply_json_patch(&self, json: &mut Value, patch: &JsonPatchOp) -> Result<()> {
        match patch.op {
            JsonPatchOpType::Add => {
                if let Some(value) = &patch.value {
                    self.json_set_path(json, &patch.path, value.clone())?;
                }
            }
            JsonPatchOpType::Remove => {
                self.json_remove_path(json, &patch.path)?;
            }
            JsonPatchOpType::Replace => {
                if let Some(value) = &patch.value {
                    self.json_set_path(json, &patch.path, value.clone())?;
                }
            }
            JsonPatchOpType::Test => {
                if let Some(expected) = &patch.value {
                    let actual = self.json_get_path(json, &patch.path)?;
                    if actual != *expected {
                        anyhow::bail!("JSON patch test failed: expected {:?}, got {:?}", expected, actual);
                    }
                }
            }
            JsonPatchOpType::Move | JsonPatchOpType::Copy => {
                // These operations are more complex and less commonly used
                anyhow::bail!("JSON patch operations Move and Copy are not yet implemented");
            }
        }
        Ok(())
    }

    /// Set a value at a JSON path
    fn json_set_path(&self, json: &mut Value, path: &str, value: Value) -> Result<()> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let mut current = json;

        // Navigate to the parent of the target
        for part in &parts[..parts.len().saturating_sub(1)] {
            current = current.as_object_mut()
                .context("Expected object in JSON path")?
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }

        // Set the final value
        if let Some(key) = parts.last() {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(key.to_string(), value);
            }
        }

        Ok(())
    }

    /// Get a value at a JSON path
    fn json_get_path(&self, json: &Value, path: &str) -> Result<Value> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let mut current = json;

        for part in parts {
            if part.is_empty() {
                continue;
            }
            current = current.get(part)
                .context(format!("Path component '{}' not found", part))?;
        }

        Ok(current.clone())
    }

    /// Remove a value at a JSON path
    fn json_remove_path(&self, json: &mut Value, path: &str) -> Result<()> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        if parts.is_empty() {
            return Ok(());
        }

        let mut current = json;

        // Navigate to the parent
        for part in &parts[..parts.len().saturating_sub(1)] {
            current = current.get_mut(part)
                .context("Path not found in JSON")?;
        }

        // Remove the final key
        if let Some(key) = parts.last() {
            if let Some(obj) = current.as_object_mut() {
                obj.remove(*key);
            }
        }

        Ok(())
    }

    /// Replace lines in a text file
    async fn replace_lines(
        &self,
        file_path: &Path,
        pattern: &str,
        replacement: &str,
        use_regex: bool,
    ) -> Result<()> {
        let content = if file_path.exists() {
            fs::read_to_string(file_path)
                .context("Failed to read existing file")?
        } else {
            String::new()
        };

        let modified_content = if use_regex {
            let re = regex::Regex::new(pattern)
                .context("Invalid regex pattern")?;
            re.replace_all(&content, replacement).to_string()
        } else {
            content.replace(pattern, replacement)
        };

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create parent directory")?;
        }

        fs::write(file_path, modified_content)
            .context("Failed to write modified file")?;

        debug!("Replaced lines in {:?}", file_path);
        Ok(())
    }

    /// Create a backup of a file
    fn create_backup(&self, file_path: &Path, backup_path: &Path) -> Result<()> {
        let backup_file = backup_path.join(
            file_path.file_name()
                .context("Invalid file path")?
        );

        // Ensure backup directory exists
        if let Some(parent) = backup_file.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create backup directory")?;
        }

        fs::copy(file_path, &backup_file)
            .context("Failed to create backup")?;

        debug!("Created backup: {:?} -> {:?}", file_path, backup_file);
        Ok(())
    }

    /// Generate a unique backup identifier
    fn generate_backup_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        format!("backup_{}", timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_replace_file() {
        let temp_dir = TempDir::new().unwrap();
        let applier = WriterApplier::new(temp_dir.path());
        
        let file_path = temp_dir.path().join("test.txt");
        let content = "Hello, World!";
        
        applier.replace_file(&file_path, content).await.unwrap();
        
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, content);
    }

    #[tokio::test]
    async fn test_ini_section_modification() {
        let temp_dir = TempDir::new().unwrap();
        let applier = WriterApplier::new(temp_dir.path());
        
        let file_path = temp_dir.path().join("test.ini");
        let initial_content = "[Section1]\nkey1=value1\n\n[Section2]\nkey2=value2\n";
        fs::write(&file_path, initial_content).unwrap();
        
        let mut changes = HashMap::new();
        changes.insert("key1".to_string(), "new_value1".to_string());
        changes.insert("key3".to_string(), "value3".to_string());
        
        applier.modify_ini_section(&file_path, "Section1", &changes).await.unwrap();
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("key1=new_value1"));
        assert!(content.contains("key3=value3"));
        assert!(content.contains("[Section2]"));
    }

    #[tokio::test]
    async fn test_json_patch() {
        let temp_dir = TempDir::new().unwrap();
        let applier = WriterApplier::new(temp_dir.path());
        
        let file_path = temp_dir.path().join("test.json");
        let initial_json = r#"{"existing": "value"}"#;
        fs::write(&file_path, initial_json).unwrap();
        
        let patches = vec![
            JsonPatchOp {
                op: JsonPatchOpType::Add,
                path: "/new_key".to_string(),
                value: Some(serde_json::Value::String("new_value".to_string())),
                from: None,
            },
        ];
        
        applier.apply_json_patches(&file_path, &patches).await.unwrap();
        
        let content = fs::read_to_string(&file_path).unwrap();
        let json: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["new_key"], "new_value");
        assert_eq!(json["existing"], "value");
    }
}