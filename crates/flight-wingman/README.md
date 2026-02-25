# flight-wingman

Project Wingman process-detection adapter for [Flight Hub](https://flight-hub.dev).

Project Wingman (Simmer Entertainment, 2021) exposes no in-process telemetry API. This adapter integrates at the process level — detecting when the game launches and routing processed HOTAS output through a virtual XInput device.

## Features

- Process detection: activates Flight Hub profiles automatically when `ProjectWingman.exe` is running
- Presence snapshot: publishes a presence `BusSnapshot` (all validity flags `false`) to signal the game is active
- Virtual controller: stub `VirtualController` abstraction for routing axis/button output

## Virtual Controller

The stub implementation (`StubVirtualController`) logs axis/button values but does not create a real virtual device. To produce actual XInput output on Windows, install **ViGEm Bus** (<https://github.com/nefarius/ViGEmBus>) and replace the stub with a ViGEm-backed controller.

## Usage

```rust
use flight_wingman::WingmanAdapter;

#[tokio::main]
async fn main() {
    let adapter = WingmanAdapter::new();
    adapter.start().await;
    // Profiles activate automatically when ProjectWingman.exe is detected
}
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
