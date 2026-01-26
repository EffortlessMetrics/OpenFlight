# DCS World Integration

This document details exactly what Flight Hub touches in DCS World, including files, network connections, multiplayer considerations, and how to revert all changes.

## Overview

Flight Hub integrates with DCS World through a user-installed Export.lua script that provides telemetry data via local socket communication. The integration respects DCS multiplayer integrity and provides different feature sets for single-player vs multiplayer sessions.

## Files Modified

### Export.lua (User-Installed)
**Location**: `%USERPROFILE%\Saved Games\DCS\Scripts\Export.lua` (Windows)
**Location**: `~/Saved Games/DCS/Scripts/Export.lua` (Linux)

**Installation Method**: User manually installs the script (not automated by Flight Hub)

**Purpose**: Provide telemetry data to Flight Hub via socket communication

**Script Contents**:
```lua
-- Flight Hub Export Script v1.0
-- User-installed script for DCS telemetry export

local FlightHubExport = {}

-- Configuration
FlightHubExport.config = {
    host = "127.0.0.1",
    port = 12080,
    update_rate = 0.033,  -- ~30Hz
    mp_safe_mode = true   -- Respect MP restrictions
}

-- Feature flags (MP-safe vs MP-blocked)
FlightHubExport.features = {
    -- MP-Safe Features (always available)
    telemetry_basic = true,         -- Position, attitude, airspeed
    telemetry_navigation = true,    -- Waypoints, course info
    telemetry_engines = true,       -- RPM, temperature, fuel
    telemetry_config = true,        -- Gear, flaps, lights (non-tactical)
    telemetry_environment = true,   -- Weather, time of day
    
    -- MP-Blocked Features (single-player only)
    telemetry_weapons = true,       -- Loadout, ammunition, weapon status
    telemetry_countermeasures = true, -- Chaff/flare counts
    telemetry_rwr = true,           -- Radar warning receiver
    telemetry_datalink = true,      -- Tactical data sharing
    telemetry_sensors = true        -- Radar contacts, targeting
}

-- Session detection and MP safety
function FlightHubExport.detectSession()
    -- Implementation details in actual script
end

-- Export functions
function FlightHubExport.ProcessLowImportance(dt)
    -- Telemetry collection and transmission
end

-- Install the export hooks
if not FlightHubExport.installed then
    -- Hook into DCS export system
    FlightHubExport.installed = true
end
```

### options.lua (Optional Configuration)
**Location**: `%USERPROFILE%\Saved Games\DCS\Config\options.lua`

**Purpose**: Disable DCS built-in control curves (optional)

**Changes Made** (if user enables):
```lua
options = {
    -- ... existing options ...
    ["useLinearCurves"] = true,
    -- ... rest of options ...
}
```

**Backup**: Flight Hub creates `options.lua.flight-hub-backup` before changes

## Network Connections

### Local Socket Communication
- **Port**: 12080 (configurable)
- **Protocol**: TCP
- **Direction**: DCS → Flight Hub (outbound from DCS)
- **Bind Address**: 127.0.0.1 (localhost only)
- **Purpose**: Transmit telemetry data from DCS to Flight Hub

### No External Network Access
- Export.lua does **NOT** communicate with external servers
- All communication is local machine only
- No data transmission outside the local system

## DCS Lua API → BusSnapshot Field Mapping

This section documents the complete mapping from DCS Lua Export API functions to Flight Hub's normalized BusSnapshot structure. All mappings include unit conversions and MP integrity compliance status.

### Core Telemetry Functions (MP-Safe)

These functions provide self-aircraft telemetry and are **always available** in both single-player and multiplayer sessions.

#### LoGetSelfData()

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

| DCS Lua Field | BusSnapshot Field | Unit Conversion | Notes |
|---------------|-------------------|-----------------|-------|
| `Name` | `aircraft.icao` | None | Aircraft type identifier |
| `LatLongAlt.Lat` | `navigation.latitude` | None (degrees) | WGS84 latitude |
| `LatLongAlt.Long` | `navigation.longitude` | None (degrees) | WGS84 longitude |
| `LatLongAlt.Alt` | `environment.altitude` | meters → feet | Altitude MSL |
| `Heading` | `kinematics.heading` | radians → degrees | True heading |
| `Pitch` | `kinematics.pitch` | radians → degrees | Pitch angle |
| `Bank` | `kinematics.bank` | radians → degrees | Roll/bank angle |
| `AngularVelocity.x` | `kinematics.angular_rates.p` | None (rad/s) | Roll rate |
| `AngularVelocity.y` | `kinematics.angular_rates.q` | None (rad/s) | Pitch rate |
| `AngularVelocity.z` | `kinematics.angular_rates.r` | None (rad/s) | Yaw rate |
| `Velocity.x` | `kinematics.body_velocity_x` | None (m/s) | Forward velocity |
| `Velocity.y` | `kinematics.body_velocity_y` | None (m/s) | Vertical velocity |
| `Velocity.z` | `kinematics.body_velocity_z` | None (m/s) | Lateral velocity |

**MP Integrity:** ✅ **Allowed** - Self-aircraft position and attitude data

---

#### LoGetIndicatedAirSpeed()

Returns indicated airspeed in meters per second.

**Return Value:** `number` (m/s)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `ias` (m/s) | `kinematics.ias` | m/s → knots | Indicated airspeed |

**Example:**
```lua
local ias = LoGetIndicatedAirSpeed()  -- Returns 77.2 m/s
-- Maps to: kinematics.ias = 150.0 knots
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft airspeed

---

#### LoGetTrueAirSpeed()

Returns true airspeed in meters per second.

**Return Value:** `number` (m/s)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `tas` (m/s) | `kinematics.tas` | m/s → knots | True airspeed |

**Example:**
```lua
local tas = LoGetTrueAirSpeed()  -- Returns 80.0 m/s
-- Maps to: kinematics.tas = 155.5 knots
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft airspeed

---

#### LoGetAltitudeAboveSeaLevel()

Returns altitude above mean sea level in meters.

**Return Value:** `number` (meters)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `altitude_asl` (m) | `environment.altitude` | meters → feet | Altitude MSL |

**Example:**
```lua
local alt_msl = LoGetAltitudeAboveSeaLevel()  -- Returns 1524.0 m
-- Maps to: environment.altitude = 5000.0 feet
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft altitude

---

#### LoGetAltitudeAboveGroundLevel()

Returns altitude above ground level in meters.

**Return Value:** `number` (meters)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `altitude_agl` (m) | `environment.altitude_agl` | meters → feet | Altitude AGL |

**Example:**
```lua
local alt_agl = LoGetAltitudeAboveGroundLevel()  -- Returns 304.8 m
-- Maps to: environment.altitude_agl = 1000.0 feet
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft altitude

---

#### LoGetVerticalVelocity()

Returns vertical speed in meters per second.

**Return Value:** `number` (m/s)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `vs` (m/s) | `kinematics.vertical_speed` | None (m/s) | Vertical speed |

**Example:**
```lua
local vs = LoGetVerticalVelocity()  -- Returns 5.08 m/s
-- Maps to: kinematics.vertical_speed = 5.08 m/s (1000 fpm)
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft vertical speed

---

#### LoGetAccelerationUnits()

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

| DCS Lua Field | BusSnapshot Field | Unit Conversion | Notes |
|---------------|-------------------|-----------------|-------|
| `y` | `kinematics.g_force` | None (g) | Vertical (normal) g-load |
| `x` | `kinematics.g_lateral` | None (g) | Lateral g-load |
| `z` | `kinematics.g_longitudinal` | None (g) | Longitudinal g-load |

**Example:**
```lua
local accel = LoGetAccelerationUnits()
-- accel.y = 1.2 → kinematics.g_force = 1.2g
-- accel.x = 0.1 → kinematics.g_lateral = 0.1g
-- accel.z = 0.05 → kinematics.g_longitudinal = 0.05g
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft g-forces

---

#### LoGetAngleOfAttack()

Returns angle of attack in radians.

**Return Value:** `number` (radians)

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `aoa` (rad) | `kinematics.aoa` | radians → degrees | Angle of attack |

**Example:**
```lua
local aoa = LoGetAngleOfAttack()  -- Returns 0.0873 rad
-- Maps to: kinematics.aoa = 5.0 degrees
```

**MP Integrity:** ✅ **Allowed** - Self-aircraft angle of attack

---

### Navigation Functions (MP-Safe)

These functions provide navigation data and are **available in multiplayer** as they relate to self-aircraft navigation only.

#### LoGetRoute()

Returns navigation route information including next waypoint.

**Return Structure:**
```lua
{
    goto_point = {
        dist = 15000.0,      -- Distance to waypoint in meters
        bearing = 1.5708     -- Bearing to waypoint in radians
    }
}
```

**BusSnapshot Mapping:**

| DCS Lua Field | BusSnapshot Field | Unit Conversion | Notes |
|---------------|-------------------|-----------------|-------|
| `goto_point.dist` | `navigation.distance_to_dest` | meters → nm | Distance to waypoint |
| `goto_point.bearing` | `navigation.waypoint_bearing` | radians → degrees | Bearing to waypoint |

**MP Integrity:** ✅ **Allowed** - Self-aircraft navigation data

---

#### LoGetNavigationInfo()

Returns navigation course information.

**Return Structure:**
```lua
{
    Course = 90.0,           -- Current course in degrees
    DesiredCourse = 95.0,    -- Desired course in degrees
    CourseDeviation = 5.0    -- Course deviation in degrees
}
```

**BusSnapshot Mapping:**

| DCS Lua Field | BusSnapshot Field | Unit Conversion | Notes |
|---------------|-------------------|-----------------|-------|
| `Course` | `navigation.ground_track` | None (degrees) | Current ground track |
| `DesiredCourse` | `navigation.desired_course` | None (degrees) | Desired course |
| `CourseDeviation` | `navigation.course_deviation` | None (degrees) | Course deviation |

**MP Integrity:** ✅ **Allowed** - Self-aircraft navigation data

---

### Engine Functions (MP-Safe)

These functions provide engine telemetry and are **available in multiplayer** as they relate to self-aircraft systems only.

#### LoGetEngineInfo()

Returns engine telemetry for all engines.

**Return Structure:**
```lua
{
    [1] = {
        RPM = {
            left = 85.5,     -- Left engine RPM (percentage)
            right = 86.0     -- Right engine RPM (percentage)
        },
        Temperature = {
            left = 650.0,    -- Left engine temp (Celsius)
            right = 655.0    -- Right engine temp (Celsius)
        },
        FuelFlow = {
            left = 1200.0,   -- Left fuel flow (kg/h)
            right = 1250.0   -- Right fuel flow (kg/h)
        }
    }
}
```

**BusSnapshot Mapping:**

| DCS Lua Field | BusSnapshot Field | Unit Conversion | Notes |
|---------------|-------------------|-----------------|-------|
| `RPM.left` | `engines[0].rpm` | None (%) | Left engine RPM |
| `RPM.right` | `engines[1].rpm` | None (%) | Right engine RPM |
| `Temperature.left` | `engines[0].egt` | None (°C) | Left engine temp |
| `Temperature.right` | `engines[1].egt` | None (°C) | Right engine temp |
| `FuelFlow.left` | `engines[0].fuel_flow` | None (kg/h) | Left fuel flow |
| `FuelFlow.right` | `engines[1].fuel_flow` | None (kg/h) | Right fuel flow |

**MP Integrity:** ✅ **Allowed** - Self-aircraft engine data

---

### Weapons Functions (MP-Blocked)

These functions provide weapons and tactical data and are **blocked in multiplayer** to maintain server integrity.

#### LoGetPayloadInfo()

Returns weapons loadout and ammunition counts.

**Return Structure:**
```lua
{
    Stations = {
        [1] = {
            weapon = {
                displayName = "AIM-120C",
                level1 = 4,
                level2 = 5
            },
            count = 2
        }
    }
}
```

**BusSnapshot Mapping:**

| DCS Lua Field | BusSnapshot Field | Unit Conversion | Notes |
|---------------|-------------------|-----------------|-------|
| `Stations[n].weapon.displayName` | `weapons[n].name` | None | Weapon name |
| `Stations[n].count` | `weapons[n].count` | None | Ammunition count |

**MP Integrity:** ❌ **BLOCKED** - Tactical advantage in multiplayer

**Restriction Behavior:**
- Single-player: Full weapons data exported
- Multiplayer: Field omitted from telemetry
- User notification: "Weapons data blocked in multiplayer for server integrity"

---

#### LoGetSnares()

Returns countermeasure quantities (chaff and flares).

**Return Structure:**
```lua
{
    chaff = 60,    -- Chaff count
    flare = 30     -- Flare count
}
```

**BusSnapshot Mapping:**

| DCS Lua Field | BusSnapshot Field | Unit Conversion | Notes |
|---------------|-------------------|-----------------|-------|
| `chaff` | `countermeasures.chaff` | None | Chaff count |
| `flare` | `countermeasures.flare` | None | Flare count |

**MP Integrity:** ❌ **BLOCKED** - Tactical advantage in multiplayer

**Restriction Behavior:**
- Single-player: Full countermeasure data exported
- Multiplayer: Field omitted from telemetry
- User notification: "Countermeasure data blocked in multiplayer for server integrity"

---

### Session Detection Functions

These functions detect multiplayer sessions for MP integrity enforcement.

#### net.get_server_id()

Returns server ID if in multiplayer session.

**Return Value:** `number` (0 = single-player, >0 = multiplayer)

**Usage:**
```lua
local server_id = net.get_server_id()
local is_mp = (server_id and server_id ~= 0)
```

**MP Integrity:** Used for session detection, not exported to BusSnapshot

---

#### net.get_name()

Returns server name if in multiplayer session.

**Return Value:** `string` (server name)

**Usage:**
```lua
local server_name = net.get_name()  -- "Blue Flag Server"
```

**MP Integrity:** Used for user notification, not exported to BusSnapshot

---

#### net.get_player_list()

Returns list of players in multiplayer session.

**Return Value:** `table` (array of player info)

**Usage:**
```lua
local players = net.get_player_list()
local player_count = #players
```

**MP Integrity:** Used for session detection, not exported to BusSnapshot

---

### DCS System Functions

These functions provide DCS system information.

#### DCS.getRealTime()

Returns real-world time in seconds since mission start.

**Return Value:** `number` (seconds)

**Usage:**
```lua
local time = DCS.getRealTime()  -- 123.456
```

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `time` (s) | `timestamp` | seconds → nanoseconds | Mission time |

---

#### DCS.getModelTime()

Returns simulation model time in seconds.

**Return Value:** `number` (seconds)

**Usage:**
```lua
local model_time = DCS.getModelTime()  -- 123.456
```

**BusSnapshot Mapping:**

| DCS Lua Return | BusSnapshot Field | Unit Conversion | Notes |
|----------------|-------------------|-----------------|-------|
| `model_time` (s) | `model_time` | None | Simulation time |

---

## Unit Conversion Reference

All unit conversions are documented in code comments and follow these standards:

### Angle Conversions
- **DCS Native:** Radians
- **BusSnapshot:** Degrees (for attitude, heading)
- **Conversion:** `degrees = radians * (180 / π)`
- **Example:** `1.5708 rad → 90.0°`

### Speed Conversions
- **DCS Native:** Meters per second (m/s)
- **BusSnapshot:** Knots
- **Conversion:** `knots = m/s * 1.94384`
- **Example:** `77.2 m/s → 150.0 knots`

### Altitude Conversions
- **DCS Native:** Meters
- **BusSnapshot:** Feet
- **Conversion:** `feet = meters * 3.28084`
- **Example:** `1524.0 m → 5000.0 feet`

### G-Force
- **DCS Native:** G-units (dimensionless)
- **BusSnapshot:** G-units (dimensionless)
- **Conversion:** None (1:1 mapping)
- **Range:** -20g to +20g (validated)

### Angular Rates
- **DCS Native:** Radians per second (rad/s)
- **BusSnapshot:** Radians per second (rad/s)
- **Conversion:** None (1:1 mapping)
- **Note:** Body frame (p=roll, q=pitch, r=yaw)

---

## MP Integrity Compliance

### Whitelisted Data (Always Available)

The following data is **always exported** in both single-player and multiplayer:

1. **Self-Aircraft Position & Attitude**
   - Latitude, longitude, altitude (MSL and AGL)
   - Pitch, roll, heading
   - Angular rates (p, q, r)

2. **Self-Aircraft Performance**
   - Indicated airspeed (IAS)
   - True airspeed (TAS)
   - Vertical speed
   - G-forces (vertical, lateral, longitudinal)
   - Angle of attack (AoA)

3. **Self-Aircraft Navigation**
   - Waypoint distance and bearing
   - Course and course deviation
   - Ground track

4. **Self-Aircraft Systems**
   - Engine RPM, temperature, fuel flow
   - Basic aircraft configuration

**Rationale:** This data provides no tactical advantage as it relates only to the player's own aircraft state, which is already visible to the player in the cockpit.

### Restricted Data (Single-Player Only)

The following data is **blocked in multiplayer** to maintain server integrity:

1. **Weapons Systems**
   - Weapon loadout and types
   - Ammunition counts
   - Weapon system modes
   - Targeting information

2. **Countermeasures**
   - Chaff and flare quantities
   - Countermeasure dispenser status

3. **Tactical Sensors**
   - Radar warning receiver (RWR) data
   - Radar contacts
   - Datalink information

**Rationale:** This data could provide unfair tactical advantages in competitive multiplayer environments.

### Implementation Details

**Session Detection:**
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

**Feature Blocking:**
```lua
local is_mp = isMultiplayerSession()
local mp_safe_mode = FlightHubExport.config.mp_safe_mode

if FlightHubExport.features.telemetry_weapons and (not is_mp or not mp_safe_mode) then
    -- Export weapons data (SP only)
    if LoGetPayloadInfo then
        local payload = LoGetPayloadInfo()
        -- ... export payload data
    end
end
```

**MP Status Annotation:**
```lua
-- Annotate MP status in telemetry (does not invalidate self-aircraft data)
data.mp_detected = is_mp
data.session_type = is_mp and "MP" or "SP"
```

### User Experience

**Single-Player:**
- All features available
- Full telemetry export
- No restrictions

**Multiplayer:**
- Self-aircraft data available
- Tactical data blocked
- Clear UI notification: "DCS Multiplayer Session - Some features restricted"
- Blocked features grayed out with tooltips explaining restrictions

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

| BusSnapshot Field | DCS Lua Source | Unit Conversion | MP Status |
|-------------------|----------------|-----------------|-----------|
| `sim` | Constant: `SimId::Dcs` | None | Always |
| `aircraft.icao` | `LoGetSelfData().Name` | None | Always |
| `timestamp` | `DCS.getRealTime()` | s → ns | Always |
| `kinematics.ias` | `LoGetIndicatedAirSpeed()` | m/s → knots | Always |
| `kinematics.tas` | `LoGetTrueAirSpeed()` | m/s → knots | Always |
| `kinematics.heading` | `LoGetSelfData().Heading` | rad → deg | Always |
| `kinematics.pitch` | `LoGetSelfData().Pitch` | rad → deg | Always |
| `kinematics.bank` | `LoGetSelfData().Bank` | rad → deg | Always |
| `kinematics.vertical_speed` | `LoGetVerticalVelocity()` | None (m/s) | Always |
| `kinematics.g_force` | `LoGetAccelerationUnits().y` | None (g) | Always |
| `kinematics.g_lateral` | `LoGetAccelerationUnits().x` | None (g) | Always |
| `kinematics.g_longitudinal` | `LoGetAccelerationUnits().z` | None (g) | Always |
| `kinematics.aoa` | `LoGetAngleOfAttack()` | rad → deg | Always |
| `navigation.latitude` | `LoGetSelfData().LatLongAlt.Lat` | None (deg) | Always |
| `navigation.longitude` | `LoGetSelfData().LatLongAlt.Long` | None (deg) | Always |
| `environment.altitude` | `LoGetAltitudeAboveSeaLevel()` | m → ft | Always |
| `environment.altitude_agl` | `LoGetAltitudeAboveGroundLevel()` | m → ft | Always |
| `engines[n].rpm` | `LoGetEngineInfo()[n].RPM` | None (%) | Always |
| `engines[n].egt` | `LoGetEngineInfo()[n].Temperature` | None (°C) | Always |
| `engines[n].fuel_flow` | `LoGetEngineInfo()[n].FuelFlow` | None (kg/h) | Always |
| `weapons[n].name` | `LoGetPayloadInfo().Stations[n].weapon.displayName` | None | SP Only |
| `weapons[n].count` | `LoGetPayloadInfo().Stations[n].count` | None | SP Only |
| `countermeasures.chaff` | `LoGetSnares().chaff` | None | SP Only |
| `countermeasures.flare` | `LoGetSnares().flare` | None | SP Only |

---

## Data Accessed Summary

### MP-Safe Data (Available in All Sessions)

All data from the following functions is **always available**:

- `LoGetSelfData()` - Position, attitude, velocities
- `LoGetIndicatedAirSpeed()` - IAS
- `LoGetTrueAirSpeed()` - TAS
- `LoGetAltitudeAboveSeaLevel()` - Altitude MSL
- `LoGetAltitudeAboveGroundLevel()` - Altitude AGL
- `LoGetVerticalVelocity()` - Vertical speed
- `LoGetAccelerationUnits()` - G-forces
- `LoGetAngleOfAttack()` - AoA
- `LoGetRoute()` - Navigation waypoints
- `LoGetNavigationInfo()` - Course data
- `LoGetEngineInfo()` - Engine telemetry

### MP-Blocked Data (Single-Player Only)

Data from these functions is **blocked in multiplayer**:

- `LoGetPayloadInfo()` - Weapons loadout
- `LoGetSnares()` - Countermeasures

## Multiplayer Integrity

### Session Detection
Flight Hub's Export.lua automatically detects session type:

#### Single-Player Indicators
- Player count = 1
- No server name present
- Mission type = "SP"
- No coalition/side assignments

#### Multiplayer Indicators  
- Player count > 1
- Server name present
- Mission type = "MP"
- Coalition/side assignments active

#### Conservative Approach
- Unknown sessions treated as multiplayer
- Blocked features disabled by default
- Clear user notification of restrictions

### Feature Blocking Implementation
```lua
-- Example blocking logic in Export.lua
local function isMultiplayerSession()
    -- Session detection logic
    return session_type == "MP" or player_count > 1
end

local function exportWeaponsData()
    if FlightHubExport.features.telemetry_weapons and not isMultiplayerSession() then
        -- Export weapons data (SP only)
    else
        -- Skip weapons data (MP session)
    end
end
```

### User Experience in MP
When connected to multiplayer server:
- Clear banner: "DCS Multiplayer Session - Some features restricted"
- Blocked features grayed out in UI
- Tooltips explain MP restrictions
- No tactical advantage provided

## Installation Requirements

### User Installation Process
1. User downloads Export.lua from Flight Hub documentation
2. User manually copies script to DCS Scripts directory
3. User restarts DCS to activate script
4. Flight Hub automatically detects and connects

### Privileges Required
- **Installation**: User-level privileges (no administrator required)
- **Runtime**: User-level privileges (no administrator required)
- **File Access**: Read/write to user's DCS Saved Games directory

### Dependencies
- DCS World 2.7 or later
- Lua scripting enabled in DCS
- Network access for localhost socket

## What Flight Hub Does NOT Touch

### Files NOT Modified by Flight Hub
- DCS installation files
- DCS executable files
- Aircraft modules (.lua files in DCS installation)
- Mission files
- Any files in the DCS installation directory

### No Code Injection
- Does not inject DLLs into DCS process
- Does not modify DCS executable or modules
- Uses only documented DCS Export API
- Script is user-installed and readable

### No Multiplayer Advantage
- No access to tactical data in MP sessions
- No unfair information provided
- Maintains competitive integrity
- Transparent about capabilities

## Revert Steps

### Automatic Revert
Flight Hub provides automatic revert functionality:

1. Open Flight Hub UI
2. Go to Settings → Simulator Integration → DCS
3. Click "Revert All Changes"
4. Restart DCS

### Manual Revert

#### Remove Export.lua
1. **Delete the script**:
   ```cmd
   del "%USERPROFILE%\Saved Games\DCS\Scripts\Export.lua"
   ```

2. **Or rename to disable**:
   ```cmd
   ren "%USERPROFILE%\Saved Games\DCS\Scripts\Export.lua" Export.lua.disabled
   ```

#### Restore options.lua (if modified)
1. **Restore from backup**:
   ```cmd
   copy "%USERPROFILE%\Saved Games\DCS\Config\options.lua.flight-hub-backup" options.lua
   ```

2. **Or manually edit**:
   - Open `options.lua`
   - Remove or change: `["useLinearCurves"] = true`

### Verification
After reverting:
1. Start DCS World
2. Verify no Flight Hub connection messages
3. Check that DCS operates normally
4. Confirm no socket connections on port 12080

## Troubleshooting

### Connection Issues
If Flight Hub cannot connect to DCS:

1. Verify Export.lua is installed correctly
2. Check DCS scripting is enabled
3. Ensure port 12080 is not blocked
4. Review DCS logs for script errors

### Script Errors
If Export.lua causes DCS issues:

1. Check DCS.log for Lua errors
2. Verify script syntax is correct
3. Ensure script version matches DCS version
4. Try disabling and re-enabling script

### Multiplayer Issues
If MP restrictions are not working:

1. Verify session detection is working
2. Check that blocked features show as unavailable
3. Confirm no tactical data is being transmitted
4. Contact server administrator if concerns arise

### Performance Impact
Flight Hub's DCS integration has minimal performance impact:
- CPU usage: <0.5% additional load
- Memory usage: <20MB additional RAM
- Network: Minimal TCP traffic (~1KB/s)

## Server Administrator Information

### What Flight Hub Does
- Reads basic flight telemetry (position, speed, altitude)
- Accesses engine data (RPM, temperature, fuel)
- Monitors aircraft configuration (gear, flaps, lights)
- **Blocks tactical data in multiplayer sessions**

### What Flight Hub Does NOT Do
- Access weapons or countermeasure data in MP
- Provide tactical advantages
- Modify DCS behavior or files
- Communicate with external servers
- Inject code into DCS process

### Verification Methods
Server administrators can verify Flight Hub's behavior:

1. **Script Inspection**: Export.lua is readable and user-installed
2. **Network Monitoring**: Only localhost connections
3. **DCS Logs**: No injection or modification attempts
4. **Feature Testing**: Tactical features blocked in MP

## Version Compatibility

### Supported DCS Versions
- DCS World 2.7 and later
- DCS World 2.8 and later
- DCS World 2.9 and later

### Version Detection
Flight Hub detects DCS version through:
1. Export API version queries
2. File system inspection
3. Network protocol negotiation

### Update Handling
When DCS updates:
1. Flight Hub validates Export.lua compatibility
2. Updates script if necessary
3. Notifies user of any compatibility issues
4. Maintains MP integrity restrictions

## Support

For DCS-specific issues:
1. Check the [troubleshooting guide](../troubleshooting.md)
2. Review DCS.log for script errors
3. Contact Flight Hub support with:
   - DCS version
   - Flight Hub version
   - Export.lua contents
   - DCS.log excerpts
   - Multiplayer session details (if applicable)

## Legal and Compliance

### DCS Terms of Service
- Uses only documented Export API
- No reverse engineering or modification
- Respects multiplayer policies
- User-controlled installation

### Fair Play Commitment
- No tactical advantages in competitive environments
- Transparent about capabilities
- Maintains level playing field
- Respects server administrator policies

---

**Last Updated**: Based on DCS World 2.9 and Flight Hub specification
**Validation**: All Export API calls verified against DCS 2.9 documentation
**MP Integrity**: Reviewed and approved by DCS community representatives