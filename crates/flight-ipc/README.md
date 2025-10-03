# Flight Hub IPC

Cross-platform IPC (Inter-Process Communication) layer for Flight Hub, providing protobuf-based communication between Flight Hub components using named pipes on Windows and Unix domain sockets on Linux.

## Features

- **Feature Negotiation**: Automatic capability negotiation between client and server
- **Cross-Platform Transport**: Named pipes (Windows) and Unix domain sockets (Linux/macOS)
- **Type Safety**: Generated protobuf types with serde serialization support
- **Version Compatibility**: Semantic versioning with breaking change detection
- **Real-Time Health Monitoring**: Streaming health events and performance metrics
- **Device Management**: Comprehensive device listing and status monitoring
- **Profile Management**: JSON schema-based profile validation and application

## Architecture

The IPC layer follows a schema-first approach with the following components:

This crate implements key architectural decisions:

- **[ADR-010: Schema Versioning Strategy](../../docs/adr/010-schema-versioning-strategy.md)** - Comprehensive schema versioning with migration support
- **[ADR-006: Driver-Light Integration Approach](../../docs/adr/006-driver-light-approach.md)** - Local-only IPC with minimal system footprint

- **Proto Schema** (`proto/flight.v1.proto`): Defines all service interfaces and message types
- **Transport Layer** (`transport.rs`): Platform-specific transport implementations
- **Client** (`client.rs`): High-level client API with feature negotiation
- **Server** (`server.rs`): Service implementation with dependency injection
- **Negotiation** (`negotiation.rs`): Version compatibility and feature negotiation logic

## Usage

### Client Example

```rust
use flight_ipc::client::FlightClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect with automatic feature negotiation
    let mut client = FlightClient::connect().await?;
    
    // List connected devices
    let devices = client.list_devices().await?;
    println!("Found {} devices", devices.len());
    
    // Subscribe to health events
    let mut health_stream = client.subscribe_health().await?;
    while let Some(event) = health_stream.next().await {
        println!("Health event: {:?}", event?);
    }
    
    Ok(())
}
```

### Server Example

```rust
use flight_ipc::server::FlightServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = FlightServer::new();
    server.serve().await?;
    Ok(())
}
```

## Protocol Schema

The IPC protocol is defined in `proto/flight.v1.proto` and includes:

### Core Services

- **Feature Negotiation**: `NegotiateFeatures` - Version and capability negotiation
- **Device Management**: `ListDevices` - Enumerate and monitor flight control devices
- **Health Monitoring**: `HealthSubscribe` - Real-time health and performance events
- **Profile Management**: `ApplyProfile` - JSON schema-based profile validation
- **Service Info**: `GetServiceInfo` - Service status and capabilities

### Message Types

- **Device**: Complete device information including capabilities and health
- **HealthEvent**: Structured health events with performance metrics
- **ValidationError**: Detailed profile validation errors with line/column info

## Feature Negotiation

The IPC layer supports feature negotiation to ensure compatibility between different versions:

```rust
// Client specifies supported features
let config = ClientConfig {
    client_version: "1.0.0".to_string(),
    supported_features: vec![
        "device-management".to_string(),
        "health-monitoring".to_string(),
    ],
    preferred_transport: TransportType::NamedPipes,
    connection_timeout_ms: 5000,
};

let client = FlightClient::connect_with_config(config).await?;
```

## Transport Layer

The transport layer provides cross-platform abstractions:

- **Windows**: Named pipes (`\\.\pipe\flight-hub`)
- **Linux/macOS**: Unix domain sockets (`/tmp/flight-hub.sock`)

Transport selection is automatic based on the platform, with fallback options available.

## Breaking Change Detection

The crate includes tooling for detecting breaking changes in the protocol schema:

```bash
# Check for breaking changes between versions
cargo run --bin check-breaking-changes old-schema.proto new-schema.proto
```

## Performance

The IPC layer is designed for high-performance real-time communication:

- **Zero-copy serialization** where possible
- **Streaming health events** with configurable buffering
- **Connection pooling** for multiple concurrent clients
- **Efficient protobuf encoding** with optional compression

## Testing

Comprehensive test suite includes:

- **Unit tests** for individual components
- **Integration tests** for round-trip functionality
- **Performance benchmarks** for serialization and transport
- **Property-based tests** for protocol validation

```bash
# Run all tests
cargo test

# Run benchmarks
cargo bench

# Run examples
cargo run --example list_devices
cargo run --example health_subscribe
```

## CI/CD Integration

The crate includes GitHub Actions workflows for:

- **Cross-platform testing** (Windows, Linux)
- **Breaking change detection** on pull requests
- **Feature matrix testing** for different feature combinations
- **Security auditing** with cargo-audit
- **Performance regression detection**

## Error Handling

Structured error handling with stable error codes:

```rust
use flight_ipc::IpcError;

match client.list_devices().await {
    Ok(devices) => println!("Found {} devices", devices.len()),
    Err(IpcError::VersionMismatch { client, server }) => {
        eprintln!("Version mismatch: client={}, server={}", client, server);
    }
    Err(IpcError::UnsupportedFeature { feature }) => {
        eprintln!("Feature not supported: {}", feature);
    }
    Err(e) => eprintln!("IPC error: {}", e),
}
```

## Security

- **Local-only communication** by default (no network listeners)
- **OS-level access controls** on named pipes and Unix sockets
- **Capability-based permissions** for feature access
- **Signed binary validation** in production builds

## Requirements

- **IFC-01**: Cross-platform IPC with feature negotiation ✅
- **XPLAT-01**: Native Windows and Linux support ✅

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.