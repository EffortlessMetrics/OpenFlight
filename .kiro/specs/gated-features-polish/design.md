# Design Document

## Overview

This design document outlines the technical approach for polishing the gated features implementation across the flight-ipc and flight-hid crates. The design ensures zero-warning compilation, prevents cyclic dependencies, provides safe APIs for packed struct manipulation, and establishes CI guardrails against unintended public API changes.

The work is organized into six independent components that can be implemented in parallel, with two open decisions requiring maintainer input before implementation.

## Architecture

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     CI Pipeline                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Default      │  │ Gated IPC    │  │ Gated HID    │      │
│  │ Workspace    │  │ Smoke Tests  │  │ Smoke Tests  │      │
│  │ Check        │  │ (on-demand)  │  │ (on-demand)  │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
│                   ┌────────▼────────┐                        │
│                   │  Public API     │                        │
│                   │  Guard Check    │                        │
│                   └─────────────────┘                        │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                  flight-ipc Crate                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  benches/ipc_benchmarks.rs                           │   │
│  │  - #![deny(unused_imports)]                          │   │
│  │  - #[cfg(feature = "ipc-bench-serde")] serde block   │   │
│  │  - #[cfg(not(feature = "ipc-bench-serde"))] no-op    │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  examples/client_example.rs                          │   │
│  │  - Option A: FlightClient::list_devices() [gated]    │   │
│  │  - Option B: Use get_service_info() [existing RPC]   │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                  flight-hid Crate                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  src/protocol/ofp1.rs                                │   │
│  │  - CapabilitiesReport::set_cap_flag()                │   │
│  │  - HealthStatusReport::set_status_flag()             │   │
│  │  - #[cfg_attr(...)] Clone/Copy derives               │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  tests/* (updated to use helpers)                    │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│         flight-virtual Crate (or new integration crate)      │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  tests/ofp1_integration.rs                           │   │
│  │  - Emulator tests moved from flight-hid              │   │
│  │  - Depends on: flight-hid + flight-virtual           │   │
│  │  - One-way dependency (no cycle)                     │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. flight-ipc Benchmark Cleanup

**Location:** `crates/flight-ipc/benches/ipc_benchmarks.rs`

**Changes:**
- Add `#![deny(unused_imports)]` at file top to enforce zero warnings
- Remove unused imports (e.g., `ListDevicesRequest`, etc.)
- Wrap serde-specific imports in `#[cfg(feature = "ipc-bench-serde")]`
- Wrap JSON roundtrip benchmark in `#[cfg(feature = "ipc-bench-serde")]`
- Add no-op placeholder when `ipc-bench-serde` is disabled

**Feature Configuration:**
```toml
# crates/flight-ipc/Cargo.toml
[features]
ipc-bench = []
ipc-bench-serde = ["dep:serde_json"]
```

**Code Pattern:**
```rust
#![deny(unused_imports)]

// Always-available imports
use criterion::{criterion_group, criterion_main, Criterion};

// Serde-specific imports
#[cfg(feature = "ipc-bench-serde")]
use serde_json;

fn bench_ipc_operations(c: &mut Criterion) {
    // Core benchmarks...
    
    #[cfg(feature = "ipc-bench-serde")]
    {
        c.bench_function("json_device_roundtrip", |b| {
            let device = Device { /* ... */ };
            b.iter(|| {
                let json = serde_json::to_string(&device).unwrap();
                serde_json::from_str::<Device>(&json).unwrap()
            });
        });
    }
    
    #[cfg(not(feature = "ipc-bench-serde"))]
    {
        // No-op: serde benchmarks disabled
    }
}
```

**Verification Commands:**
```bash
RUSTFLAGS="-Dunused-imports" cargo bench -p flight-ipc --features ipc-bench --no-run
RUSTFLAGS="-Dunused-imports" cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run
```

### 2. FlightClient Example Decision Point

**Location:** `crates/flight-ipc/examples/client_example.rs` and `crates/flight-ipc/src/client.rs`

**Option A: Keep Shim (Gated)**

Feature-gate the shim to prevent public API growth:

```rust
// crates/flight-ipc/src/client.rs
impl FlightClient {
    #[cfg(feature = "ipc-examples")]
    pub fn list_devices(&self) -> Result<Vec<Device>> {
        // Shim implementation
    }
}
```

Or make it crate-private:
```rust
impl FlightClient {
    pub(crate) fn list_devices(&self) -> Result<Vec<Device>> {
        // Shim implementation
    }
}
```

Changelog entry:
```markdown
### Internal
- Added `FlightClient::list_devices()` for example convenience (not a supported public API; no semver guarantee)
```

**Option B: Drop Shim (Minimal Surface)**

Update example to use existing RPC:

```rust
// crates/flight-ipc/examples/client_example.rs
fn main() -> Result<()> {
    let client = FlightClient::connect("127.0.0.1:50051")?;
    
    // Use existing RPC instead of shim
    let info = client.get_service_info()?;
    println!("Service: {:?}", info);
    
    Ok(())
}
```

Remove the `list_devices()` method entirely.

**Public API Verification:**
```bash
cargo public-api -p flight-ipc --diff-git main..HEAD
# or
cargo semver-checks -p flight-ipc
```

### 3. flight-hid Emulator Test Relocation

**Problem:** Cyclic dev-dependency between flight-hid and flight-virtual causes multi-version conflicts.

**Solution:** Move emulator tests to break the cycle.

**Option A (Recommended): Move to flight-virtual**

```
crates/flight-virtual/
├── src/
├── tests/
│   └── ofp1_integration.rs  ← New location
└── Cargo.toml
```

```toml
# crates/flight-virtual/Cargo.toml
[dev-dependencies]
flight-hid = { path = "../flight-hid", features = ["ofp1-tests"] }
```

```rust
// crates/flight-virtual/tests/ofp1_integration.rs
use flight_hid::protocol::ofp1::*;
use flight_virtual::emulator::*;

#[test]
fn test_emulator_capabilities() {
    let emulator = Ofp1Emulator::new();
    let report = emulator.get_capabilities();
    assert!(report.capability_flags.has_flag(CapabilityFlags::ANALOG_INPUT));
}
```

**Option B: New Integration Crate**

```
crates/flight-hid-integration-tests/
├── src/
│   └── lib.rs  (empty or minimal)
├── tests/
│   └── ofp1_emulator.rs
└── Cargo.toml
```

```toml
# crates/flight-hid-integration-tests/Cargo.toml
[package]
name = "flight-hid-integration-tests"
version = "0.1.0"
publish = false

[dev-dependencies]
flight-hid = { path = "../flight-hid", features = ["ofp1-tests"] }
flight-virtual = { path = "../flight-virtual" }
```

**Verification Commands:**
```powershell
# Verify no cycle
cargo tree -p flight-hid --edges dev,normal | Select-String flight-virtual
# Should print nothing

# Verify single version
cargo tree -p flight-hid | Select-String 'flight_hid v'
# Should show exactly one version

# Run relocated tests
cargo test -p flight-virtual --tests
# or
cargo test -p flight-hid-integration-tests
```

### 4. Packed Field Safe Helpers

**Location:** `crates/flight-hid/src/protocol/ofp1.rs`

**Problem:** Direct field access on `#[repr(packed)]` structs causes E0793 errors.

**Solution:** Provide safe helper methods that use copy-modify-write-back pattern.

**Implementation:**

```rust
#[repr(packed)]
pub struct CapabilitiesReport {
    pub(crate) capability_flags: CapabilityFlags,
    // other fields...
}

impl CapabilitiesReport {
    /// Safely sets a capability flag without taking a reference to packed field.
    #[inline]
    pub fn set_cap_flag(&mut self, flag: CapabilityFlags) {
        // Copy-modify-write-back pattern
        let mut flags = self.capability_flags;
        flags.set_flag(flag);
        self.capability_flags = flags;
    }
    
    /// Safely clears a capability flag without taking a reference to packed field.
    #[inline]
    pub fn clear_cap_flag(&mut self, flag: CapabilityFlags) {
        let mut flags = self.capability_flags;
        flags.clear_flag(flag);
        self.capability_flags = flags;
    }
}

#[repr(packed)]
pub struct HealthStatusReport {
    pub(crate) status_flags: StatusFlags,
    // other fields...
}

impl HealthStatusReport {
    /// Safely sets a status flag without taking a reference to packed field.
    #[inline]
    pub fn set_status_flag(&mut self, flag: StatusFlags) {
        let mut flags = self.status_flags;
        flags.set_flag(flag);
        self.status_flags = flags;
    }
    
    /// Safely clears a status flag without taking a reference to packed field.
    #[inline]
    pub fn clear_status_flag(&mut self, flag: StatusFlags) {
        let mut flags = self.status_flags;
        flags.clear_flag(flag);
        self.status_flags = flags;
    }
}
```

**Test Updates:**

```rust
// Before (causes E0793)
let mut report = CapabilitiesReport::default();
report.capability_flags.set_flag(CapabilityFlags::ANALOG_INPUT);

// After (safe)
let mut report = CapabilitiesReport::default();
report.set_cap_flag(CapabilityFlags::ANALOG_INPUT);
```

**Documentation (if fields are public):**

```rust
#[repr(packed)]
pub struct CapabilitiesReport {
    /// Capability flags.
    /// 
    /// **Warning:** Do not take references to this field directly due to packed layout.
    /// Use [`set_cap_flag`](Self::set_cap_flag) and [`clear_cap_flag`](Self::clear_cap_flag) instead.
    pub capability_flags: CapabilityFlags,
}
```

### 5. Clone/Copy Derive Strategy

**Location:** `crates/flight-hid/src/protocol/ofp1.rs`

**Option A: Gate for Tests (No Public API Change)**

For same-crate tests:
```rust
#[cfg_attr(test, derive(Clone, Copy))]
#[repr(packed)]
pub struct CapabilitiesReport {
    // fields...
}
```

For external tests (e.g., in flight-virtual):
```rust
#[cfg_attr(any(test, feature = "ofp1-tests"), derive(Clone, Copy))]
#[repr(packed)]
pub struct CapabilitiesReport {
    // fields...
}
```

```toml
# crates/flight-hid/Cargo.toml
[features]
ofp1-tests = []  # Not in default features
```

**Option B: Make Public (With Changelog)**

```rust
#[derive(Clone, Copy)]
#[repr(packed)]
pub struct CapabilitiesReport {
    // fields...
}
```

Changelog entry:
```markdown
### Added
- Implemented `Clone` and `Copy` for `CapabilitiesReport` and `HealthStatusReport` to improve ergonomics
```

Public API check will record this as an approved addition.

**Consistency:** Apply the chosen strategy to all affected types:
- `CapabilitiesReport`
- `HealthStatusReport`
- Any other public packed structs in flight-hid

### 6. CI Gated Feature Smoke Tests

**Location:** `.github/workflows/ci.yml` (or equivalent)

**Design:**

```yaml
name: CI

on:
  pull_request:
  push:
    branches: [main]
  schedule:
    - cron: "0 3 * * *"  # 3 AM UTC daily

jobs:
  # Default workspace check (always runs)
  workspace-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Check workspace
        run: cargo check --workspace

  # Public API guard (always runs on PRs)
  public-api-check:
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-public-api
        run: cargo install cargo-public-api
      - name: Check flight-ipc public API
        run: cargo public-api -p flight-ipc --diff-git origin/main..HEAD
      - name: Check flight-hid public API
        run: cargo public-api -p flight-hid --diff-git origin/main..HEAD

  # Gated IPC smoke tests (on-demand)
  gated-ipc-smoke:
    runs-on: ubuntu-latest
    if: |
      github.event_name == 'schedule' ||
      contains(github.event.pull_request.labels.*.name, 'run-gated') ||
      contains(github.event.pull_request.changed_files, 'crates/flight-ipc/')
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Smoke test IPC benchmarks
        run: RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features ipc-bench
      - name: Smoke test IPC benchmarks with serde
        run: RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features "ipc-bench,ipc-bench-serde"

  # Gated HID smoke tests (on-demand)
  gated-hid-smoke:
    runs-on: ubuntu-latest
    if: |
      github.event_name == 'schedule' ||
      contains(github.event.pull_request.labels.*.name, 'run-gated') ||
      contains(github.event.pull_request.changed_files, 'crates/flight-hid/')
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Smoke test HID with ofp1-tests
        run: cargo test --no-run -p flight-hid --features ofp1-tests

  # Cross-platform verification (runs on schedule)
  cross-platform:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    if: github.event_name == 'schedule'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Check workspace
        run: cargo check --workspace
```

**Trigger Mechanisms:**
1. **Always:** Default workspace check and public API guard on PRs
2. **Scheduled:** Nightly cron runs all smoke tests
3. **On-demand:** PR label `run-gated` triggers smoke tests
4. **Path-based:** Changes to specific crates trigger their smoke tests

## Data Models

No new data models are introduced. Existing models are modified:

### CapabilitiesReport (flight-hid)
```rust
#[repr(packed)]
#[cfg_attr(any(test, feature = "ofp1-tests"), derive(Clone, Copy))]  // Option A
// or
#[derive(Clone, Copy)]  // Option B
pub struct CapabilitiesReport {
    pub(crate) capability_flags: CapabilityFlags,
    // ... other fields
}

impl CapabilitiesReport {
    #[inline]
    pub fn set_cap_flag(&mut self, flag: CapabilityFlags) { /* ... */ }
    
    #[inline]
    pub fn clear_cap_flag(&mut self, flag: CapabilityFlags) { /* ... */ }
}
```

### HealthStatusReport (flight-hid)
```rust
#[repr(packed)]
#[cfg_attr(any(test, feature = "ofp1-tests"), derive(Clone, Copy))]  // Option A
// or
#[derive(Clone, Copy)]  // Option B
pub struct HealthStatusReport {
    pub(crate) status_flags: StatusFlags,
    // ... other fields
}

impl HealthStatusReport {
    #[inline]
    pub fn set_status_flag(&mut self, flag: StatusFlags) { /* ... */ }
    
    #[inline]
    pub fn clear_status_flag(&mut self, flag: StatusFlags) { /* ... */ }
}
```

## Error Handling

### Benchmark Compilation Errors
- **Error:** Unused import warnings in `ipc_benchmarks.rs`
- **Detection:** `#![deny(unused_imports)]` or `RUSTFLAGS="-Dunused-imports"`
- **Resolution:** Remove unused imports or wrap in appropriate `#[cfg]` blocks

### Cyclic Dependency Errors
- **Error:** Multiple versions of flight-hid in dependency tree
- **Detection:** `cargo tree -p flight-hid | Select-String 'flight_hid v'`
- **Resolution:** Move emulator tests to break cycle

### Packed Field Reference Errors (E0793)
- **Error:** "reference to packed field is unaligned"
- **Detection:** Compile-time error when taking `&mut` to packed field
- **Resolution:** Use safe helper methods instead of direct field access

### Public API Drift
- **Error:** Unintended public API additions
- **Detection:** `cargo public-api --diff-git` or `cargo-semver-checks`
- **Resolution:** Gate new APIs behind features or mark as `pub(crate)`

## Testing Strategy

### Unit Tests
- **Packed field helpers:** Test that `set_cap_flag` and `set_status_flag` correctly modify fields
- **Feature gating:** Verify Clone/Copy derives are available under correct feature flags

### Integration Tests
- **Emulator tests:** Relocated to `flight-virtual/tests/` or new integration crate
- **Cross-crate:** Verify flight-virtual can use flight-hid types with gated features

### Smoke Tests (CI)
- **IPC benchmarks:** Compile with `ipc-bench` and `ipc-bench,ipc-bench-serde`
- **HID tests:** Compile with `ofp1-tests` feature
- **Frequency:** Nightly cron + on-demand via PR label

### Verification Commands
```bash
# Default workspace
cargo check --workspace

# IPC smoke tests
RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features ipc-bench
RUSTFLAGS="-Dunused-imports" cargo bench --no-run -p flight-ipc --features "ipc-bench,ipc-bench-serde"

# HID smoke tests
cargo test --no-run -p flight-hid --features ofp1-tests

# Dependency verification
cargo tree -p flight-hid --edges dev,normal | Select-String flight-virtual  # Should be empty
cargo tree -p flight-hid | Select-String 'flight_hid v'  # Should show one version

# Public API verification
cargo public-api -p flight-ipc --diff-git origin/main..HEAD
cargo public-api -p flight-hid --diff-git origin/main..HEAD

# Lint verification
cargo fmt --check -p flight-ipc -p flight-hid
cargo clippy -p flight-ipc -p flight-hid -- -Dwarnings
```

## Implementation Notes

### Parallel Implementation
All six components are independent and can be implemented in parallel:
1. IPC benchmark cleanup
2. FlightClient example decision
3. HID emulator test relocation
4. Packed field safe helpers
5. Clone/Copy derive strategy
6. CI smoke tests

### Open Decisions Required Before Implementation
1. **FlightClient::list_devices():** Keep (gated) or drop (use existing RPC)?
2. **Clone/Copy derives:** Gate for tests or make public with changelog?

### Changelog Updates
Document decisions in respective crate changelogs:
- `crates/flight-ipc/CHANGELOG.md`
- `crates/flight-hid/CHANGELOG.md`

### Documentation Updates
- Add rustdoc warnings for packed fields if they remain public
- Document gated features in crate README files
- Update CI documentation with smoke test trigger mechanisms
