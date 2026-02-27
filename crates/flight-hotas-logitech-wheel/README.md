# flight-hotas-logitech-wheel

Logitech racing wheel parsers for OpenFlight.

## Supported Devices

| Device | VID | PID | Notes |
|---|---|---|---|
| Logitech G29 | 0x046D | 0xC24F | 900° wheel, 3 pedals, FFB |
| Logitech G920 | 0x046D | 0xC262 | Xbox/PC variant of G29 |
| Logitech G923 (PS) | 0x046D | 0xC266 | G923 with TrueForce (PS) |
| Logitech G923 (Xbox) | 0x046D | 0xC267 | G923 with TrueForce (Xbox/PC) |
| Logitech G27 | 0x046D | 0xC29B | H-pattern shifter, paddle shifters |
| Logitech G25 | 0x046D | 0xC299 | Classic GT wheel, paddle shifters only |

## Features

- 12-byte G29/G920/G923 HID report parsing (wheel, 3 pedals, 16 buttons, hat)
- 11-byte G27/G25 HID report parsing (wheel, 3 pedals, buttons)
- Report ID validation
- `normalize_wheel`: raw u16 → −1.0..=1.0 (bipolar)
- `normalize_pedal`: raw u16 → 0.0..=1.0 (unipolar)

## Usage

```rust
use flight_hotas_logitech_wheel::{normalize_pedal, normalize_wheel, parse_g29};

let raw: &[u8] = /* read from HID device, including Report ID byte */;
let state = parse_g29(raw)?;
println!(
    "wheel: {:.2}, gas: {:.2}",
    normalize_wheel(state.wheel),
    normalize_pedal(state.gas),
);
```

## Protocol Notes

**Report ID:** These parsers expect the USB HID Report ID byte at position 0
(`0x01`). Strip or prepend it as needed depending on how your OS exposes the
HID device.

**Caution:** Byte layouts are based on community documentation and have not
been independently verified on hardware against raw USB captures. Validate
with a USB sniffer (`lsusb -v` / Wireshark) before relying on these parsers
in production.

## Relationship to `flight-hotas-logitech`

The `flight-hotas-logitech` crate covers Logitech *flight* peripherals
(joysticks, yokes, throttles). This crate covers Logitech *racing* wheels,
which share the same VID (0x046D) but use different HID report formats and
FFB characteristics.

## License

MIT OR Apache-2.0
