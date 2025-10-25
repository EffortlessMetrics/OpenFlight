# Requirements Document

## Introduction

This specification covers the final polish and cleanup work required to land the gated features implementation cleanly. The work focuses on eliminating warnings, preventing future API drift, breaking cyclic dependencies, and ensuring all opt-in targets compile correctly without affecting public APIs.

## Glossary

- **Gated Feature**: A Cargo feature flag that enables optional functionality (e.g., `ipc-bench`, `ofp1-tests`)
- **flight-ipc**: The IPC (Inter-Process Communication) crate containing benchmarks
- **flight-hid**: The HID (Human Interface Device) crate containing OFP1 protocol tests
- **Packed Field**: A struct field in a `#[repr(packed)]` struct that requires special handling to avoid undefined behavior
- **Cyclic Dependency**: A circular dependency between crates that prevents clean compilation
- **Public API**: The externally visible interface of a crate that users depend on

## Requirements

### Requirement 1

**User Story:** As a developer, I want the flight-ipc benchmarks to compile warning-free under all feature combinations, so that CI remains clean and maintainable.

#### Acceptance Criteria

1. WHEN the flight-ipc benchmarks are compiled with feature `ipc-bench`, THE build system SHALL produce zero unused import warnings
2. WHEN the flight-ipc benchmarks are compiled with features `ipc-bench` and `ipc-bench-serde`, THE build system SHALL produce zero unused import warnings
3. WHEN the JSON micro-bench scaffolding is enabled via `ipc-bench-serde`, THE benchmark code SHALL execute serde_json roundtrip tests on Device
4. WHEN the `ipc-bench-serde` feature is disabled, THE benchmark file SHALL compile with no-op placeholders
5. THE command `cargo bench -p flight-ipc --features ipc-bench --no-run` SHALL complete successfully with zero warnings

### Requirement 2

**User Story:** As a maintainer, I want to decide whether to keep the FlightClient::list_devices() shim or use existing RPCs, so that the public API surface remains intentional and minimal.

#### Acceptance Criteria

1. WHEN the FlightClient shim is retained, THE example code SHALL compile successfully using `FlightClient::list_devices()`
2. WHEN the FlightClient shim is removed, THE example code SHALL be updated to use an existing RPC method such as `get_service_info`
3. THE chosen approach SHALL be documented in the crate's changelog
4. THE public API surface SHALL not grow unintentionally

### Requirement 3

**User Story:** As a developer, I want the flight-hid emulator tests moved out of the flight-hid crate, so that cyclic dependencies are eliminated permanently.

#### Acceptance Criteria

1. WHEN emulator tests require both flight-hid and flight-virtual, THE tests SHALL reside in a separate location that creates a one-way dependency edge
2. THE flight-hid crate SHALL not have a cyclic dev-dependency on flight-virtual
3. WHEN the tests are moved, THE emulator tests SHALL be re-enabled without comment blocks
4. THE build system SHALL compile flight-hid without multi-version crate conflicts

### Requirement 4

**User Story:** As a developer, I want safe helper methods for modifying packed struct fields, so that future code cannot accidentally create undefined behavior through direct field references.

#### Acceptance Criteria

1. THE CapabilitiesReport struct SHALL provide a `set_cap_flag` method that safely modifies capability_flags
2. THE HealthStatusReport struct SHALL provide a `set_status_flag` method that safely modifies status_flags
3. WHEN test code needs to modify packed fields, THE test code SHALL use the safe helper methods instead of direct field access
4. THE helper methods SHALL follow the copy-modify-write-back pattern to avoid E0793 errors
5. THE implementation SHALL prevent future developers from taking references to packed fields

### Requirement 5

**User Story:** As a maintainer, I want to control whether Clone/Copy derives on public types are exposed or gated, so that public API changes are intentional and documented.

#### Acceptance Criteria

1. WHEN Clone/Copy derives are added for test convenience, THE derives SHALL be gated with `#[cfg_attr(test, derive(Clone, Copy))]` if public API stability is required
2. WHEN public derives are acceptable, THE changelog SHALL document the API addition
3. THE test code SHALL compile successfully with the chosen approach (gated derives, public derives, or test-only newtypes)
4. THE approach SHALL be consistent across all affected types in flight-hid

### Requirement 6

**User Story:** As a CI maintainer, I want smoke tests for gated features that run on-demand, so that optional targets remain buildable without slowing every PR.

#### Acceptance Criteria

1. THE default CI check SHALL run `cargo check --workspace` without gated features
2. WHEN gated feature smoke tests are triggered, THE CI SHALL run `cargo bench --no-run -p flight-ipc --features ipc-bench`
3. WHEN gated feature smoke tests are triggered, THE CI SHALL run `cargo test --no-run -p flight-hid --features ofp1-tests`
4. THE gated smoke tests SHALL run on a cron schedule or when touching relevant crates
5. THE gated smoke tests SHALL not block standard PR workflows
