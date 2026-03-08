// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Depth tests for `flight-workspace-meta`.
//!
//! Covers: constant validation, struct behaviour, metadata resolution
//! with workspace inheritance, filesystem-based workspace discovery,
//! validation report semantics, and edge-case handling.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use flight_workspace_meta::{
    CrateMetadataIssue, CratesIoMetadata, MetadataValidationReport,
    REQUIRED_CRATES_IO_METADATA_FIELDS, load_workspace_microcrate_names,
    load_workspace_microcrates, validate_workspace_crates_io_metadata,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("could not derive workspace root from CARGO_MANIFEST_DIR")
        .to_path_buf()
}

/// Create a minimal workspace inside a temp dir.
/// Returns the `TempDir` (kept alive) and the workspace root path.
fn make_workspace(
    workspace_toml: &str,
    crates: &[(&str, &str)], // (relative dir under crates/, crate Cargo.toml content)
) -> (TempDir, PathBuf) {
    let tmp = TempDir::new().expect("failed to create tempdir");
    let root = tmp.path().to_path_buf();

    fs::write(root.join("Cargo.toml"), workspace_toml).unwrap();

    for (dir, content) in crates {
        let crate_dir = root.join("crates").join(dir);
        fs::create_dir_all(&crate_dir).unwrap();
        fs::write(crate_dir.join("Cargo.toml"), content).unwrap();
    }

    (tmp, root)
}

// ---------------------------------------------------------------------------
// REQUIRED_CRATES_IO_METADATA_FIELDS constant
// ---------------------------------------------------------------------------

#[test]
fn required_fields_has_eleven_entries() {
    assert_eq!(REQUIRED_CRATES_IO_METADATA_FIELDS.len(), 11);
}

#[test]
fn required_fields_are_unique() {
    let set: HashSet<&str> = REQUIRED_CRATES_IO_METADATA_FIELDS.iter().copied().collect();
    assert_eq!(set.len(), REQUIRED_CRATES_IO_METADATA_FIELDS.len());
}

#[test]
fn required_fields_contains_all_expected() {
    let expected = [
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
    for field in &expected {
        assert!(
            REQUIRED_CRATES_IO_METADATA_FIELDS.contains(field),
            "missing required field: {field}"
        );
    }
}

#[test]
fn required_fields_are_non_empty_strings() {
    for field in &REQUIRED_CRATES_IO_METADATA_FIELDS {
        assert!(!field.is_empty(), "empty string in required fields");
    }
}

// ---------------------------------------------------------------------------
// CratesIoMetadata — readme_path
// ---------------------------------------------------------------------------

#[test]
fn readme_path_none_when_field_is_none() {
    let meta = CratesIoMetadata::default();
    assert!(meta.readme_path(Path::new("/a/b/Cargo.toml")).is_none());
}

#[test]
fn readme_path_some_when_field_is_some_even_if_empty() {
    let meta = CratesIoMetadata {
        readme: Some(String::new()),
        ..Default::default()
    };
    // An empty readme still yields a path (empty component joined)
    // — the function returns Some because the Option is Some.
    assert!(meta.readme_path(Path::new("/a/b/Cargo.toml")).is_some());
}

#[test]
fn readme_path_relative_joins_to_manifest_parent() {
    let meta = CratesIoMetadata {
        readme: Some("README.md".into()),
        ..Default::default()
    };
    let path = meta.readme_path(Path::new("/crates/foo/Cargo.toml"));
    assert_eq!(path, Some(PathBuf::from("/crates/foo/README.md")));
}

#[test]
fn readme_path_relative_nested() {
    let meta = CratesIoMetadata {
        readme: Some("docs/README.md".into()),
        ..Default::default()
    };
    let path = meta.readme_path(Path::new("/ws/crates/bar/Cargo.toml"));
    assert_eq!(path, Some(PathBuf::from("/ws/crates/bar/docs/README.md")));
}

#[test]
fn readme_path_absolute_returned_as_is() {
    let abs = if cfg!(windows) {
        "C:\\docs\\README.md"
    } else {
        "/docs/README.md"
    };
    let meta = CratesIoMetadata {
        readme: Some(abs.into()),
        ..Default::default()
    };
    let path = meta.readme_path(Path::new("/irrelevant/Cargo.toml"));
    assert_eq!(path, Some(PathBuf::from(abs)));
}

#[test]
fn readme_path_manifest_without_parent() {
    let meta = CratesIoMetadata {
        readme: Some("README.md".into()),
        ..Default::default()
    };
    // A bare filename has no parent directory component.
    let path = meta.readme_path(Path::new("Cargo.toml"));
    // Parent of "Cargo.toml" is "" which is treated as current dir.
    assert!(path.is_some());
}

// ---------------------------------------------------------------------------
// CratesIoMetadata — Default
// ---------------------------------------------------------------------------

#[test]
fn crates_io_metadata_default_has_no_data() {
    let meta = CratesIoMetadata::default();
    assert!(meta.version.is_none());
    assert!(meta.edition.is_none());
    assert!(meta.rust_version.is_none());
    assert!(meta.license.is_none());
    assert!(meta.repository.is_none());
    assert!(meta.homepage.is_none());
    assert!(meta.description.is_none());
    assert!(meta.readme.is_none());
    assert!(meta.keywords.is_empty());
    assert!(meta.categories.is_empty());
}

// ---------------------------------------------------------------------------
// MetadataValidationReport
// ---------------------------------------------------------------------------

#[test]
fn default_report_is_success() {
    let r = MetadataValidationReport::default();
    assert!(r.is_success());
    assert_eq!(r.checked, 0);
}

#[test]
fn report_with_zero_issues_is_success() {
    let r = MetadataValidationReport {
        checked: 5,
        issues: vec![],
    };
    assert!(r.is_success());
}

#[test]
fn report_with_one_issue_is_not_success() {
    let r = MetadataValidationReport {
        checked: 1,
        issues: vec![CrateMetadataIssue {
            crate_name: "x".into(),
            missing_fields: vec!["license".into()],
            invalid_fields: vec![],
        }],
    };
    assert!(!r.is_success());
}

#[test]
fn report_with_multiple_issues_is_not_success() {
    let r = MetadataValidationReport {
        checked: 3,
        issues: vec![
            CrateMetadataIssue {
                crate_name: "a".into(),
                missing_fields: vec!["version".into()],
                invalid_fields: vec![],
            },
            CrateMetadataIssue {
                crate_name: "b".into(),
                missing_fields: vec![],
                invalid_fields: vec!["readme missing".into()],
            },
        ],
    };
    assert!(!r.is_success());
    assert_eq!(r.issues.len(), 2);
}

// ---------------------------------------------------------------------------
// CrateMetadataIssue — summary
// ---------------------------------------------------------------------------

#[test]
fn summary_with_only_missing_fields() {
    let issue = CrateMetadataIssue {
        crate_name: "alpha".into(),
        missing_fields: vec!["version".into(), "license".into()],
        invalid_fields: vec![],
    };
    let s = issue.summary();
    assert!(s.starts_with("alpha"));
    assert!(s.contains("missing: version, license"));
    assert!(!s.contains("invalid"));
}

#[test]
fn summary_with_only_invalid_fields() {
    let issue = CrateMetadataIssue {
        crate_name: "beta".into(),
        missing_fields: vec![],
        invalid_fields: vec!["readme does not exist".into()],
    };
    let s = issue.summary();
    assert!(s.contains("invalid"));
    assert!(!s.contains("missing"));
}

#[test]
fn summary_with_both_missing_and_invalid() {
    let issue = CrateMetadataIssue {
        crate_name: "gamma".into(),
        missing_fields: vec!["keywords".into()],
        invalid_fields: vec!["readme does not exist".into()],
    };
    let s = issue.summary();
    assert!(s.contains("missing: keywords"));
    assert!(s.contains("invalid: readme does not exist"));
    assert!(s.contains(';'));
}

#[test]
fn summary_with_empty_fields() {
    let issue = CrateMetadataIssue::default();
    let s = issue.summary();
    // Should still contain the crate name (empty) and parenthesized content
    assert!(s.contains('('));
    assert!(s.contains(')'));
}

#[test]
fn issue_default_has_empty_fields() {
    let issue = CrateMetadataIssue::default();
    assert!(issue.crate_name.is_empty());
    assert!(issue.missing_fields.is_empty());
    assert!(issue.invalid_fields.is_empty());
}

// ---------------------------------------------------------------------------
// load_workspace_microcrate_names — live workspace
// ---------------------------------------------------------------------------

#[test]
fn live_workspace_names_contains_known_crates() {
    let names = load_workspace_microcrate_names(workspace_root()).unwrap();
    // Structural invariant: at least one workspace microcrate exists.
    assert!(
        !names.is_empty(),
        "expected at least one workspace microcrate, found none"
    );
    // All reported crate names should be non-empty strings.
    for name in &names {
        assert!(
            !name.is_empty(),
            "workspace microcrate name should not be empty"
        );
    }
    // This crate must always be present in the workspace.
    assert!(
        names.contains("flight-workspace-meta"),
        "expected flight-workspace-meta in workspace members"
    );
}

#[test]
fn live_workspace_names_returns_btreeset() {
    let names = load_workspace_microcrate_names(workspace_root()).unwrap();
    // BTreeSet is ordered; first element should be lexicographically smallest
    let first = names.iter().next().expect("no crates found");
    let second = names.iter().nth(1).expect("fewer than 2 crates");
    assert!(first <= second, "BTreeSet ordering violated");
}

#[test]
fn live_workspace_names_no_duplicates() {
    let names = load_workspace_microcrate_names(workspace_root()).unwrap();
    let vec: Vec<_> = names.iter().collect();
    let set: HashSet<_> = vec.iter().collect();
    assert_eq!(vec.len(), set.len(), "duplicate crate names detected");
}

// ---------------------------------------------------------------------------
// load_workspace_microcrates — live workspace
// ---------------------------------------------------------------------------

#[test]
fn live_microcrates_sorted_by_name() {
    let crates = load_workspace_microcrates(workspace_root()).unwrap();
    let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "crates should be sorted by name");
}

#[test]
fn live_microcrates_manifest_paths_exist() {
    let crates = load_workspace_microcrates(workspace_root()).unwrap();
    for c in &crates {
        assert!(
            c.manifest_path.exists(),
            "manifest {} does not exist",
            c.manifest_path.display()
        );
    }
}

#[test]
fn live_microcrates_all_have_names() {
    let crates = load_workspace_microcrates(workspace_root()).unwrap();
    for c in &crates {
        assert!(!c.name.is_empty(), "crate with empty name found");
    }
}

#[test]
fn live_microcrates_have_metadata() {
    let crates = load_workspace_microcrates(workspace_root()).unwrap();
    // At least some crates should have version/edition set via workspace inheritance
    let with_version = crates
        .iter()
        .filter(|c| c.metadata.version.is_some())
        .count();
    assert!(
        with_version > 0,
        "expected at least one crate to have a resolved version"
    );
}

// ---------------------------------------------------------------------------
// validate_workspace_crates_io_metadata — live workspace
// ---------------------------------------------------------------------------

#[test]
fn live_validation_checks_all_microcrates() {
    let names = load_workspace_microcrate_names(workspace_root()).unwrap();
    let report = validate_workspace_crates_io_metadata(workspace_root()).unwrap();
    assert_eq!(
        report.checked,
        names.len(),
        "report should check all discovered crates"
    );
}

#[test]
fn live_validation_report_has_plausible_checked_count() {
    let report = validate_workspace_crates_io_metadata(workspace_root()).unwrap();
    assert!(
        report.checked > 5,
        "expected many crates, got {}",
        report.checked
    );
}

#[test]
fn live_validation_issue_names_are_workspace_members() {
    let names = load_workspace_microcrate_names(workspace_root()).unwrap();
    let report = validate_workspace_crates_io_metadata(workspace_root()).unwrap();
    for issue in &report.issues {
        assert!(
            names.contains(&issue.crate_name),
            "issue crate '{}' not in workspace members",
            issue.crate_name
        );
    }
}

// ---------------------------------------------------------------------------
// Synthetic workspace — field resolution & inheritance
// ---------------------------------------------------------------------------

fn minimal_workspace_toml(members: &[&str]) -> String {
    let members_str: Vec<String> = members.iter().map(|m| format!("\"crates/{m}\"")).collect();
    format!(
        r#"
[workspace]
members = [{members}]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.92.0"
license = "MIT"
repository = "https://github.com/example/test"
homepage = "https://example.com"
authors = ["Test"]
categories = ["simulation"]
keywords = ["test"]
"#,
        members = members_str.join(", ")
    )
}

fn full_crate_toml(name: &str) -> String {
    format!(
        r#"
[package]
name = "{name}"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "A test crate"
readme = "README.md"
keywords.workspace = true
categories.workspace = true
"#
    )
}

#[test]
fn synthetic_single_crate_inherits_workspace_fields() {
    let ws = minimal_workspace_toml(&["alpha"]);
    let crate_toml = full_crate_toml("alpha");
    let (_tmp, root) = make_workspace(&ws, &[("alpha", &crate_toml)]);
    fs::write(root.join("crates/alpha/README.md"), "# Alpha").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates.len(), 1);
    let c = &crates[0];
    assert_eq!(c.name, "alpha");
    assert_eq!(c.metadata.version.as_deref(), Some("0.1.0"));
    assert_eq!(c.metadata.edition.as_deref(), Some("2024"));
    assert_eq!(c.metadata.rust_version.as_deref(), Some("1.92.0"));
    assert_eq!(c.metadata.license.as_deref(), Some("MIT"));
}

#[test]
fn synthetic_crate_local_description_not_overridden() {
    let ws = minimal_workspace_toml(&["beta"]);
    let crate_toml = full_crate_toml("beta");
    let (_tmp, root) = make_workspace(&ws, &[("beta", &crate_toml)]);
    fs::write(root.join("crates/beta/README.md"), "# Beta").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(
        crates[0].metadata.description.as_deref(),
        Some("A test crate")
    );
}

#[test]
fn synthetic_multiple_crates_sorted() {
    let ws = minimal_workspace_toml(&["zeta", "alpha", "mu"]);
    let (_tmp, root) = make_workspace(
        &ws,
        &[
            ("zeta", &full_crate_toml("zeta")),
            ("alpha", &full_crate_toml("alpha")),
            ("mu", &full_crate_toml("mu")),
        ],
    );
    for name in ["zeta", "alpha", "mu"] {
        fs::write(root.join(format!("crates/{name}/README.md")), "# X").unwrap();
    }

    let crates = load_workspace_microcrates(&root).unwrap();
    let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mu", "zeta"]);
}

#[test]
fn synthetic_crate_names_as_btreeset() {
    let ws = minimal_workspace_toml(&["bb", "aa"]);
    let (_tmp, root) = make_workspace(
        &ws,
        &[
            ("bb", &full_crate_toml("bb")),
            ("aa", &full_crate_toml("aa")),
        ],
    );

    let names = load_workspace_microcrate_names(&root).unwrap();
    let expected: BTreeSet<String> = ["aa", "bb"].iter().map(|s| s.to_string()).collect();
    assert_eq!(names, expected);
}

#[test]
fn synthetic_non_crates_member_excluded() {
    // Members not under "crates/" should be excluded by is_microcrate_member
    let ws = r#"
[workspace]
members = ["tools/xtask", "crates/real"]

[workspace.package]
version = "0.1.0"
edition = "2024"
"#;
    let (_tmp, root) = make_workspace(ws, &[("real", &full_crate_toml("real"))]);
    // Also create tools/xtask with a valid Cargo.toml
    let xtask_dir = root.join("tools/xtask");
    fs::create_dir_all(&xtask_dir).unwrap();
    fs::write(
        xtask_dir.join("Cargo.toml"),
        "[package]\nname = \"xtask\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(root.join("crates/real/README.md"), "# Real").unwrap();

    let names = load_workspace_microcrate_names(&root).unwrap();
    assert!(names.contains("real"));
    assert!(!names.contains("xtask"), "xtask should not be a microcrate");
}

#[test]
fn synthetic_missing_manifest_skipped() {
    // Member declared but Cargo.toml missing on disk
    let ws = minimal_workspace_toml(&["exists", "ghost"]);
    let (_tmp, root) = make_workspace(&ws, &[("exists", &full_crate_toml("exists"))]);
    fs::write(root.join("crates/exists/README.md"), "# X").unwrap();
    // "ghost" directory not created at all

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates.len(), 1);
    assert_eq!(crates[0].name, "exists");
}

// ---------------------------------------------------------------------------
// Synthetic workspace — validation
// ---------------------------------------------------------------------------

#[test]
fn synthetic_validation_all_fields_present_passes() {
    let ws = minimal_workspace_toml(&["good"]);
    let (_tmp, root) = make_workspace(&ws, &[("good", &full_crate_toml("good"))]);
    fs::write(root.join("crates/good/README.md"), "# Good").unwrap();

    let report = validate_workspace_crates_io_metadata(&root).unwrap();
    assert_eq!(report.checked, 1);
    assert!(
        report.is_success(),
        "expected clean validation, got issues: {:?}",
        report
            .issues
            .iter()
            .map(|i| i.summary())
            .collect::<Vec<_>>()
    );
}

#[test]
fn synthetic_validation_missing_description() {
    let ws = minimal_workspace_toml(&["nodesc"]);
    let crate_toml = r#"
[package]
name = "nodesc"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords.workspace = true
categories.workspace = true
"#;
    let (_tmp, root) = make_workspace(&ws, &[("nodesc", crate_toml)]);
    fs::write(root.join("crates/nodesc/README.md"), "# No desc").unwrap();

    let report = validate_workspace_crates_io_metadata(&root).unwrap();
    assert!(!report.is_success());
    let issue = &report.issues[0];
    assert_eq!(issue.crate_name, "nodesc");
    assert!(issue.missing_fields.contains(&"description".to_string()));
}

#[test]
fn synthetic_validation_missing_readme_file() {
    let ws = minimal_workspace_toml(&["noreadme"]);
    let crate_toml = full_crate_toml("noreadme");
    let (_tmp, root) = make_workspace(&ws, &[("noreadme", &crate_toml)]);
    // README.md NOT created on disk

    let report = validate_workspace_crates_io_metadata(&root).unwrap();
    assert!(!report.is_success());
    let issue = &report.issues[0];
    assert_eq!(issue.crate_name, "noreadme");
    assert!(
        issue.invalid_fields.iter().any(|f| f.contains("readme")),
        "expected invalid readme field, got: {:?}",
        issue.invalid_fields
    );
}

#[test]
fn synthetic_validation_missing_keywords() {
    let ws = r#"
[workspace]
members = ["crates/nokw"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.92.0"
license = "MIT"
repository = "https://github.com/example/test"
homepage = "https://example.com"
categories = ["simulation"]
"#;
    let crate_toml = r#"
[package]
name = "nokw"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "Test"
readme = "README.md"
categories.workspace = true
"#;
    let (_tmp, root) = make_workspace(&ws, &[("nokw", crate_toml)]);
    fs::write(root.join("crates/nokw/README.md"), "# No KW").unwrap();

    let report = validate_workspace_crates_io_metadata(&root).unwrap();
    assert!(!report.is_success());
    assert!(
        report.issues[0]
            .missing_fields
            .contains(&"keywords".to_string())
    );
}

#[test]
fn synthetic_validation_missing_categories() {
    let ws = r#"
[workspace]
members = ["crates/nocat"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.92.0"
license = "MIT"
repository = "https://github.com/example/test"
homepage = "https://example.com"
keywords = ["test"]
"#;
    let crate_toml = r#"
[package]
name = "nocat"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "Test"
readme = "README.md"
keywords.workspace = true
"#;
    let (_tmp, root) = make_workspace(&ws, &[("nocat", crate_toml)]);
    fs::write(root.join("crates/nocat/README.md"), "# No Cat").unwrap();

    let report = validate_workspace_crates_io_metadata(&root).unwrap();
    assert!(!report.is_success());
    assert!(
        report.issues[0]
            .missing_fields
            .contains(&"categories".to_string())
    );
}

#[test]
fn synthetic_validation_empty_workspace_has_zero_checked() {
    let ws = r#"
[workspace]
members = []
"#;
    let (_tmp, root) = make_workspace(ws, &[]);

    let report = validate_workspace_crates_io_metadata(&root).unwrap();
    assert_eq!(report.checked, 0);
    assert!(report.is_success());
}

// ---------------------------------------------------------------------------
// find_workspace_root — traversal from subdirectory
// ---------------------------------------------------------------------------

#[test]
fn load_from_subdirectory_finds_workspace() {
    let ws = minimal_workspace_toml(&["sub"]);
    let (_tmp, root) = make_workspace(&ws, &[("sub", &full_crate_toml("sub"))]);
    fs::write(root.join("crates/sub/README.md"), "# Sub").unwrap();

    // Invoke from the crate directory rather than workspace root
    let crate_dir = root.join("crates").join("sub");
    let crates = load_workspace_microcrates(&crate_dir).unwrap();
    assert_eq!(crates.len(), 1);
    assert_eq!(crates[0].name, "sub");
}

#[test]
fn load_from_file_inside_workspace() {
    let ws = minimal_workspace_toml(&["leaf"]);
    let (_tmp, root) = make_workspace(&ws, &[("leaf", &full_crate_toml("leaf"))]);
    fs::write(root.join("crates/leaf/README.md"), "# Leaf").unwrap();

    // Pass the Cargo.toml file path directly (find_workspace_root handles files)
    let manifest = root.join("crates").join("leaf").join("Cargo.toml");
    let crates = load_workspace_microcrates(&manifest).unwrap();
    assert_eq!(crates.len(), 1);
}

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

#[test]
fn load_from_nonexistent_dir_returns_error() {
    let tmp = TempDir::new().unwrap();
    let missing_dir = tmp.path().join("definitely_missing");
    let result = load_workspace_microcrates(&missing_dir);
    assert!(result.is_err());
}

#[test]
fn load_from_dir_without_workspace_returns_error() {
    let tmp = TempDir::new().unwrap();
    // No Cargo.toml at all
    let result = load_workspace_microcrates(tmp.path());
    assert!(result.is_err());
}

#[test]
fn load_from_dir_with_non_workspace_cargo_toml_returns_error() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"leaf\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    // Cargo.toml exists but has no [workspace] section
    let result = load_workspace_microcrates(tmp.path());
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Keyword/category vec resolution
// ---------------------------------------------------------------------------

#[test]
fn synthetic_keywords_inherited_from_workspace() {
    let ws = minimal_workspace_toml(&["kw"]);
    let (_tmp, root) = make_workspace(&ws, &[("kw", &full_crate_toml("kw"))]);
    fs::write(root.join("crates/kw/README.md"), "# KW").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates[0].metadata.keywords, vec!["test"]);
}

#[test]
fn synthetic_categories_inherited_from_workspace() {
    let ws = minimal_workspace_toml(&["cat"]);
    let (_tmp, root) = make_workspace(&ws, &[("cat", &full_crate_toml("cat"))]);
    fs::write(root.join("crates/cat/README.md"), "# Cat").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates[0].metadata.categories, vec!["simulation"]);
}

#[test]
fn synthetic_local_keywords_override_workspace() {
    let ws = minimal_workspace_toml(&["localkw"]);
    let crate_toml = r#"
[package]
name = "localkw"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "Test"
readme = "README.md"
keywords = ["custom-a", "custom-b"]
categories.workspace = true
"#;
    let (_tmp, root) = make_workspace(&ws, &[("localkw", crate_toml)]);
    fs::write(root.join("crates/localkw/README.md"), "# Local KW").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates[0].metadata.keywords, vec!["custom-a", "custom-b"]);
}

// ---------------------------------------------------------------------------
// Edge cases — whitespace trimming
// ---------------------------------------------------------------------------

#[test]
fn synthetic_whitespace_in_description_trimmed() {
    let ws = minimal_workspace_toml(&["ws"]);
    let crate_toml = r#"
[package]
name = "ws"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "   padded   "
readme = "README.md"
keywords.workspace = true
categories.workspace = true
"#;
    let (_tmp, root) = make_workspace(&ws, &[("ws", crate_toml)]);
    fs::write(root.join("crates/ws/README.md"), "# WS").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates[0].metadata.description.as_deref(), Some("padded"));
}

#[test]
fn synthetic_empty_keyword_strings_filtered_out() {
    let ws = r#"
[workspace]
members = ["crates/empkw"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.92.0"
license = "MIT"
repository = "https://github.com/example/test"
homepage = "https://example.com"
categories = ["simulation"]
keywords = ["good", "  ", ""]
"#;
    let crate_toml = r#"
[package]
name = "empkw"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "Test"
readme = "README.md"
keywords.workspace = true
categories.workspace = true
"#;
    let (_tmp, root) = make_workspace(&ws, &[("empkw", crate_toml)]);
    fs::write(root.join("crates/empkw/README.md"), "# Empty KW").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates[0].metadata.keywords, vec!["good"]);
}

// ---------------------------------------------------------------------------
// Name fallback — package.name absent
// ---------------------------------------------------------------------------

#[test]
fn synthetic_name_fallback_to_directory() {
    let ws = minimal_workspace_toml(&["fallback-name"]);
    let crate_toml = r#"
[package]
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "No name"
readme = "README.md"
keywords.workspace = true
categories.workspace = true
"#;
    let (_tmp, root) = make_workspace(&ws, &[("fallback-name", crate_toml)]);
    fs::write(root.join("crates/fallback-name/README.md"), "# FB").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    assert_eq!(crates[0].name, "fallback-name");
}

// ---------------------------------------------------------------------------
// WorkspaceCrateMetadata — Clone / Debug
// ---------------------------------------------------------------------------

#[test]
fn workspace_crate_metadata_is_cloneable() {
    let ws = minimal_workspace_toml(&["cl"]);
    let (_tmp, root) = make_workspace(&ws, &[("cl", &full_crate_toml("cl"))]);
    fs::write(root.join("crates/cl/README.md"), "# CL").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    let cloned = crates[0].clone();
    assert_eq!(cloned.name, crates[0].name);
}

#[test]
fn workspace_crate_metadata_is_debuggable() {
    let ws = minimal_workspace_toml(&["dbg"]);
    let (_tmp, root) = make_workspace(&ws, &[("dbg", &full_crate_toml("dbg"))]);
    fs::write(root.join("crates/dbg/README.md"), "# DBG").unwrap();

    let crates = load_workspace_microcrates(&root).unwrap();
    let debug_str = format!("{:?}", crates[0]);
    assert!(
        debug_str.contains("dbg"),
        "Debug output should contain crate name"
    );
}

#[test]
fn crates_io_metadata_is_cloneable() {
    let meta = CratesIoMetadata {
        version: Some("1.0.0".into()),
        ..Default::default()
    };
    let cloned = meta.clone();
    assert_eq!(cloned.version, meta.version);
}

#[test]
fn validation_report_is_cloneable() {
    let report = MetadataValidationReport {
        checked: 3,
        issues: vec![CrateMetadataIssue {
            crate_name: "x".into(),
            missing_fields: vec!["a".into()],
            invalid_fields: vec![],
        }],
    };
    let cloned = report.clone();
    assert_eq!(cloned.checked, 3);
    assert_eq!(cloned.issues.len(), 1);
}

#[test]
fn crate_metadata_issue_is_cloneable() {
    let issue = CrateMetadataIssue {
        crate_name: "test".into(),
        missing_fields: vec!["version".into()],
        invalid_fields: vec!["readme".into()],
    };
    let cloned = issue.clone();
    assert_eq!(cloned.crate_name, "test");
    assert_eq!(cloned.missing_fields.len(), 1);
    assert_eq!(cloned.invalid_fields.len(), 1);
}
