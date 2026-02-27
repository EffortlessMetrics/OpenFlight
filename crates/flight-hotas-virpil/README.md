# flight-hotas-virpil

VIRPIL Controls VPC device support for [OpenFlight](https://github.com/flight-hub/openflight).

## Supported devices

| Device | VID | PID | Report bytes | Support tier |
|---|---|---|---|---|
| VPC Throttle CM3 | 0x3344 | 0x0194 | 23 | 2 |
| VPC MongoosT-50CM3 Stick | 0x3344 | 0x4130 | 15 | 2 |
| VPC Control Panel 1 | 0x3344 | 0x025B | 7 | 2 |

## VID/PID source

- VID 0x3344 confirmed: [the-sz.com USB ID database](https://www.the-sz.com/products/usbid/index.php?v=0x3344) (VIRPIL, UAB)
- PIDs from Buzzec/virpil open-source Rust LED control library, cross-referenced against community usage.

All parsers are tier-2 (community-documented, automated parser tests, no HIL).
