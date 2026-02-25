# flight-hotas-winwing

WinWing HOTAS driver for [Flight Hub](https://flight-hub.dev).

Supports the **Orion 2 Throttle**, **Orion 2 F/A-18C Stick**, and **TFRP Rudder Pedals** via USB HID.

## USB Identifiers

| Product               | VID    | PID    |
|-----------------------|--------|--------|
| Orion 2 Throttle      | 0x4098 | 0xBE62 |
| Orion 2 F/A-18C Stick | 0x4098 | 0xBE63 |
| TFRP Rudder Pedals    | 0x4098 | 0xBE64 |

## Features

- Throttle: dual-lever axes, ministick (X/Y), rotary dials, 30+ buttons
- Stick: roll (X), pitch (Y), twist (RZ), ministick, 12 buttons + 4-way HAT
- Rudder pedals: left/right toe brakes, rudder axis, anti-splay adjustment
- Ghost-filter for potentiometer jitter suppression
- Health monitor per device: connectivity, ghost-input rate, failure tracking
- Preset axis configs tuned to WinWing hardware

## Usage

```rust
use flight_hotas_winwing::input::{parse_throttle_report, THROTTLE_REPORT_LEN};

let raw = [0u8; THROTTLE_REPORT_LEN]; // replace with real HID report
let state = parse_throttle_report(&raw).unwrap();
let combined = state.axes.throttle_combined;
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
