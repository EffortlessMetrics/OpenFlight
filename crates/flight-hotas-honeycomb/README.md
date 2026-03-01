# flight-hotas-honeycomb

Honeycomb Aeronautical Alpha Yoke and Bravo Throttle Quadrant driver for OpenFlight.

## Devices

| Device | VID | PID | Legacy VID | Legacy PID | Status |
|---|---|---|---|---|---|
| Alpha Flight Controls XPC (Yoke) | 0x294B | 0x0102 | 0x04D8 | 0xE6D6 | Tier 2 — simulated tests only |
| Bravo Throttle Quadrant | 0x294B | 0x1901 | 0x04D8 | 0xE6D5 | Tier 2 — simulated tests only |

**Note:** Early production Honeycomb units shipped with the Microchip default VID (0x04D8).
Current production uses the Honeycomb VID (0x294B). Both are recognised automatically.

## Features

### Alpha Yoke Input

- 2 axes (roll/pitch), 12-bit resolution
- 36 buttons (including magneto switch positions)
- 1 hat switch (8-way)

### Bravo Throttle Input

- 7 axes (5× throttle, flap lever, spoiler), 12-bit resolution
- 64 buttons (AP panel, landing gear, toggle switches, reverse levers)
- 21 LEDs via HID feature report

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

## Protocol Notes

**Input report layout (estimated):** The HID input report format is inferred from the
HID joystick specification and community documentation. It requires hardware validation
before use in production. See `compat/devices/honeycomb/` for compat manifests.

**LED protocol (confirmed):** Verified from BetterBravoLights (RoystonS, GitHub),
which is a production-tested tool that controls Bravo LEDs using the same protocol.

## Named Button Enums

`AlphaButton` and `BravoButton` enums provide named variants for known button
assignments, avoiding magic numbers:

```rust
use flight_hotas_honeycomb::{parse_alpha_report, AlphaButton};

let state = parse_alpha_report(&report).unwrap();
if AlphaButton::Ptt.is_active(&state.buttons) {
    // push-to-talk pressed
}
```

## Button State Change Detection

`ButtonDelta` computes which buttons were newly pressed or released between
consecutive reports:

```rust
use flight_hotas_honeycomb::ButtonDelta;

let delta = ButtonDelta::compute(prev_mask, current_mask);
for btn in delta.pressed_buttons() {
    println!("Button {btn} just pressed");
}
```
