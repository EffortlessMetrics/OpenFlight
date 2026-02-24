# flight-ac7-telemetry

`flight-ac7-telemetry` receives AC7 bridge packets over UDP and publishes normalized
`BusSnapshot` frames into Flight Hub.

## Responsibilities

- Bind a local UDP endpoint for AC7 bridge telemetry.
- Parse/validate payloads via `flight-ac7-protocol`.
- Convert packets to `flight-bus::BusSnapshot` with `SimId::AceCombat7`.
- Track adapter state and adapter metrics.

## Notes

- This crate does not inject into or patch AC7.
- It expects a user-provided bridge plugin/process to emit telemetry.
