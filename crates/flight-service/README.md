# flight-service

Main service orchestration for Flight Hub, coordinating real-time engines, adapters, and user interfaces.

## Overview

The flight-service crate provides the main service implementation that orchestrates all Flight Hub components. It manages the application lifecycle, coordinates between real-time engines and adapters, handles IPC communication, and provides health monitoring and diagnostics. This is the primary entry point for the Flight Hub system.

## Key Features

- **Service Orchestration**: Coordinates RT engines, adapters, and UI components
- **Health Monitoring**: Comprehensive system health and performance monitoring
- **Profile Management**: Automatic profile application and aircraft detection
- **Safety Coordination**: Manages safety gates and fault responses across subsystems
- **IPC Server**: Provides protobuf-based IPC for client communication
- **Graceful Shutdown**: Clean shutdown with proper resource cleanup

## Architecture

This crate implements the application layer that ties together all architectural decisions:

- **[ADR-001: Real-Time Spine Architecture](../../docs/adr/001-rt-spine-architecture.md)** - Orchestrates RT and non-RT systems
- **[ADR-002: Writers as Data Pattern](../../docs/adr/002-writers-as-data.md)** - Manages sim configuration writers
- **[ADR-003: Plugin Classes](../../docs/adr/003-plugin-classes.md)** - Coordinates plugin execution and isolation

## Core Components

### Service Manager

```rust
use flight_service::{FlightService, ServiceConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create service with configuration
    let config = ServiceConfig {
        rt_priority: true,
        safe_mode: false,
        ipc_endpoint: "flight-hub".to_string(),
        log_level: "info".to_string(),
    };
    
    let service = FlightService::new(config).await?;
    
    // Start all subsystems
    service.start().await?;
    
    // Run until shutdown signal
    service.run_until_shutdown().await?;
    
    Ok(())
}
```

### Health Monitoring

```rust
use flight_service::{HealthMonitor, SystemHealth, ComponentHealth};

let health_monitor = HealthMonitor::new();

// Subscribe to health events
let mut health_stream = health_monitor.subscribe().await?;

while let Some(health) = health_stream.next().await {
    match health.overall_status {
        SystemHealth::Healthy => println!("System healthy"),
        SystemHealth::Degraded => println!("System degraded: {}", health.issues),
        SystemHealth::Faulted => println!("System faulted: {}", health.faults),
    }
}
```

## Service Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 Flight Service                          │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐   │
│  │ IPC Server  │ │ Health Mon  │ │ Profile Manager │   │
│  └─────────────┘ └─────────────┘ └─────────────────┘   │
└─────────────────────┬───────────────────────────────────┘
                      │ Orchestration
┌─────────────────────┴───────────────────────────────────┐
│                RT Engines                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐   │
│  │ Axis Engine │ │ FFB Engine  │ │   Scheduler     │   │
│  └─────────────┘ └─────────────┘ └─────────────────┘   │
└─────────────────────┬───────────────────────────────────┘
                      │ Device I/O
┌─────────────────────┴───────────────────────────────────┐
│                  Adapters                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐   │
│  │ HID Adapter │ │ Sim Adapters│ │ Panel Adapters  │   │
│  └─────────────┘ └─────────────┘ └─────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Startup Sequence

```rust
use flight_service::{StartupSequence, StartupPhase};

impl FlightService {
    async fn start(&self) -> Result<(), ServiceError> {
        let sequence = StartupSequence::new();
        
        // Phase 1: Initialize core systems
        sequence.execute_phase(StartupPhase::CoreInit).await?;
        
        // Phase 2: Start RT engines
        sequence.execute_phase(StartupPhase::RtEngines).await?;
        
        // Phase 3: Initialize adapters
        sequence.execute_phase(StartupPhase::Adapters).await?;
        
        // Phase 4: Start IPC server
        sequence.execute_phase(StartupPhase::IpcServer).await?;
        
        // Phase 5: Enable user interfaces
        sequence.execute_phase(StartupPhase::UserInterfaces).await?;
        
        Ok(())
    }
}
```

## Profile Management Integration

```rust
use flight_service::{ProfileManager, AircraftDetection};

// Automatic profile switching
let profile_manager = ProfileManager::new();

// Subscribe to aircraft changes
let mut aircraft_stream = profile_manager.subscribe_aircraft_changes().await?;

while let Some(aircraft_change) = aircraft_stream.next().await {
    println!("Aircraft changed: {} -> {}", 
             aircraft_change.from, aircraft_change.to);
    
    // Profile applied automatically within 500ms
    let profile_applied = profile_manager.wait_for_profile_application().await?;
    println!("Profile applied: {}", profile_applied.profile_hash);
}
```

## Safety Gate Coordination

```rust
use flight_service::{SafetyGate, SafetyLevel, FaultCoordinator};

let safety_gate = SafetyGate::new();

// Configure safety levels
safety_gate.configure_level(SafetyLevel::Safe, vec![
    "axis_processing",
    "basic_hid_output",
]).await?;

safety_gate.configure_level(SafetyLevel::Normal, vec![
    "axis_processing",
    "ffb_safe_torque", 
    "panel_integration",
    "sim_adapters",
]).await?;

safety_gate.configure_level(SafetyLevel::HighTorque, vec![
    "axis_processing",
    "ffb_high_torque",
    "panel_integration", 
    "sim_adapters",
    "plugin_system",
]).await?;

// Coordinate fault responses
let fault_coordinator = FaultCoordinator::new();
fault_coordinator.register_fault_handler(|fault| async move {
    match fault.severity {
        FaultSeverity::Critical => safety_gate.set_level(SafetyLevel::Safe).await,
        FaultSeverity::Major => safety_gate.set_level(SafetyLevel::Normal).await,
        FaultSeverity::Minor => { /* Log and continue */ },
    }
}).await?;
```

## IPC Service Implementation

```rust
use flight_service::{IpcService, FlightServiceImpl};
use flight_ipc::flight_service_server::FlightServiceServer;

#[tonic::async_trait]
impl FlightService for FlightServiceImpl {
    async fn list_devices(
        &self,
        request: Request<ListDevicesRequest>,
    ) -> Result<Response<ListDevicesResponse>, Status> {
        let devices = self.device_manager.list_devices().await?;
        Ok(Response::new(ListDevicesResponse { devices }))
    }
    
    async fn apply_profile(
        &self,
        request: Request<ApplyProfileRequest>,
    ) -> Result<Response<ApplyProfileResponse>, Status> {
        let profile = request.into_inner().profile;
        let result = self.profile_manager.apply_profile(profile).await?;
        Ok(Response::new(ApplyProfileResponse { 
            success: result.success,
            profile_hash: result.hash,
        }))
    }
    
    async fn health_subscribe(
        &self,
        request: Request<HealthSubscribeRequest>,
    ) -> Result<Response<Self::HealthSubscribeStream>, Status> {
        let stream = self.health_monitor.subscribe().await?;
        Ok(Response::new(Box::pin(stream)))
    }
}
```

## Safe Mode Operation

```rust
use flight_service::{SafeMode, SafeModeConfig};

// Start in safe mode (--safe flag)
let safe_config = SafeModeConfig {
    axis_only: true,
    no_panels: true,
    no_plugins: true,
    no_tactile: true,
    basic_profile: true,
};

let service = FlightService::new_safe_mode(safe_config).await?;
```

## Power Management Integration

```rust
use flight_service::{PowerManager, PowerHints};

let power_manager = PowerManager::new();

// Check and configure power settings
let hints = power_manager.check_power_configuration().await?;

for hint in hints {
    match hint {
        PowerHints::UsbSelectiveSuspend => {
            println!("⚠️ USB selective suspend enabled - may cause timing issues");
            power_manager.suggest_usb_power_fix().await?;
        },
        PowerHints::PowerThrottling => {
            println!("⚠️ Power throttling detected - disabling for RT performance");
            power_manager.disable_power_throttling().await?;
        },
        PowerHints::RtkitUnavailable => {
            println!("⚠️ rtkit unavailable - RT scheduling may be limited");
            power_manager.suggest_rtkit_setup().await?;
        },
    }
}
```

## Graceful Shutdown

```rust
use flight_service::{ShutdownCoordinator, ShutdownPhase};

impl FlightService {
    async fn shutdown(&self) -> Result<(), ServiceError> {
        let coordinator = ShutdownCoordinator::new();
        
        // Phase 1: Stop accepting new requests
        coordinator.execute_phase(ShutdownPhase::StopAccepting).await?;
        
        // Phase 2: Drain in-flight requests
        coordinator.execute_phase(ShutdownPhase::DrainRequests).await?;
        
        // Phase 3: Stop adapters
        coordinator.execute_phase(ShutdownPhase::StopAdapters).await?;
        
        // Phase 4: Stop RT engines (with torque ramp)
        coordinator.execute_phase(ShutdownPhase::StopRtEngines).await?;
        
        // Phase 5: Final cleanup
        coordinator.execute_phase(ShutdownPhase::FinalCleanup).await?;
        
        Ok(())
    }
}
```

## Performance Monitoring

```rust
use flight_service::{PerformanceMonitor, PerformanceMetrics};

let perf_monitor = PerformanceMonitor::new();

// Monitor key metrics
let metrics = perf_monitor.get_current_metrics().await?;

println!("Performance Metrics:");
println!("  CPU Usage: {:.1}%", metrics.cpu_usage_percent);
println!("  Memory RSS: {:.1} MB", metrics.memory_rss_mb);
println!("  Axis Jitter p99: {:.3} ms", metrics.axis_jitter_p99_ms);
println!("  HID Latency p99: {:.3} ms", metrics.hid_latency_p99_ms);
println!("  Missed Ticks: {}", metrics.missed_tick_count);

// Fail if performance degrades
if metrics.axis_jitter_p99_ms > 0.5 {
    return Err(ServiceError::PerformanceRegression);
}
```

## Testing

```bash
# Run service integration tests
cargo test --package flight-service

# Run end-to-end tests with virtual devices
cargo test --package flight-service test_e2e -- --ignored

# Test graceful shutdown
cargo test --package flight-service test_shutdown_sequence

# Test safe mode operation
cargo test --package flight-service test_safe_mode
```

## Requirements

This crate satisfies multiple system requirements:

- **DM-01**: Device management and hot-plug support coordination
- **GI-01**: Multi-simulator support orchestration  
- **PRF-01**: Profile management and auto-switching
- **UX-01**: User interface and experience coordination
- **NFR-01**: Performance and resource management
- **SAFE-01**: Safety system coordination

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.