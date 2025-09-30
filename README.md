# Flight Hub

A comprehensive PC flight simulation input management system that provides a unified control plane for flight controls, panels, and force feedback devices across multiple simulators.

## Features

- **Real-time 250Hz axis processing** with deterministic performance
- **Multi-simulator support** for MSFS, X-Plane, and DCS
- **Force feedback safety systems** with proper interlocks
- **Auto-profile switching** based on aircraft detection
- **Panel and StreamDeck integration** with rule-based LED control
- **Comprehensive diagnostics** and blackbox recording

## Architecture

Flight Hub is built as a modular Rust workspace with the following components:

- `flight-core` - Core data structures and profile management
- `flight-axis` - Real-time 250Hz axis processing engine
- `flight-scheduler` - Platform-specific real-time scheduling
- `flight-ipc` - Protobuf-based inter-process communication
- `flight-service` - Main service daemon (`flightd`)
- `flight-cli` - Command-line interface (`flightctl`)

## Building

### Prerequisites

- Rust 1.75.0 or later
- On Windows: Windows SDK for HID support
- On Linux: libudev development headers

### Build Commands

```bash
# Build all components
cargo build --workspace

# Run tests
cargo test --workspace

# Build release version
cargo build --release --workspace

# Run linting
cargo fmt --check
cargo clippy --workspace -- -D warnings

# Security audit
cargo audit
cargo deny check
```

## Development

### Code Style

This project uses `rustfmt` and `clippy` for code formatting and linting. Configuration is provided in `rustfmt.toml` and `clippy.toml`.

### CI/CD

GitHub Actions workflows provide:
- Cross-platform testing (Windows + Linux)
- Security auditing with `cargo-audit` and `cargo-deny`
- Performance regression detection
- Automated releases

### Performance Requirements

The system maintains strict performance requirements:
- Axis processing latency ≤ 5ms p99
- Jitter ≤ 0.5ms p99 at 250Hz
- Zero allocations on real-time hot paths
- CPU usage < 3% of one core during normal operation

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions are welcome! Please read our contributing guidelines and ensure all tests pass before submitting a pull request.