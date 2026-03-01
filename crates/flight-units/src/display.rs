// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Formatted display helpers for aviation values.

use crate::SpeedUnit;

/// Default transition altitude in feet (below → "ft", at/above → "FL").
const DEFAULT_TRANSITION_ALT: f64 = 18_000.0;

/// Format an altitude for display.
///
/// Altitudes at or above the transition altitude (default 18 000 ft)
/// are shown as flight levels (e.g. `"FL350"`). Below transition they
/// are shown as comma-separated feet (e.g. `"5,000 ft"`).
pub fn format_altitude(feet: f64) -> String {
    format_altitude_with_transition(feet, DEFAULT_TRANSITION_ALT)
}

/// Format an altitude with a custom transition altitude.
pub fn format_altitude_with_transition(feet: f64, transition_ft: f64) -> String {
    if feet >= transition_ft {
        let fl = (feet / 100.0).round() as i32;
        format!("FL{fl}")
    } else {
        let rounded = feet.round() as i64;
        let negative = rounded < 0;
        let abs_val = rounded.unsigned_abs();
        let formatted = format_with_commas(abs_val);
        if negative {
            format!("-{formatted} ft")
        } else {
            format!("{formatted} ft")
        }
    }
}

/// Format a speed in the requested display unit.
pub fn format_speed(knots: f64, unit: SpeedUnit) -> String {
    match unit {
        SpeedUnit::Knots => format!("{} kt", knots.round() as i64),
        SpeedUnit::Mps => {
            let mps = crate::conversions::knots_to_mps_f64(knots);
            format!("{:.1} m/s", mps)
        }
        SpeedUnit::Kph => {
            let kph = crate::conversions::knots_to_kph_f64(knots);
            format!("{} km/h", kph.round() as i64)
        }
        SpeedUnit::Mph => {
            let mph = crate::conversions::knots_to_mph(knots);
            format!("{} mph", mph.round() as i64)
        }
    }
}

/// Format a heading for display.
///
/// Always 3 digits; uses `"360°"` instead of `"000°"`.
pub fn format_heading(degrees: f64) -> String {
    let normalized = ((degrees % 360.0) + 360.0) % 360.0;
    let rounded = normalized.round() as u32;
    let display = if rounded == 0 { 360 } else { rounded };
    format!("{display:03}°")
}

/// Format a barometric pressure for display (QNH in hPa).
pub fn format_pressure(hpa: f64) -> String {
    format!("QNH {}", hpa.round() as i64)
}

/// Format a temperature for display with an explicit sign.
pub fn format_temperature(celsius: f64) -> String {
    let rounded = celsius.round() as i64;
    if rounded >= 0 {
        format!("+{rounded}°C")
    } else {
        format!("{rounded}°C")
    }
}

/// Insert thousands-separating commas into an integer.
fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Altitude ────────────────────────────────────────────────

    #[test]
    fn altitude_flight_level() {
        assert_eq!(format_altitude(35_000.0), "FL350");
        assert_eq!(format_altitude(41_000.0), "FL410");
    }

    #[test]
    fn altitude_below_transition() {
        assert_eq!(format_altitude(5_000.0), "5,000 ft");
        assert_eq!(format_altitude(500.0), "500 ft");
        assert_eq!(format_altitude(10_500.0), "10,500 ft");
    }

    #[test]
    fn altitude_at_transition() {
        assert_eq!(format_altitude(18_000.0), "FL180");
    }

    #[test]
    fn altitude_zero() {
        assert_eq!(format_altitude(0.0), "0 ft");
    }

    // ── Speed ───────────────────────────────────────────────────

    #[test]
    fn speed_knots() {
        assert_eq!(format_speed(250.0, SpeedUnit::Knots), "250 kt");
    }

    #[test]
    fn speed_kph() {
        assert_eq!(format_speed(250.0, SpeedUnit::Kph), "463 km/h");
    }

    #[test]
    fn speed_mps() {
        let s = format_speed(100.0, SpeedUnit::Mps);
        assert!(s.contains("m/s"));
        assert!(s.starts_with("51.4"));
    }

    #[test]
    fn speed_mph() {
        assert_eq!(format_speed(100.0, SpeedUnit::Mph), "115 mph");
    }

    // ── Heading ─────────────────────────────────────────────────

    #[test]
    fn heading_north() {
        assert_eq!(format_heading(0.0), "360°");
        assert_eq!(format_heading(360.0), "360°");
    }

    #[test]
    fn heading_three_digits() {
        assert_eq!(format_heading(5.0), "005°");
        assert_eq!(format_heading(90.0), "090°");
        assert_eq!(format_heading(270.0), "270°");
    }

    #[test]
    fn heading_negative() {
        assert_eq!(format_heading(-90.0), "270°");
    }

    // ── Pressure ────────────────────────────────────────────────

    #[test]
    fn pressure_standard() {
        assert_eq!(format_pressure(1013.25), "QNH 1013");
    }

    #[test]
    fn pressure_low() {
        assert_eq!(format_pressure(998.7), "QNH 999");
    }

    // ── Temperature ─────────────────────────────────────────────

    #[test]
    fn temperature_positive() {
        assert_eq!(format_temperature(15.0), "+15°C");
    }

    #[test]
    fn temperature_negative() {
        assert_eq!(format_temperature(-5.0), "-5°C");
    }

    #[test]
    fn temperature_zero() {
        assert_eq!(format_temperature(0.0), "+0°C");
    }

    #[test]
    fn temperature_extreme_cold() {
        assert_eq!(format_temperature(-56.5), "-57°C");
    }

    // ── Commas helper ───────────────────────────────────────────

    #[test]
    fn commas_formatting() {
        assert_eq!(format_with_commas(0), "0");
        assert_eq!(format_with_commas(999), "999");
        assert_eq!(format_with_commas(1_000), "1,000");
        assert_eq!(format_with_commas(1_000_000), "1,000,000");
    }
}
