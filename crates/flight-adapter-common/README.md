# flight-adapter-common

Shared adapter primitives for simulator integrations in OpenFlight.

## Responsibilities

- Defines shared adapter configuration, state, and error types.
- Provides reconnection and lifecycle helpers reused by simulator adapters.
- Keeps adapter metrics and behavior consistent across backends.

## Key Modules

- `src/config.rs`
- `src/error.rs`
- `src/metrics.rs`
- `src/reconnection.rs`
- `src/state.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
