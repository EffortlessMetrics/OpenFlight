//! Diagnostic bundle generation for support and debugging.
//!
//! Generates a text archive containing logs, configuration, metrics snapshot,
//! and system information for troubleshooting.

use std::path::{Path, PathBuf};

/// Information about the system for diagnostic bundle.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub openflight_version: String,
    pub rust_version: String,
}

impl SystemInfo {
    pub fn collect() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            openflight_version: env!("CARGO_PKG_VERSION").to_string(),
            rust_version: "1.92+".to_string(),
        }
    }

    pub fn to_text(&self) -> String {
        format!(
            "OS: {}\nArch: {}\nOpenFlight: {}\nRust MSRV: {}\n",
            self.os, self.arch, self.openflight_version, self.rust_version
        )
    }
}

/// Configuration for diagnostic bundle generation.
#[derive(Debug, Clone)]
pub struct DiagnosticBundleConfig {
    /// Output path for the bundle (defaults to temp dir).
    pub output_path: Option<PathBuf>,
    /// Include config files in bundle.
    pub include_config: bool,
    /// Maximum log lines to include (0 = unlimited).
    pub max_log_lines: usize,
}

impl Default for DiagnosticBundleConfig {
    fn default() -> Self {
        Self {
            output_path: None,
            include_config: true,
            max_log_lines: 10000,
        }
    }
}

/// Diagnostic bundle entry (content to be included in the bundle).
#[derive(Debug, Clone)]
pub struct BundleEntry {
    /// Path within the bundle (e.g., "logs/service.log").
    pub name: String,
    /// Content of the entry.
    pub content: Vec<u8>,
}

impl BundleEntry {
    pub fn new(name: impl Into<String>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
        }
    }

    pub fn from_text(name: impl Into<String>, text: impl AsRef<str>) -> Self {
        Self::new(name, text.as_ref().as_bytes().to_vec())
    }
}

/// Generates diagnostic bundle entries.
pub struct DiagnosticBundleBuilder {
    config: DiagnosticBundleConfig,
    entries: Vec<BundleEntry>,
}

impl DiagnosticBundleBuilder {
    pub fn new(config: DiagnosticBundleConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
        }
    }

    /// Add system info entry.
    pub fn add_system_info(&mut self) -> &mut Self {
        let info = SystemInfo::collect();
        self.entries
            .push(BundleEntry::from_text("system_info.txt", info.to_text()));
        self
    }

    /// Add a text entry.
    pub fn add_text(&mut self, name: impl Into<String>, text: impl AsRef<str>) -> &mut Self {
        self.entries.push(BundleEntry::from_text(name, text));
        self
    }

    /// Add a file entry from disk.
    pub fn add_file(&mut self, name: impl Into<String>, path: &Path) -> &mut Self {
        if let Ok(content) = std::fs::read(path) {
            self.entries.push(BundleEntry::new(name, content));
        }
        self
    }

    /// Returns the default output path.
    pub fn default_output_path() -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("openflight_diag_{}.txt", ts))
    }

    /// Finalize the bundle entries as a flat text summary.
    pub fn finalize_as_text(&self) -> String {
        let mut out = String::new();
        out.push_str("=== OpenFlight Diagnostic Bundle ===\n\n");
        for entry in &self.entries {
            out.push_str(&format!("--- {} ---\n", entry.name));
            if let Ok(text) = std::str::from_utf8(&entry.content) {
                out.push_str(text);
            } else {
                out.push_str(&format!("[binary {} bytes]\n", entry.content.len()));
            }
            out.push('\n');
        }
        out
    }

    /// Write the bundle to the configured output path.
    pub fn write(&self) -> std::io::Result<PathBuf> {
        let path = self
            .config
            .output_path
            .clone()
            .unwrap_or_else(Self::default_output_path);
        let content = self.finalize_as_text();
        std::fs::write(&path, content)?;
        Ok(path)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn config(&self) -> &DiagnosticBundleConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_system_info_collect() {
        let info = SystemInfo::collect();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(!info.openflight_version.is_empty());
    }

    #[test]
    fn test_system_info_to_text() {
        let info = SystemInfo::collect();
        let text = info.to_text();
        assert!(text.contains("OS:"));
        assert!(text.contains("Arch:"));
    }

    #[test]
    fn test_builder_entry_count() {
        let mut builder = DiagnosticBundleBuilder::new(Default::default());
        assert_eq!(builder.entry_count(), 0);
        builder.add_text("test.txt", "hello");
        assert_eq!(builder.entry_count(), 1);
    }

    #[test]
    fn test_builder_add_system_info() {
        let mut builder = DiagnosticBundleBuilder::new(Default::default());
        builder.add_system_info();
        assert_eq!(builder.entry_count(), 1);
    }

    #[test]
    fn test_finalize_as_text_contains_header() {
        let mut builder = DiagnosticBundleBuilder::new(Default::default());
        builder.add_text("test.txt", "content");
        let text = builder.finalize_as_text();
        assert!(text.contains("OpenFlight Diagnostic Bundle"));
        assert!(text.contains("test.txt"));
        assert!(text.contains("content"));
    }

    #[test]
    fn test_write_creates_file() {
        let output = env::temp_dir().join("openflight_diag_test.txt");
        let _ = std::fs::remove_file(&output);
        let config = DiagnosticBundleConfig {
            output_path: Some(output.clone()),
            ..Default::default()
        };
        let mut builder = DiagnosticBundleBuilder::new(config);
        builder.add_system_info();
        let path = builder.write().expect("Should write bundle");
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_default_config_values() {
        let config = DiagnosticBundleConfig::default();
        assert!(config.include_config);
        assert_eq!(config.max_log_lines, 10000);
        assert!(config.output_path.is_none());
    }

    #[test]
    fn test_bundle_entry_from_text() {
        let entry = BundleEntry::from_text("foo.txt", "bar");
        assert_eq!(entry.name, "foo.txt");
        assert_eq!(entry.content, b"bar");
    }
}
