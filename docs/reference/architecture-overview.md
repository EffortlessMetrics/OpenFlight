---
doc_id: DOC-REF-ARCHITECTURE
title: "Architecture Overview"
status: active
category: reference
group: infrastructure
requirements:
  - REQ-1
adrs:
  - ADR-001
  - ADR-004
  - ADR-005
  - ADR-007
---

# Architecture Overview

OpenFlight is structured as a Rust workspace of ~95 crates organised
around a **protected real-time (RT) spine** that processes flight
control inputs at 250 Hz with sub-millisecond jitter.

## High-Level Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Non-RT Systems                           │
│                                                                 │
│  ┌──────────────┐  ┌──────────┐  ┌─────────────┐  ┌─────────┐ │
│  │ Sim Adapters │  │  Panels  │  │ Diagnostics │  │  gRPC   │ │
│  │ (MSFS/XP/DCS)│  │ (Saitek) │  │ (Metrics)   │  │  (IPC)  │ │
│  └──────┬───────┘  └────┬─────┘  └──────┬──────┘  └────┬────┘ │
│         │               │               │               │      │
│         └───────────────┴───────┬───────┴───────────────┘      │
│                                 │                               │
│               Drop-tail lock-free channels                      │
│                                 │                               │
├─────────────────────────────────┼───────────────────────────────┤
│                        RT Spine │(250 Hz)                       │
│                                 │                               │
│  ┌──────────────┐  ┌───────────▼──┐  ┌────────────────────┐   │
│  │ Axis Engine  │  │  Event Bus   │  │    FFB Engine       │   │
│  │ (flight-axis)│  │ (flight-bus) │  │  (flight-ffb)       │   │
│  └──────┬───────┘  └──────────────┘  └────────────────────┘   │
│         │                                                       │
│  ┌──────▼───────────────────────────────────────────────────┐  │
│  │              RT Scheduler (flight-scheduler)              │  │
│  │         MMCSS (Windows) · rtkit (Linux) · PLL             │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

The boundary between Non-RT and RT is enforced by **lock-free,
drop-tail channels**. Non-RT systems may allocate, block, and perform
I/O. The RT spine **never blocks, allocates, or takes locks** on its
hot path (see [ADR-004](../explanation/adr/004-zero-allocation-constraint.md)).

## Crate Dependency Graph

### RT Core

```
flight-scheduler
    ├── PLL timing (ADR-005)
    ├── MMCSS / rtkit backends
    └── SpscRing (lock-free queue)

flight-axis
    ├── AxisFrame (in_raw → out)
    ├── Processing nodes:
    │   Deadzone → Curve → Detent → Mixer
    │   Slew Limiter · EMA Filter
    │   Normalise · Calibrate · Invert
    └── AxisEngine (atomic pipeline swap)

flight-bus
    ├── EventRouter (fixed-size, allocation-free)
    ├── BusEvent / EventKind / EventPayload
    ├── BusPublisher (rate-limited)
    └── SimAdapter trait

flight-ffb
    ├── Force effect synthesis
    ├── Safety interlocks (ADR-009)
    └── Envelope enforcement
```

### Hardware

```
flight-hid ── flight-hid-types ── flight-hid-support
    │
    ├── flight-hotas-saitek      (X52 Pro, X65F)
    ├── flight-hotas-thrustmaster (T.Flight HOTAS)
    ├── flight-hotas-vkb          (Gladiator NXT EVO)
    ├── flight-hotas-virpil
    └── … (18 device crates)

flight-panels
    ├── flight-panels-core
    ├── flight-panels-saitek
    └── flight-panels-cougar

flight-streamdeck
flight-tactile
flight-virtual   (synthetic test harness)
```

### Simulator Adapters

```
flight-adapter-common
    ├── AdapterState machine
    ├── AdapterConfig trait
    └── ReconnectionStrategy

flight-simconnect ── flight-simconnect-sys (C FFI)
flight-xplane        (UDP)
flight-dcs-export    (Export.lua)
flight-ac7-protocol ── flight-ac7-telemetry ── flight-ac7-input
flight-sim-racing
```

### Infrastructure

```
flight-core       (types, profiles, aircraft detection)
flight-profile    (schema, validation, cascade)
flight-rules      (rule engine for panel/LED)
flight-ipc        (gRPC — tonic 0.14 / prost 0.14)
flight-units      (unit conversions)
flight-tracing    (structured logging)
flight-metrics    (Prometheus / Axum export)
flight-blackbox   (flight data recorder)
flight-session    (session persistence)
flight-watchdog   (timeout / overrun protection)
flight-security   (capability enforcement)
flight-updater    (self-update mechanism)
flight-writers    (data-driven sim configs — ADR-002)
```

### Applications

```
flight-service    (flightd daemon)
flight-cli        (flightctl command-line)
flight-ui         (GUI — future)
```

## Data Flow

### Input Path (Device → Simulator)

```
USB HID Report
    │
    ▼
flight-hotas-* : parse_report() → InputSnapshot
    │
    ▼
flight-bus : BusPublisher.publish(snapshot)
    │  (lock-free channel, drop-tail on overflow)
    ▼
flight-axis : AxisEngine.process_frame()
    │
    │  Pipeline: Deadzone → Curve → Detent → Mixer
    │            Slew Limiter · EMA Filter
    │
    ▼
AxisFrame { in_raw, out, d_in_dt, ts_mono_ns }
    │
    ▼
flight-simconnect / flight-xplane / … : write to simulator
```

### Configuration Path (Profile → RT Spine)

```
YAML Profile File
    │
    ▼
flight-profile : load + validate + canonicalise
    │
    ▼
flight-core : Profile::merge_with() (cascade)
    │
    │  Global → Simulator → Aircraft → Phase-of-Flight
    │
    ▼
Compiled pipeline (off-thread)
    │
    ▼
Atomic swap at tick boundary → AxisEngine
```

Profile compilation happens **off the RT thread**. The compiled
result is placed in a pending slot and atomically swapped into the
active slot at the next tick boundary. There is zero interruption
to input processing.

### Telemetry Path (Simulator → OpenFlight)

```
Simulator SDK / UDP / Export.lua
    │
    ▼
flight-*-adapter : poll() → RawFrame
    │
    ▼
SimAdapter::convert_to_snapshot() → BusSnapshot
    │
    ▼
flight-bus : EventRouter dispatches to subscribers
    │
    ├── Aircraft detection (triggers profile cascade)
    ├── Phase-of-flight detection (triggers PoF overrides)
    ├── Metrics / Blackbox recording
    └── UI / CLI subscribers
```

## Threading Model

OpenFlight uses a hybrid threading model:

| Thread | Purpose | Priority | Crate |
|--------|---------|----------|-------|
| **RT thread** | 250 Hz tick loop (axis + FFB processing) | Real-time (MMCSS/rtkit) | `flight-scheduler` |
| **Tokio runtime** | Async I/O, gRPC, adapter polling | Normal | `flight-service` |
| **HID polling** | USB device read loops | Elevated | `flight-hid` |
| **Profile compiler** | Off-thread profile merge + validation | Normal | `flight-core` |
| **Metrics server** | HTTP endpoint (Axum) | Low | `flight-metrics` |
| **Watchdog** | Timeout + overrun detection | Elevated | `flight-watchdog` |

### RT Thread Detail

The RT thread is managed by `flight-scheduler`:

1. **Absolute scheduling** — each tick targets a fixed wall-clock time
   (4 ms period at 250 Hz).
2. **PLL phase correction** — a software phase-locked loop
   ([ADR-005](../explanation/adr/005-pll-timing-discipline.md))
   corrects for OS scheduling jitter.
3. **Busy-spin tail** — the last ~65 µs of each tick uses a busy loop
   for sub-microsecond precision.
4. **Jitter tracking** — `JitterTracker` records per-tick latency.
   The quality gate target is **p99 ≤ 0.5 ms**.

### Communication Between Threads

| Channel | Direction | Mechanism |
|---------|-----------|-----------|
| Device → RT | HID → Axis Engine | `SpscRing` (lock-free, drop-tail) |
| RT → Simulator | Axis Engine → Adapter | `SpscRing` |
| Profile → RT | Compiler → Axis Engine | Atomic pointer swap |
| Adapter → Bus | Sim → Subscribers | `crossbeam` channel |
| Service → UI | Orchestrator → CLI/GUI | gRPC (tonic) |

All channels crossing the RT boundary use **drop-tail** policy:
if the consumer is slow, the oldest unread item is silently
discarded. The RT spine is **never** blocked.

## Service Lifecycle

The `flight-service` daemon (`flightd`) boots through an ordered
state machine:

```
Initializing → BusReady → SchedulerReady → AdaptersReady → Running
```

Shutdown is the reverse:

```
Running → Stopping (drain adapters → stop scheduler → close bus) → Stopped
```

### Degraded Modes

| Mode | Trigger | Behaviour |
|------|---------|-----------|
| `Running` | Normal operation | All systems active |
| `SafeMode` | Adapter/panel fault | Axis-only (no panels/plugins/tactile) |
| `Degraded` | Partial subsystem failure | Reduced functionality, logged |
| `Failed` | Unrecoverable error | Graceful shutdown initiated |

## Plugin Architecture (ADR-003)

Third-party extensions use a tiered model:

| Tier | Runtime | Frequency | Isolation | Use Case |
|------|---------|-----------|-----------|----------|
| **WASM** | wasmtime sandbox | 20–120 Hz | No file/net access | Telemetry displays, panel logic |
| **Native fast-path** | Helper process | Per-tick budget (100 µs) | SPSC + watchdog | Signal processing |
| **Service** | Managed thread | Event-driven | Full access (user consent) | Drivers, integrations |

Plugins declare capabilities in a manifest and require signature
verification before loading.

## Performance Targets

| Metric | Target | Enforcement |
|--------|--------|-------------|
| Axis processing latency | ≤ 5 ms p99 | `QG-SANITY-GATE` |
| RT tick jitter | ≤ 0.5 ms p99 | `QG-RT-JITTER` (hardware runners) |
| HID write latency | ≤ 300 µs p99 | `QG-HID-LATENCY` (hardware runners) |
| HID enumeration | < 100 ms | Unit test |
| Zero heap allocations on RT path | 0 per tick | CI gate + `AllocationGuard` |

## Key ADRs

| ADR | Topic | Summary |
|-----|-------|---------|
| [001](../explanation/adr/001-rt-spine-architecture.md) | RT Spine | Protected 250 Hz core, atomic swaps, drop-tail queues |
| [002](../explanation/adr/002-writers-as-data.md) | Writers as Data | JSON diff tables for sim configs, not code |
| [003](../explanation/adr/003-plugin-classes.md) | Plugin Tiers | WASM → Native → Service, increasing privilege |
| [004](../explanation/adr/004-zero-allocation-constraint.md) | Zero Allocation | No heap alloc on RT hot paths |
| [005](../explanation/adr/005-pll-timing-discipline.md) | PLL Timing | Software PLL for jitter control |
| [007](../explanation/adr/007-pipeline-ownership-model.md) | Pipeline Ownership | Global → Sim → Aircraft → PoF cascade |
| [009](../explanation/adr/009-safety-interlock-design.md) | Safety Interlocks | FFB fault detection and ramp-to-zero |

## See Also

- [Configuration Reference](configuration.md) — profile schema
- [Supported Hardware](supported-hardware.md) — device matrix
- [Getting Started](../how-to/getting-started.md) — build and run
- [Quality Gates](../explanation/quality-gates.md) — CI enforcement
