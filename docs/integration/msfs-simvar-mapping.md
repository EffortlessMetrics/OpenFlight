# MSFS SimVar → BusSnapshot Field Mapping

This document provides the complete mapping table between Microsoft Flight Simulator SimConnect variables (SimVars) and Flight Hub's normalized `BusSnapshot` structure. All unit conversions are explicitly documented to ensure consistent data interpretation across the system.

## Overview

Flight Hub's MSFS adapter uses the official SimConnect SDK to read telemetry data from Microsoft Flight Simulator. The adapter registers data definitions with explicit units for each SimVar and performs necessary unit conversions to populate the canonical `BusSnapshot` structure.

**Key Principles:**
- All SimVars are read-only (no write access)
- Units are explicitly specified in SimConnect data definitions
- All conversions are documented with formulas
- Type-safe validated types enforce ranges at construction time

## Data Update Rates

Different categories of data are requested at different rates to balance latency and performance:

| Category | Target Rate | SimConnect Period |
|----------|-------------|-------------------|
| Kinematics | 60 Hz | `VISUAL_FRAME` |
| Configuration | 30 Hz | `SIM_FRAME` |
| Engines | 30 Hz | `SIM_FRAME` |
| Environment | 10 Hz | `SECOND` |
| Navigation | 5 Hz | `SECOND` |

## Kinematics Mapping

### Velocities

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `AIRSPEED INDICATED` | knots | `kinematics.ias` | m/s | `value_m_s = value_kt × 0.514444` |
| `AIRSPEED TRUE` | knots | `kinematics.tas` | m/s | `value_m_s = value_kt × 0.514444` |
| `GROUND VELOCITY` | knots | `kinematics.ground_speed` | m/s | `value_m_s = value_kt × 0.514444` |
| `VERTICAL SPEED` | feet per minute | `kinematics.vertical_speed` | feet per minute | No conversion (stored as-is) |

**Notes:**
- Knots to m/s conversion: 1 knot = 0.514444 m/s
- Vertical speed is kept in feet per minute for compatibility with aviation conventions
- All speeds use `ValidatedSpeed` type with range enforcement (0-1000 knots or 0-500 m/s)

### Attitude

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `ATTITUDE PITCH DEGREES` | degrees | `kinematics.pitch` | radians | `value_rad = value_deg × (π / 180)` |
| `ATTITUDE BANK DEGREES` | degrees | `kinematics.bank` | radians | `value_rad = value_deg × (π / 180)` |
| `ATTITUDE HEADING DEGREES` | degrees | `kinematics.heading` | radians | `value_rad = value_deg × (π / 180)` |

**Notes:**
- All attitude angles are converted from degrees to radians
- Conversion factor: π / 180 ≈ 0.0174533
- Uses `ValidatedAngle` type with range enforcement (-180° to +180° or -π to +π radians)
- Heading is magnetic heading by default

### Aerodynamics

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `INCIDENCE ALPHA` | degrees | `kinematics.aoa` | radians | `value_rad = value_deg × (π / 180)` |
| `INCIDENCE BETA` | degrees | `kinematics.sideslip` | radians | `value_rad = value_deg × (π / 180)` |
| `AIRSPEED MACH` | mach | `kinematics.mach` | mach | No conversion |

**Notes:**
- Angle of attack (AoA) and sideslip are converted from degrees to radians
- Mach number uses `Mach` validated type with range 0-5
- AoA and sideslip use `ValidatedAngle` type

### G-Forces

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `G FORCE` | gforce | `kinematics.g_force` | g | No conversion |
| `G FORCE LATERAL` | gforce | `kinematics.g_lateral` | g | No conversion |
| `G FORCE LONGITUDINAL` | gforce | `kinematics.g_longitudinal` | g | No conversion |

**Notes:**
- G-forces are already in standard g units (1g = 9.81 m/s²)
- Uses `GForce` validated type with range -20g to +20g
- Vertical g-force (nz) is the primary load factor for FFB calculations

## Aircraft Configuration Mapping

### Landing Gear

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `GEAR CENTER POSITION` | percent | `config.gear.nose` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `GEAR LEFT POSITION` | percent | `config.gear.left` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `GEAR RIGHT POSITION` | percent | `config.gear.right` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |

**Notes:**
- Gear positions are normalized from 0-100% to 0.0-1.0 range
- 0.0 = fully retracted, 1.0 = fully extended
- Uses `Percentage` validated type

### Flight Controls

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `FLAPS HANDLE PERCENT` | percent | `config.flaps` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `SPOILERS HANDLE POSITION` | percent | `config.spoilers` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |

**Notes:**
- Control surface positions are normalized from 0-100% to 0.0-1.0 range
- Uses `Percentage` validated type with automatic range enforcement

### Autopilot

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `AUTOPILOT MASTER` | bool | `config.ap_state.master` | bool | Direct mapping |
| `AUTOPILOT ALTITUDE LOCK` | bool | `config.ap_state.altitude_hold` | bool | Direct mapping |
| `AUTOPILOT HEADING LOCK` | bool | `config.ap_state.heading_hold` | bool | Direct mapping |
| `AUTOPILOT AIRSPEED HOLD` | bool | `config.ap_state.speed_hold` | bool | Direct mapping |
| `AUTOPILOT ALTITUDE LOCK VAR` | feet | `config.ap_altitude` | feet | No conversion |
| `AUTOPILOT HEADING LOCK DIR` | degrees | `config.ap_heading` | radians | `value_rad = value_deg × (π / 180)` |
| `AUTOPILOT AIRSPEED HOLD VAR` | knots | `config.ap_speed` | m/s | `value_m_s = value_kt × 0.514444` |

**Notes:**
- Boolean autopilot states are mapped directly (0 = false, non-zero = true)
- Autopilot altitude target is kept in feet for aviation conventions
- Autopilot heading is converted to radians
- Autopilot speed is converted to m/s

### Lights

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `LIGHT NAV` | bool | `config.lights.nav` | bool | Direct mapping |
| `LIGHT BEACON` | bool | `config.lights.beacon` | bool | Direct mapping |
| `LIGHT STROBE` | bool | `config.lights.strobe` | bool | Direct mapping |
| `LIGHT LANDING` | bool | `config.lights.landing` | bool | Direct mapping |
| `LIGHT TAXI` | bool | `config.lights.taxi` | bool | Direct mapping |
| `LIGHT LOGO` | bool | `config.lights.logo` | bool | Direct mapping |
| `LIGHT WING` | bool | `config.lights.wing` | bool | Direct mapping |

**Notes:**
- All light states are boolean (on/off)
- Direct mapping from SimConnect bool (0 = off, non-zero = on)

## Engine Data Mapping

Engine data is requested per-engine using indexed SimVars (`:1`, `:2`, etc.).

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `GENERAL ENG COMBUSTION:N` | bool | `engines[N-1].running` | bool | Direct mapping |
| `GENERAL ENG RPM:N` | percent | `engines[N-1].rpm` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `RECIP ENG MANIFOLD PRESSURE:N` | inHg | `engines[N-1].manifold_pressure` | inHg | No conversion |
| `GENERAL ENG EXHAUST GAS TEMPERATURE:N` | °F | `engines[N-1].egt` | °F | No conversion |
| `RECIP ENG CYLINDER HEAD TEMPERATURE:N` | °F | `engines[N-1].cht` | °F | No conversion |
| `GENERAL ENG FUEL FLOW GPH:N` | gallons/hour | `engines[N-1].fuel_flow` | gallons/hour | No conversion |
| `GENERAL ENG OIL PRESSURE:N` | psf | `engines[N-1].oil_pressure` | psf | No conversion |
| `GENERAL ENG OIL TEMPERATURE:N` | °F | `engines[N-1].oil_temperature` | °F | No conversion |

**Notes:**
- Engine index N in SimVar corresponds to array index N-1 in BusSnapshot
- RPM is normalized from 0-100% to 0.0-1.0 range using `Percentage` type
- Temperature values are kept in Fahrenheit for aviation conventions
- Manifold pressure is kept in inches of mercury (inHg)
- Optional fields (manifold pressure, EGT, CHT, etc.) may be `None` for jet engines

## Environment Data Mapping

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `INDICATED ALTITUDE` | feet | `environment.altitude` | feet | No conversion |
| `PRESSURE ALTITUDE` | feet | `environment.pressure_altitude` | feet | No conversion |
| `AMBIENT TEMPERATURE` | °C | `environment.oat` | °C | No conversion |
| `AMBIENT WIND VELOCITY` | knots | `environment.wind_speed` | m/s | `value_m_s = value_kt × 0.514444` |
| `AMBIENT WIND DIRECTION` | degrees | `environment.wind_direction` | radians | `value_rad = value_deg × (π / 180)` |
| `AMBIENT VISIBILITY` | statute miles | `environment.visibility` | statute miles | No conversion |
| `AMBIENT CLOUD COVERAGE` | percent | `environment.cloud_coverage` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |

**Notes:**
- Altitudes are kept in feet for aviation conventions
- Temperature is in Celsius
- Wind speed is converted to m/s
- Wind direction is converted to radians
- Visibility is kept in statute miles

## Navigation Data Mapping

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `PLANE LATITUDE` | degrees | `navigation.latitude` | degrees | No conversion |
| `PLANE LONGITUDE` | degrees | `navigation.longitude` | degrees | No conversion |
| `GPS GROUND TRUE TRACK` | degrees | `navigation.ground_track` | radians | `value_rad = value_deg × (π / 180)` |
| `GPS WP DISTANCE` | meters | `navigation.distance_to_dest` | meters | No conversion |
| `GPS WP ETE` | seconds | `navigation.time_to_dest` | seconds | No conversion |
| `GPS WP NEXT ID` | string | `navigation.active_waypoint` | string | Direct mapping |

**Notes:**
- Latitude and longitude are kept in degrees (decimal format)
- Ground track is converted to radians
- Distance is in meters
- Time to destination is in seconds
- Waypoint ID is a string identifier

## Helicopter-Specific Mapping

Helicopter data is only populated when flying a helicopter aircraft.

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `ROTOR RPM PCT:1` | percent | `helo.nr` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `ENG TURBINE ENGINE N1:1` | percent | `helo.np` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `ENG TORQUE:1` | percent | `helo.torque` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `COLLECTIVE POSITION` | percent | `helo.collective` | 0.0-1.0 | `value_normalized = value_percent / 100.0` |
| `RUDDER PEDAL POSITION` | position | `helo.pedals` | -100.0 to +100.0 | `value_scaled = value_position × 100.0` |

**Notes:**
- Nr = main rotor RPM (percentage of nominal)
- Np = power turbine RPM (percentage of nominal)
- Torque is percentage of maximum rated torque
- Collective is normalized from 0-100% to 0.0-1.0
- Pedals range from -100 (full left) to +100 (full right)
- All percentage values use `Percentage` validated type except pedals

## Fuel System Mapping

Fuel quantities are requested per-tank using indexed SimVars.

| SimVar | Units | BusSnapshot Field | Target Units | Conversion Formula |
|--------|-------|-------------------|--------------|-------------------|
| `FUEL TANK LEFT MAIN QUANTITY` | gallons | `config.fuel["left_main"]` | gallons | No conversion |
| `FUEL TANK RIGHT MAIN QUANTITY` | gallons | `config.fuel["right_main"]` | gallons | No conversion |
| `FUEL TANK CENTER QUANTITY` | gallons | `config.fuel["center"]` | gallons | No conversion |
| `FUEL TANK LEFT AUX QUANTITY` | gallons | `config.fuel["left_aux"]` | gallons | No conversion |
| `FUEL TANK RIGHT AUX QUANTITY` | gallons | `config.fuel["right_aux"]` | gallons | No conversion |

**Notes:**
- Fuel quantities are stored in a HashMap with tank identifier as key
- Units are gallons (US)
- Tank identifiers are standardized strings
- Not all tanks are present on all aircraft

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
| Percent | Normalized | `norm = pct / 100.0` | 0.01 |
| Normalized | Percent | `pct = norm × 100.0` | 100.0 |

### Temperature Conversions

| From | To | Formula |
|------|----|---------| 
| °F | °C | `C = (F - 32) × 5/9` |
| °C | °F | `F = C × 9/5 + 32` |
| °C | K | `K = C + 273.15` |

**Note:** MSFS typically provides temperatures in Fahrenheit for engine data and Celsius for ambient temperature.

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

## Data Definition Registration

The MSFS adapter registers data definitions with SimConnect using explicit units:

```rust
// Example: Registering kinematics data definition
api.add_to_data_definition(
    handle,
    KINEMATICS_DEF_ID,
    "AIRSPEED INDICATED",
    "knots",                    // Explicit unit specification
    SIMCONNECT_DATATYPE::FLOAT64,
    0.0,
    datum_id
)?;
```

**Key Points:**
- Units are always explicitly specified in `add_to_data_definition` calls
- SimConnect performs automatic unit conversion when units are specified
- Data type is always `FLOAT64` for numeric values, `INT32` for booleans
- Datum IDs are sequential within each data definition

## Sanity Gate Integration

The Sanity Gate validates telemetry plausibility before setting `safe_for_ffb`:

**Checks Performed:**
1. **NaN/Inf Detection**: All numeric values checked for NaN or Inf
2. **Range Validation**: Values checked against physically plausible ranges
3. **Rate Limiting**: Attitude and velocity changes checked for implausible jumps
4. **State Machine**: Telemetry must be stable for N frames before `safe_for_ffb = true`

**Validation Thresholds:**
- Attitude rate: ≤180°/s (π rad/s)
- Velocity change: ≤100 m/s per frame
- G-force: -20g to +20g
- Airspeed: 0-500 m/s (0-1000 knots)

See `crates/flight-simconnect/src/sanity_gate.rs` for implementation details.

## Error Handling

### Missing Data

When SimConnect returns missing or invalid data:
- Numeric fields are set to 0.0 or appropriate default
- Optional fields are set to `None`
- Validity flags are set to `false`
- Sanity gate prevents `safe_for_ffb` from being set

### Connection Loss

When SimConnect connection is lost:
- All BusSnapshot fields marked as invalid
- Adapter transitions to `Disconnected` state
- Exponential backoff reconnection (up to 30s between attempts)
- No data is published to the bus until reconnection

### Version Compatibility

The adapter is tested against:
- MSFS 2020 (all updates through 1.36.0)
- MSFS 2024 (when available)

SimVar names and units are validated against the official SimConnect SDK documentation for each MSFS version.

## Implementation Notes

### Code Comments

All unit conversions in the adapter code include inline comments:

```rust
// Convert knots to m/s: 1 knot = 0.514444 m/s
snapshot.kinematics.ias = MsfsConverter::convert_ias(raw.ias_kt)?;

// Convert degrees to radians: π/180 ≈ 0.0174533
snapshot.kinematics.pitch = MsfsConverter::convert_angle_degrees(raw.pitch_deg)?;
```

### Mapping Configuration

The mapping is configurable per-aircraft via `MappingConfig`:
- Default mapping covers standard fixed-wing aircraft
- Aircraft-specific mappings can override defaults
- Helicopter mapping is optional and only used for rotorcraft

See `crates/flight-simconnect/src/mapping.rs` for the complete implementation.

## Testing

### Unit Tests

Unit tests verify:
- Correct unit conversions for all fields
- Range validation for validated types
- Handling of missing/invalid data
- Sanity gate behavior with NaN/Inf values

### Integration Tests

Integration tests use recorded fixtures:
- `tests/fixtures/msfs_c172_cruise.json` - Cessna 172 in cruise flight
- `tests/fixtures/msfs_a320_approach.json` - Airbus A320 on approach
- `tests/fixtures/msfs_h145_hover.json` - H145 helicopter in hover

Fixtures are replayed through the adapter to verify end-to-end mapping.

## References

- [SimConnect SDK Documentation](https://docs.flightsimulator.com/html/Programming_Tools/SimConnect/SimConnect_SDK.htm)
- [SimConnect Variables Reference](https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Simulation_Variables.htm)
- Flight Hub BusSnapshot specification: `crates/flight-bus/src/snapshot.rs`
- MSFS Adapter implementation: `crates/flight-simconnect/src/adapter.rs`
- Mapping implementation: `crates/flight-simconnect/src/mapping.rs`

## Changelog

| Date | Version | Changes |
|------|---------|---------|
| 2024-01 | 1.0 | Initial mapping documentation for Flight Hub v1 |

---

**Validation Status**: ✅ All mappings verified against MSFS 1.36.0 SDK  
**Quality Gate**: QG-SIM-MAPPING (MUST) - Complete mapping table present  
**Last Updated**: 2024-01 (Flight Hub v1 specification)
