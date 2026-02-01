# Force Feedback Device Configuration Guide

This guide explains how to configure force feedback (FFB) devices with Flight Hub, including device setup, safety configuration, and tuning.

## Supported FFB Devices

Flight Hub supports force feedback devices via DirectInput on Windows. Common supported devices include:

| Device | Type | Notes |
|--------|------|-------|
| Brunner CLS-E | Yoke/Stick | Full FFB support |
| VPforce Rhino | Stick | Full FFB support |
| Moza AB9 | Stick | Full FFB support |
| Microsoft Sidewinder FFB2 | Stick | Legacy device, full support |
| Logitech G940 | HOTAS | Stick FFB only |
| Thrustmaster T.16000M FCS | Stick | No FFB (axis only) |

### Device Requirements

- **Windows**: DirectInput-compatible FFB device
- **Linux**: FFB support is experimental (via hidraw)
- **Connection**: USB 2.0 or higher (USB 3.0 recommended for lower latency)

## Initial Setup

### Step 1: Connect Your FFB Device

1. Connect your FFB device to a USB port
2. Wait for Windows to install drivers (if needed)
3. Verify the device appears in Windows Game Controllers

### Step 2: Verify Device Detection

```cmd
flightctl devices
```

You should see your FFB device listed with `[FFB]` indicator:

```
Detected Devices:
  [1] VPforce Rhino FFB [FFB]
      Vendor: 0x2341  Product: 0x8036
      Capabilities: X, Y, Z axes | FFB: Constant, Spring, Damper
  [2] Thrustmaster TWCS Throttle
      Vendor: 0x044F  Product: 0xB687
      Capabilities: X, Y, Z, Slider axes
```

### Step 3: Enable FFB in Flight Hub

```cmd
flightctl ffb enable
```

Or in the Flight Hub UI:
1. Go to **Settings** → **Force Feedback**
2. Toggle **Enable Force Feedback** to ON
3. Select your FFB device from the dropdown

## Safety Configuration

Flight Hub implements multiple safety systems to protect you and your equipment. **Read this section carefully before using FFB.**

### Safety Envelope

The Safety Envelope limits all FFB output to safe values:

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `max_torque_nm` | 15.0 Nm | 1-50 Nm | Maximum torque output |
| `max_slew_rate_nm_per_s` | 100 Nm/s | 10-500 Nm/s | Maximum rate of torque change |
| `max_jerk_nm_per_s2` | 1000 Nm/s² | 100-5000 Nm/s² | Maximum rate of slew change |

Configure via CLI:
```cmd
flightctl ffb config --max-torque 12.0 --max-slew-rate 80.0
```

Or in the Flight Hub UI:
1. Go to **Settings** → **Force Feedback** → **Safety**
2. Adjust the sliders for each parameter
3. Click **Apply**

### Safety States

Flight Hub operates in one of these safety states:

| State | Description | FFB Output |
|-------|-------------|------------|
| **SafeTorque** | Normal operation, safety limits active | Limited to envelope |
| **HighTorque** | Elevated limits (requires explicit enable) | Higher limits |
| **Faulted** | Fault detected, FFB disabled | Zero (ramping down) |
| **Disabled** | FFB manually disabled | Zero |

### Emergency Stop

Flight Hub provides an emergency stop that immediately disables FFB:

**Keyboard**: Press `Escape` (configurable)
**UI**: Click the red **EMERGENCY STOP** button
**CLI**: `flightctl ffb stop`

See [Emergency Stop Configuration](../how-to/configure-emergency-stop.md) for detailed API documentation.

### Fault Detection

Flight Hub automatically detects and responds to faults:

| Fault Type | Detection | Response |
|------------|-----------|----------|
| USB Disconnect | Device enumeration fails | Immediate ramp to zero |
| USB Stall | 3+ consecutive write failures | Ramp to zero within 12ms |
| Over-torque | Torque exceeds envelope | Clamp to limit |
| Telemetry Loss | No sim data for 500ms | Ramp to zero |
| `safe_for_ffb` False | Sim reports unsafe condition | Ramp to zero |

All faults are logged to the blackbox for diagnostics.

## FFB Tuning

### Global Strength

Adjust overall FFB strength (0-100%):

```cmd
flightctl ffb strength 75
```

This scales all FFB effects proportionally.

### Per-Effect Tuning

Flight Hub supports tuning individual FFB effects:

| Effect | Description | Default |
|--------|-------------|---------|
| **Constant** | Steady force (stick centering) | 100% |
| **Spring** | Position-proportional force | 80% |
| **Damper** | Velocity-proportional force | 60% |
| **Friction** | Static resistance | 40% |
| **Inertia** | Acceleration resistance | 50% |

Configure via CLI:
```cmd
flightctl ffb effect spring 90
flightctl ffb effect damper 50
```

### Aircraft-Specific Profiles

Create FFB profiles for different aircraft types:

```cmd
# Create a profile for heavy aircraft
flightctl profile create heavy-ffb --ffb-strength 100 --spring 90 --damper 70

# Create a profile for light aircraft
flightctl profile create light-ffb --ffb-strength 60 --spring 50 --damper 40
```

Profiles can be automatically selected based on aircraft detection.

## Telemetry Synthesis

Flight Hub can synthesize FFB effects from simulator telemetry:

### Available Effects

| Effect | Source Data | Description |
|--------|-------------|-------------|
| **Stick Force** | G-forces, airspeed | Simulates aerodynamic forces |
| **Buffet** | AoA, airspeed | Simulates stall buffet |
| **Trim** | Trim position | Simulates trim forces |
| **Ground Effect** | Altitude AGL | Simulates ground effect |

### Enable Telemetry Synthesis

```cmd
flightctl ffb synth enable
```

Configure synthesis parameters:
```cmd
flightctl ffb synth config --stick-force 80 --buffet 60 --trim 100
```

### Synthesis Requirements

Telemetry synthesis requires:
1. Active simulator connection
2. Valid telemetry data (`safe_for_ffb = true`)
3. FFB device in SafeTorque or HighTorque state

## Diagnostics

### View FFB Status

```cmd
flightctl ffb status
```

Output:
```
Force Feedback Status:
  Device: VPforce Rhino FFB
  State: SafeTorque
  Current Torque: 3.2 Nm
  Safety Envelope:
    Max Torque: 15.0 Nm
    Max Slew Rate: 100.0 Nm/s
    Max Jerk: 1000.0 Nm/s²
  Telemetry Synthesis: Enabled
  Last Fault: None
```

### View FFB Metrics

```cmd
flightctl metrics ffb
```

Output:
```
FFB Metrics (last 60s):
  Torque Output:
    Mean: 2.8 Nm
    Max: 12.1 Nm
    p99: 8.5 Nm
  Write Latency:
    Mean: 180 μs
    p99: 290 μs
  Faults: 0
  Emergency Stops: 0
```

### Blackbox Recording

FFB events are automatically recorded to the blackbox:

```cmd
# View recent FFB events
flightctl blackbox show --filter ffb --last 5m

# Export blackbox for support
flightctl blackbox export --output ffb-debug.bin
```

## Troubleshooting

### No FFB Output

**Symptoms**: Device detected but no forces felt

**Solutions**:
1. Verify FFB is enabled: `flightctl ffb status`
2. Check safety state is not Faulted or Disabled
3. Verify simulator is connected and `safe_for_ffb = true`
4. Check device in Windows Game Controllers → Properties → Test FFB
5. Try increasing global strength: `flightctl ffb strength 100`

### Weak or Inconsistent FFB

**Symptoms**: Forces feel weak or vary unexpectedly

**Solutions**:
1. Check USB connection (avoid hubs, use direct port)
2. Verify device firmware is up to date
3. Check for USB power issues (try powered hub)
4. Review safety envelope settings (may be too restrictive)
5. Check telemetry synthesis settings

### FFB Cuts Out Intermittently

**Symptoms**: FFB works then suddenly stops

**Solutions**:
1. Check for USB stall faults in blackbox
2. Verify USB cable is secure
3. Check for driver conflicts
4. Review fault log: `flightctl ffb faults`
5. Try different USB port or cable

### Device Not Detected as FFB

**Symptoms**: Device appears but without `[FFB]` indicator

**Solutions**:
1. Verify device supports DirectInput FFB
2. Check device drivers are installed correctly
3. Test FFB in Windows Game Controllers
4. Some devices require specific driver versions

For more issues, see the [Troubleshooting Guide](../how-to/troubleshoot-common-issues.md#force-feedback-issues).

## Safety Guidelines

### Before Each Session

1. **Test emergency stop** - Verify you can quickly stop FFB
2. **Check safety state** - Ensure device is in SafeTorque mode
3. **Verify limits** - Confirm safety envelope is appropriate
4. **Clear workspace** - Ensure nothing can be caught by moving controls

### During Operation

1. **Keep hands clear** - Don't grip controls too tightly
2. **Monitor forces** - Be aware of unexpected force changes
3. **Use emergency stop** - Don't hesitate if something feels wrong
4. **Watch for faults** - Pay attention to fault warnings

### Maintenance

1. **Check connections** - Regularly inspect USB cables
2. **Update firmware** - Keep device firmware current
3. **Review logs** - Periodically check blackbox for issues
4. **Test safety systems** - Verify emergency stop works

### Important Warnings

⚠️ **Never disable safety systems** - The safety envelope exists to protect you

⚠️ **Start with low strength** - Increase gradually as you become familiar

⚠️ **Keep emergency stop accessible** - Know how to stop FFB instantly

⚠️ **Don't use damaged equipment** - Inspect devices before use

⚠️ **Supervise children** - FFB devices can produce significant forces

---

**Requirements**: 17.3 (FFB device configuration guide, safety guidelines)

**Related Documentation**:
- [Emergency Stop Configuration](../how-to/configure-emergency-stop.md)
- [XInput Integration](../how-to/integrate-xinput.md)
- [XInput Limitations](../explanation/xinput-limitations.md)
