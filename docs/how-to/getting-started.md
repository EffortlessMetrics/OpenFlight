---
doc_id: DOC-HOWTO-GETTING-STARTED
kind: how-to
area: user
status: active
links:
  requirements: []
  tasks: []
  adrs: []
---

# Getting Started with OpenFlight

This guide walks you through building OpenFlight from source, running the
service daemon, and configuring your first device profile.

## Prerequisites

### Rust Toolchain

OpenFlight requires **Rust 1.92.0** or later (edition 2024).

```bash
# Install rustup (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install the required toolchain
rustup install 1.92.0
rustup default 1.92.0
```

### Platform Dependencies

**Windows:**

- Visual Studio 2022 with **C++ build tools**
- **Windows SDK** (required for HID and SimConnect support)

**Linux (Debian / Ubuntu):**

```bash
sudo apt-get update
sudo apt-get install build-essential pkg-config libusb-1.0-0-dev libudev-dev protobuf-compiler
```

**macOS:**

```bash
xcode-select --install
brew install libusb protobuf
```

### Optional Tools

| Tool | Purpose |
|------|---------|
| `cargo-nextest` | Faster parallel test runner |
| `cargo-public-api` | API stability checks (`cargo xtask validate`) |
| `cargo-watch` | Automatic rebuild on save |
| `cargo-fuzz` | Fuzz testing protocol parsers |
| `cargo-mutants` | Mutation testing |

If you use **Nix**, a dev shell is provided:

```bash
nix develop   # Includes Rust 1.92.0, protoc, pkg-config, libusb
```

## Building from Source

```bash
# Clone the repository
git clone https://github.com/EffortlessMetrics/OpenFlight.git
cd OpenFlight

# Build all workspace crates (debug)
cargo build --workspace

# Build optimised release
cargo build --release --workspace

# Build with RT-optimised profile (release + debug symbols)
cargo build --profile rt --workspace
```

Verify the build:

```bash
cargo test --workspace
```

## Running the Service (`flightd`)

The OpenFlight daemon manages device discovery, axis processing, sim
connections, and the gRPC API.

```bash
# Run in debug mode
cargo run -p flight-service

# Run release build
cargo run --release -p flight-service
```

Set the log level with `RUST_LOG`:

```bash
RUST_LOG=info cargo run -p flight-service
RUST_LOG=debug cargo run -p flight-service     # verbose
RUST_LOG=flight_axis=trace cargo run -p flight-service  # per-crate
```

## Using the CLI (`flightctl`)

`flightctl` communicates with the running `flightd` daemon over gRPC.

```bash
# Check daemon status
cargo run -p flight-cli -- status

# List detected devices
cargo run -p flight-cli -- devices list

# Show active profile
cargo run -p flight-cli -- profile show

# Load a profile
cargo run -p flight-cli -- profile load my-profile.toml
```

## Configuring a Device

Devices are auto-detected via HID enumeration when `flightd` starts. To see
what OpenFlight found:

```bash
cargo run -p flight-cli -- devices list
```

Each device is matched against the device manifests in
`compat/manifests/devices/` by vendor/product ID. The manifest defines axis
counts, button maps, and FFB capabilities.

## Creating a Profile

Profiles are TOML files that define axis curves, deadzones, button mappings,
and panel LED rules. Profiles cascade in this order:

1. **Global** — baseline settings
2. **Simulator** — simulator-specific overrides
3. **Aircraft** — aircraft-specific overrides
4. **Phase-of-Flight** — approach/cruise/taxi overrides

Example minimal profile:

```toml
[metadata]
name = "My First Profile"
simulator = "msfs"

[axes.roll]
curve = "linear"
deadzone = 0.03
sensitivity = 1.0

[axes.pitch]
curve = "scurve"
deadzone = 0.05
sensitivity = 0.8
curvature = 0.3
```

Place profiles in the configured profile directory (see `FLIGHT_PROFILE_PATH`
environment variable) and load them with `flightctl profile load`.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Logging level (`trace`, `debug`, `info`, `warn`, `error`) |
| `FLIGHT_CONFIG_PATH` | Platform default | Path to configuration directory |
| `FLIGHT_PROFILE_PATH` | Platform default | Path to profile directory |

## Validating Your Setup

Run the fast smoke test to verify everything works:

```bash
cargo xtask check
```

This runs formatting checks, clippy lints on core crates, and unit tests.

For full validation (including benchmarks and API stability checks):

```bash
cargo xtask validate
```

## Next Steps

- [Set up a full dev environment](./setup-dev-env.md) (Docker, IDE, hot reload)
- [Run tests](./run-tests.md) (unit, property, integration, FFB safety)
- [Run benchmarks](./run-benchmarks.md)
- Browse the [crate index](../reference/crate-index.md)
- Read the [Architecture Decision Records](../explanation/adr/)
