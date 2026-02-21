# flight-device-common

Shared primitives for device-layer crates in OpenFlight.

## Responsibilities

- Defines stable device identifiers used across HID/panel/virtual paths.
- Defines a shared health model for device managers.
- Provides a common device manager trait.
- Provides lightweight device operation metrics helpers that can publish to
  `flight-metrics`.

## Key Modules

- `src/device.rs`
- `src/health.rs`
- `src/manager.rs`
- `src/metrics.rs`
