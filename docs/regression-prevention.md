# Regression Prevention Guide

This document describes the regression prevention measures implemented in the OpenFlight workspace to maintain code quality and prevent compilation errors from reoccurring.

## Overview

The regression prevention system consists of several layers:

1. **Workspace Dependency Alignment** - Ensures consistent versions across crates
2. **Feature Powerset Testing** - Validates all feature combinations work
3. **Strict Clippy Enforcement** - Maintains code quality for core crates
4. **Critical Pattern Verification** - Prevents specific known issues
5. **Automated CI Checks** - Runs all checks on every PR

## Quick Start

### Local Development

Run regression checks locally before pushing:

```bash
# Run all checks
make all

# Quick checks during development
make quick

# Individual checks
make feature-powerset
make clippy-strict
make verify-patterns
```

### Using the Regression Script

```bash
# Run all checks
cargo +nightly -Zscript scripts/regression_prevention.rs

# Run specific checks
cargo +nightly -Zscript scripts/regression_prevention.rs feature-powerset
cargo +nightly -Zscript scripts/regression_prevention.rs clippy-strict
cargo +nightly -Zscript scripts/regression_prevention.rs verify-patterns
```

## Regression Prevention Measures

### 1. Workspace Dependency Alignment

**Purpose**: Prevent version conflicts and ensure consistent behavior across crates.

**Implementation**:
- All common dependencies (tokio, futures, tonic, etc.) are defined in workspace `Cargo.toml`
- Individual crates use `{ workspace = true }` to inherit versions
- CI verifies no crates use non-workspace versions

**Aligned Dependencies**:
- `tokio = "1.35"` - Async runtime
- `futures = "0.3"` - Futures utilities  
- `futures-core = "0.3"` - Core futures traits
- `tokio-stream = "0.1"` - Stream utilities
- `tokio-util = "0.7"` - Tokio utilities
- `tonic = "0.14.2"` - gRPC framework
- `tonic-build = "0.14.2"` - gRPC code generation

### 2. Feature Powerset Testing

**Purpose**: Ensure all feature combinations compile and work correctly.

**Implementation**:
```bash
cargo hack check --workspace --feature-powerset --depth 2
```

**Coverage**:
- Tests all combinations of features up to depth 2
- Validates optional dependencies work correctly
- Catches feature interaction bugs early

**CI Integration**: Runs on every PR in dedicated job

### 3. Strict Clippy Enforcement

**Purpose**: Maintain high code quality for core crates that other crates depend on.

**Core Crates** (must pass `-D warnings`):
- `flight-core` - Core types and traits
- `flight-axis` - Axis processing engine
- `flight-bus` - Event bus system
- `flight-hid` - HID device interface
- `flight-ipc` - Inter-process communication
- `flight-service` - Service framework
- `flight-simconnect` - SimConnect integration
- `flight-panels` - Panel management

**Non-Core Crates**: Allow warnings for development ergonomics

### 4. Critical Pattern Verification

**Purpose**: Prevent specific compilation errors that have occurred before.

**Verified Patterns**:

#### Profile API Usage
```bash
# ❌ Old API (causes compilation error)
Profile::merge(base, overlay)

# ✅ New API (correct)
Profile::merge_with(base, overlay)
```

#### BlackboxWriter Constructor
```bash
# ❌ Incorrect (if constructor doesn't return Result)
let writer = BlackboxWriter::new(config)?;

# ✅ Correct
let writer = BlackboxWriter::new(config);
```

#### Engine Constructor Signature
```bash
# ❌ Old signature
Engine::new(config)

# ✅ New signature
Engine::new(name, config)
```

#### Benchmark Black Box Usage
```bash
# ❌ Old Criterion API
criterion::black_box(value)

# ✅ New std API
std::hint::black_box(value)
```

#### Packed Struct Safety
```bash
# ❌ Unsafe (creates unaligned reference)
let value = &packed_struct.field;

# ✅ Safe (copy by value)
let value = packed_struct.field;
let reference = &value;
```

### 5. Dead Code Cleanup

**Purpose**: Remove unused code and imports to reduce maintenance burden.

**Implementation**:
```bash
cargo fix --workspace --allow-dirty
```

**What it fixes**:
- Unused imports
- Dead code
- Unnecessary `mut` keywords
- Other automatic fixes

## CI Integration

### Workflow Jobs

1. **Test Suite** (`test`)
   - Runs on Ubuntu and Windows
   - Includes critical pattern verification
   - Strict clippy checks for core crates

2. **Feature Powerset** (`feature-powerset`)
   - Runs feature powerset testing
   - Verifies workspace dependency alignment
   - Ubuntu only (faster feedback)

3. **Security Audit** (`security`)
   - Dependency vulnerability scanning
   - HTTP stack unification validation

4. **Build Release** (`build`)
   - Cross-platform release builds
   - Artifact generation

5. **Performance** (`performance`)
   - Benchmark execution
   - Performance regression detection (TODO)

### Pattern Verification in CI

The CI runs these checks on every PR:

```bash
# Profile::merge usage
git grep -n "Profile::merge(" | grep -v "Profile::merge_with"

# BlackboxWriter::new usage  
git grep -n "BlackboxWriter::new.*?"

# Engine::new signature
git grep -n "Engine::new(" | grep -v ","

# Criterion black_box usage
git grep -n "criterion::black_box"

# Workspace dependency alignment
grep -r "tokio.*=" crates/*/Cargo.toml | grep -v "workspace = true"
```

## Adding New Regression Prevention

### For New API Changes

1. **Add Pattern Check**: Update `scripts/regression_prevention.rs` with new pattern
2. **Update CI**: Add check to `.github/workflows/ci.yml`
3. **Update Makefile**: Add target to `Makefile`
4. **Document**: Add to this guide

### For New Core Crates

1. **Add to Strict Clippy**: Update core crates list in CI and Makefile
2. **Feature Testing**: Ensure crate participates in feature powerset testing
3. **Dependency Alignment**: Move common dependencies to workspace level

### For New Dependencies

1. **Workspace Level**: Add to `[workspace.dependencies]` in root `Cargo.toml`
2. **Version Alignment**: Update all crates to use `{ workspace = true }`
3. **CI Verification**: Add to dependency alignment checks

## Troubleshooting

### Feature Powerset Failures

**Symptom**: `cargo hack check --workspace --feature-powerset --depth 2` fails

**Common Causes**:
- Feature combinations that don't compile together
- Missing optional dependencies
- Conflicting feature flags

**Solutions**:
- Use `--exclude` to skip problematic crates temporarily
- Add `[features]` constraints in `Cargo.toml`
- Fix feature interaction bugs

### Clippy Strict Failures

**Symptom**: Core crate fails `cargo clippy -p <crate> -- -D warnings`

**Common Causes**:
- New clippy lints in Rust updates
- Code quality regressions
- Platform-specific warnings

**Solutions**:
- Fix the underlying issue (preferred)
- Add targeted `#[allow(clippy::lint_name)]` if justified
- Move crate out of core list if quality requirements too strict

### Pattern Verification Failures

**Symptom**: CI fails on pattern verification step

**Common Causes**:
- Reintroduction of old API usage
- New code using deprecated patterns
- False positives from grep patterns

**Solutions**:
- Fix the code to use correct patterns
- Update pattern if API legitimately changed
- Refine grep pattern to reduce false positives

### Dependency Alignment Failures

**Symptom**: CI fails on workspace dependency alignment

**Common Causes**:
- New crate added with inline dependency versions
- Dependency version bumped in individual crate
- New dependency not added to workspace

**Solutions**:
- Move dependency to workspace level
- Update all crates to use `{ workspace = true }`
- Align versions across workspace

## Performance Considerations

### CI Time Impact

- **Feature Powerset**: ~5-10 minutes (depth 2 limit)
- **Strict Clippy**: ~2-3 minutes (core crates only)
- **Pattern Verification**: ~30 seconds (grep operations)
- **Total Overhead**: ~8-14 minutes per PR

### Local Development Impact

- **Quick Checks**: ~2-3 minutes (`make quick`)
- **Full Checks**: ~8-12 minutes (`make all`)
- **Incremental**: Most checks are incremental and cache-friendly

### Optimization Strategies

1. **Parallel Execution**: CI jobs run in parallel
2. **Selective Checking**: Only core crates get strict treatment
3. **Depth Limiting**: Feature powerset limited to depth 2
4. **Caching**: Rust compilation cache reduces repeated work

## Future Improvements

### Planned Enhancements

1. **Performance Regression Detection**: Automated benchmark comparison
2. **Dependency Update Automation**: Automated dependency updates with testing
3. **Custom Lints**: Project-specific clippy lints for domain rules
4. **Integration Testing**: Cross-crate integration test automation

### Metrics and Monitoring

1. **Build Time Tracking**: Monitor CI build time trends
2. **Pattern Violation Frequency**: Track how often patterns are violated
3. **Feature Combination Coverage**: Measure feature testing coverage
4. **Code Quality Trends**: Track clippy warning trends over time

## References

- [Cargo Hack Documentation](https://github.com/taiki-e/cargo-hack)
- [Clippy Lint Reference](https://rust-lang.github.io/rust-clippy/)
- [Cargo Workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [GitHub Actions Workflow Syntax](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions)