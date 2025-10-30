# Requirements Document

## Introduction

This feature addresses Clippy lints in the flight-core crate that are currently blocking the IPC benchmark workflow. When running `cargo clippy` on flight-ipc with `-D warnings`, Clippy walks local workspace dependencies and fails on flight-core warnings. The goal is to apply minimal, mechanical refactors to flight-core to satisfy Clippy's recommendations without changing behavior, ensuring the IPC bench workflow passes cleanly. The solution includes CI guardrails for deterministic builds, behavior verification, and decoupled lint checking.

## Glossary

- **Clippy**: Rust's official linter that catches common mistakes and suggests idiomatic improvements
- **flight-core**: Core crate in the workspace containing shared functionality
- **flight-ipc**: IPC (Inter-Process Communication) crate that depends on flight-core
- **IPC Bench Workflow**: The benchmark compilation and linting workflow for the IPC crate (task 7.3)
- **Workspace Dependencies**: Local crates within the same Cargo workspace that depend on each other
- **Mechanical Refactor**: Code changes that improve style/idioms without altering runtime behavior
- **MSRV**: Minimum Supported Rust Version, the oldest Rust version the crate supports
- **Public API**: The externally visible interface of a crate (public functions, types, traits)
- **Clippy Lint Rules**: Specific warnings issued by Clippy (e.g., for_kv_map, manual_flatten)

## Requirements

### Requirement 1

**User Story:** As a developer, I want the IPC benchmark workflow to pass Clippy checks, so that I can validate my IPC changes without being blocked by unrelated warnings in dependencies

#### Acceptance Criteria

1. WHEN the developer runs `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings`, THEN the Clippy System SHALL complete without errors on both ubuntu-latest and windows-latest
2. WHEN the developer runs `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings`, THEN the Clippy System SHALL complete without errors on both ubuntu-latest and windows-latest
3. WHEN the developer runs `cargo clippy -p flight-core -- -Dwarnings`, THEN the Clippy System SHALL complete without errors on both ubuntu-latest and windows-latest
4. WHEN Clippy checks run in CI or locally, THEN the Build System SHALL use the MSRV defined in clippy.toml (1.75.0)
5. WHERE the IPC bench workflow is blocked by transient flight-core issues, THE CI System SHALL provide a fallback lane using `--no-deps` flag to unblock IPC validation

### Requirement 2

**User Story:** As a developer, I want flight-core code to follow Clippy's idiomatic recommendations, so that the codebase maintains consistent Rust best practices

#### Acceptance Criteria

1. WHERE the code in profile.rs iterates over map key-value pairs but only uses values, THE flight-core Crate SHALL use `.values_mut()` method to address for_kv_map lint
2. WHERE the code in profile.rs manually checks if a value is within a range, THE flight-core Crate SHALL use `RangeInclusive::contains()` method to address manual_range_contains lint
3. WHERE the code in writers.rs uses `if let Ok(entry) = entry` pattern on directory iterators, THE flight-core Crate SHALL use `.flatten()` method to address manual_flatten lint
4. WHERE the code in writers.rs iterates over map key-value pairs but only uses values, THE flight-core Crate SHALL use `.values()` method to address for_kv_map lint
5. WHERE the code in watchdog.rs uses `format!()` on string literals, THE flight-core Crate SHALL use `.to_string()` method to address useless_format lint
6. WHERE function parameters in aircraft_switch.rs accept `&PathBuf`, THE flight-core Crate SHALL accept `&Path` instead to address ptr_arg lint
7. WHEN parameter types change from `&PathBuf` to `&Path`, THEN the Rust Compiler SHALL guarantee no change in call-site semantics

### Requirement 3

**User Story:** As a developer, I want simplified control flow in flight-core, so that the code is more readable and maintainable

#### Acceptance Criteria

1. WHERE the code in security/verification.rs has identical if-else branches returning VerificationStatus::Fail, THE flight-core Crate SHALL combine the conditions using logical OR operators to address if_same_then_else lint
2. WHERE the code in security.rs has nested if statements checking ACL conditions, THE flight-core Crate SHALL collapse them into a single condition with logical AND operators to address collapsible_if lint
3. WHERE the code in security.rs uses `match` on SignatureStatus with a single meaningful arm, THE flight-core Crate SHALL use `if let` instead to address single_match lint
4. WHEN control flow is simplified, THEN the flight-core Crate SHALL maintain identical runtime behavior with no changes to execution paths

### Requirement 4

**User Story:** As a developer, I want all existing tests to pass after Clippy fixes, so that I can verify no regressions were introduced

#### Acceptance Criteria

1. WHEN the developer runs `cargo test -p flight-core`, THEN the Test System SHALL pass all existing tests with no additions or deletions
2. WHEN the developer runs `cargo test -p flight-virtual --tests`, THEN the Test System SHALL pass all existing tests
3. WHEN the developer runs `cargo bench -p flight-ipc --features ipc-bench --no-run`, THEN the Build System SHALL compile benchmarks successfully
4. WHEN the developer runs `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run`, THEN the Build System SHALL compile benchmarks successfully
5. WHEN all tests complete, THEN the Test System SHALL show no regressions from the baseline on both ubuntu-latest and windows-latest

### Requirement 5

**User Story:** As a developer, I want to verify that flight-core's public API remains unchanged, so that downstream crates are not broken by refactoring

#### Acceptance Criteria

1. WHEN the developer runs `cargo public-api -p flight-core --diff-git origin/main..HEAD`, THEN the Public API Tool SHALL report no changes to the public interface
2. THE flight-core Crate SHALL NOT add, remove, or modify any public functions, types, traits, or methods
3. THE flight-core Crate SHALL NOT change any function signatures visible to external crates
4. WHERE internal implementation details change, THEN the flight-core Crate SHALL ensure no impact on public API surface

### Requirement 6

**User Story:** As a developer, I want documentation of the Clippy rules addressed, so that future maintainers understand what was fixed and why

#### Acceptance Criteria

1. THE Documentation System SHALL record the exact Clippy lint rules addressed in a reference document
2. THE Documentation System SHALL map each lint rule to the specific file and refactor pattern applied
3. THE Documentation System SHALL include the following lint rules: for_kv_map, manual_range_contains, manual_flatten, useless_format, ptr_arg, if_same_then_else, collapsible_if, single_match
4. THE Documentation System SHALL provide the mapping in a format suitable for code review and future reference


## Lint-to-Patch Mapping

This section provides the exact mapping of Clippy lint rules to file locations and refactor patterns for traceability and review.

| File | Line | Lint Rule | Refactor Pattern |
|------|------|-----------|------------------|
| crates/flight-core/src/profile.rs | ~TBD | for_kv_map | Change `for (_, config) in &mut canonical.axes` to `for config in canonical.axes.values_mut()` |
| crates/flight-core/src/profile.rs | ~TBD | manual_range_contains | Change `if deadzone < 0.0 \|\| deadzone > MAX_DEADZONE` to `if !(0.0..=MAX_DEADZONE).contains(&deadzone)` |
| crates/flight-core/src/writers.rs | ~159 | manual_flatten | Change `for entry in entries { if let Ok(entry) = entry` to `for entry in entries.flatten()` and remove nested if let |
| crates/flight-core/src/writers.rs | ~464 | for_kv_map | Change `for (_, value) in &diff.changes` to `for value in diff.changes.values()` |
| crates/flight-core/src/writers.rs | ~687 | manual_flatten | Change `for entry in entries { if let Ok(entry) = entry` to `for entry in entries.flatten()` and remove nested if let |
| crates/flight-core/src/watchdog.rs | ~667 | useless_format | Change `format!("Component quarantined due to excessive failures")` to `"Component quarantined due to excessive failures".to_string()` |
| crates/flight-core/src/aircraft_switch.rs | ~686 | ptr_arg | Change parameter `base_path: &PathBuf` to `base_path: &Path` |
| crates/flight-core/src/security/verification.rs | ~531 | if_same_then_else | Combine `if has_failures { Fail } else if has_warnings && fail_on_warnings { Fail }` to `if has_failures \|\| (has_warnings && fail_on_warnings) { Fail }` |
| crates/flight-core/src/security.rs | ~384 | collapsible_if | Combine nested ifs: `if current_user_only { if user_id != get_current_user_id()` to `if current_user_only && user_id != get_current_user_id()` |
| crates/flight-core/src/security.rs | ~393 | collapsible_if | Combine nested ifs: `if !allowed_users.is_empty() { if !allowed_users.contains(&user_id)` to `if !allowed_users.is_empty() && !allowed_users.contains(&user_id)` |
| crates/flight-core/src/security.rs | ~431 | single_match | Change `match &manifest.signature { SignatureStatus::Signed { .. } => { .. } _ => {} }` to `if let SignatureStatus::Signed { .. } = &manifest.signature { .. }` |

**Note:** Line numbers are approximate and based on the original error reports. Exact line numbers will be confirmed during implementation.
