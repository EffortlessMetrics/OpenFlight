# flight-warthunder

War Thunder HTTP telemetry adapter for [Flight Hub](https://flight-hub.dev).

War Thunder exposes a local HTTP API at `http://localhost:8111` while the game is running. This adapter polls the `/indicators` endpoint and publishes normalised [`BusSnapshot`] values to the Flight Hub bus.

## Prerequisites

- War Thunder running on the same machine
- No extra configuration required — the HTTP API is enabled by default

## Features

- Airspeed (IAS/TAS), altitude, heading, pitch, roll, G-force, vertical speed
- Engine RPM and throttle position
- Automatic start/stop detection (polls until the endpoint responds)
- Configurable polling rate (default 10 Hz)
- Bus snapshot publishing

## Usage

```rust
use flight_warthunder::{WarthunderAdapter, WarthunderConfig};

#[tokio::main]
async fn main() {
    let adapter = WarthunderAdapter::new(WarthunderConfig::default());
    adapter.start().await;

    if let Some(snap) = adapter.current_snapshot().await {
        println!("IAS: {} m/s", snap.kinematics.indicated_airspeed);
    }

    adapter.stop().await;
}
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
