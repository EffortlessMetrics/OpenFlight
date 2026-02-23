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

- Devices: VKB STECS Space Throttle (Mini / Mini+ / Standard grips)
- Interface: USB HID joystick/throttle (not force feedback)
- Focus: baseline control naming, encoder semantics, and profile safety

## USB identity (fingerprints)

- Vendor ID: 0x231D (VKB-Sim)
- Known PIDs observed in community button maps:
  - 0x0136: STECS Left Space Throttle Grip Mini
  - 0x013A: STECS Right Space Throttle Grip Mini
  - 0x0137: STECS Left Space Throttle Grip Mini+
  - 0x013B: STECS Right Space Throttle Grip Mini+
  - 0x0138: STECS Left Space Throttle Grip Standard
  - 0x013C: STECS Right Space Throttle Grip Standard

Notes:
- Treat the PID list as a fingerprint set, not a single constant.
- VKB devices are programmable; a user profile can expose additional virtual
  buttons or change logical ordering.

## Control naming baseline (community-sourced)

OpenFlight ships a baseline control map derived from Elite Dangerous button map
files. The map is surfaced in device metadata as `control_map` and should be
treated as a *best-effort* naming hint, not a hard guarantee.

### Mini grips (PIDs 0x0136 left, 0x013A right)

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

### Mini+ grips (PIDs 0x0137 left, 0x013B right)

- Axes: RX, RY, X, Y, Z (5 total)
- Buttons: 1-42
- Encoders:
  - `LSTECS Rot` (Left Mini+), `RSTECS Rot` (Right Mini+)
  - CW=9, CCW=8, Press=20

Axis names:

| Usage | Left Mini+ | Right Mini+ |
| --- | --- | --- |
| RX | LSTECS SpaceBrake | RSTECS SpaceBrake |
| RY | LSTECS Laser Power | RSTECS Laser Power |
| X | LSTECS [x52prox] | RSTECS [x52prox] |
| Y | LSTECS [x52proy] | RSTECS [x52proy] |
| Z | LSTECS Throttle | RSTECS Throttle |

### Standard grips (PIDs 0x0138 left, 0x013C right)

- Axes: RX, RY, X, Y, Z (5 total)
- Buttons: 1-53
- Encoders:
  - `STECS - STEM Enc1` (CW=47, CCW=46, Press=50)
  - `STECS - STEM Enc2` (CW=49, CCW=48, Press=51)

Axis names:

| Usage | Name |
| --- | --- |
| RX | STECS - Space Brake |
| RY | STECS - Laser Power |
| X | STECS - [x52prox] |
| Y | STECS - [x52proy] |
| Z | STECS - [x52z] |

Notes:
- Standard left/right grips swap the "Index Left/Right" button names at indices 18/19.

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

OpenFlight now exposes per-interface STECS metadata in IPC device `metadata`:

- `stecs.physical_id` - shared id for all interfaces of one physical unit
- `stecs.virtual_controller_index` - zero-based `VC` index (`0`, `1`, `2`)
- `stecs.virtual_controller` - display form (`VC0`, `VC1`, `VC2`)
- `stecs.interface_count` - number of interfaces seen for that physical unit
- `stecs.virtual_button_range` - expected firmware button window (`1-32`, `33-64`, ...)

This lets clients correlate separate HID interfaces before applying bindings.

### Linux multi-interface quirk guidance

Some Linux setups expose only the first 32-button interface unless a
multi-input quirk is applied for VKB devices.

Example modprobe snippet:

```conf
# /etc/modprobe.d/flight-hub-vkb.conf
options usbhid quirks=0x231d:0x0136:0x0004,0x231d:0x0137:0x0004,0x231d:0x0138:0x0004,0x231d:0x013a:0x0004,0x231d:0x013b:0x0004,0x231d:0x013c:0x0004
```

Then regenerate initramfs/reboot and verify interfaces with `evtest` or
`usbhid-dump`.

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
