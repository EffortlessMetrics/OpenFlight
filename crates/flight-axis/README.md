# flight-axis

Real-time 250Hz axis processing engine for OpenFlight.

## Responsibilities

- Compiles profile data into deterministic axis pipelines.
- Runs RT-safe processing nodes such as deadzone, curve, slew, and mixer stages.
- Supports atomic compiled-state swaps at tick boundaries.

## Key Modules

- `src/blackbox.rs`
- `src/compiler.rs`
- `src/conflict.rs`
- `src/counters.rs`
- `src/engine.rs`
- `src/frame.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
