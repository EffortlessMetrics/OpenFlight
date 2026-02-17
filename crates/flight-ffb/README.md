# flight-ffb

Force feedback engine with safety interlocks and mode negotiation.

## Responsibilities

- Implements the force-feedback safety state machine and fault handling.
- Negotiates runtime modes such as DirectInput, raw torque, and telemetry synth.
- Provides trim, soft-stop, interlock, and device-health subsystems.

## Key Modules

- `src/audio.rs`
- `src/blackbox.rs`
- `src/device_health.rs`
- `src/dinput_backend.rs`
- `src/dinput_com.rs`
- `src/dinput_device.rs`
- `src/tests/`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
