# OpenFlight (Flight Hub)

[![CI](https://img.shields.io/github/actions/workflow/status/EffortlessMetrics/OpenFlight/ci.yml?branch=main&label=CI&logo=github&style=flat-square)](https://github.com/EffortlessMetrics/OpenFlight/actions)
[![MSRV](https://img.shields.io/badge/MSRV-1.92.0-blue?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)](#license)
[![Repo](https://img.shields.io/badge/repo-EffortlessMetrics%2FOpenFlight-blue?logo=github&style=flat-square)](https://github.com/EffortlessMetrics/OpenFlight)

OpenFlight is a PC flight simulation input management system written in Rust. It provides a unified control plane for flight controls, panels, force feedback devices, and simulator adapters with a hard real-time **250 Hz** processing spine.

## Features

- **Real-time axis processing** — 250 Hz loop with ≤5 ms p99 latency and ≤0.5 ms p99 jitter
- **Zero-allocation RT core** — the hot path never blocks, allocates, or locks
- **Curve & response tuning** — deadzones, detents, S-curves, and mixers
- **Force feedback synthesis** — multi-effect FFB with safety interlocks (envelope limits, emergency stop)
- **Panel drivers** — Saitek/Logitech flight panels, Cougar MFDs, GoFlight modules, Stream Deck
- **HOTAS support** — native drivers for Saitek, Thrustmaster, VKB, VirPil, WinWing, Honeycomb, Brunner, CH Products, Simucube, and more
- **Profile cascade** — Global → Simulator → Aircraft → Phase-of-Flight layered profiles
- **Rule engine** — conditional LED/annunciator control driven by sim variables
- **Head tracking** — TrackIR and OpenTrack integration
- **VR overlay** — OpenXR-based in-sim overlay
- **Motion platforms** — motion cueing output
- **Plugin system** — WASM sandboxed, native fast-path, and service-tier plugins
- **gRPC IPC** — daemon (`flightd`) + CLI (`flightctl`) + GUI (`flight-ui`)
- **Observability** — structured tracing, Prometheus metrics, black-box flight recorder

## Supported Simulators

| Simulator | Crate | Protocol |
|---|---|---|
| **Microsoft Flight Simulator** (MSFS 2020/2024) | `flight-simconnect` | SimConnect SDK |
| **X-Plane** (11/12) | `flight-xplane` | UDP + plugin |
| **DCS World** | `flight-dcs-export` | Export.lua |
| **Prepar3D** | `flight-prepar3d` | SimConnect |
| **Falcon BMS** | `flight-falcon-bms` | Shared memory |
| **IL-2 Sturmovik** | `flight-il2` | Telemetry UDP |
| **Aerofly FS** | `flight-aerofly` | IPC bridge |
| **Ace Combat 7** | `flight-ac7-*` | Telemetry bridge |
| **War Thunder** | `flight-warthunder` | HTTP telemetry |
| **Kerbal Space Program** | `flight-ksp` | Telemetry bridge |
| **Elite Dangerous** | `flight-elite` | Journal API |
| **Sim Racing** (generic) | `flight-sim-racing` | Shared memory |

## Supported Hardware

OpenFlight ships device manifests for **2,200+** controllers, sticks, yokes, pedals, throttles, and panels. See [`COMPATIBILITY.md`](COMPATIBILITY.md) for the full device matrix with tier ratings and test coverage.

Highlights include: Thrustmaster Warthog/Viper, VKB Gladiator/Gunfighter, VirPil, WinWing, Honeycomb Alpha/Bravo, CH Products, Saitek/Logitech panels, GoFlight modules, Brunner CLS, Simucube, Stream Deck, and many more.

## Getting Started

> Full guide: [`docs/how-to/getting-started.md`](docs/how-to/getting-started.md)

### Prerequisites

- **Rust 1.92.0+** — install via [rustup](https://rustup.rs)
- **Windows:** Visual Studio C++ build tools + Windows SDK
- **Linux:** `build-essential pkg-config libusb-1.0-0-dev libudev-dev`
- **macOS:** Xcode command-line tools + `brew install libusb`

### Build & Run

```bash
# Build everything
cargo build --workspace

# Run the daemon
cargo run -p flight-service

# Use the CLI
cargo run -p flight-cli -- status
```

### Validate

```bash
cargo xtask check      # Fast: fmt + clippy + core tests
cargo xtask validate   # Full: tests, benches, API checks
make quick             # Clippy-strict + pattern verification
```

## Architecture

```
Non-RT:  Sim Adapters │ Panels │ Diagnostics │ IPC (gRPC)
              │  drop-tail lock-free queues  │
RT Spine (250 Hz):  Axis Engine │ FFB Engine │ Scheduler
```

The RT spine **never blocks, allocates, or takes locks**. Configuration changes are compiled off-thread and swapped atomically at tick boundaries. Platform RT scheduling uses MMCSS on Windows and rtkit on Linux.

Profiles cascade **Global → Simulator → Aircraft → Phase-of-Flight**; merging happens off-thread and the compiled result is atomically swapped into the spine.

See [`docs/explanation/adr/`](docs/explanation/adr/) for all Architecture Decision Records:

| ADR | Topic |
|-----|-------|
| [001](docs/explanation/adr/001-rt-spine-architecture.md) | RT spine architecture |
| [002](docs/explanation/adr/002-writers-as-data.md) | Writers as data (JSON diff tables) |
| [003](docs/explanation/adr/003-plugin-classes.md) | Plugin tiers (WASM / native / service) |
| [004](docs/explanation/adr/004-zero-allocation-constraint.md) | Zero-allocation constraint |
| [005](docs/explanation/adr/005-pll-timing-discipline.md) | PLL timing discipline |
| [006](docs/explanation/adr/006-driver-light-approach.md) | Driver-light approach |
| [007](docs/explanation/adr/007-pipeline-ownership-model.md) | Profile pipeline ownership |
| [008](docs/explanation/adr/008-ffb-mode-selection.md) | FFB mode selection |
| [009](docs/explanation/adr/009-safety-interlock-design.md) | Safety interlock design |
| [010](docs/explanation/adr/010-schema-versioning-strategy.md) | Schema versioning |
| [011](docs/explanation/adr/011-observability-architecture.md) | Observability architecture |

## Workspace Crates

> Full reference: [`docs/reference/crate-index.md`](docs/reference/crate-index.md)

| Group | Crates |
|-------|--------|
| **RT Core** | `flight-axis`, `flight-scheduler`, `flight-bus`, `flight-blackbox` |
| **Hardware** | `flight-hid`, `flight-ffb`, `flight-panels`, `flight-streamdeck`, `flight-tactile`, `flight-virtual`, `flight-motion` |
| **HOTAS Drivers** | `flight-hotas-saitek`, `-thrustmaster`, `-vkb`, `-virpil`, `-winwing`, `-honeycomb`, `-brunner`, `-ch`, `-logitech`, `-simucube`, `-vpforce`, `-microsoft`, `-sony`, `-turtlebeach` |
| **Panel Drivers** | `flight-panels-core`, `-saitek`, `-cougar`, `-goflight` |
| **Sim Adapters** | `flight-simconnect`, `-xplane`, `-dcs-export`, `-prepar3d`, `-falcon-bms`, `-il2`, `-aerofly`, `-ac7-*`, `-warthunder`, `-ksp`, `-elite`, `-sim-racing` |
| **Tracking & VR** | `flight-trackir`, `flight-opentrack`, `flight-openxr`, `flight-vr`, `flight-vr-overlay` |
| **Infrastructure** | `flight-core`, `-ipc`, `-profile`, `-rules`, `-writers`, `-units`, `-tracing`, `-metrics`, `-session`, `-updater`, `-watchdog`, `-security`, `-process-detection`, `-replay`, `-cloud-profiles`, `-plugin` |
| **Applications** | `flight-service` (`flightd`), `flight-cli` (`flightctl`), `flight-ui` |

Each crate documents its scope in `crates/<crate>/README.md`.

## Testing

OpenFlight uses a multi-layer test pyramid:

| Layer | What | How to run |
|-------|------|------------|
| **Unit** | Per-crate logic, pure functions | `cargo test -p <crate>` |
| **Property** | Invariant checking with proptest | `cargo test -p <crate> -- proptest` |
| **Snapshot** | Golden-file regression for writers | `cargo test -p flight-writers` |
| **Integration** | Cross-crate and IPC round-trips | `cargo test -p flight-integration-tests` |
| **FFB Safety** | Envelope limits and emergency stop | `cargo test -p flight-ffb safety` |
| **Fuzz** | `cargo-fuzz` on protocol parsers | See `crates/*/fuzz/` |
| **Mutation** | `cargo-mutants` spot checks | `cargo mutants -p flight-axis` |
| **Benchmarks** | Criterion micro-benchmarks | `cargo bench -p <crate>` |

Run everything: `cargo test --workspace`

## Documentation

Documentation follows the [Diátaxis](https://diataxis.fr/) framework:

- [`docs/how-to/`](docs/how-to/) — Task-oriented guides (getting started, testing, benchmarks)
- [`docs/explanation/`](docs/explanation/) — Architecture Decision Records and design rationale
- [`docs/reference/`](docs/reference/) — API reference, crate index, sim mappings
- [`docs/tutorials/`](docs/tutorials/) — Step-by-step tutorials
- [`docs/NOW_NEXT_LATER.md`](docs/NOW_NEXT_LATER.md) — Current project priorities

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for guidelines. Before submitting a PR:

1. Run `cargo xtask validate` (or `make all`)
2. Ensure strict clippy passes on the 8 core crates
3. Check [`docs/NOW_NEXT_LATER.md`](docs/NOW_NEXT_LATER.md) for priority alignment

## License

Licensed under either of:

- Apache-2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT ([`LICENSE-MIT`](LICENSE-MIT))

at your option.
