# flight-tracing

Tracing backends and performance counter infrastructure.

## Responsibilities

- Implements ETW and tracepoint event backends by platform.
- Collects performance counters used for regression gates.
- Provides shared tracing configuration and error handling types.

## Key Modules

- `src/counters.rs`
- `src/etw.rs`
- `src/events.rs`
- `src/regression.rs`
- `src/tracepoints.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
