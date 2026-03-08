# flight-testkit

Comprehensive mock/fake infrastructure for OpenFlight integration testing.

## Modules

- **deterministic_clock** — Thread-safe deterministic clock for timing-sensitive tests
- **trace_recorder** — Records bus events with timestamps; sequence/negative assertions; JSON export
- **fake_device** — Builder-pattern fake HID devices with signal patterns and fault injection
- **fake_sim** — Fake simulator backend with canned telemetry and disconnect simulation
- **assertions** — Domain-specific assertion helpers (axis range, monotonic, NaN, latency, jitter p99)
- **golden** — File-based golden/snapshot testing with `UPDATE_GOLDEN=1` support
