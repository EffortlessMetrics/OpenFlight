# flight-virtual

Virtual HID devices and performance gates for CI testing.

## Responsibilities

- Provides virtual HID devices for hardware-free integration tests.
- Includes loopback and OFP-1 emulation utilities for CI.
- Runs performance-gate checks for latency and jitter regressions.

## Key Modules

- `src/device.rs`
- `src/loopback.rs`
- `src/ofp1_emulator.rs`
- `src/perf_gate.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
