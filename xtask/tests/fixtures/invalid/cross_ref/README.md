# Cross-Reference Test Fixtures

This directory contains test fixtures for validating cross-reference checking functionality.

## Files

### docs_broken_link.md
A documentation file with front matter that references a non-existent requirement ID (REQ-999).
Used to test detection of broken requirement links.

### spec_ledger_missing_test.yaml
A spec ledger that references a test function that doesn't exist in the codebase.
Used to test detection of missing test references.

### spec_ledger_external_crate.yaml
A spec ledger that references a test in an external (non-workspace) crate.
Used to test that external crate references produce warnings instead of errors.

## Usage

These fixtures are used by:
- Unit tests in `xtask/src/cross_ref.rs`
- Integration tests in `xtask/tests/cross_ref_integration_test.rs`
