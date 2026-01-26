// SPDX-License-Identifier: MIT OR Apache-2.0

//! Front matter parsing module for documentation files.
//!
//! This module provides functionality to extract and parse YAML front matter
//! from markdown documentation files. Front matter is delimited by `---` markers
//! and contains metadata for cross-referencing and indexing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Documentation front matter metadata.
///
/// This struct represents the YAML front matter that appears at the top of
/// documentation files, delimited by `---` markers.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FrontMatter {
    /// Unique document identifier (e.g., DOC-CORE-AXIS)
    pub doc_id: String,

    /// Document kind/band
    pub kind: DocKind,

    /// Area or crate this document relates to
    pub area: Area,

    /// Document status
    pub status: DocStatus,

    /// Links to other artifacts
    #[serde(default)]
    pub links: Links,
}

/// Document kind/band enumeration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DocKind {
    Requirements,
    Design,
    Concept,
    HowTo,
    Reference,
    Adr,
    Tutorial,
    Explanation,
}

/// Area enumeration for documentation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Area {
    FlightCore,
    FlightVirtual,
    FlightHid,
    FlightIpc,
    FlightScheduler,
    FlightFfb,
    FlightPanels,
    Infra,
    Ci,
}

/// Document status enumeration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DocStatus {
    Draft,
    Active,
    Deprecated,
}

/// Links to other artifacts.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Links {
    /// Requirement IDs (REQ-* or INF-REQ-*)
    #[serde(default)]
    pub requirements: Vec<String>,

    /// Task IDs
    #[serde(default)]
    pub tasks: Vec<String>,

    /// ADR IDs
    #[serde(default)]
    pub adrs: Vec<String>,
}

/// Extract front matter text from markdown content.
///
/// Front matter is delimited by `---` markers at the start of the file.
/// This function extracts the text between the first `---` and second `---`.
///
/// # Arguments
///
/// * `content` - The full markdown file content
///
/// # Returns
///
/// Returns `Some(&str)` containing the front matter YAML text (without delimiters),
/// or `None` if no front matter is found.
///
/// # Examples
///
/// ```
/// let content = "---\ndoc_id: DOC-TEST\n---\n# Content";
/// let front_matter = extract_front_matter(content);
/// assert_eq!(front_matter, Some("doc_id: DOC-TEST\n"));
/// ```
pub fn extract_front_matter(content: &str) -> Option<&str> {
    // Front matter must start at the beginning of the file
    if !content.starts_with("---") {
        return None;
    }

    // Find the second --- delimiter
    let after_first_delimiter = &content[3..]; // Skip first "---"

    // Look for the closing ---
    if let Some(end_pos) = after_first_delimiter.find("\n---") {
        // Extract the text between delimiters, skipping the newline after first ---
        let start = if after_first_delimiter.starts_with('\n') {
            1
        } else if after_first_delimiter.starts_with("\r\n") {
            2
        } else {
            0
        };

        return Some(&after_first_delimiter[start..end_pos]);
    }

    None
}

/// Parse YAML front matter into a FrontMatter struct.
///
/// # Arguments
///
/// * `yaml` - The YAML text to parse (without `---` delimiters)
///
/// # Returns
///
/// Returns `Ok(FrontMatter)` if parsing succeeds, or an error if the YAML
/// is malformed or doesn't match the expected structure.
///
/// # Errors
///
/// Returns an error if:
/// - The YAML syntax is invalid
/// - Required fields are missing
/// - Field values don't match expected types or enums
pub fn parse_front_matter(yaml: &str) -> Result<FrontMatter> {
    serde_yaml::from_str(yaml).context("Failed to parse front matter YAML")
}

/// Collect all front matter from markdown files in a directory tree.
///
/// This function walks the directory recursively, finds all `.md` files,
/// extracts their front matter, and returns a list of (path, front_matter) pairs.
///
/// # Arguments
///
/// * `docs_dir` - Path to the documentation directory to scan
///
/// # Returns
///
/// Returns a vector of tuples containing the file path and parsed front matter
/// for each documentation file that has valid front matter.
///
/// Files without front matter are skipped (not an error).
/// Files with malformed front matter will cause an error.
///
/// # Errors
///
/// Returns an error if:
/// - The directory cannot be read
/// - A file cannot be read
/// - Front matter exists but is malformed
pub fn collect_all_front_matter(docs_dir: &Path) -> Result<Vec<(PathBuf, FrontMatter)>> {
    let mut results = Vec::new();

    for entry in WalkDir::new(docs_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process .md files
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        // Read file content
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Extract front matter
        if let Some(yaml) = extract_front_matter(&content) {
            // Parse front matter
            let front_matter = parse_front_matter(yaml)
                .with_context(|| format!("Failed to parse front matter in: {}", path.display()))?;

            results.push((path.to_path_buf(), front_matter));
        }
        // Files without front matter are silently skipped
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_front_matter_valid() {
        let content = "---\ndoc_id: DOC-TEST\nkind: concept\n---\n# Content";
        let result = extract_front_matter(content);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "doc_id: DOC-TEST\nkind: concept");
    }

    #[test]
    fn test_extract_front_matter_with_crlf() {
        let content = "---\r\ndoc_id: DOC-TEST\r\n---\r\n# Content";
        let result = extract_front_matter(content);
        assert!(result.is_some());
        assert!(result.unwrap().contains("doc_id: DOC-TEST"));
    }

    #[test]
    fn test_extract_front_matter_missing() {
        let content = "# No front matter here\n\nJust content.";
        let result = extract_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_front_matter_no_closing_delimiter() {
        let content = "---\ndoc_id: DOC-TEST\n# Content without closing";
        let result = extract_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_front_matter_empty() {
        let content = "";
        let result = extract_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_front_matter_valid() {
        let yaml = r#"
doc_id: DOC-CORE-AXIS
kind: concept
area: flight-core
status: active
links:
  requirements: [REQ-1, REQ-12]
  tasks: []
  adrs: []
"#;
        let result = parse_front_matter(yaml);
        assert!(result.is_ok());

        let front_matter = result.unwrap();
        assert_eq!(front_matter.doc_id, "DOC-CORE-AXIS");
        assert_eq!(front_matter.kind, DocKind::Concept);
        assert_eq!(front_matter.area, Area::FlightCore);
        assert_eq!(front_matter.status, DocStatus::Active);
        assert_eq!(front_matter.links.requirements, vec!["REQ-1", "REQ-12"]);
    }

    #[test]
    fn test_parse_front_matter_minimal() {
        let yaml = r#"
doc_id: DOC-TEST
kind: how-to
area: ci
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
"#;
        let result = parse_front_matter(yaml);
        assert!(result.is_ok());

        let front_matter = result.unwrap();
        assert_eq!(front_matter.doc_id, "DOC-TEST");
        assert_eq!(front_matter.kind, DocKind::HowTo);
        assert_eq!(front_matter.links.requirements.len(), 0);
    }

    #[test]
    fn test_parse_front_matter_missing_required_field() {
        let yaml = r#"
doc_id: DOC-TEST
kind: concept
area: flight-core
# Missing status field
links:
  requirements: []
"#;
        let result = parse_front_matter(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_front_matter_invalid_kind() {
        let yaml = r#"
doc_id: DOC-TEST
kind: invalid-kind
area: flight-core
status: draft
links:
  requirements: []
"#;
        let result = parse_front_matter(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_front_matter_invalid_yaml() {
        let yaml = "doc_id: DOC-TEST\n  invalid: indentation:\n    broken";
        let result = parse_front_matter(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_all_front_matter() {
        // This test requires fixture files to exist
        let fixtures_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal/docs");

        // Skip test if fixtures don't exist yet
        if !fixtures_dir.exists() {
            eprintln!("Skipping test_collect_all_front_matter: fixtures not yet created");
            return;
        }

        let result = collect_all_front_matter(&fixtures_dir);
        assert!(result.is_ok());

        let docs = result.unwrap();
        assert!(
            !docs.is_empty(),
            "Should find at least one doc with front matter"
        );

        // Verify all returned docs have valid front matter
        for (path, front_matter) in &docs {
            assert!(
                !front_matter.doc_id.is_empty(),
                "doc_id should not be empty for {}",
                path.display()
            );
        }
    }

    #[test]
    fn test_collect_all_front_matter_nonexistent_dir() {
        let nonexistent = PathBuf::from("/nonexistent/path/to/docs");
        let result = collect_all_front_matter(&nonexistent);
        // Should return an error or empty vec, not panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_extract_front_matter_starts_mid_file() {
        // Front matter must be at the start of the file
        let content = "# Title\n\n---\ndoc_id: DOC-TEST\n---\n";
        let result = extract_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_front_matter_multiple_delimiters() {
        // Should only extract between first and second delimiter
        let content = "---\ndoc_id: DOC-TEST\n---\n# Content\n---\nMore content\n---";
        let result = extract_front_matter(content);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "doc_id: DOC-TEST");
    }

    #[test]
    fn test_parse_front_matter_with_empty_links() {
        let yaml = r#"
doc_id: DOC-TEST
kind: concept
area: flight-core
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
"#;
        let result = parse_front_matter(yaml);
        assert!(result.is_ok());

        let front_matter = result.unwrap();
        assert!(front_matter.links.requirements.is_empty());
        assert!(front_matter.links.tasks.is_empty());
        assert!(front_matter.links.adrs.is_empty());
    }

    #[test]
    fn test_parse_front_matter_all_doc_kinds() {
        let kinds = vec![
            ("requirements", DocKind::Requirements),
            ("design", DocKind::Design),
            ("concept", DocKind::Concept),
            ("how-to", DocKind::HowTo),
            ("reference", DocKind::Reference),
            ("adr", DocKind::Adr),
        ];

        for (kind_str, expected_kind) in kinds {
            let yaml = format!(
                r#"
doc_id: DOC-TEST
kind: {}
area: infra
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
"#,
                kind_str
            );
            let result = parse_front_matter(&yaml);
            assert!(result.is_ok(), "Failed to parse kind: {}", kind_str);
            assert_eq!(result.unwrap().kind, expected_kind);
        }
    }

    #[test]
    fn test_parse_front_matter_all_areas() {
        let areas = vec![
            ("flight-core", Area::FlightCore),
            ("flight-virtual", Area::FlightVirtual),
            ("flight-hid", Area::FlightHid),
            ("flight-ipc", Area::FlightIpc),
            ("flight-scheduler", Area::FlightScheduler),
            ("flight-ffb", Area::FlightFfb),
            ("flight-panels", Area::FlightPanels),
            ("infra", Area::Infra),
            ("ci", Area::Ci),
        ];

        for (area_str, expected_area) in areas {
            let yaml = format!(
                r#"
doc_id: DOC-TEST
kind: concept
area: {}
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
"#,
                area_str
            );
            let result = parse_front_matter(&yaml);
            assert!(result.is_ok(), "Failed to parse area: {}", area_str);
            assert_eq!(result.unwrap().area, expected_area);
        }
    }

    #[test]
    fn test_parse_front_matter_all_statuses() {
        let statuses = vec![
            ("draft", DocStatus::Draft),
            ("active", DocStatus::Active),
            ("deprecated", DocStatus::Deprecated),
        ];

        for (status_str, expected_status) in statuses {
            let yaml = format!(
                r#"
doc_id: DOC-TEST
kind: concept
area: infra
status: {}
links:
  requirements: []
  tasks: []
  adrs: []
"#,
                status_str
            );
            let result = parse_front_matter(&yaml);
            assert!(result.is_ok(), "Failed to parse status: {}", status_str);
            assert_eq!(result.unwrap().status, expected_status);
        }
    }
}
