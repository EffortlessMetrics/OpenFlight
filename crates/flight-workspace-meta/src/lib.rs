// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Workspace microcrate discovery and crates.io metadata validation.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const REQUIRED_CRATES_IO_METADATA_FIELDS: [&str; 11] = [
    "name",
    "version",
    "edition",
    "rust-version",
    "license",
    "repository",
    "homepage",
    "description",
    "readme",
    "keywords",
    "categories",
];

#[derive(Debug, Clone)]
pub struct WorkspaceCrateMetadata {
    pub name: String,
    pub manifest_path: PathBuf,
    pub metadata: CratesIoMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct CratesIoMetadata {
    pub version: Option<String>,
    pub edition: Option<String>,
    pub rust_version: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

impl CratesIoMetadata {
    pub fn readme_path(&self, manifest_path: &Path) -> Option<PathBuf> {
        let readme = self.readme.as_ref()?;
        let readme_path = Path::new(readme);
        if readme_path.is_absolute() {
            return Some(readme_path.to_path_buf());
        }

        manifest_path
            .parent()
            .map(|manifest_dir| manifest_dir.join(readme_path))
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetadataValidationReport {
    pub checked: usize,
    pub issues: Vec<CrateMetadataIssue>,
}

impl MetadataValidationReport {
    pub fn is_success(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct CrateMetadataIssue {
    pub crate_name: String,
    pub missing_fields: Vec<String>,
    pub invalid_fields: Vec<String>,
}

impl CrateMetadataIssue {
    pub fn summary(&self) -> String {
        let mut reasons = Vec::new();
        if !self.missing_fields.is_empty() {
            reasons.push(format!("missing: {}", self.missing_fields.join(", ")));
        }
        if !self.invalid_fields.is_empty() {
            reasons.push(format!("invalid: {}", self.invalid_fields.join(", ")));
        }
        format!("{} ({})", self.crate_name, reasons.join("; "))
    }
}

pub fn load_workspace_microcrate_names(
    workspace_root: impl AsRef<Path>,
) -> Result<BTreeSet<String>> {
    Ok(load_workspace_microcrates(workspace_root)?
        .into_iter()
        .map(|crate_metadata| crate_metadata.name)
        .collect())
}

pub fn load_workspace_microcrates(
    workspace_root: impl AsRef<Path>,
) -> Result<Vec<WorkspaceCrateMetadata>> {
    let workspace_root = find_workspace_root(workspace_root.as_ref())?;
    let workspace_manifest_path = workspace_root.join("Cargo.toml");
    let workspace_manifest: WorkspaceManifest = parse_toml_file(&workspace_manifest_path)
        .with_context(|| {
            format!(
                "failed to parse workspace manifest at {}",
                workspace_manifest_path.display()
            )
        })?;

    let workspace = workspace_manifest
        .workspace
        .context("workspace manifest does not define [workspace]")?;
    let workspace_defaults = workspace.package.unwrap_or_default();

    let mut members = Vec::new();
    for member in workspace.members {
        if !is_microcrate_member(&member) {
            continue;
        }

        let manifest_path = workspace_root.join(&member).join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }

        let manifest: CrateManifest = parse_toml_file(&manifest_path).with_context(|| {
            format!(
                "failed to parse crate manifest at {}",
                manifest_path.display()
            )
        })?;
        let package = manifest
            .package
            .with_context(|| format!("missing [package] section in {}", manifest_path.display()))?;

        let default_name = Path::new(&member)
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string())
            .unwrap_or(member.clone());
        let name = package.name.clone().unwrap_or(default_name);

        let metadata = CratesIoMetadata {
            version: resolve_string_field(&package.version, workspace_defaults.version.as_ref()),
            edition: resolve_string_field(&package.edition, workspace_defaults.edition.as_ref()),
            rust_version: resolve_string_field(
                &package.rust_version,
                workspace_defaults.rust_version.as_ref(),
            ),
            license: resolve_string_field(&package.license, workspace_defaults.license.as_ref()),
            repository: resolve_string_field(
                &package.repository,
                workspace_defaults.repository.as_ref(),
            ),
            homepage: resolve_string_field(&package.homepage, workspace_defaults.homepage.as_ref()),
            description: resolve_string_field(
                &package.description,
                workspace_defaults.description.as_ref(),
            ),
            readme: resolve_string_field(&package.readme, workspace_defaults.readme.as_ref()),
            keywords: resolve_vec_field(&package.keywords, workspace_defaults.keywords.as_ref()),
            categories: resolve_vec_field(
                &package.categories,
                workspace_defaults.categories.as_ref(),
            ),
        };

        members.push(WorkspaceCrateMetadata {
            name,
            manifest_path,
            metadata,
        });
    }

    members.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(members)
}

pub fn validate_workspace_crates_io_metadata(
    workspace_root: impl AsRef<Path>,
) -> Result<MetadataValidationReport> {
    let crates = load_workspace_microcrates(workspace_root)?;
    let mut report = MetadataValidationReport {
        checked: crates.len(),
        issues: Vec::new(),
    };

    for crate_metadata in crates {
        let mut issue = CrateMetadataIssue {
            crate_name: crate_metadata.name.clone(),
            missing_fields: Vec::new(),
            invalid_fields: Vec::new(),
        };

        if crate_metadata.name.trim().is_empty() {
            issue.missing_fields.push("name".to_string());
        }

        ensure_non_empty_string_field(
            "version",
            crate_metadata.metadata.version.as_deref(),
            &mut issue.missing_fields,
        );
        ensure_non_empty_string_field(
            "edition",
            crate_metadata.metadata.edition.as_deref(),
            &mut issue.missing_fields,
        );
        ensure_non_empty_string_field(
            "rust-version",
            crate_metadata.metadata.rust_version.as_deref(),
            &mut issue.missing_fields,
        );
        ensure_non_empty_string_field(
            "license",
            crate_metadata.metadata.license.as_deref(),
            &mut issue.missing_fields,
        );
        ensure_non_empty_string_field(
            "repository",
            crate_metadata.metadata.repository.as_deref(),
            &mut issue.missing_fields,
        );
        ensure_non_empty_string_field(
            "homepage",
            crate_metadata.metadata.homepage.as_deref(),
            &mut issue.missing_fields,
        );
        ensure_non_empty_string_field(
            "description",
            crate_metadata.metadata.description.as_deref(),
            &mut issue.missing_fields,
        );

        if crate_metadata
            .metadata
            .readme
            .as_deref()
            .is_none_or(str::is_empty)
        {
            issue.missing_fields.push("readme".to_string());
        } else if let Some(readme_path) = crate_metadata
            .metadata
            .readme_path(&crate_metadata.manifest_path)
            && !readme_path.exists()
        {
            issue.invalid_fields.push(format!(
                "readme file does not exist ({})",
                readme_path.display()
            ));
        }

        if crate_metadata.metadata.keywords.is_empty() {
            issue.missing_fields.push("keywords".to_string());
        }
        if crate_metadata.metadata.categories.is_empty() {
            issue.missing_fields.push("categories".to_string());
        }

        if !issue.missing_fields.is_empty() || !issue.invalid_fields.is_empty() {
            report.issues.push(issue);
        }
    }

    Ok(report)
}

fn ensure_non_empty_string_field(
    field_name: &str,
    value: Option<&str>,
    missing_fields: &mut Vec<String>,
) {
    if value.is_none_or(str::is_empty) {
        missing_fields.push(field_name.to_string());
    }
}

fn resolve_string_field(
    field: &Option<WorkspaceStringField>,
    workspace_default: Option<&String>,
) -> Option<String> {
    match field {
        Some(WorkspaceStringField::Value(value)) => Some(value.trim().to_string()),
        Some(WorkspaceStringField::Inherit(inherit)) if inherit.workspace => {
            workspace_default.cloned()
        }
        _ => None,
    }
}

fn resolve_vec_field(
    field: &Option<WorkspaceVecField>,
    workspace_default: Option<&Vec<String>>,
) -> Vec<String> {
    let values = match field {
        Some(WorkspaceVecField::Value(values)) => values.clone(),
        Some(WorkspaceVecField::Inherit(inherit)) if inherit.workspace => {
            workspace_default.cloned().unwrap_or_default()
        }
        _ => Vec::new(),
    };

    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn find_workspace_root(start: &Path) -> Result<PathBuf> {
    let mut current = if start.is_file() {
        start
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let workspace_manifest = current.join("Cargo.toml");
        if workspace_manifest.exists() {
            let content = fs::read_to_string(&workspace_manifest)
                .with_context(|| format!("failed to read {}", workspace_manifest.display()))?;
            if content.contains("[workspace]") {
                return Ok(current);
            }
        }

        if !current.pop() {
            anyhow::bail!("could not find workspace root from {}", start.display());
        }
    }
}

fn is_microcrate_member(member_path: &str) -> bool {
    Path::new(member_path)
        .components()
        .next()
        .is_some_and(|component| {
            matches!(component, Component::Normal(name) if name.to_string_lossy() == "crates")
        })
}

fn parse_toml_file<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

#[derive(Debug, Deserialize, Default)]
struct WorkspaceManifest {
    workspace: Option<WorkspaceSection>,
}

#[derive(Debug, Deserialize, Default)]
struct WorkspaceSection {
    #[serde(default)]
    members: Vec<String>,
    package: Option<WorkspacePackage>,
}

#[derive(Debug, Deserialize, Default)]
struct CrateManifest {
    package: Option<CratePackage>,
}

#[derive(Debug, Deserialize, Default)]
struct CratePackage {
    name: Option<String>,
    version: Option<WorkspaceStringField>,
    edition: Option<WorkspaceStringField>,
    #[serde(rename = "rust-version")]
    rust_version: Option<WorkspaceStringField>,
    license: Option<WorkspaceStringField>,
    repository: Option<WorkspaceStringField>,
    homepage: Option<WorkspaceStringField>,
    description: Option<WorkspaceStringField>,
    readme: Option<WorkspaceStringField>,
    keywords: Option<WorkspaceVecField>,
    categories: Option<WorkspaceVecField>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct WorkspacePackage {
    version: Option<String>,
    edition: Option<String>,
    #[serde(rename = "rust-version")]
    rust_version: Option<String>,
    license: Option<String>,
    repository: Option<String>,
    homepage: Option<String>,
    description: Option<String>,
    readme: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum WorkspaceStringField {
    Value(String),
    Inherit(WorkspaceInheritance),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum WorkspaceVecField {
    Value(Vec<String>),
    Inherit(WorkspaceInheritance),
}

#[derive(Debug, Deserialize, Clone)]
struct WorkspaceInheritance {
    workspace: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn metadata_validation_report_starts_successful() {
        let report = MetadataValidationReport::default();
        assert!(report.is_success());
        assert_eq!(report.checked, 0);
    }

    #[test]
    fn metadata_validation_report_with_issues_fails() {
        let mut report = MetadataValidationReport {
            checked: 1,
            issues: vec![],
        };
        report.issues.push(CrateMetadataIssue {
            crate_name: "test-crate".to_string(),
            missing_fields: vec!["version".to_string()],
            invalid_fields: vec![],
        });
        assert!(!report.is_success());
    }

    #[test]
    fn crate_metadata_issue_summary_missing_fields() {
        let issue = CrateMetadataIssue {
            crate_name: "my-crate".to_string(),
            missing_fields: vec!["version".to_string(), "license".to_string()],
            invalid_fields: vec![],
        };
        let summary = issue.summary();
        assert!(summary.contains("my-crate"));
        assert!(summary.contains("missing"));
        assert!(summary.contains("version"));
    }

    #[test]
    fn crate_metadata_issue_summary_invalid_fields() {
        let issue = CrateMetadataIssue {
            crate_name: "bad-crate".to_string(),
            missing_fields: vec![],
            invalid_fields: vec!["readme file does not exist".to_string()],
        };
        let summary = issue.summary();
        assert!(summary.contains("invalid"));
    }

    #[test]
    fn readme_path_absolute_returned_unchanged() {
        let meta = CratesIoMetadata {
            readme: Some("/absolute/README.md".to_string()),
            ..Default::default()
        };
        let manifest = PathBuf::from("/some/crate/Cargo.toml");
        let result = meta.readme_path(&manifest);
        assert_eq!(result, Some(PathBuf::from("/absolute/README.md")));
    }

    #[test]
    fn readme_path_relative_joined_to_manifest_dir() {
        let meta = CratesIoMetadata {
            readme: Some("README.md".to_string()),
            ..Default::default()
        };
        let manifest = PathBuf::from("/crates/my-crate/Cargo.toml");
        let result = meta.readme_path(&manifest);
        assert_eq!(result, Some(PathBuf::from("/crates/my-crate/README.md")));
    }

    #[test]
    fn readme_path_none_when_no_readme() {
        let meta = CratesIoMetadata::default();
        let manifest = PathBuf::from("/crates/my-crate/Cargo.toml");
        assert!(meta.readme_path(&manifest).is_none());
    }

    #[test]
    fn required_metadata_fields_has_expected_count() {
        assert_eq!(REQUIRED_CRATES_IO_METADATA_FIELDS.len(), 11);
        assert!(REQUIRED_CRATES_IO_METADATA_FIELDS.contains(&"name"));
        assert!(REQUIRED_CRATES_IO_METADATA_FIELDS.contains(&"keywords"));
    }

    #[test]
    fn load_workspace_microcrate_names_succeeds_from_repo_root() {
        // Use the actual workspace root — this is a filesystem test
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()  // crates/
            .and_then(|p| p.parent())  // workspace root
            .map(|p| p.to_path_buf())
            .unwrap();
        let names = load_workspace_microcrate_names(&workspace_root).unwrap();
        // The workspace has many crates; confirm at least some known ones are present
        assert!(names.contains("flight-core"), "expected flight-core in workspace members");
        assert!(names.contains("flight-axis"), "expected flight-axis in workspace members");
    }
}
