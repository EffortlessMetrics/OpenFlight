# flight-hotas-thrustmaster

Thrustmaster T.Flight HOTAS driver support for OpenFlight.

## Responsibilities

- Decodes Thrustmaster HOTAS input reports and axis data.
- Handles merged and separate axis-mode behavior.
- Provides device health and preset mapping helpers.

## Key Modules

- `src/health.rs`
- `src/input.rs`
- `src/presets.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
