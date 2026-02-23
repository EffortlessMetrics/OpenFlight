# flight-hotas-vkb

VKB family input support for OpenFlight.

## Responsibilities

- Detects VKB STECS and VKB Gladiator NXT EVO variants.
- Exposes baseline per-device `control_map` metadata hints for STECS/Gladiator families.
- Computes physical-device and per-interface metadata for VKB multi-interface layouts.
- Parses per-interface reports into axis/button states.
- Merges `VC0..VC2` reports into one 96-button logical state.
- Provides basic device health monitoring primitives.

## Key Modules

- `src/input.rs`
- `src/health.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
