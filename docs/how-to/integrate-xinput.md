---
doc_id: DOC-HOWTO-INTEGRATE-XINPUT
kind: how-to
area: flight-ffb
status: active
links:
  requirements: ["REQ-8"]
  tasks: []
  adrs: []
---

# XInput Rumble Integration Guide

## Overview

This guide explains how XInput rumble is integrated into Flight Hub's FFB framework and how to use it in your code.

## Architecture

XInput rumble integration follows the same pattern as other FFB devices but with significant limitations:

```
Telemetry Synthesis Engine
         ↓
   Effect Generator
         ↓
   XInput Mapper ← Maps effects to rumble channels
         ↓
  XInput Device ← Sets motor intensities
```

## Usage Example

### Basic Setup

```rust
use flight_ffb::{XInputRumbleDevice, RumbleChannels};

// Create device for user index 0 (first controller)
let mut device = XInputRumbleDevice::new(0)?;

// Check if device is connected
if device.check_connection() {
    println!("XInput controller connected");
}
```

### Setting Rumble

```rust
// Set rumble intensities (0.0-1.0 for each motor)
let channels = RumbleChannels {
    low_freq: 0.5,   // 50% low-frequency motor
    high_freq: 0.7,  // 70% high-frequency motor
};

device.set_rumble(channels)?;
```

### Mapping Flight Effects

```rust
use flight_ffb::map_effects_to_rumble;

// Calculate effects from telemetry
let buffeting = calculate_buffeting_intensity(aoa, airspeed);
let engine_vib = calculate_engine_vibration(rpm);

// Map to rumble channels
let rumble = map_effects_to_rumble(buffeting, engine_vib);

// Apply to device
device.set_rumble(rumble)?;
```

### Integration with FFB Engine

```rust
use flight_ffb::{FfbEngine, FfbConfig, FfbMode};

// Configure FFB engine for telemetry synthesis
let config = FfbConfig {
    max_torque_nm: 5.0,  // Not used for XInput
    fault_timeout_ms: 50,
    interlock_required: false,
    mode: FfbMode::TelemetrySynth,  // XInput uses telemetry synthesis
    device_path: None,
};

let mut ffb_engine = FfbEngine::new(config)?;

// XInput device is managed separately from FFB engine
let mut xinput_device = XInputRumbleDevice::new(0)?;

// In main loop:
loop {
    // Update telemetry synthesis
    if let Some(effect_output) = ffb_engine.update_telemetry_synthesis(&bus_snapshot)? {
        // Map synthesized effects to XInput rumble
        let rumble = map_effects_to_rumble(
            effect_output.intensity * 0.5,  // Buffeting component
            effect_output.frequency_hz / 100.0,  // Engine vibration component
        );
        
        xinput_device.set_rumble(rumble)?;
    }
}
```

## Effect Mapping Strategy

### Buffeting (Low-Frequency Motor)

Buffeting intensity is calculated from:
- Angle of attack (increases near stall)
- Airspeed (increases at high speeds)
- G-load (increases during maneuvers)

```rust
fn calculate_buffeting(aoa: f32, airspeed: f32, g_load: f32) -> f32 {
    let aoa_factor = if aoa > 12.0 {
        ((aoa - 12.0) / 8.0).min(1.0)  // Ramp up from 12° to 20° AoA
    } else {
        0.0
    };
    
    let speed_factor = (airspeed / 200.0).min(1.0);  // Scale with airspeed
    let g_factor = ((g_load - 1.0).abs() / 3.0).min(1.0);  // Scale with g-load
    
    (aoa_factor * 0.6 + speed_factor * 0.2 + g_factor * 0.2).clamp(0.0, 1.0)
}
```

### Engine Vibration (High-Frequency Motor)

Engine vibration is calculated from:
- RPM (increases with engine speed)
- Manifold pressure (increases with power)
- Engine roughness (increases with damage or poor mixture)

```rust
fn calculate_engine_vibration(rpm: f32, manifold_pressure: f32) -> f32 {
    let rpm_factor = (rpm / 2700.0).min(1.0);  // Scale to redline
    let power_factor = (manifold_pressure / 30.0).min(1.0);  // Scale to max MP
    
    (rpm_factor * 0.7 + power_factor * 0.3).clamp(0.0, 1.0)
}
```

## Best Practices

### 1. Rate Limiting

Don't update rumble too frequently:

```rust
use std::time::{Duration, Instant};

let mut last_update = Instant::now();
let update_interval = Duration::from_millis(16);  // ~60 Hz

if last_update.elapsed() >= update_interval {
    device.set_rumble(rumble)?;
    last_update = Instant::now();
}
```

### 2. Smooth Transitions

Avoid abrupt changes in rumble intensity:

```rust
struct RumbleSmoothing {
    current: RumbleChannels,
    target: RumbleChannels,
    smoothing_factor: f32,  // 0.0-1.0, higher = faster response
}

impl RumbleSmoothing {
    fn update(&mut self, target: RumbleChannels) -> RumbleChannels {
        self.target = target;
        
        // Exponential smoothing
        self.current.low_freq += (self.target.low_freq - self.current.low_freq) * self.smoothing_factor;
        self.current.high_freq += (self.target.high_freq - self.current.high_freq) * self.smoothing_factor;
        
        self.current
    }
}
```

### 3. Battery Conservation

Reduce rumble intensity when battery is low:

```rust
// Check battery level (platform-specific)
let battery_level = get_battery_level()?;

let intensity_scale = if battery_level < 0.2 {
    0.5  // Reduce to 50% when battery < 20%
} else {
    1.0
};

let scaled_rumble = RumbleChannels {
    low_freq: rumble.low_freq * intensity_scale,
    high_freq: rumble.high_freq * intensity_scale,
};
```

### 4. User Preferences

Allow users to adjust rumble intensity:

```rust
struct RumblePreferences {
    enabled: bool,
    low_freq_scale: f32,   // 0.0-1.0
    high_freq_scale: f32,  // 0.0-1.0
}

fn apply_preferences(rumble: RumbleChannels, prefs: &RumblePreferences) -> RumbleChannels {
    if !prefs.enabled {
        return RumbleChannels::default();
    }
    
    RumbleChannels {
        low_freq: rumble.low_freq * prefs.low_freq_scale,
        high_freq: rumble.high_freq * prefs.high_freq_scale,
    }
}
```

## Limitations Reminder

Remember that XInput rumble:

- ❌ Cannot provide directional forces
- ❌ Cannot simulate spring centering
- ❌ Cannot provide resistance to movement
- ❌ Cannot model aerodynamic loads accurately
- ✅ Can only provide vibration effects

For realistic force feedback, use a DirectInput-compatible FFB device.

## Platform Support

### Windows

Full XInput support via Windows SDK:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = ["Win32_System_Threading", "Win32_Gaming"] }
```

### Linux

No native XInput support. Use SDL2 for gamepad rumble:

```toml
[dependencies]
sdl2 = { version = "0.35", features = ["use-vcpkg"] }
```

### macOS

No native XInput support. Use SDL2 or similar for gamepad rumble.

## Testing

Run XInput rumble tests:

```bash
# Run all XInput tests
cargo test --package flight-ffb xinput_rumble

# Run specific test
cargo test --package flight-ffb xinput_rumble::tests::test_rumble_clamping
```

Note: Tests run on all platforms but XInput functionality is Windows-only.

## References

- [XInput API Documentation](https://docs.microsoft.com/en-us/windows/win32/xinput/xinput-game-controller-apis-portal)
- [XInput Limitations](./xinput-limitations.md)
- Flight Hub Requirements: FFB-HID-01.5
