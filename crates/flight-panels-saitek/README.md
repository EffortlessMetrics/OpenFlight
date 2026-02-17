# flight-panels-saitek

Saitek and Logitech panel driver implementation.

## Responsibilities

- Implements Saitek/Logitech panel HID mapping and state conversion.
- Integrates panel behavior with shared panel-core abstractions.
- Includes verify-matrix logic used to validate panel behavior.

## Key Modules

- `src/saitek.rs`
- `src/verify_matrix.rs`
- `src/saitek/`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
