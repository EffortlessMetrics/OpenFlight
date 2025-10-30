# Requirements Document

## Introduction

This feature addresses Clippy lints in the flight-core crate that are currently blocking the IPC benchmark workflow. When running `cargo clippy` on flight-ipc with `-D warnings`, Clippy walks local workspace dependencies and fails on flight-core warnings. The goal is to apply minimal, mechanical refactors to flight-core to satisfy Clippy's recommendations without changing behavior, ensuring the IPC bench workflow passes cleanly.

## Glossary

- **Clippy**: Rust's official linter that catches common mistakes and suggests idiomatic improvements
- **flight-core**: Core crate in the workspace containing shared functionality
- **flight-ipc**: IPC (Inter-Process Communication) crate that depends on flight-core
- **IPC Bench Workflow**: The benchmark compilation and linting workflow for the IPC crate (task 7.3)
- **Workspace Dependencies**: Local crates within the same Cargo workspace that depend on each other
- **Mechanical Refactor**: Code changes that improve style/idioms without altering runtime behavior

## Requirements

### Requirement 1

**User Story:** As a developer, I want the IPC benchmark workflow to pass Clippy checks, so that I can validate my IPC changes without being blocked by unrelated warnings in dependencies

#### Acceptance Criteria

1. WHEN the developer runs `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings`, THEN the Clippy System SHALL complete without errors
2. WHEN the developer runs `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings`, THEN the Clippy System SHALL complete without errors
3. WHEN the developer runs `cargo clippy -p flight-core -- -Dwarnings`, THEN the Clippy System SHALL complete without errors
4. WHEN Clippy checks complete successfully, THEN the Build System SHALL preserve all existing runtime behavior in flight-core

### Requirement 2

**User Story:** As a developer, I want flight-core code to follow Clippy's idiomatic recommendations, so that the codebase maintains consistent Rust best practices

#### Acceptance Criteria

1. WHERE the code iterates over map key-value pairs but only uses values, THE flight-core Crate SHALL use `.values()` or `.values_mut()` methods
2. WHERE the code manually checks if a value is within a range, THE flight-core Crate SHALL use `RangeInclusive::contains()` method
3. WHERE the code uses `if let Ok(entry) = entry` pattern on iterators, THE flight-core Crate SHALL use `.flatten()` method
4. WHERE the code uses `format!()` on string literals, THE flight-core Crate SHALL use `.to_string()` method
5. WHERE function parameters accept `&PathBuf`, THE flight-core Crate SHALL accept `&Path` instead

### Requirement 3

**User Story:** As a developer, I want simplified control flow in flight-core, so that the code is more readable and maintainable

#### Acceptance Criteria

1. WHERE the code has identical if-else branches, THE flight-core Crate SHALL combine the conditions using logical operators
2. WHERE the code has nested if statements that can be combined, THE flight-core Crate SHALL collapse them into a single condition with logical AND operators
3. WHERE the code uses `match` with a single meaningful arm, THE flight-core Crate SHALL use `if let` instead
4. WHEN control flow is simplified, THEN the flight-core Crate SHALL maintain identical runtime behavior

### Requirement 4

**User Story:** As a developer, I want all existing tests to pass after Clippy fixes, so that I can verify no regressions were introduced

#### Acceptance Criteria

1. WHEN the developer runs `cargo test -p flight-core`, THEN the Test System SHALL pass all existing tests
2. WHEN the developer runs `cargo bench -p flight-ipc --features ipc-bench --no-run`, THEN the Build System SHALL compile benchmarks successfully
3. WHEN the developer runs `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run`, THEN the Build System SHALL compile benchmarks successfully
4. WHEN the developer runs the full test suite, THEN the Test System SHALL show no regressions from the baseline
