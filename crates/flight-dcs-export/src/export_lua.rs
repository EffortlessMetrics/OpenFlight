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
        format!(
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
-- MP Integrity:
-- - Single Player: All features available
-- - Multiplayer: Weapons/countermeasures data blocked for server integrity
--
-- Version: 1.0
-- Protocol: Flight Hub DCS Export v1.0

local FlightHubExport = {{}}"#
        )
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
            if self.config.mp_safe_mode { "true" } else { "false" }
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
            "telemetry_weapons",
            "telemetry_countermeasures",
            "telemetry_rwr",
            "session_detection",
        ];

        for feature in &all_features {
            features.insert(*feature, self.config.enabled_features.contains(&feature.to_string()));
        }

        let mut feature_lines = Vec::new();
        feature_lines.push("-- Feature flags (MP-safe features marked)".to_string());
        feature_lines.push("FlightHubExport.features = {".to_string());

        for (feature, enabled) in &features {
            let mp_safe = !matches!(*feature, "telemetry_weapons" | "telemetry_countermeasures" | "telemetry_rwr");
            let comment = if mp_safe { " -- MP-safe" } else { " -- MP-blocked" };
            feature_lines.push(format!("    {} = {},{}",
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
-- Socket communication
FlightHubExport.socket = nil
FlightHubExport.connected = false
FlightHubExport.last_heartbeat = 0

function FlightHubExport.connect()
    if FlightHubExport.socket then
        FlightHubExport.socket:close()
    end
    
    FlightHubExport.socket = require('socket').tcp()
    FlightHubExport.socket:settimeout(0) -- Non-blocking
    
    local result, err = FlightHubExport.socket:connect(
        FlightHubExport.config.socket_address,
        FlightHubExport.config.socket_port
    )
    
    if result then
        FlightHubExport.connected = true
        FlightHubExport.send_handshake()
        return true
    else
        FlightHubExport.connected = false
        return false
    end
end

function FlightHubExport.send_message(message)
    if not FlightHubExport.connected or not FlightHubExport.socket then
        return false
    end
    
    local json_str = FlightHubExport.to_json(message) .. "\n"
    local result, err = FlightHubExport.socket:send(json_str)
    
    if not result then
        FlightHubExport.connected = false
        return false
    end
    
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
end"#.to_string()
    }

    /// Generate telemetry collection code
    fn generate_telemetry_code(&self) -> String {
        r#"
-- Telemetry data collection
function FlightHubExport.collect_telemetry()
    local data = {}
    local aircraft_name = "Unknown"
    
    -- Get aircraft info
    if DCS.getModelTime then
        local model_time = DCS.getModelTime()
        if model_time and model_time > 0 then
            data.model_time = model_time
        end
    end
    
    -- Session detection
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
    
    -- Basic telemetry (always available)
    if FlightHubExport.features.telemetry_basic then
        if LoGetSelfData then
            local self_data = LoGetSelfData()
            if self_data then
                aircraft_name = self_data.Name or "Unknown"
                data.aircraft = aircraft_name
                
                if self_data.LatLongAlt then
                    data.latitude = self_data.LatLongAlt.Lat
                    data.longitude = self_data.LatLongAlt.Long
                    data.altitude = self_data.LatLongAlt.Alt
                end
                
                if self_data.Heading then
                    data.heading = math.deg(self_data.Heading)
                end
                
                if self_data.Pitch then
                    data.pitch = math.deg(self_data.Pitch)
                end
                
                if self_data.Bank then
                    data.bank = math.deg(self_data.Bank)
                end
            end
        end
        
        if LoGetIndicatedAirSpeed then
            data.ias = LoGetIndicatedAirSpeed()
        end
        
        if LoGetTrueAirSpeed then
            data.tas = LoGetTrueAirSpeed()
        end
        
        if LoGetAltitudeAboveSeaLevel then
            data.altitude_asl = LoGetAltitudeAboveSeaLevel()
        end
        
        if LoGetVerticalVelocity then
            data.vertical_speed = LoGetVerticalVelocity()
        end
        
        if LoGetAccelerationUnits then
            local accel = LoGetAccelerationUnits()
            if accel then
                data.g_force = accel.y or 1.0
                data.g_lateral = accel.x or 0.0
                data.g_longitudinal = accel.z or 0.0
            end
        end
    end
    
    -- Navigation data (MP-safe)
    if FlightHubExport.features.telemetry_navigation then
        if LoGetRoute then
            local route = LoGetRoute()
            if route and route.goto_point then
                data.waypoint_distance = route.goto_point.dist
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
    
    -- Engine data (MP-safe)
    if FlightHubExport.features.telemetry_engines then
        if LoGetEngineInfo then
            local engines = LoGetEngineInfo()
            if engines then
                data.engines = {}
                for i, engine in pairs(engines) do
                    data.engines[i] = {
                        rpm = engine.RPM,
                        temperature = engine.Temperature,
                        fuel_flow = engine.FuelFlow
                    }
                end
            end
        end
    end
    
    -- Weapons data (MP-blocked)
    if FlightHubExport.features.telemetry_weapons and (not is_mp or not mp_safe_mode) then
        if LoGetPayloadInfo then
            local payload = LoGetPayloadInfo()
            if payload then
                data.weapons = {}
                for station, weapon in pairs(payload.Stations) do
                    if weapon.weapon then
                        data.weapons[station] = {
                            name = weapon.weapon.displayName,
                            count = weapon.count
                        }
                    end
                end
            end
        end
    end
    
    -- Countermeasures data (MP-blocked)
    if FlightHubExport.features.telemetry_countermeasures and (not is_mp or not mp_safe_mode) then
        if LoGetSnares then
            local snares = LoGetSnares()
            if snares then
                data.countermeasures = {
                    chaff = snares.chaff,
                    flare = snares.flare
                }
            end
        end
    end
    
    return data, aircraft_name
end"#.to_string()
    }

    /// Generate main loop code
    fn generate_main_loop(&self) -> String {
        format!(
            r#"
-- Main export loop
FlightHubExport.last_update = 0

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
        )
    }

    /// Generate script footer
    fn generate_footer(&self) -> String {
        r#"
-- DCS Export hooks
local function LuaExportStart()
    -- Initialize Flight Hub export
    FlightHubExport.connect()
end

local function LuaExportBeforeNextFrame()
    -- Update telemetry
    FlightHubExport.update()
end

local function LuaExportStop()
    -- Clean up connection
    if FlightHubExport.socket then
        FlightHubExport.socket:close()
        FlightHubExport.socket = nil
    end
    FlightHubExport.connected = false
end

-- Hook into DCS export system
DCS.setUserCallbacks({
    onSimulationStart = LuaExportStart,
    onSimulationFrame = LuaExportBeforeNextFrame,
    onSimulationStop = LuaExportStop
})

-- Legacy hook support (if other exports exist)
if LuaExportStart_Original then
    local original_start = LuaExportStart_Original
    LuaExportStart_Original = function()
        original_start()
        LuaExportStart()
    end
else
    LuaExportStart_Original = LuaExportStart
end

if LuaExportBeforeNextFrame_Original then
    local original_frame = LuaExportBeforeNextFrame_Original
    LuaExportBeforeNextFrame_Original = function()
        original_frame()
        LuaExportBeforeNextFrame()
    end
else
    LuaExportBeforeNextFrame_Original = LuaExportBeforeNextFrame
end

if LuaExportStop_Original then
    local original_stop = LuaExportStop_Original
    LuaExportStop_Original = function()
        original_stop()
        LuaExportStop()
    end
else
    LuaExportStop_Original = LuaExportStop
end"#.to_string()
    }

    /// Write script to file
    pub fn write_script(&self, path: &Path) -> Result<()> {
        let script_content = self.generate_script();
        std::fs::write(path, script_content)
            .with_context(|| format!("Failed to write Export.lua to {}", path.display()))?;
        Ok(())
    }

    /// Get default DCS Saved Games path
    pub fn get_dcs_saved_games_path() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .context("Could not determine home directory")?;
        
        // Try common DCS paths
        let dcs_paths = [
            "Saved Games/DCS.openbeta",
            "Saved Games/DCS",
            "Documents/DCS.openbeta", 
            "Documents/DCS",
        ];

        for path in &dcs_paths {
            let full_path = home_dir.join(path);
            if full_path.exists() {
                return Ok(full_path);
            }
        }

        // Default to DCS.openbeta if none found
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
        assert!(script.contains("DCS.setUserCallbacks"));
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
}