---
doc_id: DOC-HOWTO-GETTING-STARTED
title: "Getting Started with OpenFlight"
status: active
category: how-to
group: infrastructure
requirements:
  - INF-REQ-8
---

# Getting Started with OpenFlight

This guide walks you through installing OpenFlight, building from source,
connecting your first device, applying a profile, and running with a simulator.

## Prerequisites

### Rust Toolchain

OpenFlight requires **Rust 1.92.0 or later** (2024 edition).

```bash
# Install via rustup (https://rustup.rs)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify version
rustc --version   # Must be >= 1.92.0
```

### Platform Dependencies

**Windows** (primary platform):
- Windows 10/11
- Windows SDK (for HID support) — installed automatically with Visual Studio
  Build Tools or the full Visual Studio installation
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  with the "Desktop development with C++" workload

**Linux**:
- libudev development headers:
  ```bash
  # Debian / Ubuntu
  sudo apt install libudev-dev

  # Fedora
  sudo dnf install systemd-devel
  ```

### Optional Tools

| Tool | Purpose | Install |
|------|---------|---------|
| `cargo-nextest` | Faster test runner | `cargo install cargo-nextest` |
| `cargo-watch` | Auto-rebuild on save | `cargo install cargo-watch` |
| `cargo-public-api` | API compatibility checks | `cargo install cargo-public-api` |

## Clone and Build

```bash
git clone https://github.com/EffortlessMetrics/OpenFlight.git
cd OpenFlight

# Development build (faster compilation)
cargo build --workspace

# Release build (optimised)
cargo build --release --workspace

# RT-optimised build with debug symbols
cargo build --profile rt --workspace
```

### Verify the Build

Run the quick smoke-test to confirm everything compiles and passes lint:

```bash
cargo xtask check          # fmt + clippy + core tests
```

For a full validation (tests, benchmarks, API checks):

```bash
cargo xtask validate
```

## Run Without Hardware (Virtual Harness)

You can explore OpenFlight without any physical devices using the virtual
device harness:

```bash
cargo run -p flight-virtual
```

This spawns synthetic axis and button inputs so you can see the processing
pipeline in action.

## Connect a Device

1. **Plug in** your flight stick, throttle, or panel via USB.

2. **Check detection** — the `flightctl` CLI lists recognised devices:
   ```bash
   cargo run -p flight-cli -- devices list
   ```
   You should see your device with its vendor ID, product ID, and
   detected capabilities (axes, buttons, hats, FFB).

3. **Supported hardware** — see [Supported Hardware](../reference/supported-hardware.md)
   for the device compatibility matrix. Over 2 200 devices are catalogued
   across five support tiers.

> **Tip:** If your device is not listed, OpenFlight will still attempt
> generic HID enumeration. You can contribute a device manifest — see
> [Adding a Device](adding-a-device.md).

## Apply a Profile

Profiles control how raw device input is transformed before it reaches
the simulator. OpenFlight uses a hierarchical cascade:

**Global → Simulator → Aircraft → Phase-of-Flight**

More-specific profiles override less-specific ones.

### Create a Basic Profile

Create a file `my-profile.yaml`:

```yaml
schema: "flight.profile/1"

axes:
  x:
    deadzone:
      center: 0.05
      edge: 0.02
    expo: 0.3
    slew_rate: 50.0
  y:
    deadzone:
      center: 0.05
      edge: 0.02
    expo: 0.3
  z:
    deadzone:
      center: 0.08
```

### Load the Profile

```bash
cargo run -p flight-cli -- profile load my-profile.yaml
```

The profile is compiled off-thread and atomically swapped into the
250 Hz processing loop at the next tick boundary — there is no
interruption to input processing.

See [Configuration Reference](../reference/configuration.md) for the
full profile schema, including curves, detents, filters, and
per-aircraft overrides.

## Run with a Simulator

### Microsoft Flight Simulator (MSFS)

1. Ensure SimConnect is available (ships with MSFS).
2. Start MSFS.
3. Start the OpenFlight daemon:
   ```bash
   cargo run -p flight-service
   ```
4. The adapter auto-detects the running simulator and transitions
   through `Connecting → Connected → DetectingAircraft → Active`.

### X-Plane

1. Start X-Plane.
2. Start the daemon:
   ```bash
   cargo run -p flight-service
   ```
3. OpenFlight communicates via X-Plane's UDP data interface.

### DCS World

1. Start DCS.
2. Start the daemon:
   ```bash
   cargo run -p flight-service
   ```
3. OpenFlight uses the DCS Export.lua interface for telemetry.

> **Note:** Each simulator adapter follows the same state-machine model
> described in the [Architecture Overview](../reference/architecture-overview.md).
> Aircraft auto-switching and profile cascade happen automatically once
> the adapter reaches the `Active` state.

## Next Steps

- [Configuration Reference](../reference/configuration.md) — full
  profile schema
- [Architecture Overview](../reference/architecture-overview.md) — how
  the real-time spine works
- [Adding a Device](adding-a-device.md) — contribute support for new
  hardware
- [Adding a Simulator](adding-a-simulator.md) — write a new sim adapter
- [Troubleshooting](troubleshoot-common-issues.md) — common issues and
  fixes
