# Tech Stack

## Language & Toolchain
- Rust 2024 edition
- MSRV: 1.89.0
- Workspace-based monorepo

## Key Dependencies
- `tokio` 1.49 - Async runtime
- `tonic` 0.14 / `prost` 0.14 - gRPC IPC
- `crossbeam` / `parking_lot` - Real-time concurrency
- `serde` / `serde_json` / `postcard` - Serialization
- `clap` 4.5 - CLI parsing
- `tracing` - Observability
- `proptest` - Property-based testing
- `criterion` - Benchmarking

## Platform-Specific
- Windows: `windows` crate (HID, MMCSS scheduling)
- Linux: `nix`, `libc` (rtkit scheduling, udev)

## Build Commands

```bash
# Build
cargo build --workspace
cargo build --release --workspace

# Test
cargo test --workspace
cargo test -p <crate-name>        # Single crate

# Lint
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# Security
cargo audit --deny warnings
cargo deny check

# xtask shortcuts
cargo xtask check                 # Fast smoke test
cargo xtask validate              # Full validation

# Makefile targets
make quick                        # Clippy + pattern verification
make all                          # Full regression prevention
make ci-simulation                # Full CI pipeline locally
make clippy-strict                # Strict clippy on core crates
make feature-powerset             # Test all feature combinations
```

## Critical Patterns (Enforced)
- Use `Profile::merge_with` not `Profile::merge`
- Use `std::hint::black_box` not `criterion::black_box`
- All workspace dependencies via `workspace = true`
- Core crates must pass `cargo clippy -p <crate> -- -D warnings`

## Core Crates Requiring Strict Clippy
flight-core, flight-axis, flight-bus, flight-hid, flight-ipc, flight-service, flight-simconnect, flight-panels
