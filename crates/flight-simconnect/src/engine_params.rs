// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! MSFS engine parameter reading via SimConnect (REQ-831)
//!
//! Parses engine-related SimConnect variables (RPM, manifold pressure, fuel
//! flow, EGT, oil temperature, oil pressure) from raw SimConnect response
//! buffers.  Supports up to 4 engines per aircraft.

/// Maximum number of engines supported.
pub const MAX_ENGINES: usize = 4;

/// Number of parameters tracked per engine.
pub const PARAMS_PER_ENGINE: usize = 6;

/// Engine parameters for a single engine.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineParameters {
    /// Engine RPM.
    pub rpm: f64,
    /// Manifold pressure in inches of mercury (inHg).
    pub manifold_pressure: f64,
    /// Fuel flow in gallons per hour.
    pub fuel_flow: f64,
    /// Exhaust gas temperature in degrees Celsius.
    pub egt: f64,
    /// Oil temperature in degrees Celsius.
    pub oil_temp: f64,
    /// Oil pressure in PSI.
    pub oil_pressure: f64,
}

impl Default for EngineParameters {
    fn default() -> Self {
        Self {
            rpm: 0.0,
            manifold_pressure: 0.0,
            fuel_flow: 0.0,
            egt: 0.0,
            oil_temp: 0.0,
            oil_pressure: 0.0,
        }
    }
}

/// SimVar name for each engine parameter, with a `{}` placeholder for
/// the 1-based engine index.
pub const ENGINE_SIMVAR_TEMPLATES: &[&str] = &[
    "GENERAL ENG RPM:{}",
    "RECIP ENG MANIFOLD PRESSURE:{}",
    "ENG FUEL FLOW GPH:{}",
    "GENERAL ENG EXHAUST GAS TEMPERATURE:{}",
    "GENERAL ENG OIL TEMPERATURE:{}",
    "GENERAL ENG OIL PRESSURE:{}",
];

/// Expand SimVar templates for the given number of engines (1-based index).
///
/// Returns a flat list of SimVar names suitable for a SimConnect data
/// definition request.
pub fn simvars_for_engines(engine_count: usize) -> Vec<String> {
    let count = engine_count.min(MAX_ENGINES);
    let mut vars = Vec::with_capacity(count * PARAMS_PER_ENGINE);
    for engine_idx in 1..=count {
        for template in ENGINE_SIMVAR_TEMPLATES {
            vars.push(template.replace("{}", &engine_idx.to_string()));
        }
    }
    vars
}

/// Parse engine parameters from a raw SimConnect response buffer.
///
/// The buffer must contain `engine_count × 6` consecutive little-endian `f64`
/// values, one set of parameters per engine in index order.
pub fn parse_engine_params(data: &[u8], engine_count: usize) -> Vec<EngineParameters> {
    const F64_SIZE: usize = std::mem::size_of::<f64>();
    let count = engine_count.min(MAX_ENGINES);
    let expected = count * PARAMS_PER_ENGINE * F64_SIZE;

    if data.len() < expected {
        return vec![EngineParameters::default(); count];
    }

    let read_f64 = |offset: usize| -> f64 {
        let bytes: [u8; 8] = data[offset..offset + F64_SIZE]
            .try_into()
            .unwrap_or([0u8; 8]);
        f64::from_le_bytes(bytes)
    };

    let mut engines = Vec::with_capacity(count);
    for i in 0..count {
        let base = i * PARAMS_PER_ENGINE * F64_SIZE;
        engines.push(EngineParameters {
            rpm: read_f64(base),
            manifold_pressure: read_f64(base + F64_SIZE),
            fuel_flow: read_f64(base + 2 * F64_SIZE),
            egt: read_f64(base + 3 * F64_SIZE),
            oil_temp: read_f64(base + 4 * F64_SIZE),
            oil_pressure: read_f64(base + 5 * F64_SIZE),
        });
    }
    engines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_params(params: &EngineParameters) -> Vec<u8> {
        let mut buf = Vec::with_capacity(PARAMS_PER_ENGINE * 8);
        buf.extend_from_slice(&params.rpm.to_le_bytes());
        buf.extend_from_slice(&params.manifold_pressure.to_le_bytes());
        buf.extend_from_slice(&params.fuel_flow.to_le_bytes());
        buf.extend_from_slice(&params.egt.to_le_bytes());
        buf.extend_from_slice(&params.oil_temp.to_le_bytes());
        buf.extend_from_slice(&params.oil_pressure.to_le_bytes());
        buf
    }

    #[test]
    fn test_parse_single_engine() {
        let expected = EngineParameters {
            rpm: 2400.0,
            manifold_pressure: 29.92,
            fuel_flow: 12.5,
            egt: 650.0,
            oil_temp: 95.0,
            oil_pressure: 60.0,
        };
        let buf = encode_params(&expected);
        let result = parse_engine_params(&buf, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_parse_multi_engine() {
        let eng1 = EngineParameters {
            rpm: 2400.0,
            manifold_pressure: 29.0,
            fuel_flow: 12.0,
            egt: 640.0,
            oil_temp: 90.0,
            oil_pressure: 58.0,
        };
        let eng2 = EngineParameters {
            rpm: 2350.0,
            manifold_pressure: 28.5,
            fuel_flow: 11.8,
            egt: 635.0,
            oil_temp: 92.0,
            oil_pressure: 57.0,
        };
        let mut buf = encode_params(&eng1);
        buf.extend_from_slice(&encode_params(&eng2));
        let result = parse_engine_params(&buf, 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], eng1);
        assert_eq!(result[1], eng2);
    }

    #[test]
    fn test_short_buffer_returns_defaults() {
        let result = parse_engine_params(&[0u8; 10], 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], EngineParameters::default());
        assert_eq!(result[1], EngineParameters::default());
    }

    #[test]
    fn test_simvars_for_single_engine() {
        let vars = simvars_for_engines(1);
        assert_eq!(vars.len(), PARAMS_PER_ENGINE);
        assert_eq!(vars[0], "GENERAL ENG RPM:1");
        assert_eq!(vars[1], "RECIP ENG MANIFOLD PRESSURE:1");
        assert_eq!(vars[5], "GENERAL ENG OIL PRESSURE:1");
    }

    #[test]
    fn test_simvars_for_four_engines() {
        let vars = simvars_for_engines(4);
        assert_eq!(vars.len(), 4 * PARAMS_PER_ENGINE);
        // Engine 1 first param
        assert_eq!(vars[0], "GENERAL ENG RPM:1");
        // Engine 4 last param
        assert_eq!(
            vars[4 * PARAMS_PER_ENGINE - 1],
            "GENERAL ENG OIL PRESSURE:4"
        );
    }

    #[test]
    fn test_engine_count_capped_at_max() {
        let vars = simvars_for_engines(10);
        assert_eq!(vars.len(), MAX_ENGINES * PARAMS_PER_ENGINE);
        let result = parse_engine_params(&[0u8; 192], 10);
        assert_eq!(result.len(), MAX_ENGINES);
    }

    #[test]
    fn test_default_engine_parameters() {
        let params = EngineParameters::default();
        assert_eq!(params.rpm, 0.0);
        assert_eq!(params.manifold_pressure, 0.0);
        assert_eq!(params.fuel_flow, 0.0);
        assert_eq!(params.egt, 0.0);
        assert_eq!(params.oil_temp, 0.0);
        assert_eq!(params.oil_pressure, 0.0);
    }

    #[test]
    fn test_simvar_templates_count() {
        assert_eq!(ENGINE_SIMVAR_TEMPLATES.len(), PARAMS_PER_ENGINE);
    }
}
