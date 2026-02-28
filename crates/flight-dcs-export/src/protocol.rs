// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS Export.lua UDP protocol parser
//!
//! Parses the structured text format sent by DCS Export.lua over UDP.
//! Each UDP packet contains newline-separated `key=value` pairs with a
//! header line identifying the packet type and timestamp.

use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during DCS export protocol parsing.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("missing header line in telemetry batch")]
    MissingHeader,
    #[error("invalid header format: {0}")]
    InvalidHeader(String),
    #[error("invalid key=value format: {0}")]
    InvalidKeyValue(String),
    #[error("invalid numeric value for '{key}': {raw}")]
    InvalidNumeric { key: String, raw: String },
    #[error("missing required field: {0}")]
    MissingField(String),
}

/// A single key=value entry from the DCS export stream.
#[derive(Debug, Clone, PartialEq)]
pub struct DcsExportEntry {
    pub key: String,
    pub value: String,
}

/// Flight data extracted from a DCS telemetry packet.
#[derive(Debug, Clone, PartialEq)]
pub struct DcsFlightData {
    pub altitude_m: f64,
    pub airspeed_ms: f64,
    pub heading_deg: f64,
    pub pitch_deg: f64,
    pub roll_deg: f64,
    pub aoa_deg: f64,
    pub g_load: f64,
    pub mach: f64,
    pub vertical_speed_ms: f64,
    pub engine_rpm_percent: Vec<f64>,
    pub fuel_total_kg: f64,
    pub gear_position: Vec<f64>,
}

impl Default for DcsFlightData {
    fn default() -> Self {
        Self {
            altitude_m: 0.0,
            airspeed_ms: 0.0,
            heading_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
            aoa_deg: 0.0,
            g_load: 1.0,
            mach: 0.0,
            vertical_speed_ms: 0.0,
            engine_rpm_percent: Vec::new(),
            fuel_total_kg: 0.0,
            gear_position: Vec::new(),
        }
    }
}

/// Complete telemetry packet as sent by Export.lua.
#[derive(Debug, Clone, PartialEq)]
pub struct DcsTelemetryPacket {
    pub timestamp: f64,
    pub model_time: f64,
    pub aircraft_name: String,
    pub indicators: HashMap<String, f64>,
    pub flight_data: DcsFlightData,
}

/// Parse a single `key=value` export line.
///
/// Lines may contain leading/trailing whitespace and optional comments
/// starting with `--` (Lua comment syntax).
pub fn parse_export_line(line: &str) -> Result<DcsExportEntry, ParseError> {
    // Strip Lua-style comments
    let line = line.split("--").next().unwrap_or("").trim();
    if line.is_empty() {
        return Err(ParseError::InvalidKeyValue("empty line".into()));
    }

    let (key, value) = line
        .split_once('=')
        .ok_or_else(|| ParseError::InvalidKeyValue(line.to_string()))?;

    let key = key.trim();
    let value = value.trim();

    if key.is_empty() {
        return Err(ParseError::InvalidKeyValue(line.to_string()));
    }

    Ok(DcsExportEntry {
        key: key.to_string(),
        value: value.to_string(),
    })
}

/// Parse a DCS indicator/numeric value string.
///
/// Handles DCS quirks: trailing whitespace, Lua `inf`/`nan` literals,
/// scientific notation, and bare `-` for zero.
pub fn parse_indicator_value(raw: &str) -> Result<f64, ParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return Ok(0.0);
    }

    // Lua-style special values
    let lower = trimmed.to_ascii_lowercase();
    if lower == "inf" || lower == "1/0" {
        return Ok(f64::INFINITY);
    }
    if lower == "-inf" || lower == "-1/0" {
        return Ok(f64::NEG_INFINITY);
    }
    if lower == "nan" || lower == "0/0" {
        return Ok(f64::NAN);
    }

    trimmed
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidNumeric {
            key: String::new(),
            raw: raw.to_string(),
        })
}

/// Parse a full UDP telemetry batch.
///
/// Expected format:
/// ```text
/// HEADER:timestamp=<f64>,model_time=<f64>,aircraft=<name>
/// key1=value1
/// key2=value2
/// ...
/// ```
///
/// Flight-data keys are mapped to [`DcsFlightData`] fields; all other
/// numeric keys are collected into `indicators`.
pub fn parse_telemetry_batch(data: &str) -> Result<DcsTelemetryPacket, ParseError> {
    let mut lines = data.lines();

    // --- Parse header ---
    let header_line = lines.next().ok_or(ParseError::MissingHeader)?;
    let header_body = header_line
        .strip_prefix("HEADER:")
        .ok_or_else(|| ParseError::InvalidHeader(header_line.to_string()))?;

    let header_map = parse_comma_pairs(header_body)?;

    let timestamp = header_map
        .get("timestamp")
        .ok_or_else(|| ParseError::MissingField("timestamp".into()))
        .and_then(|v| parse_indicator_value(v).map_err(|_| ParseError::InvalidHeader(v.clone())))?;
    let model_time = header_map
        .get("model_time")
        .ok_or_else(|| ParseError::MissingField("model_time".into()))
        .and_then(|v| parse_indicator_value(v).map_err(|_| ParseError::InvalidHeader(v.clone())))?;
    let aircraft_name = header_map
        .get("aircraft")
        .ok_or_else(|| ParseError::MissingField("aircraft".into()))?
        .clone();

    // --- Parse body key=value lines ---
    let mut raw: HashMap<String, String> = HashMap::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = parse_export_line(trimmed) {
            raw.insert(entry.key, entry.value);
        }
    }

    // Build flight data from well-known keys
    let mut flight_data = DcsFlightData::default();
    let mut indicators: HashMap<String, f64> = HashMap::new();

    // Well-known flight-data keys
    const FLIGHT_KEYS: &[&str] = &[
        "altitude_m",
        "airspeed_ms",
        "heading_deg",
        "pitch_deg",
        "roll_deg",
        "aoa_deg",
        "g_load",
        "mach",
        "vertical_speed_ms",
        "fuel_total_kg",
    ];

    for (key, value) in &raw {
        if key.starts_with("engine_rpm_") {
            if let Ok(v) = parse_indicator_value(value) {
                flight_data.engine_rpm_percent.push(v);
            }
        } else if key.starts_with("gear_") {
            if let Ok(v) = parse_indicator_value(value) {
                flight_data.gear_position.push(v);
            }
        } else if FLIGHT_KEYS.contains(&key.as_str()) {
            if let Ok(v) = parse_indicator_value(value) {
                match key.as_str() {
                    "altitude_m" => flight_data.altitude_m = v,
                    "airspeed_ms" => flight_data.airspeed_ms = v,
                    "heading_deg" => flight_data.heading_deg = v,
                    "pitch_deg" => flight_data.pitch_deg = v,
                    "roll_deg" => flight_data.roll_deg = v,
                    "aoa_deg" => flight_data.aoa_deg = v,
                    "g_load" => flight_data.g_load = v,
                    "mach" => flight_data.mach = v,
                    "vertical_speed_ms" => flight_data.vertical_speed_ms = v,
                    "fuel_total_kg" => flight_data.fuel_total_kg = v,
                    _ => {}
                }
            }
        } else if let Ok(v) = parse_indicator_value(value) {
            indicators.insert(key.clone(), v);
        }
    }

    // Sort engine/gear vectors for deterministic output
    flight_data
        .engine_rpm_percent
        .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    flight_data
        .gear_position
        .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    Ok(DcsTelemetryPacket {
        timestamp,
        model_time,
        aircraft_name,
        indicators,
        flight_data,
    })
}

/// Parse `key=val,key=val,...` header pairs.
fn parse_comma_pairs(s: &str) -> Result<HashMap<String, String>, ParseError> {
    let mut map = HashMap::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) = part
            .split_once('=')
            .ok_or_else(|| ParseError::InvalidHeader(part.to_string()))?;
        map.insert(k.trim().to_string(), v.trim().to_string());
    }
    Ok(map)
}

/// Parse a LoGetSelfData-style structured position string.
///
/// DCS LoGetSelfData returns lat/lon/alt as a Lua table serialised like:
/// `{lat=42.123,lon=-71.456,alt=5000.0}`
///
/// Returns `(latitude, longitude, altitude)` in degrees/metres.
pub fn parse_position_data(raw: &str) -> Result<(f64, f64, f64), ParseError> {
    let trimmed = raw.trim().trim_matches(|c| c == '{' || c == '}');
    let pairs = parse_comma_pairs(trimmed)?;

    let lat = pairs
        .get("lat")
        .ok_or_else(|| ParseError::MissingField("lat".into()))
        .and_then(|v| {
            parse_indicator_value(v).map_err(|_| ParseError::InvalidNumeric {
                key: "lat".into(),
                raw: v.clone(),
            })
        })?;
    let lon = pairs
        .get("lon")
        .ok_or_else(|| ParseError::MissingField("lon".into()))
        .and_then(|v| {
            parse_indicator_value(v).map_err(|_| ParseError::InvalidNumeric {
                key: "lon".into(),
                raw: v.clone(),
            })
        })?;
    let alt = pairs
        .get("alt")
        .ok_or_else(|| ParseError::MissingField("alt".into()))
        .and_then(|v| {
            parse_indicator_value(v).map_err(|_| ParseError::InvalidNumeric {
                key: "alt".into(),
                raw: v.clone(),
            })
        })?;

    Ok((lat, lon, alt))
}

/// Parse a semicolon-separated multi-value list.
///
/// Some DCS exports pack multiple values into a single key using `;` as a
/// delimiter, e.g. `engine_rpm=95.0;94.5` for twin engines.
pub fn parse_multi_value(raw: &str) -> Result<Vec<f64>, ParseError> {
    raw.split(';')
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            parse_indicator_value(s.trim()).map_err(|_| ParseError::InvalidNumeric {
                key: String::new(),
                raw: s.to_string(),
            })
        })
        .collect()
}

/// Parse a DCS device argument string.
///
/// Device arguments in DCS have the format `device_id:command_id:value`,
/// used for cockpit clickable elements.
pub fn parse_device_arg(raw: &str) -> Result<(u32, u32, f64), ParseError> {
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 3 {
        return Err(ParseError::InvalidKeyValue(format!(
            "expected device_id:command_id:value, got '{raw}'"
        )));
    }

    let device_id: u32 = parts[0]
        .trim()
        .parse()
        .map_err(|_| ParseError::InvalidNumeric {
            key: "device_id".into(),
            raw: parts[0].to_string(),
        })?;
    let command_id: u32 = parts[1]
        .trim()
        .parse()
        .map_err(|_| ParseError::InvalidNumeric {
            key: "command_id".into(),
            raw: parts[1].to_string(),
        })?;
    let value = parse_indicator_value(parts[2].trim())?;

    Ok((device_id, command_id, value))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_export_line tests ---

    #[test]
    fn test_parse_simple_line() {
        let entry = parse_export_line("altitude_m=5000.5").unwrap();
        assert_eq!(entry.key, "altitude_m");
        assert_eq!(entry.value, "5000.5");
    }

    #[test]
    fn test_parse_line_with_whitespace() {
        let entry = parse_export_line("  heading_deg = 270.0  ").unwrap();
        assert_eq!(entry.key, "heading_deg");
        assert_eq!(entry.value, "270.0");
    }

    #[test]
    fn test_parse_line_with_lua_comment() {
        let entry = parse_export_line("mach=0.85 -- transonic").unwrap();
        assert_eq!(entry.key, "mach");
        assert_eq!(entry.value, "0.85");
    }

    #[test]
    fn test_parse_empty_line_error() {
        assert!(parse_export_line("").is_err());
    }

    #[test]
    fn test_parse_no_equals_error() {
        assert!(parse_export_line("no_separator").is_err());
    }

    #[test]
    fn test_parse_empty_key_error() {
        assert!(parse_export_line("=value").is_err());
    }

    #[test]
    fn test_parse_empty_value_ok() {
        let entry = parse_export_line("key=").unwrap();
        assert_eq!(entry.key, "key");
        assert_eq!(entry.value, "");
    }

    // --- parse_indicator_value tests ---

    #[test]
    fn test_parse_integer() {
        assert!((parse_indicator_value("42").unwrap() - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_float() {
        assert!((parse_indicator_value("3.14").unwrap() - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_parse_negative() {
        assert!((parse_indicator_value("-9.8").unwrap() - (-9.8)).abs() < 1e-10);
    }

    #[test]
    fn test_parse_scientific() {
        assert!((parse_indicator_value("1.5e3").unwrap() - 1500.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_dash_is_zero() {
        assert!((parse_indicator_value("-").unwrap()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_empty_is_zero() {
        assert!((parse_indicator_value("").unwrap()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_lua_inf() {
        assert!(parse_indicator_value("inf").unwrap().is_infinite());
        assert!(parse_indicator_value("-inf").unwrap().is_infinite());
    }

    #[test]
    fn test_parse_lua_nan() {
        assert!(parse_indicator_value("nan").unwrap().is_nan());
    }

    #[test]
    fn test_parse_lua_division_inf() {
        assert!(parse_indicator_value("1/0").unwrap().is_infinite());
    }

    #[test]
    fn test_parse_invalid_string() {
        assert!(parse_indicator_value("abc").is_err());
    }

    // --- parse_telemetry_batch tests ---

    fn sample_batch() -> String {
        [
            "HEADER:timestamp=1234.5,model_time=600.0,aircraft=F-16C_50",
            "altitude_m=5000.0",
            "airspeed_ms=250.0",
            "heading_deg=90.0",
            "pitch_deg=5.0",
            "roll_deg=-10.0",
            "aoa_deg=3.2",
            "g_load=1.5",
            "mach=0.85",
            "vertical_speed_ms=2.0",
            "fuel_total_kg=3200.0",
            "engine_rpm_0=95.0",
            "engine_rpm_1=94.5",
            "gear_nose=0.0",
            "gear_left=0.0",
            "gear_right=0.0",
            "custom_indicator=42.0",
        ]
        .join("\n")
    }

    #[test]
    fn test_parse_full_batch() {
        let pkt = parse_telemetry_batch(&sample_batch()).unwrap();
        assert!((pkt.timestamp - 1234.5).abs() < f64::EPSILON);
        assert!((pkt.model_time - 600.0).abs() < f64::EPSILON);
        assert_eq!(pkt.aircraft_name, "F-16C_50");
        assert!((pkt.flight_data.altitude_m - 5000.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.mach - 0.85).abs() < 1e-10);
        assert_eq!(pkt.flight_data.engine_rpm_percent.len(), 2);
        assert_eq!(pkt.flight_data.gear_position.len(), 3);
        assert!(pkt.indicators.contains_key("custom_indicator"));
    }

    #[test]
    fn test_parse_batch_missing_header() {
        assert!(parse_telemetry_batch("altitude_m=5000").is_err());
    }

    #[test]
    fn test_parse_batch_missing_timestamp() {
        let data = "HEADER:model_time=1.0,aircraft=Su-25T\naltitude_m=100";
        assert!(parse_telemetry_batch(data).is_err());
    }

    #[test]
    fn test_parse_batch_missing_aircraft() {
        let data = "HEADER:timestamp=1.0,model_time=1.0\naltitude_m=100";
        assert!(parse_telemetry_batch(data).is_err());
    }

    #[test]
    fn test_parse_batch_empty_body() {
        let data = "HEADER:timestamp=1.0,model_time=1.0,aircraft=A-10C\n";
        let pkt = parse_telemetry_batch(data).unwrap();
        assert_eq!(pkt.aircraft_name, "A-10C");
        assert!(pkt.indicators.is_empty());
    }

    #[test]
    fn test_parse_batch_skips_blank_lines() {
        let data = "HEADER:timestamp=0.0,model_time=0.0,aircraft=F-14B\n\naltitude_m=1000\n\n";
        let pkt = parse_telemetry_batch(data).unwrap();
        assert!((pkt.flight_data.altitude_m - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_batch_with_comments() {
        let data = "HEADER:timestamp=0.0,model_time=0.0,aircraft=Ka-50\nmach=0.3 -- subsonic";
        let pkt = parse_telemetry_batch(data).unwrap();
        assert!((pkt.flight_data.mach - 0.3).abs() < 1e-10);
    }

    // --- parse_position_data tests ---

    #[test]
    fn test_parse_position_data_basic() {
        let (lat, lon, alt) = parse_position_data("{lat=42.123,lon=-71.456,alt=5000.0}").unwrap();
        assert!((lat - 42.123).abs() < 1e-10);
        assert!((lon - (-71.456)).abs() < 1e-10);
        assert!((alt - 5000.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_position_data_no_braces() {
        let (lat, lon, alt) = parse_position_data("lat=10.0,lon=20.0,alt=100.0").unwrap();
        assert!((lat - 10.0).abs() < 1e-10);
        assert!((lon - 20.0).abs() < 1e-10);
        assert!((alt - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_position_data_with_spaces() {
        let (lat, lon, alt) =
            parse_position_data("{ lat = 55.75, lon = 37.62, alt = 150.0 }").unwrap();
        assert!((lat - 55.75).abs() < 1e-10);
        assert!((lon - 37.62).abs() < 1e-10);
        assert!((alt - 150.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_position_data_missing_field() {
        assert!(parse_position_data("{lat=10.0,lon=20.0}").is_err());
        assert!(parse_position_data("{lat=10.0,alt=100.0}").is_err());
        assert!(parse_position_data("{lon=20.0,alt=100.0}").is_err());
    }

    #[test]
    fn test_parse_position_data_negative_coords() {
        let (lat, lon, alt) = parse_position_data("{lat=-33.87,lon=151.21,alt=0.0}").unwrap();
        assert!((lat - (-33.87)).abs() < 1e-10);
        assert!((lon - 151.21).abs() < 1e-10);
        assert!(alt.abs() < 1e-10);
    }

    // --- parse_multi_value tests ---

    #[test]
    fn test_parse_multi_value_basic() {
        let vals = parse_multi_value("95.0;94.5").unwrap();
        assert_eq!(vals.len(), 2);
        assert!((vals[0] - 95.0).abs() < 1e-10);
        assert!((vals[1] - 94.5).abs() < 1e-10);
    }

    #[test]
    fn test_parse_multi_value_single() {
        let vals = parse_multi_value("100.0").unwrap();
        assert_eq!(vals.len(), 1);
        assert!((vals[0] - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_multi_value_with_spaces() {
        let vals = parse_multi_value(" 1.0 ; 2.0 ; 3.0 ").unwrap();
        assert_eq!(vals.len(), 3);
        assert!((vals[0] - 1.0).abs() < 1e-10);
        assert!((vals[1] - 2.0).abs() < 1e-10);
        assert!((vals[2] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_multi_value_trailing_semicolon() {
        let vals = parse_multi_value("1.0;2.0;").unwrap();
        assert_eq!(vals.len(), 2);
    }

    #[test]
    fn test_parse_multi_value_empty() {
        let vals = parse_multi_value("").unwrap();
        assert!(vals.is_empty());
    }

    #[test]
    fn test_parse_multi_value_invalid_element() {
        assert!(parse_multi_value("1.0;abc;3.0").is_err());
    }

    #[test]
    fn test_parse_multi_value_negative() {
        let vals = parse_multi_value("-1.0;0.0;1.0").unwrap();
        assert_eq!(vals.len(), 3);
        assert!((vals[0] - (-1.0)).abs() < 1e-10);
    }

    // --- parse_device_arg tests ---

    #[test]
    fn test_parse_device_arg_basic() {
        let (dev, cmd, val) = parse_device_arg("0:3001:1.0").unwrap();
        assert_eq!(dev, 0);
        assert_eq!(cmd, 3001);
        assert!((val - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_device_arg_negative_value() {
        let (dev, cmd, val) = parse_device_arg("4:101:-0.5").unwrap();
        assert_eq!(dev, 4);
        assert_eq!(cmd, 101);
        assert!((val - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn test_parse_device_arg_with_spaces() {
        let (dev, cmd, val) = parse_device_arg(" 1 : 200 : 0.75 ").unwrap();
        assert_eq!(dev, 1);
        assert_eq!(cmd, 200);
        assert!((val - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_parse_device_arg_too_few_parts() {
        assert!(parse_device_arg("0:3001").is_err());
    }

    #[test]
    fn test_parse_device_arg_too_many_parts() {
        assert!(parse_device_arg("0:3001:1.0:extra").is_err());
    }

    #[test]
    fn test_parse_device_arg_invalid_device_id() {
        assert!(parse_device_arg("abc:3001:1.0").is_err());
    }

    #[test]
    fn test_parse_device_arg_zero_value() {
        let (dev, cmd, val) = parse_device_arg("0:100:0.0").unwrap();
        assert_eq!(dev, 0);
        assert_eq!(cmd, 100);
        assert!(val.abs() < 1e-10);
    }

    // --- Additional parse_indicator_value edge cases ---

    #[test]
    fn test_parse_indicator_lua_division_neg_inf() {
        assert!(parse_indicator_value("-1/0").unwrap().is_infinite());
        assert!(parse_indicator_value("-1/0").unwrap().is_sign_negative());
    }

    #[test]
    fn test_parse_indicator_lua_division_nan() {
        assert!(parse_indicator_value("0/0").unwrap().is_nan());
    }

    #[test]
    fn test_parse_indicator_whitespace_only() {
        assert!((parse_indicator_value("   ").unwrap()).abs() < f64::EPSILON);
    }

    // --- Additional parse_telemetry_batch edge cases ---

    #[test]
    fn test_parse_batch_multiple_engines() {
        let data = [
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=F-15C",
            "engine_rpm_left=90.0",
            "engine_rpm_right=91.0",
            "engine_rpm_center=85.0",
        ]
        .join("\n");
        let pkt = parse_telemetry_batch(&data).unwrap();
        assert_eq!(pkt.flight_data.engine_rpm_percent.len(), 3);
    }

    #[test]
    fn test_parse_batch_custom_indicators() {
        let data = [
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=A-10C",
            "altitude_m=3000.0",
            "custom_gauge_1=0.75",
            "custom_gauge_2=0.50",
        ]
        .join("\n");
        let pkt = parse_telemetry_batch(&data).unwrap();
        assert!((pkt.indicators["custom_gauge_1"] - 0.75).abs() < 1e-10);
        assert!((pkt.indicators["custom_gauge_2"] - 0.50).abs() < 1e-10);
    }

    #[test]
    fn test_parse_batch_all_flight_data_fields() {
        let data = [
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=FA-18C",
            "altitude_m=10000.0",
            "airspeed_ms=300.0",
            "heading_deg=180.0",
            "pitch_deg=-2.5",
            "roll_deg=15.0",
            "aoa_deg=8.0",
            "g_load=2.5",
            "mach=0.92",
            "vertical_speed_ms=-5.0",
            "fuel_total_kg=4500.0",
        ]
        .join("\n");
        let pkt = parse_telemetry_batch(&data).unwrap();
        let fd = &pkt.flight_data;
        assert!((fd.altitude_m - 10000.0).abs() < f64::EPSILON);
        assert!((fd.airspeed_ms - 300.0).abs() < f64::EPSILON);
        assert!((fd.heading_deg - 180.0).abs() < f64::EPSILON);
        assert!((fd.pitch_deg - (-2.5)).abs() < f64::EPSILON);
        assert!((fd.roll_deg - 15.0).abs() < f64::EPSILON);
        assert!((fd.aoa_deg - 8.0).abs() < f64::EPSILON);
        assert!((fd.g_load - 2.5).abs() < f64::EPSILON);
        assert!((fd.mach - 0.92).abs() < 1e-10);
        assert!((fd.vertical_speed_ms - (-5.0)).abs() < f64::EPSILON);
        assert!((fd.fuel_total_kg - 4500.0).abs() < f64::EPSILON);
    }
}
