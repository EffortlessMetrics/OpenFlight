# flight-hotas-turtlebeach

Turtle Beach VelocityOne HOTAS, yoke, and rudder driver for OpenFlight.

## Supported Devices

### Primary VID (0x10F5 — Turtle Beach Corporation)

| Model                    | PID    | Tier | Category       |
|--------------------------|--------|------|----------------|
| VelocityOne Flight       | 0x1050 | 2    | Yoke + Panel   |
| VelocityOne Rudder       | 0x1051 | 2    | Pedals         |
| VelocityOne Flightstick  | 0x1052 | 2    | Joystick       |
| VelocityOne Flight Pro   | 0x0210 | 3    | Premium Yoke   |
| VelocityOne Flight Univ. | 0x1073 | 3    | All-in-One     |
| VelocityOne Flight Yoke  | 0x3085 | 2    | Dedicated Yoke |

### Legacy VID (0x1432)

| Model                         | PID    | Tier | Notes             |
|-------------------------------|--------|------|-------------------|
| VelocityOne Flightdeck (yoke) | 0xB300 | 1    | Confirmed usb.ids |
| VelocityOne Stick             | 0xB301 | 3    | PID estimated     |
| VelocityOne Rudder            | 0xB302 | 3    | PID estimated     |

Tier 3 devices have estimated PIDs and require USB capture for verification.
See `docs/reference/hotas-claims.md` for protocol verification status.

## Key Modules

- `src/devices.rs` — Device database, capability descriptors, VID/PID matching
- `src/protocol.rs` — HID report parsing for Flight/Flightstick, LED & display control
- `src/profiles.rs` — Default axis/button configuration profiles per device
- `src/presets.rs` — Tuned deadzone, filter, and slew-rate presets per device
- `src/velocityone.rs` — Legacy HID report parsing for VID 0x1432 devices

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
