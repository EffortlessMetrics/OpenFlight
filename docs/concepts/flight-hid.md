---
doc_id: DOC-HID-OVERVIEW
kind: concept
area: flight-hid
status: active
links:
  requirements: [REQ-3]
  tasks: []
  adrs: []
---

# Flight HID Concepts

The `flight-hid` crate provides low-level hardware interface abstractions for USB HID devices, specifically targeting force feedback flight control hardware.

## Overview

Flight HID is responsible for:
- USB HID device enumeration and connection management
- OFP1 protocol implementation for force feedback devices
- Raw device I/O with minimal overhead
- File descriptor safety and resource management

## Key Components

### OFP1 Protocol

The OFP1 (OpenFlight Protocol v1) is a custom USB HID protocol for force feedback devices. It provides:
- Bidirectional communication with force feedback hardware
- Effect upload and playback control
- Device state queries and configuration
- Low-latency command/response patterns

### Device Management

The device management layer handles:
- USB device hotplug detection
- Device capability negotiation
- Connection lifecycle management
- Error recovery and reconnection

### Safety Guarantees

Flight HID implements strict safety guarantees:
- File descriptor leak prevention
- Proper cleanup on device disconnect
- Thread-safe device access
- Resource exhaustion protection

## Performance Characteristics

- Device enumeration: < 100ms
- Command latency: < 1ms typical
- Effect upload: < 10ms for standard effects
- Zero-copy I/O where possible

## Related Requirements

This component implements **REQ-3: Force Feedback Device Support**, which specifies the requirements for USB HID device communication and force feedback effect management.

## Related Components

- `flight-ffb`: Higher-level force feedback system that uses flight-hid
- `flight-virtual`: Virtual device emulation for testing
- `flight-service`: Service layer that manages device lifecycle

## Testing

Flight HID includes comprehensive tests:
- Unit tests for protocol encoding/decoding
- Integration tests with virtual devices
- File descriptor safety tests
- Hardware-in-the-loop tests (when hardware available)

