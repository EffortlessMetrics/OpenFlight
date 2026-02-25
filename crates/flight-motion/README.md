# flight-motion

6DOF motion platform support for OpenFlight.

## Overview

This crate implements the motion platform pipeline:

```
BusSnapshot (kinematics, angular rates)
       │
       ▼
  MotionMapper
  ├── Translational: g_longitudinal / g_lateral / g_force → high-pass washout
  └── Angular: bank / pitch (low-pass tilt) + yaw rate (high-pass onset)
       │
       ▼
  MotionFrame (-1.0 to +1.0 per DoF)
       │
       ▼
  SimToolsUdpOutput → SimTools / SFX-100 / D-Box / any compatible motion software
```

## Channel Mapping

| DoF   | Source                          | Filter    | Description                         |
|-------|---------------------------------|-----------|-------------------------------------|
| Surge | `kinematics.g_longitudinal`     | High-pass | Forward/back onset cue              |
| Sway  | `kinematics.g_lateral`          | High-pass | Left/right onset cue                |
| Heave | `kinematics.g_force` − 1G       | High-pass | Vertical onset cue (gravity removed)|
| Roll  | `kinematics.bank`               | Low-pass  | Sustained tilt                      |
| Pitch | `kinematics.pitch`              | Low-pass  | Sustained tilt                      |
| Yaw   | `angular_rates.r` (rad/s)       | High-pass | Rotation onset cue                  |

## Washout Filter

The classic washout algorithm is used to prevent platform saturation:

- **Translational channels** (surge/sway/heave) use a **high-pass filter** to provide
  transient acceleration onset cues. The platform gradually returns to neutral as the
  acceleration becomes sustained.

- **Angular channels** (roll/pitch) use a **low-pass filter** to follow sustained
  attitude changes, providing tilt cues for sustained g-loading.

- **Yaw** uses a high-pass filter on yaw rate to cue rotation onset.

The corner frequencies are configurable via `WashoutConfig`:
- `hp_frequency_hz` (default: 0.5 Hz) — translational washout rate
- `lp_frequency_hz` (default: 5.0 Hz) — angular smoothing cutoff

## Quick Start

```rust
use flight_motion::{MotionConfig, MotionMapper};
use flight_bus::BusSnapshot;

let config = MotionConfig::default();
let mut mapper = MotionMapper::new(config, 1.0 / 60.0);  // 60 Hz update rate

// In your update loop:
let snapshot = BusSnapshot::default();  // from flight-bus
let frame = mapper.process(&snapshot);
println!("{}", frame.to_simtools_string());  // e.g. "A12B-5C8D0E0F0\n"
```

## SimTools UDP Output

```rust
use flight_motion::output::{SimToolsUdpOutput, SimToolsConfig};
use flight_motion::MotionFrame;

let config = SimToolsConfig::default();  // remote: 127.0.0.1:4123
let mut output = SimToolsUdpOutput::bind(config).await?;
output.send(frame).await?;
```

## Configuration

```toml
[motion]
intensity = 0.8          # Global scale (0.0–1.0)
max_g = 3.0              # G-force → full excursion
max_angle_deg = 30.0     # Degrees → full tilt

[motion.washout]
hp_frequency_hz = 0.5    # Translational washout corner
lp_frequency_hz = 5.0    # Angular smoothing corner

[motion.roll]
enabled = true
gain = 1.0
invert = false
```

## Output Protocol

SimTools-compatible UDP format:

```
A{surge}B{sway}C{heave}D{roll}E{pitch}F{yaw}\n
```

Values are integers in **-100..100** representing platform excursion percentage.
Default port: **4123** (SimTools default).
