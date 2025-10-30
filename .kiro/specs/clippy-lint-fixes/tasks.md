# Implementation Plan

- [ ] 1. Align toolchain and capture baseline
  - Update clippy.toml MSRV to match workspace Cargo.toml (1.89.0) or remove msrv line to inherit
  - Run `cargo clippy -p flight-core -- -Dwarnings 2>&1 | tee clippy-before.log` to capture all current warnings
  - Run `cargo public-api -p flight-core > baseline-api.txt` to capture current public API
  - _Requirements: 1.4, 5.1_

- [ ] 2. Fix Clippy idiom lints in profile.rs
  - Change `for (_, config) in &mut canonical.axes` to `for config in canonical.axes.values_mut()`
  - Change `if deadzone < 0.0 || deadzone > MAX_DEADZONE` to `if !(0.0..=MAX_DEADZONE).contains(&deadzone)`
  - _Requirements: 2.1, 2.2_

- [ ] 3. Fix Clippy idiom lints in writers.rs
  - Change first `for entry in entries { if let Ok(entry) = entry` to `for entry in entries.flatten()` (around line 159)
  - Change `for (_, value) in &diff.changes` to `for value in diff.changes.values()` (around line 464)
  - Change second `for entry in entries { if let Ok(entry) = entry` to `for entry in entries.flatten()` (around line 687)
  - _Requirements: 2.3, 2.4_

- [ ] 4. Fix Clippy idiom lints in watchdog.rs
  - Change `format!("Component quarantined due to excessive failures")` to `"Component quarantined due to excessive failures".to_string()`
  - _Requirements: 2.5_

- [ ] 5. Fix Clippy idiom lints in security/verification.rs
  - Combine identical if branches: change `if has_failures { Fail } else if has_warnings && fail_on_warnings { Fail }` to `if has_failures || (has_warnings && fail_on_warnings) { Fail }`
  - _Requirements: 3.1_

- [ ] 6. Fix Clippy idiom lints in security.rs
  - Collapse nested if for current_user_only check: `if current_user_only && user_id != get_current_user_id()` (around line 384)
  - Collapse nested if for allowed_users check: `if !allowed_users.is_empty() && !allowed_users.contains(&user_id)` (around line 393)
  - Change match to if let: `if let SignatureStatus::Signed { valid_until, .. } = &manifest.signature` (around line 431)
  - _Requirements: 3.2, 3.3_

- [ ] 7. Fix rustc warnings - unused imports and variables
  - Review clippy-before.log for unused_imports warnings
  - Add `#[cfg(windows)]` to Windows-specific imports to avoid Linux warnings
  - Remove genuinely unused imports
  - Prefix unused function parameters with `_` or use `let _ = value` for intentional ignores
  - Files likely affected: process_detection.rs, blackbox.rs, security.rs, rules.rs, aircraft_switch.rs
  - _Requirements: 4.1, 4.2_

- [ ] 8. Fix rustc warnings - dead code and private interfaces
  - Add item-scoped `#[allow(dead_code)]` for symbols required for parity but not referenced in some builds
  - Fix private interface visibility issues (prefer lowering method visibility over raising type visibility)
  - Ensure Windows-specific code is properly gated with `#[cfg(windows)]`
  - _Requirements: 4.1, 4.2_

- [ ] 9. Fix ptr_arg lint in aircraft_switch.rs
  - Check if `load_profile_from_path` is public using `cargo public-api -p flight-core | grep load_profile_from_path`
  - If private/pub(crate): change parameter from `&PathBuf` to `&Path` directly
  - If public: keep existing signature, add `load_profile_from_path_impl(&Path)` helper, delegate to it
  - Update internal call sites to use `&Path` version
  - _Requirements: 2.6, 2.7, 5.2, 5.3_

- [ ] 10. Validate all fixes
  - Run `cargo clippy -p flight-core -- -Dwarnings` on current platform (must pass with zero warnings)
  - Run `cargo test -p flight-core` (all tests must pass, no new tests)
  - Run `cargo test -p flight-virtual --tests` (all tests must pass)
  - Run `cargo bench -p flight-ipc --features ipc-bench --no-run` (must compile)
  - Run `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run` (must compile)
  - Run `cargo public-api -p flight-core --diff-git origin/main..HEAD` (must show no changes)
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4_

- [ ] 11. Validate IPC bench workflow
  - Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` (must pass)
  - Run `cargo clippy -p flight-ipc --benches --features "ipc-bench,ipc-bench-serde" -- -Dwarnings` (must pass)
  - Verify task 7.3 from IPC bench workflow now passes without `--no-deps` workaround
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 12. Update CI workflows
  - Add `clippy-core` job with ubuntu-latest and windows-latest matrix
  - Add path filters for `crates/flight-core/**`, `Cargo.toml`, `clippy.toml`
  - Pin toolchain to `dtolnay/rust-toolchain@1.89.0` in all lint jobs
  - Add `clippy-ipc-benches` job with strict and unblock modes
  - Add `public-api-check` job with nightly fallback
  - Configure required checks for PR approval
  - _Requirements: 1.4, 1.5_

- [ ] 13. Create documentation
  - Create `docs/dev/clippy-core.md` with complete lint-to-patch mapping
  - Include all addressed lint rules: for_kv_map, manual_range_contains, manual_flatten, useless_format, ptr_arg, if_same_then_else, collapsible_if, single_match
  - Include rustc warnings addressed: unused_imports, unused_variables, dead_code, private_interfaces
  - Document file locations, line numbers, and refactor patterns applied
  - _Requirements: 6.1, 6.2, 6.3, 6.4_
