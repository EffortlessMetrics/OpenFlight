# flight-hotas-microsoft

Microsoft SideWinder Force Feedback and Precision joystick drivers for OpenFlight.

## Supported Devices

| Device | VID | PID | FFB | Support |
|---|---|---|---|---|
| Microsoft SideWinder Force Feedback Pro | 0x045E | 0x001B | ✓ | Tier 2 (parser tests) |
| Microsoft SideWinder Force Feedback 2 | 0x045E | 0x001C | ✓ | Tier 2 (parser tests) |
| Microsoft SideWinder Precision 2 | 0x045E | 0x002B | ✗ | Tier 2 (parser tests) |

## Features

- Full axis parsing: X/Y (10-bit), twist/Rz (8-bit), throttle (8-bit)
- 9-button bitmask with per-button query API
- 8-way hat switch with center detection
- Property-based tests via proptest
- Shared report parser for FFB Pro and FFB 2 (identical HID layout)

## Usage

```rust
use flight_hotas_microsoft::parse_sidewinder_ffb_pro;

let data: &[u8] = /* 7-byte HID report (report ID stripped) */;
let state = parse_sidewinder_ffb_pro(data)?;
println!("X: {:.2}, Y: {:.2}", state.axes.x, state.axes.y);
```

## Protocol Notes

The SideWinder FFB Pro (0x001B) and FFB 2 (0x001C) share an identical
7-byte input report layout. X and Y are 10-bit axes (center ~512), Rz is
8-bit (center ~128), and Throttle is 8-bit (0 = slider forward/top,
255 = slider aft/bottom). The hat switch uses 0 = North, 7 = NorthWest,
8+ = center/released.

The OS typically prepends a 1-byte report ID; strip it before calling these
parsers, so `data[0]` is the first axis byte.

## License

MIT OR Apache-2.0
