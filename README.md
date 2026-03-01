# Flight Hub

[![Repo](https://img.shields.io/badge/repo-EffortlessMetrics%2FOpenFlight-blue?logo=github&style=flat-square)](https://github.com/EffortlessMetrics/OpenFlight)

Flight Hub is a PC flight simulation input management system written in Rust. It provides a unified control plane for flight controls, panels, force feedback devices, and simulator adapters.

## Project Status

| Metric | Count |
|--------|-------|
| Workspace crates | 91 |
| Unit / integration tests | 9,200+ |
| Device manifests | 2,301 |
| Supported games | 18 |
| BDD feature files | 1,038 |
| BDD scenarios | 5,300+ |
| Fuzz targets | 92 |

## Product Posture

Flight Hub is an accessory/input manager that requires a simulator such as MSFS, X-Plane, or DCS. Experimental AC7 support is available via user-provided telemetry bridges. It does not emulate or replace any simulator.

For simulator integration boundaries and compliance notes, see `docs/product-posture.md`.

## Workspace Crates

- **Real-time spine:** `flight-axis`, `flight-scheduler`, `flight-bus`
- **Applications:** `flight-service`, `flight-cli`, `flight-ui`, `flight-headless`
- **IPC:** `flight-ipc`
- **Core domain:** `flight-core`, `flight-profile`, `flight-rules`, `flight-units`, `flight-session`
- **Simulator adapters:** `flight-simconnect`, `flight-simconnect-sys`, `flight-xplane`, `flight-xplane-plugin`, `flight-dcs-export`, `flight-dcs-modules`, `flight-prepar3d`, `flight-ac7-protocol`, `flight-ac7-telemetry`, `flight-ac7-input`, `flight-adapter-common`, `flight-sim-racing`, `flight-elite`, `flight-il2`, `flight-falcon-bms`, `flight-ksp`, `flight-warthunder`, `flight-psx`, `flight-aerofly`, `flight-opentrack`
- **Device and hardware:** `flight-hid`, `flight-hid-support`, `flight-hid-types`, `flight-device-common`, `flight-virtual`, `flight-macos-hid`
- **HOTAS vendor drivers:** `flight-hotas-brunner`, `flight-hotas-ch`, `flight-hotas-honeycomb`, `flight-hotas-logitech`, `flight-hotas-logitech-wheel`, `flight-hotas-microsoft`, `flight-hotas-saitek`, `flight-hotas-simucube`, `flight-hotas-sony`, `flight-hotas-thrustmaster`, `flight-hotas-turtlebeach`, `flight-hotas-virpil`, `flight-hotas-vkb`, `flight-hotas-vpforce`, `flight-hotas-winwing`
- **Panel and control hardware:** `flight-panels`, `flight-panels-core`, `flight-panels-saitek`, `flight-panels-cougar`, `flight-panels-goflight`, `flight-streamdeck`
- **Force feedback:** `flight-ffb`, `flight-ffb-moza`, `flight-ffb-vpforce`, `flight-tactile`
- **Safety and diagnostics:** `flight-watchdog`, `flight-blackbox`, `flight-tracing`, `flight-metrics`, `flight-metrics-http`
- **Persistence and platform:** `flight-writers`, `flight-updater`, `flight-security`, `flight-process-detection`, `flight-replay`, `flight-installer`
- **Extended features:** `flight-cloud-profiles`, `flight-motion`, `flight-open-hardware`, `flight-openxr`, `flight-vr`, `flight-vr-overlay`, `flight-trackir`, `flight-wingman`, `flight-plugin`
- **Testing and tooling:** `flight-test-helpers`, `flight-integration-tests`, `flight-bdd-metrics`, `flight-workspace-meta`, `flight-hub-examples`, `specs`, `xtask`

Each crate documents its scope in `crates/<crate>/README.md`.

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

## Build And Validate

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

Workspace helpers:

```bash
cargo xtask check
cargo xtask validate
make quick
make all
```

## Performance Constraints

- 250Hz real-time processing loop
- Axis processing latency target: <=5ms p99
- Jitter target: <=0.5ms p99
- Zero allocations on RT hot paths

## License

Licensed under either:

- Apache-2.0 (`LICENSE-APACHE`)
- MIT (`LICENSE-MIT`)
