# flight-ffb-vpforce

VPforce Rhino FFB joystick driver for [Flight Hub](https://flight-hub.dev).

Provides HID report parsing, force-feedback effect output, and health monitoring for the **VPforce Rhino** (revisions 2 and 3).

## USB Identifiers

| Product   | VID    | PID    |
|-----------|--------|--------|
| Rhino v2  | 0x0483 | 0xA1C0 |
| Rhino v3  | 0x0483 | 0xA1C1 |

## Features

- Full axis parsing: roll (X), pitch (Y), throttle, twist (RZ), and slider
- 32 buttons + 8-way HAT
- FFB effect output: spring, damper, friction, constant force, sinusoidal
- Ghost-filter for potentiometer jitter suppression
- Health monitor: connectivity, ghost-input rate, consecutive failure tracking
- Preset axis configs tuned to Rhino hardware

## Usage

```rust
use flight_ffb_vpforce::input::{parse_report, RHINO_REPORT_LEN};
use flight_ffb_vpforce::effects::{FfbEffect, serialize_effect};

let raw = [0u8; RHINO_REPORT_LEN]; // replace with real HID report
let state = parse_report(&raw).unwrap();

// Send a spring centering effect
let report = serialize_effect(FfbEffect::Spring { coefficient: 0.4 });
```

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
