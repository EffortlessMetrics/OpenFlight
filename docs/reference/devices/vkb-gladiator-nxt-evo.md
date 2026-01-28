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
- VKB firmware can split large axis counts across multiple virtual devices to stay within DirectInput limits.

## USB identity (fingerprints)

- Vendor ID: 0x231D (VKB-Sim)
- Known PIDs observed in community device listings:
  - 0x0200: Gladiator NXT EVO Right
  - 0x0201: Gladiator NXT EVO Left

Notes:
- Treat the PID list as a fingerprint set, not a single constant.
- Omni Throttle variants typically use the same EVO Left/Right PIDs.

### Windows hardware ID examples

- EVO Right: `USB\\VID_231D&PID_0200`
- EVO Left: `USB\\VID_231D&PID_0201`

## Expected control surface (baseline)

The Gladiator NXT EVO family exposes many controls compared to gamepads:

- Grip: multiple hats, triggers, buttons, and an analog ministick with push
- Base: throttle wheel, encoder, and extra buttons

Exact layout is configuration-dependent; use the HID descriptor as the source of truth.

## GNX ecosystem considerations

- GNX modules can be attached and may appear as separate HID devices.
- VKB firmware can split large axis counts across multiple virtual devices to stay within legacy limits.
- Prefer raw HID ingestion and merge devices into a single logical rig using serial or arrival-time grouping.
- GNX modules can be used standalone via the GNX USB controller (HID-Main), or combined with Gladiator bases.

## OpenFlight integration guidance

### Descriptor-first mapping

- Treat the HID report descriptor as the contract; map by usage and logical min/max.
- Avoid static axis ordering; record usage plus logical range in profiles.
- Export discovery data so users can annotate semantics (throttle, POV, etc.).

OpenFlight exposes descriptor discovery in device metadata and via:

- `flightctl devices dump <device-id>`
- `descriptor_discovery` metadata key (JSON summary + discovered controls)

### VKB family adapter (thin)

- Match by VID 0x231D and product string for family detection.
- Apply only soft hints (deadzone defaults, semantic suggestions for X/Y/RZ/Slider usages).
- Keep per-PID templates optional and clearly marked as best-effort.

### Multi-device composition

When GNX modules are present, devices may enumerate as a bundle of HID collections.
Group by (VID, serial, arrival window) and present a single logical rig.

## Data needed for a deterministic device entry

To ship a deterministic default map, capture:

- HID report descriptor for each top-level collection
- Sample reports while exercising every control
- Any VKBDevCfg profiles used to produce the report layout

## Fast capture plan

- Windows: USBView (descriptor), raw HID logging, VKBDevCfg profile export
- Linux: lsusb -v, usbhid-dump, evtest, hid-recorder
