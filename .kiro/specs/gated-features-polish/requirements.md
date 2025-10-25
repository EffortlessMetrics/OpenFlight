# Requirements Document

## Introduction

This specification covers the final polish and cleanup work required to land the gated features implementation cleanly. The work focuses on eliminating warnings, preventing future API drift, breaking cyclic dependencies, and ensuring all opt-in targets compile correctly without affecting public APIs.

## Global Assumptions

- **Toolchain**: Rust stable ≥ 1.75 (project MSRV) on CI
- **OS**: Builds must succeed on Windows and Linux; Windows-specific targets (e.g., SimConnect) are explicitly documented
- **Lint Policy**: For targeted scopes (benches/examples/tests), `RUSTFLAGS=-Dunused-imports` or `#![deny(unused_imports)]` turns acceptance criteria into machine-verifiable facts without burdening the whole workspace
- **Public API Guard**: CI uses `cargo public-api` or `cargo-semver-checks` to assert no unintentional public API deltas for stable crates

## Glossary

- **Gated Feature**: A Cargo feature flag that enables optional functionality (e.g., `ipc-bench`, `ofp1-tests`)
- **flight-ipc**: The IPC (Inter-Process Communication) crate containing benchmarks
- **flight-hid**: The HID (Human Interface Device) crate containing OFP1 protocol tests
- **Packed Field**: A struct field in a `#[repr(packed)]` struct that requires special handling to avoid undefined behavior
- **Cyclic Dependency**: A circular dependency between crates that prevents clean compilation
- **Public API**: The externally visible interface of a crate that users depend on
- **MSRV**: Minimum Supported Rust Version

## Requirements

### Requirement 1

**User Story:** As a developer, I want the flight-ipc benchmarks to compile warning-free under all feature combinations, so that CI remains clean and maintainable.

#### Acceptance Criteria

1. WHEN the flight-ipc benchmarks are compiled with feature `ipc-bench`, THE file `benches/ipc_benchmarks.rs` SHALL produce zero unused import warnings
2. THE enforcement SHALL use `#![deny(unused_imports)]` at the top of the benchmark file or `RUSTFLAGS="-Dunused-imports"` during compilation
3. WHEN the flight-ipc benchmarks are compiled with features `ipc-bench` and `ipc-bench-serde`, THE build system SHALL produce zero unused import warnings
4. WHEN the JSON micro-bench scaffolding is enabled via `ipc-bench-serde`, THE benchmark code SHALL execute serde_json roundtrip tests on Device
5. WHEN the `ipc-bench-serde` feature is disabled, THE benchmark file SHALL compile with no-op placeholders
6. THE imports for serde functionality SHALL be contained within `#[cfg(feature = "ipc-bench-serde")]` blocks to avoid false positives
7. THE command `RUSTFLAGS="-Dunused-imports" cargo bench -p flight-ipc --features ipc-bench --no-run` SHALL complete successfully
8. THE command `RUSTFLAGS="-Dunused-imports" cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run` SHALL complete successfully

### Requirement 2

**User Story:** As a maintainer, I want to decide whether to keep the FlightClient::list_devices() shim or use existing RPCs, so that the public API surface remains intentional and minimal.

#### Acceptance Criteria (Shim Kept Path)

1. WHEN the FlightClient shim is retained, THE example code SHALL compile successfully using `FlightClient::list_devices()` with `--features ipc-examples`
2. THE shim SHALL be gated behind the `ipc-examples` feature or marked `pub(crate)` to prevent unintended surface growth
3. THE changelog SHALL include an entry explaining the rationale: "example convenience; not a supported public API"
4. THE changelog SHALL confirm no semver promise for the shim

#### Acceptance Criteria (Shim Removed Path)

1. WHEN the FlightClient shim is removed, THE example code SHALL be updated to use an existing RPC method such as `get_service_info`
2. THE example SHALL compile successfully with `--features ipc-examples`

#### Acceptance Criteria (Both Paths)

1. THE CI job SHALL run `cargo public-api -p flight-ipc --diff-git main..HEAD` or `cargo-semver-checks` and pass
2. THE public API check SHALL verify no new public items exist unless explicitly accepted

### Requirement 3

**User Story:** As a developer, I want the flight-hid emulator tests moved out of the flight-hid crate, so that cyclic dependencies are eliminated permanently.

#### Acceptance Criteria

1. WHEN emulator tests require both flight-hid and flight-virtual, THE tests SHALL reside in either `crates/flight-virtual/tests/ofp1_integration.rs` (recommended) or a new `crates/flight-hid-integration-tests/` crate
2. THE chosen location SHALL create a one-way dependency edge
3. THE flight-hid crate SHALL not have a dev-dependency on flight-virtual
4. THE enforcement SHALL verify `cargo tree -p flight-hid --edges dev,normal | Select-String flight-virtual` prints nothing
5. WHEN the tests are moved, THE emulator tests SHALL be re-enabled without comment blocks
6. THE command `cargo test -p flight-virtual --tests` (or `cargo test -p flight-hid-integration-tests`) SHALL compile and run the emulator tests
7. THE build system SHALL compile flight-hid without multi-version crate conflicts
8. THE enforcement SHALL verify `cargo tree -p flight-hid | Select-String 'flight_hid v'` shows exactly one version

### Requirement 4

**User Story:** As a developer, I want safe helper methods for modifying packed struct fields, so that future code cannot accidentally create undefined behavior through direct field references.

#### Acceptance Criteria

1. THE CapabilitiesReport struct SHALL provide a `set_cap_flag(&mut self, flag: CapabilityFlags)` method marked with `#[inline]`
2. THE HealthStatusReport struct SHALL provide a `set_status_flag(&mut self, flag: StatusFlags)` method marked with `#[inline]`
3. THE helper methods SHALL follow the copy-modify-write-back pattern: copy field value, modify copy, write back to field
4. WHEN test code needs to modify packed fields, THE test code SHALL use the safe helper methods instead of direct field access
5. THE helper methods SHALL eliminate E0793 errors by avoiding references to packed fields
6. IF packed fields are currently `pub(crate)`, THE visibility SHALL remain `pub(crate)`
7. IF packed fields are public and visibility cannot change, THE fields SHALL have rustdoc notes linking to helper methods and discouraging direct mutation
8. THE implementation SHALL prevent future developers from taking references to packed fields

### Requirement 5

**User Story:** As a maintainer, I want to control whether Clone/Copy derives on public types are exposed or gated, so that public API changes are intentional and documented.

#### Acceptance Criteria

1. WHEN public API stability is required, THE derives SHALL be gated with `#[cfg_attr(test, derive(Clone, Copy))]` for same-crate tests
2. WHEN external tests in another crate need Clone/Copy, THE derives SHALL be gated with `#[cfg_attr(any(test, feature = "ofp1-tests"), derive(Clone, Copy))]`
3. THE gated feature SHALL not be part of default features
4. WHEN public derives are intentionally added, THE changelog SHALL document the API addition
5. THE public API check SHALL record the approved delta
6. THE test code SHALL compile successfully with the chosen approach (gated derives, public derives, or test-only newtypes)
7. THE chosen policy SHALL be applied consistently to all affected public types in flight-hid

### Requirement 6

**User Story:** As a CI maintainer, I want smoke tests for gated features that run on-demand, so that optional targets remain buildable without slowing every PR.

#### Acceptance Criteria

1. THE default CI check SHALL run `cargo check --workspace` without gated features
2. WHEN gated IPC smoke tests are triggered, THE CI SHALL run `RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features ipc-bench`
3. WHEN gated HID smoke tests are triggered, THE CI SHALL run `cargo test --no-run -p flight-hid --features ofp1-tests`
4. THE IPC smoke tests SHALL trigger on changes under `crates/flight-ipc/**`
5. THE HID smoke tests SHALL trigger on changes under `crates/flight-hid/**` or the integration tests crate path
6. THE gated smoke tests SHALL run on a cron schedule (e.g., nightly at 3 AM UTC)
7. THE gated smoke tests SHALL be triggerable via a PR label (e.g., `run-gated`)
8. THE gated smoke tests SHALL not block standard PR workflows


## Definition of Done

1. THE command `cargo check --workspace` SHALL complete successfully
2. THE command `RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features ipc-bench` SHALL complete successfully
3. THE command `cargo test --no-run -p flight-hid --features ofp1-tests` SHALL complete successfully
4. THE CI public API check (using `cargo public-api` or `cargo-semver-checks`) SHALL pass
5. THE touched crates SHALL pass `rustfmt` and `clippy -Dwarnings` for modified code
6. THE changelog SHALL be updated per Requirement 2 and Requirement 5 decisions
7. THE helper methods for packed fields SHALL be implemented and tests SHALL use them
8. THE emulator tests SHALL be relocated and re-enabled

## Open Decisions

### Decision 1: FlightClient::list_devices() Shim

**Options:**
- **Keep**: Convenient for examples but adds tiny surface growth; gate behind `ipc-examples` feature or mark `pub(crate)`
- **Drop**: Minimal surface; update example to call existing RPC like `get_service_info`

**Impact:** Affects public API surface and example code maintenance

### Decision 2: Clone/Copy Derives for HID Public Types

**Options:**
- **Gate for tests/features**: Use `#[cfg_attr(test, derive(Clone, Copy))]` or feature-gated derives; no public API change
- **Make public with changelog**: Add derives to public API and document in changelog as intentional addition

**Impact:** Affects public API stability and semver guarantees

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| "Zero warnings" creep from upstream dependencies | Scope denial to benches/test files via `#![deny(unused_imports)]` or `RUSTFLAGS` for specific invocations only |
| Public API drift via helpers/derives | Keep helpers `pub(crate)` where possible; gate derives; add public API check to CI |
| Test relocation friction | Choose Option A (move to `flight-virtual/tests/`) to avoid creating a new crate; update path filters in CI |
| Windows-specific build failures | Ensure CI runs on both Windows and Linux; document platform-specific requirements |

## Traceability

- **Requirement 1** → Bench imports trimmed, cfg'd serde block, deny unused-imports for benches
- **Requirement 2** → Either keep shim gated or migrate example; public API CI check + changelog
- **Requirement 3** → Move emulator tests (Option A or B), remove dev cycle, prove single flight_hid in graph
- **Requirement 4** → Add `set_cap_flag` / `set_status_flag`; tests updated
- **Requirement 5** → Derive strategy chosen + enforced + documented
- **Requirement 6** → CI jobs added (non-blocking) with path/cron triggers
