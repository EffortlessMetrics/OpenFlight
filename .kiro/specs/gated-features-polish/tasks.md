# Implementation Plan

- [ ] 1. Clean up flight-ipc benchmarks for warning-free compilation
  - Add `#![deny(unused_imports)]` at the top of `crates/flight-ipc/benches/ipc_benchmarks.rs`
  - Remove unused imports (e.g., `ListDevicesRequest`) from the benchmark file
  - Add optional `serde_json` dependency to `crates/flight-ipc/Cargo.toml` with `[dependencies] serde_json = { version = "1", optional = true }`
  - Update feature definitions in `crates/flight-ipc/Cargo.toml` to include `ipc-bench = []` and `ipc-bench-serde = ["dep:serde_json"]`
  - Wrap serde-specific imports in `#[cfg(feature = "ipc-bench-serde")]` blocks
  - Wrap JSON roundtrip benchmark code in `#[cfg(feature = "ipc-bench-serde")]` block
  - Add no-op placeholder in `#[cfg(not(feature = "ipc-bench-serde"))]` block
  - Verify with `RUSTFLAGS="-Dunused-imports" cargo bench -p flight-ipc --features ipc-bench --no-run`
  - Verify with `RUSTFLAGS="-Dunused-imports" cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run`
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8_

- [ ] 2. Resolve FlightClient example decision and ensure public API stability
  - Review current `FlightClient::list_devices()` implementation in `crates/flight-ipc/src/client.rs`
  - **Decision Path A (Recommended):** Drop the shim and update example to use existing RPC like `get_service_info()`
  - **Decision Path B:** Keep shim as `pub(crate)` or move helper to example file only
  - Update `crates/flight-ipc/examples/client_example.rs` based on chosen path
  - Add changelog entry to `crates/flight-ipc/CHANGELOG.md` documenting the decision
  - Verify example compiles successfully
  - Verify public API check passes: `cargo public-api -p flight-ipc --diff-git origin/main..HEAD`
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

- [ ] 3. Relocate flight-hid emulator tests to break cyclic dependency
  - Identify emulator tests currently in `crates/flight-hid/` that depend on flight-virtual
  - **Decision Path A (Recommended):** Move tests to `crates/flight-virtual/tests/ofp1_integration.rs`
  - **Decision Path B:** Create new `crates/flight-hid-integration-tests/` crate with minimal structure
  - Remove dev-dependency on flight-virtual from `crates/flight-hid/Cargo.toml`
  - Add dev-dependency on flight-hid (with `ofp1-tests` feature) to flight-virtual or integration crate
  - Move test code and uncomment any previously commented test blocks
  - Verify no cycle: `cargo tree -p flight-hid --edges dev,normal | rg 'flight-virtual' -n` (should be empty)
  - Verify single version: `cargo tree -p flight-hid | rg '^flight_hid v' | wc -l` (should output 1)
  - Verify relocated tests compile and run: `cargo test -p flight-virtual --tests` or `cargo test -p flight-hid-integration-tests`
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8_

- [ ] 4. Implement safe helper methods for packed struct fields
  - [ ] 4.1 Add safe accessors to CapabilitiesReport
    - Locate `CapabilitiesReport` struct in `crates/flight-hid/src/protocol/ofp1.rs`
    - Add `#[inline] pub fn cap_flags(&self) -> CapabilityFlags` getter method
    - Add `#[inline] pub fn set_cap_flag(&mut self, flag: CapabilityFlags)` method using copy-modify-write-back pattern
    - Add `#[inline] pub fn clear_cap_flag(&mut self, flag: CapabilityFlags)` method using copy-modify-write-back pattern
    - Add rustdoc warnings if `capability_flags` field is public, with usage examples
    - _Requirements: 4.1, 4.3, 4.5, 4.7_
  
  - [ ] 4.2 Add safe accessors to HealthStatusReport
    - Locate `HealthStatusReport` struct in `crates/flight-hid/src/protocol/ofp1.rs`
    - Add `#[inline] pub fn status_flags(&self) -> StatusFlags` getter method
    - Add `#[inline] pub fn set_status_flag(&mut self, flag: StatusFlags)` method using copy-modify-write-back pattern
    - Add `#[inline] pub fn clear_status_flag(&mut self, flag: StatusFlags)` method using copy-modify-write-back pattern
    - Add rustdoc warnings if `status_flags` field is public, with usage examples
    - _Requirements: 4.2, 4.3, 4.5, 4.7_
  
  - [ ] 4.3 Update tests to use safe helper methods
    - Find all test code in `crates/flight-hid/tests/` that directly accesses packed fields
    - Replace direct field access (e.g., `report.capability_flags.set_flag()`) with helper calls (e.g., `report.set_cap_flag()`)
    - Verify tests compile without E0793 errors
    - Run tests to ensure functionality is preserved
    - _Requirements: 4.4, 4.5, 4.8_

- [ ] 5. Apply Clone/Copy derive strategy to flight-hid public types
  - Locate public packed structs in `crates/flight-hid/src/protocol/ofp1.rs` (CapabilitiesReport, HealthStatusReport, etc.)
  - **Decision Path A (Recommended):** Add `#[cfg_attr(any(test, feature = "ofp1-tests"), derive(Clone, Copy))]` to structs
  - **Decision Path B:** Add `#[derive(Clone, Copy)]` directly and document in changelog
  - Add `ofp1-tests = []` feature to `crates/flight-hid/Cargo.toml` (not in default features) if using Path A
  - Apply chosen strategy consistently to all affected public packed structs
  - Add changelog entry to `crates/flight-hid/CHANGELOG.md` if using Path B
  - Verify tests compile successfully
  - Verify public API check passes: `cargo public-api -p flight-hid --diff-git origin/main..HEAD`
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7_

- [ ] 6. Set up CI gated feature smoke tests
  - [ ] 6.1 Add public API guard job
    - Create or update `.github/workflows/ci.yml`
    - Add `public-api-check` job that runs on pull requests
    - Configure checkout with `fetch-depth: 0`
    - Install `cargo-public-api` tool
    - Add step to check flight-ipc: `cargo public-api -p flight-ipc --diff-git origin/main..HEAD`
    - Add step to check flight-hid: `cargo public-api -p flight-hid --diff-git origin/main..HEAD`
    - _Requirements: 6.1_
  
  - [ ] 6.2 Add path filter for conditional smoke tests
    - Add `path-filter` job using `dorny/paths-filter@v3` action
    - Configure filters for `crates/flight-ipc/**` and `crates/flight-hid/**`
    - Set up job outputs for `ipc` and `hid` paths
    - _Requirements: 6.4, 6.5_
  
  - [ ] 6.3 Add gated IPC smoke test job
    - Add `gated-ipc-smoke` job that depends on `path-filter`
    - Configure conditional execution: schedule, `run-gated` label, or IPC path changes
    - Add step: `RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features ipc-bench`
    - Add step: `RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features "ipc-bench,ipc-bench-serde"`
    - Add step: `cargo clippy -p flight-ipc --benches -- -Dwarnings`
    - _Requirements: 6.2, 6.4, 6.7_
  
  - [ ] 6.4 Add gated HID smoke test job
    - Add `gated-hid-smoke` job that depends on `path-filter`
    - Configure conditional execution: schedule, `run-gated` label, or HID path changes
    - Add step: `cargo test --no-run -p flight-hid --features ofp1-tests`
    - Add step: `cargo clippy -p flight-hid --tests -- -Dwarnings`
    - _Requirements: 6.3, 6.5, 6.7_
  
  - [ ] 6.5 Add scheduled cross-platform verification
    - Add `cross-platform` job with matrix for ubuntu-latest and windows-latest
    - Configure to run on cron schedule: `0 3 * * *` (3 AM UTC daily)
    - Add step: `cargo check --workspace`
    - Document platform-specific requirements (e.g., Windows-only SimConnect)
    - _Requirements: 6.6, 6.8_

- [ ] 7. Update documentation and finalize
  - [ ] 7.1 Update crate README files
    - Add feature flag documentation to `crates/flight-ipc/README.md`: `ipc-bench`, `ipc-bench-serde`
    - Add feature flag documentation to `crates/flight-hid/README.md`: `ofp1-tests`
    - Document that these are dev-only features
  
  - [ ] 7.2 Add Cargo.toml comments
    - Add inline comments in `crates/flight-ipc/Cargo.toml` explaining feature purposes
    - Add inline comments in `crates/flight-hid/Cargo.toml` explaining feature purposes
  
  - [ ] 7.3 Verify Definition of Done
    - Run `cargo check --workspace` and verify it passes
    - Run all verification commands from design document
    - Verify all clippy and fmt checks pass
    - Confirm changelogs are updated per decisions
    - Confirm all tests pass with relocated emulator tests
