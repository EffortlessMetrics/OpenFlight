# PR: Microcrate Architecture Refactor & Advanced Testing Infrastructure

## Summary
This PR executes a major architectural refactor of the light-core monolith, breaking it down into specialized microcrates to improve compilation times, enforce separation of concerns, and enable granular testing.

Simultaneously, it introduces a comprehensive **Advanced Testing Infrastructure** comprising Property-Based Testing (PBT), Fuzzing, and Mutation Testing to ensure mission-critical reliability for the flight simulation platform.

## 🏗️ Architecture Changes

### Microcrate Extraction
The following functional domains have been extracted from light-core into independent crates:

*   **light-units**: Physical unit conversions (Angle, Distance, Speed) with strictly typed operations.
*   **light-profile**: Aircraft profile management, validation, and JSON serialization.
*   **light-rules**: The Rules DSL parser, condition evaluation engine, and action dispatcher.
*   **light-ipc**: Inter-Process Communication, protobuf definitions, and protocol negotiation.
*   **light-process-detection**: OS-level process monitoring and simulator auto-detection strategies.
*   **light-session**: Session state management, active profile tracking, and telemetry snapshotting.
*   **light-blackbox**: High-performance binary flight recorder implementation.
*   **light-watchdog**: System health monitoring, component quarantine, and fault injection.
*   **light-axis**: Zero-allocation real-time axis processing pipeline.

light-core has been refactored to serve as a facade/integration layer that re-exports these crates, maintaining backward compatibility where possible while enforcing a clean dependency graph.

## 🛡️ Testing Infrastructure

### 1. Property-Based Testing (PBT)
We have integrated proptest across the workspace to verify invariants under randomized inputs:

| Crate | Properties Verified |
|-------|---------------------|
| light-units | Round-trip conversions, normalization bounds (0-360°), and arithmetic distributivity. |
| light-profile | JSON schema validity, capability enforcement, and profile validation rules. |
| light-rules | DSL parsing robustness, condition logic truth tables. |
| light-ipc | Protocol version negotiation compatibility, feature flags intersection logic. |
| light-process-detection | Process name matching strategies (fuzzy vs exact). |
| light-session | Telemetry snapshot serialization round-trips. |
| light-blackbox | Binary format header/footer integrity, index calculation, and CRC32C validation. |
| light-watchdog | Watchdog configuration bounds, component ID stability. |
| light-axis | Deadzone/Curve mathematics, monotonicity, and mixer logic clamping. |

### 2. Fuzzing
New fuzzing targets have been established using cargo-fuzz (libFuzzer) to harden security boundaries:
*   **crates/flight-ipc/fuzz**: Targets protobuf message decoding to prevent DoS or panic on malformed network packets.
*   **crates/flight-rules/fuzz**: Targets the Rules DSL compiler to ensure it handles arbitrary user input without crashing.

### 3. Mutation Testing
We have configured cargo-mutants to verify test suite quality:
*   Added mutants.toml configuration for the workspace.
*   Verified that test suites correctly kill synthetic mutants (logic inversions, off-by-one errors) in critical crates like light-units and light-profile.

## 🔍 Verification

*   **Build**: Workspace compiles successfully with all new crates linked.
*   **Unit Tests**: All existing unit tests pass (cargo test --workspace).
*   **PBT**: All property tests pass, verifying thousands of input combinations per run.
*   **Fuzzing**: Fuzz targets compile and run (verified in nightly environment).

## Checklist
- [x] Extracted core modules into microcrates.
- [x] Updated Cargo.toml workspace members and dependencies.
- [x] Implemented Property-Based Tests for 9 core crates.
- [x] Established Fuzzing harness for IPC and Rules.
- [x] Configured Mutation Testing.
- [x] Verified all tests pass.
