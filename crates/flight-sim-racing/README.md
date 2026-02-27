# flight-sim-racing

Sim racing game adapter for OpenFlight — racing telemetry ingestion and force-feedback translation.

## Responsibilities

- Implements a generic binary UDP telemetry protocol (port 20777, compatible with SimHub / OpenSimHardware).
- Parses incoming packets into a normalised [`RacingTelemetry`] structure.
- Translates racing telemetry into FFB effect parameters via [`RacingFfbTranslator`].

## Supported Protocols

| Protocol | Transport | Port |
|---|---|---|
| Generic racing UDP (SimHub-compatible) | UDP binary LE | 20777 |

## Packet Format (Generic UDP)

| Offset | Size | Field |
|---|---|---|
| 0–3 | 4 | Magic `0x52414345` (`"RACE"`) |
| 4 | 1 | Version (`0x01`) |
| 5 | 1 | Gear (`i8`, −1=R, 0=N, 1–8) |
| 6–9 | 4 | `speed_ms` f32 LE |
| 10–13 | 4 | `lateral_g` f32 LE |
| 14–17 | 4 | `longitudinal_g` f32 LE |
| 18–21 | 4 | `vertical_g` f32 LE |
| 22–25 | 4 | `throttle` f32 LE |
| 26–29 | 4 | `brake` f32 LE |
| 30–33 | 4 | `steering_angle` f32 LE |
| 34–37 | 4 | `rpm` f32 LE |
| 38–41 | 4 | `rpm_max` f32 LE |
