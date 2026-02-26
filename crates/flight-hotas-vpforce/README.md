# flight-hotas-vpforce

VPforce Rhino FFB joystick HID input support for [OpenFlight](https://github.com/flight-hub/openflight).

This crate handles **HID input parsing only** (axes, buttons, hat switch).
Force-feedback output is implemented in [`flight-ffb-vpforce`](../flight-ffb-vpforce).

## Supported devices

| Device | VID | PID | Report bytes | Support tier |
|---|---|---|---|---|
| VPforce Rhino (v2) | 0x0483 | 0xA1C0 | 20 | 2 |
| VPforce Rhino (v3 / Mk II) | 0x0483 | 0xA1C1 | 20 | 2 |

## VID/PID notes

VID `0x0483` belongs to STMicroelectronics (the MCU manufacturer). VPforce does
not hold a dedicated USB VID; this is documented in
`compat/devices/vpforce/rhino.yaml` (quirk `STM_VID`).

PIDs `0xA1C0` (v2) and `0xA1C1` (v3) are confirmed from the compat metadata
and validated by the property tests in `flight-ffb-vpforce`.

## Report format

```text
byte  0         : report_id (0x01)
bytes  1– 2     : X  (roll),           i16 LE  →  [−1.0, 1.0]
bytes  3– 4     : Y  (pitch),          i16 LE  →  [−1.0, 1.0]
bytes  5– 6     : Z  (throttle),       i16 LE  →  [ 0.0, 1.0] (remapped)
bytes  7– 8     : Rx (rocker),         i16 LE  →  [−1.0, 1.0]
bytes  9–10     : Ry (aux, unused),    i16 LE  →  [−1.0, 1.0]
bytes 11–12     : Rz (twist),          i16 LE  →  [−1.0, 1.0]
bytes 13–16     : button bitmask, u32 LE
byte  17        : POV hat (0=N … 7=NW, 0xFF=centred)
bytes 18–19     : reserved
```
