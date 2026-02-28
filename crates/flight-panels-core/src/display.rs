// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! 7-segment display formatting for flight-sim panel values.
//!
//! All formatters produce a 5-character string suitable for encoding with
//! [`crate::led`]-level 7-segment helpers or the Saitek `LcdDisplay` type.

/// Format a magnetic heading (0–359) as a right-justified 5-character string.
///
/// Values are clamped to 0–359. Single- and double-digit headings are
/// space-padded on the left.
///
/// # Examples
///
/// ```
/// # use flight_panels_core::display::format_heading;
/// assert_eq!(format_heading(0),   "    0");
/// assert_eq!(format_heading(42),  "   42");
/// assert_eq!(format_heading(180), "  180");
/// assert_eq!(format_heading(359), "  359");
/// ```
pub fn format_heading(degrees: u16) -> String {
    let clamped = degrees % 360;
    format!("{clamped:>5}")
}

/// Format an altitude in feet as a right-justified 5-character string.
///
/// Positive values up to 99 999 and negative values down to −9 999 are
/// representable. Out-of-range values are clamped.
///
/// # Examples
///
/// ```
/// # use flight_panels_core::display::format_altitude;
/// assert_eq!(format_altitude(0),      "    0");
/// assert_eq!(format_altitude(12500),  "12500");
/// assert_eq!(format_altitude(-500),   " -500");
/// ```
pub fn format_altitude(feet: i32) -> String {
    if feet < 0 {
        let clamped = feet.max(-9999);
        format!("{clamped:>5}")
    } else {
        format!("{:>5}", feet.min(99999))
    }
}

/// Format a vertical speed in feet-per-minute as a right-justified
/// 5-character string.
///
/// Range: −9 999 to +9 999. Values are clamped to this range.
/// Positive values are shown without a sign prefix.
///
/// # Examples
///
/// ```
/// # use flight_panels_core::display::format_vs;
/// assert_eq!(format_vs(0),     "    0");
/// assert_eq!(format_vs(1200),  " 1200");
/// assert_eq!(format_vs(-800),  " -800");
/// ```
pub fn format_vs(fpm: i32) -> String {
    let clamped = fpm.clamp(-9999, 9999);
    format!("{clamped:>5}")
}

/// Format a COM frequency in kHz as a 5-digit string (implied decimal
/// between digits 3 and 4).
///
/// COM range: 118 000 – 136 975 kHz (118.000 – 136.975 MHz).
///
/// The display shows digits only — the decimal point position is fixed by
/// the panel hardware. For example, 123 500 kHz → `"12350"` (reads as 123.50).
///
/// Out-of-range inputs are clamped.
///
/// # Examples
///
/// ```
/// # use flight_panels_core::display::format_com_freq;
/// assert_eq!(format_com_freq(118_000), "11800");
/// assert_eq!(format_com_freq(123_500), "12350");
/// assert_eq!(format_com_freq(136_975), "13697");
/// ```
pub fn format_com_freq(freq_khz: u32) -> String {
    let clamped = freq_khz.clamp(118_000, 136_975);
    // Convert kHz to display digits: 123 500 → 12350
    // Drop the last digit (units of kHz) since panels display 10 kHz resolution
    let display_val = clamped / 10;
    format!("{display_val:>5}")
}

/// Format a NAV frequency in kHz as a 5-digit string (implied decimal
/// between digits 3 and 4).
///
/// NAV range: 108 000 – 117 950 kHz (108.00 – 117.95 MHz).
///
/// # Examples
///
/// ```
/// # use flight_panels_core::display::format_nav_freq;
/// assert_eq!(format_nav_freq(108_000), "10800");
/// assert_eq!(format_nav_freq(110_250), "11025");
/// assert_eq!(format_nav_freq(117_950), "11795");
/// ```
pub fn format_nav_freq(freq_khz: u32) -> String {
    let clamped = freq_khz.clamp(108_000, 117_950);
    let display_val = clamped / 10;
    format!("{display_val:>5}")
}

/// Format a transponder (XPDR) code as a right-justified 4-digit string
/// padded to 5 characters.
///
/// Valid transponder codes are 0000–7777 (octal digits only). This
/// formatter treats the input as a plain integer 0–7777 and zero-pads to
/// 4 digits.
///
/// # Examples
///
/// ```
/// # use flight_panels_core::display::format_xpdr;
/// assert_eq!(format_xpdr(1200), " 1200");
/// assert_eq!(format_xpdr(7700), " 7700");
/// assert_eq!(format_xpdr(0),    " 0000");
/// ```
pub fn format_xpdr(code: u16) -> String {
    let clamped = code.min(7777);
    format!(" {clamped:04}")
}

/// Format an ADF frequency in kHz as a right-justified 5-character string.
///
/// ADF range: 190–1 750 kHz.
///
/// # Examples
///
/// ```
/// # use flight_panels_core::display::format_adf;
/// assert_eq!(format_adf(340),  "  340");
/// assert_eq!(format_adf(1750), " 1750");
/// assert_eq!(format_adf(190),  "  190");
/// ```
pub fn format_adf(freq_khz: u16) -> String {
    let clamped = freq_khz.clamp(190, 1750);
    format!("{clamped:>5}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Heading ──────────────────────────────────────────────────────────────

    #[test]
    fn test_heading_zero() {
        assert_eq!(format_heading(0), "    0");
    }

    #[test]
    fn test_heading_single_digit() {
        assert_eq!(format_heading(5), "    5");
    }

    #[test]
    fn test_heading_three_digit() {
        assert_eq!(format_heading(270), "  270");
    }

    #[test]
    fn test_heading_max() {
        assert_eq!(format_heading(359), "  359");
    }

    #[test]
    fn test_heading_wraps_at_360() {
        assert_eq!(format_heading(360), "    0");
        assert_eq!(format_heading(361), "    1");
    }

    // ── Altitude ─────────────────────────────────────────────────────────────

    #[test]
    fn test_altitude_zero() {
        assert_eq!(format_altitude(0), "    0");
    }

    #[test]
    fn test_altitude_positive() {
        assert_eq!(format_altitude(35000), "35000");
    }

    #[test]
    fn test_altitude_negative() {
        assert_eq!(format_altitude(-200), " -200");
    }

    #[test]
    fn test_altitude_clamp_max() {
        assert_eq!(format_altitude(100_000), "99999");
    }

    #[test]
    fn test_altitude_clamp_min() {
        assert_eq!(format_altitude(-20_000), "-9999");
    }

    // ── Vertical speed ───────────────────────────────────────────────────────

    #[test]
    fn test_vs_zero() {
        assert_eq!(format_vs(0), "    0");
    }

    #[test]
    fn test_vs_positive() {
        assert_eq!(format_vs(1800), " 1800");
    }

    #[test]
    fn test_vs_negative() {
        assert_eq!(format_vs(-500), " -500");
    }

    #[test]
    fn test_vs_clamp() {
        assert_eq!(format_vs(99999), " 9999");
        assert_eq!(format_vs(-99999), "-9999");
    }

    // ── COM frequency ────────────────────────────────────────────────────────

    #[test]
    fn test_com_freq_low_end() {
        assert_eq!(format_com_freq(118_000), "11800");
    }

    #[test]
    fn test_com_freq_mid() {
        assert_eq!(format_com_freq(123_500), "12350");
    }

    #[test]
    fn test_com_freq_high_end() {
        assert_eq!(format_com_freq(136_975), "13697");
    }

    #[test]
    fn test_com_freq_guard() {
        // 121.500 MHz = 121 500 kHz → "12150"
        assert_eq!(format_com_freq(121_500), "12150");
    }

    // ── NAV frequency ────────────────────────────────────────────────────────

    #[test]
    fn test_nav_freq_low_end() {
        assert_eq!(format_nav_freq(108_000), "10800");
    }

    #[test]
    fn test_nav_freq_ils() {
        // ILS 110.30 MHz = 110 300 kHz → "11030"
        assert_eq!(format_nav_freq(110_300), "11030");
    }

    #[test]
    fn test_nav_freq_high_end() {
        assert_eq!(format_nav_freq(117_950), "11795");
    }

    // ── Transponder ──────────────────────────────────────────────────────────

    #[test]
    fn test_xpdr_vfr() {
        assert_eq!(format_xpdr(1200), " 1200");
    }

    #[test]
    fn test_xpdr_emergency() {
        assert_eq!(format_xpdr(7700), " 7700");
    }

    #[test]
    fn test_xpdr_zero_padded() {
        assert_eq!(format_xpdr(0), " 0000");
        assert_eq!(format_xpdr(77), " 0077");
    }

    // ── ADF ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_adf_low_end() {
        assert_eq!(format_adf(190), "  190");
    }

    #[test]
    fn test_adf_ndb() {
        assert_eq!(format_adf(340), "  340");
    }

    #[test]
    fn test_adf_high_end() {
        assert_eq!(format_adf(1750), " 1750");
    }

    // ── All formatters produce exactly 5 characters ──────────────────────────

    #[test]
    fn test_all_formatters_length_five() {
        assert_eq!(format_heading(0).len(), 5);
        assert_eq!(format_heading(359).len(), 5);
        assert_eq!(format_altitude(0).len(), 5);
        assert_eq!(format_altitude(-500).len(), 5);
        assert_eq!(format_altitude(35000).len(), 5);
        assert_eq!(format_vs(0).len(), 5);
        assert_eq!(format_vs(-1800).len(), 5);
        assert_eq!(format_com_freq(121_500).len(), 5);
        assert_eq!(format_nav_freq(110_300).len(), 5);
        assert_eq!(format_xpdr(1200).len(), 5);
        assert_eq!(format_adf(340).len(), 5);
    }
}
