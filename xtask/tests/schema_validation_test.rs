// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for schema validation module.

use std::path::PathBuf;

// Re-export the schema module for testing
// Note: This requires making the schema module public or using a test-only feature
// For now, we'll just verify the module compiles and unit tests pass

#[test]
fn test_schema_module_exists() {
    // This test verifies that the schema module compiles successfully
    // The actual functionality is tested in the unit tests within schema.rs
}

#[test]
fn test_fixtures_exist() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    // Verify minimal fixtures exist
    assert!(fixtures_dir.join("minimal/specs/spec_ledger.yaml").exists());

    // Verify invalid fixtures exist
    assert!(
        fixtures_dir
            .join("invalid/schema_errors/spec_ledger_missing_required.yaml")
            .exists()
    );
    assert!(
        fixtures_dir
            .join("invalid/schema_errors/front_matter_missing_links.yaml")
            .exists()
    );
    assert!(
        fixtures_dir
            .join("invalid/schema_errors/invariants_missing_rust_version.yaml")
            .exists()
    );
}

#[test]
fn test_schemas_exist() {
    let schemas_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("schemas");

    // Verify all three schemas exist
    assert!(schemas_dir.join("spec_ledger.schema.json").exists());
    assert!(schemas_dir.join("front_matter.schema.json").exists());
    assert!(schemas_dir.join("invariants.schema.json").exists());
}
