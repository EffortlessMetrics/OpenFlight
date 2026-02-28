# flight-hotas-virpil

VIRPIL Controls VPC device support for [OpenFlight](https://github.com/flight-hub/openflight).

## Supported devices

| Device | VID | PID | Report bytes | Support tier |
|---|---|---|---|---|
| VPC Throttle CM3 | 0x3344 | 0x0194 | 23 | 2 |
| VPC MongoosT-50CM3 Stick | 0x3344 | 0x4130 | 15 | 2 |
| VPC Constellation Alpha | 0x3344 | 0x838F | 15 | 2 |
| VPC Constellation Alpha Prime (L) | 0x3344 | 0x0138 | 15 | 2 |
| VPC Constellation Alpha Prime (R) | 0x3344 | 0x4139 | 15 | 2 |
| VPC WarBRD | 0x3344 | 0x40CC | 15 | 2 |
| VPC WarBRD-D | 0x3344 | 0x43F5 | 15 | 2 |
| VPC Control Panel 1 | 0x3344 | 0x025B | 7 | 2 |
| VPC Control Panel 2 | 0x3344 | 0x0259 | 11 | 2 |
| VPC ACE Torq | 0x3344 | 0x0198 | 5 | 2 |
| VPC ACE Collection Pedals | 0x3344 | 0x019C | 9 | 2 |
| VPC Rotor TCS Plus | 0x3344 | 0x01A0 | 11 | 2 |

## Modules

- **`protocol`** — consolidated VPC HID protocol: VID/PID table, LED feature reports, axis normalization
- **`profiles`** — default device configuration descriptors (axes, buttons, hats, roles)
- **`stick_alpha`** / **`stick_alpha_prime`** — Constellation Alpha grip parsers
- **`stick_mongoost`** — MongoosT-50CM3 stick parser
- **`stick_warbrd`** — WarBRD / WarBRD-D base parser
- **`throttle_cm3`** — CM3 dual-throttle parser
- **`throttle_ace_torq`** — ACE Torq single-axis throttle parser
- **`pedals_ace`** — ACE Collection pedals (rudder + toe brakes)
- **`collective_tcs`** — Rotor TCS Plus helicopter collective parser
- **`panel_1`** / **`panel_2`** — Control Panel parsers

## VID/PID source

- VID 0x3344 confirmed: [the-sz.com USB ID database](https://www.the-sz.com/products/usbid/index.php?v=0x3344) (VIRPIL, UAB)
- PIDs from Buzzec/virpil open-source Rust LED control library, cross-referenced against community usage.

All parsers are tier-2 (community-documented, automated parser tests, no HIL).
