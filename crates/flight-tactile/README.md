# Flight Tactile Bridge

A rate-limited tactile feedback bridge for Flight Hub that routes flight simulation effects to SimShaker-class applications.

## Features

- **Basic Channel Routing**: Routes touchdown, rumble, and stall effects to appropriate tactile channels
- **Rate-Limited Thread**: Operates at configurable update rates (10-60 Hz) without blocking real-time systems
- **SimShaker Bridge**: Compatible with SimShaker and similar tactile feedback applications via UDP
- **User Toggle Control**: Independent enable/disable control for tactile feedback
- **Zero AX/FFB Jitter**: Designed to not interfere with real-time axis and force feedback processing
- **Effect Processing**: Generates tactile effects from normalized telemetry data

## Architecture

The tactile bridge operates independently from the real-time axis/FFB loops to prevent jitter regression:

```
Telemetry Data → Effect Processor → Channel Router → SimShaker Bridge → UDP Output
```

### Components

- **TactileManager**: Main interface for tactile feedback control
- **EffectProcessor**: Converts telemetry data into tactile effect events
- **ChannelRouter**: Routes effects to appropriate channels with gain control
- **SimShakerBridge**: UDP communication with SimShaker-class applications
- **TactileBridge**: Rate-limited processing thread

## Effect Types

The system supports the following tactile effects:

- **Touchdown**: Landing impact effects
- **GroundRoll**: Taxi/takeoff/landing rumble
- **StallBuffet**: Aerodynamic stall vibration
- **EngineVibration**: Engine-based vibration
- **GearWarning**: Landing gear warning alerts
- **RotorVibration**: Helicopter rotor effects

## Usage

```rust
use flight_tactile::{TactileManager, TactileConfig, EffectType};

// Create and configure tactile manager
let mut manager = TactileManager::new();
let config = TactileConfig::default();
manager.initialize(config)?;

// Enable tactile feedback
manager.set_enabled(true);

// Start the bridge (connects to SimShaker)
manager.start()?;

// Process telemetry data
manager.process_telemetry(&telemetry_snapshot)?;

// Test specific effects
manager.test_effect(EffectType::Touchdown, 0.8)?;

// Get statistics
if let Some(stats) = manager.get_stats() {
    println!("Effects generated: {}", stats.effects_generated);
}
```

## Configuration

The tactile bridge can be configured for different SimShaker setups:

```rust
let mut config = TactileConfig::default();

// SimShaker connection
config.simshaker.target_address = "127.0.0.1".to_string();
config.simshaker.target_port = 4123;
config.simshaker.update_rate_hz = 60.0;

// Channel mapping
config.channel_mapping.set_mapping(EffectType::Touchdown, ChannelId::new(0));
config.channel_mapping.set_channel_gain(ChannelId::new(0), 0.8);

// Effect enable/disable
config.effect_enabled.insert(EffectType::StallBuffet, false);
```

## Performance

The tactile bridge is designed for minimal performance impact:

- **Processing Time**: <10μs per telemetry snapshot
- **Memory Usage**: <1MB additional RSS
- **Thread Isolation**: Runs on separate thread from RT loops
- **Rate Limiting**: Configurable update rates prevent system overload

## SimShaker Protocol

The bridge implements a simple UDP protocol compatible with SimShaker:

```
Packet Structure:
- Header: 4 bytes ("SHKR" magic)
- Channels: 8 bytes (0-255 intensity per channel)
- Sequence: 4 bytes (packet counter)
- Checksum: 2 bytes (validation)
```

## Testing

Run the test suite:

```bash
cargo test -p flight-tactile
```

Run performance tests:

```bash
cargo test -p flight-tactile --test performance_test
```

Run the demo:

```bash
cargo run -p flight-tactile --example tactile_demo
```

## Requirements

The tactile bridge satisfies the following requirements from PNL-01:

- ✅ Basic channel routing for touchdown/rumble/stall effects
- ✅ Rate-limited thread with SimShaker-class app bridge  
- ✅ User toggle functionality for independent control
- ✅ Verified no AX/FFB jitter regression through comprehensive testing

## Integration

The tactile bridge integrates with the Flight Hub ecosystem:

- **flight-bus**: Consumes normalized telemetry data
- **flight-core**: Uses shared error types and configuration patterns
- **flight-panels**: Similar architecture for external device integration

## Safety

The tactile bridge operates safely alongside real-time systems:

- **Thread Isolation**: Separate thread prevents RT interference
- **Error Handling**: Network failures don't affect flight systems
- **Graceful Degradation**: Continues operation if SimShaker unavailable
- **Resource Limits**: Bounded queues prevent memory exhaustion