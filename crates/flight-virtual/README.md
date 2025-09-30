# flight-virtual

Virtual HID devices and performance testing infrastructure for Flight Hub CI.

## Overview

The flight-virtual crate provides virtual flight control devices and comprehensive performance testing capabilities. It enables CI testing without physical hardware while maintaining realistic timing and behavior characteristics.

## Key Features

- **Virtual HID Devices**: Simulate joysticks, throttles, rudders, and panels
- **Loopback HID Interface**: Test HID communication without hardware
- **Performance Gates**: Automated performance regression detection
- **Realistic Simulation**: Packet loss, latency, and device health simulation

## Architecture

This crate supports the Real-Time Spine architecture by providing test infrastructure that validates timing guarantees without requiring physical hardware. It implements the performance gate system that enforces quality requirements in CI.

### Core Components

- **VirtualDevice**: Simulates flight control hardware with realistic behavior
- **LoopbackHid**: Provides HID communication testing without hardware
- **PerfGate**: Automated performance testing and regression detection
- **TimingValidator**: Long-running timing discipline validation

## Usage

### Virtual Device Creation

```rust
use flight_virtual::{VirtualDeviceManager, VirtualDeviceConfig, DeviceType};

let mut manager = VirtualDeviceManager::new();

// Create virtual joystick
let config = VirtualDeviceConfig {
    name: "Test Joystick".to_string(),
    device_type: DeviceType::Joystick { axes: 3 },
    vid: 0x1234,
    pid: 0x5678,
    serial: "TEST001".to_string(),
    latency_us: 100,
    packet_loss_rate: 0.001,
};

let device = manager.create_device(config);

// Simulate input
device.set_axis(0, 0.5);  // 50% deflection
device.set_button(0, true);

// Generate HID report
if let Some(report) = device.generate_input_report() {
    // Process report data
}
```

### Performance Gate Testing

```rust
use flight_virtual::{PerfGate, PerfGateConfig};

let config = PerfGateConfig {
    frequency_hz: 250,
    duration: Duration::from_secs(60),
    max_jitter_p99_ns: 500_000,  // 0.5ms
    max_hid_latency_p99_us: 300, // 300μs
    max_miss_rate: 0.001,        // 0.1%
    hid_samples: 1000,
};

let mut gate = PerfGate::new(config);
let result = gate.run();

if !result.passed {
    eprintln!("Performance gate FAILED");
    std::process::exit(1);
}
```

### HID Loopback Testing

```rust
use flight_virtual::{LoopbackHid, HidReport};

let loopback = LoopbackHid::new();

// Send input report (device -> host)
let report = HidReport::new(0x01, vec![0x12, 0x34]);
loopback.send_input_report(report);

// Receive on host side
if let Some(received) = loopback.receive_input_report() {
    assert_eq!(received.data, vec![0x12, 0x34]);
}

// Check statistics
let stats = loopback.get_stats();
println!("Average latency: {:.1}μs", stats.avg_latency_us);
```

## Performance Testing

The crate provides comprehensive performance testing capabilities:

### Quick Tests (CI)
- Basic scheduler timing validation
- HID latency measurement
- Virtual device functionality

### Full Validation (Nightly)
- Extended timing discipline tests (10+ minutes)
- Comprehensive jitter analysis
- Overload behavior validation
- Multi-device stress testing

### Quality Gates

Performance gates enforce strict requirements:

- **Jitter p99**: ≤ 0.5ms over continuous operation
- **HID Latency p99**: ≤ 300μs for write operations
- **Miss Rate**: ≤ 0.1% of ticks significantly late
- **Zero Drops**: No dropped samples in 10-minute captures

## CI Integration

The performance gate system integrates with GitHub Actions:

```yaml
- name: Run Performance Gate
  run: |
    cargo test --package flight-virtual --lib test_ci_benchmark -- --nocapture
    
    # Check if performance gate passed
    if [ $? -ne 0 ]; then
      echo "❌ Performance gate FAILED - build rejected"
      exit 1
    fi
```

## Device Simulation

Virtual devices simulate realistic hardware behavior:

### Timing Characteristics
- Configurable USB latency (50-200μs typical)
- Packet loss simulation for stress testing
- Device health monitoring (temperature, voltage, current)

### Device Types
- **Joystick**: Multi-axis flight stick with buttons
- **Throttle**: Multi-lever throttle quadrant
- **Rudder**: Rudder pedals with toe brakes
- **Panel**: LED/switch panels for aircraft systems
- **Force Feedback**: FFB-capable devices with torque simulation

### Realistic Behavior
- USB frame timing (1ms intervals)
- Report rate limiting
- Device disconnect/reconnect simulation
- Calibration drift over time

## Architecture Decisions

This crate supports several architectural decisions:

- **[ADR-001: Real-Time Spine Architecture](../../docs/adr/001-rt-spine-architecture.md)** - Provides test infrastructure for RT validation
- **[ADR-004: Zero-Allocation Constraint](../../docs/adr/004-zero-allocation-constraint.md)** - Validates allocation-free operation

## Testing

```bash
# Run all virtual device tests
cargo test --package flight-virtual

# Run quick performance gate
cargo test --package flight-virtual test_ci_benchmark -- --nocapture

# Run extended integration tests (requires time)
cargo test --package flight-virtual test_extended_integration -- --ignored --nocapture
```

## CI Performance Dashboard

The performance gate exports metrics for tracking:

- `FLIGHT_HUB_JITTER_P99_US`: Scheduler jitter p99 in microseconds
- `FLIGHT_HUB_HID_P99_US`: HID write latency p99 in microseconds  
- `FLIGHT_HUB_MISS_RATE`: Fraction of missed timing deadlines

These metrics are tracked over time to detect performance regressions.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.