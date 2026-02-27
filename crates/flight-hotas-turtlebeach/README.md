# flight-hotas-turtlebeach

Turtle Beach VelocityOne HOTAS, yoke, and rudder driver for OpenFlight.

## Supported Devices

| Model                         | VID    | PID    | Tier | Notes                 |
|-------------------------------|--------|--------|------|-----------------------|
| VelocityOne Flightdeck (yoke) | 0x1432 | 0xB300 | 1    | Confirmed via usb.ids |
| VelocityOne Stick             | 0x1432 | 0xB301 | 3    | PID estimated         |
| VelocityOne Rudder            | 0x1432 | 0xB302 | 3    | PID estimated         |

Tier 3 devices have estimated PIDs and require USB capture for verification.
See `docs/reference/hotas-claims.md` for protocol verification status.

## Key Modules

- `src/velocityone.rs` — HID report parsing for Flightdeck yoke and Rudder pedals

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
