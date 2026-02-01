# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OpenFlight (Flight Hub) is a PC flight simulation input management system written in Rust. It provides unified control for flight controls, panels, and force feedback devices across MSFS, X-Plane, and DCS simulators with real-time 250Hz axis processing.

**Key constraints:**
- Real-time 250Hz processing with <5ms p99 latency and <0.5ms p99 jitter
- Zero allocations on RT hot paths (see ADR-004)
- Rust 2024 edition, MSRV 1.89.0

**Prerequisites:**
- Windows: Windows SDK for HID support
- Linux: libudev development headers (`libudev-dev` on Debian/Ubuntu)

## Build Commands

```bash
# Build
cargo build --workspace
cargo build --release --workspace
cargo build --profile rt --workspace   # RT-optimized with debug symbols

# Test
cargo test --workspace                    # All tests
cargo test -p flight-core                 # Single crate
cargo test -- --nocapture                 # With output

# Lint
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# Security
cargo audit --deny warnings
cargo deny check
```

## Custom Tasks (xtask)

```bash
cargo xtask check      # Fast smoke test (fmt, clippy, core tests)
cargo xtask validate   # Full validation (tests, benches, API checks)
```

## Makefile Targets

```bash
make quick             # Clippy + pattern verification
make all               # Full regression prevention
make ci-simulation     # Full CI pipeline locally
make clippy-strict     # Strict clippy on core crates
make feature-powerset  # Test all feature combinations
make verify-patterns   # Check for known problematic patterns
```

## Quality Gates

| Gate | What it checks |
|------|---------------|
| QG-SANITY-GATE | Compilation, formatting |
| QG-FFB-SAFETY | Force feedback safety systems |
| QG-RT-JITTER | Timer jitter p99 ≤ 0.5ms (hardware runners) |
| QG-HID-LATENCY | HID write latency p99 ≤ 300μs (hardware runners) |

Run FFB safety tests before modifying force feedback code:
```bash
cargo test -p flight-ffb safety
cargo test -p flight-ffb envelope
```

## Architecture

### Real-Time Spine (ADR-001)

The system uses a protected 250Hz RT core that never blocks, allocates memory, or takes locks:

```
Non-RT Systems (Sim Adapters, Panels, Diagnostics)
                    │ Drop-tail queues
RT Spine (250Hz): Axis Engine │ FFB Engine │ Scheduler
```

Configuration changes are compiled off-thread and swapped atomically at tick boundaries.

### Zero-Allocation Constraint (ADR-004)

**Forbidden in RT code** (`flight-axis`, `flight-scheduler`, `flight-ffb` hot paths):
- `Box::new()`, `Vec::push()` past capacity, `String` operations that allocate
- `HashMap::insert()` that triggers rehashing
- `Arc::new()`, `Rc::new()`
- Any blocking syscalls or locks

**Allowed:** Stack allocation, pre-allocated containers, atomic operations, static data.

### Crate Organization

**Real-Time Core:**
- `flight-axis` - 250Hz axis processing (curves, deadzones, detents, mixers)
- `flight-scheduler` - Platform RT scheduling (MMCSS on Windows, rtkit on Linux)
- `flight-bus` - Event bus for inter-component communication

**Hardware:**
- `flight-hid` - HID device management
- `flight-ffb` - Force feedback synthesis
- `flight-panels` - Generic panel driver
- `flight-streamdeck` - StreamDeck integration

**Simulators:**
- `flight-simconnect` - MSFS SimConnect adapter
- `flight-xplane` - X-Plane UDP/plugin adapter
- `flight-dcs-export` - DCS Export.lua integration

**Infrastructure:**
- `flight-core` - Core types, profile management, aircraft detection
- `flight-ipc` - gRPC-based IPC (tonic 0.14, prost 0.14)
- `flight-profile` - Profile schema and validation
- `flight-rules` - Rule engine for panel/LED control

**Applications:**
- `flight-service` - Main daemon (`flightd`)
- `flight-cli` - CLI (`flightctl`)

### Critical Patterns

The Makefile enforces these patterns via `make verify-patterns`:
- Use `Profile::merge_with` not `Profile::merge`
- Use `std::hint::black_box` not `criterion::black_box`
- All workspace dependencies via `workspace = true` (especially tokio, futures, tonic)

### Core Crates Requiring Strict Clippy

These crates must pass `cargo clippy -p <crate> -- -D warnings`:
- flight-core, flight-axis, flight-bus, flight-hid
- flight-ipc, flight-service, flight-simconnect, flight-panels

## Architecture Decision Records

Located in `docs/explanation/adr/`:
- **ADR-001**: RT Spine - Protected 250Hz core with atomic state swaps
- **ADR-004**: Zero-allocation constraint for RT hot paths
- **ADR-005**: PLL timing discipline for jitter control
- **ADR-009**: Safety interlock design for FFB

## Documentation

Documentation follows the Diataxis framework:
- `docs/explanation/adr/` - Architecture Decision Records
- `docs/how-to/` - Task-oriented guides
- `docs/reference/` - API and specification reference
- `docs/NOW_NEXT_LATER.md` - Current priorities

## Branching

- `main` - Stable development branch
- `feat/` - Feature branches
- `fix/` - Bug fix branches
- `docs/` - Documentation updates

## Before Submitting PRs

1. Run `cargo xtask validate` or `make all`
2. Ensure strict clippy passes on core crates
3. Check `docs/NOW_NEXT_LATER.md` for alignment with priorities
