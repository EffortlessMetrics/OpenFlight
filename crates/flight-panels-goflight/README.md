# flight-panels-goflight

GoFlight USB HID panel module driver for OpenFlight.

## Overview

GoFlight modules communicate over USB HID using a common 8-byte report format.
This crate provides parsing for encoder deltas, button bitmasks, and LED output
commands for all GoFlight panel modules.

## Supported Modules

| Module | Description           |
|--------|-----------------------|
| GF-46  | COM/NAV radio panel   |
| GF-45  | Autopilot panel       |
| GF-LGT | Landing gear / lights |
| GF-WCP | Weather / climate     |

## Key Modules

- `src/modules.rs` — HID report parsing and LED command building

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
