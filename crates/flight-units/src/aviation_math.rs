// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Common aviation calculations.
//!
//! Functions follow the ISA (International Standard Atmosphere) model
//! and conventional aviation formulas.

use crate::conversions;

// ISA sea-level constants
const ISA_SEA_LEVEL_TEMP_C: f64 = 15.0;
const ISA_SEA_LEVEL_PRESSURE_HPA: f64 = 1013.25;
const ISA_SEA_LEVEL_DENSITY: f64 = 1.225; // kg/m³
const ISA_LAPSE_RATE: f64 = 0.001_98; // °C per foot (≈ 1.98 °C / 1000 ft)
const ISA_TROPOPAUSE_FT: f64 = 36_089.0;
const ISA_TROPOPAUSE_TEMP_C: f64 = -56.5;

/// Compute ISA standard atmosphere properties at a given altitude.
///
/// Returns `(temperature_c, pressure_hpa, density_kg_m3)`.
///
/// Uses the troposphere linear-lapse model below 36 089 ft and an
/// isothermal layer above.
pub fn standard_atmosphere(altitude_ft: f64) -> (f64, f64, f64) {
    if altitude_ft <= ISA_TROPOPAUSE_FT {
        let temp_c = ISA_SEA_LEVEL_TEMP_C - ISA_LAPSE_RATE * altitude_ft;
        let temp_ratio = (temp_c + 273.15) / (ISA_SEA_LEVEL_TEMP_C + 273.15);
        let pressure = ISA_SEA_LEVEL_PRESSURE_HPA * temp_ratio.powf(5.255_876);
        let density = ISA_SEA_LEVEL_DENSITY * temp_ratio.powf(4.255_876);
        (temp_c, pressure, density)
    } else {
        let (_, trop_p, trop_d) = standard_atmosphere(ISA_TROPOPAUSE_FT);
        let exponent = -((altitude_ft - ISA_TROPOPAUSE_FT) * 0.3048)
            / (29.271 * (ISA_TROPOPAUSE_TEMP_C + 273.15));
        let factor = exponent.exp();
        (ISA_TROPOPAUSE_TEMP_C, trop_p * factor, trop_d * factor)
    }
}

/// Compute True Airspeed from Indicated Airspeed.
///
/// Uses the approximation TAS ≈ IAS × √(ρ₀ / ρ), correcting the ISA
/// density for the actual outside air temperature.
pub fn true_airspeed(ias: f64, altitude_ft: f64, oat_c: f64) -> f64 {
    let (isa_temp, _, isa_density) = standard_atmosphere(altitude_ft);
    let temp_ratio = (oat_c + 273.15) / (isa_temp + 273.15);
    let actual_density = isa_density / temp_ratio;
    ias * (ISA_SEA_LEVEL_DENSITY / actual_density).sqrt()
}

/// Compute density altitude from pressure altitude and OAT.
///
/// DA = pressure altitude + 120 × (OAT − ISA temperature at that altitude).
pub fn density_altitude(pressure_alt_ft: f64, oat_c: f64) -> f64 {
    let isa_temp = ISA_SEA_LEVEL_TEMP_C - ISA_LAPSE_RATE * pressure_alt_ft;
    pressure_alt_ft + 120.0 * (oat_c - isa_temp)
}

/// Compute headwind and crosswind components.
///
/// `wind_angle_deg` is the angle between the wind direction and the
/// runway/track heading. Returns `(headwind, crosswind)` — positive
/// headwind means wind from ahead.
pub fn crosswind_component(wind_speed: f64, wind_angle_deg: f64) -> (f64, f64) {
    let angle_rad = conversions::deg_to_rad(wind_angle_deg);
    let headwind = wind_speed * angle_rad.cos();
    let crosswind = wind_speed * angle_rad.sin();
    (headwind, crosswind)
}

/// Compute Mach number from TAS and OAT.
///
/// Speed of sound ≈ 38.967 × √(OAT in Kelvin) knots.
pub fn mach_number(tas_knots: f64, oat_c: f64) -> f64 {
    let oat_k = oat_c + 273.15;
    let speed_of_sound = 38.967 * oat_k.sqrt();
    tas_knots / speed_of_sound
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Standard atmosphere ─────────────────────────────────────

    #[test]
    fn isa_sea_level() {
        let (t, p, d) = standard_atmosphere(0.0);
        assert!((t - 15.0).abs() < 0.1, "temp: {t}");
        assert!((p - 1013.25).abs() < 0.1, "pressure: {p}");
        assert!((d - 1.225).abs() < 0.001, "density: {d}");
    }

    #[test]
    fn isa_at_fl350() {
        let (t, p, _) = standard_atmosphere(35_000.0);
        assert!((t - (-54.3)).abs() < 0.5, "temp: {t}");
        assert!((p - 238.0).abs() < 5.0, "pressure: {p}");
    }

    #[test]
    fn isa_above_tropopause() {
        let (t, p, _) = standard_atmosphere(40_000.0);
        assert!((t - (-56.5)).abs() < 0.1, "temp should be isothermal: {t}");
        assert!(p < 200.0 && p > 100.0, "pressure at 40k ft: {p}");
    }

    #[test]
    fn isa_temperature_decreases_with_altitude() {
        let (t0, _, _) = standard_atmosphere(0.0);
        let (t10, _, _) = standard_atmosphere(10_000.0);
        let (t30, _, _) = standard_atmosphere(30_000.0);
        assert!(t0 > t10);
        assert!(t10 > t30);
    }

    // ── True airspeed ───────────────────────────────────────────

    #[test]
    fn tas_at_sea_level_standard() {
        let tas = true_airspeed(250.0, 0.0, 15.0);
        assert!((tas - 250.0).abs() < 1.0, "TAS at SL: {tas}");
    }

    #[test]
    fn tas_increases_with_altitude() {
        let tas_low = true_airspeed(250.0, 5_000.0, 5.0);
        let tas_high = true_airspeed(250.0, 35_000.0, -54.0);
        assert!(tas_high > tas_low, "TAS should increase with altitude");
        let ratio = tas_high / 250.0;
        assert!(ratio > 1.5 && ratio < 2.0, "TAS ratio at FL350: {ratio}");
    }

    // ── Density altitude ────────────────────────────────────────

    #[test]
    fn density_altitude_standard() {
        let da = density_altitude(0.0, 15.0);
        assert!(da.abs() < 1.0, "DA at SL ISA: {da}");
    }

    #[test]
    fn density_altitude_hot_day() {
        let da = density_altitude(5_000.0, 30.0);
        assert!(
            da > 5_000.0,
            "DA should exceed field elevation on hot day: {da}"
        );
    }

    #[test]
    fn density_altitude_cold_day() {
        let da = density_altitude(5_000.0, -10.0);
        assert!(
            da < 5_000.0,
            "DA should be below field elevation on cold day: {da}"
        );
    }

    // ── Crosswind ───────────────────────────────────────────────

    #[test]
    fn crosswind_pure_headwind() {
        let (hw, xw) = crosswind_component(20.0, 0.0);
        assert!((hw - 20.0).abs() < 0.01);
        assert!(xw.abs() < 0.01);
    }

    #[test]
    fn crosswind_pure_crosswind() {
        let (hw, xw) = crosswind_component(20.0, 90.0);
        assert!(hw.abs() < 0.01);
        assert!((xw - 20.0).abs() < 0.01);
    }

    #[test]
    fn crosswind_45_degrees() {
        let (hw, xw) = crosswind_component(20.0, 45.0);
        let expected = 20.0 * std::f64::consts::FRAC_1_SQRT_2;
        assert!((hw - expected).abs() < 0.01);
        assert!((xw - expected).abs() < 0.01);
    }

    #[test]
    fn crosswind_tailwind() {
        let (hw, _) = crosswind_component(20.0, 180.0);
        assert!((hw - (-20.0)).abs() < 0.01);
    }

    // ── Mach number ─────────────────────────────────────────────

    #[test]
    fn mach_at_fl350() {
        // At FL350 ISA (−54.3 °C), Mach 1 ≈ 573 kt TAS
        let m = mach_number(573.0, -54.3);
        assert!((m - 1.0).abs() < 0.05, "Mach: {m}");
    }

    #[test]
    fn mach_typical_cruise() {
        // Typical airliner: TAS ~450 kt at −56.5 °C → Mach ~0.79
        let m = mach_number(450.0, -56.5);
        assert!(m > 0.7 && m < 0.9, "Cruise Mach: {m}");
    }

    #[test]
    fn mach_at_sea_level() {
        // At SL ISA (15 °C), Mach 1 ≈ 661 kt
        let m = mach_number(661.0, 15.0);
        assert!((m - 1.0).abs() < 0.02, "Mach at SL: {m}");
    }
}
