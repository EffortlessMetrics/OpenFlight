---
doc_id: DOC-REF-TFLIGHT-HOTAS
kind: reference
area: flight-hid
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
---

# Thrustmaster T.Flight HOTAS One / HOTAS 4 - Device Notes

This reference captures stable identification, USB/HID surface details, and known quirks for solid support in OpenFlight.

## Scope

- Devices: T.Flight HOTAS One (Xbox/PC) and T.Flight HOTAS 4 (PS4/PC)
- Interface: USB HID joystick (not force feedback)

## USB identity (fingerprints)

- Vendor ID: 0x044F (Thrustmaster)
- Known PIDs observed in the wild:
  - 0xB68B: T.Flight HOTAS One (Bulk)
  - 0xB67A: T.Flight HOTAS 4 (Bulk)

Notes:
- Treat the PID list as a fingerprint set, not a single constant.
- Thrustmaster hardware can appear with additional PIDs for bulk/retail or regional variants.

### Windows hardware ID examples

Use these for matching when collecting field data:

- HOTAS One: `USB\\VID_044F&PID_B68B`
- HOTAS 4: `USB\\VID_044F&PID_B67A`

## Reported control surface (advertised)

Both devices present as a traditional HOTAS:

- Axes: 5
- Buttons: 14
- Hat switch: 1
- Stick: X/Y
- Twist rudder: RZ
- Throttle: Slider 0
- Rudder rocker: Slider 1

Expected behavior:
- Buttons and hat are standard HID joystick inputs.
- Axes, buttons, and hat indices should always be derived from the HID descriptor.

## Axis mode quirk (critical)

Both devices have an axis mode that changes what the HID descriptor exposes.

- 4/6 axis mode:
  - Throttle (Slider 0), twist (RZ), and rocker (Slider 1) are merged into a single axis (RZ).
  - Expect fewer independent axes to appear.
- 5/8 axis mode:
  - Throttle = Slider 0, twist = RZ, rocker = Slider 1 are distinct.

Implementation guidance:
- Treat axis mode as a runtime quirk, not a static device trait.
- Do not hardcode axis indices (e.g., "axis 2 is twist").
- Detect the mode by HID usages present (missing Slider 0/Slider 1 is a strong signal).
- When merged mode is detected, surface a clear warning:
  - "Rudder + throttle are merged. Switch to 5/8 axis mode for full mapping."

### Mode detection heuristic

This is a best-effort heuristic, not a guarantee:

1. Parse HID usages for the joystick collection.
2. If X, Y, and RZ are present but Slider 0 and Slider 1 are missing, treat as merged mode.
3. If X, Y, RZ, Slider 0, and Slider 1 are present, treat as separate-axis mode.
4. If ambiguous, fall back to user selection and show a warning.

### Merged mode behavior (4/6 axis)

When merged mode is detected, the throttle/twist/rocker intent cannot be separated.
Recommended handling:

- Expose a single "RZ (combined)" axis.
- Prompt the user to switch to 5/8 axis mode to unlock separate throttle and rudder.

## Windows driver reality

If axes or buttons appear missing on PC, Thrustmaster drivers are commonly required.

Recommended troubleshooting note:
- Install the Thrustmaster driver package and confirm the device is in 5/8 axis mode (PC switch set to PC on HOTAS One).

## Suggested default mapping (5/8 axis mode)

| Control | HID usage | Default axis |
| --- | --- | --- |
| Stick X | X | Roll |
| Stick Y | Y | Pitch (invert optional) |
| Throttle | Slider 0 | Throttle |
| Twist | RZ | Yaw (primary) |
| Rudder rocker | Slider 1 | Yaw (alternate) |
| Hat | POV | View/Trim (policy-dependent) |

Allow users to switch yaw source between twist and rocker.

### Suggested default mapping (4/6 axis mode)

In merged mode, only a single RZ axis is available for the combined inputs:

- RZ (combined) -> Yaw (default)
- Show a warning that throttle + rudder are merged and suggest 5/8 axis mode.

## Minimum registry facts

Store the following as stable device support facts:

- Vendor: 0x044F (Thrustmaster)
- Known PIDs:
  - 0xB68B: T.Flight HOTAS One (Bulk)
  - 0xB67A: T.Flight HOTAS 4 (Bulk)
- Quirk: axis_mode
  - 4/6 axis mode merges throttle + twist + rocker into RZ
  - 5/8 axis mode exposes Slider 0, RZ, Slider 1 separately

## Recommended user-facing warnings

Use short, specific copy:

- "Rudder + throttle are merged. Switch to 5/8 axis mode for full mapping."
- "Missing axes? Install Thrustmaster drivers and confirm 5/8 axis mode."

## Integration checklist

When adding or validating support:

- Verify enumeration by VID/PID fingerprint set (do not assume a single PID).
- Parse HID descriptor and map by usage, not index.
- Confirm axis-mode detection by swapping between 4/6 and 5/8 modes.
- Verify merged-mode warning triggers when Slider 0/1 are absent.
- Confirm default mapping matches expected roll/pitch/throttle/yaw behavior.

## Data needed for a concrete device entry

To build a robust device registry entry and mapping rule in OpenFlight, capture:

- One Windows hardware ID line per device (Device Manager)
- HID report descriptor or OpenFlight HID introspection output
