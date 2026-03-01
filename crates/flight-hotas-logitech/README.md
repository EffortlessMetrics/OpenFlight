# flight-hotas-logitech

Logitech joystick driver for OpenFlight.

## Supported Devices

| Device | VID | PID | Support |
|---|---|---|---|
| Logitech Extreme 3D Pro | 0x046D | 0xC215 | Tier 2 (parser tests) |
| X56 Rhino Stick (Mad Catz) | 0x0738 | 0x2221 | Tier 2 (parser tests) |
| X56 Rhino Throttle (Mad Catz) | 0x0738 | 0xA221 | Tier 2 (parser tests) |
| X56 RGB Stick (Logitech) | 0x06A3 | 0x0C59 | Tier 2 (parser tests) |
| X56 RGB Throttle (Logitech) | 0x06A3 | 0x0C5B | Tier 2 (parser tests) |
| Saitek Pro Flight Rudder Pedals | 0x06A3 | 0x0763 | Tier 2 (parser tests) |
| Logitech Flight Rudder Pedals | 0x046D | 0xC264 | Tier 2 (parser tests) |
| G Flight Yoke System | 0x046D | 0xC259 | Tier 2 (parser tests) |
| G Flight Throttle Quadrant | 0x046D | 0xC25A | Tier 2 (parser tests) |
| Flight System G940 | 0x046D | 0xC287 | Tier 2 (parser tests) |

## Features

- Full axis parsing: X/Y (10-bit), twist/Rz (8-bit), throttle (7-bit)
- X56 HOTAS: 12-bit stick axes, 10-bit dual throttle, RGB LED control
- Rudder pedals: bipolar rudder + independent toe brakes
- 12 button bitmask with per-button query API
- 8-way hat switch with center detection
- X56 RGB LED zone-set commands (see `protocol` module)
- Tightly-packed HID report parsing
- Property-based tests via proptest

## Usage

```rust
use flight_hotas_logitech::{parse_x56_stick, parse_x56_throttle, parse_rudder_pedals};

// X56 stick
let stick_data: &[u8] = /* read from HID device */;
let stick = parse_x56_stick(stick_data)?;
println!("X: {:.2}, Y: {:.2}, Twist: {:.2}", stick.axes.x, stick.axes.y, stick.axes.rz);

// X56 throttle
let throttle_data: &[u8] = /* read from HID device */;
let throttle = parse_x56_throttle(throttle_data)?;
println!("L: {:.2}, R: {:.2}", throttle.axes.throttle_left, throttle.axes.throttle_right);

// Rudder pedals
let rudder_data: &[u8] = /* read from HID device */;
let rudder = parse_rudder_pedals(rudder_data)?;
println!("Rudder: {:.2}", rudder.axes.rudder);
```

## Protocol Notes

All fields are packed in LSB-first HID input reports. The throttle
slider reads 0 at the top/forward position and 127 at bottom/back — invert
in your profile if conventional throttle direction is needed.

## License

MIT OR Apache-2.0
