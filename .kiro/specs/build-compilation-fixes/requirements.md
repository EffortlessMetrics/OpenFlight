# Requirements Document

## Introduction

This feature addresses critical compilation errors across the OpenFlight workspace that are preventing successful builds. The implementation focuses on resolving API changes, missing dependencies, platform-specific code issues, and type mismatches that have accumulated across multiple crates. These fixes are essential to restore a working build state and enable continued development.

## Requirements

### Requirement BC-01

**User Story:** As a developer, I want the flight-axis crate to compile successfully, so that I can build and test axis-related functionality.

#### Acceptance Criteria

1. WHEN building flight-axis examples THEN the system SHALL compile without missing field errors in EngineConfig
2. WHEN creating an Engine instance THEN the system SHALL accept the new API signature with name parameter
3. WHEN EngineConfig is initialized THEN the system SHALL include conflict_detector_config and enable_conflict_detection fields
4. WHEN running axis integration tests THEN the system SHALL compile and execute without API mismatch errors
5. WHEN building axis performance tests THEN the system SHALL use the correct Engine::new signature

### Requirement BC-02

**User Story:** As a developer, I want serialization to work across all crates, so that data can be properly serialized and deserialized for storage and transmission.

#### Acceptance Criteria

1. WHEN serializing AxisFrame THEN the system SHALL have Serialize/Deserialize traits available
2. WHEN serializing SessionConfig THEN the system SHALL have Serialize/Deserialize traits available
3. WHEN using serde features THEN the system SHALL conditionally compile serialization code behind feature flags
4. WHEN dependent crates need serialization THEN the system SHALL enable serde features appropriately
5. WHEN using bincode serialization THEN the system SHALL compile without trait bound errors

### Requirement BC-03

**User Story:** As a developer, I want flight-simconnect to compile on Windows, so that SimConnect integration works properly.

#### Acceptance Criteria

1. WHEN building on Windows THEN the system SHALL include required windows crate dependencies
2. WHEN using futures in simconnect THEN the system SHALL include the futures crate dependency
3. WHEN handling async operations THEN the system SHALL not await on non-future types
4. WHEN managing subscriptions THEN the system SHALL handle mutable/immutable borrow conflicts properly
5. WHEN converting error types THEN the system SHALL have proper From implementations for error conversion
6. WHEN importing SimConnect types THEN the system SHALL resolve SIMCONNECT_RECV_ID imports correctly

### Requirement BC-04

**User Story:** As a developer, I want cross-platform compatibility, so that the code builds on both Windows and Unix systems.

#### Acceptance Criteria

1. WHEN building on Windows THEN the system SHALL not import Unix-specific std::os::fd modules
2. WHEN building on Unix THEN the system SHALL not import Windows-specific std::os::windows modules
3. WHEN using file descriptors THEN the system SHALL use platform-appropriate types and imports
4. WHEN running tests THEN the system SHALL conditionally compile platform-specific test code
5. WHEN handling raw handles/fds THEN the system SHALL use cfg-gated platform abstractions

### Requirement BC-05

**User Story:** As a developer, I want gRPC services to compile correctly, so that inter-service communication works properly.

#### Acceptance Criteria

1. WHEN using tonic-generated code THEN the system SHALL import from correct module paths
2. WHEN implementing gRPC services THEN the system SHALL use proper associated types for streams
3. WHEN building flight-ipc THEN the system SHALL resolve proto module imports correctly
4. WHEN defining service implementations THEN the system SHALL use Self::Type for associated types
5. WHEN handling gRPC streams THEN the system SHALL use proper Pin<Box<dyn Stream>> types

### Requirement BC-06

**User Story:** As a developer, I want examples to build and run, so that I can demonstrate and test functionality.

#### Acceptance Criteria

1. WHEN building top-level examples THEN the system SHALL have access to workspace crate dependencies
2. WHEN running examples THEN the system SHALL use correct field names for configuration structs
3. WHEN using BlackboxConfig THEN the system SHALL use output_dir instead of output_path
4. WHEN creating writers THEN the system SHALL handle constructor return types correctly
5. WHEN examples need tokio THEN the system SHALL have proper async runtime configuration

### Requirement BC-07

**User Story:** As a developer, I want cryptographic operations to work, so that signature verification and security features function properly.

#### Acceptance Criteria

1. WHEN using ed25519-dalek THEN the system SHALL use v2 API with VerifyingKey and SigningKey
2. WHEN verifying signatures THEN the system SHALL convert byte arrays to proper signature types
3. WHEN generating keys THEN the system SHALL use the new key generation API
4. WHEN handling signature bytes THEN the system SHALL convert Vec<u8> to [u8; 64] arrays properly
5. WHEN using cryptographic functions THEN the system SHALL include required rand dependencies

### Requirement BC-08

**User Story:** As a developer, I want memory-safe operations with packed structs, so that the code follows Rust safety guidelines.

#### Acceptance Criteria

1. WHEN accessing packed struct fields THEN the system SHALL not create direct references
2. WHEN reading packed fields THEN the system SHALL use read_unaligned or copy by value
3. WHEN working with packed data THEN the system SHALL avoid undefined behavior from unaligned references
4. WHEN the compiler warns about packed fields THEN the system SHALL use ptr::addr_of! for safe access
5. WHEN handling packed structs THEN the system SHALL maintain memory safety without compromising functionality

### Requirement BC-09

**User Story:** As a developer, I want test code to compile and run, so that I can verify functionality and maintain code quality.

#### Acceptance Criteria

1. WHEN tests access private fields THEN the system SHALL provide test-only accessors or appropriate visibility
2. WHEN using Criterion benchmarks THEN the system SHALL use the correct async API
3. WHEN benchmarking code THEN the system SHALL use std::hint::black_box instead of deprecated alternatives
4. WHEN tests need unsafe operations THEN the system SHALL wrap them in proper unsafe blocks
5. WHEN running integration tests THEN the system SHALL compile without visibility or API errors

### Requirement BC-10

**User Story:** As a developer, I want FFI bindings to compile cleanly, so that C interop works without excessive warnings.

#### Acceptance Criteria

1. WHEN building FFI sys crates THEN the system SHALL allow non-Rust naming conventions
2. WHEN compiling C bindings THEN the system SHALL suppress style lints for generated code
3. WHEN using SimConnect bindings THEN the system SHALL compile without hundreds of naming warnings
4. WHEN building sys crates THEN the system SHALL maintain C compatibility while reducing noise
5. WHEN FFI types are used THEN the system SHALL preserve original C naming without Rust style enforcement

## Non-Functional Requirements

**NFR-A (Build Performance):** Compilation fixes SHALL not significantly impact build times or introduce unnecessary dependencies.

**NFR-B (Maintainability):** API changes SHALL be applied consistently across all affected call sites to prevent future breakage.

**NFR-C (Platform Support):** Cross-platform code SHALL use appropriate cfg gates to maintain compatibility across Windows and Unix systems.