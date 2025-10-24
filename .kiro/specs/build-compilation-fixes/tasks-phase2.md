# Implementation Plan - Phase 2: Remaining Compilation Fixes

This task list addresses compilation errors discovered during verification (task 5.6) that were not covered in the initial implementation phase.

## Background

After completing tasks 1.1-5.6, `cargo check --workspace` revealed additional compilation errors in:
- **flight-hub-examples** package (examples with outdated APIs)
- **flight-simconnect** crate (missing serde derives, borrow conflicts, async safety)

These issues require fixes to bring the workspace to a fully compiling state.

---

## Tasks

- [ ] 1. Fix flight-hub-examples compilation errors
  - Update BlackboxWriter API usage to match current implementation
  - Fix BlackboxReader constructor calls
  - Add missing async runtime attributes
  - Remove references to missing flight_bus crate
  - _Requirements: BC-06, BC-09_

- [ ] 1.1 Fix BlackboxWriter API calls in examples
  - Update `start_recording()` calls to provide 3 required arguments (aircraft_id, session_id, metadata)
  - Fix `write_record()` calls to use current API
  - Update BlackboxRecord enum variant construction
  - _Requirements: BC-06.2, BC-06.3_

- [ ] 1.2 Fix BlackboxReader API calls in examples
  - Replace `BlackboxReader::new()` with `BlackboxReader::open()`
  - Update record iteration to match current API
  - Fix BlackboxStats field access (use records_written, bytes_written, etc.)
  - _Requirements: BC-06.2_

- [ ] 1.3 Add async runtime support to examples
  - Add `#[tokio::main]` attribute to async main functions
  - Ensure tokio dependency includes required features
  - _Requirements: BC-06.5_

- [ ] 1.4 Remove flight_bus dependencies from examples
  - Replace flight_bus types with appropriate alternatives
  - Update snapshot creation code
  - Fix gear position and autopilot state references
  - _Requirements: BC-06.1_

- [ ] 2. Fix flight-simconnect compilation errors
  - Add serde derives to SessionFixture
  - Resolve borrow checker conflicts in mapping setup
  - Fix async safety issues with Mutex usage
  - _Requirements: BC-03, BC-02_

- [ ] 2.1 Add serde support to SessionFixture
  - Add `#[derive(Serialize, Deserialize)]` to SessionFixture struct
  - Ensure all nested types also implement serde traits
  - _Requirements: BC-02.1, BC-02.2_

- [ ] 2.2 Fix borrow conflicts in mapping.rs
  - Refactor `setup_data_definitions` to avoid simultaneous immutable/mutable borrows
  - Use scoped borrows or clone data before mutable operations
  - Apply pattern from design document section 4
  - _Requirements: BC-03.4_

- [ ] 2.3 Fix async safety in adapter.rs
  - Replace `std::sync::Mutex` with `tokio::sync::Mutex` for event_receiver
  - Update lock() calls to use .await for async mutex
  - Ensure spawned tasks are Send-safe
  - _Requirements: BC-03.3_

- [ ] 3. Verify all fixes with comprehensive checks
  - Run `cargo check --workspace` on Windows
  - Run `cargo check --workspace` on Linux (if available)
  - Verify examples compile: `cargo check -p flight-hub-examples`
  - Test serde features: `cargo check -p flight-simconnect --features serde`
  - _Requirements: All BC requirements verification_

---

## Notes

- These tasks build on the completed Phase 1 tasks (1.1-5.6)
- Focus on bringing the workspace to a fully compiling state
- Examples may need significant refactoring if APIs have changed substantially
- Consider whether flight-hub-examples should be updated or deprecated
