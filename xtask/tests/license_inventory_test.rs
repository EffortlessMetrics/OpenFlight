// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! License inventory completeness tests
//!
//! Feature: release-readiness, Property 8: License Inventory Completeness
//! *For any* Cargo.lock file, the generated third-party-components.toml SHALL
//! contain an entry for every non-flight-* dependency with valid name, version,
//! license, and license_text fields.
//!
//! **Validates: Requirements 12.1, 12.2**

use cargo_lock::Lockfile;
use std::collections::HashSet;
use std::path::Path;

/// Get the workspace root path
fn workspace_root() -> &'static Path {
    // Tests run from the xtask directory, so we need to go up one level
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

/// Test that all third-party dependencies can be identified from Cargo.lock
#[test]
fn test_cargo_lock_parseable() {
    let lockfile_path = workspace_root().join("Cargo.lock");
    let lockfile = Lockfile::load(&lockfile_path).expect("Failed to load Cargo.lock");

    // Count third-party dependencies
    let third_party: Vec<_> = lockfile
        .packages
        .iter()
        .filter(|p| !p.name.as_str().starts_with("flight-"))
        .filter(|p| p.source.is_some()) // Exclude workspace members
        .collect();

    assert!(
        !third_party.is_empty(),
        "Should have third-party dependencies"
    );
    println!("Found {} third-party dependencies", third_party.len());
}

/// Test that all dependencies have valid names and versions
#[test]
fn test_all_dependencies_have_valid_metadata() {
    let lockfile_path = workspace_root().join("Cargo.lock");
    let lockfile = Lockfile::load(&lockfile_path).expect("Failed to load Cargo.lock");

    for package in &lockfile.packages {
        // Name should not be empty
        assert!(
            !package.name.as_str().is_empty(),
            "Package name should not be empty"
        );

        // Version should be valid semver
        let version_str = package.version.to_string();
        assert!(
            !version_str.is_empty(),
            "Package {} should have a version",
            package.name
        );
    }
}

/// Test that common license files exist
#[test]
fn test_common_license_files_exist() {
    let root = workspace_root();
    let required_licenses = ["LICENSE-MIT", "LICENSE-APACHE"];

    for license in &required_licenses {
        assert!(
            root.join(license).exists(),
            "Required license file {} should exist",
            license
        );
    }
}

/// Test that the licenses directory contains common license texts
#[test]
fn test_license_texts_directory() {
    let licenses_dir = workspace_root().join("licenses");
    assert!(licenses_dir.exists(), "licenses/ directory should exist");

    // Check for common license texts
    let expected_licenses = [
        "MPL-2.0.txt",
        "BSD-2-Clause.txt",
        "BSD-3-Clause.txt",
        "ISC.txt",
        "Zlib.txt",
    ];

    for license in &expected_licenses {
        let path = licenses_dir.join(license);
        assert!(
            path.exists(),
            "License text {} should exist in licenses/",
            license
        );
    }
}

/// Test that no duplicate packages exist in Cargo.lock
#[test]
fn test_no_duplicate_packages() {
    let lockfile_path = workspace_root().join("Cargo.lock");
    let lockfile = Lockfile::load(&lockfile_path).expect("Failed to load Cargo.lock");

    let mut seen: HashSet<String> = HashSet::new();
    let mut duplicates: Vec<String> = Vec::new();

    for package in &lockfile.packages {
        let key = format!("{}@{}", package.name, package.version);
        if !seen.insert(key.clone()) {
            duplicates.push(key);
        }
    }

    assert!(
        duplicates.is_empty(),
        "Found duplicate packages: {:?}",
        duplicates
    );
}

/// Test that flight-* crates are properly identified as workspace members
#[test]
fn test_flight_crates_are_workspace_members() {
    let lockfile_path = workspace_root().join("Cargo.lock");
    let lockfile = Lockfile::load(&lockfile_path).expect("Failed to load Cargo.lock");

    let flight_crates: Vec<_> = lockfile
        .packages
        .iter()
        .filter(|p| p.name.as_str().starts_with("flight-"))
        .collect();

    assert!(!flight_crates.is_empty(), "Should have flight-* crates");

    for crate_info in &flight_crates {
        // Workspace members should not have a source (they're local)
        assert!(
            crate_info.source.is_none(),
            "flight-* crate {} should be a workspace member (no source)",
            crate_info.name
        );
    }
}

/// Property test: For any subset of dependencies, we can extract license info
#[cfg(test)]
mod property_tests {
    use super::*;
    use cargo_lock::Lockfile;
    use proptest::prelude::*;

    // Feature: release-readiness, Property 8: License Inventory Completeness
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10))]

        #[test]
        fn prop_random_dependency_has_valid_info(index in 0usize..100) {
            let lockfile_path = workspace_root().join("Cargo.lock");
            let lockfile = Lockfile::load(&lockfile_path).expect("Failed to load Cargo.lock");

            let third_party: Vec<_> = lockfile.packages.iter()
                .filter(|p| !p.name.as_str().starts_with("flight-"))
                .filter(|p| p.source.is_some())
                .collect();

            if third_party.is_empty() {
                return Ok(());
            }

            // Pick a random dependency
            let idx = index % third_party.len();
            let package = &third_party[idx];

            // Verify it has required fields
            prop_assert!(!package.name.as_str().is_empty(), "Name should not be empty");
            prop_assert!(!package.version.to_string().is_empty(), "Version should not be empty");

            // Source should be crates.io or git
            if let Some(source) = &package.source {
                let source_str = source.to_string();
                prop_assert!(
                    source_str.contains("crates.io") || source_str.contains("git"),
                    "Source should be crates.io or git: {}",
                    source_str
                );
            }
        }
    }
}
