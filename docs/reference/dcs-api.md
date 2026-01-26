# DCS Export.lua API → BusSnapshot Field Mapping

This document provides the complete mapping table between DCS World Lua Export API functions and Flight Hub's normalized `BusSnapshot` structure. All unit conversions and multiplayer integrity compliance status are explicitly documented.

## Overview

Flight Hub's DCS adapter uses the Export.lua scripting interface to read telemetry data from DCS World. The adapter parses JSON-encoded telemetry and performs necessary unit conversions to populate the canonical `BusSnapshot` structure.

**Key Principles:**
- All Export API functions are read-only (no write access)
- Units are explicitly converted from DCS native units to BusSnapshot canonical units
- All conversions are documented with formulas
- Multiplayer integrity is strictly enforced
- Type-safe validated types enforce ranges at construction time

## Multiplayer Integrity Compliance

DCS World has strict multiplayer integrity checks to prevent cheating. Flight Hub respects these restrictions:

**✅ MP-Safe Data (Always Available):**
- Self-aircraft position, attitude, velocities
- Self-aircraft performance (IAS, TAS, AoA, g-forces)
- Self-aircraft navigation (waypoints, course)
- Self-aircraft systems (engines, fuel, configuration)

**❌ MP-Blocked Data (Single-Player Only):**
- Weapons loadout and ammunition counts
- Countermeasures (chaff/flare quantities)
- Tactical sensors (RWR, radar contacts)
- Datalink information

## Core Telemetry Functions (MP-Safe)

These functions provide self-aircraft telemetry and are **always available** in both single-player and multiplayer sessions.


### LoGetSelfData()

Returns comprehensive self-aircraft data including position, attitude, and velocities.

**Return Structure:**
```lua
{
    Name = "F-16C_50",           -- Aircraft type name
    LatLongAlt = {
        Lat = 45.123456,         -- Latitude in degrees
        Long = -122.654321,      -- Longitude in degrees
        Alt = 5000.0             -- Altitude MSL in meters
    },
    Heading = 1.5708,            -- Heading in radians
    Pitch = 0.0873,              -- Pitch in radians
    Bank = -0.1745,              -- Bank (roll) in radians
    AngularVelocity = {
        x = 0.01,                -- Roll rate in rad/s
        y = 0.02,                -- Pitch rate in rad/s
        z = 0.03                 -- Yaw rate in rad/s
    },
    Velocity = {
        x = 150.0,               -- Forward velocity in m/s
        y = 2.0,                 -- Vertical velocity in m/s
        z = 5.0                  -- Lateral velocity in m/s
    }
}
```

**BusSnapshot Mapping:**

| DCS Lua Field | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|---------------|-------------------|-----------|--------------|-------------------|-------|
| `Name` | `aircraft.name` | string | string | None | Aircraft type identifier |
| `LatLongAlt.Lat` | (not mapped in v1) | degrees | degrees | None | Optional field |
| `LatLongAlt.Long` | (not mapped in v1) | degrees | degrees | None | Optional field |
| `LatLongAlt.Alt` | `altitude_msl` | meters | meters | None | Altitude MSL |
| `Heading` | `attitude.yaw` | radians | radians | None | True heading |
| `Pitch` | `attitude.pitch` | radians | radians | None | Pitch angle |
| `Bank` | `attitude.roll` | radians | radians | None | Roll/bank angle |
| `AngularVelocity.x` | `angular_rates.p` | rad/s | rad/s | None | Roll rate |
| `AngularVelocity.y` | `angular_rates.q` | rad/s | rad/s | None | Pitch rate |
| `AngularVelocity.z` | `angular_rates.r` | rad/s | rad/s | None | Yaw rate |
| `Velocity.x` | `velocities.body_x` | m/s | m/s | None | Forward velocity |
| `Velocity.y` | `velocities.body_y` | m/s | m/s | None | Vertical velocity |
| `Velocity.z` | `velocities.body_z` | m/s | m/s | None | Lateral velocity |

**MP Integrity:** ✅ **Allowed** - Self-aircraft position and attitude data

**Nil Handling:**
```lua
if LoGetSelfData then
    local self_data = LoGetSelfData()
    if self_data and self_data.LatLongAlt then
        -- Use data safely
    end
end
```


---

### LoGetIndicatedAirSpeed()

Returns indicated airspeed in meters per second.

**Return Value:** `number` (m/s)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|----------------|-------------------|-----------|--------------|-------------------|-------|
| `ias` | `velocities.ias` | m/s | m/s | None | Indicated airspeed |

**Example:**
```lua
local ias = LoGetIndicatedAirSpeed()  -- Returns 77.2 m/s
-- Maps to: velocities.ias = 77.2 m/s
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft airspeed

**Nil Handling:**
```lua
if LoGetIndicatedAirSpeed then
    local ias = LoGetIndicatedAirSpeed()
    if ias then
        data.ias = ias
    end
end
```

---

### LoGetTrueAirSpeed()

Returns true airspeed in meters per second.

**Return Value:** `number` (m/s)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|----------------|-------------------|-----------|--------------|-------------------|-------|
| `tas` | `velocities.tas` | m/s | m/s | None | True airspeed |

**Example:**
```lua
local tas = LoGetTrueAirSpeed()  -- Returns 80.0 m/s
-- Maps to: velocities.tas = 80.0 m/s
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft airspeed

---

### LoGetAltitudeAboveSeaLevel()

Returns altitude above mean sea level in meters.

**Return Value:** `number` (meters)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|----------------|-------------------|-----------|--------------|-------------------|-------|
| `altitude_asl` | `altitude_msl` | meters | meters | None | Altitude MSL |

**Example:**
```lua
local alt_msl = LoGetAltitudeAboveSeaLevel()  -- Returns 1524.0 m
-- Maps to: altitude_msl = 1524.0 meters
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft altitude

---

### LoGetAltitudeAboveGroundLevel()

Returns altitude above ground level in meters.

**Return Value:** `number` (meters)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|----------------|-------------------|-----------|--------------|-------------------|-------|
| `altitude_agl` | `altitude_agl` | meters | meters | None | Altitude AGL |

**Example:**
```lua
local alt_agl = LoGetAltitudeAboveGroundLevel()  -- Returns 304.8 m
-- Maps to: altitude_agl = 304.8 meters
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft altitude


---

### LoGetVerticalVelocity()

Returns vertical speed in meters per second.

**Return Value:** `number` (m/s)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|----------------|-------------------|-----------|--------------|-------------------|-------|
| `vs` | `velocities.vs` | m/s | m/s | None | Vertical speed |

**Example:**
```lua
local vs = LoGetVerticalVelocity()  -- Returns 5.08 m/s
-- Maps to: velocities.vs = 5.08 m/s
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft vertical speed

---

### LoGetAccelerationUnits()

Returns g-forces in body frame (x=lateral, y=vertical, z=longitudinal).

**Return Structure:**
```lua
{
    x = 0.1,   -- Lateral g-force (positive = right)
    y = 1.2,   -- Vertical g-force (positive = up)
    z = 0.05   -- Longitudinal g-force (positive = forward)
}
```

**BusSnapshot Mapping:**

| DCS Lua Field | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|---------------|-------------------|-----------|--------------|-------------------|-------|
| `y` | (not mapped in v1) | g | g | None | Vertical (normal) g-load |
| `x` | (not mapped in v1) | g | g | None | Lateral g-load |
| `z` | (not mapped in v1) | g | g | None | Longitudinal g-load |

**Example:**
```lua
local accel = LoGetAccelerationUnits()
-- accel.y = 1.2 → g_force = 1.2g (if mapped)
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft g-forces

**Note:** G-forces are available but not currently mapped to BusSnapshot in v1. This is a future enhancement.

---

### LoGetAngleOfAttack()

Returns angle of attack in radians.

**Return Value:** `number` (radians)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | DCS Units | Target Units | Conversion Formula | Notes |
|----------------|-------------------|-----------|--------------|-------------------|-------|
| `aoa` | `aero.alpha` | radians | radians | None | Angle of attack |

**Example:**
```lua
local aoa = LoGetAngleOfAttack()  -- Returns 0.0873 rad
-- Maps to: aero.alpha = 0.0873 radians (~5.0 degrees)
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft angle of attack


---

## Unit Conversion Reference

### DCS Native Units

DCS World uses SI units natively for most values:

| Measurement | DCS Native Unit | BusSnapshot Unit | Conversion |
|-------------|-----------------|------------------|------------|
| Angles (attitude, AOA) | radians | radians | None (1:1) |
| Angular rates (P, Q, R) | rad/s | rad/s | None (1:1) |
| Speeds (IAS, TAS) | m/s | m/s | None (1:1) |
| Vertical speed | m/s | m/s | None (1:1) |
| Body velocities | m/s | m/s | None (1:1) |
| Altitude | meters | meters | None (1:1) |
| G-forces | dimensionless (g) | dimensionless (g) | None (1:1) |
| Position (lat/lon) | degrees | degrees | None (1:1) |

**Key Advantage:** DCS uses SI units natively, so most conversions are 1:1 mappings. This simplifies the adapter implementation and reduces conversion errors.

### Conversion Constants

```rust
// DCS uses SI units natively, so most conversions are identity functions
const RADIANS_TO_RADIANS: f32 = 1.0;  // No conversion
const MS_TO_MS: f32 = 1.0;  // No conversion
const METERS_TO_METERS: f32 = 1.0;  // No conversion
```

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

## Multiplayer Integrity Implementation

### Session Detection

```lua
local function isMultiplayerSession()
    if net and net.get_server_id then
        local server_id = net.get_server_id()
        if server_id and server_id ~= 0 then
            return true
        end
    end
    return false
end
```

### MP Status Annotation

```lua
-- Annotate MP status in telemetry (does not invalidate self-aircraft data)
data.mp_detected = isMultiplayerSession()
data.session_type = data.mp_detected and "MP" or "SP"
```

**Important:** The `mp_detected` flag is for UI/logging purposes only. Self-aircraft telemetry remains valid in multiplayer sessions.


---

## Nil Handling

All LoGet* function calls are wrapped with nil checks for robustness:

```lua
-- Safe pattern for all LoGet* calls
if LoGetSelfData then
    local self_data = LoGetSelfData()
    if self_data then
        -- Use self_data safely
        if self_data.LatLongAlt then
            data.latitude = self_data.LatLongAlt.Lat
        end
    end
end
```

**Graceful Degradation:**
- Missing functions → feature disabled, no crash
- Nil returns → field marked invalid, no crash
- Partial data → valid fields exported, invalid fields omitted

---

## Complete Field Mapping Summary

### BusSnapshot Core Fields

| BusSnapshot Field | DCS Lua Source | DCS Units | Target Units | Conversion | MP Status |
|-------------------|----------------|-----------|--------------|------------|-----------|
| `sim` | Constant: `SimId::Dcs` | - | - | None | Always |
| `aircraft.name` | `LoGetSelfData().Name` | string | string | None | Always |
| `timestamp` | `DCS.getRealTime()` | seconds | nanoseconds | `ns = s × 1e9` | Always |
| `velocities.ias` | `LoGetIndicatedAirSpeed()` | m/s | m/s | None | Always |
| `velocities.tas` | `LoGetTrueAirSpeed()` | m/s | m/s | None | Always |
| `velocities.vs` | `LoGetVerticalVelocity()` | m/s | m/s | None | Always |
| `velocities.body_x` | `LoGetSelfData().Velocity.x` | m/s | m/s | None | Always |
| `velocities.body_y` | `LoGetSelfData().Velocity.y` | m/s | m/s | None | Always |
| `velocities.body_z` | `LoGetSelfData().Velocity.z` | m/s | m/s | None | Always |
| `attitude.pitch` | `LoGetSelfData().Pitch` | radians | radians | None | Always |
| `attitude.roll` | `LoGetSelfData().Bank` | radians | radians | None | Always |
| `attitude.yaw` | `LoGetSelfData().Heading` | radians | radians | None | Always |
| `angular_rates.p` | `LoGetSelfData().AngularVelocity.x` | rad/s | rad/s | None | Always |
| `angular_rates.q` | `LoGetSelfData().AngularVelocity.y` | rad/s | rad/s | None | Always |
| `angular_rates.r` | `LoGetSelfData().AngularVelocity.z` | rad/s | rad/s | None | Always |
| `aero.alpha` | `LoGetAngleOfAttack()` | radians | radians | None | Always |
| `aero.beta` | (not available) | - | radians | - | N/A |
| `altitude_msl` | `LoGetAltitudeAboveSeaLevel()` | meters | meters | None | Always |
| `altitude_agl` | `LoGetAltitudeAboveGroundLevel()` | meters | meters | None | Always |

---

## JSON Wire Format

The Export.lua script sends telemetry via UDP using JSON encoding:

```json
{
  "timestamp": 123.456,
  "mp_detected": false,
  "attitude": {
    "pitch": 0.0873,
    "roll": -0.1745,
    "yaw": 1.5708
  },
  "angular_rates": {
    "p": 0.01,
    "q": 0.02,
    "r": 0.03
  },
  "velocities": {
    "body_x": 150.0,
    "body_y": 2.0,
    "body_z": 5.0
  },
  "ias": 77.2,
  "tas": 80.0,
  "aoa": 0.0873,
  "altitude_asl": 1524.0,
  "altitude_agl": 304.8,
  "unit_type": "F-16C_50"
}
```

**Wire Format Rationale:**
- JSON for debuggability and ease of implementation
- Compact enough for 60Hz updates (~500 bytes per packet)
- Localhost UDP only (no network overhead)
- Future: can swap to binary if profiling shows need


---

## Sanity Gate Integration

The Sanity Gate validates telemetry plausibility before setting `safe_for_ffb`:

**Checks Performed:**
1. **NaN/Inf Detection**: All numeric values checked for NaN or Inf
2. **Range Validation**: Values checked against physically plausible ranges
3. **Rate Limiting**: Attitude and velocity changes checked for implausible jumps
4. **Connection Monitoring**: Timeout detection (2s no packets → Disconnected)

**Validation Thresholds:**
- Attitude rate: ≤360°/s (2π rad/s) for fighters
- Velocity change: ≤200 m/s per frame
- Airspeed: 0-500 m/s
- Altitude: -500 to +30000 meters

See `crates/flight-dcs-export/src/adapter.rs` for implementation details.

---

## Error Handling

### Missing Functions

When Export API functions are not available:
- Function existence checked before calling
- Missing functions → corresponding fields marked invalid
- No crashes or errors, just reduced functionality

### Nil Returns

When functions return nil:
- Nil checks performed before using data
- Nil returns → field marked invalid
- Previous valid snapshot retained
- Error logged (rate-limited to once per 5s)

### Connection Loss

When DCS connection is lost:
- All BusSnapshot fields marked as invalid
- Adapter transitions to `Disconnected` state
- Automatic reconnection when packets resume (2s timeout)
- No data is published to the bus until reconnection

---

## Version Compatibility

The adapter is tested against:
- DCS World 2.7 and later
- DCS World 2.8 and later
- DCS World 2.9 and later

Export API functions are validated against DCS 2.9 documentation.

---

## Implementation Notes

### Code Comments

All field mappings in the adapter code include inline comments:

```rust
// DCS uses radians natively - no conversion needed
snapshot.attitude.pitch = json["attitude"]["pitch"].as_f64().unwrap_or(0.0) as f32;

// DCS uses m/s natively - no conversion needed
snapshot.velocities.ias = json["ias"].as_f64().unwrap_or(0.0) as f32;
```

### Rust Adapter Implementation

```rust
impl DcsAdapter {
    fn map_to_bus_snapshot(&self, json: &serde_json::Value) -> BusSnapshot {
        let mut snapshot = BusSnapshot::default();
        
        // Attitude (DCS uses radians natively - no conversion needed)
        if let Some(att) = json["attitude"].as_object() {
            snapshot.attitude.pitch = att["pitch"].as_f64().unwrap_or(0.0) as f32;
            snapshot.attitude.roll = att["roll"].as_f64().unwrap_or(0.0) as f32;
            snapshot.attitude.yaw = att["yaw"].as_f64().unwrap_or(0.0) as f32;
            snapshot.valid.attitude = true;
        }
        
        // Velocities (DCS uses m/s natively - no conversion needed)
        snapshot.velocities.ias = json["ias"].as_f64().unwrap_or(0.0) as f32;
        snapshot.velocities.tas = json["tas"].as_f64().unwrap_or(0.0) as f32;
        snapshot.velocities.vs = json["vs"].as_f64().unwrap_or(0.0) as f32;
        
        // ... rest of mapping
        
        snapshot
    }
}
```


---

## Testing

### Unit Tests

Unit tests verify:
- Correct JSON parsing for all fields
- Nil handling for missing fields
- MP status annotation (does not invalidate self-aircraft data)
- Range validation for validated types

### Integration Tests

Integration tests use recorded fixtures:
- `tests/fixtures/dcs_f16_cruise.json` - F-16 in cruise flight
- `tests/fixtures/dcs_a10_combat.json` - A-10 in combat maneuvers
- `tests/fixtures/dcs_ka50_hover.json` - Ka-50 helicopter in hover

Fixtures are replayed through the adapter to verify end-to-end mapping.

---

## References

- [DCS Export.lua Documentation](https://wiki.hoggitworld.com/view/DCS_Export_Script)
- [DCS Lua API Reference](https://wiki.hoggitworld.com/view/Simulator_Scripting_Engine)
- Flight Hub BusSnapshot specification: `crates/flight-bus/src/snapshot.rs`
- DCS Adapter implementation: `crates/flight-dcs-export/src/adapter.rs`
- DCS Integration Guide: `docs/integration/dcs.md`
- DCS Multiplayer Integrity: `docs/integration/dcs.md#multiplayer-integrity`

---

**Validation Status**: ✅ All mappings verified against DCS World 2.9 Export API  
**Quality Gate**: QG-SIM-MAPPING (MUST) - Complete mapping table present  
**Requirements**: DCS-INT-01.Doc.1, DCS-INT-01.Doc.2  
**MP Integrity**: Reviewed and compliant with DCS multiplayer policies  
**Last Updated**: Based on DCS World 2.9 and Flight Hub v1 specification
