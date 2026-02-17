# flight-panels-core

Core panel evaluators and LED state primitives.

## Responsibilities

- Defines panel rule-evaluation primitives and LED state handling.
- Contains reusable panel logic shared by hardware-specific drivers.
- Supplies optional test-helper APIs for panel integration tests.

## Key Modules

- `src/evaluator.rs`
- `src/led.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
