---
doc_id: DOC-REF-CRATE-INDEX
kind: reference
area: architecture
status: active
links:
  requirements: []
  tasks: []
  adrs: [ADR-001, ADR-003, ADR-004]
---

# Crate Index

Reference index for the major workspace crates in OpenFlight, grouped by
functional area. Each crate has its own `README.md` in `crates/<name>/`.

---

## RT Core

These crates form the 250 Hz real-time processing spine. They follow the
**zero-allocation constraint** (ADR-004) on hot paths.

| Crate | Description |
|-------|-------------|
| `flight-axis` | Axis processing engine — curves, deadzones, detents, mixers. Runs at 250 Hz. |
| `flight-scheduler` | Platform RT thread scheduling (MMCSS on Windows, rtkit on Linux). |
| `flight-bus` | Lock-free event bus for inter-component communication. |
| `flight-blackbox` | Flight-recorder style ring-buffer telemetry capture. |

## Hardware — HID & Device Layer

| Crate | Description |
|-------|-------------|
| `flight-hid` | HID device discovery, enumeration, and I/O. |
| `flight-hid-support` | Shared HID utilities and platform abstraction. |
| `flight-hid-types` | Common HID type definitions (vendor/product IDs, report descriptors). |
| `flight-device-common` | Cross-device abstractions shared by HOTAS and panel drivers. |
| `flight-virtual` | Virtual device emulation for testing and bridging. |
| `flight-macos-hid` | macOS-specific IOKit HID backend. |
| `flight-open-hardware` | Support for community open-hardware controllers. |

## Hardware — Force Feedback

| Crate | Description |
|-------|-------------|
| `flight-ffb` | Force feedback synthesis engine with safety interlocks (ADR-009). |
| `flight-ffb-vpforce` | VPForce Rhino direct-drive FFB driver. |
| `flight-ffb-moza` | MOZA direct-drive FFB driver. |
| `flight-tactile` | Tactile/haptic transducer output (bass shakers). |

## Hardware — HOTAS Drivers

Vendor-specific drivers for sticks, throttles, pedals, and wheel bases.

| Crate | Description |
|-------|-------------|
| `flight-hotas-saitek` | Saitek/Logitech X52, X56, yoke, pedals. |
| `flight-hotas-thrustmaster` | Thrustmaster Warthog, Viper TQS, TWCS, T.16000M. |
| `flight-hotas-vkb` | VKB Gladiator, Gunfighter, STECS. |
| `flight-hotas-virpil` | VirPil WarBRD, MongoosT-50, throttle, pedals. |
| `flight-hotas-winwing` | WinWing Orion, Super Libra, panels. |
| `flight-hotas-honeycomb` | Honeycomb Alpha yoke, Bravo throttle. |
| `flight-hotas-brunner` | Brunner CLS-E/P force-feedback yoke/stick. |
| `flight-hotas-ch` | CH Products Fighterstick, Pro Throttle, pedals. |
| `flight-hotas-logitech` | Logitech Extreme 3D Pro, G X52/X56. |
| `flight-hotas-logitech-wheel` | Logitech racing wheels (G29, G923, etc.). |
| `flight-hotas-simucube` | Simucube direct-drive wheel bases. |
| `flight-hotas-vpforce` | VPForce Rhino joystick base. |
| `flight-hotas-microsoft` | Microsoft SideWinder series. |
| `flight-hotas-sony` | Sony DualSense / DualShock (flight-sim usage). |
| `flight-hotas-turtlebeach` | Turtle Beach VelocityOne controllers. |

## Hardware — Panel Drivers

| Crate | Description |
|-------|-------------|
| `flight-panels` | High-level panel manager and dispatch. |
| `flight-panels-core` | Shared panel abstractions (display, LED, encoder, switch). |
| `flight-panels-saitek` | Saitek/Logitech flight instrument panels (radio, multi, switch). |
| `flight-panels-cougar` | Thrustmaster Cougar MFD panels. |
| `flight-panels-goflight` | GoFlight GF-series avionics modules. |
| `flight-streamdeck` | Elgato Stream Deck integration for custom button pages. |

## Simulator Adapters

| Crate | Description |
|-------|-------------|
| `flight-simconnect` | Microsoft Flight Simulator adapter (SimConnect SDK). |
| `flight-simconnect-sys` | Raw FFI bindings for SimConnect. |
| `flight-xplane` | X-Plane 11/12 adapter (UDP dataref protocol). |
| `flight-xplane-plugin` | X-Plane native plugin (XPLM SDK). |
| `flight-dcs-export` | DCS World adapter (Export.lua). |
| `flight-dcs-modules` | DCS per-module variable tables. |
| `flight-prepar3d` | Prepar3D adapter (SimConnect). |
| `flight-falcon-bms` | Falcon BMS adapter (shared memory). |
| `flight-il2` | IL-2 Sturmovik adapter (telemetry UDP). |
| `flight-aerofly` | Aerofly FS adapter. |
| `flight-ac7-protocol` | Ace Combat 7 telemetry protocol definitions. |
| `flight-ac7-telemetry` | AC7 telemetry receiver. |
| `flight-ac7-input` | AC7 input injection. |
| `flight-warthunder` | War Thunder HTTP telemetry adapter. |
| `flight-ksp` | Kerbal Space Program telemetry adapter. |
| `flight-elite` | Elite Dangerous journal API adapter. |
| `flight-sim-racing` | Generic sim-racing shared-memory adapter. |
| `flight-wingman` | AI wingman / voice-command bridge. |
| `flight-adapter-common` | Shared traits and utilities for all sim adapters. |

## Tracking & VR

| Crate | Description |
|-------|-------------|
| `flight-trackir` | NaturalPoint TrackIR head-tracking integration. |
| `flight-opentrack` | OpenTrack UDP head-tracking protocol. |
| `flight-openxr` | OpenXR runtime interface. |
| `flight-vr` | VR headset abstraction layer. |
| `flight-vr-overlay` | In-sim VR overlay rendering. |

## Motion

| Crate | Description |
|-------|-------------|
| `flight-motion` | Motion platform cueing output. |

## Infrastructure

| Crate | Description |
|-------|-------------|
| `flight-core` | Core domain types, aircraft detection, shared error types. |
| `flight-ipc` | gRPC-based IPC (tonic 0.14 / prost 0.14). Proto sources in `crates/flight-ipc/proto/`. |
| `flight-profile` | Profile schema, validation, and cascade merging. |
| `flight-rules` | Rule engine for conditional panel/LED behaviour. |
| `flight-writers` | Sim-variable writer configs as JSON diff tables (ADR-002). |
| `flight-units` | Dimensional-analysis unit types (degrees, radians, Newtons, etc.). |
| `flight-tracing` | Structured tracing setup and span conventions. |
| `flight-metrics` | Prometheus metrics registry and exporters. |
| `flight-metrics-http` | HTTP endpoint for Prometheus scraping. |
| `flight-session` | Session lifecycle management. |
| `flight-updater` | Self-update mechanism. |
| `flight-watchdog` | Health monitoring and automatic recovery. |
| `flight-security` | Permission model and capability checking. |
| `flight-process-detection` | Detects running simulators by process name. |
| `flight-replay` | Telemetry replay from black-box recordings. |
| `flight-cloud-profiles` | Cloud-synced profile storage. |
| `flight-plugin` | Plugin host runtime (WASM, native, service tiers — ADR-003). |

## Applications

| Crate | Description |
|-------|-------------|
| `flight-service` | Main daemon binary (`flightd`). Orchestrates all subsystems. |
| `flight-cli` | Command-line interface (`flightctl`). Communicates with `flightd` via gRPC. |
| `flight-ui` | Graphical configuration interface. |
| `flight-headless` | Headless mode for CI and automated testing. |

## Internal / Testing

| Crate | Description |
|-------|-------------|
| `flight-test-helpers` | Shared test utilities, fixtures, and mocks. |
| `flight-integration-tests` | Cross-crate integration test suite. |
| `flight-bdd-metrics` | BDD requirement coverage metrics. |
| `flight-workspace-meta` | Workspace metadata and code-generation helpers. |
