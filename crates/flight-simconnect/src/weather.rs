// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! MSFS weather data reading via SimConnect (REQ-673)
//!
//! Parses weather-related SimConnect variables (wind speed, direction,
//! temperature, pressure, visibility) from raw SimConnect response data.

/// Weather data parsed from SimConnect SimVars.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherData {
    /// Wind speed in knots.
    pub wind_speed: f64,
    /// Wind direction in degrees (0–360).
    pub wind_direction: f64,
    /// Outside air temperature in degrees Celsius.
    pub temperature: f64,
    /// Barometric pressure in millibars (hPa).
    pub pressure: f64,
    /// Visibility in statute miles.
    pub visibility: f64,
}

impl Default for WeatherData {
    fn default() -> Self {
        Self {
            wind_speed: 0.0,
            wind_direction: 0.0,
            temperature: 15.0, // ISA standard
            pressure: 1013.25, // ISA standard
            visibility: 10.0,
        }
    }
}

/// Configuration for weather data polling.
#[derive(Debug, Clone)]
pub struct WeatherConfig {
    /// Whether weather polling is enabled.
    pub enabled: bool,
    /// Poll rate in Hz.
    pub poll_rate_hz: f32,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_rate_hz: 1.0,
        }
    }
}

/// The SimConnect weather variables we subscribe to, in order.
///
/// The raw data buffer is expected to contain five consecutive `f64` values
/// in this exact order.
pub const WEATHER_SIMVARS: &[&str] = &[
    "AMBIENT WIND VELOCITY",
    "AMBIENT WIND DIRECTION",
    "AMBIENT TEMPERATURE",
    "AMBIENT PRESSURE",
    "AMBIENT VISIBILITY",
];

/// Parse weather SimVars from a raw SimConnect response buffer.
///
/// Expects a buffer of exactly 5 × 8 = 40 bytes representing five consecutive
/// little-endian `f64` values in the order defined by [`WEATHER_SIMVARS`].
pub fn parse_weather_simvars(data: &[u8]) -> WeatherData {
    const F64_SIZE: usize = std::mem::size_of::<f64>();
    const EXPECTED: usize = 5 * F64_SIZE;

    if data.len() < EXPECTED {
        return WeatherData::default();
    }

    let read_f64 = |offset: usize| -> f64 {
        let bytes: [u8; 8] = data[offset..offset + F64_SIZE]
            .try_into()
            .unwrap_or([0u8; 8]);
        f64::from_le_bytes(bytes)
    };

    WeatherData {
        wind_speed: read_f64(0),
        wind_direction: read_f64(F64_SIZE),
        temperature: read_f64(2 * F64_SIZE),
        pressure: read_f64(3 * F64_SIZE),
        visibility: read_f64(4 * F64_SIZE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_weather(w: &WeatherData) -> Vec<u8> {
        let mut buf = Vec::with_capacity(40);
        buf.extend_from_slice(&w.wind_speed.to_le_bytes());
        buf.extend_from_slice(&w.wind_direction.to_le_bytes());
        buf.extend_from_slice(&w.temperature.to_le_bytes());
        buf.extend_from_slice(&w.pressure.to_le_bytes());
        buf.extend_from_slice(&w.visibility.to_le_bytes());
        buf
    }

    #[test]
    fn test_parse_valid_data() {
        let expected = WeatherData {
            wind_speed: 15.0,
            wind_direction: 270.0,
            temperature: 22.5,
            pressure: 1015.0,
            visibility: 8.0,
        };
        let buf = encode_weather(&expected);
        let parsed = parse_weather_simvars(&buf);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_short_buffer_returns_default() {
        let parsed = parse_weather_simvars(&[0u8; 10]);
        assert_eq!(parsed, WeatherData::default());
    }

    #[test]
    fn test_default_config() {
        let config = WeatherConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.poll_rate_hz, 1.0);
    }

    #[test]
    fn test_toggle_enabled() {
        let mut config = WeatherConfig::default();
        assert!(!config.enabled);
        config.enabled = true;
        assert!(config.enabled);
    }

    #[test]
    fn test_default_weather_data_isa_values() {
        let data = WeatherData::default();
        assert_eq!(data.temperature, 15.0);
        assert_eq!(data.pressure, 1013.25);
        assert_eq!(data.visibility, 10.0);
    }

    #[test]
    fn test_weather_simvars_count() {
        assert_eq!(WEATHER_SIMVARS.len(), 5);
    }

    #[test]
    fn test_parse_zero_buffer() {
        let buf = [0u8; 40];
        let parsed = parse_weather_simvars(&buf);
        assert_eq!(parsed.wind_speed, 0.0);
        assert_eq!(parsed.wind_direction, 0.0);
        assert_eq!(parsed.temperature, 0.0);
        assert_eq!(parsed.pressure, 0.0);
        assert_eq!(parsed.visibility, 0.0);
    }
}
