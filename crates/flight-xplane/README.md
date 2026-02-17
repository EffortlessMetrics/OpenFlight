# flight-xplane

X-Plane adapter with DataRef, UDP, and plugin integration.

## Responsibilities

- Implements X-Plane telemetry ingestion over UDP and plugin surfaces.
- Manages DataRef mapping and aircraft detection state.
- Includes latency and fixture tooling for adapter validation.

## Key Modules

- `src/adapter.rs`
- `src/aircraft.rs`
- `src/dataref.rs`
- `src/fixtures.rs`
- `src/latency.rs`
- `src/plugin.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
