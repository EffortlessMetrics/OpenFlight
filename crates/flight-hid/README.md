# Flight HID

Human Interface Device (HID) protocol implementation for Flight Hub, providing support for flight control hardware using the OFP1 (OpenFlight Protocol v1) specification.

## Features

- **OFP1 Protocol**: Implementation of OpenFlight Protocol v1 for HID devices
- **Packed Struct Support**: Safe accessors for packed field manipulation
- **Device Capabilities**: Capability reporting and feature negotiation
- **Health Monitoring**: Device health status reporting
- **Type Safety**: Strongly-typed protocol messages with validation

## Architecture

The HID layer provides low-level protocol support for flight control devices:

- **Protocol Definitions** (`protocol/ofp1.rs`): OFP1 message types and structures
- **Safe Accessors**: Helper methods for packed struct field manipulation
- **Device Emulation**: Virtual device support for testing and development

## Usage

### Basic Device Interaction

```rust
use flight_hid::protocol::ofp1::{CapabilitiesReport, CapabilityFlags};

// Create a capabilities report
let mut report = CapabilitiesReport::default();

// Use safe helper methods to modify packed fields
report.set_cap_flag(CapabilityFlags::ANALOG_INPUT);
report.set_cap_flag(CapabilityFlags::DIGITAL_OUTPUT);

// Read capabilities safely
let flags = report.cap_flags();
assert!(flags.has_flag(CapabilityFlags::ANALOG_INPUT));
```

### Health Status Reporting

```rust
use flight_hid::protocol::ofp1::{HealthStatusReport, StatusFlags};

let mut status = HealthStatusReport::default();
status.set_status_flag(StatusFlags::OPERATIONAL);

let flags = status.status_flags();
assert!(flags.has_flag(StatusFlags::OPERATIONAL));
```

## Cargo Features

### Development-Only Features

These features are intended for development and testing only:

- `ofp1-tests`: Enable `Clone` and `Copy` derives on public types for external integration tests (dev-only, not for production use)

**Note:** The `ofp1-tests` feature is specifically designed for integration tests in other crates (e.g., `flight-virtual`) that need to clone or copy OFP1 protocol structures. This feature should not be enabled in production builds.

## Packed Struct Safety

The OFP1 protocol uses `#[repr(packed)]` structs for efficient wire format representation. Direct field access on packed structs can cause undefined behavior. This crate provides safe helper methods:

### Safe Accessors

Instead of directly accessing packed fields:

```rust
// ❌ UNSAFE: Direct field access on packed struct
report.capability_flags.set_flag(flag);  // Causes E0793 error
```

Use the provided helper methods:

```rust
// ✅ SAFE: Use helper methods
report.set_cap_flag(flag);
let flags = report.cap_flags();
```

### Available Helpers

**CapabilitiesReport:**
- `cap_flags(&self) -> CapabilityFlags` - Read capability flags
- `set_cap_flag(&mut self, flag: CapabilityFlags)` - Set a capability flag
- `clear_cap_flag(&mut self, flag: CapabilityFlags)` - Clear a capability flag

**HealthStatusReport:**
- `status_flags(&self) -> StatusFlags` - Read status flags
- `set_status_flag(&mut self, flag: StatusFlags)` - Set a status flag
- `clear_status_flag(&mut self, flag: StatusFlags)` - Clear a status flag

## Testing

```bash
# Run unit tests
cargo test -p flight-hid

# Run tests with ofp1-tests feature (for integration testing)
cargo test -p flight-hid --features ofp1-tests
```

## Integration with Other Crates

When writing integration tests in other crates that need to use flight-hid types:

```toml
# In your crate's Cargo.toml
[dev-dependencies]
flight-hid = { path = "../flight-hid", features = ["ofp1-tests"] }
```

This enables `Clone` and `Copy` derives on protocol types for easier test setup.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
