# flight-hotas-sony

Sony PlayStation DualShock/DualSense controller adapter for OpenFlight.

## Supported Devices

| Device | VID | PID | Support |
|---|---|---|---|
| DualShock 3 | 0x054C | 0x0268 | Tier 2 (parser tests) |
| DualShock 4 v1 | 0x054C | 0x05C4 | Tier 2 (parser tests) |
| DualShock 4 v2 | 0x054C | 0x09CC | Tier 2 (parser tests) |
| DualSense (PS5) | 0x054C | 0x0CE6 | Tier 2 (parser tests) |
| DualSense Edge | 0x054C | 0x0DF2 | Tier 2 (parser tests) |

## Features

- DualShock 4 USB HID report parsing (64-byte, report ID 0x01)
- DualSense USB HID report parsing with touchpad axes
- Normalized axis values: sticks −1.0..=1.0, triggers 0.0..=1.0
- Button bitmask and D-pad extraction
- Property-based tests via proptest

## Usage

```rust
use flight_hotas_sony::{parse_ds4_report, parse_dualsense_report};

let data: &[u8] = /* read from HID device */;
let state = parse_ds4_report(data)?;
println!("Left X: {:.2}, Left Y: {:.2}", state.left_x, state.left_y);
```

## Protocol Notes

### DualShock 4 (USB, report ID 0x01)

Byte layout (64 bytes total):
- Byte 0: report_id (0x01)
- Byte 1: left_x (0=left, 127=center, 255=right)
- Byte 2: left_y (0=up, 127=center, 255=down)
- Byte 3: right_x
- Byte 4: right_y
- Byte 5: L2 trigger (0..255)
- Byte 6: R2 trigger (0..255)
- Bytes 7–8: buttons (D-pad in low nibble of byte 7)
- Byte 9: PS/options buttons

### DualSense (USB, report ID 0x01)

Byte layout:
- Bytes 1–4: sticks (same order as DS4)
- Bytes 5–6: L2/R2 triggers
- Bytes 7–9: buttons
- Byte 10: hat switch / D-pad
