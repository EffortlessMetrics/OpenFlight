# flight-replay

Offline replay harness for recorded OpenFlight runs.

## Responsibilities

- Replays recorded sessions through offline axis and FFB engines.
- Compares outputs with configurable numeric tolerances.
- Provides synthetic and acceptance-oriented validation harnesses.

## Key Modules

- `src/acceptance.rs`
- `src/comparison.rs`
- `src/harness.rs`
- `src/metrics.rs`
- `src/offline_engine.rs`
- `src/replay_config.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
