# flight-simconnect

MSFS SimConnect adapter for telemetry and aircraft detection.

## Responsibilities

- Implements a high-level adapter over SimConnect transport primitives.
- Maps simulator events/data into normalized OpenFlight telemetry.
- Manages aircraft and session tracking for profile switching.

## Key Modules

- `src/adapter.rs`
- `src/aircraft.rs`
- `src/events.rs`
- `src/fixtures.rs`
- `src/mapping.rs`
- `src/sanity_gate.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
