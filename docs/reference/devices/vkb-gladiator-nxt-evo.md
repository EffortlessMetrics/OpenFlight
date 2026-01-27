---
doc_id: DOC-REF-VKB-GLADIATOR-NXT-EVO
kind: reference
area: flight-hid
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
---

# VKB Gladiator NXT EVO / Omni Throttle - Device Notes

This reference captures stable identification, configurability guardrails, and integration guidance
for VKB Gladiator NXT EVO class devices, including the Omni Throttle variant.

## Scope

- Devices: Gladiator NXT EVO (Left/Right) and Omni Throttle (OTA) variants
- Interface: USB HID joystick (not force feedback)
- Focus: USB identity, descriptor-first mapping, and GNX ecosystem behavior

## Stable vs configurable behavior

### Stable (safe to code against)

- Standard USB HID joystick family. Use HID report descriptors and usages to discover axes, hats, and buttons.
- Omni Throttle is a mechanical adapter on the Gladiator NXT EVO electronics; it does not imply a unique USB PID.

### Configurable (do not hardcode)

- VKBDevCfg profiles can remap hats, ministicks, encoders, and axes.
- Firmware can expose multiple HID devices or top-level collections when GNX modules are attached.

## USB identity (fingerprints)

- Vendor ID: 0x231D (VKB-Sim)
- Known PIDs observed in community device listings:
  - 0x0200: Gladiator NXT EVO Right
  - 0x0201: Gladiator NXT EVO Left

Notes:
- Treat the PID list as a fingerprint set, not a single constant.
- Omni Throttle variants typically use the same EVO Left/Right PIDs.

## Expected control surface (baseline)

The Gladiator NXT EVO family exposes lots of controls compared to gamepads:
- Multiple hats, triggers, and buttons
- Analog ministick with push
- Base controls such as a throttle wheel and encoder

Exact layout is configuration-dependent; use the HID descriptor as the source of truth.

## GNX ecosystem considerations

- GNX modules can be attached and may appear as separate HID devices.
- VKB firmware can split large axis counts across multiple virtual devices to stay within legacy limits.
- Prefer raw HID ingestion and merge devices into a single logical rig using serial or arrival-time grouping.

## OpenFlight integration guidance

- Treat the HID report descriptor as the contract; map by usage and logical min/max.
- Avoid static axis ordering; record usage plus logical range in profiles.
- Provide a user-level export of discovered controls and allow semantic annotations (throttle, POV, etc.).
- Plan for multi-device composition when GNX modules are present.

## Data needed for a concrete device entry

To ship a deterministic default map, capture:
- HID report descriptor for each top-level collection
- Sample reports while exercising every control
- Any VKBDevCfg profiles used to produce the report layout

## Fast capture plan

- Windows: USBView (descriptor), raw HID logging, and VKBDevCfg profile export
- Linux: lsusb -v, usbhid-dump, evtest, hid-recorder
