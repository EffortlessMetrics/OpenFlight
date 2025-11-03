# Design Document

## Overview

This design addresses critical repository health issues preventing a "properly working" state. The approach prioritizes correctness (failing tests), API hygiene (private_interfaces), configuration consistency (MSRV/edition alignment), and CI robustness. All changes are targeted fixes with minimal scope to reduce risk and enable incremental progress.

## Architecture

### Fix Categories

The fixes are organized into four categories based on impact and dependencies:

1. **Correctness Fixes** (Highest Priority)
   - Aircraft auto-switch test failures
   - flight-virtual abnormal exits
   - Meaningful test assertions

2. **API Hygiene**
   - flight-hid private_interfaces warnings
   - IPC bench lint strictness

3. **Configuration Consistency**
   - rustfmt stable compatibility
   - MSRV/edition alignment

4. **Infrastructure**
   - CI hardening
   - Documentation completeness

## Components and Interfaces

### 1. Aircraft Auto-Switch Test Fixes

#### Problem Analysis

**Root Causes**:
1. PhaseOfFlight classification prioritizes ground phases (Taxi) over airborne phases (Cruise)
2. Tests expect C172 profile but no fixture exists
3. Metrics counters not incremented on switch commit/force

#### Solution Design

**1.1 Reorder PhaseOfFlight Classification**

**File**: `crates/flight-core/src/aircraft_switch.rs`

**Current Logic** (problematic):
```rust
fn classify_phase(s: &Snapshot, t: &PofThresholds) -> PhaseOfFlight {
    if s.emergency { return PhaseOfFlight::Emergency; }
    
    // Ground phases checked early - can shadow airborne phases
    if s.on_ground && s.ground_speed < t.taxi_speed_max { 
        return PhaseOfFlight::Taxi; 
    }
    
    // Airborne phases checked later
    if s.alt_agl >= t.cruise_agl_min && ... {
        return PhaseOfFlight::Cruise;
    }
    // ...
}
```

**New Logic** (correct priority):
```rust
fn classify_phase(s: &Snapshot, t: &PofThresholds) -> PhaseOfFlight {
    if s.emergency { return PhaseOfFlight::Emergency; }
    
    // High-energy phases first (unambiguous)
    if s.on_runway && s.ias >= t.takeoff_ias_min { 
        return PhaseOfFlight::Takeoff; 
    }
    if s.vs > t.climb_vs_min && s.alt_agl > t.climb_agl_min { 
        return PhaseOfFlight::Climb; 
    }
    
    // Cruise (requires altitude + stable VS + speed)
    if s.alt_agl >= t.cruise_agl_min
        && s.vs.abs() <= t.cruise_vs_abs_max
        && s.ias >= t.cruise_ias_min {
        return PhaseOfFlight::Cruise;
    }
    
    // Descent
    if s.vs < -t.descent_vs_min { 
        return PhaseOfFlight::Descent; 
    }
    
    // Approach
    if s.alt_agl <= t.approach_agl_max && s.ias <= t.approach_ias_max {
        return PhaseOfFlight::Approach;
    }
    
    // Ground-only phases last (only when clearly on ground)
    if s.on_runway && s.ground_contact { 
        return PhaseOfFlight::Landing; 
    }
    if s.on_ground && s.ground_speed < t.taxi_speed_max { 
        return PhaseOfFlight::Taxi; 
    }
    
    PhaseOfFlight::Park
}
```

**Rationale**: 
- Airborne phases have stricter criteria (altitude, vertical speed, airspeed)
- Ground phases should only match when unambiguously on ground
- Prevents Taxi from capturing cruise conditions with permissive ground detection

**1.2 Provide Test Fixture for C172**

**Option A: Fixture Directory** (Recommended - keeps library clean)

```rust
#[cfg(test)]
fn test_profile_repo() -> ProfileRepo {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/profiles");
    ProfileRepo::new(fixture_dir)
}

// In tests:
#[test]
fn test_auto_switch_c172() {
    let repo = test_profile_repo();
    // ... use repo with C172.json fixture
}
```

**Option B: Dependency Injection** (Most flexible)

```rust
trait ProfileSource {
    fn load(&self, id: &str) -> Result<CompiledProfile>;
}

struct FilesystemSource { /* ... */ }
struct InMemorySource { /* ... */ }

#[cfg(test)]
fn test_profile_source() -> InMemorySource {
    let mut source = InMemorySource::new();
    source.add("C172", CompiledProfile { /* ... */ });
    source
}
```

**Decision**: Use Option A (fixture directory) to keep the library free of test-only content. Avoids embedding JSON in library code and keeps tests readable. Can add multi-aircraft fixtures without growing the binary.

**Rationale**:
- Keeps production code clean
- Reduces link-time overhead
- Makes test data easy to inspect and modify
- Follows standard Rust testing patterns

**1.3 Increment Metrics on Switch**

**File**: `crates/flight-core/src/aircraft_switch.rs`

**Semantics Decision**: A committed switch is counted when:
1. The active profile pointer changes (different aircraft ID), OR
2. A `force_switch` operation explicitly records a committed switch

**Current Code** (missing increment):
```rust
fn commit_switch(&mut self, new_profile: &CompiledProfile) -> Result<()> {
    self.current_profile = Some(new_profile.clone());
    // Missing: self.metrics.committed_switches += 1;
    Ok(())
}

pub fn force_switch(&mut self, id: &AircraftId) -> Result<()> {
    let profile = self.resolve_profile(id)?;
    
    // Early return prevents metrics increment
    if self.current_profile.as_ref().map(|p| &p.id) == Some(id) {
        return Ok(());
    }
    
    self.commit_switch(&profile)
}
```

**Fixed Code** (Option 1: Count only on ID change):
```rust
fn commit_switch(&mut self, new_profile: &CompiledProfile) -> Result<()> {
    let changed = self.current_profile.as_ref().map(|p| &p.id) != Some(&new_profile.id);
    self.current_profile = Some(new_profile.clone());
    
    if changed {
        self.metrics.committed_switches = self.metrics.committed_switches
            .checked_add(1)
            .unwrap_or_else(|| {
                tracing::warn!("Switch counter overflow, resetting to 0");
                0
            });
    }
    Ok(())
}

pub fn force_switch(&mut self, id: &AircraftId) -> Result<()> {
    let profile = self.resolve_profile(id)?;
    self.commit_switch(&profile)  // Will count if ID differs
}
```

**Fixed Code** (Option 2: Count on any force):
```rust
fn commit_switch(&mut self, new_profile: &CompiledProfile, force: bool) -> Result<()> {
    let changed = self.current_profile.as_ref().map(|p| &p.id) != Some(&new_profile.id);
    self.current_profile = Some(new_profile.clone());
    
    if changed || force {
        self.metrics.committed_switches = self.metrics.committed_switches
            .checked_add(1)
            .unwrap_or_else(|| {
                tracing::warn!("Switch counter overflow, resetting to 0");
                0
            });
    }
    Ok(())
}

pub fn force_switch(&mut self, id: &AircraftId) -> Result<()> {
    let profile = self.resolve_profile(id)?;
    self.commit_switch(&profile, true)  // Always counts
}
```

**Recommendation**: Use Option 1 (count only on ID change) unless tests explicitly require force-same-target to increment. Use `checked_add` with logging instead of `saturating_add` to detect overflow issues early.

**Rationale**:
- `commit_switch` is the single point where switches happen
- `checked_add` with logging prevents silent overflow
- Clear semantics prevent future ambiguity

### 2. flight-hid Private Interfaces Fix

#### Problem Analysis

**Issue**: Public method returns `pub(crate)` type, causing private_interfaces warning

**Example**:
```rust
pub(crate) struct EndpointState { /* ... */ }

impl DeviceManager {
    pub fn get_endpoint_state(&self, id: &EndpointId) -> Option<&EndpointState> {
        // ❌ Public method exposes private type
        self.endpoints.get(id)
    }
}
```

#### Solution Design

**Option A: Lower Method Visibility** (Simplest)

```rust
impl DeviceManager {
    pub(crate) fn get_endpoint_state(&self, id: &EndpointId) -> Option<&EndpointState> {
        self.endpoints.get(id)
    }
}
```

**Pros**: One-line fix, no API surface change
**Cons**: Removes public access if external crates need it

**Option B: Opaque View Type** (Preserves public access)

```rust
pub struct EndpointView<'a>(&'a EndpointState);

impl<'a> EndpointView<'a> {
    pub fn success_rate(&self) -> f64 { 
        self.0.success_rate() 
    }
    pub fn avg_bytes_per_operation(&self) -> f64 { 
        self.0.avg_bytes_per_operation() 
    }
    // Expose only what's needed
}

impl DeviceManager {
    pub fn get_endpoint_state(&self, id: &EndpointId) -> Option<EndpointView<'_>> {
        self.endpoints.get(id).map(EndpointView)
    }
}
```

**Pros**: Maintains public access, controlled API surface
**Cons**: More code, requires deciding which methods to expose

**Decision**: Use Option A unless external crates depend on this method. Check with `cargo public-api -p flight-hid`.

### 3. flight-virtual Abnormal Exit Investigation

#### Problem Analysis

**Symptom**: Test run exits with code 1 without clear failing test

**Common Causes**:
1. Panic in spawned thread (not caught by test harness)
2. Channel send/recv unwrap on closed channel
3. Double-close of OS handle in Drop
4. Timing assumptions without proper synchronization

#### Solution Design

**Investigation Steps**:
```bash
$env:RUST_BACKTRACE="1"
$env:RUST_LOG="debug"
cargo test -p flight-virtual -- --nocapture
```

**Common Fixes**:

**Pattern 1: Spawned Thread Panics**
```rust
// Before
std::thread::spawn(|| {
    // ... work that might panic
});

// After
let handle = std::thread::spawn(|| {
    // ... work that might panic
});
handle.join().expect("Background thread panicked");
```

**Pattern 2: Channel Errors**
```rust
// Before
tx.send(value).unwrap();  // Panics if receiver dropped

// After
tx.send(value).expect("Receiver dropped unexpectedly");
// Or handle gracefully:
if tx.send(value).is_err() {
    // Receiver gone, clean shutdown
}
```

**Pattern 3: Timing Assumptions**
```rust
// Before
std::thread::sleep(Duration::from_millis(100));
assert!(condition);  // Might not be ready yet

// After
let start = Instant::now();
let timeout = Duration::from_secs(5);
while !condition && start.elapsed() < timeout {
    std::thread::sleep(Duration::from_millis(10));
}
assert!(condition, "Condition not met within timeout");
```

### 4. Rustfmt Stable Compatibility

#### Problem Analysis

**Issue**: rustfmt.toml contains nightly-only options, causing warnings on stable

**Example Nightly Options**:
- `imports_granularity`
- `group_imports`
- `format_code_in_doc_comments`
- `normalize_comments`
- `wrap_comments`

#### Solution Design

**rustfmt.toml** (stable-safe):
```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Max"
newline_style = "Auto"
# Remove or comment out nightly-only options
```

**rustfmt.nightly.toml** (optional, for local use):
```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Max"
newline_style = "Auto"

# Nightly-only features
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
format_code_in_doc_comments = true
```

**Usage**:
```bash
# Stable (default)
cargo fmt --all

# Nightly (opt-in)
cargo +nightly fmt --all -- --config-path rustfmt.nightly.toml
```

### 5. MSRV/Edition Alignment

#### Problem Analysis

**Inconsistencies**:
- README says "Rust 1.89.0 MSRV"
- README mentions "2024 edition features"
- Cargo.toml might not specify edition = "2024"
- Codebase uses let-chains (2024 feature)

#### Solution Design

**Workspace Cargo.toml**:
```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.89.0"
```

**Crate Cargo.toml** (inherit from workspace):
```toml
[package]
name = "flight-core"
edition.workspace = true
rust-version.workspace = true
```

**README.md** (ensure accuracy):
```markdown
## Requirements

- Rust 1.89.0 or later (MSRV)
- Edition 2024 features (let-chains, etc.)
```

### 6. IPC Bench Lint Strictness

#### Problem Analysis

**Issue**: Broad `#[allow(...)]` attributes hide genuine issues

**Example**:
```rust
#[allow(unused_variables)]  // ❌ Too broad
fn benchmark_fn(config: Config, data: Data) {
    // Only 'config' unused in some feature configs
}
```

#### Solution Design

**Scoped Allows**:
```rust
// Parameter-level
fn benchmark_fn(
    #[cfg_attr(not(feature = "ipc-bench"), allow(unused_variables))]
    config: Config,
    data: Data
) {
    // ...
}

// Field-level
struct BenchState {
    #[cfg_attr(not(any(feature = "ipc-bench", test)), allow(dead_code))]
    inner: InnerState,
    always_used: u64,
}
```

### 7. CI Hardening

#### Solution Design

**Concurrency Control**:
```yaml
concurrency:
  group: ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.sha }}
  cancel-in-progress: true
```

**Platform-Appropriate Timeouts**:
```yaml
jobs:
  test:
    strategy:
      fail-fast: false  # Don't let Linux mask Windows issues
      matrix:
        os: [ubuntu-latest, windows-latest]
    timeout-minutes: ${{ matrix.os == 'windows-latest' && 20 || 10 }}
    steps:
      - name: Run tests
        run: cargo test --all
  
  build:
    timeout-minutes: ${{ matrix.os == 'windows-latest' && 45 || 30 }}
```

**Tool Pinning with --locked**:
```yaml
- name: Install cargo-public-api
  run: cargo install --locked cargo-public-api@=0.38.0
```

**Caching with Toolchain Hash**:
```yaml
- name: Get Rust toolchain
  id: rust-toolchain
  run: echo "cachekey=$(rustc -Vv | sha256sum | cut -d' ' -f1)" >> $GITHUB_OUTPUT

- uses: actions/cache@v3
  with:
    path: |
      ~/.cargo/registry
      ~/.cargo/git
      target/
      ~/.cargo/bin/cargo-public-api
    key: ${{ runner.os }}-${{ steps.rust-toolchain.outputs.cachekey }}-${{ hashFiles('**/Cargo.lock') }}
```

**MSRV Job**:
```yaml
jobs:
  msrv-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.89.0
      - run: cargo build --all --all-features
      - run: cargo clippy -p flight-core -- -Dwarnings
```

### 8. Test Assertion Cleanup

#### Problem Analysis

**Issue**: Meaningless assertions on unsigned types

**Example**:
```rust
let duration_ms: u64 = measure_duration();
assert!(duration_ms >= 0);  // ❌ Always true for unsigned
```

#### Solution Design

```rust
// Option 1: Remove meaningless assertion
let duration_ms: u64 = measure_duration();
// No assertion needed

// Option 2: Change to meaningful bound
let duration_ms: u64 = measure_duration();
assert!(duration_ms > 0, "Duration should be non-zero");

// Option 3: Check reasonable range
let duration_ms: u64 = measure_duration();
assert!(duration_ms > 0 && duration_ms < 10_000, 
    "Duration {} ms outside expected range", duration_ms);
```

### 9. Documentation Completeness

#### Solution Design

**Create Missing ADRs** (stub format):
```markdown
# ADR-001: [Title]

## Status
Proposed / Accepted / Deprecated

## Context
Brief description of the problem or decision point.

## Decision
What was decided and why.

## Consequences
Positive and negative outcomes of this decision.
```

**Files to Create/Verify**:
- `docs/adr/001-*.md` through `005-*.md`
- `docs/regression-prevention.md`
- `docs/SUMMARY.md` (if using mdBook)

## Data Models

No data model changes required. All fixes preserve existing structures.

## Error Handling

No error handling changes required. Fixes preserve existing error types and propagation.

## Testing Strategy

### Pre-Fix Validation

1. **Capture Baseline**:
   ```bash
   cargo test -p flight-core 2>&1 | tee test-baseline.log
   cargo test -p flight-virtual 2>&1 | tee virtual-baseline.log
   ```

2. **Document Failures**:
   - Count of failing tests
   - Specific test names
   - Error messages

### Post-Fix Validation

1. **Core Tests**:
   ```bash
   cargo test -p flight-core
   # Must pass all tests
   ```

2. **Virtual Tests**:
   ```bash
   RUST_BACKTRACE=1 cargo test -p flight-virtual -- --nocapture
   # Must complete without abnormal exit
   ```

3. **Linting**:
   ```bash
   cargo clippy -p flight-core -- -Dwarnings
   cargo clippy -p flight-hid -- -Dwarnings
   cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings
   ```

4. **Formatting**:
   ```bash
   cargo fmt --all -- --check
   # Must pass on stable 1.89.0
   ```

5. **API Stability**:
   ```bash
   cargo public-api -p flight-core --diff-git origin/main..HEAD
   cargo public-api -p flight-hid --diff-git origin/main..HEAD
   ```

## Implementation Order

1. **Aircraft Auto-Switch Fixes** (highest impact)
   - Reorder PhaseOfFlight classification
   - Add C172 test fixture
   - Increment metrics on switch

2. **flight-hid Private Interfaces** (quick win)
   - Check public API usage
   - Lower visibility or add view type

3. **Rustfmt Cleanup** (prevents noise)
   - Create stable rustfmt.toml
   - Optionally add rustfmt.nightly.toml
   - Format examples/

4. **MSRV/Edition Alignment** (configuration)
   - Update workspace Cargo.toml
   - Update README
   - Verify crate inheritance

5. **flight-virtual Investigation** (requires debugging)
   - Run with backtrace
   - Fix identified issues
   - Add proper error handling

6. **Test Assertion Cleanup** (low risk)
   - Remove meaningless assertions
   - Add meaningful bounds

7. **IPC Bench Strictness** (code quality)
   - Replace broad allows with scoped ones
   - Verify lints still pass

8. **CI Hardening** (infrastructure)
   - Add concurrency control
   - Add timeouts
   - Pin tool versions
   - Improve caching

9. **Documentation** (completeness)
   - Create missing ADRs
   - Verify all links

## Success Criteria

- ✅ `cargo test -p flight-core` passes (0 failures)
- ✅ `cargo test -p flight-virtual` passes (no abnormal exit)
- ✅ `cargo clippy -- -Dwarnings` passes for flight-core, flight-hid, flight-ipc
- ✅ `cargo fmt --all -- --check` passes on stable
- ✅ Workspace edition = "2024" and rust-version = "1.89.0"
- ✅ CI has concurrency control and timeouts
- ✅ All documentation links work
- ✅ Repository meets "properly working" definition
