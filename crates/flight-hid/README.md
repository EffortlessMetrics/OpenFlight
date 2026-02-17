# flight-hid

HID device management and OFP-1 protocol support for OpenFlight.

## Responsibilities

- Handles HID device I/O and writer lifecycle concerns.
- Implements OFP-1 protocol integration for flight hardware.
- Connects device operations with watchdog and metrics pipelines.

## Key Modules

- `src/hid_writer.rs`
- `src/ofp1.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
