# flight-pedals

Unified rudder pedals input model for OpenFlight with vendor-specific HID report
parsers for all major pedal brands.

## Supported devices

| Vendor | Model | VID | PID | Axes |
|--------|-------|-----|-----|------|
| Thrustmaster | TFRP (T.Flight Rudder Pedals) | 0x044F | 0xB678 | rudder + L/R toe brakes |
| Thrustmaster | TPR (Pendular Rudder) | 0x044F | 0xB68F | rudder + L/R toe brakes |
| MFG | Crosswind V3 | 0x1551 | 0x0003* | rudder + L/R toe brakes |
| Slaw Device | RX Viper | 0x0483 | 0x5746* | rudder + L/R toe brakes |
| VKB | T-Rudder Mk.IV | 0x231D | 0x0126 | rudder + L/R toe brakes |
| Saitek/Logitech | Pro Flight Rudder Pedals | 0x06A3 | 0x0763 | rudder + L/R toe brakes |

\* PID is a community estimate; confirm with `lsusb` on real hardware.

## Architecture

All pedals share the same three-axis pattern: **rudder yaw** + **left toe brake** +
**right toe brake**.  The `PedalsAxes` struct normalises every vendor's raw HID
report to `0.0–1.0` floating-point values.

Vendor-specific parsers handle differences in report layout, byte order,
resolution (10–16 bit), and axis inversion.  Calibration support allows
per-device min/max overrides.

## License

MIT OR Apache-2.0
