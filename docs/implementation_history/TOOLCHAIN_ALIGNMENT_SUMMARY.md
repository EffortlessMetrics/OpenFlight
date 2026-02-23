# Toolchain Alignment Summary

## Task 1: Align toolchain and capture baseline - COMPLETED

### 1. MSRV Alignment ✓

**Workspace Cargo.toml:**
- `rust-version = "1.92.0"` (single source of truth)

**clippy.toml:**
- `msrv = "1.92.0"` (already aligned)

**Status:** Both files are already aligned to MSRV 1.92.0. No changes needed.

### 2. CI Toolchain Configuration

**Current State:**
- Main test job (`.github/workflows/ci.yml` line 30): Uses `dtolnay/rust-toolchain@master` with matrix `[stable, 1.92.0]`
- IPC bench job (line 177): Uses `dtolnay/rust-toolchain@stable`
- Other jobs: Mix of `@stable` and `@nightly`

**Note:** Task 12 will update CI workflows to pin toolchain to `dtolnay/rust-toolchain@1.92.0` for all lint jobs.

### 3. Baseline Captures ✓

**clippy-before.log:**
- Created: ✓
- Lines: 889
- Command: `cargo clippy -p flight-core -- -Dwarnings 2>&1 | Tee-Object -FilePath clippy-before.log`
- Contains: 58 errors (26 Clippy lints + 32 rustc warnings)

**Clippy Lint Summary:**
- `for_kv_map`: 2 instances
- `manual_range_contains`: 1 instance
- `manual_flatten`: 2 instances
- `useless_format`: 1 instance
- `ptr_arg`: 1 instance
- `if_same_then_else`: 1 instance
- `collapsible_if`: 20 instances
- `single_match`: 1 instance

**Rustc Warning Summary:**
- `unused_imports`: 10 instances
- `unused_variables`: 2 instances
- `dead_code`: 25 instances

**baseline-api.txt:**
- Created: ✓
- Lines: 7,648
- Command: `cargo public-api -p flight-core > baseline-api.txt`
- Contains: Complete public API surface of flight-core crate

### 4. Requirements Satisfied

- ✓ Requirement 1.4: MSRV aligned and documented
- ✓ Requirement 5.1: Baseline public API captured for comparison

### Next Steps

Proceed to Task 2: Fix Clippy idiom lints in profile.rs
