# flight-hotas-saitek

Saitek and Logitech HOTAS driver support for OpenFlight.

## Responsibilities

- Decodes Saitek/Logitech HOTAS input reports and control states.
- Contains optional experimental output paths for MFD, LED, and RGB features.
- Implements policy and health logic specific to supported HOTAS devices.

## Key Modules

- `src/health.rs`
- `src/input.rs`
- `src/policy.rs`
- `src/traits.rs`
- `src/led/`
- `src/mfd/`
- `src/rgb/`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
