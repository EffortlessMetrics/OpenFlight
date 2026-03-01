# flight-hotas-honeycomb

Honeycomb Aeronautical Alpha Yoke, Bravo Throttle Quadrant, and Charlie Rudder Pedals driver for OpenFlight.

## Devices

| Device | VID | PID | Status |
|---|---|---|---|
| Alpha Flight Controls XPC (Yoke) | 0x294B | 0x0102 | Tier 2 — simulated tests only |
| Bravo Throttle Quadrant | 0x294B | 0x1901 | Tier 2 — simulated tests only |
| Charlie Rudder Pedals | 0x294B | 0x1902 | Tier 3 — PID community-inferred |

**Note:** The Alpha Yoke PID (0x0102) is a community-reported value that has not been
hardware-validated. The Bravo PID (0x1901) is confirmed from multiple independent sources.
The Charlie PID (0x1902) is inferred from the sequential Honeycomb numbering scheme.

## Features

### Alpha Yoke Input

- 2 axes (roll/pitch), 12-bit resolution
- 36 buttons (including magneto switch positions)
- 1 hat switch (8-way)
- 2 rocker switches (left horn, decoded as ±1 directional values)
- Magneto switch decoding (Off/R/L/Both/Start)

### Bravo Throttle Input

- 7 axes (5× throttle, flap lever, spoiler), 12-bit resolution
- 64 buttons (AP panel, landing gear, toggle switches, reverse levers)
- Rotary encoder tracking (CW/CCW edge detection)
- Flap switch position tracking (4 detents: UP/1/2/FULL)
- Trim wheel delta tracking (±1 per click)
- 7 toggle switches (3-state: Up/Center/Down)
- Landing gear indicator state (Up/Down/Transit)
- Wrapping encoder presets (heading, altitude, VS, course)

### Bravo LED Output

LED control uses a 5-byte HID feature report (report ID 0x00):

```
[0x00, ap_byte, gear_byte, annunciator1_byte, annunciator2_byte]
```

| Byte | Bit 0 | Bit 1 | Bit 2 | Bit 3 | Bit 4 | Bit 5 | Bit 6 | Bit 7 |
|------|-------|-------|-------|-------|-------|-------|-------|-------|
| 1 | HDG | NAV | APR | REV | ALT | VS | IAS | AUTOPILOT |
| 2 | GearLGreen | GearLRed | GearCGreen | GearCRed | GearRGreen | GearRRed | MasterWarning | EngineFire |
| 3 | LowOilPressure | LowFuelPressure | AntiIce | StarterEngaged | APU | MasterCaution | Vacuum | LowHydPressure |
| 4 | AuxFuelPump | ParkingBrake | LowVolts | Door | — | — | — | — |

Supports both serialization (`serialize_led_report`) and deserialization
(`deserialize_led_report`) for diagnostics and round-trip testing.

### Charlie Rudder Pedals Input

- 3 axes (rudder bipolar, left/right toe brakes unipolar), 12-bit resolution
- No buttons

## Protocol Notes

**Input report layout (estimated):** The HID input report format is inferred from the
HID joystick specification and community documentation. It requires hardware validation
before use in production. See `compat/devices/honeycomb/` for compat manifests.

**LED protocol (confirmed):** Verified from BetterBravoLights (RoystonS, GitHub),
which is a production-tested tool that controls Bravo LEDs using the same protocol.
