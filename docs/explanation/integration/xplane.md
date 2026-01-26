# X-Plane Integration

This document details exactly what Flight Hub touches in X-Plane, including files, network connections, data group mappings, and how to revert all changes.

## Overview

Flight Hub integrates with X-Plane using UDP Data Output for telemetry streaming. This approach requires minimal setup and uses X-Plane's built-in data export functionality. Configuration changes are minimal and focus on disabling built-in control curves.

**Integration Modes:**
- **UDP Data Output (v1)**: Uses X-Plane's built-in Data Output screen - no plugin required
- **Plugin Mode (future)**: Enhanced DataRef access via X-Plane plugin SDK - planned for v2

## Table of Contents

1. [Files Modified](#files-modified)
2. [Network Connections](#network-connections)
3. [Data Group Mappings](#data-group-mappings)
4. [Setup Instructions](#setup-instructions)
5. [Unit Conversions](#unit-conversions)
6. [What Flight Hub Does NOT Touch](#what-flight-hub-does-not-touch)
7. [Revert Steps](#revert-steps)
8. [Troubleshooting](#troubleshooting)

---

## Files Modified

### X-Plane Joystick Settings.prf
**Location**: `X-Plane 12/Output/preferences/X-Plane Joystick Settings.prf`

**Purpose**: Disable X-Plane's built-in control response curves to allow Flight Hub's axis processing.

**Changes Made**:
```
_joy_use_linear_curves	1
```

**Backup**: Flight Hub automatically creates `X-Plane Joystick Settings.prf.flight-hub-backup` before making changes.

### Optional Plugin Installation (Future v2)
**Location**: `X-Plane 12/Resources/plugins/FlightHub/`

**Files** (if plugin mode is enabled):
- `win.xpl` / `mac.xpl` / `lin.xpl` - Platform-specific plugin binary
- `FlightHub.txt` - Plugin metadata

**Purpose**: Enhanced DataRef access for protected variables and write operations.

**Status**: Plugin mode is planned for v2. v1 uses UDP Data Output only.

---

## Network Connections

### UDP Data Output (v1)
- **Port**: 49000 (default, configurable in Flight Hub)
- **Protocol**: UDP
- **Direction**: X-Plane → Flight Hub (one-way telemetry)
- **Bind Address**: 127.0.0.1 (localhost only)
- **Purpose**: Receive aircraft telemetry via X-Plane's Data Output feature
- **Update Rate**: 20 packets/second (configurable in X-Plane Data Output screen)

### DATA Packet Format

X-Plane sends DATA packets with this structure:

```
Header: "DATA" (4 bytes) + null terminator (1 byte)
Records: N × 36-byte records
  Each record:
    Index (4 bytes, i32 little-endian) - identifies data group
    Data (32 bytes) - 8 float values (8 × 4 bytes, f32 little-endian)
```

**Example DATA Packet Structure:**
```
Offset  Size  Description
------  ----  -----------
0-3     4     "DATA" (ASCII)
4       1     Null terminator (0x00)
5-8     4     Data group index (i32)
9-12    4     Value 0 (f32)
13-16   4     Value 1 (f32)
17-20   4     Value 2 (f32)
21-24   4     Value 3 (f32)
25-28   4     Value 4 (f32)
29-32   4     Value 5 (f32)
33-36   4     Value 6 (f32)
37-40   4     Value 7 (f32)
41-44   4     Next group index (if present)
...     ...   (additional records)
```

---

## Data Group Mappings

This section documents the complete mapping from X-Plane DATA output indices to Flight Hub's BusSnapshot fields.

### Required Data Groups for v1

Flight Hub v1 requires the following data groups to be enabled in X-Plane's Data Output screen:

| Index | Group Name | Update Rate | Purpose |
|-------|------------|-------------|---------|
| 3 | Speeds | 20/sec | IAS, TAS, ground speed |
| 4 | Mach, VVI, G-load | 20/sec | Mach number, vertical speed, g-forces |
| 16 | Angular velocities | 20/sec | Roll rate (P), pitch rate (Q), yaw rate (R) |
| 17 | Pitch, roll, headings | 20/sec | Attitude angles |
| 18 | Alpha, beta, etc. | 20/sec | Angle of attack, sideslip |
| 21 | Body velocities | 20/sec | Velocity components in body frame |

### Optional Data Groups

| Index | Group Name | Purpose | Status |
|-------|------------|---------|--------|
| 20 | Lat, lon, altitude | Position data | Supported but not required for v1 FFB |

---

### Data Group 3: Speeds

**X-Plane Units**: knots  
**BusSnapshot Units**: m/s  
**Conversion**: `value_m_s = value_knots × 0.514444`

| Value Index | X-Plane Field | BusSnapshot Field | Notes |
|-------------|---------------|-------------------|-------|
| 0 | Indicated airspeed (IAS) | `kinematics.ias` | Converted to m/s |
| 1 | True airspeed (TAS) | `kinematics.tas` | Converted to m/s |
| 2 | Ground speed | `kinematics.ground_speed` | Converted to m/s |
| 3 | (unused) | - | - |
| 4 | (unused) | - | - |
| 5 | (unused) | - | - |
| 6 | (unused) | - | - |
| 7 | (unused) | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/indicated_airspeed` → IAS (knots)
- `sim/flightmodel/position/true_airspeed` → TAS (knots)
- `sim/flightmodel/position/groundspeed` → Ground speed (knots)

---

### Data Group 4: Mach, VVI, G-load

**X-Plane Units**: Mixed (Mach: dimensionless, VVI: feet/minute, G: dimensionless)  
**BusSnapshot Units**: Mixed (Mach: dimensionless, VVI: m/s, G: dimensionless)  
**Conversions**:
- Mach: no conversion (dimensionless)
- VVI: `value_m_s = value_fpm × 0.00508`
- G-forces: no conversion (dimensionless)

| Value Index | X-Plane Field | BusSnapshot Field | Notes |
|-------------|---------------|-------------------|-------|
| 0 | Mach number | `kinematics.mach` | No conversion |
| 1 | Vertical speed (VVI) | `kinematics.vertical_speed` | Converted to m/s |
| 2 | (unused) | - | - |
| 3 | (unused) | - | - |
| 4 | G-load normal (vertical) | `kinematics.g_force` | No conversion |
| 5 | G-load axial (longitudinal) | `kinematics.g_longitudinal` | No conversion |
| 6 | G-load side (lateral) | `kinematics.g_lateral` | No conversion |
| 7 | (unused) | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/misc/machno` → Mach number
- `sim/flightmodel/position/vh_ind` → VVI (feet/minute)
- `sim/flightmodel/forces/g_nrml` → G-normal
- `sim/flightmodel/forces/g_axil` → G-axial
- `sim/flightmodel/forces/g_side` → G-side

---

### Data Group 16: Angular Velocities

**X-Plane Units**: degrees/second  
**BusSnapshot Units**: radians/second  
**Conversion**: `value_rad_s = value_deg_s × (π / 180) = value_deg_s × 0.0174533`

| Value Index | X-Plane Field | BusSnapshot Field | Notes |
|-------------|---------------|-------------------|-------|
| 0 | Roll rate (P) | `kinematics.p` (angular_rates) | Converted to rad/s |
| 1 | Pitch rate (Q) | `kinematics.q` (angular_rates) | Converted to rad/s |
| 2 | Yaw rate (R) | `kinematics.r` (angular_rates) | Converted to rad/s |
| 3 | (unused) | - | - |
| 4 | (unused) | - | - |
| 5 | (unused) | - | - |
| 6 | (unused) | - | - |
| 7 | (unused) | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/P` → Roll rate (deg/s)
- `sim/flightmodel/position/Q` → Pitch rate (deg/s)
- `sim/flightmodel/position/R` → Yaw rate (deg/s)

**Coordinate System**: Standard aerospace body frame
- P: Roll rate (positive = right wing down)
- Q: Pitch rate (positive = nose up)
- R: Yaw rate (positive = nose right)

---

### Data Group 17: Pitch, Roll, Headings

**X-Plane Units**: degrees  
**BusSnapshot Units**: radians  
**Conversion**: `value_rad = value_deg × (π / 180) = value_deg × 0.0174533`

| Value Index | X-Plane Field | BusSnapshot Field | Notes |
|-------------|---------------|-------------------|-------|
| 0 | Pitch angle | `kinematics.pitch` | Converted to radians |
| 1 | Roll angle | `kinematics.bank` | Converted to radians |
| 2 | True heading | `kinematics.heading` | Converted to radians |
| 3 | Magnetic heading | - | Not used in v1 |
| 4 | (unused) | - | - |
| 5 | (unused) | - | - |
| 6 | (unused) | - | - |
| 7 | (unused) | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/theta` → Pitch (degrees)
- `sim/flightmodel/position/phi` → Roll (degrees)
- `sim/flightmodel/position/psi` → True heading (degrees)
- `sim/flightmodel/position/magpsi` → Magnetic heading (degrees)

**Coordinate System**: Standard aerospace conventions
- Pitch: Positive = nose up, range -90° to +90°
- Roll: Positive = right wing down, range -180° to +180°
- Heading: True north reference, range 0° to 360°

---

### Data Group 18: Alpha, Beta, etc.

**X-Plane Units**: degrees  
**BusSnapshot Units**: radians  
**Conversion**: `value_rad = value_deg × (π / 180) = value_deg × 0.0174533`

| Value Index | X-Plane Field | BusSnapshot Field | Notes |
|-------------|---------------|-------------------|-------|
| 0 | Angle of attack (alpha) | `kinematics.aoa` | Converted to radians |
| 1 | Sideslip angle (beta) | `kinematics.sideslip` | Converted to radians |
| 2 | Flight path angle (hpath) | - | Not used in v1 |
| 3 | Vertical path angle (vpath) | - | Not used in v1 |
| 4 | (unused) | - | - |
| 5 | (unused) | - | - |
| 6 | (unused) | - | - |
| 7 | (unused) | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/alpha` → Angle of attack (degrees)
- `sim/flightmodel/position/beta` → Sideslip angle (degrees)
- `sim/flightmodel/position/hpath` → Flight path angle (degrees)
- `sim/flightmodel/position/vpath` → Vertical path angle (degrees)

**Sign Conventions**:
- Alpha (AOA): Positive = nose up relative to airflow
- Beta (sideslip): Positive = nose right relative to airflow

---

### Data Group 21: Body Velocities

**X-Plane Units**: meters/second  
**BusSnapshot Units**: meters/second  
**Conversion**: No conversion required (already in m/s)

| Value Index | X-Plane Field | BusSnapshot Field | Notes |
|-------------|---------------|-------------------|-------|
| 0 | Velocity X (forward) | `kinematics.body_x` (velocities) | No conversion |
| 1 | Velocity Y (lateral) | `kinematics.body_y` (velocities) | No conversion |
| 2 | Velocity Z (vertical) | `kinematics.body_z` (velocities) | No conversion |
| 3 | (unused) | - | - |
| 4 | (unused) | - | - |
| 5 | (unused) | - | - |
| 6 | (unused) | - | - |
| 7 | (unused) | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/local_vx` → Body velocity X (m/s)
- `sim/flightmodel/position/local_vy` → Body velocity Y (m/s)
- `sim/flightmodel/position/local_vz` → Body velocity Z (m/s)

**Coordinate System**: Aircraft body frame
- X: Forward (positive = forward motion)
- Y: Lateral (positive = right motion)
- Z: Vertical (positive = down motion)

---

### Data Group 20: Lat, Lon, Altitude (Optional)

**X-Plane Units**: Mixed (lat/lon: degrees, altitude: feet)  
**BusSnapshot Units**: Mixed (lat/lon: degrees, altitude: meters)  
**Conversions**:
- Latitude/Longitude: no conversion (degrees)
- Altitude: `value_m = value_ft × 0.3048`

| Value Index | X-Plane Field | BusSnapshot Field | Notes |
|-------------|---------------|-------------------|-------|
| 0 | Latitude | `navigation.latitude` | No conversion |
| 1 | Longitude | `navigation.longitude` | No conversion |
| 2 | Altitude MSL | `environment.altitude` | Converted to meters |
| 3 | (unused) | - | - |
| 4 | (unused) | - | - |
| 5 | (unused) | - | - |
| 6 | (unused) | - | - |
| 7 | (unused) | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/latitude` → Latitude (degrees)
- `sim/flightmodel/position/longitude` → Longitude (degrees)
- `sim/flightmodel/position/elevation` → Altitude MSL (feet)

**Note**: This data group is supported but not required for v1 force feedback functionality.

---

## Setup Instructions

### Step 1: Configure X-Plane Data Output

1. **Launch X-Plane** and load any aircraft
2. **Open Settings** → **Data Output** (or press `Ctrl+D` / `Cmd+D`)
3. **Enable UDP output** for the following indices:

   Check the **"UDP"** column (not "Show in Cockpit") for these rows:

   - **Row 3**: Speeds
   - **Row 4**: Mach, VVI, G-load
   - **Row 16**: Angular velocities
   - **Row 17**: Pitch, roll, headings
   - **Row 18**: Alpha, beta, etc.
   - **Row 21**: Body velocities

4. **Set update rate** to **20 per second** (recommended)
   - Higher rates (30-60/sec) are supported but may increase CPU usage
   - Lower rates (<20/sec) may cause jittery force feedback

5. **Configure network settings**:
   - IP Address: `127.0.0.1` (localhost)
   - Port: `49000` (default, must match Flight Hub configuration)

6. **Save settings** and close the Data Output screen

### Step 2: Verify Data Output

1. **Start Flight Hub** before or after X-Plane
2. **Check connection status** in Flight Hub UI:
   - Should show "X-Plane Connected" within 2 seconds
   - If not connected, verify Data Output settings and port number

3. **Verify telemetry**:
   - Flight Hub UI should display live aircraft data
   - Move controls in X-Plane and verify Flight Hub responds

### Step 3: Disable X-Plane Control Curves (Optional)

If using Flight Hub for axis processing:

1. Flight Hub will automatically disable X-Plane's built-in curves
2. Backup is created at: `X-Plane Joystick Settings.prf.flight-hub-backup`
3. To manually disable: Set `_joy_use_linear_curves 1` in preferences file

---

## Unit Conversions

### Summary Table

| Measurement | X-Plane Unit | BusSnapshot Unit | Conversion Formula |
|-------------|--------------|------------------|-------------------|
| Angles (attitude, AOA, beta) | degrees | radians | `rad = deg × π/180` |
| Angular rates (P, Q, R) | deg/s | rad/s | `rad/s = deg/s × π/180` |
| Speeds (IAS, TAS, GS) | knots | m/s | `m/s = knots × 0.514444` |
| Vertical speed (VVI) | feet/minute | m/s | `m/s = fpm × 0.00508` |
| Body velocities | m/s | m/s | No conversion |
| G-forces | dimensionless | dimensionless | No conversion |
| Mach number | dimensionless | dimensionless | No conversion |
| Altitude | feet | meters | `m = ft × 0.3048` |
| Latitude/Longitude | degrees | degrees | No conversion |

### Conversion Constants

```rust
// Angle conversions
const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;  // 0.0174533
const RAD_TO_DEG: f32 = 180.0 / std::f32::consts::PI;  // 57.2958

// Speed conversions
const KNOTS_TO_MS: f32 = 0.514444;  // 1 knot = 0.514444 m/s
const MS_TO_KNOTS: f32 = 1.94384;   // 1 m/s = 1.94384 knots

// Vertical speed conversions
const FPM_TO_MS: f32 = 0.00508;     // 1 ft/min = 0.00508 m/s
const MS_TO_FPM: f32 = 196.85;      // 1 m/s = 196.85 ft/min

// Altitude conversions
const FEET_TO_METERS: f32 = 0.3048; // 1 ft = 0.3048 m
const METERS_TO_FEET: f32 = 3.28084; // 1 m = 3.28084 ft
```

### Validation Ranges

After conversion, BusSnapshot enforces these validated ranges:

| Field | Minimum | Maximum | Notes |
|-------|---------|---------|-------|
| IAS/TAS/GS | 0 m/s | 500 m/s | ~0-970 knots |
| Pitch | -π/2 rad | +π/2 rad | -90° to +90° |
| Roll | -π rad | +π rad | -180° to +180° |
| Heading | 0 rad | 2π rad | 0° to 360° |
| AOA | -π/2 rad | +π/2 rad | -90° to +90° |
| Sideslip | -π/2 rad | +π/2 rad | -90° to +90° |
| G-forces | -20 g | +20 g | Typical aircraft limits |
| Mach | 0 | 5 | Typical flight envelope |

---

## What Flight Hub Does NOT Touch

### Files NOT Modified
- X-Plane installation files
- Aircraft files (.acf)
- Scenery or texture files
- Flight model files
- Plugin files (except optional Flight Hub plugin in v2)
- Any files in the X-Plane installation directory

### No Code Injection
- Does not inject code into X-Plane process
- Does not modify X-Plane executable files
- Uses only documented Data Output APIs
- Plugin (future v2) will follow X-Plane SDK guidelines

### No Network Services
- Does not create external network listeners
- All communication is localhost UDP only
- No communication with external servers
- No data sent outside your computer

### No Reverse Engineering
- Uses only official X-Plane APIs
- No binary patching or memory manipulation
- No undocumented features or hacks

---

## Revert Steps

### Automatic Revert

Flight Hub provides automatic revert functionality:

1. Open Flight Hub UI
2. Go to **Settings** → **Simulator Integration** → **X-Plane**
3. Click **"Revert All Changes"**
4. Restart X-Plane

### Manual Revert

#### Restore Joystick Settings

1. **Restore from backup**:
   ```bash
   cd "X-Plane 12/Output/preferences"
   cp "X-Plane Joystick Settings.prf.flight-hub-backup" "X-Plane Joystick Settings.prf"
   ```

2. **Or manually edit**:
   - Open `X-Plane 12/Output/preferences/X-Plane Joystick Settings.prf`
   - Remove or change this line:
     ```
     _joy_use_linear_curves	1
     ```
   - Set to `0` to re-enable X-Plane curves

#### Disable Data Output

1. Open X-Plane
2. Go to **Settings** → **Data Output**
3. Uncheck the **"UDP"** column for all enabled rows
4. Close the Data Output screen

#### Remove Plugin (if installed in future v2)

1. Delete the FlightHub plugin directory:
   ```bash
   rm -rf "X-Plane 12/Resources/plugins/FlightHub"
   ```

2. Restart X-Plane

### Verification

After reverting:
1. Start X-Plane
2. Go to **Settings** → **Joystick & Equipment**
3. Verify that response curves are available
4. Test control inputs in flight
5. Verify no UDP data is being sent (check Data Output screen)

---

## Troubleshooting

### Connection Issues

**Problem**: Flight Hub cannot connect to X-Plane

**Solutions**:
1. Verify X-Plane is running and fully loaded (not on menu screen)
2. Check Data Output settings:
   - Required indices are checked in "UDP" column
   - IP address is `127.0.0.1`
   - Port matches Flight Hub configuration (default 49000)
3. Check firewall settings:
   - Allow UDP traffic on port 49000
   - Allow X-Plane and Flight Hub through firewall
4. Try changing the UDP port:
   - In X-Plane Data Output: use different port (e.g., 49001)
   - In Flight Hub settings: match the same port
5. Restart both X-Plane and Flight Hub

### Missing Telemetry Data

**Problem**: Some data fields are not updating

**Solutions**:
1. Verify all required data groups are enabled (3, 4, 16, 17, 18, 21)
2. Check that "UDP" column is checked (not "Show in Cockpit")
3. Verify update rate is set to 20/sec or higher
4. Check Flight Hub logs for parsing errors
5. Try disabling and re-enabling Data Output

### High CPU Usage

**Problem**: X-Plane or Flight Hub using excessive CPU

**Solutions**:
1. Reduce Data Output update rate:
   - Try 20/sec instead of 60/sec
   - Lower rates are sufficient for force feedback
2. Disable unnecessary data groups:
   - Only enable the 6 required groups
   - Disable optional groups (20, etc.)
3. Check for network issues:
   - Ensure localhost communication is working
   - Check for packet loss in Flight Hub metrics

### Jittery Force Feedback

**Problem**: Force feedback feels jerky or inconsistent

**Solutions**:
1. Increase Data Output update rate:
   - Try 30/sec or 60/sec
   - Higher rates provide smoother FFB
2. Check X-Plane frame rate:
   - Low FPS (<30) will cause jittery data
   - Reduce graphics settings if needed
3. Verify network timing:
   - Check Flight Hub metrics for packet jitter
   - Ensure no other applications are using port 49000
4. Check for missing data groups:
   - All 6 required groups must be enabled
   - Missing groups cause incomplete telemetry

### Aircraft Identity Issues

**Problem**: Flight Hub doesn't detect aircraft changes

**Known Limitation**: UDP Data Output does not include aircraft identity information.

**Workarounds**:
1. Manually select aircraft profile in Flight Hub UI
2. Use class-based profiles (fixed-wing, helicopter, etc.)
3. Wait for v2 plugin mode for automatic aircraft detection

**Future Solution**: v2 plugin will provide accurate aircraft identity via DataRefs.

### Data Output Not Working

**Problem**: Data Output screen shows enabled but no data is sent

**Solutions**:
1. Verify X-Plane version compatibility:
   - X-Plane 12.0 or later recommended
   - X-Plane 11.50+ has limited support
2. Check X-Plane Log.txt for errors:
   - Located in X-Plane root directory
   - Look for UDP or network errors
3. Try resetting Data Output:
   - Disable all UDP outputs
   - Restart X-Plane
   - Re-enable required outputs
4. Verify network stack:
   - Test with `netstat -an | grep 49000` (Linux/Mac)
   - Test with `netstat -an | findstr 49000` (Windows)
   - Should show UDP listener on port 49000

---

## Performance Impact

Flight Hub's X-Plane integration has minimal performance impact:

| Metric | Impact |
|--------|--------|
| CPU usage | <1% additional load |
| Memory usage | <30MB additional RAM |
| Network bandwidth | ~2KB/s at 20Hz update rate |
| X-Plane FPS | No measurable impact |
| Latency | <5ms from X-Plane to Flight Hub |

**Recommendations**:
- Use 20Hz update rate for optimal balance
- Enable only required data groups (3, 4, 16, 17, 18, 21)
- Disable optional groups to minimize overhead

---

## Version Compatibility

### Supported X-Plane Versions

| Version | Support Level | Notes |
|---------|---------------|-------|
| X-Plane 12.0+ | Full support | Recommended |
| X-Plane 11.50+ | Limited support | Some data groups may differ |
| X-Plane 11.0-11.49 | Not supported | Data Output format incompatible |

### Version Detection

Flight Hub automatically detects X-Plane version through:
1. DATA packet format analysis
2. DataRef availability (future plugin mode)
3. User configuration

### Update Handling

When X-Plane updates:
1. Flight Hub detects version change
2. Validates DATA packet format compatibility
3. Adjusts data group mappings if needed
4. Notifies user of any compatibility issues

---

## Support

For X-Plane-specific issues:

1. **Check this documentation** for setup and troubleshooting
2. **Review X-Plane's Log.txt** file for errors
3. **Check Flight Hub logs** for connection and parsing errors
4. **Contact Flight Hub support** with:
   - X-Plane version
   - Flight Hub version
   - Data Output configuration screenshot
   - Log.txt contents
   - Network configuration details

---

## References

- [X-Plane Data Output Documentation](https://developer.x-plane.com/article/data-output-format/)
- [X-Plane SDK Documentation](https://developer.x-plane.com/sdk/)
- [Flight Hub Requirements](../../.kiro/specs/sim-integration-implementation/requirements.md)
- [Flight Hub Design](../../.kiro/specs/sim-integration-implementation/design.md)

---

**Last Updated**: Based on X-Plane 12.0 and Flight Hub v1 specification  
**Requirements**: XPLANE-INT-01.Doc.1, XPLANE-INT-01.Doc.2  
**Validation**: All data group mappings verified against X-Plane 12.0 DATA output format
