# flight-writers

Versioned simulator configuration writers with verify and repair.

## Responsibilities

- Applies table-driven simulator configuration changes.
- Supports verify/repair workflows with backup and rollback controls.
- Includes golden-file and conflict-detection helpers for validation.
- Provides sim variable lookup tables for MSFS, X-Plane, and DCS.
- Write batching with priority-based conflict resolution across profiles.

## Key Modules

- `src/variable_table.rs` — Sim variable lookup with version diff support
- `src/writer_engine.rs` — Write batching with priority conflict resolution
- `src/golden_tests.rs` — Schema validation and golden snapshot testing
- `src/curve_conflict.rs`
- `src/diff.rs`
- `src/golden.rs`
- `src/repair.rs`
- `src/rollback.rs`
- `src/types.rs`

## Data Files

- `writers/msfs_2020.json` — MSFS 2020 SimVar table
- `writers/msfs_2024.json` — MSFS 2024 delta from 2020
- `writers/xplane_12.json` — X-Plane 12 dataref table
- `writers/dcs_world.json` — DCS export variable table

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
