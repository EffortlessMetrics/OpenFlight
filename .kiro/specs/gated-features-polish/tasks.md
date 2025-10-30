# Implementation Plan

- [x] 1. Clean up flight-ipc benchmarks for warning-free compilation










  - Remove unused imports (e.g., `ListDevicesRequest`) from `crates/flight-ipc/benches/ipc_benchmarks.rs`
  - Add `#![deny(unused_imports)]` at the top of `crates/flight-ipc/benches/ipc_benchmarks.rs` to enforce unused import checks at file level
  - Add optional `serde_json` dependency to `crates/flight-ipc/Cargo.toml`: `[dependencies] serde_json = { version = "1", optional = true }`
  - Update feature definitions in `crates/flight-ipc/Cargo.toml` to include `default = []`, `ipc-bench = []`, and `ipc-bench-serde = ["dep:serde_json"]`
  - Wrap serde-specific imports in `#[cfg(feature = "ipc-bench-serde")]` blocks
  - Check if proto-generated `Device` type has serde derives; if not, create `DeviceJson` mirror struct with serde derives and document as approximation
  - Wrap JSON roundtrip benchmark code in `#[cfg(feature = "ipc-bench-serde")]` block
  - Add no-op comment in `#[cfg(not(feature = "ipc-bench-serde"))]` block
  - Verify with `cargo bench -p flight-ipc --features ipc-bench --no-run`
  - Verify with `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run`
  - Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` to catch any remaining issues
  - Run `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings` to verify both feature combos
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8_

- [x] 2. Resolve FlightClient example decision and ensure public API stability





  - Review current `FlightClient::list_devices()` implementation in `crates/flight-ipc/src/client.rs`
  - **Decision Path A (Recommended):** Drop the shim entirely and update `crates/flight-ipc/examples/client_example.rs` to use existing RPC like `get_service_info()`
  - **Decision Path B:** Make shim `pub(crate)` in library OR move helper function into example file only (no library change)
  - Update example code based on chosen path
  - Add changelog entry to `crates/flight-ipc/CHANGELOG.md` documenting the decision and rationale
  - Verify example compiles: `cargo build -p flight-ipc --examples`
  - Verify public API check passes: `cargo public-api -p flight-ipc --diff-git origin/main..HEAD` (requires `fetch-depth: 0`)
  - Optionally run `cargo semver-checks -p flight-ipc` for type-level semver verification
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

- [x] 3. Relocate flight-hid emulator tests to break cyclic dependency





  - Identify emulator tests currently in `crates/flight-hid/` that depend on flight-virtual
  - **Decision Path A (Recommended):** Move tests to `crates/flight-virtual/tests/ofp1_integration.rs`
  - **Decision Path B:** Create new `crates/flight-hid-integration-tests/` crate with minimal structure
  - Remove dev-dependency on flight-virtual from `crates/flight-hid/Cargo.toml`
  - Add dev-dependency to `crates/flight-virtual/Cargo.toml`: `flight-hid = { path = "../flight-hid", features = ["ofp1-tests"] }`
  - Move test code and uncomment any previously commented test blocks
  - Verify no cycle: `cargo tree -p flight-hid --edges dev,normal | rg 'flight-virtual' -n` (should be empty, exit code 1)
  - Verify single version: `cargo tree -p flight-hid | rg '^flight_hid v' | wc -l` (should output 1)
  - Verify no duplicates: `cargo tree -p flight-hid -d` (should print "No duplicate packages found")
  - Verify relocated tests compile: `cargo test -p flight-virtual --tests --no-run` or `cargo test -p flight-hid-integration-tests --no-run`
  - Run relocated tests: `cargo test -p flight-virtual --tests` or `cargo test -p flight-hid-integration-tests`
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8_

- [x] 4. Implement safe helper methods for packed struct fields






  - [x] 4.1 Add safe accessors to CapabilitiesReport

    - Locate `CapabilitiesReport` struct in `crates/flight-hid/src/protocol/ofp1.rs`
    - Add `#[inline] pub fn cap_flags(&self) -> CapabilityFlags` getter method that returns a copy
    - Add `#[inline] pub fn set_cap_flag(&mut self, flag: CapabilityFlags)` method using copy-modify-write-back pattern
    - Add `#[inline] pub fn clear_cap_flag(&mut self, flag: CapabilityFlags)` method using copy-modify-write-back pattern
    - If `capability_flags` field is public, add loud rustdoc warning with links to helper methods and usage examples
    - Consider creating a macro if other packed structs need similar helpers for consistency
    - _Requirements: 4.1, 4.3, 4.5, 4.6, 4.7_
  

  - [x] 4.2 Add safe accessors to HealthStatusReport

    - Locate `HealthStatusReport` struct in `crates/flight-hid/src/protocol/ofp1.rs`
    - Add `#[inline] pub fn status_flags(&self) -> StatusFlags` getter method that returns a copy
    - Add `#[inline] pub fn set_status_flag(&mut self, flag: StatusFlags)` method using copy-modify-write-back pattern
    - Add `#[inline] pub fn clear_status_flag(&mut self, flag: StatusFlags)` method using copy-modify-write-back pattern
    - If `status_flags` field is public, add loud rustdoc warning with links to helper methods and usage examples
    - _Requirements: 4.2, 4.3, 4.5, 4.6, 4.7_
  

  - [x] 4.3 Update tests to use safe helper methods

    - Search for direct packed field access in `crates/flight-hid/tests/` and `crates/flight-hid/src/` test modules
    - Replace direct field access (e.g., `report.capability_flags.set_flag()`) with helper calls (e.g., `report.set_cap_flag()`)
    - Verify no direct references remain: `rg '\.capability_flags\.' crates/flight-hid/tests/` and `rg '\.status_flags\.' crates/flight-hid/tests/` (should be empty)
    - Verify tests compile without E0793 errors: `cargo test -p flight-hid --no-run`
    - Run tests to ensure functionality is preserved: `cargo test -p flight-hid`
    - _Requirements: 4.4, 4.5, 4.8_

- [x] 5. Apply Clone/Copy derive strategy to flight-hid public types





  - Locate public packed structs in `crates/flight-hid/src/protocol/ofp1.rs` (CapabilitiesReport, HealthStatusReport, etc.)
  - **Decision Path A (Recommended):** Add `#[cfg_attr(any(test, feature = "ofp1-tests"), derive(Clone, Copy))]` to all affected structs
  - **Decision Path B:** Add `#[derive(Clone, Copy)]` directly and document in changelog as intentional public API addition
  - If using Path A, add `ofp1-tests = []` feature to `crates/flight-hid/Cargo.toml` under `[features]` (not in default features)
  - If using Path A, ensure external test crate enables the feature: `flight-hid = { path = "../flight-hid", features = ["ofp1-tests"] }`
  - Apply chosen strategy consistently to all affected public packed structs
  - If using Path B, add changelog entry to `crates/flight-hid/CHANGELOG.md` documenting the API addition
  - Verify tests compile: `cargo test -p flight-hid --no-run` and `cargo test -p flight-virtual --tests --no-run`
  - Verify public API check passes: `cargo public-api -p flight-hid --diff-git origin/main..HEAD`
  - Confirm tests still use helper methods (Copy doesn't fix packed reference UB)
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7_

- [x] 6. Set up CI gated feature smoke tests




  - [x] 6.1 Add public API guard job


    - Create or update `.github/workflows/ci.yml`
    - Add `public-api-check` job that runs on pull requests
    - Configure checkout with `fetch-depth: 0`
    - Install `cargo-public-api` tool
    - Add step to check flight-ipc: `cargo public-api -p flight-ipc --diff-git origin/main..HEAD`
    - Add step to check flight-hid: `cargo public-api -p flight-hid --diff-git origin/main..HEAD`
    - _Requirements: 6.1_
  
  - [x] 6.2 Add path filter for conditional smoke tests


    - Add `path-filter` job using `dorny/paths-filter@v3` action that runs on pull requests
    - Configure filters for `crates/flight-ipc/**` and `crates/flight-hid/**` paths
    - Set up job outputs for `ipc` and `hid` that downstream jobs can reference
    - Ensure downstream jobs use `needs: path-filter` and check outputs in `if:` conditions
    - _Requirements: 6.4, 6.5_
  
-

  - [x] 6.3 Add gated IPC smoke test job




    - Add `gated-ipc-smoke` job that depends on `path-filter` with `needs: path-filter`
    - Configure conditional execution in `if:` clause: `github.event_name == 'schedule'` OR `contains(github.event.pull_request.labels.*.name, 'run-gated')` OR `needs.path-filter.outputs.ipc == 'true'`
    - Add step: `cargo bench --no-run -p flight-ipc --features ipc-bench`
    - Add step: `cargo bench --no-run -p flight-ipc --features "ipc-bench,ipc-bench-serde"`
    - Add step: `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings`
    - Add step: `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings`
    - Optionally add `workflow_dispatch` trigger with input flag for manual runs
    - _Requirements: 6.2, 6.4, 6.7_
  
  - [x] 6.4 Add gated HID smoke test job


    - Add `gated-hid-smoke` job that depends on `path-filter` with `needs: path-filter`
    - Configure conditional execution in `if:` clause: `github.event_name == 'schedule'` OR `contains(github.event.pull_request.labels.*.name, 'run-gated')` OR `needs.path-filter.outputs.hid == 'true'`
    - Add step: `cargo test --no-run -p flight-hid --features ofp1-tests`
    - Add step: `cargo clippy -p flight-hid --tests -- -Dwarnings`
    - _Requirements: 6.3, 6.5, 6.7_
  
  - [x] 6.5 Add scheduled cross-platform verification


    - Add `cross-platform` job with matrix for ubuntu-latest and windows-latest
    - Configure to run on cron schedule: `0 3 * * *` (3 AM UTC daily)
    - Add step: `cargo check --workspace` (or `--workspace --all-targets` if appropriate)
    - Guard Windows-only crates (e.g., SimConnect) with `cfg(windows)` in Cargo.toml or use feature guards so Linux CI doesn't fail
    - Document platform-specific requirements in README or CI documentation
    - Optionally add `cargo clippy --workspace --all-targets -- -Dwarnings` for strict baseline
    - Optionally add `cargo fmt --all -- --check` to verify formatting
    - _Requirements: 6.6, 6.8_

- [-] 7. Update documentation and finalize






  - [x] 7.1 Update crate README files

    - Add feature flag documentation to `crates/flight-ipc/README.md`: `ipc-bench`, `ipc-bench-serde`
    - Add feature flag documentation to `crates/flight-hid/README.md`: `ofp1-tests`
    - Document that these are dev-only features
  

  - [x] 7.2 Add Cargo.toml comments

    - Add inline comments in `crates/flight-ipc/Cargo.toml` explaining feature purposes
    - Add inline comments in `crates/flight-hid/Cargo.toml` explaining feature purposes
  

  - [ ] 7.3 Verify Definition of Done






    - **Build Verification:** Run `cargo check --workspace` on stable (and MSRV 1.75+ if applicable) - must pass
    - **IPC Benches (build):** Run `cargo bench -p flight-ipc --features ipc-bench --no-run` - must pass
    - **IPC Benches with Serde (build):** Run `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run` - must pass
    - **IPC Benches (lint):** Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` - must pass
    - **IPC Benches with Serde (lint):** Run `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings` - must pass
    - **HID Tests:** Run `cargo test --no-run -p flight-hid --features ofp1-tests` - must pass
    - **Relocated Tests:** Run `cargo test -p flight-virtual --tests` (or integration crate) - must pass
    - **No Cycles:** Run `cargo tree -p flight-hid --edges dev,normal | rg 'flight-virtual' -n` - must be empty (exit 1)
    - **Single Version:** Run `cargo tree -p flight-hid | rg '^flight_hid v' | wc -l` - must output 1
    - **No Duplicates:** Run `cargo tree -p flight-hid -d` - must print "No duplicate packages found"
    - **Helpers Used:** Run `rg '\.capability_flags\.' crates/flight-hid/tests/` and `rg '\.status_flags\.' crates/flight-hid/tests/` - must be empty
    - **Public API Guard:** Run `cargo public-api -p flight-ipc --diff-git origin/main..HEAD` and `cargo public-api -p flight-hid --diff-git origin/main..HEAD` - must pass
    - **Clippy:** Run `cargo clippy -p flight-ipc --benches -- -Dwarnings` and `cargo clippy -p flight-hid --tests -- -Dwarnings` - must pass
    - **Format:** Run `cargo fmt --check -p flight-ipc -p flight-hid` - must pass
    - **Changelogs:** Verify `crates/flight-ipc/CHANGELOG.md` and `crates/flight-hid/CHANGELOG.md` are updated per decisions
    - **Documentation:** Verify README files document new features (`ipc-bench`, `ipc-bench-serde`, `ofp1-tests`)
    - **Note:** File-level `#![deny(unused_imports)]` in `crates/flight-ipc/benches/ipc_benchmarks.rs` ensures unused import enforcement without workspace-wide RUSTFLAGS
