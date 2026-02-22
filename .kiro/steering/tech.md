# Tech Stack

## Language & Toolchain
- Rust 2024 edition
- MSRV: 1.92.0
- Workspace-based monorepo

## Key Dependencies
- `tokio` 1.49.0 - Async runtime
- `tonic` 0.14.2 / `prost` 0.14.3 - gRPC IPC
- `crossbeam` 0.8 / `parking_lot` 0.12 - Real-time concurrency
- `serde` 1.0 / `serde_json` / `postcard` - Serialization
- `clap` 4.5 - CLI parsing
- `tracing` 0.1 - Observability
- `proptest` 1.9 - Property-based testing
- `criterion` 0.8 - Benchmarking
- `reqwest` 0.13 (rustls) - HTTP client

## Platform-Specific
- Windows: `windows` 0.62 crate (HID, MMCSS scheduling)
- Linux: `nix` 0.31, `libc` 0.2 (rtkit scheduling, udev)

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
make verify-patterns              # Check for known problematic patterns
make check-workspace-deps         # Check dependency alignment
```

## Critical Patterns (Enforced via `make verify-patterns`)
- Use `Profile::merge_with` not `Profile::merge`
- Use `std::hint::black_box` not `criterion::black_box`
- All workspace dependencies via `workspace = true`
- No `BlackboxWriter::new` with `?` operator
- Core crates must pass `cargo clippy -p <crate> -- -D warnings`

## Core Crates Requiring Strict Clippy
flight-core, flight-axis, flight-bus, flight-hid, flight-ipc, flight-service, flight-simconnect, flight-panels

## Release Profile
- LTO enabled, single codegen unit, panic=abort
- RT profile: inherits release with debug symbols, no overflow checks
