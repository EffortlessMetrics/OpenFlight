# X-Plane DATA Group → BusSnapshot Field Mapping

This document provides the complete mapping table between X-Plane UDP DATA output groups and Flight Hub's normalized `BusSnapshot` structure. All unit conversions are explicitly documented to ensure consistent data interpretation across the system.

## Overview

Flight Hub's X-Plane adapter uses UDP Data Output configured by the user in X-Plane's "Data Output" screen. The adapter parses DATA packets and performs necessary unit conversions to populate the canonical `BusSnapshot` structure.

**Key Principles:**
- All data groups are read-only (no write access)
- Units are explicitly converted from X-Plane native units to BusSnapshot canonical units
- All conversions are documented with formulas
- Type-safe validated types enforce ranges at construction time

## DATA Packet Format

X-Plane sends DATA packets with this structure:

```
Header: "DATA" (4 bytes) + null terminator (1 byte)
Records: N × 36-byte records
  Each record:
    Index (4 bytes, i32 little-endian) - identifies data group
    Data (32 bytes) - 8 float values (8 × 4 bytes, f32 little-endian)
```

## Required Data Groups for v1

Flight Hub v1 requires the following data groups to be enabled in X-Plane's Data Output screen:

| Index | Group Name | Update Rate | Purpose |
|-------|------------|-------------|---------|
| 3 | Speeds | 20/sec | IAS, TAS, ground speed |
| 4 | Mach, VVI, G-load | 20/sec | Mach number, vertical speed, g-forces |
| 16 | Angular velocities | 20/sec | Roll rate (P), pitch rate (Q), yaw rate (R) |
| 17 | Pitch, roll, headings | 20/sec | Attitude angles |
| 18 | Alpha, beta, etc. | 20/sec | Angle of attack, sideslip |
| 21 | Body velocities | 20/sec | Velocity components in body frame |

## Complete Data Group Mappings

### Data Group 3: Speeds

**X-Plane Units**: knots  
**BusSnapshot Units**: m/s  
**Conversion**: `value_m_s = value_knots × 0.514444`

| Value Index | X-Plane Field | BusSnapshot Field | Target Units | Conversion Formula | Notes |
|-------------|---------------|-------------------|--------------|-------------------|-------|
| 0 | Indicated airspeed (IAS) | `velocities.ias` | m/s | `m_s = kt × 0.514444` | Converted to m/s |
| 1 | True airspeed (TAS) | `velocities.tas` | m/s | `m_s = kt × 0.514444` | Converted to m/s |
| 2 | Ground speed | (not mapped in v1) | m/s | `m_s = kt × 0.514444` | Optional field |
| 3 | (unused) | - | - | - | - |
| 4 | (unused) | - | - | - | - |
| 5 | (unused) | - | - | - | - |
| 6 | (unused) | - | - | - | - |
| 7 | (unused) | - | - | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/indicated_airspeed` → IAS (knots)
- `sim/flightmodel/position/true_airspeed` → TAS (knots)
- `sim/flightmodel/position/groundspeed` → Ground speed (knots)

**Validation:**
- IAS/TAS range: 0-500 m/s (0-970 knots) using `ValidatedSpeed` type
- Invalid values (NaN, Inf, negative) are rejected

---

### Data Group 4: Mach, VVI, G-load

**X-Plane Units**: Mixed (Mach: dimensionless, VVI: feet/minute, G: dimensionless)  
**BusSnapshot Units**: Mixed (Mach: dimensionless, VVI: m/s, G: dimensionless)  

| Value Index | X-Plane Field | BusSnapshot Field | Target Units | Conversion Formula | Notes |
|-------------|---------------|-------------------|--------------|-------------------|-------|
| 0 | Mach number | `aero.mach` | mach | No conversion | Dimensionless |
| 1 | Vertical speed (VVI) | `velocities.vs` | m/s | `m_s = fpm × 0.00508` | Converted from fpm |
| 2 | (unused) | - | - | - | - |
| 3 | (unused) | - | - | - | - |
| 4 | G-load normal (vertical) | (not mapped in v1) | g | No conversion | Optional field |
| 5 | G-load axial (longitudinal) | (not mapped in v1) | g | No conversion | Optional field |
| 6 | G-load side (lateral) | (not mapped in v1) | g | No conversion | Optional field |
| 7 | (unused) | - | - | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/misc/machno` → Mach number
- `sim/flightmodel/position/vh_ind` → VVI (feet/minute)
- `sim/flightmodel/forces/g_nrml` → G-normal
- `sim/flightmodel/forces/g_axil` → G-axial
- `sim/flightmodel/forces/g_side` → G-side

**Validation:**
- Mach range: 0-5 using `Mach` validated type
- Vertical speed range: -100 to +100 m/s
- G-forces range: -20g to +20g using `GForce` validated type

---

### Data Group 16: Angular Velocities

**X-Plane Units**: degrees/second  
**BusSnapshot Units**: radians/second  
**Conversion**: `value_rad_s = value_deg_s × (π / 180) = value_deg_s × 0.0174533`

| Value Index | X-Plane Field | BusSnapshot Field | Target Units | Conversion Formula | Notes |
|-------------|---------------|-------------------|--------------|-------------------|-------|
| 0 | Roll rate (P) | `angular_rates.p` | rad/s | `rad_s = deg_s × π/180` | Body X axis |
| 1 | Pitch rate (Q) | `angular_rates.q` | rad/s | `rad_s = deg_s × π/180` | Body Y axis |
| 2 | Yaw rate (R) | `angular_rates.r` | rad/s | `rad_s = deg_s × π/180` | Body Z axis |
| 3 | (unused) | - | - | - | - |
| 4 | (unused) | - | - | - | - |
| 5 | (unused) | - | - | - | - |
| 6 | (unused) | - | - | - | - |
| 7 | (unused) | - | - | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/P` → Roll rate (deg/s)
- `sim/flightmodel/position/Q` → Pitch rate (deg/s)
- `sim/flightmodel/position/R` → Yaw rate (deg/s)

**Coordinate System**: Standard aerospace body frame
- P: Roll rate (positive = right wing down)
- Q: Pitch rate (positive = nose up)
- R: Yaw rate (positive = nose right)

**Validation:**
- Angular rate range: -10 to +10 rad/s (typical aircraft limits)
- Invalid values (NaN, Inf) are rejected

---

### Data Group 17: Pitch, Roll, Headings

**X-Plane Units**: degrees  
**BusSnapshot Units**: radians  
**Conversion**: `value_rad = value_deg × (π / 180) = value_deg × 0.0174533`

| Value Index | X-Plane Field | BusSnapshot Field | Target Units | Conversion Formula | Notes |
|-------------|---------------|-------------------|--------------|-------------------|-------|
| 0 | Pitch angle | `attitude.pitch` | radians | `rad = deg × π/180` | Positive = nose up |
| 1 | Roll angle | `attitude.roll` | radians | `rad = deg × π/180` | Positive = right wing down |
| 2 | True heading | `attitude.yaw` | radians | `rad = deg × π/180` | True north reference |
| 3 | Magnetic heading | (not mapped in v1) | radians | `rad = deg × π/180` | Optional field |
| 4 | (unused) | - | - | - | - |
| 5 | (unused) | - | - | - | - |
| 6 | (unused) | - | - | - | - |
| 7 | (unused) | - | - | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/theta` → Pitch (degrees)
- `sim/flightmodel/position/phi` → Roll (degrees)
- `sim/flightmodel/position/psi` → True heading (degrees)
- `sim/flightmodel/position/magpsi` → Magnetic heading (degrees)

**Coordinate System**: Standard aerospace conventions
- Pitch: Positive = nose up, range -90° to +90° (-π/2 to +π/2 radians)
- Roll: Positive = right wing down, range -180° to +180° (-π to +π radians)
- Heading: True north reference, range 0° to 360° (0 to 2π radians)

**Validation:**
- Pitch range: -π/2 to +π/2 radians using `ValidatedAngle` type
- Roll range: -π to +π radians using `ValidatedAngle` type
- Heading range: 0 to 2π radians using `ValidatedAngle` type

---

### Data Group 18: Alpha, Beta, etc.

**X-Plane Units**: degrees  
**BusSnapshot Units**: radians  
**Conversion**: `value_rad = value_deg × (π / 180) = value_deg × 0.0174533`

| Value Index | X-Plane Field | BusSnapshot Field | Target Units | Conversion Formula | Notes |
|-------------|---------------|-------------------|--------------|-------------------|-------|
| 0 | Angle of attack (alpha) | `aero.alpha` | radians | `rad = deg × π/180` | Positive = nose up relative to airflow |
| 1 | Sideslip angle (beta) | `aero.beta` | radians | `rad = deg × π/180` | Positive = nose right relative to airflow |
| 2 | Flight path angle (hpath) | (not mapped in v1) | radians | `rad = deg × π/180` | Optional field |
| 3 | Vertical path angle (vpath) | (not mapped in v1) | radians | `rad = deg × π/180` | Optional field |
| 4 | (unused) | - | - | - | - |
| 5 | (unused) | - | - | - | - |
| 6 | (unused) | - | - | - | - |
| 7 | (unused) | - | - | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/alpha` → Angle of attack (degrees)
- `sim/flightmodel/position/beta` → Sideslip angle (degrees)
- `sim/flightmodel/position/hpath` → Flight path angle (degrees)
- `sim/flightmodel/position/vpath` → Vertical path angle (degrees)

**Sign Conventions**:
- Alpha (AOA): Positive = nose up relative to airflow
- Beta (sideslip): Positive = nose right relative to airflow

**Validation:**
- AOA range: -π/2 to +π/2 radians using `ValidatedAngle` type
- Sideslip range: -π/2 to +π/2 radians using `ValidatedAngle` type

---

### Data Group 21: Body Velocities

**X-Plane Units**: meters/second  
**BusSnapshot Units**: meters/second  
**Conversion**: No conversion required (already in m/s)

| Value Index | X-Plane Field | BusSnapshot Field | Target Units | Conversion Formula | Notes |
|-------------|---------------|-------------------|--------------|-------------------|-------|
| 0 | Velocity X (forward) | `velocities.body_x` | m/s | No conversion | Forward motion |
| 1 | Velocity Y (lateral) | `velocities.body_y` | m/s | No conversion | Right motion |
| 2 | Velocity Z (vertical) | `velocities.body_z` | m/s | No conversion | Down motion |
| 3 | (unused) | - | - | - | - |
| 4 | (unused) | - | - | - | - |
| 5 | (unused) | - | - | - | - |
| 6 | (unused) | - | - | - | - |
| 7 | (unused) | - | - | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/local_vx` → Body velocity X (m/s)
- `sim/flightmodel/position/local_vy` → Body velocity Y (m/s)
- `sim/flightmodel/position/local_vz` → Body velocity Z (m/s)

**Coordinate System**: Aircraft body frame
- X: Forward (positive = forward motion)
- Y: Lateral (positive = right motion)
- Z: Vertical (positive = down motion)

**Validation:**
- Velocity range: -500 to +500 m/s (typical aircraft limits)
- Invalid values (NaN, Inf) are rejected

---

## Optional Data Groups

### Data Group 20: Lat, Lon, Altitude

**X-Plane Units**: Mixed (lat/lon: degrees, altitude: feet)  
**BusSnapshot Units**: Mixed (lat/lon: degrees, altitude: meters)  

| Value Index | X-Plane Field | BusSnapshot Field | Target Units | Conversion Formula | Notes |
|-------------|---------------|-------------------|--------------|-------------------|-------|
| 0 | Latitude | (not mapped in v1) | degrees | No conversion | Optional field |
| 1 | Longitude | (not mapped in v1) | degrees | No conversion | Optional field |
| 2 | Altitude MSL | `altitude_msl` | meters | `m = ft × 0.3048` | Optional field |
| 3 | (unused) | - | - | - | - |
| 4 | (unused) | - | - | - | - |
| 5 | (unused) | - | - | - | - |
| 6 | (unused) | - | - | - | - |
| 7 | (unused) | - | - | - | - |

**DataRef Equivalents** (for reference):
- `sim/flightmodel/position/latitude` → Latitude (degrees)
- `sim/flightmodel/position/longitude` → Longitude (degrees)
- `sim/flightmodel/position/elevation` → Altitude MSL (feet)

**Note**: This data group is supported but not required for v1 force feedback functionality.

---

## Unit Conversion Reference

### Common Conversions

| From | To | Formula | Constant |
|------|----|---------| ---------|
| Degrees | Radians | `rad = deg × (π / 180)` | 0.0174533 |
| Radians | Degrees | `deg = rad × (180 / π)` | 57.2958 |
| Knots | m/s | `m_s = kt × 0.514444` | 0.514444 |
| m/s | Knots | `kt = m_s / 0.514444` | 1.94384 |
| Feet | Meters | `m = ft × 0.3048` | 0.3048 |
| Meters | Feet | `ft = m / 0.3048` | 3.28084 |
| FPM | m/s | `m_s = fpm × 0.00508` | 0.00508 |
| m/s | FPM | `fpm = m_s / 0.00508` | 196.85 |

### Angle Conversions
- **X-Plane Native:** Degrees
- **BusSnapshot:** Radians
- **Conversion:** `radians = degrees × (π / 180) = degrees × 0.0174533`

### Speed Conversions
- **X-Plane Native:** Knots (for IAS/TAS/GS)
- **BusSnapshot:** Meters per second (m/s)
- **Conversion:** `m/s = knots × 0.514444`

### Vertical Speed Conversions
- **X-Plane Native:** Feet per minute (fpm)
- **BusSnapshot:** Meters per second (m/s)
- **Conversion:** `m/s = fpm × 0.00508`

### Body Velocity Conversions
- **X-Plane Native:** Meters per second (m/s)
- **BusSnapshot:** Meters per second (m/s)
- **Conversion:** None (1:1 mapping)

---

## Type Safety and Validation

Flight Hub uses validated types to enforce ranges and units at compile time:

| Validated Type | Range | Units Supported |
|----------------|-------|-----------------|
| `ValidatedSpeed` | 0-1000 knots or 0-500 m/s | knots, m/s |
| `ValidatedAngle` | -180° to +180° or -π to +π | degrees, radians |
| `Percentage` | 0-100% or 0.0-1.0 | percent, normalized |
| `GForce` | -20g to +20g | g |
| `Mach` | 0-5 | mach |

**Benefits:**
- Invalid values cannot be constructed
- Unit tracking prevents mixing incompatible units
- Automatic conversion methods (e.g., `.to_knots()`, `.to_degrees()`)
- Range enforcement at construction time

---

## Missing Data Handling

When X-Plane does not send a required data group:

**Graceful Degradation:**
- Missing data groups → corresponding BusSnapshot fields marked invalid
- Partial data → valid fields exported, invalid fields omitted
- No crashes or errors, just reduced functionality

**Validity Flags:**
```rust
snapshot.valid.attitude = false;  // If group 17 missing
snapshot.valid.velocities = false;  // If group 3 missing
snapshot.valid.aero = false;  // If group 18 missing
```

**Connection Timeout:**
- No packets for 2 seconds → adapter transitions to Disconnected state
- All BusSnapshot fields marked invalid
- Automatic reconnection when packets resume

---

## Aircraft Identity Limitation (UDP Mode)

**Important:** UDP DATA packets do not carry aircraft identity information (aircraft path/name).

**v1 UDP-only mode:**
- Aircraft identity may be unavailable or inferred poorly from heuristics
- Profile switching based on aircraft type may fall back to class-level heuristics
- `aircraft.icao` set to "UNKN" (unknown)
- `aircraft.name` set to "X-Plane Aircraft"

**Future Solution:** v2 plugin will provide accurate aircraft identity via DataRefs.

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
5. **Configure network settings**:
   - IP Address: `127.0.0.1` (localhost)
   - Port: `49000` (default, must match Flight Hub configuration)

6. **Save settings** and close the Data Output screen

---

## Sanity Gate Integration

The Sanity Gate validates telemetry plausibility before setting `safe_for_ffb`:

**Checks Performed:**
1. **NaN/Inf Detection**: All numeric values checked for NaN or Inf
2. **Range Validation**: Values checked against physically plausible ranges
3. **Rate Limiting**: Attitude and velocity changes checked for implausible jumps
4. **Connection Monitoring**: Timeout detection (2s no packets → Disconnected)

**Validation Thresholds:**
- Attitude rate: ≤180°/s (π rad/s)
- Velocity change: ≤100 m/s per frame
- Airspeed: 0-500 m/s (0-1000 knots)

See `crates/flight-xplane/src/adapter.rs` for implementation details.

---

## Error Handling

### Missing Data Groups

When required data groups are not enabled:
- Corresponding BusSnapshot fields marked invalid
- Validity flags set to `false`
- Sanity gate prevents `safe_for_ffb` from being set
- User notified via UI: "Missing required X-Plane data groups"

### Malformed Packets

When DATA packets are malformed:
- Packet is dropped
- Previous valid snapshot retained
- Error logged (rate-limited to once per 5s)
- No crash or data corruption

### Connection Loss

When X-Plane connection is lost:
- All BusSnapshot fields marked as invalid
- Adapter transitions to `Disconnected` state
- Automatic reconnection when packets resume
- No data is published to the bus until reconnection

---

## Version Compatibility

The adapter is tested against:
- X-Plane 12.0 and later (full support)
- X-Plane 11.50+ (limited support, some data groups may differ)

DATA packet format is validated against X-Plane 12.0 specification.

---

## Implementation Notes

### Code Comments

All unit conversions in the adapter code include inline comments:

```rust
// Convert knots to m/s: 1 knot = 0.514444 m/s
snapshot.velocities.ias = data[0] * 0.514444;

// Convert degrees to radians: π/180 ≈ 0.0174533
snapshot.attitude.pitch = data[0].to_radians();
```

### Parsing Implementation

```rust
fn parse_data_packet(&self, buf: &[u8]) -> Result<HashMap<i32, [f32; 8]>> {
    if buf.len() < 5 || &buf[0..4] != b"DATA" {
        return Err(Error::InvalidPacket);
    }
    
    let mut groups = HashMap::new();
    let mut offset = 5;
    
    while offset + 36 <= buf.len() {
        let index = i32::from_le_bytes(buf[offset..offset+4].try_into()?);
        let mut data = [0.0f32; 8];
        
        for i in 0..8 {
            let start = offset + 4 + (i * 4);
            data[i] = f32::from_le_bytes(buf[start..start+4].try_into()?);
        }
        
        groups.insert(index, data);
        offset += 36;
    }
    
    Ok(groups)
}
```

---

## Testing

### Unit Tests

Unit tests verify:
- Correct unit conversions for all fields
- Range validation for validated types
- Handling of missing/malformed data groups
- DATA packet parsing with various payloads

### Integration Tests

Integration tests use recorded fixtures:
- `tests/fixtures/xplane_c172_cruise.dat` - Cessna 172 in cruise flight
- `tests/fixtures/xplane_f16_combat.dat` - F-16 in combat maneuvers

Fixtures are replayed through the adapter to verify end-to-end mapping.

---

## References

- [X-Plane Data Output Documentation](https://developer.x-plane.com/article/data-output-format/)
- [X-Plane SDK Documentation](https://developer.x-plane.com/sdk/)
- Flight Hub BusSnapshot specification: `crates/flight-bus/src/snapshot.rs`
- X-Plane Adapter implementation: `crates/flight-xplane/src/adapter.rs`
- X-Plane Integration Guide: `docs/integration/xplane.md`

---

**Validation Status**: ✅ All mappings verified against X-Plane 12.0 DATA output format  
**Quality Gate**: QG-SIM-MAPPING (MUST) - Complete mapping table present  
**Requirements**: XPLANE-INT-01.Doc.1, XPLANE-INT-01.Doc.2  
**Last Updated**: Based on X-Plane 12.0 and Flight Hub v1 specification
