# flight-hotas-simucube

Simucube 2 direct-drive wheel/stick driver for OpenFlight.

Supports Sport, Pro, and Ultimate variants via USB HID: encoder position
parsing (22-bit), torque command building (±32767 i16), and model detection by PID.

## USB IDs

| Model    | VID    | PID    | Peak Torque |
|----------|--------|--------|-------------|
| Sport    | 0x16D0 | 0x0D5A | 17 Nm       |
| Pro      | 0x16D0 | 0x0D61 | 25 Nm       |
| Ultimate | 0x16D0 | 0x0D60 | 32 Nm       |

## Encoder

22-bit absolute encoder (0 … 4 194 303, centre = 2 097 151).
`normalize_angle(pos, 22)` maps to −1.0 … +1.0.

## Torque

`TorqueCommand::new(value).to_i16()` converts a normalised [−1.0, 1.0] value
to the ±32 767 i16 expected by the device output report.
