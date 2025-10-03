# Microsoft Flight Simulator (MSFS) Integration

This document details exactly what Flight Hub touches in Microsoft Flight Simulator, including files, network connections, and how to revert all changes.

## Overview

Flight Hub integrates with MSFS using the official SimConnect SDK and makes minimal configuration changes to disable built-in control curves for optimal axis processing.

## Files Modified

### UserCfg.opt
**Location**: `%APPDATA%\Microsoft Flight Simulator\UserCfg.opt`

**Purpose**: Disable MSFS built-in control response curves to allow Flight Hub's axis processing to work optimally.

**Changes Made**:
```ini
[CONTROLS]
UseLinearCurves=1
DisableNonLinearControls=1
```

**Backup**: Flight Hub automatically creates `UserCfg.opt.flight-hub-backup` before making changes.

## Network Connections

### SimConnect
- **Protocol**: TCP (dynamic port assignment by Windows)
- **Direction**: Outbound from Flight Hub to MSFS
- **Purpose**: Read aircraft telemetry and send input events
- **Data Accessed**:
  - Aircraft position, attitude, and velocity
  - Engine parameters (RPM, temperature, fuel flow)
  - Aircraft configuration (gear, flaps, lights)
  - Navigation data (GPS, autopilot status)
  - Environmental data (weather, time)

### Input Events (MSFS SDK)
- **Method**: SimConnect Input Events API
- **Purpose**: Send control inputs to aircraft systems
- **Scope**: Standard published input events only
- **Examples**: `GEAR_TOGGLE`, `FLAPS_SET`, `AP_MASTER`

## Variables Accessed

### SimConnect Variables (Read-Only)
Flight Hub reads the following standard SimConnect variables:

#### Aircraft State
- `PLANE_LATITUDE` - Aircraft latitude position
- `PLANE_LONGITUDE` - Aircraft longitude position  
- `PLANE_ALTITUDE` - Aircraft altitude MSL
- `AIRSPEED_INDICATED` - Indicated airspeed
- `AIRSPEED_TRUE` - True airspeed
- `VERTICAL_SPEED` - Vertical speed
- `PLANE_HEADING_DEGREES_TRUE` - True heading
- `PLANE_PITCH_DEGREES` - Pitch attitude
- `PLANE_BANK_DEGREES` - Bank attitude

#### Engine Data
- `GENERAL_ENG_RPM:1` - Engine 1 RPM
- `GENERAL_ENG_RPM:2` - Engine 2 RPM (if applicable)
- `ENG_EXHAUST_GAS_TEMPERATURE:1` - EGT Engine 1
- `FUEL_TOTAL_QUANTITY` - Total fuel quantity

#### Aircraft Configuration
- `GEAR_POSITION` - Landing gear position
- `FLAPS_HANDLE_PERCENT` - Flap handle position
- `LIGHT_LANDING` - Landing light state
- `LIGHT_NAV` - Navigation light state

#### Navigation
- `GPS_WP_NEXT_LAT` - Next waypoint latitude
- `GPS_WP_NEXT_LON` - Next waypoint longitude
- `AUTOPILOT_MASTER` - Autopilot master state
- `AUTOPILOT_ALTITUDE_LOCK` - Altitude hold state

### No Write Access to Variables
Flight Hub does **NOT** write to any SimConnect variables directly. All control inputs are sent through the Input Events system.

## Registry Access

### Read-Only Registry Access
Flight Hub may read the following registry keys to locate MSFS installation:

**Windows Registry Keys (Read-Only)**:
- `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Microsoft Games\Flight Simulator`
- `HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Microsoft Games\Flight Simulator`

**Purpose**: Locate MSFS installation directory and configuration files.

## Installation Requirements

### Privileges Required
- **Installation**: User-level privileges (no administrator required)
- **Runtime**: User-level privileges (no administrator required)
- **File Access**: Read/write to user's MSFS configuration directory

### Dependencies
- Microsoft Visual C++ Redistributable (automatically installed)
- SimConnect SDK (distributed with MSFS)

## What Flight Hub Does NOT Touch

### Files NOT Modified
- MSFS installation files
- Aircraft configuration files
- Scenery or texture files
- Any files in the MSFS installation directory
- Windows system files

### No Code Injection
- Does not inject DLLs into MSFS process
- Does not modify MSFS executable files
- Uses only official SimConnect APIs
- No memory patching or hooking

### No Network Services
- Does not create network listeners
- Does not communicate with external servers
- All communication is local (SimConnect)

## Revert Steps

### Automatic Revert
Flight Hub provides automatic revert functionality:

1. Open Flight Hub UI
2. Go to Settings → Simulator Integration → MSFS
3. Click "Revert All Changes"
4. Restart MSFS

### Manual Revert

If automatic revert is not available:

1. **Restore UserCfg.opt**:
   ```cmd
   cd "%APPDATA%\Microsoft Flight Simulator"
   copy UserCfg.opt.flight-hub-backup UserCfg.opt
   ```

2. **Or manually edit UserCfg.opt**:
   - Open `%APPDATA%\Microsoft Flight Simulator\UserCfg.opt`
   - Remove or comment out these lines:
     ```ini
     UseLinearCurves=1
     DisableNonLinearControls=1
     ```

3. **Restart MSFS** to apply changes

### Verification
After reverting:
1. Start MSFS
2. Go to Options → Controls
3. Verify that sensitivity curves are available and functional
4. Test control inputs in flight

## Troubleshooting

### SimConnect Connection Issues
If Flight Hub cannot connect to MSFS:

1. Ensure MSFS is running and fully loaded
2. Check Windows Firewall settings
3. Verify SimConnect is enabled in MSFS settings
4. Restart both MSFS and Flight Hub

### Control Curve Issues
If controls feel different after Flight Hub installation:

1. Check that Flight Hub axis processing is configured correctly
2. Verify MSFS curves are properly disabled
3. Use Flight Hub's curve editor to adjust response
4. Consider reverting MSFS changes temporarily for comparison

### Performance Impact
Flight Hub's MSFS integration has minimal performance impact:
- CPU usage: <1% additional load
- Memory usage: <50MB additional RAM
- Network: Minimal SimConnect traffic (~1KB/s)

## Support

For MSFS-specific issues:
1. Check the [troubleshooting guide](../troubleshooting.md)
2. Review MSFS logs in `%LOCALAPPDATA%\Packages\Microsoft.FlightSimulator_*\LocalState\`
3. Contact Flight Hub support with:
   - MSFS version
   - Flight Hub version
   - UserCfg.opt contents
   - SimConnect error messages (if any)

## Version Compatibility

### Supported MSFS Versions
- MSFS 2020 (all updates)
- MSFS 2024 (when available)

### Version Detection
Flight Hub automatically detects MSFS version and applies appropriate configuration changes. Version-specific configurations are maintained in the writers system.

### Update Handling
When MSFS updates:
1. Flight Hub detects version change
2. Runs verification tests
3. Applies any necessary configuration updates
4. Notifies user of any issues

---

**Last Updated**: Based on MSFS version 1.36.0 and Flight Hub specification
**Validation**: All file paths and registry keys verified on Windows 10/11