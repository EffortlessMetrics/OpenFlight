# Requirements Document

## Introduction

This feature addresses critical supply chain security issues and dependency management problems that are blocking the CI gates. The implementation focuses on resolving P0 licensing issues, unifying HTTP stack dependencies, updating major dependency versions, and ensuring compliance with security policies. These fixes are essential to unblock the build pipeline and maintain a secure, compliant dependency baseline.

## Definitions

**Direct dependency (non-dev):** A dependency in [dependencies] of any publishable workspace crate; excludes [dev-dependencies] and [build-dependencies]. Count via: `cargo tree -e normal --no-dev-dependencies -p <crate>` summed across publishable crates once (dedupe by package name).

**HTTP unification scope:** The whole workspace dependency graph must not contain hyper 0.14.x, hyper-tls, or native-tls.

**License gate result:** `cargo deny check licenses` must exit 0 and print "licenses ok"; "license-not-encountered" is a warning.

**Examples crate:** examples is treated as a crate; it must have a valid license expression and publish = false.

**Tooling versions:** Use cargo-deny ≥ 0.14, cargo-about ≥ 0.6 for stable CI behavior.

## Requirements

### Requirement SC-01

**User Story:** As a developer, I want the cargo-deny license gate to pass, so that the CI pipeline can complete successfully and the project maintains license compliance.

#### Acceptance Criteria

1. WHEN cargo deny check licenses is executed THEN the system SHALL allow Unicode-3.0, Unicode-DFS-2016, and MPL-2.0 licenses
2. WHEN checking license compliance THEN the system SHALL include proper exceptions for unicode-ident crate
3. WHEN the license gate runs THEN the system SHALL return a successful exit code with no violations
4. WHEN examples crate is checked THEN the system SHALL have a valid license expression defined
5. WHEN cargo deny reports license-not-encountered warnings THEN the gate SHALL not fail
6. WHEN the license policy runs THEN the deny.toml used SHALL be the repo's checked-in version (no per-developer overrides)

### Requirement SC-02

**User Story:** As a developer, I want unified HTTP stack dependencies, so that there are no conflicting versions causing build conflicts and TLS surface expansion.

#### Acceptance Criteria

1. WHEN checking HTTP dependencies THEN the system SHALL use only hyper 1.x versions
2. WHEN reqwest is used THEN the system SHALL use version 0.12 with rustls-tls features
3. WHEN building the project THEN the system SHALL NOT include hyper 0.14 or native-tls dependencies
4. WHEN cargo tree -i hyper is executed THEN the system SHALL show only hyper 1.x versions
5. WHEN cargo tree -i native-tls and cargo tree -i hyper-tls are executed THEN the system SHALL print no results
6. WHEN cargo tree -i openssl is executed THEN the system SHALL print no results

### Requirement SC-03

**User Story:** As a developer, I want updated gRPC dependencies, so that the project uses compatible versions with the unified HTTP stack.

#### Acceptance Criteria

1. WHEN using tonic THEN the system SHALL use version 0.14 or later
2. WHEN using prost THEN the system SHALL use version 0.14 or later
3. WHEN building gRPC services THEN the system SHALL compile without HTTP version conflicts
4. WHEN tonic-build generates code THEN the system SHALL use compatible prost codegen
5. WHEN building gRPC THEN prost, prost-build, tonic, and tonic-build SHALL be the same major/minor line (0.14.x)
6. WHEN custom transports are used THEN tonic SHALL set default-features = false with features = ["codegen","prost"], and the build SHALL succeed

### Requirement SC-04

**User Story:** As a developer, I want updated system-level dependencies, so that the project uses secure and compatible versions of nix and windows crates.

#### Acceptance Criteria

1. WHEN using nix crate THEN the system SHALL use version 0.30 with OwnedFd/BorrowedFd types
2. WHEN using windows crate THEN the system SHALL use version 0.62 with updated bindings
3. WHEN handling file descriptors on Linux THEN the system SHALL use typed FD APIs
4. WHEN compiling on Linux and Windows THEN the build SHALL complete without deprecated or unused warnings for public API crates (treat warnings at RUSTFLAGS="-Dwarnings" in CI for those crates)
5. WHEN Linux FD APIs are used THEN OwnedFd/BorrowedFd/AsFd SHALL be used instead of raw FDs in public functions

### Requirement SC-05

**User Story:** As a developer, I want the direct dependency count under control, so that the supply chain gate passes and the project maintains manageable dependencies.

#### Acceptance Criteria

1. WHEN counting direct dependencies THEN the system SHALL have ≤ 150 non-dev dependencies
2. WHEN checking for duplicate majors THEN the system SHALL use unified versions of axum, tower, hyper, thiserror, and syn
3. WHEN running the dependency gate THEN the system SHALL exclude dev-dependencies and build-dependencies from the count
4. WHEN using heavy crates THEN the system SHALL use minimal feature sets
5. WHEN checking duplicate majors THEN cargo tree -d | rg -E "(axum|tower|hyper|thiserror|syn)" SHALL show at most one major per family across the workspace

### Requirement SC-06

**User Story:** As a developer, I want complete third-party license documentation, so that the project meets legal compliance requirements.

#### Acceptance Criteria

1. WHEN generating license documentation THEN the system SHALL include full Unicode license text
2. WHEN generating license documentation THEN the system SHALL include full MPL-2.0 license text
3. WHEN THIRD_PARTY_LICENSES.md is generated THEN the system SHALL include all transitive dependency licenses
4. WHEN checking license completeness THEN the system SHALL show no missing license entries
5. WHEN generating THIRD_PARTY_LICENSES.md THEN it SHALL be produced by cargo-about using a checked-in about.hjson and include Unicode & MPL full texts
6. WHEN license docs are generated THEN the result SHALL be deterministic (same input lockfile → same output) and checked into the repo

### Requirement SC-07

**User Story:** As a developer, I want the CI supply chain gate logic fixed, so that it accurately reports pass/fail status based on actual violations.

#### Acceptance Criteria

1. WHEN cargo deny exits with non-zero code THEN the gate SHALL report FAIL status
2. WHEN parsing cargo deny output THEN the gate SHALL check stderr for "licenses FAILED" or "licenses ok"
3. WHEN encountering "license-not-encountered" warnings THEN the gate SHALL treat them as warnings, not failures
4. WHEN the gate completes THEN the gate SHALL accurately correlate violation counts with pass/fail status
5. WHEN the gate runs THEN it SHALL attach cargo-deny and cargo-about outputs as CI artifacts for audit trail

## Non-Functional Requirements

**NFR-A (Determinism):** The supply-chain outputs (SBOM/SPDX/3P licenses) SHALL be reproducible given the same Cargo.lock.

**NFR-B (MSRV/Edition discipline):** CI SHALL enforce edition = "2024" and rust-version = "1.92.0" across workspace packages.

**NFR-C (Security posture):** Registry sources SHALL be limited to crates.io (no git sources) unless a task-specific exception exists in deny.toml.