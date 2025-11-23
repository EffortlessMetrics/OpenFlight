// SPDX-License-Identifier: MIT OR Apache-2.0

//! Documentation normalization module.
//!
//! This module provides functionality to normalize documentation front matter
//! and generate documentation indexes. It verifies doc_id uniqueness and
//! creates a comprehensive index of all documentation grouped by kind and area.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::front_matter::{FrontMatter, collect_all_front_matter};

/// Verify that all doc_id values are unique across all documentation files.
///
/// # Arguments
///
/// * `docs` - Slice of tuples containing file paths and their front matter
///
/// # Returns
///
/// Returns `Ok(())` if all doc_ids are unique, or an error listing duplicates.
///
/// # Errors
///
/// Returns an error if duplicate doc_ids are found, including the paths of
/// all files that share the same doc_id.
pub fn verify_doc_id_uniqueness(docs: &[(PathBuf, FrontMatter)]) -> Result<()> {
    let mut doc_id_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

    // Build a map of doc_id -> list of file paths
    for (path, front_matter) in docs {
        doc_id_map
            .entry(front_matter.doc_id.clone())
            .or_default()
            .push(path.clone());
    }

    // Find duplicates
    let duplicates: Vec<_> = doc_id_map
        .iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();

    if !duplicates.is_empty() {
        let mut error_msg = String::from("Duplicate doc_id values found:\n");
        for (doc_id, paths) in duplicates {
            error_msg.push_str(&format!("\n  doc_id: {}\n", doc_id));
            for path in paths {
                error_msg.push_str(&format!("    - {}\n", path.display()));
            }
        }
        anyhow::bail!(error_msg);
    }

    Ok(())
}

/// Extract the title from a markdown file.
///
/// This function looks for the first H1 heading (# Title) in the markdown
/// content and returns it as the title.
///
/// # Arguments
///
/// * `content` - The full markdown file content
///
/// # Returns
///
/// Returns the title string if found, or "Untitled" if no H1 heading exists.
fn extract_title(content: &str) -> String {
    // Skip front matter if present
    let content_start = if content.starts_with("---") {
        // Find the end of front matter
        if let Some(end_pos) = content[3..].find("\n---") {
            end_pos + 7 // Skip past the closing "---\n"
        } else {
            0
        }
    } else {
        0
    };

    let content_after_front_matter = &content[content_start..];

    // Look for first H1 heading
    for line in content_after_front_matter.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return trimmed[2..].trim().to_string();
        }
    }

    "Untitled".to_string()
}

/// Generate a documentation index grouped by kind (band) and area.
///
/// # Arguments
///
/// * `docs` - Slice of tuples containing file paths and their front matter
///
/// # Returns
///
/// Returns a markdown string containing tables of documentation grouped by
/// kind and area, with columns for Doc ID, Title, Area, Status, and Links.
pub fn generate_docs_index(docs: &[(PathBuf, FrontMatter)]) -> Result<String> {
    let mut output = String::new();

    // Group docs by kind (band)
    let mut by_kind: HashMap<String, Vec<(PathBuf, FrontMatter, String)>> = HashMap::new();

    for (path, front_matter) in docs {
        // Read the file to extract title
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        let title = extract_title(&content);

        let kind_key = format!("{:?}", front_matter.kind).to_lowercase();
        by_kind
            .entry(kind_key)
            .or_default()
            .push((path.clone(), front_matter.clone(), title));
    }

    // Define the order of bands
    let band_order = vec![
        ("requirements", "Requirements"),
        ("design", "Design"),
        ("concept", "Concepts"),
        ("howto", "How-To Guides"),
        ("reference", "Reference"),
        ("adr", "Architecture Decision Records"),
    ];

    // Generate tables for each band
    for (band_key, band_title) in band_order {
        if let Some(docs_in_band) = by_kind.get(band_key) {
            output.push_str(&format!("## {}\n\n", band_title));

            // Group by area within this band
            let mut by_area: HashMap<String, Vec<&(PathBuf, FrontMatter, String)>> = HashMap::new();

            for doc in docs_in_band {
                let area_key = format!("{:?}", doc.1.area).to_lowercase();
                by_area.entry(area_key).or_default().push(doc);
            }

            // Sort areas alphabetically
            let mut areas: Vec<_> = by_area.keys().collect();
            areas.sort();

            for area_key in areas {
                let docs_in_area = by_area.get(area_key).unwrap();

                // Sort docs by doc_id within area
                let mut sorted_docs = docs_in_area.clone();
                sorted_docs.sort_by(|a, b| a.1.doc_id.cmp(&b.1.doc_id));

                // Generate table for this area
                output.push_str(&format!("### {}\n\n", capitalize_area(area_key)));
                output.push_str("| Doc ID | Title | Status | Requirements | Tasks | ADRs |\n");
                output.push_str("|--------|-------|--------|--------------|-------|------|\n");

                for (path, front_matter, title) in sorted_docs {
                    let req_links = if front_matter.links.requirements.is_empty() {
                        "-".to_string()
                    } else {
                        front_matter.links.requirements.join(", ")
                    };

                    let task_links = if front_matter.links.tasks.is_empty() {
                        "-".to_string()
                    } else {
                        front_matter.links.tasks.join(", ")
                    };

                    let adr_links = if front_matter.links.adrs.is_empty() {
                        "-".to_string()
                    } else {
                        front_matter.links.adrs.join(", ")
                    };

                    let status = format!("{:?}", front_matter.status).to_lowercase();

                    // Make title a link to the file
                    let relative_path = path.display().to_string();
                    let title_link = format!("[{}](../{})", title, relative_path);

                    output.push_str(&format!(
                        "| {} | {} | {} | {} | {} | {} |\n",
                        front_matter.doc_id, title_link, status, req_links, task_links, adr_links
                    ));
                }

                output.push('\n');
            }
        }
    }

    Ok(output)
}

/// Capitalize an area key for display.
fn capitalize_area(area: &str) -> String {
    match area {
        "flightcore" => "Flight Core".to_string(),
        "flightvirtual" => "Flight Virtual".to_string(),
        "flighthid" => "Flight HID".to_string(),
        "flightipc" => "Flight IPC".to_string(),
        "flightscheduler" => "Flight Scheduler".to_string(),
        "flightffb" => "Flight FFB".to_string(),
        "flightpanels" => "Flight Panels".to_string(),
        "infra" => "Infrastructure".to_string(),
        "ci" => "CI".to_string(),
        _ => area.to_string(),
    }
}

/// Run the normalize-docs command.
///
/// This function:
/// 1. Collects all front matter from docs/
/// 2. Verifies doc_id uniqueness
/// 3. Generates docs/README.md with an index of all documentation
///
/// # Returns
///
/// Returns `Ok(())` if normalization succeeds, or an error if any step fails.
pub fn run_normalize_docs() -> Result<()> {
    println!("Normalizing documentation...");

    let docs_dir = Path::new("docs");

    // Collect all front matter
    println!("  Collecting front matter from docs/...");
    let docs = collect_all_front_matter(docs_dir)
        .context("Failed to collect front matter from documentation")?;

    println!(
        "  Found {} documentation files with front matter",
        docs.len()
    );

    // Verify doc_id uniqueness
    println!("  Verifying doc_id uniqueness...");
    verify_doc_id_uniqueness(&docs).context("doc_id uniqueness check failed")?;
    println!("  ✓ All doc_ids are unique");

    // Generate docs index
    println!("  Generating documentation index...");
    let index_content = generate_docs_index(&docs).context("Failed to generate docs index")?;

    // Get git commit hash for header
    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string();

    // Get current timestamp
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Build full README content with header
    let mut readme_content = String::new();
    readme_content.push_str("<!--\n");
    readme_content.push_str("  AUTO-GENERATED FILE: DO NOT EDIT BY HAND.\n");
    readme_content.push_str("  Generated by: cargo xtask normalize-docs\n");
    readme_content.push_str(&format!("  Generated at: {}\n", timestamp));
    readme_content.push_str(&format!("  Git commit: {}\n", git_hash));
    readme_content.push_str("  Source of truth: docs/**/*.md front matter\n");
    readme_content.push_str("-->\n\n");
    readme_content.push_str("# Documentation Index\n\n");
    readme_content.push_str(
        "This index is automatically generated from the front matter of all documentation files.\n\n",
    );
    readme_content.push_str(&index_content);

    // Write docs/README.md
    let readme_path = docs_dir.join("README.md");
    std::fs::write(&readme_path, readme_content)
        .with_context(|| format!("Failed to write {}", readme_path.display()))?;

    println!("  ✓ Generated {}", readme_path.display());
    println!("\n✓ Documentation normalization complete");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front_matter::{Area, DocKind, DocStatus, Links};

    #[test]
    fn test_verify_doc_id_uniqueness_success() {
        let docs = vec![
            (
                PathBuf::from("doc1.md"),
                FrontMatter {
                    doc_id: "DOC-1".to_string(),
                    kind: DocKind::Concept,
                    area: Area::FlightCore,
                    status: DocStatus::Draft,
                    links: Links::default(),
                },
            ),
            (
                PathBuf::from("doc2.md"),
                FrontMatter {
                    doc_id: "DOC-2".to_string(),
                    kind: DocKind::Concept,
                    area: Area::FlightCore,
                    status: DocStatus::Draft,
                    links: Links::default(),
                },
            ),
        ];

        let result = verify_doc_id_uniqueness(&docs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_doc_id_uniqueness_failure() {
        let docs = vec![
            (
                PathBuf::from("doc1.md"),
                FrontMatter {
                    doc_id: "DOC-DUPLICATE".to_string(),
                    kind: DocKind::Concept,
                    area: Area::FlightCore,
                    status: DocStatus::Draft,
                    links: Links::default(),
                },
            ),
            (
                PathBuf::from("doc2.md"),
                FrontMatter {
                    doc_id: "DOC-DUPLICATE".to_string(),
                    kind: DocKind::HowTo,
                    area: Area::Ci,
                    status: DocStatus::Draft,
                    links: Links::default(),
                },
            ),
        ];

        let result = verify_doc_id_uniqueness(&docs);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("DOC-DUPLICATE"));
        assert!(error_msg.contains("doc1.md"));
        assert!(error_msg.contains("doc2.md"));
    }

    #[test]
    fn test_extract_title_with_h1() {
        let content = "---\ndoc_id: DOC-TEST\n---\n\n# My Title\n\nContent here.";
        let title = extract_title(content);
        assert_eq!(title, "My Title");
    }

    #[test]
    fn test_extract_title_no_h1() {
        let content = "---\ndoc_id: DOC-TEST\n---\n\nSome content without a title.";
        let title = extract_title(content);
        assert_eq!(title, "Untitled");
    }

    #[test]
    fn test_extract_title_no_front_matter() {
        let content = "# Direct Title\n\nContent.";
        let title = extract_title(content);
        assert_eq!(title, "Direct Title");
    }

    #[test]
    fn test_extract_title_with_extra_spaces() {
        let content = "#   Title with Spaces   \n\nContent.";
        let title = extract_title(content);
        assert_eq!(title, "Title with Spaces");
    }

    #[test]
    fn test_capitalize_area() {
        assert_eq!(capitalize_area("flightcore"), "Flight Core");
        assert_eq!(capitalize_area("flightvirtual"), "Flight Virtual");
        assert_eq!(capitalize_area("infra"), "Infrastructure");
        assert_eq!(capitalize_area("ci"), "CI");
    }

    #[test]
    fn test_generate_docs_index_empty() {
        let docs = vec![];
        let result = generate_docs_index(&docs);
        assert!(result.is_ok());
        let index = result.unwrap();
        // Should not contain any tables
        assert!(!index.contains("| Doc ID |"));
    }
}
