# flight-hotas-vkb

VKB STECS HOTAS driver support for OpenFlight.

## Responsibilities

- Detects VKB STECS variants and virtual-controller interfaces.
- Parses per-interface reports into axis/button states.
- Merges `VC0..VC2` reports into one 96-button logical state.
- Provides basic device health monitoring primitives.

## Key Modules

- `src/input.rs`
- `src/health.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.

