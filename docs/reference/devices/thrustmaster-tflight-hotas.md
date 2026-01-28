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

This reference captures stable identification, axis-mode semantics, and integration guidance
for reliable OpenFlight support.

## Scope

- Devices: T.Flight HOTAS One (Xbox/PC) and T.Flight HOTAS 4 (PS4/PC)
- Interface: USB HID joystick (not force feedback)
- Focus: axis-mode behavior, yaw semantics, and runtime capability changes

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

## Axis mode semantics (critical)

Both devices expose different axis layouts depending on mode. The most important
behavioral change is yaw: multiple physical yaw sources are coupled in "simple"
mode and become independent axes in "full" mode.

### HOTAS 4 (PC)

Thrustmaster documents two PC modes:

- 4-axis mode:
  - Stick twist yaw, throttle-mounted rudder rocker, and rudder pedal yaw are
    coupled into a single RZ axis.
- 5-axis mode:
  - Stick twist remains RZ.
  - Rudder rocker is exposed as Slider 0.
  - Rudder pedal yaw is exposed as Slider 1.

Implementation implication: yaw sources can appear on RZ, Slider 0, or Slider 1
depending on mode.

### HOTAS One (PC)

Thrustmaster documents two PC modes:

- 4/6 axes mode:
  - Rudder rocker and rudder pedal yaw are coupled with stick twist (yaw).
- 5/8 axes mode:
  - Rudder rocker and rudder pedal yaw become independent of stick twist,
    providing three additional progressive axes.

The HOTAS One manual calls out the Xbox Guide button as the PC mode switch.
The device LED changes to indicate the current mode.

Implementation implication: axis mode is a runtime capability change, not a
compile-time device trait.

### Runtime behavior

- Treat axis mode as a dynamic capability. Users can switch modes mid-session.
- The HID descriptor may or may not change when the mode switch happens. Detect
  changes by usage fingerprint, not by index or axis count alone.

## Physical controls and mechanical features (Hotas One details)

The HOTAS One manual is unusually explicit about physical controls:

- Throttle is detachable and supports multiple mounting positions.
- Rudder (yaw) can come from:
  - stick twist,
  - throttle-mounted rudder rocker,
  - TFRP rudder pedals (RJ12).
- A rudder locking screw can disable stick twist.
- An Xbox/PC selector switch must be set before connecting.
- The device advertises 8 axes, including 3 reserved for TFRP pedals.
- Pedal axes auto-calibrate after moving to their physical stops.

These details matter for interpreting "flat" or non-moving axes and for avoiding
false device health warnings.

## OpenFlight integration guidance

### Do not hardcode yaw == RZ

OpenFlight wants a single logical Yaw channel, but these devices expose multiple
physical yaw sources that can be independent or merged depending on mode.

Practical rule set (auto policy):

1. If a distinct pedals yaw axis exists, prefer it as Yaw.
2. Else if a distinct rudder rocker axis exists, prefer it as Yaw.
3. Else use stick twist as Yaw.
4. Expose a profile override: `yaw_source = pedals|rocker|twist|auto`.

### Treat axis mode as runtime capability change

Implementation shape:

- Enumerate device and build a capabilities fingerprint
  (axis usages present, slider count, hat presence).
- Map to internal channels.
- If the fingerprint changes, hot-swap the device mapping
  (compile off-thread, atomic swap at tick boundary).

### Axis mode detection heuristic

Best-effort heuristic (not a guarantee):

1. Parse HID usages for the joystick collection.
2. If X, Y, and RZ are present but no Slider/Dial usages are present, treat as
   merged mode.
3. If X, Y, RZ, and at least two Slider/Dial usages are present, treat as
   separate-axis mode.
4. If ambiguous, fall back to user selection and show a warning.

### Guardrails and edge cases

- Twist lock: stick twist can be physically locked, producing a flat axis.
  Do not treat a flat twist axis as a device fault.
- Pedal auto-calibration: the first few seconds after connect can look wrong
  until pedals hit their physical stops. Avoid aggressive "unhealthy device"
  judgments during this window.

## Suggested defaults (full-axis modes)

These defaults are for full-axis modes:

- HOTAS 4: 5-axis mode
- HOTAS One: 5/8 axes mode

### Logical mapping policy (full-axis mode)

| Logical | Preferred physical source | Notes |
| --- | --- | --- |
| Roll | Stick X | |
| Pitch | Stick Y | Invert optional |
| Throttle | Throttle axis | Derive by usage from HID descriptor |
| Yaw | Pedals -> rocker -> twist | Use `yaw_source` override |
| POV | Hat | View/Trim policy-dependent |

### Logical mapping policy (merged-axis mode)

When merged mode is detected, yaw sources are coupled and cannot be separated:

- Yaw -> RZ (combined)
- Provide a warning: "Rudder sources are merged. Switch to full-axis mode for
  separate yaw inputs."

## Windows driver reality

If axes or buttons appear missing on PC, Thrustmaster drivers are commonly
required.

Recommended troubleshooting note:

- Install the Thrustmaster driver package and confirm the device is in full-axis
  mode (HOTAS 4: 5-axis, HOTAS One: 5/8 axes).

## Minimum registry facts

Store the following as stable device support facts:

- Vendor: 0x044F (Thrustmaster)
- Known PIDs:
  - 0xB68B: T.Flight HOTAS One (Bulk)
  - 0xB67A: T.Flight HOTAS 4 (Bulk)
- Quirk: axis_mode
  - Merged mode couples multiple yaw sources into RZ.
  - Full-axis mode exposes additional Slider/Dial usages.

## Data needed for a concrete device entry

To build robust device registry entries and mapping rules, capture:

- USB identifiers (VID/PID variants)
- HID report descriptor (axis usages and ordering, hat encoding, button layout)
- Whether mode switching changes the descriptor or only the data stream

## Fast capture plan

- Windows:
  - Capture HID descriptor via USBView (Windows SDK).
  - Compare Windows "Game Controllers" view after each mode switch.
- Linux:
  - `lsusb -v`, `usbhid-dump`, `evtest` for stable event codes.

## Integration checklist

When adding or validating support:

- Verify enumeration by VID/PID fingerprint set (do not assume a single PID).
- Parse HID descriptor and map by usage, not index.
- Confirm axis-mode detection by swapping between modes.
- Verify merged-mode warning triggers when Slider/Dial usages are absent.
- Confirm default mapping matches expected roll/pitch/throttle/yaw behavior.
