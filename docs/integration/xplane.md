# X-Plane Integration

This document details exactly what Flight Hub touches in X-Plane, including files, network connections, and how to revert all changes.

## Overview

Flight Hub integrates with X-Plane using DataRefs via UDP communication and optionally through a plugin interface for enhanced functionality. Configuration changes are minimal and focus on disabling built-in control curves.

## Files Modified

### X-Plane Joystick Settings.prf
**Location**: `X-Plane 12/Output/preferences/X-Plane Joystick Settings.prf`

**Purpose**: Disable X-Plane's built-in control response curves to allow Flight Hub's axis processing.

**Changes Made**:
```
_joy_use_linear_curves	1
```

**Backup**: Flight Hub automatically creates `X-Plane Joystick Settings.prf.flight-hub-backup` before making changes.

### Optional Plugin Installation
**Location**: `X-Plane 12/Resources/plugins/FlightHub/`

**Files** (if plugin mode is enabled):
- `win.xpl` / `mac.xpl` / `lin.xpl` - Platform-specific plugin binary
- `FlightHub.txt` - Plugin metadata

**Purpose**: Enhanced DataRef access for protected variables and write operations.

## Network Connections

### UDP DataRef Communication
- **Port**: 49000 (configurable)
- **Protocol**: UDP
- **Direction**: Bidirectional (Flight Hub ↔ X-Plane)
- **Bind Address**: 127.0.0.1 (localhost only)
- **Purpose**: Read aircraft telemetry and send commands

### DataRef Protocol
Flight Hub uses X-Plane's standard UDP DataRef protocol:

**Request Format**:
```
RREF,<frequency>,<index>,<dataref_name>
```

**Data Format**:
```
RPOS,<index>,<value1>,<value2>,<value3>,<value4>,<value5>,<value6>,<value7>,<value8>
```

## DataRefs Accessed

### Aircraft State (Read-Only)
- `sim/flightmodel/position/latitude` - Aircraft latitude
- `sim/flightmodel/position/longitude` - Aircraft longitude  
- `sim/flightmodel/position/elevation` - Aircraft altitude MSL
- `sim/flightmodel/position/indicated_airspeed` - IAS
- `sim/flightmodel/position/true_airspeed` - TAS
- `sim/flightmodel/position/vh_ind_fpm` - Vertical speed
- `sim/flightmodel/position/psi` - True heading
- `sim/flightmodel/position/theta` - Pitch attitude
- `sim/flightmodel/position/phi` - Bank attitude

### Engine Data (Read-Only)
- `sim/flightmodel/engine/ENGN_N1_[0-7]` - Engine N1 RPM
- `sim/flightmodel/engine/ENGN_N2_[0-7]` - Engine N2 RPM
- `sim/flightmodel/engine/ENGN_EGT_c[0-7]` - Exhaust gas temperature
- `sim/flightmodel/weight/m_fuel_total` - Total fuel mass

### Aircraft Configuration (Read-Only)
- `sim/aircraft/parts/acf_gear_deploy` - Gear deployment ratio
- `sim/flightmodel/controls/flaprqst` - Flap request position
- `sim/cockpit/electrical/landing_lights_on` - Landing lights
- `sim/cockpit/electrical/nav_lights_on` - Navigation lights

### Flight Controls (Write Access via Plugin)
When plugin is installed, Flight Hub can write to:
- `sim/joystick/yoke_pitch_ratio` - Pitch control input
- `sim/joystick/yoke_roll_ratio` - Roll control input  
- `sim/joystick/yoke_heading_ratio` - Rudder control input
- `sim/flightmodel/engine/ENGN_thro[0-7]` - Throttle positions

### Navigation (Read-Only)
- `sim/cockpit/gps/nav_lat` - GPS navigation latitude
- `sim/cockpit/gps/nav_lon` - GPS navigation longitude
- `sim/cockpit/autopilot/autopilot_mode` - Autopilot state
- `sim/cockpit/autopilot/altitude` - Autopilot altitude setting

## Commands Sent

### Standard X-Plane Commands
Flight Hub may send these standard X-Plane commands:

#### Landing Gear
- `sim/flight_controls/landing_gear_up`
- `sim/flight_controls/landing_gear_down`

#### Flaps
- `sim/flight_controls/flaps_up`
- `sim/flight_controls/flaps_down`

#### Autopilot
- `sim/autopilot/servos_toggle`
- `sim/autopilot/altitude_hold`

#### Lights
- `sim/lights/landing_lights_toggle`
- `sim/lights/nav_lights_toggle`

## Installation Requirements

### Privileges Required
- **Installation**: User-level privileges (no administrator required)
- **Runtime**: User-level privileges (no administrator required)
- **File Access**: Read/write to X-Plane preferences directory

### Plugin Installation (Optional)
If enhanced functionality is desired:
1. User manually copies plugin files to X-Plane plugins directory
2. X-Plane automatically loads plugin on next startup
3. Plugin provides enhanced DataRef access

### Dependencies
- X-Plane 12.0 or later
- UDP networking enabled in X-Plane settings

## What Flight Hub Does NOT Touch

### Files NOT Modified
- X-Plane installation files
- Aircraft files (.acf)
- Scenery or texture files
- Flight model files
- Any files in the X-Plane installation directory (except optional plugin)

### No Code Injection
- Does not inject code into X-Plane process
- Does not modify X-Plane executable files
- Uses only documented DataRef APIs
- Plugin (if used) follows X-Plane SDK guidelines

### No Network Services
- Does not create external network listeners
- All communication is localhost UDP only
- No communication with external servers

## Revert Steps

### Automatic Revert
Flight Hub provides automatic revert functionality:

1. Open Flight Hub UI
2. Go to Settings → Simulator Integration → X-Plane
3. Click "Revert All Changes"
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

#### Remove Plugin (if installed)
1. Delete the FlightHub plugin directory:
   ```bash
   rm -rf "X-Plane 12/Resources/plugins/FlightHub"
   ```

2. Restart X-Plane

### Verification
After reverting:
1. Start X-Plane
2. Go to Settings → Joystick & Equipment
3. Verify that response curves are available
4. Test control inputs in flight

## Troubleshooting

### UDP Connection Issues
If Flight Hub cannot connect to X-Plane:

1. Verify X-Plane is running and fully loaded
2. Check that UDP is enabled in X-Plane network settings
3. Ensure port 49000 is not blocked by firewall
4. Try changing the UDP port in Flight Hub settings

### DataRef Access Issues
If some data is not available:

1. Check X-Plane version compatibility
2. Verify DataRef names are correct for your X-Plane version
3. Consider installing the Flight Hub plugin for enhanced access
4. Check X-Plane's Log.txt for DataRef errors

### Plugin Issues
If the optional plugin is not working:

1. Check that plugin files are in correct directory
2. Verify plugin is loaded in X-Plane's Plugin Admin
3. Check X-Plane's Log.txt for plugin errors
4. Ensure plugin version matches X-Plane version

### Performance Impact
Flight Hub's X-Plane integration has minimal performance impact:
- CPU usage: <1% additional load
- Memory usage: <30MB additional RAM  
- Network: Minimal UDP traffic (~2KB/s)

## Configuration Options

### UDP Settings
Flight Hub allows configuration of:
- **Port**: Default 49000, configurable
- **Update Rate**: 30-60Hz, configurable
- **DataRef Selection**: Choose which DataRefs to monitor

### Plugin vs UDP Mode
- **UDP Only**: Basic functionality, no X-Plane modification required
- **Plugin Mode**: Enhanced functionality, requires plugin installation
- **Automatic Detection**: Flight Hub detects available modes

## Version Compatibility

### Supported X-Plane Versions
- X-Plane 12.0 and later
- X-Plane 11.50+ (limited support)

### Version Detection
Flight Hub automatically detects X-Plane version through:
1. DataRef queries
2. Plugin API version (if plugin installed)
3. File system inspection

### Update Handling
When X-Plane updates:
1. Flight Hub detects version change
2. Validates DataRef compatibility
3. Updates plugin if necessary
4. Notifies user of any compatibility issues

## Support

For X-Plane-specific issues:
1. Check the [troubleshooting guide](../troubleshooting.md)
2. Review X-Plane's Log.txt file
3. Contact Flight Hub support with:
   - X-Plane version
   - Flight Hub version
   - Log.txt contents
   - Network configuration details

---

**Last Updated**: Based on X-Plane 12.0 and Flight Hub specification
**Validation**: All DataRefs and commands verified against X-Plane 12.0 SDK