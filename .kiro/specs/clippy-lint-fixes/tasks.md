# Implementation Plan

- [ ] 1. Align toolchain and capture baseline
  - Choose workspace Cargo.toml `rust-version = "1.89.0"` as single source of truth
  - Update clippy.toml: set `msrv = "1.89.0"` or remove the msrv line entirely to inherit from Cargo.toml
  - Verify toolchain will be pinned to `dtolnay/rust-toolchain@1.89.0` in all CI lint jobs
  - Run `cargo clippy -p flight-core -- -Dwarnings 2>&1 | tee clippy-before.log` to capture all current warnings
  - Run `cargo public-api -p flight-core > baseline-api.txt` to capture current public API
  - _Requirements: 1.4, 5.1_

- [ ] 2. Fix Clippy idiom lints in profile.rs
  - Change `for (_, config) in &mut canonical.axes` to `for config in canonical.axes.values_mut()`
  - Change `if deadzone < 0.0 || deadzone > MAX_DEADZONE` to `if !(0.0..=MAX_DEADZONE).contains(&deadzone)`
  - _Requirements: 2.1, 2.2_

- [ ] 3. Fix Clippy idiom lints in writers.rs
  - Change first `for entry in entries { if let Ok(entry) = entry` to `for entry in entries.flatten()` (around line 159)
  - Add comment: "Intentionally ignore read errors; preserves prior behavior"
  - Change `for (_, value) in &diff.changes` to `for value in diff.changes.values()` (around line 464)
  - Change second `for entry in entries { if let Ok(entry) = entry` to `for entry in entries.flatten()` (around line 687)
  - Add comment: "Intentionally ignore read errors; preserves prior behavior"
  - _Requirements: 2.3, 2.4_

- [ ] 4. Fix Clippy idiom lints in watchdog.rs
  - Change `format!("Component quarantined due to excessive failures")` to `"Component quarantined due to excessive failures".to_string()`
  - _Requirements: 2.5_

- [ ] 5. Fix Clippy idiom lints in security/verification.rs
  - Combine identical if branches: change `if has_failures { Fail } else if has_warnings && fail_on_warnings { Fail }` to `if has_failures || (has_warnings && fail_on_warnings) { Fail }`
  - Verify by truth table: both conditions lead to Fail; else-chain unchanged (Warn if has_warnings, Pass otherwise)
  - Ensure no change in execution paths or return values
  - _Requirements: 3.1_

- [ ] 6. Fix Clippy idiom lints in security.rs
  - Collapse nested if for current_user_only check: `if current_user_only && user_id != get_current_user_id()` (around line 384)
  - Collapse nested if for allowed_users check: `if !allowed_users.is_empty() && !allowed_users.contains(&user_id)` (around line 393)
  - Change match to if let: `if let SignatureStatus::Signed { valid_until, .. } = &manifest.signature` (around line 431)
  - Verify by truth table: combined conditions are logically equivalent; error propagation with `?` unchanged
  - _Requirements: 3.2, 3.3_

- [ ] 7. Fix rustc warnings - unused imports and variables
  - Review clippy-before.log for all unused_imports and unused_variables warnings
  - Add `#[cfg(windows)]` (or `#[cfg(target_os = "windows")]`) to Windows-specific imports (e.g., GetModuleFileNameExW, HANDLE, Windows API types)
  - Add `#[cfg(unix)]` for Unix-specific imports where symmetric
  - Remove genuinely unused imports
  - Prefix unused function parameters with `_` (e.g., `_client_info`) or use `let _ = value;` for intentional ignores
  - Files explicitly affected: process_detection.rs, blackbox.rs, security.rs, rules.rs, aircraft_switch.rs (check clippy-before.log for complete list)
  - _Requirements: 4.1, 4.2_

- [ ] 8. Fix rustc warnings - dead code and private interfaces
  - Add item-scoped `#[allow(dead_code)]` only for symbols required for cross-platform parity but not referenced in some builds
  - Avoid crate-level allows; keep allows targeted to specific items
  - Fix private_interfaces lint (especially in flight-hid): prefer lowering method visibility (e.g., `pub` → `pub(crate)`) over raising type visibility
  - Ensure Windows-specific types and functions are properly gated with `#[cfg(windows)]`
  - _Requirements: 4.1, 4.2_

- [ ] 9. Fix ptr_arg lint in aircraft_switch.rs
  - Check if `load_profile_from_path` is public using `cargo public-api -p flight-core | grep load_profile_from_path`
  - If private/pub(crate): change parameter from `&PathBuf` to `&Path` directly
  - If public: 
    - Keep existing signature with `&PathBuf` parameter
    - Add `#[deprecated(since = "0.1.0", note = "Use internal implementation with &Path")]` to existing function
    - Create `load_profile_from_path_impl(base_path: &Path, filename: &str)` as `pub(crate)` helper
    - Delegate public function to helper: `load_profile_from_path_impl(base_path.as_path(), filename).await`
  - Update all internal call sites to use `&Path` version directly
  - _Requirements: 2.6, 2.7, 5.2, 5.3_

- [ ] 10. Validate all fixes
  - Run `cargo clippy -p flight-core -- -Dwarnings` on current platform (must pass with zero warnings)
  - Run `cargo test -p flight-core` (all tests must pass, no new tests)
  - Run `cargo test -p flight-virtual --tests` (all tests must pass)
  - Run `cargo bench -p flight-ipc --features ipc-bench --no-run` (must compile)
  - Run `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run` (must compile)
  - Run `cargo public-api -p flight-core --diff-git origin/main..HEAD` (must show no changes, or only deprecation additions)
  - Run `cargo fmt --all -- --check` (ensure formatting is clean)
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4_

- [ ] 10.1 Optional: Run feature sweep for drift detection

  - Run `cargo clippy -p flight-core --all-targets --all-features -- -Dwarnings`
  - This catches warnings that only appear under different feature/target combinations
  - Can be run as nightly cron job instead of blocking PR
  - _Requirements: 4.1_

- [ ] 11. Validate IPC bench workflow
  - Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` (must pass)
  - Run `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings` (must pass)
  - Verify task 7.3 from IPC bench workflow now passes without `--no-deps` workaround
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 12. Update CI workflows
  - Add `clippy-core` job with ubuntu-latest and windows-latest matrix
  - Add path filters: `crates/flight-core/**`, `Cargo.toml`, `clippy.toml`
  - Pin toolchain to `dtolnay/rust-toolchain@1.89.0` in all lint jobs (clippy-core, clippy-ipc-benches)
  - Add `clippy-ipc-benches` job with strict mode (with deps) and unblock mode (--no-deps, label-gated)
  - Add `public-api-check` job on ubuntu-latest only (not Windows) with nightly fallback if rustdoc-json fails
  - Add `cargo fmt --all -- --check` step to clippy-core job or as separate formatting job
  - Add caching for cargo registry, target directory, and cargo-public-api binary
  - Configure required checks for PR approval: clippy-core (both OS), clippy-ipc-benches (strict, both OS), public-api-check
  - _Requirements: 1.4, 1.5_

- [ ] 13. Create documentation
  - Create `docs/dev/clippy-core.md` with complete lint-to-patch mapping
  - Include all addressed Clippy lint rules: for_kv_map, manual_range_contains, manual_flatten, useless_format, ptr_arg, if_same_then_else, collapsible_if, single_match
  - Include all addressed rustc warnings: unused_imports, unused_variables, dead_code, private_interfaces
  - Document file locations, approximate line numbers, and refactor patterns applied
  - Include rationale for platform-gating decisions (#[cfg(windows)] usage)
  - Document deprecation strategy for public API changes (if applicable)
  - _Requirements: 6.1, 6.2, 6.3, 6.4_


## Implementation Notes

### Commit Sequencing for Review Hygiene

For easier code review, consider splitting commits by file/topic:

1. **Toolchain alignment**: Update clippy.toml MSRV and CI configuration
2. **profile.rs idioms**: Iterator and range refactors
3. **writers.rs idioms**: Flatten and values() refactors
4. **watchdog.rs idioms**: String literal fix
5. **security/*.rs control-flow**: Boolean logic simplification
6. **Platform-gated warnings**: Add #[cfg(windows)], remove unused imports
7. **Dead code and visibility**: Item-scoped allows and visibility fixes
8. **ptr_arg fix**: Add helper, deprecate old API (if public)
9. **CI and documentation**: Workflow updates and docs/dev/clippy-core.md

This keeps diffs obviously mechanical and easier to verify.

### Validation Checklist

Before marking complete, ensure:
- ✅ `cargo clippy -p flight-core -- -Dwarnings` passes on both platforms
- ✅ `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` passes
- ✅ `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings` passes
- ✅ All tests pass: `cargo test -p flight-core` and `cargo test -p flight-virtual --tests`
- ✅ Benchmarks compile: both IPC feature combinations with `--no-run`
- ✅ Public API unchanged: `cargo public-api -p flight-core --diff-git origin/main..HEAD`
- ✅ Formatting clean: `cargo fmt --all -- --check`
- ✅ CI jobs configured with path filters and toolchain pinning
- ✅ Documentation created with complete lint mapping
