# flight-ffb-brunner

Brunner Elektronik AG CLS-E Force Feedback driver for OpenFlight.

Provides protocol types, FFB effect engine, safety envelope enforcement,
and per-aircraft FFB profiles for the Brunner CLS-E joystick base and CLS-P
FFB pedals.

## Brunner CLS-E overview

The CLS-E is a high-end brushless DC motor force-feedback joystick base from
Brunner Elektronik AG (Switzerland). It communicates over USB HID for input
and uses the **CLS2Sim** middleware for force-feedback output via a TCP/UDP
remote control interface.

| Device | VID | PID | Description |
|--------|------|------|-------------|
| CLS-E Joystick | 0x25BB | 0x0063 | FFB yoke / joystick base |
| CLS-P Pedals | 0x25BB | 0x006B | FFB rudder pedals |

## Modules

- **protocol** — Brunner-specific USB vendor control transfers and CLS2Sim command framing
- **effects** — Spring, damper, friction, constant force, and periodic effect computations
- **safety** — Force magnitude limiting, rate-of-change limiting, watchdog, and emergency stop
- **profiles** — Per-aircraft FFB presets (GA, transport, fighter, helicopter)

## License

MIT OR Apache-2.0
