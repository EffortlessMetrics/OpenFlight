# flight-updater

Signed update system with channels, deltas, and rollback.

## Responsibilities

- Manages update channels, manifests, and rollout strategy.
- Implements signature validation and delta package handling.
- Supports rollback flows and a docs-validator utility binary.

## Key Modules

- `src/channels.rs`
- `src/delta.rs`
- `src/integration_docs.rs`
- `src/packaging.rs`
- `src/rollback.rs`
- `src/signature.rs`
- `src/bin/`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
