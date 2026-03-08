---
doc_id: DOC-HOWTO-ADDING-SIMULATOR
title: "How to Add a New Simulator Adapter"
status: active
category: how-to
group: flight-adapter-common
requirements:
  - REQ-5
  - REQ-6
  - REQ-7
---

# How to Add a New Simulator Adapter

This guide explains how to write a simulator adapter that connects
OpenFlight to a new flight (or racing) simulator.

## Prerequisites

- Familiarity with the target simulator's SDK or data export API
- Understanding of the [Architecture Overview](../reference/architecture-overview.md),
  specifically the non-RT / RT boundary

## Architecture Overview

Every simulator adapter lives in its own crate and communicates with
the RT spine through the **event bus**. Adapters run on the **non-RT**
side — they may allocate, perform I/O, and use `async` code freely.

```
┌───────────────────────────────┐
│  Simulator Process            │
│  (MSFS / X-Plane / DCS / …)  │
└──────────┬────────────────────┘
           │  SDK / UDP / Export.lua
┌──────────▼────────────────────┐
│  flight-mysim (adapter crate) │
│  AdapterState machine         │
│  BusSnapshot conversion       │
└──────────┬────────────────────┘
           │  lock-free bus channel
┌──────────▼────────────────────┐
│  RT Spine (250 Hz)            │
│  Axis Engine · FFB Engine     │
└───────────────────────────────┘
```

## Step 1 — Create the Adapter Crate

```bash
cargo init crates/flight-mysim --lib
```

Add it to the workspace in the root `Cargo.toml` and use workspace
dependencies:

```toml
# crates/flight-mysim/Cargo.toml
[dependencies]
flight-adapter-common = { workspace = true }
flight-bus            = { workspace = true }
flight-core           = { workspace = true }
thiserror             = { workspace = true }
tokio                 = { workspace = true }
tracing               = { workspace = true }
```

## Step 2 — Implement the Adapter State Machine

All adapters follow a six-state lifecycle defined in
`flight-adapter-common`:

```
Disconnected ──► Connecting ──► Connected ──► DetectingAircraft ──► Active
      ▲                                                                │
      └────────────────── Error ◄──────────────────────────────────────┘
```

### `AdapterState`

| State | Meaning |
|-------|---------|
| `Disconnected` | No connection to the simulator |
| `Connecting` | Handshake / SDK initialisation in progress |
| `Connected` | Link established, no aircraft data yet |
| `DetectingAircraft` | Polling for the active aircraft type |
| `Active` | Fully operational — telemetry flowing |
| `Error` | Unrecoverable fault (triggers reconnect) |

### Implement `AdapterConfig`

```rust
use flight_adapter_common::{AdapterConfig, ReconnectionStrategy};

pub struct MySimConfig {
    pub host: String,
    pub port: u16,
}

impl AdapterConfig for MySimConfig {
    fn publish_rate_hz(&self) -> u32 { 30 }
    fn connection_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }
    fn max_reconnect_attempts(&self) -> u32 { 5 }
    fn enable_auto_reconnect(&self) -> bool { true }
}
```

### Drive State Transitions

```rust
pub struct MySimAdapter {
    state: AdapterState,
    config: MySimConfig,
    metrics: AdapterMetrics,
}

impl MySimAdapter {
    pub async fn connect(&mut self) -> Result<(), AdapterError> {
        self.state = AdapterState::Connecting;
        // Perform SDK handshake / open socket …
        self.state = AdapterState::Connected;
        Ok(())
    }

    pub async fn poll(&mut self) -> Result<Option<RawFrame>, AdapterError> {
        match self.state {
            AdapterState::Active => {
                // Read telemetry from the simulator
                let frame = self.read_frame().await?;
                Ok(Some(frame))
            }
            _ => Ok(None),
        }
    }
}
```

### Reconnection

Use the built-in `ReconnectionStrategy` for exponential backoff:

```rust
let strategy = ReconnectionStrategy::new(
    self.config.max_reconnect_attempts(),
    Duration::from_secs(1),   // initial delay
    Duration::from_secs(30),  // max delay
);
```

When the adapter enters `Error`, the service orchestrator will apply
the reconnection strategy automatically.

## Step 3 — Convert to `BusSnapshot` and Publish

The bus expects a normalised `BusSnapshot`. Implement the `SimAdapter`
trait from `flight-bus`:

```rust
use flight_bus::{SimAdapter, BusSnapshot};

impl SimAdapter for MySimAdapter {
    fn sim_id(&self) -> &str {
        "mysim"
    }

    fn convert_to_snapshot(&self, raw: &RawFrame) -> BusSnapshot {
        BusSnapshot {
            // Map simulator-specific variables to normalised values
            pitch: raw.pitch_deg / 90.0,
            bank:  raw.bank_deg / 180.0,
            ias:   raw.airspeed_kts,
            // …
        }
    }

    fn validate_raw_data(&self, raw: &RawFrame) -> bool {
        // Reject NaN / Inf / implausible jumps
        raw.pitch_deg.is_finite() && raw.bank_deg.is_finite()
    }
}
```

### Publish to the Bus

Use `BusPublisher` for rate-limited, non-blocking publication:

```rust
use flight_bus::BusPublisher;

let publisher = BusPublisher::new(config);

// In your polling loop:
if let Some(frame) = adapter.poll().await? {
    if adapter.validate_raw_data(&frame) {
        let snapshot = adapter.convert_to_snapshot(&frame);
        publisher.publish(snapshot)?;
    }
}
```

The bus uses **drop-tail** back-pressure: if a subscriber is too slow,
the oldest unread message is discarded — the RT spine is never blocked.

## Step 4 — Register with the Service

Add your adapter to the service orchestrator in
`crates/flight-service/src/orchestrator.rs` so it is started during the
`AdaptersReady` boot phase.

The orchestrator will:

1. Spawn your adapter as a Tokio task
2. Monitor its `AdapterState`
3. Apply reconnection on `Error`
4. Trigger aircraft auto-switching when `DetectingAircraft` resolves

## Step 5 — Write Tests

### Unit Tests

Test snapshot conversion with known telemetry frames:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_level_flight() {
        let raw = RawFrame { pitch_deg: 0.0, bank_deg: 0.0, airspeed_kts: 120.0 };
        let adapter = MySimAdapter::new(MySimConfig::default());
        let snap = adapter.convert_to_snapshot(&raw);
        assert!((snap.pitch).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_nan_telemetry() {
        let raw = RawFrame { pitch_deg: f32::NAN, bank_deg: 0.0, airspeed_kts: 0.0 };
        let adapter = MySimAdapter::new(MySimConfig::default());
        assert!(!adapter.validate_raw_data(&raw));
    }
}
```

### State Machine Tests

Verify correct transitions:

```rust
#[tokio::test]
async fn state_transitions() {
    let mut adapter = MySimAdapter::new(MySimConfig::default());
    assert_eq!(adapter.state, AdapterState::Disconnected);

    adapter.connect().await.unwrap();
    assert_eq!(adapter.state, AdapterState::Connected);
}
```

### Integration Tests

If you have access to the simulator, add an ignored integration test:

```rust
#[tokio::test]
#[ignore] // Requires running simulator
async fn live_telemetry_stream() {
    let mut adapter = MySimAdapter::new(MySimConfig::default());
    adapter.connect().await.unwrap();
    let frame = adapter.poll().await.unwrap();
    assert!(frame.is_some());
}
```

### Run Tests

```bash
cargo test -p flight-mysim
```

## Step 6 — Add a Game Manifest

Create `compat/games/mysim.yaml` describing the simulator's
capabilities and data interface:

```yaml
schema_version: "1"

game:
  name: "MySim"
  developer: "MySim Studios"
  interface: "udp"
  platforms: [windows, linux]

data_exports:
  telemetry: true
  aircraft_detection: true
  force_feedback: false

support:
  tier: 3
```

Regenerate the compatibility matrix:

```bash
cargo xtask gen-compat
```

## Checklist

- [ ] Crate created with workspace dependencies
- [ ] `AdapterConfig` implemented
- [ ] State machine drives `Disconnected` → `Active`
- [ ] `SimAdapter` trait implemented (`convert_to_snapshot`, `validate_raw_data`)
- [ ] Publishing via `BusPublisher`
- [ ] Registered in the service orchestrator
- [ ] Unit tests for conversion and validation
- [ ] State-machine transition tests
- [ ] Game manifest added to `compat/games/`
- [ ] `cargo xtask check` passes
- [ ] PR opened against `main`

## Reference

- [Architecture Overview](../reference/architecture-overview.md) —
  RT / non-RT boundary
- [Flight Core Concepts](../explanation/flight-core.md) — profile
  and aircraft switching
- Existing adapters:
  - `crates/flight-simconnect/` — MSFS (SimConnect SDK)
  - `crates/flight-xplane/` — X-Plane (UDP)
  - `crates/flight-dcs-export/` — DCS (Export.lua)
