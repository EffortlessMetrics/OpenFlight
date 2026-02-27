# HOTAS Protocol Claims Ledger

This document tracks verified and unverified claims about Saitek/Logitech HOTAS device protocols.

## Confidence Levels

| Level | Meaning |
|-------|---------|
| **Known** | We have a committed artifact (lsusb dump, HID descriptor fixture, USB capture) |
| **Likely** | Consistent community reports, but no artifact committed yet |
| **Unverified** | Single source or hypothesis, needs USB capture verification |
| **Suspect** | Collision or contradictory evidence - DO NOT USE until verified |

## Device Identification

### Vendor IDs (Known)

| Vendor | VID | Era | Notes |
|--------|-----|-----|-------|
| Saitek | 0x06A3 | Legacy | Original Saitek devices, X52/X52 Pro still use this |
| Mad Catz | 0x0738 | Transitional | Post-acquisition X55/X56 ("blue" variants) |
| Logitech | 0x046D | Modern | Current production; CAUTION: shared with non-HOTAS devices |

### Product IDs

| Device | VID | PID | Topology | Confidence | Evidence Needed |
|--------|-----|-----|----------|------------|-----------------|
| X52 | 0x06A3 | 0x075C | Unified | **Known** | ✓ USB-IF, libx52 |
| X52 Pro | 0x06A3 | 0x0762 | Unified | **Known** | ✓ USB-IF, libx52, hid-saitek.c |
| X65F | 0x06A3 | 0x0B6A | Unified | **Likely** | Linux kernel hid-ids.h; lsusb from real hardware |
| X55 Stick | 0x06A3 | 0x2215 | Split | **Likely** | HID descriptor fixture |
| X55 Throttle | 0x06A3 | 0xA215 | Split | **Likely** | HID descriptor fixture |
| X55 Stick | 0x0738 | 0x2215 | Split | **Likely** | lsusb dump (Mad Catz era) |
| X55 Throttle | 0x0738 | 0xA215 | Split | **Likely** | lsusb dump (Mad Catz era) |
| X56 Stick (Mad Catz) | 0x0738 | 0x2221 | Split | **Likely** | lsusb dump |
| X56 Throttle (Mad Catz) | 0x0738 | 0xA221 | Split | **Likely** | lsusb dump |
| X56 Stick (Logitech) | 0x046D | 0xC229 | Split | **Likely** | lsusb dump |
| X56 Throttle (Logitech) | 0x046D | ???? | Split | **Unverified** | lsusb from real hardware |

### Suspect / Collision Claims

| Claim | Status | Risk | Action Required |
|-------|--------|------|-----------------|
| Logitech X56 Throttle = 046D:C22A | **Suspect** | HIGH | 046D:C22A is assigned to Logitech G110 keyboard. Driver collision risk. Requires lsusb verification from a physical Logitech-era X56 throttle. |

> **IMPORTANT**: The codebase intentionally does NOT match Logitech VID with unknown PIDs.
> This prevents accidentally binding to non-HOTAS Logitech devices.

## Device Topology

| Device | Topology | Confidence | Notes |
|--------|----------|------------|-------|
| X52 | Unified USB | **Known** | Single composite device |
| X52 Pro | Unified USB | **Known** | Single composite device |
| X65F | Unified USB | **Likely** | Single composite device (F-22 style HOTAS) |
| X55 | Split USB | **Known** | Separate stick/throttle |
| X56 | Split USB | **Known** | Separate stick/throttle |

## Input Path Claims

| Claim | Confidence | Verification |
|-------|------------|--------------|
| All devices use standard HID for input | **Known** | Works with generic HID drivers |
| X52/X52 Pro have 11-bit axis resolution | **Unverified** | Needs HID descriptor analysis |
| X55/X56 have 16-bit axis resolution | **Unverified** | Needs HID descriptor analysis |
| Ghost inputs on X55/X56 mini-sticks | **Known** | Widely reported hardware issue |

## Output Path Claims (X52 Pro)

### MFD Protocol

| Claim | Confidence | Verification Needed |
|-------|------------|---------------------|
| Uses USB control transfers | **Likely** | USB capture |
| bmRequestType = 0x40 (vendor, host-to-device) | **Unverified** | USB capture |
| bRequest = 0x91 for line write | **Unverified** | USB capture from official software |
| wValue encodes line number (0-2) | **Unverified** | USB capture |
| Text encoding is ASCII subset | **Unverified** | USB capture |
| Max 16 characters per line | **Likely** | Physical display size |

### LED Protocol

| Claim | Confidence | Verification Needed |
|-------|------------|---------------------|
| Uses USB control transfers | **Likely** | USB capture |
| bRequest = 0xB8 for LED control | **Unverified** | USB capture |
| wValue = LED ID | **Unverified** | USB capture |
| wIndex = color/state | **Unverified** | USB capture |
| Supports green, amber, red states | **Likely** | Visual observation |

## Output Path Claims (X56)

### RGB Protocol

| Claim | Confidence | Verification Needed |
|-------|------------|---------------------|
| Uses USB control transfers | **Unverified** | USB capture |
| Full RGB color control | **Likely** | Product marketing, user reports |
| Per-zone color control | **Unverified** | USB capture |
| Packet format unknown | - | Needs complete protocol capture |

## Hardware Characteristics

| Claim | Confidence | Notes |
|-------|------------|-------|
| X52 X/Y axes are Hall effect | **Suspect** | Marketing claim, no teardown verification |
| X52 throttle is potentiometer | **Likely** | Noise characteristics consistent with resistive wiper |
| X56 Stick main X/Y axes are Hall effect | **Likely** | Confirmed by marketing and user teardowns |
| X56 mini-sticks are potentiometers | **Known** | Standard dual-pot modules, susceptible to drift |
| X56 throttle axis is potentiometer | **Suspect** | Marketing implies Hall, but drift reports suggest otherwise |

## Ghost Input Analysis

Ghost inputs are a **verified electrical phenomenon**, not a protocol defect.

### Root Causes

| Cause | Mechanism | Affected Devices |
|-------|-----------|------------------|
| Power starvation | USB 2.0 ports limited to 500mA; X56 RGB + sensors exceed this | X55, X56 |
| EMI/Crosstalk | Unshielded wiring bundles; PWM LED signals induce noise on button lines | X55, X56 |
| Voltage sag | Logic threshold drift when 5V rail sags under load | X55, X56 |

### Verified Mitigations

| Mitigation | Effectiveness | Notes |
|------------|---------------|-------|
| Powered USB 3.0 hub | **High** | Provides stable 5V rail, 900mA per port |
| USB port isolation | **Medium** | Stick/throttle on separate host controllers |
| Software debouncing | **Low** | Can help but doesn't address root cause |

### Software Approach

The ghost filter should:
1. **Observe first**: Count and surface "ghostiness" metrics without dropping events by default
2. **Suppress only when enabled**: User/config must explicitly enable event dropping
3. **Never guess**: Impossible-state detection must be data-driven, not hardcoded per device

## Verification Artifacts

Verified protocol captures should be stored in `fixtures/hotas/`:

```
fixtures/hotas/
  x52pro/
    descriptor.bin      # HID report descriptor
    mfd_captures.md     # Documented USB captures
    led_captures.md     # Documented USB captures
  x56/
    stick_descriptor.bin
    throttle_descriptor.bin
    rgb_captures.md
```

## Contributing

To verify a claim:

1. Use `cargo xtask hotas capture <device>` to record USB traffic
2. Document findings in the appropriate captures file
3. Update this ledger with verification results
4. Submit PR with artifacts

See `.github/ISSUE_TEMPLATE/hotas-verification.md` for verification request template.
