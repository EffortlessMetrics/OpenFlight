---
name: HOTAS Protocol Verification
about: Report USB capture data to verify HOTAS protocol claims
title: '[hotas] Verify <CLAIM>'
labels: hotas, verification, community
assignees: ''
---

## Claim Being Verified

<!-- What protocol claim are you verifying? Reference docs/reference/hotas-claims.md -->

## Device Information

- **Device Model:** <!-- e.g., X52 Pro, X56 Stick -->
- **Vendor ID:** <!-- e.g., 0x06A3 -->
- **Product ID:** <!-- e.g., 0x0762 -->
- **Firmware Version:** <!-- From Device Manager or system info -->
- **Purchase Date (approx):** <!-- Helps identify hardware revision -->

## Capture Environment

- **OS:** <!-- e.g., Windows 11 23H2 -->
- **Capture Tool:** <!-- e.g., Wireshark + USBPcap, Wireshark + usbmon -->
- **Official Software Version:** <!-- e.g., Logitech G Hub 2024.x -->

## Verification Steps Performed

1. <!-- Step 1 -->
2. <!-- Step 2 -->
3. <!-- etc. -->

## Captured Data

### USB Control Transfer (if applicable)

```
bmRequestType: 0x__
bRequest: 0x__
wValue: 0x____
wIndex: 0x____
wLength: __
Data: [hex bytes]
```

### HID Report (if applicable)

```
Report ID: __
Data: [hex bytes]
```

### Raw Capture File

<!-- Attach .pcapng file or link to it -->

## Analysis

<!-- Your interpretation of the captured data -->

## Proposed Claim Update

| Field | Old Value | New Value |
|-------|-----------|-----------|
| Confidence | Unverified | Known |
| Details | ... | ... |

## Checklist

- [ ] Captured with official software (not third-party)
- [ ] Firmware version documented
- [ ] Raw capture file attached
- [ ] Hex dump included in issue
- [ ] Tested replay of isolated command (if possible)

## Additional Context

<!-- Any other relevant information -->
