# flight-dcs-export

DCS Export.lua adapter and installer integration for OpenFlight.

## Responsibilities

- Manages DCS Export.lua installation and generation workflows.
- Bridges DCS telemetry over a local socket transport.
- Enforces multiplayer-safe behavior boundaries during data export.

## Key Modules

- `src/adapter.rs`
- `src/export_lua.rs`
- `src/installer.rs`
- `src/mp_detection.rs`
- `src/socket_bridge.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
