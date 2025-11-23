# CI Configuration

This directory documents the Continuous Integration (CI) pipeline for the Flight Hub project.

## Overview

The CI pipeline enforces quality gates to ensure code quality, correctness, and maintainability. All checks must pass before code can be merged to the main branch.

## CI Jobs and Their Purpose

### 1. Validate Job (`validate`)
**Purpose**: Unified quality gate that runs all validation checks via `cargo xtask validate`

**What it does**:
- Schema validation (spec ledger, front matter, infrastructure invariants)
- Cross-reference checking (docs ↔ ledger ↔ tests ↔ Gherkin)
- Code formatting verification (`cargo fmt --check`)
- Linting for core crates (`cargo clippy` with `-D warnings`)
- Unit tests for core crates
- Public API stability verification (if `cargo-public-api` is installed)
- Generates validation report artifact

**Runs on**: All pushes and pull requests

**Artifacts**:
- `docs/validation_report.md` - Comprehensive validation results

### 2. Test Suite (`test`)
**Purpose**: Run comprehensive test suite across platforms and Rust versions

**What it does**:
- Formatting checks
- Clippy linting (general and strict for core crates)
- File descriptor safety tests
- Critical pattern verification
- Unit tests and doc tests
- ADR link validation

**Runs on**: Ubuntu and Windows, with stable and MSRV (1.89.0)

### 3. MSRV Check (`msrv-check`)
**Purpose**: Verify compatibility with Minimum Supported Rust Version

**What it does**:
- Build all crates with Rust 1.89.0
- Run clippy with MSRV

**Runs on**: Ubuntu

### 4. Clippy Core (`clippy-core`)
**Purpose**: Strict linting for flight-core crate

**What it does**:
- Run clippy with `-D warnings` on flight-core
- Conditional execution based on path filters

**Runs on**: Ubuntu and Windows

### 5. Clippy IPC Benches (`clippy-ipc-benches`)
**Purpose**: Lint IPC benchmarks with optional unblock mode

**What it does**:
- Strict mode: Clippy with dependencies
- Unblock mode: Clippy without dependencies (requires `clippy-unblock` label)

**Runs on**: Ubuntu and Windows

### 6. Public API Guard (`public-api-check`)
**Purpose**: Prevent unintended public API changes

**What it does**:
- Compare public API against main branch
- Check flight-core, flight-ipc, and flight-hid
- Retry with nightly if stable fails

**Runs on**: Pull requests only

### 7. Gated Tests (`gated-ipc-smoke`, `gated-hid-smoke`)
**Purpose**: Run feature-gated tests that require special setup

**What it does**:
- Smoke test IPC benchmarks
- Smoke test HID with ofp1-tests feature

**Runs on**: Scheduled runs, manual dispatch, or with `run-gated` label

### 8. Cross-Platform Verification (`cross-platform`)
**Purpose**: Verify workspace builds on all platforms

**What it does**:
- Check entire workspace
- Run clippy with strict warnings
- Verify formatting

**Runs on**: Scheduled runs (daily at 3 AM UTC)

### 9. Security Audit (`security`)
**Purpose**: Check for security vulnerabilities and supply chain issues

**What it does**:
- Run `cargo audit` for known vulnerabilities
- Run `cargo deny` for license and dependency policy
- Validate HTTP stack unification (no native-tls, openssl, hyper-tls)

**Runs on**: All pushes and pull requests

### 10. Build Release (`build`)
**Purpose**: Verify release builds succeed

**What it does**:
- Build workspace in release mode
- Upload artifacts for flightd and flightctl

**Runs on**: Ubuntu and Windows

### 11. Feature Powerset Testing (`feature-powerset`)
**Purpose**: Test feature combinations for compatibility

**What it does**:
- Run `cargo hack` with feature powerset (depth 2)
- Verify workspace dependency alignment

**Runs on**: All pushes and pull requests

### 12. Performance Tests (`performance`)
**Purpose**: Run benchmarks and check for regressions

**What it does**:
- Run all workspace benchmarks
- Performance regression detection (TODO)

**Runs on**: All pushes and pull requests

## Quality Gates Enforced

The following quality gates MUST pass for code to be merged:

### 1. Code Formatting
- **Tool**: `cargo fmt --all -- --check`
- **Standard**: Rust 2024 edition formatting rules
- **Enforcement**: All jobs that run formatting checks

### 2. Linting (Clippy)
- **Tool**: `cargo clippy`
- **Standard**: 
  - Core crates: `-D warnings` (no warnings allowed)
  - Other crates: Warnings allowed for development ergonomics
- **Core crates**: flight-core, flight-axis, flight-bus, flight-hid, flight-ipc, flight-service, flight-simconnect, flight-panels
- **Enforcement**: `test`, `clippy-core`, `clippy-ipc-benches` jobs

### 3. Unit Tests
- **Tool**: `cargo test`
- **Coverage**: All workspace crates
- **Enforcement**: `test` job

### 4. API Stability
- **Tool**: `cargo-public-api`
- **Standard**: No breaking changes to public APIs without review
- **Crates monitored**: flight-core, flight-ipc, flight-hid
- **Enforcement**: `public-api-check` job (PR only)

### 5. Schema Validation
- **Tool**: `cargo xtask validate`
- **Standard**: All YAML/JSON files must conform to schemas
- **Files checked**:
  - `specs/spec_ledger.yaml` → `schemas/spec_ledger.schema.json`
  - `docs/**/*.md` front matter → `schemas/front_matter.schema.json`
  - `infra/**/invariants.yaml` → `schemas/invariants.schema.json`
- **Enforcement**: `validate` job

### 6. Cross-Reference Integrity
- **Tool**: `cargo xtask validate`
- **Standard**: All requirement links, test references, and Gherkin tags must be valid
- **Checks**:
  - Documentation → spec ledger (requirement links)
  - Spec ledger → codebase (test references)
  - Gherkin → spec ledger (tags)
- **Enforcement**: `validate` job

### 7. Security
- **Tools**: `cargo audit`, `cargo deny`
- **Standard**: No known vulnerabilities, compliant licenses
- **Enforcement**: `security` job

### 8. MSRV Compatibility
- **Tool**: Build and clippy with Rust 1.89.0
- **Standard**: Code must compile and pass clippy on MSRV
- **Enforcement**: `msrv-check` job

## Guarantees

When CI passes, the following guarantees are provided:

1. **Code Quality**: All code is formatted, linted, and passes strict clippy checks for core crates
2. **Correctness**: All unit tests pass on both Ubuntu and Windows
3. **API Stability**: Public APIs have not changed unexpectedly (for PRs)
4. **Documentation Integrity**: All documentation links and cross-references are valid
5. **Schema Compliance**: All structured data conforms to defined schemas
6. **Security**: No known vulnerabilities in dependencies
7. **MSRV Compatibility**: Code compiles and works on Rust 1.89.0
8. **Cross-Platform**: Code builds successfully on Linux and Windows

## Running CI Checks Locally

### Quick Check (Fast)
```bash
cargo xtask check
```
Runs: formatting, clippy for core crates, unit tests for core crates (~30 seconds)

### Full Validation (Comprehensive)
```bash
cargo xtask validate
```
Runs: all checks from `cargo xtask check` plus:
- Schema validation
- Cross-reference checking
- Public API verification
- Benchmark compilation
- Report generation (~5 minutes)

### Individual Checks
```bash
# Formatting
cargo fmt --all -- --check

# Clippy (core crates)
cargo clippy -p flight-core -p flight-virtual -p flight-hid -p flight-ipc -- -D warnings

# Tests (core crates)
cargo test -p flight-core -p flight-virtual -p flight-hid -p flight-ipc

# Security audit
cargo audit
cargo deny check

# Public API check
cargo public-api -p flight-core --diff-git origin/main..HEAD
```

## CI Configuration Files

- **Workflow definition**: `.github/workflows/ci.yml`
- **Clippy configuration**: `clippy.toml`
- **Cargo deny configuration**: `deny.toml`
- **Invariants**: `infra/local/invariants.yaml`, `infra/ci/invariants.yaml` (if exists)

## Troubleshooting

### CI Fails on Formatting
**Solution**: Run `cargo fmt --all` locally and commit the changes

### CI Fails on Clippy
**Solution**: Run `cargo clippy -p <crate> -- -D warnings` locally and fix warnings

### CI Fails on Tests
**Solution**: Run `cargo test -p <crate>` locally and fix failing tests

### CI Fails on Schema Validation
**Solution**: Run `cargo xtask validate` locally to see detailed error messages with file paths and line numbers

### CI Fails on Cross-References
**Solution**: Run `cargo xtask validate` locally and check `docs/validation_report.md` for broken links

### CI Fails on Public API Check
**Solution**: Review API changes with `cargo public-api -p <crate> --diff-git origin/main..HEAD`
- If changes are intentional, document them in the PR description
- If changes are unintentional, refactor to avoid breaking changes

## Adding New CI Checks

When adding new CI checks:

1. **Implement in xtask first**: Add the check to the xtask framework
2. **Test locally**: Verify the check works with `cargo xtask <command>`
3. **Update CI workflow**: Add or modify jobs in `.github/workflows/ci.yml`
4. **Document here**: Update this README with the new check's purpose and guarantees
5. **Update validation report**: Ensure new checks appear in `docs/validation_report.md`

## CI Performance

Target execution times:
- `validate` job: < 10 minutes
- `test` job: < 10 minutes (Ubuntu), < 20 minutes (Windows)
- `clippy-core` job: < 30 minutes
- `security` job: < 30 minutes
- Total pipeline: < 45 minutes

## Related Documentation

- [xtask Commands](../../xtask/README.md) (if exists)
- [Validation Report](../../docs/validation_report.md) (generated)
- [Feature Status](../../docs/feature_status.md) (generated)
- [Infrastructure Requirements](../../.kiro/specs/project-infrastructure/requirements.md)
- [Infrastructure Design](../../.kiro/specs/project-infrastructure/design.md)
