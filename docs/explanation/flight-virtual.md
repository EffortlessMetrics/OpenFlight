---
doc_id: DOC-VIRTUAL-OVERVIEW
kind: explanation
area: flight-virtual
status: draft
links:
  requirements: []
  tasks: []
  adrs: []
---

# Flight Virtual Concepts

The `flight-virtual` crate provides virtual device emulation for testing and development without physical hardware.

## Overview

Flight Virtual enables:
- Virtual OFP1 device emulation
- Loopback testing for force feedback
- Hardware-in-the-loop (HIL) test support
- Performance validation without physical devices

## Key Components

### Virtual Device

The virtual device emulates the behavior of physical flight control hardware, allowing developers to test the full stack without requiring actual hardware.

### OFP1 Emulator

The OFP1 emulator provides a software implementation of the OFP1 protocol, enabling:
- Force feedback effect testing
- USB communication simulation
- Fault injection for robustness testing

### Loopback Testing

Loopback mode allows the system to test round-trip communication paths by routing output back as input, validating:
- Protocol correctness
- Timing characteristics
- Data integrity

## Use Cases

### Development

Developers can work on force feedback algorithms and control logic without physical hardware, speeding up the development cycle.

### Testing

Automated tests can run against virtual devices in CI, ensuring consistent test environments and enabling comprehensive test coverage.

### Performance Validation

Performance gates can validate latency and throughput characteristics using virtual devices with known timing properties.

## Related Components

- `flight-hid`: Hardware interface layer that virtual devices emulate
- `flight-ffb`: Force feedback system that uses virtual devices for testing
