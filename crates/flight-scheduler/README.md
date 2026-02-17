# flight-scheduler

Real-time scheduler and timing primitives for the 250Hz loop.

## Responsibilities

- Implements timing discipline for the 250Hz real-time loop.
- Provides bounded ring and jitter metric primitives.
- Contains platform-specific scheduler hooks for Windows and Unix.

## Key Modules

- `src/metrics.rs`
- `src/pll.rs`
- `src/ring.rs`
- `src/unix.rs`
- `src/windows.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
