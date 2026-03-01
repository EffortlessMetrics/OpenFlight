// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive unit conversions for aviation telemetry.
//!
//! Legacy `f32` helpers are preserved for backward compatibility.
//! New conversions use `f64` and are `const fn` where possible.

// ── Angle ────────────────────────────────────────────────────────

/// Convert degrees to radians (f32).
pub fn degrees_to_radians(degrees: f32) -> f32 {
    degrees.to_radians()
}

/// Convert radians to degrees (f32).
pub fn radians_to_degrees(radians: f32) -> f32 {
    radians.to_degrees()
}

/// Convert degrees to radians (f64).
pub const fn deg_to_rad(degrees: f64) -> f64 {
    degrees * (std::f64::consts::PI / 180.0)
}

/// Convert radians to degrees (f64).
pub const fn rad_to_deg(radians: f64) -> f64 {
    radians * (180.0 / std::f64::consts::PI)
}

// ── Speed (f32 legacy) ──────────────────────────────────────────

/// Convert knots to meters per second.
pub fn knots_to_mps(knots: f32) -> f32 {
    knots * 0.514444
}

/// Convert meters per second to knots.
pub fn mps_to_knots(mps: f32) -> f32 {
    mps / 0.514444
}

/// Convert kilometers per hour to meters per second.
pub fn kph_to_mps(kph: f32) -> f32 {
    kph * 0.277778
}

/// Convert meters per second to kilometers per hour.
pub fn mps_to_kph(mps: f32) -> f32 {
    mps * 3.6
}

/// Convert knots to kilometers per hour.
pub fn knots_to_kph(knots: f32) -> f32 {
    mps_to_kph(knots_to_mps(knots))
}

/// Convert kilometers per hour to knots.
pub fn kph_to_knots(kph: f32) -> f32 {
    mps_to_knots(kph_to_mps(kph))
}

/// Convert knots to miles per hour (f32).
pub fn knots_to_mph_f32(knots: f32) -> f32 {
    knots * 1.150_78
}

/// Convert miles per hour to knots (f32).
pub fn mph_to_knots_f32(mph: f32) -> f32 {
    mph / 1.150_78
}

/// Convert miles per hour to meters per second (f32).
pub fn mph_to_mps_f32(mph: f32) -> f32 {
    mph * 0.447_04
}

// ── Speed (f64) ─────────────────────────────────────────────────

const KNOTS_TO_MPS: f64 = 0.514_444;
const MPH_TO_MPS: f64 = 0.447_04;
const FPM_TO_MPS: f64 = 0.005_08;

/// Knots → m/s (f64).
pub const fn knots_to_mps_f64(knots: f64) -> f64 {
    knots * KNOTS_TO_MPS
}

/// m/s → knots (f64).
pub const fn mps_to_knots_f64(mps: f64) -> f64 {
    mps / KNOTS_TO_MPS
}

/// Knots → km/h (f64).
pub const fn knots_to_kph_f64(knots: f64) -> f64 {
    knots * 1.852
}

/// km/h → knots (f64).
pub const fn kph_to_knots_f64(kph: f64) -> f64 {
    kph / 1.852
}

/// Knots → mph (f64).
pub const fn knots_to_mph(knots: f64) -> f64 {
    knots * 1.150_78
}

/// mph → knots (f64).
pub const fn mph_to_knots(mph: f64) -> f64 {
    mph / 1.150_78
}

/// mph → km/h.
pub const fn mph_to_kph(mph: f64) -> f64 {
    mph * 1.609_344
}

/// km/h → mph.
pub const fn kph_to_mph(kph: f64) -> f64 {
    kph / 1.609_344
}

/// mph → m/s.
pub const fn mph_to_mps(mph: f64) -> f64 {
    mph * MPH_TO_MPS
}

/// m/s → mph.
pub const fn mps_to_mph(mps: f64) -> f64 {
    mps / MPH_TO_MPS
}

/// ft/min → m/s (f64).
pub const fn fpm_to_mps_f64(fpm: f64) -> f64 {
    fpm * FPM_TO_MPS
}

/// m/s → ft/min (f64).
pub const fn mps_to_fpm_f64(mps: f64) -> f64 {
    mps / FPM_TO_MPS
}

/// Knots → ft/min.
pub const fn knots_to_fpm(knots: f64) -> f64 {
    knots * KNOTS_TO_MPS / FPM_TO_MPS
}

/// ft/min → knots.
pub const fn fpm_to_knots(fpm: f64) -> f64 {
    fpm * FPM_TO_MPS / KNOTS_TO_MPS
}

// ── Altitude / Vertical distance ────────────────────────────────

/// Convert feet to meters (f32).
pub fn feet_to_meters(feet: f32) -> f32 {
    feet * 0.3048
}

/// Convert meters to feet (f32).
pub fn meters_to_feet(meters: f32) -> f32 {
    meters / 0.3048
}

/// Convert feet per minute to meters per second (f32).
pub fn fpm_to_mps(fpm: f32) -> f32 {
    fpm * 0.00508
}

/// Convert meters per second to feet per minute (f32).
pub fn mps_to_fpm(mps: f32) -> f32 {
    mps * 196.85
}

/// Feet → meters (f64).
pub const fn feet_to_meters_f64(feet: f64) -> f64 {
    feet * 0.3048
}

/// Meters → feet (f64).
pub const fn meters_to_feet_f64(meters: f64) -> f64 {
    meters / 0.3048
}

/// Feet → flight level (FL = feet / 100, rounded).
pub const fn feet_to_flight_level(feet: f64) -> u32 {
    (feet / 100.0) as u32
}

/// Flight level → feet.
pub const fn flight_level_to_feet(fl: u32) -> f64 {
    fl as f64 * 100.0
}

// ── Pressure ────────────────────────────────────────────────────

const HPA_TO_INHG: f64 = 0.029_529_98;

/// hPa → inHg.
pub const fn hpa_to_inhg(hpa: f64) -> f64 {
    hpa * HPA_TO_INHG
}

/// inHg → hPa.
pub const fn inhg_to_hpa(inhg: f64) -> f64 {
    inhg / HPA_TO_INHG
}

/// hPa → millibars (identity; 1 hPa ≡ 1 mb).
pub const fn hpa_to_mb(hpa: f64) -> f64 {
    hpa
}

/// Millibars → hPa (identity).
pub const fn mb_to_hpa(mb: f64) -> f64 {
    mb
}

// ── Temperature ─────────────────────────────────────────────────

/// °C → °F.
pub const fn celsius_to_fahrenheit(c: f64) -> f64 {
    c * 1.8 + 32.0
}

/// °F → °C.
pub const fn fahrenheit_to_celsius(f: f64) -> f64 {
    (f - 32.0) / 1.8
}

/// °C → Kelvin.
pub const fn celsius_to_kelvin(c: f64) -> f64 {
    c + 273.15
}

/// Kelvin → °C.
pub const fn kelvin_to_celsius(k: f64) -> f64 {
    k - 273.15
}

// ── Distance (horizontal) ───────────────────────────────────────

const NM_TO_KM: f64 = 1.852;
const NM_TO_MILES: f64 = 1.150_78;

/// Nautical miles → kilometers.
pub const fn nm_to_km(nm: f64) -> f64 {
    nm * NM_TO_KM
}

/// Kilometers → nautical miles.
pub const fn km_to_nm(km: f64) -> f64 {
    km / NM_TO_KM
}

/// Nautical miles → statute miles.
pub const fn nm_to_miles(nm: f64) -> f64 {
    nm * NM_TO_MILES
}

/// Statute miles → nautical miles.
pub const fn miles_to_nm(miles: f64) -> f64 {
    miles / NM_TO_MILES
}

/// Nautical miles → meters.
pub const fn nm_to_meters(nm: f64) -> f64 {
    nm * NM_TO_KM * 1000.0
}

/// Meters → nautical miles.
pub const fn meters_to_nm(meters: f64) -> f64 {
    meters / (NM_TO_KM * 1000.0)
}

/// Kilometers → statute miles.
pub const fn km_to_miles(km: f64) -> f64 {
    km / 1.609_344
}

/// Statute miles → kilometers.
pub const fn miles_to_km(miles: f64) -> f64 {
    miles * 1.609_344
}

// ── Weight ──────────────────────────────────────────────────────

const KG_TO_LBS: f64 = 2.204_623;

/// Kilograms → pounds.
pub const fn kg_to_lbs(kg: f64) -> f64 {
    kg * KG_TO_LBS
}

/// Pounds → kilograms.
pub const fn lbs_to_kg(lbs: f64) -> f64 {
    lbs / KG_TO_LBS
}

/// Kilograms → metric tonnes.
pub const fn kg_to_tonnes(kg: f64) -> f64 {
    kg / 1000.0
}

/// Metric tonnes → kilograms.
pub const fn tonnes_to_kg(tonnes: f64) -> f64 {
    tonnes * 1000.0
}

/// Pounds → metric tonnes.
pub const fn lbs_to_tonnes(lbs: f64) -> f64 {
    kg_to_tonnes(lbs_to_kg(lbs))
}

/// Metric tonnes → pounds.
pub const fn tonnes_to_lbs(tonnes: f64) -> f64 {
    kg_to_lbs(tonnes_to_kg(tonnes))
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-4;

    // -- Speed --

    #[test]
    fn knots_to_mph_known() {
        assert!((knots_to_mph(100.0) - 115.078).abs() < 0.01);
    }

    #[test]
    fn mph_kph_roundtrip() {
        let v = 60.0;
        assert!((kph_to_mph(mph_to_kph(v)) - v).abs() < EPS);
    }

    #[test]
    fn fpm_knots_roundtrip() {
        let v = 500.0;
        assert!((knots_to_fpm(fpm_to_knots(v)) - v).abs() < 0.01);
    }

    #[test]
    fn mps_mph_roundtrip() {
        let v = 25.0;
        assert!((mps_to_mph(mph_to_mps(v)) - v).abs() < EPS);
    }

    // -- Altitude --

    #[test]
    fn flight_level_350() {
        assert_eq!(feet_to_flight_level(35_000.0), 350);
        assert!((flight_level_to_feet(350) - 35_000.0).abs() < EPS);
    }

    #[test]
    fn feet_meters_f64_roundtrip() {
        let v = 36_000.0;
        assert!((meters_to_feet_f64(feet_to_meters_f64(v)) - v).abs() < 0.01);
    }

    // -- Pressure --

    #[test]
    fn standard_pressure_conversion() {
        // 1013.25 hPa ≈ 29.92 inHg
        let inhg = hpa_to_inhg(1013.25);
        assert!((inhg - 29.92).abs() < 0.01, "got {inhg}");
    }

    #[test]
    fn hpa_inhg_roundtrip() {
        let v = 1013.25;
        assert!((inhg_to_hpa(hpa_to_inhg(v)) - v).abs() < 0.01);
    }

    #[test]
    fn hpa_mb_identity() {
        assert_eq!(hpa_to_mb(1013.25), 1013.25);
        assert_eq!(mb_to_hpa(1013.25), 1013.25);
    }

    // -- Temperature --

    #[test]
    fn celsius_fahrenheit_known() {
        assert!((celsius_to_fahrenheit(0.0) - 32.0).abs() < EPS);
        assert!((celsius_to_fahrenheit(100.0) - 212.0).abs() < EPS);
        assert!((fahrenheit_to_celsius(32.0) - 0.0).abs() < EPS);
    }

    #[test]
    fn celsius_kelvin_known() {
        assert!((celsius_to_kelvin(0.0) - 273.15).abs() < EPS);
        assert!((kelvin_to_celsius(273.15) - 0.0).abs() < EPS);
    }

    #[test]
    fn temp_roundtrip() {
        let v = 15.0;
        assert!((fahrenheit_to_celsius(celsius_to_fahrenheit(v)) - v).abs() < EPS);
        assert!((kelvin_to_celsius(celsius_to_kelvin(v)) - v).abs() < EPS);
    }

    // -- Distance --

    #[test]
    fn nm_km_known() {
        assert!((nm_to_km(1.0) - 1.852).abs() < EPS);
    }

    #[test]
    fn nm_miles_known() {
        assert!((nm_to_miles(1.0) - 1.150_78).abs() < 0.001);
    }

    #[test]
    fn distance_roundtrips() {
        let v = 100.0;
        assert!((km_to_nm(nm_to_km(v)) - v).abs() < 0.01);
        assert!((miles_to_nm(nm_to_miles(v)) - v).abs() < 0.01);
        assert!((miles_to_km(km_to_miles(v)) - v).abs() < 0.01);
        assert!((meters_to_nm(nm_to_meters(v)) - v).abs() < 0.01);
    }

    // -- Weight --

    #[test]
    fn kg_lbs_known() {
        assert!((kg_to_lbs(1.0) - 2.204_623).abs() < 0.001);
    }

    #[test]
    fn weight_roundtrips() {
        let v = 75.0;
        assert!((lbs_to_kg(kg_to_lbs(v)) - v).abs() < 0.01);
        assert!((tonnes_to_kg(kg_to_tonnes(v)) - v).abs() < EPS);
        assert!((tonnes_to_lbs(lbs_to_tonnes(v)) - v).abs() < 0.01);
    }

    // -- Angle (f64) --

    #[test]
    fn deg_rad_known() {
        assert!((deg_to_rad(180.0) - std::f64::consts::PI).abs() < EPS);
        assert!((rad_to_deg(std::f64::consts::PI) - 180.0).abs() < EPS);
    }

    #[test]
    fn deg_rad_roundtrip() {
        let v = 45.0;
        assert!((rad_to_deg(deg_to_rad(v)) - v).abs() < EPS);
    }
}
