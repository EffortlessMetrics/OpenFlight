---
doc_id: DOC-REGRESSION-PREVENTION
kind: reference
area: ci
status: active
links:
  requirements: [INF-REQ-5]
  tasks: []
  adrs: []
---

# Regression Prevention Guide

This document describes the regression prevention measures implemented in the OpenFlight workspace to maintain code quality and prevent compilation errors from reoccurring.

## Overview

The OpenFlight project implements multiple layers of regression prevention:

1. **Workspace Dependency Alignment** - Centralized version management
2. **Feature Powerset Testing** - Comprehensive feature combination testing
3. **Strict Clippy Enforcement** - High-quality code standards for core crates
4. **Critical Pattern Verification** - Automated checks for known issues
5. **CI Integration** - Automated enforcement in continuous integration

## Workspace Dependency Alignment

### Strategy

All common dependencies are managed through `[workspace.dependencies]` in the root `Cargo.toml`. This ensures:

- Consistent versions across all crates
- No feature leakage (using `resolver = "2"`)
- Easier dependency updates
- Reduced compilation time

### Key Aligned Dependencies

| Dependency | Version | Notes |
|------------|---------|-------|
| tokio | 1.35 | Async runtime with aligned features |
| futures | 0.3 | Futures ecosystem alignment |
| tonic | 0.14.2 | gRPC framework |
| tonic-build | 0.14.2 | Must match tonic version |
| prost | 0.14.1 | Protocol buffers |
| serde | 1.0 | Serialization framework |
| criterion | 0.5 | Benchmarking framework |

### Verification

Check dependency alignment:
```bash
make check-workspace-deps
```

Or manually:
```bash
# Check tokio versions
grep -r "tokio.*=" Cargo.toml crates/*/Cargo.toml | grep version

# Check futures versions
grep -r "futures.*=" Cargo.toml crates/*/Cargo.toml | grep version

# Check tonic/tonic-build alignment
grep -r "tonic.*=" Cargo.toml crates/*/Cargo.toml | grep version
```

## Feature Powerset Testing

### Purpose

Feature powerset testing ensures that all valid combinations of features compile successfully. This prevents:

- Feature interaction bugs
- Conditional compilation errors
- Missing feature gates

### Implementation

Using `cargo-hack` with depth 2 to balance coverage and CI time:

```bash
cargo hack check --workspace --feature-powerset --depth 2
```

Or via Makefile:
```bash
make feature-powerset
```

### CI Integration

Feature powerset testing runs automatically in CI on every pull request:

```yaml
- name: Run feature powerset testing
  run: cargo hack check --workspace --feature-powerset --depth 2
```

## Strict Clippy Enforcement

### Core Crates

The following crates must pass strict clippy checks with `-D warnings`:

- `flight-core` - Core types and traits
- `flight-axis` - Axis processing engine
- `flight-bus` - Event bus system
- `flight-hid` - HID device handling
- `flight-ipc` - Inter-process communication
- `flight-service` - Service infrastructure
- `flight-simconnect` - SimConnect integration
- `flight-panels` - Panel device support

### Running Locally

```bash
make clippy-strict
```

Or for a specific crate:
```bash
cargo clippy -p flight-core -- -D warnings
```

### CI Integration

Strict clippy checks run in CI for all core crates:

```yaml
- name: Run clippy with strict warnings for core crates
  run: |
    cargo clippy -p flight-core --lib --tests -- -D warnings
    cargo clippy -p flight-axis --lib --tests -- -D warnings
    # ... (all core crates)
```

## Critical Pattern Verification

### Verified Patterns

The following patterns are automatically verified to prevent regression:

#### 1. Profile::merge → Profile::merge_with

**Issue**: `Profile::merge` was renamed to `Profile::merge_with`

**Verification**:
```bash
git grep -n "Profile::merge(" | grep -v "Profile::merge_with"
```

**Expected**: No matches (exit code 1)

#### 2. BlackboxWriter::new without ? operator

**Issue**: `BlackboxWriter::new` returns `T` not `Result<T, E>`, so `?` is incorrect

**Verification**:
```bash
git grep -n "BlackboxWriter::new.*?"
```

**Expected**: No matches (exit code 1)

#### 3. Engine::new signature (2 arguments)

**Issue**: `Engine::new` requires `(name: String, config: EngineConfig)`

**Verification**:
```bash
git grep -n "Engine::new(" | grep -v ","
```

**Expected**: No matches (exit code 1)

#### 4. std::hint::black_box instead of criterion::black_box

**Issue**: `criterion::black_box` is deprecated in favor of `std::hint::black_box`

**Verification**:
```bash
git grep -n "criterion::black_box"
```

**Expected**: No matches (exit code 1)

#### 5. Workspace dependency alignment

**Issue**: Crates must use workspace dependencies for common crates

**Verification**:
```bash
# Check tokio
grep -r "tokio.*=" crates/*/Cargo.toml | grep -v "workspace = true" | grep -v "features\|optional"

# Check futures
grep -r "futures.*=" crates/*/Cargo.toml | grep -v "workspace = true" | grep -v "features\|optional"
```

**Expected**: No matches (exit code 1)

### Running All Pattern Checks

```bash
make verify-patterns
```

Or use the regression prevention script:
```bash
cargo +nightly -Zscript scripts/regression_prevention.rs verify-patterns
```

## Dead Code Cleanup

### Purpose

Automatically remove unused code and imports to maintain code cleanliness.

### Running Cleanup

```bash
make dead-code-cleanup
```

Or directly:
```bash
cargo fix --workspace --allow-dirty
```

**Note**: This modifies files in place. Review changes before committing.

## Regression Prevention Script

A comprehensive Rust script is available for running all checks:

```bash
cargo +nightly -Zscript scripts/regression_prevention.rs [command]
```

### Available Commands

- `feature-powerset` - Run feature powerset testing
- `clippy-strict` - Run strict clippy checks on core crates
- `dead-code-cleanup` - Clean up dead code and imports
- `verify-patterns` - Verify critical patterns are fixed
- `all` - Run all checks (default)

### Example Usage

```bash
# Run all checks
cargo +nightly -Zscript scripts/regression_prevention.rs

# Run specific check
cargo +nightly -Zscript scripts/regression_prevention.rs clippy-strict
```

## Makefile Targets

The Makefile provides convenient targets for local development:

### Quick Reference

```bash
make all                  # Run all regression prevention checks
make quick                # Run quick checks (clippy + patterns)
make feature-powerset     # Run feature powerset testing
make clippy-strict        # Run strict clippy on core crates
make dead-code-cleanup    # Clean up dead code and imports
make verify-patterns      # Verify critical patterns
make check-workspace-deps # Check dependency alignment
make ci-simulation        # Run full CI simulation locally
make help                 # Show all available targets
```

### Recommended Workflow

Before pushing code:

```bash
# Quick check during development
make quick

# Full check before pushing
make all

# Simulate CI locally (includes tests and build)
make ci-simulation
```

## CI Integration

### Workflow Structure

The CI workflow (`.github/workflows/ci.yml`) includes:

1. **Test Suite** - Runs on Ubuntu and Windows with multiple Rust versions
2. **Security Audit** - Dependency security checks
3. **Build Release** - Release builds for both platforms
4. **Feature Powerset** - Comprehensive feature testing
5. **Performance Tests** - Benchmark execution

### Critical Pattern Verification in CI

The CI workflow includes a dedicated step for pattern verification:

```yaml
- name: Verify critical patterns are fixed
  run: |
    # Check Profile::merge is replaced with Profile::merge_with
    if git grep -n "Profile::merge(" | grep -v "Profile::merge_with"; then
      echo "❌ Found Profile::merge( calls"
      exit 1
    fi
    
    # Check BlackboxWriter::new doesn't have ? operator
    if git grep -n "BlackboxWriter::new.*?"; then
      echo "❌ Found BlackboxWriter::new with ? operator"
      exit 1
    fi
    
    # ... (additional checks)
```

### Platform Matrix

Tests run on:
- **OS**: Ubuntu Latest, Windows Latest
- **Rust**: Stable, MSRV (1.89.0)

This ensures cross-platform compatibility and MSRV compliance.

## Adding New Regression Checks

### 1. Identify the Pattern

Document the issue and the correct pattern:

```markdown
**Issue**: Description of what went wrong
**Correct Pattern**: How it should be done
**Verification**: Command to check for the issue
```

### 2. Add to verify-patterns Target

Update `Makefile`:

```makefile
verify-patterns:
    @echo "  Checking new pattern..."
    @if git grep -n "bad_pattern"; then \
        echo "❌ Found bad_pattern usage"; \
        exit 1; \
    fi
```

### 3. Add to Regression Prevention Script

Update `scripts/regression_prevention.rs`:

```rust
fn verify_critical_patterns() {
    // ... existing checks ...
    
    // New pattern check
    let new_pattern_check = Command::new("git")
        .args(&["grep", "-n", "bad_pattern"])
        .output()
        .expect("Failed to run git grep");
        
    if new_pattern_check.status.success() && !new_pattern_check.stdout.is_empty() {
        eprintln!("❌ Found bad_pattern usage:");
        eprintln!("{}", String::from_utf8_lossy(&new_pattern_check.stdout));
        exit(1);
    }
}
```

### 4. Add to CI Workflow

Update `.github/workflows/ci.yml`:

```yaml
- name: Verify critical patterns are fixed
  run: |
    # ... existing checks ...
    
    # New pattern check
    if git grep -n "bad_pattern"; then
      echo "❌ Found bad_pattern usage"
      exit 1
    fi
```

### 5. Document in This Guide

Add the new pattern to the "Critical Pattern Verification" section above.

## Best Practices

### For Developers

1. **Run quick checks frequently** during development:
   ```bash
   make quick
   ```

2. **Run full checks before pushing**:
   ```bash
   make all
   ```

3. **Use workspace dependencies** for common crates:
   ```toml
   [dependencies]
   tokio = { workspace = true, features = ["macros"] }
   ```

4. **Test feature combinations** when adding new features:
   ```bash
   cargo hack check -p your-crate --feature-powerset
   ```

### For Maintainers

1. **Review CI failures carefully** - they often indicate real issues
2. **Update regression checks** when fixing bugs
3. **Keep dependency versions aligned** in workspace Cargo.toml
4. **Document new patterns** in this guide

## Troubleshooting

### cargo-hack not installed

```bash
cargo install cargo-hack
```

Or let the Makefile install it automatically:
```bash
make check-deps
```

### Pattern check false positives

If a pattern check incorrectly flags valid code:

1. Review the grep pattern for accuracy
2. Add exclusions if needed (e.g., `grep -v "valid_usage"`)
3. Document the exception in this guide

### CI failures on Windows

Windows uses different shell syntax. Ensure CI scripts use:
- PowerShell-compatible commands
- Cross-platform tools (cargo, git)
- Proper path separators

### Feature powerset timeout

If feature powerset testing takes too long:

1. Reduce depth: `--depth 1` instead of `--depth 2`
2. Exclude problematic crates: `--exclude problematic-crate`
3. Run on specific crates: `-p specific-crate`

## References

- [Cargo Hack Documentation](https://github.com/taiki-e/cargo-hack)
- [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)
- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Feature Resolver v2](https://doc.rust-lang.org/cargo/reference/resolver.html#feature-resolver-version-2)

## Maintenance

This document should be updated when:

- New regression checks are added
- Critical patterns change
- CI workflow is modified
- New tools are introduced

Last updated: 2025-10-23
