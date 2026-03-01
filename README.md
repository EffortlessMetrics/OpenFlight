# Flight Hub

[![Repo](https://img.shields.io/badge/repo-EffortlessMetrics%2FOpenFlight-blue?logo=github&style=flat-square)](https://github.com/EffortlessMetrics/OpenFlight)

Flight Hub is a PC flight simulation input management system written in Rust. It provides a unified control plane for flight controls, panels, force feedback devices, and simulator adapters.

## Product Posture

Flight Hub is an accessory/input manager that requires a simulator such as MSFS, X-Plane, or DCS. Experimental AC7 support is available via user-provided telemetry bridges. It does not emulate or replace any simulator.

For simulator integration boundaries and compliance notes, see `docs/product-posture.md`.

## Workspace Crates

- Real-time spine: `flight-axis`, `flight-scheduler`, `flight-bus`
- Service surface: `flight-service`, `flight-cli`, `flight-ipc`, `flight-ui`
- Core domain: `flight-core`, `flight-profile`, `flight-rules`, `flight-units`, `flight-session`
- Simulator adapters: `flight-simconnect`, `flight-simconnect-sys`, `flight-xplane`, `flight-dcs-export`, `flight-ac7-protocol`, `flight-ac7-telemetry`, `flight-ac7-input`, `flight-adapter-common`, `flight-sim-racing`
- Device and hardware: `flight-hid`, `flight-hid-support`, `flight-hid-types`, `flight-virtual`
- Panel and control hardware: `flight-panels`, `flight-panels-core`, `flight-panels-saitek`, `flight-panels-cougar`, `flight-hotas-saitek`, `flight-hotas-thrustmaster`, `flight-streamdeck`
- Safety, diagnostics, and observability: `flight-ffb`, `flight-watchdog`, `flight-blackbox`, `flight-tracing`, `flight-metrics`, `flight-tactile`
- Persistence and platform integration: `flight-writers`, `flight-updater`, `flight-security`, `flight-process-detection`, `flight-replay`

Each crate now documents its scope in `crates/<crate>/README.md`.

## Architecture Decisions

Architecture Decision Records are under `docs/explanation/adr/`.

- `001-rt-spine-architecture.md`
- `002-writers-as-data.md`
- `003-plugin-classes.md`
- `004-zero-allocation-constraint.md`
- `005-pll-timing-discipline.md`
- `006-driver-light-approach.md`
- `007-pipeline-ownership-model.md`
- `008-ffb-mode-selection.md`
- `009-safety-interlock-design.md`
- `010-schema-versioning-strategy.md`
- `011-observability-architecture.md`

## Quick Start

**Prerequisites:** Rust 1.92+, Windows SDK (Windows) or `libudev-dev` (Linux).

```bash
git clone https://github.com/EffortlessMetrics/OpenFlight.git
cd OpenFlight
cargo build --workspace
cargo xtask check          # fmt + clippy + core tests
```

Try the virtual device harness (no hardware needed):

```bash
cargo run -p flight-virtual
```

For the full walkthrough — connecting a device, applying a profile,
and running with a simulator — see the
[Getting Started Guide](docs/how-to/getting-started.md).

## Build and Validate

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

Workspace helpers:

```bash
cargo xtask check      # Fast smoke test (fmt, clippy, core tests)
cargo xtask validate   # Full validation (tests, benches, API checks)
make quick             # Clippy + pattern verification
make all               # Full regression prevention
```

## Performance Constraints

- 250 Hz real-time processing loop
- Axis processing latency target: ≤ 5 ms p99
- Jitter target: ≤ 0.5 ms p99
- Zero allocations on RT hot paths

## Documentation

Documentation follows the [Diataxis](https://diataxis.fr/) framework:

| Category | Path | Description |
|----------|------|-------------|
| **How-To Guides** | [`docs/how-to/`](docs/how-to/) | [Getting Started](docs/how-to/getting-started.md) · [Adding a Device](docs/how-to/adding-a-device.md) · [Adding a Simulator](docs/how-to/adding-a-simulator.md) |
| **Reference** | [`docs/reference/`](docs/reference/) | [Configuration](docs/reference/configuration.md) · [Architecture Overview](docs/reference/architecture-overview.md) · [Supported Hardware](docs/reference/supported-hardware.md) |
| **Explanation** | [`docs/explanation/`](docs/explanation/) | ADRs, crate deep-dives, integration notes |
| **Tutorials** | [`docs/tutorials/`](docs/tutorials/) | Step-by-step learning guides |

See [`docs/README.md`](docs/README.md) for the full documentation index.

## License

Licensed under either:

- Apache-2.0 (`LICENSE-APACHE`)
- MIT (`LICENSE-MIT`)
