---
doc_id: DOC-XINPUT-LIMITATIONS
kind: explanation
area: flight-ffb
status: active
links:
  requirements: ["REQ-8"]
  tasks: []
  adrs: []
---

# XInput Rumble Integration - Limitations and Usage

## Overview

Flight Hub provides limited force feedback support for XInput-compatible controllers (Xbox controllers and similar devices) through the XInput rumble API. This document explains the significant limitations of XInput rumble compared to full force feedback devices and how Flight Hub maps effects to the available rumble motors.

## Critical Limitations

### No Directional Torque

**XInput does not support directional force feedback.** The XInput API only provides two independent vibration motors (low-frequency and high-frequency), not directional torque control. This means:

- ❌ **No stick centering forces** (spring effects)
- ❌ **No resistance to movement** (damper effects)
- ❌ **No sustained directional loads** (constant force effects)
- ❌ **No realistic control loading** based on aerodynamic forces
- ✅ **Only vibration effects** for coarse feedback

### Vibration Only

XInput rumble is fundamentally different from force feedback:

| Feature | Force Feedback (DirectInput) | XInput Rumble |
|---------|------------------------------|---------------|
| Directional torque | ✅ Yes | ❌ No |
| Spring centering | ✅ Yes | ❌ No |
| Damper resistance | ✅ Yes | ❌ No |
| Constant force | ✅ Yes | ❌ No |
| Vibration effects | ✅ Yes | ✅ Yes (only) |
| Realistic control loading | ✅ Yes | ❌ No |

### Use Cases

XInput rumble is suitable **only** for:

- ✅ Buffeting and stall warning vibration
- ✅ Engine vibration feedback
- ✅ Coarse aerodynamic effect indication
- ✅ Basic tactile feedback for events

XInput rumble is **not suitable** for:

- ❌ Realistic flight control simulation
- ❌ Accurate aerodynamic force modeling
- ❌ Professional training applications
- ❌ Any scenario requiring directional force feedback

## Motor Mapping

XInput provides two rumble motors with different characteristics:

### Low-Frequency Motor (Left Motor)

- **Frequency Range**: ~10-100 Hz
- **Characteristics**: Deep, rumbling vibration
- **Flight Hub Usage**:
  - Buffeting during high angle of attack
  - Stall warning vibration
  - Low-frequency aerodynamic effects
  - Heavy turbulence indication

### High-Frequency Motor (Right Motor)

- **Frequency Range**: ~100-300 Hz
- **Characteristics**: Sharp, buzzing vibration
- **Flight Hub Usage**:
  - Engine vibration (piston engines)
  - Fine texture effects
  - High-frequency feedback
  - Light turbulence indication

## Integration with Flight Hub

### Effect Synthesis

Flight Hub's telemetry synthesis engine generates vibration intensities based on flight conditions:

```rust
// Example: Map flight conditions to rumble
let buffeting_intensity = calculate_buffeting(aoa, airspeed);
let engine_vibration = calculate_engine_vibration(rpm, manifold_pressure);

let rumble = RumbleChannels {
    low_freq: buffeting_intensity,   // 0.0-1.0
    high_freq: engine_vibration,      // 0.0-1.0
};

xinput_device.set_rumble(rumble)?;
```

### Limitations in Practice

1. **No Stick Position Feedback**: XInput cannot provide centering forces or resistance, so the stick will feel "loose" compared to a real aircraft or a proper force feedback device.

2. **No Load Variation**: Aerodynamic loads that would normally vary with airspeed, g-force, and control deflection cannot be represented through XInput.

3. **Binary Effect Quality**: Vibration effects are either on or off (with intensity control), but cannot provide the nuanced, continuous force feedback of a proper FFB device.

4. **No Trim Simulation**: Trim forces cannot be simulated through XInput, as this requires sustained directional torque.

## Comparison with DirectInput FFB

For users considering their hardware options:

### DirectInput Force Feedback Devices

**Examples**: Logitech G940, Thrustmaster Warthog with FFB base, VKB Gunfighter with FFB

**Capabilities**:
- Full directional torque control (pitch and roll axes)
- Spring effects for centering
- Damper effects for resistance
- Constant force for sustained loads
- Periodic effects for vibration
- Realistic control loading based on flight conditions

**Flight Hub Support**: ✅ Full support with all safety features

### XInput Rumble Devices

**Examples**: Xbox One/Series controllers, Xbox-compatible gamepads

**Capabilities**:
- Two independent vibration motors only
- No directional force feedback
- No spring/damper effects
- No constant force effects

**Flight Hub Support**: ⚠️ Limited support (vibration effects only)

## Recommendations

### For Realistic Flight Simulation

If you want realistic control loading and force feedback:

1. **Use a DirectInput-compatible force feedback device** (e.g., Logitech G940, Thrustmaster FFB base)
2. XInput rumble is **not a substitute** for proper force feedback
3. Consider XInput rumble as a "better than nothing" option for basic tactile feedback

### For Casual Gaming

If you're using an Xbox controller for casual flight gaming:

1. XInput rumble can provide basic feedback for stalls and engine vibration
2. Understand that you won't get realistic control loading
3. Consider it an enhancement to visual/audio feedback, not a replacement for proper FFB

## Technical Details

### API Limitations

XInput provides only the `XInputSetState` function for rumble control:

```c
DWORD XInputSetState(
    DWORD dwUserIndex,           // Controller index (0-3)
    XINPUT_VIBRATION* pVibration // Vibration state
);

typedef struct _XINPUT_VIBRATION {
    WORD wLeftMotorSpeed;   // Low-frequency motor (0-65535)
    WORD wRightMotorSpeed;  // High-frequency motor (0-65535)
} XINPUT_VIBRATION;
```

This API provides:
- ✅ Independent control of two motors
- ✅ Intensity control (0-65535 for each motor)
- ❌ No directional information
- ❌ No force magnitude
- ❌ No effect types (spring, damper, constant force)

### Platform Support

- **Windows**: Full XInput support via Windows SDK
- **Linux**: No native XInput support (use SDL2 or similar for gamepad rumble)
- **macOS**: No native XInput support

## Conclusion

XInput rumble is a **limited fallback mechanism** for controllers without full force feedback support. It provides basic vibration effects but **cannot replace** proper force feedback devices for realistic flight simulation.

**Key Takeaway**: If you want realistic control loading and force feedback, invest in a DirectInput-compatible force feedback device. XInput rumble is suitable only for casual gaming with basic tactile feedback.

## References

- [XInput API Documentation](https://docs.microsoft.com/en-us/windows/win32/xinput/xinput-game-controller-apis-portal)
- [DirectInput Force Feedback](https://docs.microsoft.com/en-us/previous-versions/windows/desktop/ee416628(v=vs.85))
- Flight Hub Requirements: FFB-HID-01.5
