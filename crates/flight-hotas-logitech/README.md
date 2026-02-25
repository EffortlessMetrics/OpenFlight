# flight-hotas-logitech

Logitech joystick driver for OpenFlight.

## Supported Devices

| Device | VID | PID | Support |
|---|---|---|---|
| Logitech Extreme 3D Pro | 0x046D | 0xC215 | Tier 2 (parser tests) |

## Features

- Full axis parsing: X/Y (10-bit), twist/Rz (8-bit), throttle (7-bit)
- 12 button bitmask with per-button query API
- 8-way hat switch with center detection
- Tightly-packed 7-byte HID report parsing
- Property-based tests via proptest

## Usage

```rust
use flight_hotas_logitech::parse_extreme_3d_pro;

let data: &[u8] = /* read from HID device */;
let state = parse_extreme_3d_pro(data)?;
println!("X: {:.2}, Y: {:.2}", state.axes.x, state.axes.y);
```

## Protocol Notes

All fields are packed in a 7-byte HID input report, LSB-first. The throttle
slider reads 0 at the top/forward position and 127 at bottom/back — invert
in your profile if conventional throttle direction is needed.

## License

MIT OR Apache-2.0
