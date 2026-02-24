# flight-open-hardware

OpenFlight Reference Hardware — HID protocol definitions for [Flight Hub](https://flight-hub.dev).

This `#![no_std]` crate defines the USB HID report format for the **OpenFlight Reference FFB Stick**: an open-hardware force-feedback joystick design that serves as the first-party reference device for the Flight Hub ecosystem.

## Design Goals

- `#![no_std]` — usable in embedded firmware (STM32G0B1 / RP2350) and on the host
- Single source of truth for the wire format shared between firmware and the Flight Hub host driver
- Plain `[u8; N]` serialisation — no allocator required

## HID Report Summary

| Report ID | Direction         | Purpose              | Length   |
|-----------|-------------------|----------------------|----------|
| 0x01      | IN (device→host)  | Axis + button state  | 16 bytes |
| 0x10      | OUT (host→device) | FFB force command    | 8 bytes  |
| 0x20      | OUT (host→device) | LED / mode control   | 4 bytes  |
| 0xF0      | IN  (device→host) | Firmware version     | 8 bytes  |

## USB Identifiers (provisional)

| Field   | Value  | Notes                              |
|---------|--------|------------------------------------|
| VID     | 0x1209 | pid.codes open allocation          |
| PID     | 0xF170 | OpenFlight Reference Stick (draft) |

## Usage

```rust
use flight_open_hardware::input_report::{InputReport, INPUT_REPORT_LEN};
use flight_open_hardware::output_report::{FfbOutputReport, FFB_REPORT_LEN};

// Parse incoming axis + button report (device → host)
let raw = [0u8; INPUT_REPORT_LEN];
let report = InputReport::from_bytes(&raw);

// Build an FFB force-output command (host → device)
let cmd = FfbOutputReport { force_x: 1500i16, force_y: -800i16 };
let out: [u8; FFB_REPORT_LEN] = cmd.to_bytes();
```

## Reference Design

Schematics, PCB layout, BOM, and firmware source live in `docs/reference/open-hardware/`. The primary MCU target is **STM32G0B1** with a dedicated USB-FS peripheral; the fallback target is **RP2350**.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
