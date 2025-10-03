# Requirements Document

## Introduction

This feature addresses critical compilation errors across the OpenFlight workspace that are preventing successful builds. The implementation focuses on resolving API changes, missing dependencies, platform-specific code issues, and type mismatches that have accumulated across multiple crates. These fixes are essential to restore a working build state and enable continued development.

## Definitions

**Serde Feature Policy:** Types that may be serialized MUST be wrapped in `#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]` and expose a crate feature named exactly `serde`.

**Platform Code Policy:** Platform-specific imports MUST use `#[cfg(unix)] use std::os::fd::*;` and `#[cfg(windows)] use std::os::windows::io::*;`.

**Examples Location Policy:** All cross-crate demos live in `examples/` package with proper Cargo.toml dependencies, OR each crate holds its own `examples/` directory.

**FFI Lint Policy:** FFI sys crates MUST use `#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]` at crate root.

## Requirements

### Requirement BC-01

**User Story:** As a developer, I want the flight-axis crate to compile successfully, so that I can build and test axis-related functionality.

#### Acceptance Criteria

1. WHEN building flight-axis examples THEN the system SHALL compile without missing field errors in EngineConfig
2. WHEN creating an Engine instance THEN the system SHALL accept Engine::new(name: String, config: EngineConfig) signature
3. WHEN EngineConfig is initialized THEN the system SHALL include conflict_detector_config: ConflictDetectorConfig and enable_conflict_detection: bool fields
4. WHEN running axis integration tests THEN the system SHALL compile and execute without API mismatch errors
5. WHEN building axis performance tests THEN the system SHALL use the correct Engine::new signature
6. WHEN verifying the fix THEN `cargo build -p flight-axis --examples --tests --benches` SHALL pass
7. WHEN checking API usage THEN `git grep -n "Engine::new("` SHALL show 2 arguments in all call sites

### Requirement BC-02

**User Story:** As a developer, I want serialization to work across all crates, so that data can be properly serialized and deserialized for storage and transmission.

#### Acceptance Criteria

1. WHEN serializing AxisFrame THEN the system SHALL have Serialize/Deserialize traits available via serde feature
2. WHEN serializing SessionConfig THEN the system SHALL have Serialize/Deserialize traits available via serde feature
3. WHEN using serde features THEN the system SHALL conditionally compile serialization code behind exactly named "serde" feature
4. WHEN dependent crates need serialization THEN the system SHALL enable serde features via `features = ["serde"]` in Cargo.toml
5. WHEN using bincode serialization THEN the system SHALL compile without trait bound errors
6. WHEN verifying serde features THEN `cargo check -p flight-axis --features serde` SHALL pass
7. WHEN consumers use serialization THEN `bincode::serialize(&AxisFrame{...})` SHALL type-check in flight-replay

### Requirement BC-03

**User Story:** As a developer, I want flight-simconnect to compile on Windows, so that SimConnect integration works properly.

#### Acceptance Criteria

1. WHEN building on Windows THEN the system SHALL include windows crate with features ["Win32_System_Threading", "Win32_Foundation", "Win32_System_Diagnostics_ToolHelp", "Win32_System_ProcessStatus"]
2. WHEN using futures in simconnect THEN the system SHALL include futures = "0.3" dependency
3. WHEN handling async operations THEN std::sync::Mutex.lock() SHALL NOT be awaited, tokio::sync::Mutex.lock().await is correct
4. WHEN managing subscriptions THEN the system SHALL handle mutable/immutable borrow conflicts by shortening immutable borrows before mutation
5. WHEN converting error types THEN the system SHALL have From<BusTypeError> for MappingError implementation
6. WHEN importing SimConnect types THEN the system SHALL resolve SIMCONNECT_RECV_ID from flight-simconnect-sys
7. WHEN verifying Windows build THEN `cargo build -p flight-simconnect` SHALL pass on windows-latest CI

### Requirement BC-04

**User Story:** As a developer, I want cross-platform compatibility, so that the code builds on both Windows and Unix systems.

#### Acceptance Criteria

1. WHEN building on Windows THEN the system SHALL not import Unix-specific std::os::fd modules
2. WHEN building on Unix THEN the system SHALL not import Windows-specific std::os::windows modules
3. WHEN using file descriptors THEN the system SHALL use `#[cfg(unix)] use std::os::fd::*;` and `#[cfg(windows)] use std::os::windows::io::*;`
4. WHEN running tests THEN the system SHALL conditionally compile platform-specific test code with #[cfg] attributes
5. WHEN handling raw handles/fds THEN the system SHALL use cfg-gated platform abstractions
6. WHEN verifying cross-platform build THEN `cargo check --workspace` SHALL pass on both windows-latest and ubuntu-latest CI

### Requirement BC-05

**User Story:** As a developer, I want gRPC services to compile correctly, so that inter-service communication works properly.

#### Acceptance Criteria

1. WHEN using tonic-generated code THEN the system SHALL import from `crate::proto::flight_service::flight_service_client::FlightServiceClient` path
2. WHEN implementing gRPC services THEN the system SHALL use `type StreamX = Pin<Box<dyn Stream<Item = Result<Msg, Status>> + Send>>;` for associated types
3. WHEN building flight-ipc THEN the system SHALL resolve proto module imports correctly from tonic-build generated paths
4. WHEN defining service implementations THEN the system SHALL use `Self::StreamX` for associated types in return positions
5. WHEN handling gRPC streams THEN the system SHALL use proper `Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>` types
6. WHEN verifying gRPC build THEN `cargo build -p flight-ipc` and `cargo test -p flight-ipc` SHALL pass

### Requirement BC-06

**User Story:** As a developer, I want examples to build and run, so that I can demonstrate and test functionality.

#### Acceptance Criteria

1. WHEN building top-level examples THEN the system SHALL create examples/ package with proper Cargo.toml dependencies
2. WHEN running examples THEN the system SHALL use correct field names: output_dir, enable_compression, buffer_size
3. WHEN using BlackboxConfig THEN the system SHALL use output_dir: PathBuf instead of output_path
4. WHEN creating writers THEN the system SHALL handle BlackboxWriter::new(config) without ? operator if it returns T not Result<T, E>
5. WHEN examples need tokio THEN the system SHALL have tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
6. WHEN verifying examples THEN `cargo run -p openflight-examples --example <name>` SHALL work for centralized approach
7. WHEN using distributed examples THEN `cargo run -p <crate> --example <name>` SHALL work for per-crate approach

### Requirement BC-07

**User Story:** As a developer, I want cryptographic operations to work, so that signature verification and security features function properly.

#### Acceptance Criteria

1. WHEN using ed25519-dalek THEN the system SHALL use v2 API with VerifyingKey (not PublicKey) and SigningKey (not Keypair)
2. WHEN verifying signatures THEN the system SHALL use `VerifyingKey::from_bytes(&[u8;32]) -> Result<_, ed25519_dalek::Error>`
3. WHEN generating keys THEN the system SHALL use `SigningKey::generate(&mut OsRng)` with rand = "0.8" dependency
4. WHEN handling signature bytes THEN the system SHALL convert `Vec<u8>` to `[u8; 64]` via `sig_vec.as_slice().try_into()?`
5. WHEN using cryptographic functions THEN the system SHALL include ed25519-dalek = { version = "2", features = ["rand_core"] }
6. WHEN verifying crypto functionality THEN `cargo test -p flight-updater -- signature*` SHALL pass

### Requirement BC-08

**User Story:** As a developer, I want memory-safe operations with packed structs, so that the code follows Rust safety guidelines.

#### Acceptance Criteria

1. WHEN accessing packed struct fields THEN the system SHALL never take `&packed.field` references
2. WHEN reading packed fields THEN the system SHALL copy by value if Copy, or use `unsafe { ptr::read_unaligned(addr_of!(packed.field)) }`
3. WHEN working with packed data THEN the system SHALL avoid undefined behavior from unaligned references
4. WHEN the compiler warns about packed fields THEN the system SHALL use `ptr::addr_of!` for safe access
5. WHEN handling packed structs THEN the system SHALL maintain memory safety without compromising functionality
6. WHEN verifying packed field safety THEN `cargo clippy -W clippy::unaligned_references` SHALL show no warnings

### Requirement BC-09

**User Story:** As a developer, I want test code to compile and run, so that I can verify functionality and maintain code quality.

#### Acceptance Criteria

1. WHEN tests access private fields THEN the system SHALL provide `#[cfg(test)] pub(crate) fn field(&self) -> &Type` accessors
2. WHEN using Criterion benchmarks THEN the system SHALL use `b.to_async(&rt).iter()` API with `[[bench]] harness = false`
3. WHEN benchmarking code THEN the system SHALL use `std::hint::black_box` instead of `criterion::black_box`
4. WHEN tests need unsafe operations THEN the system SHALL wrap GlobalAlloc calls in `unsafe {}` blocks
5. WHEN running integration tests THEN the system SHALL compile without visibility or API errors
6. WHEN verifying tests THEN `cargo test --workspace` SHALL pass
7. WHEN verifying benchmarks THEN `cargo bench -p flight-replay` SHALL compile and run

### Requirement BC-10

**User Story:** As a developer, I want FFI bindings to compile cleanly, so that C interop works without excessive warnings.

#### Acceptance Criteria

1. WHEN building FFI sys crates THEN the system SHALL use `#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]` at crate root
2. WHEN compiling C bindings THEN the system SHALL suppress style lints for generated code while preserving functionality
3. WHEN using SimConnect bindings THEN the system SHALL compile without hundreds of naming warnings
4. WHEN building sys crates THEN the system SHALL maintain C compatibility while reducing noise
5. WHEN FFI types are used THEN the system SHALL preserve original C naming without Rust style enforcement
6. WHEN verifying FFI cleanliness THEN `cargo clippy -p flight-simconnect-sys` SHALL not flood with style warnings

## Feature Policy Table

| Feature          | Defined in          | Enables                         | Consumers must…                   |
| ---------------- | ------------------- | ------------------------------- | --------------------------------- |
| `serde`          | flight-axis, flight-simconnect | `Serialize/Deserialize` derives | Opt-in via `features = ["serde"]` |
| `unix`/`windows` | workspace (via cfg) | Platform APIs                   | N/A (driven by target)            |

## Definition of Done

**BC-01:** `cargo build -p flight-axis --examples --tests --benches` passes
**BC-02:** `cargo check -p flight-axis --features serde` and downstream consumers compile with `features=["serde"]`
**BC-03:** Windows job `cargo build -p flight-simconnect` passes
**BC-04:** OS matrix `cargo check --workspace` passes on windows-latest and ubuntu-latest
**BC-05:** `cargo build -p flight-ipc` and `cargo test -p flight-ipc` pass
**BC-06:** `cargo run -p openflight-examples --example <name>` works (or per-crate equivalent)
**BC-07:** `cargo test -p flight-updater -- signature*` passes
**BC-08:** `cargo clippy -W clippy::unaligned_references` shows no warnings (optional)
**BC-09:** `cargo test --workspace` and `cargo bench -p flight-replay` pass
**BC-10:** `cargo clippy -p flight-simconnect-sys` shows no style flood

## Non-Functional Requirements

**NFR-A (Build Performance):** Workspace debug build ≤ 8 minutes on CI standard runners; no new default features on core crates.

**NFR-B (Maintainability):** All API changes MUST be applied via a single PR with a crate-by-crate checklist; CI enforces consistent usage.

**NFR-C (Platform Support):** CI matrix runs on windows-latest + ubuntu-latest, executing `cargo check --workspace` + key package builds.