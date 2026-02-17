# flight-service

Main OpenFlight service library and daemon entry point.

## Responsibilities

- Hosts flightd orchestration and runtime lifecycle logic.
- Coordinates auto-switch, capability, and health subsystems.
- Provides stable error taxonomy and safe-mode management.

## Key Modules

- `src/aircraft_auto_switch_service.rs`
- `src/capability_service.rs`
- `src/curve_conflict_service.rs`
- `src/error_taxonomy.rs`
- `src/health.rs`
- `src/one_click_resolver.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
