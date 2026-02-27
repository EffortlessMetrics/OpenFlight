# flight-prepar3d

Lockheed Martin Prepar3D adapter for OpenFlight.

## Overview

Prepar3D (P3D) ships its own `SimConnect.dll`, which shares the same API
surface as Microsoft Flight Simulator's SimConnect. This crate provides a thin
adapter layer that reuses `flight-adapter-common` patterns to bridge P3D data
into the OpenFlight axis and FFB pipeline.

## Status

Stub implementation — SimConnect bindings will be wired in a future release.
The `simulate_connect` / `simulate_disconnect` / `process_data` interface
reflects the final production API.

## Usage

```rust
use flight_prepar3d::{Prepar3DAdapter, P3DFlightData, P3DState};

let mut adapter = Prepar3DAdapter::new();
assert_eq!(adapter.state(), P3DState::Disconnected);

// In production: call SimConnect_Open and receive callbacks.
adapter.simulate_connect("5.3");
assert_eq!(adapter.state(), P3DState::Connected);
```

## License

MIT OR Apache-2.0
