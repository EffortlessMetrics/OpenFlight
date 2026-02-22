# Design Document

## Overview
This design addresses Clippy lint warnings in flight-core through mechanical, behavior-neutral refactors. The approach prioritizes CI determinism, public API stability, and decoupled lint checking to prevent future IPC workflow blockages. All changes are local transformations that improve code idioms without altering runtime behavior.

## Architecture
- **MSRV Alignment**: Resolve discrepancy between clippy.toml (1.75.0) and Cargo.toml (1.92.0)
    - **Decision**: Use workspace Cargo.toml `rust-version = "1.92.0"` as single source of truth
    - **Rationale**: Workspace MSRV is authoritative; clippy.toml should match or be removed
    - **Action**: Update clippy.toml to `msrv = "1.92.0"` or remove the line to inherit from Cargo.toml

- Pin to `1.92.0` in all CI jobs: `uses: dtolnay/rust-toolchain@1.92.0`
The solution implements a two-lane CI approach:

1. **Strict Lane (Default)**: Runs Clippy with workspace dependencies to ensure shared crates remain lint-clean
   - Command: `cargo clippy -p flight-ipc --benches --features <features> -- -Dwarnings`
   - Validates that flight-core changes don't break IPC workflows

2. **Unblock Lane (Fallback)**: Runs Clippy with `--no-deps` to isolate IPC-specific lints
   - Command: `cargo clippy -p flight-ipc --no-deps --benches --features <features> -- -Dwarnings`
   - Allows IPC development to continue if transient flight-core issues arise
   - Gated behind workflow input or label

3. **Dedicated Core Job**: Separate CI job for flight-core lint validation
   - Command: `cargo clippy -p flight-core -- -Dwarnings`
   - Runs on both ubuntu-latest and windows-latest
   - Gates merges to ensure core stays clean

### Toolchain Determinism

- **MSRV Alignment**: Resolve discrepancy between clippy.toml (1.75.0) and Cargo.toml (1.89.0)
  - **Decision**: Use workspace Cargo.toml `rust-version = "1.89.0"` as single source of truth
  - **Rationale**: Workspace MSRV is authoritative; clippy.toml should match or be removed
  - **Action**: Update clippy.toml to `msrv = "1.89.0"` or remove the line to inherit from Cargo.toml

- **Toolchain Pinning**: CI must explicitly specify Rust version
  - Pin to `1.89.0` in all CI jobs: `uses: dtolnay/rust-toolchain@1.89.0`
  - Prevents lint drift from automatic toolchain updates
  - Optional: Add nightly cron job to detect future lint drift

## Components and Interfaces

### Refactor Categories

#### 1. Iterator Idioms (for_kv_map, manual_flatten)

**Pattern**: Replace manual iteration patterns with idiomatic iterator methods

**Affected Files**:
- `crates/flight-core/src/profile.rs`
- `crates/flight-core/src/writers.rs` (2 locations)

**Implementation**:
```rust
// Before: for_kv_map
for (_, config) in &mut canonical.axes {
    // use config
}

// After
for config in canonical.axes.values_mut() {
    // use config
}

// Before: manual_flatten
for entry in entries {
    if let Ok(entry) = entry {
        // process entry
    }
}

// After
for entry in entries.flatten() {
    // process entry
}
```

**Verification**: 
- Compiler guarantees identical iteration behavior; no runtime changes
- Note: `.flatten()` silently drops `Err` values, same as `if let Ok(entry)`
- If future error reporting is needed, use `.filter_map(Result::ok)` with logging

#### 2. Range Checking (manual_range_contains)

**Pattern**: Replace manual range checks with `RangeInclusive::contains()`

**Affected Files**:
- `crates/flight-core/src/profile.rs`

**Implementation**:
```rust
// Before
if deadzone < 0.0 || deadzone > MAX_DEADZONE {
    // error handling
}

// After
if !(0.0..=MAX_DEADZONE).contains(&deadzone) {
    // error handling
}
```

**Verification**: Mathematically equivalent; compiler optimizes identically

#### 3. String Construction (useless_format)

**Pattern**: Replace `format!()` on literals with `.to_string()`

**Affected Files**:
- `crates/flight-core/src/watchdog.rs`

**Implementation**:
```rust
// Before
context: format!("Component quarantined due to excessive failures")

// After
context: "Component quarantined due to excessive failures".to_string()
```

**Verification**: Identical heap allocation and string content

#### 4. Parameter Types (ptr_arg)

**Pattern**: Accept `&Path` instead of `&PathBuf` for function parameters without breaking public API

**Affected Files**:
- `crates/flight-core/src/aircraft_switch.rs`

**Implementation Strategy**:

If `load_profile_from_path` is **private** or `pub(crate)`:
```rust
// Direct change - no API impact
async fn load_profile_from_path(base_path: &Path, filename: &str) -> Result<Profile> {
    // implementation
}
```

If `load_profile_from_path` is **public**:
```rust
// Keep existing public API, add internal helper
pub async fn load_profile_from_path(base_path: &PathBuf, filename: &str) -> Result<Profile> {
    load_profile_from_path_impl(base_path.as_path(), filename).await
}

pub(crate) async fn load_profile_from_path_impl(base_path: &Path, filename: &str) -> Result<Profile> {
    // implementation moved here
}
```

**Call Site Updates**: 
- Internal callers should use the `&Path` version directly
- Rust's deref coercion handles `&PathBuf` → `&Path` automatically in most cases
- Explicit `.as_path()` only needed if type inference fails

**Verification**: 
- Check visibility with `cargo public-api -p flight-core` before changes
- If public: wrapper pattern preserves API; `cargo public-api --diff` shows no changes
- If private: direct change is safe; no external impact

#### 5. Rustc Warnings (unused_imports, unused_variables, dead_code, private_interfaces)

**Pattern**: Clean up compiler warnings that fail under `-Dwarnings`

**Affected Files**:
- `crates/flight-core/src/process_detection.rs`
- `crates/flight-core/src/blackbox.rs`
- `crates/flight-core/src/security.rs`
- `crates/flight-core/src/rules.rs`
- `crates/flight-core/src/aircraft_switch.rs`
- Other files as identified by `cargo clippy -p flight-core -- -Dwarnings`

**Implementation**:

**Unused Imports**:
```rust
// Before
use std::path::PathBuf;  // unused on Linux

// After - platform-gated
#[cfg(windows)]
use std::path::PathBuf;
```

**Unused Variables**:
```rust
// Before
fn process(client_info: ClientInfo) {  // client_info unused
    // ...
}

// After - prefix with underscore or use let _
fn process(_client_info: ClientInfo) {
    // ...
}

// Or if intentionally ignored
fn process(client_info: ClientInfo) {
    let _ = client_info;  // explicit ignore
    // ...
}
```

**Dead Code**:
```rust
// Before
fn helper_function() {  // not called in some build configs
    // ...
}

// After - item-scoped allow (avoid crate-level)
#[allow(dead_code)]
fn helper_function() {
    // ...
}
```

**Private Interfaces** (Windows-specific):
```rust
// Before
pub struct WindowsHandle {
    handle: HANDLE,  // HANDLE is private
}

// After - either lower visibility or raise type visibility
pub(crate) struct WindowsHandle {  // preferred
    handle: HANDLE,
}
```

**Platform-Specific Code**:
```rust
// Wrap Windows-only code to avoid Linux warnings
#[cfg(windows)]
mod windows_impl {
    use windows::Win32::System::Threading::*;
    // Windows-specific implementation
}

#[cfg(not(windows))]
mod windows_impl {
    // Stub or alternative implementation
}
```

**Verification**:
- Run `cargo clippy -p flight-core -- -Dwarnings` on both Linux and Windows
- Ensure no warnings remain on either platform
- Verify platform-gated code compiles correctly on target platforms

#### 6. Control Flow Simplification (if_same_then_else, collapsible_if, single_match)

**Pattern**: Combine redundant conditions and simplify match expressions

**Affected Files**:
- `crates/flight-core/src/security/verification.rs`
- `crates/flight-core/src/security.rs` (3 locations)

**Implementation**:
```rust
// Before: if_same_then_else
if has_failures {
    VerificationStatus::Fail
} else if has_warnings && self.config.fail_on_warnings {
    VerificationStatus::Fail
} else if has_warnings {
    VerificationStatus::Warn
} else {
    VerificationStatus::Pass
}

// After
if has_failures || (has_warnings && self.config.fail_on_warnings) {
    VerificationStatus::Fail
} else if has_warnings {
    VerificationStatus::Warn
} else {
    VerificationStatus::Pass
}

// Before: collapsible_if
if self.acl_config.current_user_only {
    if client_info.user_id != get_current_user_id()? {
        return Err(/* ... */);
    }
}

// After
if self.acl_config.current_user_only 
    && client_info.user_id != get_current_user_id()? 
{
    return Err(/* ... */);
}

// Before: single_match
match &manifest.signature {
    SignatureStatus::Signed { valid_until, .. } => {
        // validation logic
    }
    _ => {}
}

// After
if let SignatureStatus::Signed { valid_until, .. } = &manifest.signature {
    // validation logic
}
```

**Verification**: 
- Truth table equivalence for boolean logic
- Identical execution paths (no new branches)
- Same error propagation with `?` operator

## Data Models

No data model changes required. All refactors are local transformations that preserve:
- Struct definitions
- Enum variants
- Type signatures (except `&PathBuf` → `&Path` which is a compatible narrowing)
- Trait implementations

## Error Handling

No error handling changes required. All refactors preserve:
- Error types and variants
- Error propagation paths
- Result return types
- Early returns and `?` operator usage

## Testing Strategy

### Pre-Implementation Validation

1. **Baseline Capture**:
   ```bash
   cargo test -p flight-core > baseline-tests.log
   cargo public-api -p flight-core > baseline-api.txt
   ```

2. **Lint Inventory**:
   ```bash
   cargo clippy -p flight-core -- -Dwarnings 2>&1 | tee clippy-before.log
   ```

### Post-Implementation Validation

1. **Lint Resolution**:
   ```bash
   cargo clippy -p flight-core -- -Dwarnings
   # Must pass with zero warnings
   ```

2. **Test Regression Check**:
   ```bash
   cargo test -p flight-core
   cargo test -p flight-virtual --tests
   # All tests must pass; no new tests added
   ```

3. **Public API Stability**:
   ```bash
   cargo public-api -p flight-core --diff-git origin/main..HEAD
   # Must report: "No changes to the public API"
   ```

4. **Benchmark Compilation**:
   ```bash
   cargo bench -p flight-ipc --features ipc-bench --no-run
   cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run
   # Both must compile successfully
   ```

5. **IPC Workflow Validation**:
   ```bash
   cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings
   cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings
   # Both must pass without errors
   ```

### Platform Coverage

All validation commands must pass on:
- **ubuntu-latest**: Primary Linux CI environment
- **windows-latest**: Windows development and CI environment

### Verification Tools

- **cargo-public-api**: Detects any changes to public interface
  - Install: `cargo install cargo-public-api`
  - Usage: `cargo public-api -p flight-core --diff-git origin/main..HEAD`

- **Clippy**: Lint validation with `-Dwarnings` (treat warnings as errors)
  - Ensures zero tolerance for new warnings

- **cargo test**: Regression detection
  - No test modifications allowed
  - All existing tests must pass

## CI Integration

### Workflow Structure

```yaml
# .github/workflows/ci.yml additions

jobs:
  clippy-core:
    name: Clippy - flight-core
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    # Only run when flight-core changes
    paths:
      - 'crates/flight-core/**'
      - 'Cargo.toml'
      - 'clippy.toml'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.89.0  # Pin to workspace MSRV
      - run: cargo clippy -p flight-core -- -Dwarnings

  clippy-ipc-benches:
    name: Clippy - IPC Benches
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
        mode: [strict, unblock]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.89.0  # Pin to workspace MSRV
      - name: Clippy (strict - with deps)
        if: matrix.mode == 'strict'
        run: |
          cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings
          cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings
      - name: Clippy (unblock - no deps)
        if: matrix.mode == 'unblock'
        run: |
          cargo clippy -p flight-ipc --no-deps --benches --features ipc-bench -- -Dwarnings
          cargo clippy -p flight-ipc --no-deps --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings

  public-api-check:
    name: Public API Stability
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Need full history for diff
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-public-api
        run: cargo install cargo-public-api
      - name: Check public API changes
        run: cargo public-api -p flight-core --diff-git origin/main..HEAD
        continue-on-error: false  # Fail if API changes detected
      # Fallback to nightly if rustdoc-json issues occur
      - name: Retry with nightly (if needed)
        if: failure()
        run: |
          rustup toolchain install nightly
          cargo +nightly public-api -p flight-core --diff-git origin/main..HEAD
```

### Merge Gates

Required checks for PR approval:
1. `clippy-core` (both OS)
2. `clippy-ipc-benches` (strict mode, both OS)
3. `public-api-check`
4. All existing test jobs

Optional checks (informational):
- `clippy-ipc-benches` (unblock mode) - shows fallback status

## Implementation Order

1. **Toolchain Setup**: Update clippy.toml MSRV to 1.89.0 (or remove to inherit from Cargo.toml)
2. **Baseline Capture**: Run `cargo clippy -p flight-core -- -Dwarnings 2>&1 | tee clippy-before.log`
3. **Clippy Idiom Fixes**:
   - Profile.rs: Iterator and range idioms (2 changes)
   - Writers.rs: Iterator idioms (3 changes)
   - Watchdog.rs: String construction (1 change)
   - Security/verification.rs: Control flow (1 change)
   - Security.rs: Control flow (3 changes)
4. **Rustc Warning Sweep**:
   - Remove unused imports (add `#[cfg(windows)]` where needed)
   - Prefix unused variables with `_` or use `let _ = value`
   - Add item-scoped `#[allow(dead_code)]` where required
   - Fix private interface visibility issues
5. **Parameter Type Fix**:
   - Aircraft_switch.rs: Check visibility, apply wrapper pattern if public
6. **Validation**: 
   - Run `cargo clippy -p flight-core -- -Dwarnings` on both platforms
   - Run full test suite
   - Run `cargo public-api -p flight-core --diff-git origin/main..HEAD`
7. **CI Updates**: Add workflow jobs with path filters and toolchain pinning
8. **Documentation**: Create docs/dev/clippy-core.md with complete lint mapping

## Rollback Plan

If issues arise during implementation:

1. **Immediate Unblock**: Add `--no-deps` to IPC clippy steps in CI
   - Allows IPC work to continue
   - Marks flight-core fixes as non-blocking

2. **Partial Rollback**: Revert specific file changes if tests fail
   - Each file's changes are independent
   - Can land fixes incrementally

3. **Full Rollback**: Revert entire PR if public API changes detected
   - Use `cargo public-api` as gate
   - Prevents breaking downstream crates

## Success Criteria

- ✅ All lints from baseline `clippy-before.log` resolved (Clippy idioms + rustc warnings)
- ✅ `cargo clippy -p flight-core -- -Dwarnings` passes on ubuntu-latest and windows-latest
- ✅ IPC bench workflow (task 7.3) passes without `--no-deps` workaround
- ✅ Zero changes to flight-core public API (verified by `cargo public-api --diff`)
- ✅ All existing tests pass with no modifications
- ✅ MSRV aligned: clippy.toml updated to 1.89.0 or removed
- ✅ CI jobs added with path filters and toolchain pinning
- ✅ Documentation created at docs/dev/clippy-core.md with lint mapping
