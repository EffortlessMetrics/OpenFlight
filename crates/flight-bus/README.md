# flight-bus

Normalized telemetry bus and publisher for OpenFlight.

## Responsibilities

- Defines normalized telemetry snapshot types for simulator adapters.
- Provides publishing utilities for routing snapshots across components.
- Includes fixtures used by adapter and integration tests.

## Key Modules

- `src/adapter_fixtures.rs`
- `src/adapters.rs`
- `src/fixtures.rs`
- `src/publisher.rs`
- `src/snapshot.rs`
- `src/types.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
