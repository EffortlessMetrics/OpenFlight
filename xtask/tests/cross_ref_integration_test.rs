// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for cross-reference validation.

use std::path::PathBuf;

// We need to include the cross_ref module types
// Since xtask is a binary crate, we can't directly import from it in tests
// So we'll test via the command-line interface once it's integrated

#[test]
fn test_fixtures_exist() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert!(fixtures_dir.exists(), "Fixtures directory should exist");

    // Check that our cross-ref test fixtures exist
    let invalid_cross_ref = fixtures_dir.join("invalid/cross_ref");
    assert!(
        invalid_cross_ref.exists(),
        "Invalid cross-ref fixtures should exist"
    );

    let broken_link = invalid_cross_ref.join("docs_broken_link.md");
    assert!(
        broken_link.exists(),
        "Broken link fixture should exist: {}",
        broken_link.display()
    );

    let missing_test = invalid_cross_ref.join("spec_ledger_missing_test.yaml");
    assert!(
        missing_test.exists(),
        "Missing test fixture should exist: {}",
        missing_test.display()
    );

    let external_crate = invalid_cross_ref.join("spec_ledger_external_crate.yaml");
    assert!(
        external_crate.exists(),
        "External crate fixture should exist: {}",
        external_crate.display()
    );
}

#[test]
fn test_broken_link_fixture_content() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let broken_link = fixtures_dir.join("invalid/cross_ref/docs_broken_link.md");

    let content = std::fs::read_to_string(&broken_link).expect("Should read fixture file");

    // Verify the fixture contains the expected broken link
    assert!(
        content.contains("REQ-999"),
        "Fixture should contain broken link REQ-999"
    );
    assert!(
        content.contains("doc_id: DOC-BROKEN-LINK"),
        "Fixture should have valid front matter"
    );
}

#[test]
fn test_missing_test_fixture_content() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let missing_test = fixtures_dir.join("invalid/cross_ref/spec_ledger_missing_test.yaml");

    let content = std::fs::read_to_string(&missing_test).expect("Should read fixture file");

    // Verify the fixture contains a test reference to a nonexistent function
    assert!(
        content.contains("nonexistent_test_function"),
        "Fixture should reference nonexistent test"
    );
}

#[test]
fn test_external_crate_fixture_content() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let external_crate = fixtures_dir.join("invalid/cross_ref/spec_ledger_external_crate.yaml");

    let content = std::fs::read_to_string(&external_crate).expect("Should read fixture file");

    // Verify the fixture contains a reference to an external crate
    assert!(
        content.contains("external_crate"),
        "Fixture should reference external crate"
    );
}
