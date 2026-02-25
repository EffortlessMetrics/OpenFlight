# flight-ffb-moza

Moza flight peripheral driver for [Flight Hub](https://flight-hub.dev).

Supports the **AB9 FFB Base** and **R3 FFB Base** with joystick modules for flight simulation.

## USB Identifiers

| Product      | VID    | PID    |
|--------------|--------|--------|
| AB9 FFB Base | 0x346E | 0x0005 |
| R3 FFB Base  | 0x346E | 0x0002 |

## Features

- Full axis parsing: roll (X), pitch (Y), and optional twist (RZ)
- 12 buttons + 4-way HAT
- FFB torque output: direct X/Y force commands via HID output reports
- Ghost-filter for potentiometer jitter suppression
- Health monitor: connectivity, ghost-input rate, consecutive failure tracking
- Preset axis configs tuned to Moza hardware

## Usage

```rust
use flight_ffb_moza::input::{parse_ab9_report, AB9_REPORT_LEN};
use flight_ffb_moza::effects::TorqueCommand;

let raw = [0u8; AB9_REPORT_LEN]; // replace with real HID report
let state = parse_ab9_report(&raw).unwrap();

// Send a centering torque proportional to deflection
let report = TorqueCommand {
    x: -state.axes.roll * 0.3,
    y: -state.axes.pitch * 0.3,
}
.to_report();
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
