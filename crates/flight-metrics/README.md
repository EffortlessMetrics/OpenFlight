# flight-metrics

Metrics registry, collector, and common metric types.

## Responsibilities

- Defines shared metrics structures and value types.
- Provides registry and collector helpers for crate-level instrumentation.
- Normalizes metric naming and transport across subsystems.

## Key Modules

- `src/collector.rs`
- `src/common.rs`
- `src/registry.rs`
- `src/types.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
