# flight-ksp

Kerbal Space Program (kRPC) adapter for [Flight Hub](https://flight-hub.dev).

Connects to a running KSP instance via the [kRPC](https://krpc.github.io/krpc/) mod and streams vessel telemetry into the Flight Hub bus.

## Prerequisites

- KSP 1.x or KSP 2 with the kRPC server mod installed
- kRPC server running and listening (default: `localhost:50000`)

## Features

- Full telemetry: position, altitude (ASL + AGL), velocity (surface/orbital), heading, pitch, roll
- Resource monitoring: fuel, oxidizer, electric charge, monopropellant
- Stage and vessel state tracking
- Automatic reconnection with configurable retry policy
- Bus snapshot publishing at configurable polling rate

## Usage

```rust
use flight_ksp::{KspAdapter, KspConfig};

#[tokio::main]
async fn main() {
    let adapter = KspAdapter::new(KspConfig::default());
    adapter.start().await;

    if let Some(snap) = adapter.current_snapshot().await {
        println!("altitude: {} m", snap.environment.altitude);
    }

    adapter.stop().await;
}
```

## Configuration

`KspConfig` exposes host/port, polling interval, and reconnect behaviour. Defaults connect to `localhost:50000` at 10 Hz.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
