# flight-core

Core facade crate that re-exports OpenFlight domain modules.

## Responsibilities

- Re-exports shared domain crates behind a single import surface.
- Defines top-level error/result types consumed across the workspace.
- Acts as the main integration seam for service-facing code.

## Key Modules

- `src/error.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
