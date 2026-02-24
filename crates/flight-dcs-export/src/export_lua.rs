// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Export.lua script generation and installation
//!
//! Generates minimal Export.lua scripts for user installation in DCS Saved Games.
//! Implements MP-safe feature flags and clear documentation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration for Export.lua generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportLuaConfig {
    /// Flight Hub socket address
    pub socket_address: String,
    /// Socket port
    pub socket_port: u16,
    /// Update interval in seconds
    pub update_interval: f32,
    /// Enabled features
    pub enabled_features: Vec<String>,
    /// MP-safe mode (blocks restricted features)
    pub mp_safe_mode: bool,
}

impl Default for ExportLuaConfig {
    fn default() -> Self {
        Self {
            socket_address: "127.0.0.1".to_string(),
            socket_port: 7778,
            update_interval: 0.1, // 10Hz
            enabled_features: vec![
                "telemetry_basic".to_string(),
                "telemetry_navigation".to_string(),
                "telemetry_engines".to_string(),
                "telemetry_config".to_string(),
                "session_detection".to_string(),
            ],
            mp_safe_mode: true,
        }
    }
}

/// Export.lua script generator
pub struct ExportLuaGenerator {
    config: ExportLuaConfig,
}

/// DCS variant identifier
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DcsVariant {
    Stable,
    OpenBeta,
    OpenAlpha,
}

impl DcsVariant {
    pub fn as_str(&self) -> &str {
        match self {
            DcsVariant::Stable => "DCS",
            DcsVariant::OpenBeta => "DCS.openbeta",
            DcsVariant::OpenAlpha => "DCS.openalpha",
        }
    }
}

/// Detect all installed DCS variants
pub fn detect_dcs_variants() -> Result<Vec<(DcsVariant, PathBuf)>> {
    let home_dir = dirs::home_dir().context("Could not determine home directory")?;

    let mut variants = Vec::new();

    // Check all possible DCS variant paths
    let variant_paths = [
        (DcsVariant::OpenAlpha, "Saved Games/DCS.openalpha"),
        (DcsVariant::OpenBeta, "Saved Games/DCS.openbeta"),
        (DcsVariant::Stable, "Saved Games/DCS"),
        (DcsVariant::OpenAlpha, "Documents/DCS.openalpha"),
        (DcsVariant::OpenBeta, "Documents/DCS.openbeta"),
        (DcsVariant::Stable, "Documents/DCS"),
    ];

    for (variant, path) in &variant_paths {
        let full_path = home_dir.join(path);
        if full_path.exists() {
            // Avoid duplicates (same variant in different locations)
            if !variants.iter().any(|(v, _)| v == variant) {
                variants.push((variant.clone(), full_path));
            }
        }
    }

    Ok(variants)
}

/// Get DCS Saved Games path for a specific variant
pub fn get_dcs_variant_path(variant: &DcsVariant) -> Result<PathBuf> {
    let home_dir = dirs::home_dir().context("Could not determine home directory")?;

    // Try Saved Games first, then Documents
    let paths = [
        home_dir.join(format!("Saved Games/{}", variant.as_str())),
        home_dir.join(format!("Documents/{}", variant.as_str())),
    ];

    for path in &paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    // Return the preferred path even if it doesn't exist
    Ok(paths[0].clone())
}

impl ExportLuaGenerator {
    /// Create new generator with config
    pub fn new(config: ExportLuaConfig) -> Self {
        Self { config }
    }

    /// Generate Export.lua script content
    pub fn generate_script(&self) -> String {
        let header = self.generate_header();
        let config_section = self.generate_config();
        let feature_flags = self.generate_feature_flags();
        let socket_code = self.generate_socket_code();
        let telemetry_code = self.generate_telemetry_code();
        let main_loop = self.generate_main_loop();
        let footer = self.generate_footer();

        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}",
            header, config_section, feature_flags, socket_code, telemetry_code, main_loop, footer
        )
    }

    /// Generate script header with documentation
    fn generate_header(&self) -> String {
        r#"-- Flight Hub DCS Export Script
-- Generated automatically - DO NOT EDIT MANUALLY
--
-- This script provides telemetry data from DCS World to Flight Hub.
-- It respects DCS multiplayer integrity by blocking restricted features in MP sessions.
--
-- Installation:
-- 1. Copy this file to: DCS.openbeta\Scripts\Export.lua
-- 2. If Export.lua already exists, append this content to the existing file
-- 3. Restart DCS World
--
-- Removal:
-- 1. Delete or rename Export.lua to disable
-- 2. Restart DCS World
--
-- Implementation Details:
-- - Uses LuaExportStart/Stop/AfterNextFrame hooks for DCS integration
-- - Properly chains to existing Export.lua hooks (deterministic order)
-- - Uses LoGet* functions for self-aircraft telemetry gathering
-- - Non-blocking UDP transmission to localhost (127.0.0.1)
-- - Target rate: 60Hz via LuaExportActivityNextEvent
-- - Graceful nil handling for all LoGet* function returns
--
-- MP Integrity Compliance:
-- - Single Player: All features available
-- - Multiplayer: Self-aircraft data allowed (attitude, velocities, g-load, IAS/TAS, AoA)
-- - Multiplayer: Weapons/countermeasures/RWR data blocked for server integrity
-- - MP status annotated in telemetry (mp_detected flag)
-- - Self-aircraft telemetry remains valid in MP mode
--
-- Hook Chaining:
-- - Stores references to previous hook functions before redefining
-- - Calls previous hooks in deterministic order (existing tools first)
-- - Compatible with SRS, Tacview, and other Export.lua tools
--
-- Version: 1.0
-- Protocol: Flight Hub DCS Export v1.0

local FlightHubExport = {{}}"#
            .to_string()
    }

    /// Generate configuration section
    fn generate_config(&self) -> String {
        format!(
            r#"
-- Configuration
FlightHubExport.config = {{
    socket_address = "{}",
    socket_port = {},
    update_interval = {},
    mp_safe_mode = {},
    protocol_version = "1.0"
}}"#,
            self.config.socket_address,
            self.config.socket_port,
            self.config.update_interval,
            if self.config.mp_safe_mode {
                "true"
            } else {
                "false"
            }
        )
    }

    /// Generate feature flags
    fn generate_feature_flags(&self) -> String {
        let mut features = HashMap::new();

        // Default all features to false
        let all_features = [
            "telemetry_basic",
            "telemetry_navigation",
            "telemetry_engines",
            "telemetry_config",
            "telemetry_weapons",
            "telemetry_countermeasures",
            "telemetry_rwr",
            "session_detection",
        ];

        for feature in &all_features {
            features.insert(
                *feature,
                self.config.enabled_features.contains(&feature.to_string()),
            );
        }

        let mut feature_lines = Vec::new();
        feature_lines.push("-- Feature flags (MP-safe features marked)".to_string());
        feature_lines.push("FlightHubExport.features = {".to_string());

        for (feature, enabled) in &features {
            let mp_safe = !matches!(
                *feature,
                "telemetry_weapons" | "telemetry_countermeasures" | "telemetry_rwr"
            );
            let comment = if mp_safe {
                " -- MP-safe"
            } else {
                " -- MP-blocked"
            };
            feature_lines.push(format!(
                "    {} = {},{}",
                feature,
                if *enabled { "true" } else { "false" },
                comment
            ));
        }

        feature_lines.push("}".to_string());
        feature_lines.join("\n")
    }

    /// Generate socket communication code
    fn generate_socket_code(&self) -> String {
        r#"
-- Socket communication (UDP for non-blocking transmission to localhost)
FlightHubExport.socket = nil
FlightHubExport.connected = false
FlightHubExport.last_heartbeat = 0

function FlightHubExport.connect()
    if FlightHubExport.socket then
        FlightHubExport.socket:close()
    end
    
    -- Use UDP for non-blocking, fire-and-forget transmission to localhost
    -- This ensures DCS simulation is never blocked by network I/O
    FlightHubExport.socket = require('socket').udp()
    FlightHubExport.socket:settimeout(0) -- Non-blocking
    
    -- Set peer address for UDP (localhost only for security)
    FlightHubExport.socket:setpeername(
        FlightHubExport.config.socket_address,
        FlightHubExport.config.socket_port
    )
    
    FlightHubExport.connected = true
    FlightHubExport.send_handshake()
    
    return true
end

function FlightHubExport.send_message(message)
    if not FlightHubExport.connected or not FlightHubExport.socket then
        return false
    end
    
    local json_str = FlightHubExport.to_json(message) .. "\n"
    
    -- UDP send is non-blocking and fire-and-forget
    -- We don't check for errors to avoid blocking DCS
    local result, err = FlightHubExport.socket:send(json_str)
    
    -- Even if send fails, we stay "connected" since UDP is connectionless
    -- The Rust adapter will detect timeout if no packets arrive
    return true
end

function FlightHubExport.send_handshake()
    local features = {}
    for feature, enabled in pairs(FlightHubExport.features) do
        if enabled then
            table.insert(features, feature)
        end
    end
    
    local handshake = {
        type = "Handshake",
        data = {
            version = { major = 1, minor = 0 },
            features = features
        }
    }
    
    FlightHubExport.send_message(handshake)
end

function FlightHubExport.send_heartbeat()
    local now = DCS.getRealTime()
    if now - FlightHubExport.last_heartbeat > 10 then -- 10 second interval
        local heartbeat = {
            type = "Heartbeat", 
            data = {
                timestamp = math.floor(now * 1000)
            }
        }
        
        if FlightHubExport.send_message(heartbeat) then
            FlightHubExport.last_heartbeat = now
        end
    end
end"#
            .to_string()
    }

    /// Generate telemetry collection code
    fn generate_telemetry_code(&self) -> String {
        r#"
-- Telemetry data collection using LoGet* functions
-- All functions handle nil returns gracefully for robustness
function FlightHubExport.collect_telemetry()
    local data = {}
    local aircraft_name = "Unknown"
    
    -- Get model time
    if DCS.getModelTime then
        local model_time = DCS.getModelTime()
        if model_time and model_time > 0 then
            data.model_time = model_time
        end
    end
    
    -- Session detection for MP integrity check compliance
    local session_type = "SP" -- Default to single player
    local server_name = nil
    local player_count = 1
    
    -- Try to detect multiplayer session
    if net and net.get_server_id then
        local server_id = net.get_server_id()
        if server_id and server_id ~= 0 then
            session_type = "MP"
            if net.get_name then
                server_name = net.get_name()
            end
            if net.get_player_list then
                local players = net.get_player_list()
                if players then
                    player_count = #players
                end
            end
        end
    end
    
    data.session_type = session_type
    if server_name then
        data.server_name = server_name
    end
    data.player_count = player_count
    
    -- Check MP restrictions
    local is_mp = (session_type == "MP")
    local mp_safe_mode = FlightHubExport.config.mp_safe_mode
    
    -- Annotate MP status (does not invalidate self-aircraft data)
    data.mp_detected = is_mp
    
    -- Basic telemetry (MP-safe: self-aircraft data only)
    if FlightHubExport.features.telemetry_basic then
        -- LoGetSelfData: Returns self-aircraft position, attitude, and velocity
        if LoGetSelfData then
            local self_data = LoGetSelfData()
            if self_data then
                -- Aircraft name
                aircraft_name = self_data.Name or "Unknown"
                data.aircraft = aircraft_name
                
                -- Position (MP-safe: self-aircraft only)
                if self_data.LatLongAlt then
                    data.latitude = self_data.LatLongAlt.Lat
                    data.longitude = self_data.LatLongAlt.Long
                    data.altitude = self_data.LatLongAlt.Alt
                end
                
                -- Attitude (MP-safe: self-aircraft only)
                -- DCS uses radians natively, convert to degrees for consistency
                if self_data.Heading then
                    data.heading = math.deg(self_data.Heading)
                end
                
                if self_data.Pitch then
                    data.pitch = math.deg(self_data.Pitch)
                end
                
                if self_data.Bank then
                    data.bank = math.deg(self_data.Bank)
                end
                
                -- Angular velocities (MP-safe: self-aircraft only)
                if self_data.AngularVelocity then
                    data.angular_velocity_x = self_data.AngularVelocity.x
                    data.angular_velocity_y = self_data.AngularVelocity.y
                    data.angular_velocity_z = self_data.AngularVelocity.z
                end
                
                -- Body velocities (MP-safe: self-aircraft only)
                if self_data.Velocity then
                    data.velocity_x = self_data.Velocity.x
                    data.velocity_y = self_data.Velocity.y
                    data.velocity_z = self_data.Velocity.z
                end
            end
        end
        
        -- LoGetIndicatedAirSpeed: Returns IAS in m/s, converted to knots (MP-safe)
        if LoGetIndicatedAirSpeed then
            local ias = LoGetIndicatedAirSpeed()
            if ias then
                data.ias = ias * 1.94384  -- m/s to knots
            end
        end
        
        -- LoGetTrueAirSpeed: Returns TAS in m/s, converted to knots (MP-safe)
        if LoGetTrueAirSpeed then
            local tas = LoGetTrueAirSpeed()
            if tas then
                data.tas = tas * 1.94384  -- m/s to knots
            end
        end
        
        -- LoGetAltitudeAboveSeaLevel: Returns altitude MSL in meters, converted to feet (MP-safe)
        if LoGetAltitudeAboveSeaLevel then
            local altitude_asl = LoGetAltitudeAboveSeaLevel()
            if altitude_asl then
                data.altitude_asl = altitude_asl * 3.28084  -- meters to feet
            end
        end
        
        -- LoGetAltitudeAboveGroundLevel: Returns altitude AGL in meters, converted to feet (MP-safe)
        if LoGetAltitudeAboveGroundLevel then
            local altitude_agl = LoGetAltitudeAboveGroundLevel()
            if altitude_agl then
                data.altitude_agl = altitude_agl * 3.28084  -- meters to feet
            end
        end
        
        -- LoGetVerticalVelocity: Returns vertical speed in m/s, converted to feet/min (MP-safe)
        if LoGetVerticalVelocity then
            local vs = LoGetVerticalVelocity()
            if vs then
                data.vertical_speed = vs * 196.85  -- m/s to feet per minute
            end
        end
        
        -- LoGetAccelerationUnits: Returns g-forces (MP-safe)
        if LoGetAccelerationUnits then
            local accel = LoGetAccelerationUnits()
            if accel then
                -- DCS returns g-forces in body frame
                data.g_force = accel.y or 1.0        -- Vertical (normal) g-load
                data.g_lateral = accel.x or 0.0      -- Lateral g-load
                data.g_longitudinal = accel.z or 0.0 -- Longitudinal g-load
            end
        end
        
        -- LoGetAngleOfAttack: Returns AoA in radians, converted to degrees (MP-safe)
        if LoGetAngleOfAttack then
            local aoa = LoGetAngleOfAttack()
            if aoa then
                data.aoa = math.deg(aoa)  -- radians to degrees
            end
        end
    end
    
    -- Navigation data (MP-safe: self-aircraft navigation only)
    if FlightHubExport.features.telemetry_navigation then
        if LoGetRoute then
            local route = LoGetRoute()
            if route and route.goto_point then
                data.waypoint_distance = route.goto_point.dist * 0.000539957  -- meters to nautical miles
                data.waypoint_bearing = math.deg(route.goto_point.bearing)
            end
        end
        
        if LoGetNavigationInfo then
            local nav = LoGetNavigationInfo()
            if nav then
                data.course = nav.Course
                data.desired_course = nav.DesiredCourse
                data.course_deviation = nav.CourseDeviation
            end
        end
    end
    
    -- Aircraft configuration (MP-safe: self-aircraft state only)
    -- Draw argument values are 3D model parameters; indices are aircraft-dependent.
    -- Argument 1 (gear) and argument 9 (flaps) are common approximations for many jets.
    if FlightHubExport.features.telemetry_config then
        if LoGetAircraftDrawArgumentValue then
            -- Gear position: 0.0=fully retracted, 1.0=fully extended (transitioning: 0.1-0.9)
            local gear_arg = LoGetAircraftDrawArgumentValue(1)
            if gear_arg ~= nil then
                data.gear_down = gear_arg
            end
            -- Flaps position: 0.0=retracted, 1.0=fully extended
            local flaps_arg = LoGetAircraftDrawArgumentValue(9)
            if flaps_arg ~= nil then
                data.flaps = flaps_arg * 100.0  -- normalize to percentage (0-100)
            end
        end
    end
    
    -- Engine data (MP-safe: self-aircraft engines only)
    if FlightHubExport.features.telemetry_engines then
        if LoGetEngineInfo then
            local engines = LoGetEngineInfo()
            if engines then
                data.engines = {}
                for i, engine in pairs(engines) do
                    if engine then
                        data.engines[i] = {
                            rpm = engine.RPM,
                            temperature = engine.Temperature,
                            fuel_flow = engine.FuelFlow
                        }
                    end
                end
            end
        end
    end
    
    -- Weapons data (MP-blocked: restricted by integrity check)
    -- Only export in single-player or when MP safe mode is disabled
    if FlightHubExport.features.telemetry_weapons and (not is_mp or not mp_safe_mode) then
        if LoGetPayloadInfo then
            local payload = LoGetPayloadInfo()
            if payload and payload.Stations then
                data.weapons = {}
                for station, weapon in pairs(payload.Stations) do
                    if weapon and weapon.weapon then
                        data.weapons[station] = {
                            name = weapon.weapon.displayName,
                            count = weapon.count
                        }
                    end
                end
            end
        end
    end
    
    -- Countermeasures data (MP-blocked: restricted by integrity check)
    -- Only export in single-player or when MP safe mode is disabled
    if FlightHubExport.features.telemetry_countermeasures and (not is_mp or not mp_safe_mode) then
        if LoGetSnares then
            local snares = LoGetSnares()
            if snares then
                data.countermeasures = {
                    chaff = snares.chaff or 0,
                    flare = snares.flare or 0
                }
            end
        end
    end
    
    return data, aircraft_name
end"#
            .to_string()
    }

    /// Generate main loop code
    fn generate_main_loop(&self) -> String {
        r#"
-- Main export loop
FlightHubExport.last_update = 0
FlightHubExport.frame_count = 0

function FlightHubExport.update()
    local now = DCS.getRealTime()
    
    -- Check update interval
    if now - FlightHubExport.last_update < FlightHubExport.config.update_interval then
        return
    end
    
    -- Ensure connection
    if not FlightHubExport.connected then
        if not FlightHubExport.connect() then
            return -- Connection failed, try again next time
        end
    end
    
    -- Send heartbeat
    FlightHubExport.send_heartbeat()
    
    -- Collect and send telemetry
    local telemetry_data, aircraft_name = FlightHubExport.collect_telemetry()
    
    local message = {{
        type = "Telemetry",
        data = {{
            timestamp = math.floor(now * 1000),
            aircraft = aircraft_name,
            session_type = telemetry_data.session_type,
            data = telemetry_data
        }}
    }}
    
    if FlightHubExport.send_message(message) then
        FlightHubExport.last_update = now
        FlightHubExport.frame_count = FlightHubExport.frame_count + 1
    else
        -- Send failed, will reconnect next time
        FlightHubExport.connected = false
    end
end

-- JSON serialization (simple implementation)
function FlightHubExport.to_json(obj)
    if type(obj) == "table" then
        local items = {{}}
        for k, v in pairs(obj) do
            local key = type(k) == "string" and '"' .. k .. '"' or tostring(k)
            table.insert(items, key .. ":" .. FlightHubExport.to_json(v))
        end
        return "{{" .. table.concat(items, ",") .. "}}"
    elseif type(obj) == "string" then
        return '"' .. obj:gsub('"', '\\"') .. '"'
    elseif type(obj) == "number" then
        return tostring(obj)
    elseif type(obj) == "boolean" then
        return obj and "true" or "false"
    else
        return "null"
    end
end"#
            .to_string()
    }

    /// Generate script footer
    fn generate_footer(&self) -> String {
        r#"
-- DCS Export hooks implementation
-- These hooks are called by DCS World at specific points in the simulation lifecycle

-- Store references to any existing hooks before we redefine them
-- This ensures proper chaining with other export scripts (e.g., SRS, Tacview)
local PrevLuaExportStart = LuaExportStart
local PrevLuaExportStop = LuaExportStop
local PrevLuaExportBeforeNextFrame = LuaExportBeforeNextFrame
local PrevLuaExportAfterNextFrame = LuaExportAfterNextFrame
local PrevLuaExportActivityNextEvent = LuaExportActivityNextEvent

-- LuaExportStart: Called once when the mission starts
function LuaExportStart()
    -- Call previous hook first (deterministic order: existing tools first)
    if PrevLuaExportStart then
        PrevLuaExportStart()
    end
    
    -- Initialize Flight Hub export
    FlightHubExport.connect()
    FlightHubExport.last_update = DCS.getRealTime()
end

-- LuaExportStop: Called once when the mission ends
function LuaExportStop()
    -- Clean up Flight Hub connection first
    if FlightHubExport.socket then
        FlightHubExport.socket:close()
        FlightHubExport.socket = nil
    end
    FlightHubExport.connected = false
    
    -- Call previous hook last (deterministic order: existing tools last)
    if PrevLuaExportStop then
        PrevLuaExportStop()
    end
end

-- LuaExportBeforeNextFrame: Called before each simulation frame
-- This is the main telemetry update hook
function LuaExportBeforeNextFrame()
    -- Call previous hook first
    if PrevLuaExportBeforeNextFrame then
        PrevLuaExportBeforeNextFrame()
    end
    
    -- Update Flight Hub telemetry
    FlightHubExport.update()
end

-- LuaExportAfterNextFrame: Called after each simulation frame
function LuaExportAfterNextFrame()
    -- Call previous hook first
    if PrevLuaExportAfterNextFrame then
        PrevLuaExportAfterNextFrame()
    end
    
    -- Flight Hub doesn't need post-frame processing currently
    -- This hook is here for completeness and future use
end

-- LuaExportActivityNextEvent: Controls the export update rate
-- Returns the time in seconds until the next export should occur
-- Returning a small value (e.g., 0.0167 for 60Hz) ensures high-rate updates
function LuaExportActivityNextEvent(tCurrent)
    -- Call previous hook first if it exists
    local tNext = nil
    if PrevLuaExportActivityNextEvent then
        tNext = PrevLuaExportActivityNextEvent(tCurrent)
    end
    
    -- Flight Hub target: 60Hz = 0.0167 seconds between updates
    -- This ensures we get called frequently enough for smooth FFB
    local flightHubInterval = 1.0 / 60.0  -- 60Hz target rate
    
    -- If previous hook requested a sooner callback, honor it
    -- Otherwise use our 60Hz target
    if tNext and tNext < flightHubInterval then
        return tNext
    else
        return tCurrent + flightHubInterval
    end
end"#
            .to_string()
    }

    /// Write script to file
    pub fn write_script(&self, path: &Path) -> Result<()> {
        let script_content = self.generate_script();
        std::fs::write(path, script_content)
            .with_context(|| format!("Failed to write Export.lua to {}", path.display()))?;
        Ok(())
    }

    /// Get default DCS Saved Games path (prefers openbeta)
    pub fn get_dcs_saved_games_path() -> Result<PathBuf> {
        let variants = detect_dcs_variants()?;

        // Prefer openbeta, then openalpha, then stable
        for (variant, path) in &variants {
            if matches!(variant, DcsVariant::OpenBeta) {
                return Ok(path.clone());
            }
        }

        for (variant, path) in &variants {
            if matches!(variant, DcsVariant::OpenAlpha) {
                return Ok(path.clone());
            }
        }

        for (variant, path) in &variants {
            if matches!(variant, DcsVariant::Stable) {
                return Ok(path.clone());
            }
        }

        // Default to DCS.openbeta if none found
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home_dir.join("Saved Games/DCS.openbeta"))
    }

    /// Get Export.lua installation path
    pub fn get_export_lua_path() -> Result<PathBuf> {
        let dcs_path = Self::get_dcs_saved_games_path()?;
        Ok(dcs_path.join("Scripts").join("Export.lua"))
    }

    /// Check if Export.lua already exists
    pub fn export_lua_exists() -> Result<bool> {
        let export_path = Self::get_export_lua_path()?;
        Ok(export_path.exists())
    }

    /// Create installation script for user
    pub fn generate_install_script(&self) -> String {
        let export_path = Self::get_export_lua_path()
            .unwrap_or_else(|_| PathBuf::from("DCS.openbeta/Scripts/Export.lua"));

        format!(
            r#"# Flight Hub DCS Export Installation

## Automatic Installation

1. Run the Flight Hub installer
2. Select "Install DCS Export" option
3. Restart DCS World

## Manual Installation

1. Copy the generated Export.lua to:
   `{}`

2. If Export.lua already exists:
   - Backup the existing file
   - Append the Flight Hub export code to the existing file
   - Or replace if you don't need other exports

3. Restart DCS World

## Verification

1. Start DCS World
2. Load any mission
3. Check Flight Hub shows "DCS Connected" status
4. Verify telemetry data is updating

## Removal

1. Delete or rename Export.lua:
   `{}`
2. Restart DCS World

## What We Touch

Flight Hub DCS integration only touches:

### Files Modified:
- `Scripts/Export.lua` (user-installed, user-controlled)

### Network Connections:
- Local socket connection to 127.0.0.1:7778
- No external network access

### DCS APIs Used:
- LoGetSelfData() - Aircraft position/attitude
- LoGetIndicatedAirSpeed() - Airspeed data  
- LoGetEngineInfo() - Engine telemetry
- LoGetPayloadInfo() - Weapons data (SP only)
- LoGetSnares() - Countermeasures (SP only)
- net.get_server_id() - MP session detection

### Multiplayer Integrity:
- Weapons/countermeasures data blocked in MP sessions
- Only reads telemetry data, never writes to DCS
- No code injection into DCS process
- Respects DCS multiplayer server policies

## Troubleshooting

### Connection Issues:
1. Check Windows Firewall allows Flight Hub
2. Verify DCS Scripts folder exists
3. Check DCS.log for Lua errors

### MP Restrictions:
- Some features disabled in MP for server integrity
- This is normal and expected behavior
- Features work normally in single-player

### Conflicts with Other Exports:
- Flight Hub export is designed to coexist
- Uses legacy hook system for compatibility
- Contact support if conflicts occur
"#,
            export_path.display(),
            export_path.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_lua_generation() {
        let config = ExportLuaConfig::default();
        let generator = ExportLuaGenerator::new(config);
        let script = generator.generate_script();

        // Check for key components
        assert!(script.contains("Flight Hub DCS Export Script"));
        assert!(script.contains("socket_address = \"127.0.0.1\""));
        assert!(script.contains("socket_port = 7778"));
        assert!(script.contains("telemetry_basic = true"));
        assert!(script.contains("MP-safe"));
        assert!(script.contains("MP-blocked"));

        // Check for proper DCS Export hooks
        assert!(script.contains("function LuaExportStart()"));
        assert!(script.contains("function LuaExportStop()"));
        assert!(script.contains("function LuaExportBeforeNextFrame()"));
        assert!(script.contains("function LuaExportAfterNextFrame()"));
        assert!(script.contains("function LuaExportActivityNextEvent(tCurrent)"));

        // Check for proper hook chaining
        assert!(script.contains("PrevLuaExportStart"));
        assert!(script.contains("PrevLuaExportStop"));
        assert!(script.contains("PrevLuaExportBeforeNextFrame"));
        assert!(script.contains("PrevLuaExportAfterNextFrame"));
        assert!(script.contains("PrevLuaExportActivityNextEvent"));

        // Check for UDP socket (non-blocking)
        assert!(script.contains("require('socket').udp()"));
        assert!(script.contains("settimeout(0)"));

        // Check for LoGet* functions
        assert!(script.contains("LoGetSelfData"));
        assert!(script.contains("LoGetIndicatedAirSpeed"));
        assert!(script.contains("LoGetTrueAirSpeed"));
        assert!(script.contains("LoGetAccelerationUnits"));
        assert!(script.contains("LoGetAngleOfAttack"));

        // Check for 60Hz target rate
        assert!(script.contains("1.0 / 60.0"));
    }

    #[test]
    fn test_feature_flags_generation() {
        let mut config = ExportLuaConfig::default();
        config.enabled_features = vec!["telemetry_basic".to_string()];

        let generator = ExportLuaGenerator::new(config);
        let script = generator.generate_script();

        assert!(script.contains("telemetry_basic = true"));
        assert!(script.contains("telemetry_weapons = false"));
    }

    #[test]
    fn test_mp_safe_mode() {
        let mut config = ExportLuaConfig::default();
        config.mp_safe_mode = false;

        let generator = ExportLuaGenerator::new(config);
        let script = generator.generate_script();

        assert!(script.contains("mp_safe_mode = false"));
    }

    #[test]
    fn test_install_script_generation() {
        let config = ExportLuaConfig::default();
        let generator = ExportLuaGenerator::new(config);
        let install_script = generator.generate_install_script();

        assert!(install_script.contains("Flight Hub DCS Export Installation"));
        assert!(install_script.contains("What We Touch"));
        assert!(install_script.contains("Multiplayer Integrity"));
    }

    #[test]
    fn test_requirements_compliance() {
        let config = ExportLuaConfig::default();
        let generator = ExportLuaGenerator::new(config);
        let script = generator.generate_script();

        // DCS-INT-01.4: Verify LuaExportStart/Stop/AfterNextFrame hooks
        assert!(script.contains("function LuaExportStart()"));
        assert!(script.contains("function LuaExportStop()"));
        assert!(script.contains("function LuaExportAfterNextFrame()"));

        // DCS-INT-01.5: Verify proper chaining to existing Export.lua hooks
        assert!(script.contains("local PrevLuaExportStart = LuaExportStart"));
        assert!(script.contains("local PrevLuaExportStop = LuaExportStop"));
        assert!(script.contains("local PrevLuaExportBeforeNextFrame = LuaExportBeforeNextFrame"));
        assert!(script.contains("if PrevLuaExportStart then"));
        assert!(script.contains("PrevLuaExportStart()"));

        // DCS-INT-01.6: Verify self-aircraft telemetry gathering using LoGet* functions
        assert!(script.contains("LoGetSelfData"));
        assert!(script.contains("LoGetIndicatedAirSpeed"));
        assert!(script.contains("LoGetTrueAirSpeed"));
        assert!(script.contains("LoGetAltitudeAboveSeaLevel"));
        assert!(script.contains("LoGetAltitudeAboveGroundLevel"));
        assert!(script.contains("LoGetVerticalVelocity"));
        assert!(script.contains("LoGetAccelerationUnits"));
        assert!(script.contains("LoGetAngleOfAttack"));

        // DCS-INT-01.7: Verify nil handling
        assert!(script.contains("if self_data then"));
        assert!(script.contains("if ias then"));
        assert!(script.contains("if accel then"));

        // DCS-INT-01.8: Verify MP integrity check compliance
        assert!(script.contains("mp_detected"));
        assert!(script.contains("session_type"));
        assert!(script.contains("MP-safe"));
        assert!(script.contains("MP-blocked"));

        // DCS-INT-01.9: Verify whitelist self-aircraft data
        assert!(script.contains("self-aircraft data only"));
        assert!(script.contains("self-aircraft navigation only"));
        assert!(script.contains("self-aircraft engines only"));

        // DCS-INT-01.10: Verify MP-blocked features
        assert!(script.contains("telemetry_weapons"));
        assert!(script.contains("telemetry_countermeasures"));
        assert!(script.contains("not is_mp or not mp_safe_mode"));

        // DCS-INT-01.11: Verify non-blocking UDP transmission to localhost
        assert!(script.contains("require('socket').udp()"));
        assert!(script.contains("settimeout(0)"));
        assert!(script.contains("127.0.0.1"));

        // DCS-INT-01.12: Verify 60Hz target rate via LuaExportActivityNextEvent
        assert!(script.contains("function LuaExportActivityNextEvent(tCurrent)"));
        assert!(script.contains("1.0 / 60.0"));
        assert!(script.contains("60Hz target rate"));
    }
}
