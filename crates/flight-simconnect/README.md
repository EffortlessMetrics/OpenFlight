# flight-simconnect

MSFS SimConnect adapter for telemetry and aircraft detection.

## Responsibilities

- Implements a high-level adapter over SimConnect transport primitives.
- Maps simulator events/data into normalized OpenFlight telemetry.
- Manages aircraft and session tracking for profile switching.

## MSFS 2024 Notes

- Uses standard SimConnect data/event APIs with MSFS 2020/2024 compatibility.
- Handles live aircraft detection from SimConnect one-shot identification payloads.
- Maps and converts telemetry categories:
  - Kinematics
  - Aircraft config and lights
  - Per-engine data
  - Environment
  - Navigation
  - Optional helicopter data

## Key Modules

- `src/adapter.rs`
- `src/aircraft.rs`
- `src/events.rs`
- `src/fixtures.rs`
- `src/mapping.rs`
- `src/sanity_gate.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
