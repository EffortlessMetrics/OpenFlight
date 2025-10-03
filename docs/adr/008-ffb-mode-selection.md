# ADR-008: Force Feedback Mode Selection

## Status
Accepted

## Context

Force feedback devices have varying capabilities and simulators provide different levels of FFB support. Some devices support DirectInput effects, others provide raw torque control, and some require telemetry-based synthesis. The system needs intelligent mode selection that maximizes fidelity while ensuring safety and compatibility.

## Decision

We implement a capability-based FFB mode selection system with automatic negotiation:

### 1. FFB Mode Hierarchy (Preference Order)

1. **DirectInput Pass-Through** - Forward simulator PID effects to device
2. **Raw Torque Control** - Host-computed torque via OFP-1 protocol  
3. **Telemetry Synthesis** - Generate effects from normalized telemetry

### 2. Device Capability Detection

```rust
pub struct DeviceCapabilities {
    pub supports_pid: bool,           // DirectInput PID effects
    pub supports_raw_torque: bool,    // OFP-1 raw torque protocol
    pub max_torque_nm: f32,          // Maximum torque output
    pub min_period_us: u32,          // Minimum update period
    pub has_health_stream: bool,     // Real-time health monitoring
    pub safety_features: SafetyFeatures,
}

pub struct SafetyFeatures {
    pub hardware_limits: bool,        // Built-in torque limiting
    pub thermal_protection: bool,     // Over-temperature shutdown
    pub current_limiting: bool,       // Over-current protection
    pub emergency_stop: bool,         // Physical emergency stop
}
```

### 3. Mode Selection Matrix

| Simulator | Device Type | Preferred Mode | Fallback |
|-----------|-------------|----------------|----------|
| MSFS      | PID-capable | DirectInput    | Telemetry |
| MSFS      | Raw-capable | Raw Torque     | Telemetry |
| X-Plane   | Any         | Raw Torque     | Telemetry |
| DCS       | Any         | Raw Torque     | Telemetry |

### 4. Mode Negotiation Process

```rust
impl FfbModeSelector {
    pub fn select_mode(&self, device: &Device, sim: SimId) -> FfbMode {
        match (sim, &device.capabilities) {
            // MSFS with rich PID support
            (SimId::MSFS, caps) if caps.supports_pid && sim.has_rich_ffb() => {
                FfbMode::DirectInput
            },
            
            // Raw torque for advanced devices
            (_, caps) if caps.supports_raw_torque => {
                FfbMode::RawTorque {
                    frequency_hz: caps.optimal_frequency(),
                    max_torque: caps.max_torque_nm * 0.8, // 80% safety margin
                }
            },
            
            // Fallback to telemetry synthesis
            _ => FfbMode::TelemetrySynth {
                effects: self.select_effects_for_aircraft(sim.current_aircraft()),
            }
        }
    }
}
```

## Consequences

### Positive
- Optimal fidelity for each device/sim combination
- Automatic adaptation to device capabilities
- Clear fallback path ensures compatibility
- Safety margins built into selection logic

### Negative
- Complex negotiation logic
- Mode-specific code paths to maintain
- Potential user confusion about active mode
- Testing complexity across combinations

## Alternatives Considered

1. **Single Mode**: Rejected due to varying device capabilities
2. **User Selection**: Rejected due to complexity for end users
3. **Simulator-Driven**: Rejected due to inconsistent sim support
4. **Static Configuration**: Rejected due to inflexibility

## Implementation Details

### DirectInput Pass-Through Mode

```rust
impl DirectInputMode {
    pub fn forward_effect(&mut self, effect: &PidEffect) -> Result<()> {
        // Validate effect parameters
        self.validate_effect_safety(effect)?;
        
        // Apply safety scaling
        let scaled_effect = self.apply_safety_scaling(effect);
        
        // Forward to device
        self.device.send_pid_effect(scaled_effect)
    }
}
```

### Raw Torque Mode (OFP-1)

```rust
impl RawTorqueMode {
    pub fn update_torque(&mut self, torque_nm: f32) -> Result<()> {
        // Apply rate and jerk limiting
        let limited_torque = self.apply_rate_limits(torque_nm);
        
        // Safety clamp
        let safe_torque = limited_torque.clamp(-self.max_torque, self.max_torque);
        
        // Send via OFP-1 protocol
        self.device.send_raw_torque(safe_torque)
    }
}
```

### Telemetry Synthesis Mode

```rust
impl TelemetrySynthMode {
    pub fn synthesize_effects(&mut self, telemetry: &BusSnapshot) -> Vec<Effect> {
        let mut effects = Vec::new();
        
        // Stall buffet based on AoA
        if telemetry.aoa > self.aircraft_config.alpha_warn {
            effects.push(self.generate_stall_buffet(telemetry));
        }
        
        // Ground roll vibration
        if telemetry.on_ground && telemetry.ground_speed > 10.0 {
            effects.push(self.generate_ground_roll(telemetry));
        }
        
        // Gear warning
        if self.should_warn_gear(telemetry) {
            effects.push(self.generate_gear_warning());
        }
        
        effects
    }
}
```

## Safety Integration

### Mode-Specific Safety

- **DirectInput**: Effect parameter validation and scaling
- **Raw Torque**: Rate/jerk limiting, absolute torque clamps
- **Telemetry**: Effect magnitude limiting, frequency bounds

### Universal Safety

- Physical interlock required for high-torque modes
- Fault detection triggers immediate torque cutoff
- Health monitoring across all modes
- Emergency stop functionality

## User Interface

### Mode Display
- Clear indication of active FFB mode
- Explanation of why mode was selected
- Manual override option for advanced users
- Performance metrics per mode

### Configuration
- Per-aircraft mode preferences
- Safety parameter adjustment
- Effect tuning for telemetry synthesis
- Diagnostic information

## Testing Strategy

### Unit Tests
- Mode selection logic with various device/sim combinations
- Safety parameter validation
- Effect generation algorithms

### Integration Tests
- Mode switching during operation
- Device capability detection
- Cross-mode compatibility

### Hardware-in-Loop Tests
- Actual device testing with each mode
- Safety system validation
- Performance measurement
- User acceptance testing

## Performance Considerations

- Mode selection happens at device connection
- Runtime overhead minimal for each mode
- Effect computation off RT thread where possible
- Memory usage bounded by effect complexity

## References

- Flight Hub Requirements: FFB-01, SAFE-01
- [DirectInput Force Feedback](https://example.com)
- [OFP-1 Protocol Specification](https://example.com)
- [Force Feedback Safety Standards](https://example.com)