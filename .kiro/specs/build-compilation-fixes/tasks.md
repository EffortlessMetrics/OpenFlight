# Implementation Plan

- [ ] 1. Phase 1: Core API Stabilization
  - Fix critical API changes that block compilation across multiple crates
  - Implement serde feature infrastructure for serialization
  - Resolve async recursion issues in flight-updater
  - _Requirements: BC-01, BC-02_

- [ ] 1.1 Fix AxisEngine API signature and EngineConfig fields
  - Update EngineConfig struct to include missing fields: conflict_detector_config and enable_conflict_detection
  - Change AxisEngine::with_config to accept (name: String, config: EngineConfig) signature
  - Update all call sites in examples, tests, and benchmarks to use new API
  - Verify: `cargo build -p flight-axis --examples --tests --benches` passes
  - Verify: `git grep -n "Engine::new(" | wc -l` shows 2 arguments in all call sites
  - _Requirements: BC-01.1, BC-01.2, BC-01.3_

- [ ] 1.2 Fix Profile API rename in flight-core
  - Change Profile::merge calls to Profile::merge_with in aircraft_switch.rs
  - Verify argument order matches merge_with expectations (check if self-first or parameter order changed)
  - Ensure no remaining Profile::merge calls exist in codebase
  - Verify: `cargo check -p flight-core` passes
  - Verify: `git grep -n "Profile::merge(" | wc -l` returns 0
  - _Requirements: BC-01.4_

- [ ] 1.3 Resolve async recursion in flight-updater
  - **Preferred approach**: Convert recursive async functions to iterative loops (walk_updates, delta, rollback functions)
  - **Alternative**: Add async-recursion = "1" dependency and annotate specific functions if loop conversion not feasible
  - Fix duplicate function names in packaging.rs (include_integration_docs_for_sim vs include_integration_docs_for_b2)
  - Verify: `cargo check -p flight-updater` passes
  - Verify: No "recursion in an async fn requires boxing" errors
  - _Requirements: BC-07.6_

- [ ] 1.4 Implement serde feature infrastructure
  - Ensure workspace uses resolver = "2" to prevent feature leakage
  - Add serde feature to flight-axis and flight-simconnect Cargo.toml with optional = true
  - Add conditional derive macros: `#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]`
  - Update consumer crates (flight-replay) to enable serde features: `features = ["serde"]`
  - Gate all serialized types including nested structs/enums
  - Verify: `cargo check -p flight-axis --features serde` passes
  - Verify: `cargo check -p flight-replay` compiles with serde features enabled
  - _Requirements: BC-02.1, BC-02.2, BC-02.3, BC-02.4, BC-02.5_

- [ ] 2. Phase 2: Platform Compatibility
  - Add Windows-specific dependencies and fix platform-specific code
  - Implement error conversion infrastructure
  - Fix borrow checker conflicts in mapping code
  - _Requirements: BC-03, BC-04_

- [ ] 2.1 Add Windows dependencies and fix imports
  - Add windows crate with target-specific dependency: `[target.'cfg(windows)'.dependencies]`
  - Add futures = "0.3" as normal dependency
  - Fix SIMCONNECT_RECV_ID import from flight-simconnect-sys
  - Verify: `cargo build -p flight-simconnect` passes on Windows CI
  - Verify: `cargo tree -e features -p flight-simconnect | grep windows` empty on Linux
  - _Requirements: BC-03.1, BC-03.2, BC-03.6, BC-03.7_

- [ ] 2.2 Implement error conversion infrastructure
  - Create MappingError enum with `#[derive(thiserror::Error, Debug)]`
  - Implement `From<BusTypeError> for MappingError` with `#[from]` attribute
  - Implement `From<flight_simconnect_sys::SimConnectError> for MappingError`
  - Add `From<crate::transport::TransportError>` (note: crate::transport:: not crate::)
  - Update function signatures to return `Result<T, MappingError>` where ? bubbles these types
  - Verify: `cargo check -p flight-simconnect` passes without ? conversion errors
  - _Requirements: BC-03.5_

- [ ] 2.3 Fix async/sync mutex usage patterns
  - Identify std::sync::Mutex vs tokio::sync::Mutex usage
  - Remove .await from std::sync::Mutex.lock() calls
  - Ensure tokio::sync::Mutex.lock().await usage is correct
  - _Requirements: BC-03.3_

- [ ] 2.4 Fix borrow checker conflicts in mapping.rs
  - Implement scoped borrow pattern: `let keys = { self.subs.iter().map(...).collect() };`
  - Clone/extract keys or values into locals before mutation to end immutable borrow scope
  - Refactor subscription management around lines 257-279 to avoid conflicts
  - Pattern: `let k = self.key.clone(); /* immut borrow ends */ self.map.insert(k, v);`
  - Verify: `cargo check -p flight-simconnect` passes without borrow conflicts
  - _Requirements: BC-03.4_

- [ ] 2.5 Implement platform-specific code gates
  - Add `#[cfg(unix)] use std::os::fd::*;` for Unix-specific imports
  - Add `#[cfg(windows)] use std::os::windows::io::*;` for Windows-specific imports
  - Gate fd_safety_tests modules with `#[cfg(unix)]` in flight-hid and flight-ipc
  - Add Windows equivalent test modules with `#[cfg(windows)]` where needed
  - Verify: `cargo check --workspace` passes on both Windows and Linux CI
  - Verify: `git grep -n "std::os::fd" | grep -v "#\[cfg(unix)\]"` returns no hits on Windows targets
  - _Requirements: BC-04.1, BC-04.2, BC-04.3, BC-04.4, BC-04.6_

- [ ] 3. Phase 3: Service Infrastructure
  - Fix gRPC module paths and stream type definitions
  - Create feature-isolated examples package
  - Update configuration struct field names
  - _Requirements: BC-05, BC-06_

- [ ] 3.1 Fix gRPC module import paths
  - Pin tonic/tonic-build versions together in workspace to avoid module path drift
  - Update imports: `crate::proto::flight_service::flight_service_client::FlightServiceClient`
  - Update imports: `crate::proto::flight_service::flight_service_server::{FlightService, FlightServiceServer}`
  - Fix proto module imports in flight-ipc client.rs and server.rs
  - Verify: `cargo build -p flight-ipc` passes
  - Verify: No "could not find flight_service_client" errors
  - _Requirements: BC-05.1, BC-05.3, BC-05.6_

- [ ] 3.2 Define proper gRPC stream associated types
  - Define single reusable type: `type HealthSubscribeStream = Pin<Box<dyn Stream<Item = Result<HealthResponse, Status>> + Send>>;`
  - Use `Self::HealthSubscribeStream` in return positions consistently
  - Use `futures_core::Stream` or `tokio_stream::wrappers::ReceiverStream` for channel wrapping
  - Fix ambiguous associated type errors by using fully qualified `Self::StreamX` pattern
  - Verify: `cargo test -p flight-ipc` passes
  - Verify: No "ambiguous associated type" errors in service implementations
  - _Requirements: BC-05.2, BC-05.4, BC-05.5, BC-05.6_

- [ ] 3.3 Create feature-isolated examples package
  - Create examples as separate workspace member with `publish = false`
  - Use feature-gated dependencies per example to prevent heavyweight feature pulls
  - Add `[target.'cfg(windows)'.dependencies]` for Windows-only examples
  - Structure: axis_demo.rs (flight-axis only), replay_demo.rs (flight-replay), integration_demo.rs (multi-crate behind feature)
  - Alternative: Keep examples per-crate and add tiny aggregator for run commands
  - Verify: `cargo run -p openflight-examples --example axis_demo` works
  - Verify: `cargo tree -p openflight-examples | grep windows` empty on non-Windows runners
  - _Requirements: BC-06.1, BC-06.5, BC-06.6, BC-06.7_

- [ ] 3.4 Update configuration struct field names
  - Change BlackboxConfig `output_path` to `output_dir: PathBuf`
  - Change `compression_enabled` to `enable_compression: bool`
  - Add `buffer_size: usize` field where needed
  - Remove ? operator from BlackboxWriter::new() and other non-Result constructor calls
  - Verify: `cargo run -p openflight-examples --example capture_replay_demo` builds after config fix
  - Verify: `git grep -n "BlackboxWriter::new.*?" | wc -l` returns 0
  - _Requirements: BC-06.2, BC-06.3, BC-06.4_

- [ ] 4. Phase 4: Security & Safety
  - Migrate ed25519-dalek to v2 API
  - Fix packed struct memory safety violations
  - Implement proper error handling for cryptographic operations
  - _Requirements: BC-07, BC-08_

- [ ] 4.1 Migrate ed25519-dalek v2 API
  - Update dependencies: `ed25519-dalek = { version = "2", features = ["rand_core"] }`
  - Prefer `rand_core = "0.6"` over full `rand` crate unless other code needs rand
  - Replace `PublicKey` with `VerifyingKey` and `Keypair` with `SigningKey`
  - Update key generation: `SigningKey::generate(&mut OsRng)` with `use rand_core::OsRng;`
  - Fix signature verification: `verifying_key.verify(message, &signature)?`
  - Verify: `cargo test -p flight-updater -- signature` passes
  - _Requirements: BC-07.1, BC-07.2, BC-07.3, BC-07.6_

- [ ] 4.2 Fix byte array conversions for signatures
  - Implement `Vec<u8>` to `[u8; 64]` conversions: `sig_bytes.as_slice().try_into().map_err(...)?`
  - Add proper error handling: `.map_err(|_| anyhow::anyhow!("Invalid signature length"))?`
  - Create verification helper with length checks for both signatures and keys
  - Handle `VerifyingKey::from_bytes(&[u8;32])` and `Signature::from_bytes(&[u8;64])` conversions
  - Verify: All signature operations compile without type mismatch errors
  - _Requirements: BC-07.4, BC-07.5_

- [ ] 4.3 Fix packed struct memory safety violations
  - Identify all packed struct field reference violations in flight-virtual/src/ofp1_emulator.rs (lines 632-634, 677)
  - Replace `&packed.field` with `let value = packed.field; &value` for Copy types
  - Use `unsafe { ptr::read_unaligned(ptr::addr_of!(packed.field)) }` for non-Copy fields
  - Implement `ptr::addr_of!(packed.field)` for address-only operations
  - Verify: `cargo clippy --workspace -- -W clippy::unaligned_references -W clippy::borrow_deref_ref` passes
  - _Requirements: BC-08.1, BC-08.2, BC-08.3, BC-08.4, BC-08.6_

- [ ] 5. Phase 5: Quality & Cleanup
  - Fix test and benchmark infrastructure
  - Suppress FFI warnings appropriately
  - Add regression prevention measures
  - _Requirements: BC-09, BC-10_

- [ ] 5.1 Update Criterion benchmark infrastructure
  - Upgrade to `criterion = "0.5"` in dev-dependencies
  - Use `b.to_async(&rt).iter()` pattern for async benchmarks
  - Set `harness = false` in `[[bench]]` sections of Cargo.toml
  - Replace all `criterion::black_box` with `std::hint::black_box` (grep task)
  - Update imports to `use criterion::{criterion_group, criterion_main, Criterion};`
  - Verify: `cargo bench -p flight-replay` compiles and runs
  - Verify: `git grep -n "criterion::black_box" | wc -l` returns 0
  - _Requirements: BC-09.2, BC-09.3, BC-09.7_

- [ ] 5.2 Implement test-only field accessors
  - Add `#[cfg(any(test, feature = "test-helpers"))]` accessors for private fields in flight-panels
  - Create `pub(crate)` getter methods for `stack`, `variable_cache`, `actions_buffer` in RulesEvaluator
  - Add optional `test-helpers` feature to enable downstream integration test support
  - Target specific files: flight-panels tests that access private fields
  - Verify: `cargo test --workspace` passes
  - Verify: `cargo test -p flight-panels --features test-helpers` if downstream support needed
  - _Requirements: BC-09.1, BC-09.5, BC-09.6_

- [ ] 5.3 Fix unsafe operation wrapping
  - Wrap `GlobalAlloc::alloc` and `dealloc` calls in `unsafe {}` blocks in allocation tests
  - Target specific files: allocation_test.rs and similar test files with unsafe intrinsics
  - Ensure all unsafe operations are properly contained within unsafe blocks
  - Add unsafe blocks where compiler requires them for raw pointer operations
  - Verify: `cargo test --workspace` passes without unsafe operation warnings
  - _Requirements: BC-09.4_

- [ ] 5.4 Suppress FFI warnings in sys crates
  - Add `#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]` at crate root
  - Apply to flight-simconnect-sys/src/lib.rs and other FFI binding crates
  - Disable style lints in build-generated modules if bindgen emits submodules
  - Preserve C naming conventions while reducing warning noise
  - Verify: `cargo clippy -p flight-simconnect-sys` shows no style flood
  - Verify: `cargo clippy -p flight-simconnect -- -D warnings` passes after sys crate allows
  - _Requirements: BC-10.1, BC-10.2, BC-10.3, BC-10.6_

- [ ]* 5.5 Add regression prevention measures
  - Add workspace dependency version alignment task (tonic/tonic-build, tokio, futures versions)
  - Implement feature powerset testing: `cargo hack check --workspace --feature-powerset --depth 2`
  - Add clippy enforcement: `cargo clippy --workspace -- -D warnings` for core crates
  - Create dead code/import cleanup pass: `cargo fix --workspace --allow-dirty`
  - Set up CI verification greps for each critical pattern (Profile::merge, BlackboxWriter::new?, etc.)
  - _Requirements: NFR-B, NFR-C_

- [ ]* 5.6 Verify all compilation targets
  - Run `cargo check --workspace` on both Windows and Linux CI
  - Verify examples compile and run: `cargo run -p openflight-examples --example <name>`
  - Test serde feature combinations: `cargo check -p flight-axis --features serde`
  - Validate cross-platform compatibility with CI matrix
  - Run final verification commands from design document
  - _Requirements: All BC requirements verification_