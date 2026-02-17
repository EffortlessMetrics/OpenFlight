# flight-tactile

Tactile feedback bridge for SimShaker-class applications.

## Responsibilities

- Converts normalized telemetry into tactile effect events.
- Routes effect channels and gain controls to output targets.
- Provides a rate-limited UDP bridge for tactile software stacks.

## Key Modules

- `src/bridge.rs`
- `src/channel.rs`
- `src/effects.rs`
- `src/simshaker.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
