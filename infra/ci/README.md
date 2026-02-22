# CI Configuration

This directory contains documentation for the Flight Hub Continuous Integration (CI) pipeline.

## Overview

The CI pipeline enforces quality gates and ensures all code changes meet project standards before merging. All CI jobs use the same `cargo xtask` commands as local development to maintain consistency.

## CI Jobs and Their Purpose

### 1. Validation Pipeline (`validate`)
**Purpose**: Runs the comprehensive validation suite using `cargo xtask validate`

**What it does**:
- Schema validation (spec ledger, front matter, invariants)
- Cross-reference checking (docs ↔ ledger ↔ tests ↔ Gherkin)
- Code quality checks (formatting, clippy, tests)
- Public API verification
- Generates validation and feature status reports

**Artifacts**:
- `validation-report` - docs/validation_report.md
- `feature-status` - docs/feature_status.md

**Timeout**: 10 minutes

### 2. Test Suite (`test`)
**Purpose**: Runs comprehensive test suite across multiple platforms and Rust versions

**What it does**:
- Formatting checks (`cargo fmt --all -- --check`)
- Clippy linting (general and strict for core crates)
- File descriptor safety tests for public API crates
- Critical pattern verification (Profile::merge_with, BlackboxWriter, etc.)
- Full test suite (`cargo test --all-features --workspace`)
- Documentation tests (`cargo test --doc --workspace`)
- ADR link validation

**Platforms**: Ubuntu, Windows
**Rust versions**: stable, 1.92.0
**Timeout**: 10 minutes (Ubuntu), 20 minutes (Windows)

### 3. MSRV Check (`msrv-check`)
**Purpose**: Ensures code builds with Minimum Supported Rust Version (1.92.0)

**What it does**:
- Builds all crates with MSRV
- Runs clippy with strict warnings

**Timeout**: 30 minutes

### 4. Path Filter (`path-filter`)
**Purpose**: Determines which crates changed to optimize CI runs

**Outputs**:
- `ipc`: Changes in flight-ipc
- `hid`: Changes in flight-hid
- `core`: Changes in flight-core or workspace config

### 5. Clippy - Core Crates
**Purpose**: Strict clippy checks for core crates

**Jobs**:
- `clippy-core`: flight-core specific checks
- `clippy-ipc-benches`: IPC benchmark checks (strict and unblock modes)

**Timeout**: 30 minutes (Ubuntu), 45 minutes (Windows)

### 6. Public API Guard (`public-api-check`)
**Purpose**: Prevents unintended breaking changes to public APIs

**What it does**:
- Checks public API changes for flight-core, flight-ipc, flight-hid
- Compares against main branch
- Falls back to nightly toolchain if needed

**Runs on**: Pull requests only
**Timeout**: 30 minutes

### 7. Gated Feature Tests
**Purpose**: Tests optional features that require special setup

**Jobs**:
- `gated-ipc-smoke`: IPC benchmark smoke tests
- `gated-hid-smoke`: HID feature tests

**Triggers**: Schedule, workflow_dispatch, 'run-gated' label, or path changes
**Timeout**: 30 minutes

### 8. Cross-Platform Verification (`cross-platform`)
**Purpose**: Ensures code works across all supported platforms

**What it does**:
- Workspace checks
- Strict clippy on all targets
- Formatting verification

**Runs on**: Scheduled builds only
**Platforms**: Ubuntu, Windows
**Timeout**: 30 minutes (Ubuntu), 45 minutes (Windows)

### 9. Security Audit (`security`)
**Purpose**: Checks for security vulnerabilities and supply chain issues

**What it does**:
- `cargo audit` - checks for known vulnerabilities
- `cargo deny` - enforces license and dependency policies
- HTTP stack unification validation (ensures rustls, no native-tls/openssl)
- Hyper version consistency checks

**Timeout**: 30 minutes

### 10. Build Release (`build`)
**Purpose**: Verifies release builds succeed

**What it does**:
- Builds workspace in release mode
- Uploads artifacts (flightd, flightctl)

**Platforms**: Ubuntu, Windows
**Timeout**: 30 minutes (Ubuntu), 45 minutes (Windows)

### 11. Feature Powerset Testing (`feature-powerset`)
**Purpose**: Tests various feature combinations to catch feature interaction bugs

**What it does**:
- Uses `cargo-hack` to test feature combinations (depth 2)
- Verifies workspace dependency alignment
- Checks tokio, futures, tonic version consistency

**Timeout**: 30 minutes

### 12. Performance Tests (`performance`)
**Purpose**: Runs benchmark suite to detect performance regressions

**What it does**:
- Runs all workspace benchmarks
- Performance regression detection (TODO)

**Timeout**: 30 minutes

## Quality Gates Enforced

All checks must pass before code can be merged. The following quality gates are enforced:

### Code Quality
- ✅ **Formatting**: All code must be formatted with `rustfmt`
- ✅ **Linting**: Core crates must pass clippy with `-D warnings`
- ✅ **Tests**: All unit tests and doc tests must pass
- ✅ **MSRV**: Code must build with Rust 1.92.0

### Documentation
- ✅ **Schema Validation**: All YAML/JSON must conform to schemas
- ✅ **Cross-References**: All requirement links must be valid
- ✅ **ADR Links**: All referenced ADRs must exist

### Security
- ✅ **Audit**: No known security vulnerabilities
- ✅ **Supply Chain**: Dependencies must meet license/policy requirements
- ✅ **HTTP Stack**: Only rustls allowed (no native-tls/openssl)

### API Stability
- ✅ **Public API**: Breaking changes must be intentional and documented
- ✅ **Feature Compatibility**: Feature combinations must work correctly

### Platform Support
- ✅ **Cross-Platform**: Code must work on Ubuntu and Windows
- ✅ **Architecture**: Builds must succeed on all target platforms

## Guarantees

The CI pipeline provides the following guarantees:

1. **All checks must pass for merge**: No code can be merged if any CI job fails
2. **Consistent validation**: CI uses the same `cargo xtask` commands as local development
3. **Platform compatibility**: Code is tested on Ubuntu and Windows
4. **MSRV compliance**: Code builds with Rust 1.89.0
5. **Security**: No known vulnerabilities or banned dependencies
6. **API stability**: Public API changes are detected and reviewed
7. **Test coverage**: All tests pass on all supported platforms
8. **Documentation accuracy**: All cross-references are valid

## Error Reporting

CI jobs emit structured errors following the format:
```
[ERROR] <error_code>: <message>
  File: <path>:<line>:<column>
  Expected: <expected_value>
  Found: <actual_value>
  Suggestion: <fix_suggestion>
```

Error code families:
- `INF-SCHEMA-xxx`: Schema validation errors
- `INF-XREF-xxx`: Cross-reference errors
- `INF-INFRA-xxx`: Infrastructure validation errors
- `INF-VALID-xxx`: General validation pipeline errors

## Running CI Locally

To run the same checks locally that CI runs:

```bash
# Fast smoke test (fmt, clippy, core tests)
cargo xtask check

# Full validation pipeline (same as CI validate job)
cargo xtask validate

# Generate feature status report
cargo xtask ac-status

# Validate infrastructure configs
cargo xtask validate-infra
```

## Triggering Gated Tests

Some tests are gated behind labels or schedules:

- **Gated features**: Add `run-gated` label to PR
- **Clippy unblock mode**: Add `clippy-unblock` label to PR
- **Manual trigger**: Use workflow_dispatch in GitHub Actions UI

## Caching Strategy

CI uses multiple caching strategies to improve performance:

1. **Swatinem/rust-cache**: Automatic Rust dependency caching
2. **actions/cache**: Manual caching for specific tools (cargo-public-api, etc.)
3. **Toolchain hash**: Cache keys include Rust toolchain version for isolation

## Concurrency Control

CI uses concurrency groups to cancel outdated runs:
```yaml
concurrency:
  group: ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.sha }}
  cancel-in-progress: true
```

This ensures only the latest commit in a PR runs CI, saving resources.

## Timeout Configuration

Jobs have different timeouts based on platform and complexity:
- Fast checks: 10 minutes
- Standard checks: 30 minutes
- Windows builds: 45 minutes (longer due to platform overhead)

## Maintenance

### Adding New Quality Gates

1. Implement the check in `xtask/` crate first
2. Add to `cargo xtask validate` command
3. Test locally: `cargo xtask validate`
4. CI will automatically pick up the new check

### Updating Dependencies

1. Update `Cargo.toml` files
2. Run `cargo update`
3. Verify locally: `cargo xtask validate`
4. CI will verify on all platforms

### Modifying CI Jobs

1. Edit `.github/workflows/ci.yml`
2. Validate YAML syntax
3. Test with workflow_dispatch if possible
4. Monitor first run carefully

## References

- **Validation Pipeline**: See `.kiro/specs/project-infrastructure/design.md`
- **Requirements**: See `.kiro/specs/project-infrastructure/requirements.md` (INF-REQ-9)
- **xtask Commands**: See `xtask/src/main.rs`
