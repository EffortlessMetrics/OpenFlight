# flight-writers

Versioned simulator configuration writers with verify and repair.

## Responsibilities

- Applies table-driven simulator configuration changes.
- Supports verify/repair workflows with backup and rollback controls.
- Includes golden-file and conflict-detection helpers for validation.

## Key Modules

- `src/curve_conflict.rs`
- `src/diff.rs`
- `src/golden.rs`
- `src/repair.rs`
- `src/rollback.rs`
- `src/types.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
