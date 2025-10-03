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
  - _Requirements: BC-01.1, BC-01.2, BC-01.3, BC-01.6, BC-01.7_

- [ ] 1.2 Fix Profile API rename in flight-core
  - Change Profile::merge calls to Profile::merge_with in aircraft_switch.rs
  - Verify argument order matches merge_with expectations
  - Ensure no remaining Profile::merge calls exist in codebase
  - _Requirements: BC-01.4_

- [ ] 1.3 Resolve async recursion in flight-updater
  - Convert recursive async functions to iterative loops where possible
  - Add async-recursion dependency for functions that must remain recursive
  - Fix duplicate function names in packaging.rs
  - _Requirements: BC-07.6_

- [ ] 1.4 Implement serde feature infrastructure
  - Add serde feature to flight-axis and flight-simconnect Cargo.toml
  - Add conditional derive macros for AxisFrame and SessionConfig
  - Update consumer crates to enable serde features appropriately
  - _Requirements: BC-02.1, BC-02.2, BC-02.3, BC-02.4, BC-02.6, BC-02.7_

- [ ] 2. Phase 2: Platform Compatibility
  - Add Windows-specific dependencies and fix platform-specific code
  - Implement error conversion infrastructure
  - Fix borrow checker conflicts in mapping code
  - _Requirements: BC-03, BC-04_

- [ ] 2.1 Add Windows dependencies and fix imports
  - Add windows crate with required features to flight-simconnect
  - Add futures crate dependency
  - Fix SIMCONNECT_RECV_ID import from flight-simconnect-sys
  - _Requirements: BC-03.1, BC-03.2, BC-03.6, BC-03.7_

- [ ] 2.2 Implement error conversion infrastructure
  - Create MappingError enum with From implementations for BusTypeError and SimConnectError
  - Add From<crate::transport::TransportError> conversion
  - Update function signatures to return Result<_, MappingError> where needed
  - _Requirements: BC-03.5_

- [ ] 2.3 Fix async/sync mutex usage patterns
  - Identify std::sync::Mutex vs tokio::sync::Mutex usage
  - Remove .await from std::sync::Mutex.lock() calls
  - Ensure tokio::sync::Mutex.lock().await usage is correct
  - _Requirements: BC-03.3_

- [ ] 2.4 Fix borrow checker conflicts in mapping.rs
  - Implement scoped borrow pattern to narrow immutable borrow lifetimes
  - Refactor subscription management to avoid mutable/immutable conflicts
  - Use local variable collection before mutation operations
  - _Requirements: BC-03.4_

- [ ] 2.5 Implement platform-specific code gates
  - Add cfg gates for Unix-specific std::os::fd imports
  - Add cfg gates for Windows-specific std::os::windows imports
  - Gate platform-specific test modules with appropriate cfg attributes
  - _Requirements: BC-04.1, BC-04.2, BC-04.3, BC-04.4, BC-04.6_

- [ ] 3. Phase 3: Service Infrastructure
  - Fix gRPC module paths and stream type definitions
  - Create feature-isolated examples package
  - Update configuration struct field names
  - _Requirements: BC-05, BC-06_

- [ ] 3.1 Fix gRPC module import paths
  - Update tonic-generated module imports to use correct nested paths
  - Fix flight_service_client and flight_service_server import paths
  - Update proto module imports in flight-ipc
  - _Requirements: BC-05.1, BC-05.3, BC-05.6_

- [ ] 3.2 Define proper gRPC stream associated types
  - Implement Self::StreamX pattern for associated types in service implementations
  - Use Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>> for stream types
  - Fix ambiguous associated type errors in server implementations
  - _Requirements: BC-05.2, BC-05.4, BC-05.5, BC-05.6_

- [ ] 3.3 Create feature-isolated examples package
  - Create examples/Cargo.toml with feature-gated dependencies
  - Implement feature isolation to prevent unnecessary dependency pulls
  - Move top-level examples into proper package structure
  - Add platform-specific feature gates for Windows-only examples
  - _Requirements: BC-06.1, BC-06.5, BC-06.6, BC-06.7_

- [ ] 3.4 Update configuration struct field names
  - Change BlackboxConfig output_path to output_dir
  - Change compression_enabled to enable_compression
  - Add buffer_size field where needed
  - Remove ? operator from non-Result constructor calls
  - _Requirements: BC-06.2, BC-06.3, BC-06.4_

- [ ] 4. Phase 4: Security & Safety
  - Migrate ed25519-dalek to v2 API
  - Fix packed struct memory safety violations
  - Implement proper error handling for cryptographic operations
  - _Requirements: BC-07, BC-08_

- [ ] 4.1 Migrate ed25519-dalek v2 API
  - Update dependencies to ed25519-dalek v2 with rand_core feature
  - Replace PublicKey with VerifyingKey and Keypair with SigningKey
  - Update key generation to use SigningKey::generate(&mut OsRng)
  - Fix signature verification API calls
  - _Requirements: BC-07.1, BC-07.2, BC-07.3, BC-07.6_

- [ ] 4.2 Fix byte array conversions for signatures
  - Implement proper Vec<u8> to [u8; 64] conversions using try_into()
  - Add error handling for invalid signature and key lengths
  - Create verification helper functions with proper error propagation
  - _Requirements: BC-07.4, BC-07.5_

- [ ] 4.3 Fix packed struct memory safety violations
  - Identify all packed struct field reference violations
  - Replace direct field references with copy-by-value for Copy types
  - Use ptr::read_unaligned for non-Copy field access
  - Implement ptr::addr_of! for address-only operations
  - _Requirements: BC-08.1, BC-08.2, BC-08.3, BC-08.4, BC-08.6_

- [ ] 5. Phase 5: Quality & Cleanup
  - Fix test and benchmark infrastructure
  - Suppress FFI warnings appropriately
  - Add regression prevention measures
  - _Requirements: BC-09, BC-10_

- [ ] 5.1 Update Criterion benchmark infrastructure
  - Upgrade to Criterion 0.5 with proper async support
  - Use b.to_async(&rt).iter() pattern for async benchmarks
  - Set harness = false in Cargo.toml for benchmark targets
  - Replace criterion::black_box with std::hint::black_box
  - _Requirements: BC-09.2, BC-09.3, BC-09.7_

- [ ] 5.2 Implement test-only field accessors
  - Add cfg(any(test, feature = "test-helpers")) accessors for private fields
  - Create pub(crate) getter methods for stack, variable_cache, and other test-accessed fields
  - Enable downstream integration test support through test-helpers feature
  - _Requirements: BC-09.1, BC-09.5, BC-09.6_

- [ ] 5.3 Fix unsafe operation wrapping
  - Wrap GlobalAlloc::alloc and dealloc calls in unsafe blocks
  - Ensure all unsafe operations are properly contained
  - Add unsafe blocks where compiler requires them
  - _Requirements: BC-09.4_

- [ ] 5.4 Suppress FFI warnings in sys crates
  - Add crate-level lint allows for non_camel_case_types, non_snake_case, non_upper_case_globals
  - Apply to flight-simconnect-sys and other FFI binding crates
  - Preserve C naming conventions while reducing warning noise
  - _Requirements: BC-10.1, BC-10.2, BC-10.3, BC-10.6_

- [ ]* 5.5 Add regression prevention measures
  - Implement feature powerset testing with cargo hack
  - Add clippy enforcement for memory safety patterns
  - Create CI verification commands for each requirement
  - Set up workspace dependency version alignment
  - _Requirements: NFR-B, NFR-C_

- [ ]* 5.6 Verify all compilation targets
  - Run cargo check --workspace on Windows and Linux
  - Verify examples compile and run correctly
  - Test serde feature combinations
  - Validate cross-platform compatibility
  - _Requirements: All BC requirements verification_