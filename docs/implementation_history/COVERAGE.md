# MSFS SimConnect Adapter - Coverage Matrix and Compliance

## Variable Coverage Matrix

This document lists the SimConnect variables and events that are normalized by the Flight Hub MSFS adapter, fulfilling requirement GI-01.

### Kinematics Variables

| Flight Hub Field | SimConnect Variable | Units | Coverage |
|------------------|-------------------|-------|----------|
| ias | AIRSPEED INDICATED | knots | ✅ Full |
| tas | AIRSPEED TRUE | knots | ✅ Full |
| ground_speed | GROUND VELOCITY | knots | ✅ Full |
| aoa | INCIDENCE ALPHA | degrees | ✅ Full |
| sideslip | INCIDENCE BETA | degrees | ✅ Full |
| bank | ATTITUDE BANK DEGREES | degrees | ✅ Full |
| pitch | ATTITUDE PITCH DEGREES | degrees | ✅ Full |
| heading | ATTITUDE HEADING DEGREES | degrees | ✅ Full |
| g_force | G FORCE | gforce | ✅ Full |
| g_lateral | G FORCE LATERAL | gforce | ✅ Full |
| g_longitudinal | G FORCE LONGITUDINAL | gforce | ✅ Full |
| mach | AIRSPEED MACH | mach | ✅ Full |
| vertical_speed | VERTICAL SPEED | feet per minute | ✅ Full |

### Aircraft Configuration Variables

| Flight Hub Field | SimConnect Variable | Units | Coverage |
|------------------|-------------------|-------|----------|
| gear_nose | GEAR CENTER POSITION | bool | ✅ Full |
| gear_left | GEAR LEFT POSITION | bool | ✅ Full |
| gear_right | GEAR RIGHT POSITION | bool | ✅ Full |
| flaps | FLAPS HANDLE PERCENT | percent | ✅ Full |
| spoilers | SPOILERS HANDLE POSITION | percent | ✅ Full |
| ap_master | AUTOPILOT MASTER | bool | ✅ Full |
| ap_altitude_hold | AUTOPILOT ALTITUDE LOCK | bool | ✅ Full |
| ap_heading_hold | AUTOPILOT HEADING LOCK | bool | ✅ Full |
| ap_speed_hold | AUTOPILOT AIRSPEED HOLD | bool | ✅ Full |

### Engine Variables

| Flight Hub Field | SimConnect Variable | Units | Coverage |
|------------------|-------------------|-------|----------|
| running | GENERAL ENG COMBUSTION:1 | bool | ✅ Full |
| rpm | GENERAL ENG RPM:1 | percent | ✅ Full |
| manifold_pressure | RECIP ENG MANIFOLD PRESSURE:1 | inHg | ✅ Piston Only |
| egt | GENERAL ENG EXHAUST GAS TEMPERATURE:1 | celsius | ✅ Full |
| fuel_flow | GENERAL ENG FUEL FLOW GPH:1 | gallons/hour | ✅ Full |
| oil_pressure | GENERAL ENG OIL PRESSURE:1 | psi | ✅ Full |

### Environment Variables

| Flight Hub Field | SimConnect Variable | Units | Coverage |
|------------------|-------------------|-------|----------|
| altitude | INDICATED ALTITUDE | feet | ✅ Full |
| pressure_altitude | PRESSURE ALTITUDE | feet | ✅ Full |
| oat | AMBIENT TEMPERATURE | celsius | ✅ Full |
| wind_speed | AMBIENT WIND VELOCITY | knots | ✅ Full |
| wind_direction | AMBIENT WIND DIRECTION | degrees | ✅ Full |
| visibility | AMBIENT VISIBILITY | statute miles | ✅ Full |

### Navigation Variables

| Flight Hub Field | SimConnect Variable | Units | Coverage |
|------------------|-------------------|-------|----------|
| latitude | PLANE LATITUDE | degrees | ✅ Full |
| longitude | PLANE LONGITUDE | degrees | ✅ Full |
| ground_track | GPS GROUND TRUE TRACK | degrees | ✅ Full |
| distance_to_dest | GPS WP DISTANCE | nautical miles | ⚠️ GPS Only |
| active_waypoint | GPS WP NEXT ID | string | ⚠️ GPS Only |

### Events Coverage

| Event Category | SimConnect Events | Coverage |
|----------------|------------------|----------|
| Flight Controls | AXIS_ELEVATOR_SET, AXIS_AILERONS_SET, AXIS_RUDDER_SET | ✅ Full |
| Engine Controls | AXIS_THROTTLE_SET, AXIS_MIXTURE_SET, AXIS_PROPELLER_SET | ✅ Full |
| Landing Gear | GEAR_TOGGLE, GEAR_UP, GEAR_DOWN | ✅ Full |
| Flaps | FLAPS_INCR, FLAPS_DECR, FLAPS_SET | ✅ Full |
| Autopilot | AP_MASTER, AP_ALT_HOLD, AP_HDG_HOLD, AP_SPD_HOLD | ✅ Full |
| Lights | TOGGLE_NAV_LIGHTS, TOGGLE_BEACON_LIGHTS, etc. | ✅ Full |
| System Events | AircraftLoaded, SimStart, SimStop, Pause | ✅ Full |

### Aircraft-Specific Mappings

| Aircraft Type | Special Variables | Coverage |
|---------------|------------------|----------|
| General Aviation | Standard variables + mixture, propeller | ✅ Full |
| Jets | Standard variables + spoilers, thrust reversers | ✅ Full |
| Helicopters | Collective, cyclic, pedals, rotor RPM | ✅ Full |
| Turboprops | Standard variables + condition levers | ⚠️ Partial |

## Redistribution Compliance (LEG-01)

### What Flight Hub Touches

**SimConnect Integration:**
- Uses dynamic loading of `SimConnect.dll` from MSFS installation
- No redistribution of Microsoft libraries required
- Uses only public, documented SimConnect API
- No code injection into MSFS processes

**Files Accessed:**
- None - all communication via SimConnect API
- No modification of MSFS installation files
- No creation of files in MSFS directories

**Network/IPC:**
- Uses SimConnect's named pipe communication
- No direct network access to MSFS
- All communication through documented interfaces

**Registry/System:**
- No registry modifications
- No system service installation
- Uses standard Windows API for library loading

### How to Revert

Since Flight Hub makes no persistent changes to the MSFS installation:

1. **Complete Removal:** Simply uninstall Flight Hub
2. **No MSFS Changes:** No changes to revert in MSFS
3. **No File Cleanup:** No files created in MSFS directories
4. **No Registry Cleanup:** No registry entries to remove

### Compliance Statement

Flight Hub's MSFS integration is designed to be:
- **Non-invasive:** No modification of MSFS files or installation
- **Reversible:** Complete removal with no traces left
- **Compliant:** Uses only documented, public APIs
- **Redistributable:** No Microsoft libraries included in distribution

This approach ensures compliance with Microsoft's licensing terms and provides users with a clean, safe integration that can be completely removed without affecting their MSFS installation.

## Testing and Validation

### Integration Tests

The adapter includes comprehensive integration tests that validate:

1. **Variable Mapping:** All mapped variables produce valid, normalized output
2. **Event Handling:** All supported events are properly transmitted
3. **Aircraft Detection:** Aircraft identification works across different aircraft types
4. **Error Handling:** Graceful handling of connection loss and errors
5. **Performance:** Telemetry publishing meets 30-60Hz requirements

### Fixture-Based Testing

Session fixtures enable testing without live MSFS connection:

1. **Recorded Sessions:** Real MSFS sessions recorded for playback
2. **Golden Tests:** Expected outputs validated against recorded data
3. **Regression Testing:** Automated validation of adapter behavior
4. **Coverage Validation:** Ensures all mapped variables are tested

### Continuous Integration

CI pipeline validates:

1. **Coverage Matrix:** All documented variables are implemented
2. **Golden Test Compliance:** No regressions in variable mapping
3. **Performance Requirements:** Telemetry rates meet specifications
4. **Cross-Platform:** Windows compatibility validation

## Version Compatibility

| MSFS Version | SimConnect Version | Compatibility | Notes |
|--------------|-------------------|---------------|-------|
| MSFS 2020 | 0.4.x | ✅ Full | Primary target |
| MSFS 2024 | 0.5.x | ✅ Full | Input Events supported |
| FSX | Legacy | ⚠️ Limited | Basic variables only |

## Update Policy

When MSFS updates change SimConnect behavior:

1. **Automatic Detection:** Adapter detects version changes
2. **Graceful Degradation:** Falls back to compatible variable set
3. **Update Notifications:** Users notified of compatibility issues
4. **Rapid Response:** Updates released within 48 hours of MSFS updates

This ensures continuous compatibility while maintaining the documented coverage matrix.