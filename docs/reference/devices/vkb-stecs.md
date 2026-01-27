---
doc_id: DOC-REF-VKB-STECS
kind: reference
area: flight-hid
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
---

# VKB STECS - Device Notes

This reference captures stable identification, naming conventions, and integration
guidance for VKB STECS-class throttles.

## Scope

- Devices: VKB STECS Space Throttle (Mini / Mini+ grips)
- Interface: USB HID joystick/throttle (not force feedback)
- Focus: baseline control naming, encoder semantics, and profile safety

## USB identity (fingerprints)

- Vendor ID: 0x231D (VKB-Sim)
- Known PIDs observed in community button maps:
  - 0x013A: STECS Right Space Throttle Grip Mini
  - 0x0137: STECS Left Space Throttle Grip Mini+

Notes:
- Treat the PID list as a fingerprint set, not a single constant.
- VKB devices are programmable; a user profile can expose additional virtual
  buttons or change logical ordering.

## Control naming baseline (community-sourced)

OpenFlight ships a baseline control map derived from Elite Dangerous button map
files. The map is surfaced in device metadata as `control_map` and should be
treated as a *best-effort* naming hint, not a hard guarantee.

### Right Space Throttle Grip Mini (PID 0x013A)

- Axes: RX, RY, X, Y, Z (5 total)
- Buttons: 1-29
- Encoders: none in the baseline map

Axis names:

| Usage | Name |
| --- | --- |
| RX | STECS SpaceBrake |
| RY | STECS Laser Power |
| X | STECS [x52prox] |
| Y | STECS [x52proy] |
| Z | STECS [x52z] |

### Left Space Throttle Grip Mini+ (PID 0x0137)

- Axes: RX, RY, X, Y, Z (5 total)
- Buttons: 1-42
- Encoders: `LSTECS Rot` (CW=9, CCW=8, Press=20)

Axis names:

| Usage | Name |
| --- | --- |
| RX | LSTECS SpaceBrake |
| RY | LSTECS Laser Power |
| X | LSTECS [x52prox] |
| Y | LSTECS [x52proy] |
| Z | LSTECS Throttle |

## OpenFlight integration guidance

### Treat naming as a hint, not a contract

VKBDevCfg can remap buttons, encoder modes, or synthesize virtual buttons.
Use the control map as a stable default and prefer HID descriptor parsing when
authoritative mappings are required.

### Encoder semantics

STECs-class devices expose discrete CW/CCW events for encoders. OpenFlight
represents these as paired buttons with an optional press button:

```
Encoder { cw_button: N, ccw_button: M, press_button: K }
```

This prevents “two unrelated buttons” UX for users configuring encoder actions.

### Composite HID interfaces

Expect multiple HID interfaces and top-level collections. Use stable identity
fields (serial, USB path) to tie them together when enumerating devices.

### Data-driven control maps

Store per-PID control naming in data structures rather than hard-coded logic.
OpenFlight uses `device_support` to keep these maps small, static, and cheap
to swap at runtime.

## Data needed for a concrete device entry

To build a definitive STECS mapping, capture:

- USB identifiers (VID/PID variants)
- HID report descriptor (axis usages, report IDs, button bitfields)
- Short sample report stream while exercising every control

## Fast capture plan

- Windows:
  - USBView or HID descriptor viewer for report descriptors
  - USBPcap + Wireshark for report samples
- Linux:
  - `lsusb -v`, `usbhid-dump`, `evtest`, `hid-recorder`

