---
doc_id: DOC-DEV-CROSS-REF
kind: explanation
area: ci
status: active
links:
  requirements: ["INF-REQ-6"]
  tasks: []
  adrs: []
---

# Cross-Reference Checking Module

## Overview

The cross-reference checking module (`xtask/src/cross_ref.rs`) provides functionality to validate links and references between different project artifacts:

- Documentation → Spec ledger (requirement links)
- Spec ledger → Codebase (test references)
- Gherkin → Spec ledger (tags) [prepared for future use]

## Key Components

### Data Structures

- **SpecLedger**: Represents the spec ledger with requirements and acceptance criteria
- **Requirement**: A single requirement with ID, name, status, and acceptance criteria
- **AcceptanceCriteria**: An acceptance criterion with ID, description, and test references
- **TestReference**: Either a simple string path or detailed object with test/feature/command
- **CrossRefError**: Enum representing different types of cross-reference errors

### Public Functions

#### `build_req_index(ledger: &SpecLedger) -> (HashSet<String>, HashSet<String>)`
Builds indexes of requirement IDs and acceptance criteria IDs for fast lookup.

**Returns**: Tuple of (requirement_ids, ac_ids)

#### `validate_doc_links(docs: &[(PathBuf, FrontMatter)], req_ids: &HashSet<String>) -> Vec<CrossRefError>`
Validates that all requirement IDs referenced in documentation front matter exist in the spec ledger.

**Parameters**:
- `docs`: List of (path, front_matter) tuples
- `req_ids`: Set of valid requirement IDs

**Returns**: Vector of broken link errors

#### `validate_test_references(ledger: &SpecLedger) -> Vec<CrossRefError>`
Validates that all test references in the spec ledger point to actual tests in the codebase.

**Features**:
- Parses test reference format: `"<crate>::<module_path>::<test_fn_name>"`
- Checks if crate is in workspace members
- Uses ripgrep (with grep fallback) to find test functions
- Emits warnings (not errors) for external crate references
- Skips validation for command and feature references

**Returns**: Vector of missing test errors and external crate warnings

### Error Codes

All errors follow the format: `[ERROR] INF-XREF-NNN: <message>`

- **INF-XREF-001**: Broken requirement link in documentation
- **INF-XREF-002**: Missing test reference in codebase
- **INF-XREF-003**: Invalid Gherkin tag (prepared for future use)
- **INF-XREF-100**: External crate warning (not an error)

### Error Severity Levels

- **Errors (0xx codes)**: Missing tests, broken links - cause validation to fail
- **Warnings (1xx codes)**: External crate references - logged but don't fail validation

## Test Coverage

### Unit Tests (9 tests)
- `test_build_req_index`: Verifies requirement indexing
- `test_validate_doc_links_valid`: Tests valid documentation links
- `test_validate_doc_links_broken`: Tests broken link detection
- `test_validate_test_references_with_external_crate`: Tests external crate warnings
- `test_validate_test_references_skips_commands`: Verifies commands are skipped
- `test_load_workspace_members`: Tests workspace member loading
- `test_validate_single_test_reference_format`: Tests test path parsing
- `test_error_formatting`: Verifies error message formatting
- `test_error_is_warning`: Tests warning detection

### Integration Tests (4 tests)
- `test_fixtures_exist`: Verifies test fixtures are present
- `test_broken_link_fixture_content`: Validates broken link fixture
- `test_missing_test_fixture_content`: Validates missing test fixture
- `test_external_crate_fixture_content`: Validates external crate fixture

### Test Fixtures

Located in `xtask/tests/fixtures/invalid/cross_ref/`:
- `docs_broken_link.md`: Doc with reference to non-existent REQ-999
- `spec_ledger_missing_test.yaml`: Ledger with reference to nonexistent test
- `spec_ledger_external_crate.yaml`: Ledger with reference to external crate

## Implementation Details

### Workspace Member Detection

The module automatically detects workspace members by:
1. Finding the workspace root Cargo.toml (searches upward from current directory)
2. Parsing the `[workspace] members = [...]` array using regex
3. Extracting crate names from paths (e.g., "crates/flight-core" → "flight-core")

### Test Function Search

Uses a two-tier approach:
1. **Primary**: ripgrep (`rg`) with Rust file type filtering
2. **Fallback**: grep with `--include=*.rs` pattern

Search pattern: `\bfn\s+<test_fn>\b` (word boundary to avoid partial matches)

### Test Reference Format

Supports two formats:

**Simple string**:
```yaml
tests:
  - flight_core::tests::test_something
```

**Detailed object**:
```yaml
tests:
  - test: flight_core::tests::test_something
  - feature: specs/features/req_1.feature:Scenario: Test
  - command: cargo bench --bench test
```

Only simple string references are validated. Commands and features are skipped.

## Future Integration

This module is designed to be integrated into the `cargo xtask validate` command (Task 14).
The functions are currently unused (generating dead code warnings) but will be called by the
validation pipeline in the next task.

## Requirements Validated

- **INF-REQ-6.1**: Documentation → spec ledger link validation
- **INF-REQ-6.2**: Spec ledger → codebase test reference validation
- **INF-REQ-6.6**: Test reference existence checking

## Dependencies

- `regex`: Pattern matching for parsing Cargo.toml and test paths
- `serde`, `serde_yaml`: Parsing spec ledger YAML
- `anyhow`: Error handling
- External tools: `rg` (ripgrep) or `grep` for test function search
