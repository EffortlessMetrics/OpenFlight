# HOTAS Protocol Claims Ledger

This document tracks verified and unverified claims about Saitek/Logitech HOTAS device protocols.

## Confidence Levels

| Level | Meaning |
|-------|---------|
| **Known** | Confirmed via USB-IF registry, official documentation, or verified USB captures |
| **Likely** | Multiple community reports, consistent behavior across sources |
| **Unverified** | Single source or hypothesis, needs USB capture verification |
| **Suspect** | Marketing claims or conflicting information |

## Device Identification

### Vendor IDs (Known)

| Vendor | VID | Notes |
|--------|-----|-------|
| Saitek | 0x06A3 | Original Saitek devices |
| Mad Catz | 0x0738 | Post-acquisition X55/X56 |
| Logitech | 0x046D | Current X52/X56 production |

### Product IDs

| Device | VID | PID | Confidence | Source |
|--------|-----|-----|------------|--------|
| X52 | 0x06A3 | 0x075C | **Known** | USB-IF, libx52 |
| X52 Pro | 0x06A3 | 0x0762 | **Known** | USB-IF, libx52 |
| X55 Stick | 0x06A3 | 0x2215 | **Likely** | Community reports |
| X55 Throttle | 0x06A3 | 0xA215 | **Likely** | Community reports |
| X56 Stick (Saitek) | 0x06A3 | 0x0764 | **Likely** | Community reports |
| X56 Throttle (Saitek) | 0x06A3 | 0x0765 | **Likely** | Community reports |
| X56 Stick (Logitech) | 0x046D | 0xC229 | **Likely** | Community reports |
| X56 Throttle (Logitech) | 0x046D | 0xC22A | **Likely** | Community reports |

## Device Topology

| Device | Topology | Confidence | Notes |
|--------|----------|------------|-------|
| X52 | Unified USB | **Known** | Single composite device |
| X52 Pro | Unified USB | **Known** | Single composite device |
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
| X52 X/Y axes are Hall effect | **Suspect** | Marketing claim, no teardown |
| X52 throttle is potentiometer | **Likely** | Noise characteristics |
| X55/X56 all axes are potentiometers | **Likely** | User reports of drift |
| X56 has improved potentiometers | **Suspect** | Marketing claim |

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
