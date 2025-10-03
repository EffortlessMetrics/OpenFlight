# Requirements Document

## Introduction

This feature addresses critical supply chain security issues and dependency management problems that are blocking the CI gates. The implementation focuses on resolving P0 licensing issues, unifying HTTP stack dependencies, updating major dependency versions, and ensuring compliance with security policies. These fixes are essential to unblock the build pipeline and maintain a secure, compliant dependency baseline.

## Requirements

### Requirement 1

**User Story:** As a developer, I want the cargo-deny license gate to pass, so that the CI pipeline can complete successfully and the project maintains license compliance.

#### Acceptance Criteria

1. WHEN cargo deny check licenses is executed THEN the system SHALL allow Unicode-3.0, Unicode-DFS-2016, and MPL-2.0 licenses
2. WHEN checking license compliance THEN the system SHALL include proper exceptions for unicode-ident crate
3. WHEN the license gate runs THEN the system SHALL return a successful exit code with no violations
4. WHEN examples crate is checked THEN the system SHALL have a valid license expression defined

### Requirement 2

**User Story:** As a developer, I want unified HTTP stack dependencies, so that there are no conflicting versions causing build issues and security vulnerabilities.

#### Acceptance Criteria

1. WHEN checking HTTP dependencies THEN the system SHALL use only hyper 1.x versions
2. WHEN reqwest is used THEN the system SHALL use version 0.12 with rustls-tls features
3. WHEN building the project THEN the system SHALL NOT include hyper 0.14 or native-tls dependencies
4. WHEN cargo tree -i hyper is executed THEN the system SHALL show only hyper 1.x versions

### Requirement 3

**User Story:** As a developer, I want updated gRPC dependencies, so that the project uses compatible versions with the unified HTTP stack.

#### Acceptance Criteria

1. WHEN using tonic THEN the system SHALL use version 0.14 or later
2. WHEN using prost THEN the system SHALL use version 0.14 or later
3. WHEN building gRPC services THEN the system SHALL compile without HTTP version conflicts
4. WHEN tonic-build generates code THEN the system SHALL use compatible prost codegen

### Requirement 4

**User Story:** As a developer, I want updated system-level dependencies, so that the project uses secure and compatible versions of nix and windows crates.

#### Acceptance Criteria

1. WHEN using nix crate THEN the system SHALL use version 0.30 with OwnedFd/BorrowedFd types
2. WHEN using windows crate THEN the system SHALL use version 0.62 with updated bindings
3. WHEN handling file descriptors on Linux THEN the system SHALL use typed FD APIs
4. WHEN using Windows APIs THEN the system SHALL compile without deprecated warnings

### Requirement 5

**User Story:** As a developer, I want the direct dependency count under control, so that the supply chain gate passes and the project maintains manageable dependencies.

#### Acceptance Criteria

1. WHEN counting direct dependencies THEN the system SHALL have ≤ 150 non-dev dependencies
2. WHEN checking for duplicate majors THEN the system SHALL use unified versions of axum, tower, hyper, thiserror, and syn
3. WHEN running the dependency gate THEN the system SHALL exclude dev-dependencies from the count
4. WHEN using heavy crates THEN the system SHALL use minimal feature sets

### Requirement 6

**User Story:** As a developer, I want complete third-party license documentation, so that the project meets legal compliance requirements.

#### Acceptance Criteria

1. WHEN generating license documentation THEN the system SHALL include full Unicode license text
2. WHEN generating license documentation THEN the system SHALL include full MPL-2.0 license text
3. WHEN THIRD_PARTY_LICENSES.md is generated THEN the system SHALL include all transitive dependency licenses
4. WHEN checking license completeness THEN the system SHALL show no missing license entries

### Requirement 7

**User Story:** As a developer, I want the CI supply chain gate logic fixed, so that it accurately reports pass/fail status based on actual violations.

#### Acceptance Criteria

1. WHEN cargo deny exits with non-zero code THEN the gate SHALL report FAIL status
2. WHEN parsing cargo deny output THEN the gate SHALL check stderr for "licenses FAILED" or "licenses ok"
3. WHEN encountering "license-not-encountered" warnings THEN the gate SHALL treat them as warnings, not failures
4. WHEN the gate completes THEN the gate SHALL accurately correlate violation counts with pass/fail status