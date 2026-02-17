# flight-hid-support

HID descriptor helpers and device support metadata.

## Responsibilities

- Parses and normalizes HID descriptor details used by device drivers.
- Stores support metadata and quirks for known hardware families.
- Provides helper utilities such as ghost-input filtering logic.

## Key Modules

- `src/device_support.rs`
- `src/ghost_filter.rs`
- `src/hid_descriptor.rs`
- `src/saitek_hotas.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
