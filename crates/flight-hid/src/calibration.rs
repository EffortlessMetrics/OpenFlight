// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID axis calibration — raw device integer → normalised `f32` mapping.
//!
//! HID devices report axis values as unsigned integers whose range depends on
//! the hardware (10-bit → 1023, 12-bit → 4095, 16-bit → 65535, …).
//! [`AxisCalibration`] maps those raw counts to a normalised floating-point
//! value in a configurable output range with an optional centre deadzone and
//! polarity reversal.
//!
//! # Example
//!
//! ```rust
//! use flight_hid::calibration::AxisCalibration;
//!
//! // 16-bit stick axis: centre at 32767, symmetric output [-1.0, 1.0], 3% deadzone.
//! let cal = AxisCalibration {
//!     raw_min:    0,
//!     raw_max:    65535,
//!     raw_center: 32767,
//!     deadzone:   0.03,
//!     output_min: -1.0,
//!     output_max:  1.0,
//!     reversed:   false,
//! };
//!
//! assert!((cal.normalize(32767) - 0.0_f32).abs() < 1e-4, "centre → 0");
//! assert!((cal.normalize(65535) - 1.0_f32).abs() < 1e-4, "max → 1");
//! assert!((cal.normalize(0)     - (-1.0_f32)).abs() < 1e-4, "min → -1");
//! ```

/// Calibration parameters for a single HID axis channel.
///
/// Converts a raw hardware axis integer into a normalised `f32` value in
/// `[output_min, output_max]`.
///
/// # Parameter constraints
///
/// * `raw_min < raw_max` — non-degenerate raw range.  If `raw_min == raw_max`
///   the midpoint of the output range is returned for every input.
/// * `raw_min ≤ raw_center ≤ raw_max` — the centre is clamped into the raw
///   range before use, so out-of-range values are accepted but unusual.
/// * `output_min < output_max` — non-degenerate output range.
/// * `0.0 ≤ deadzone < 1.0` — clamped to `[0, 0.9999]` internally.
#[derive(Debug, Clone)]
pub struct AxisCalibration {
    /// Minimum raw value the device can report.
    pub raw_min: u32,
    /// Maximum raw value the device can report.
    pub raw_max: u32,
    /// Raw value corresponding to the neutral / centre position.
    ///
    /// For self-centring stick axes this is the mid-point of the hardware
    /// range; for throttles without a centre detent it is usually equal to
    /// `raw_min`.
    pub raw_center: u32,
    /// Deadzone radius around `raw_center`, as a fraction of the output
    /// half-range `[0.0, 1.0)`.
    ///
    /// Any raw input that falls within this radius of `raw_center` is mapped
    /// to the centre output value (i.e. zero for symmetric stick axes).
    /// Outside the deadzone the remaining range is rescaled to fill
    /// `[output_min, output_max]`.
    pub deadzone: f32,
    /// Minimum value of the normalised output range (e.g. `-1.0` for sticks,
    /// `0.0` for throttles).
    pub output_min: f32,
    /// Maximum value of the normalised output range (e.g. `1.0`).
    pub output_max: f32,
    /// When `true` the polarity is inverted: `raw_min` maps to `output_max`
    /// and `raw_max` maps to `output_min`.
    pub reversed: bool,
}

impl AxisCalibration {
    /// Normalise a raw hardware axis value into `[output_min, output_max]`.
    ///
    /// Steps:
    ///
    /// 1. Clamp `raw` to `[raw_min, raw_max]`.
    /// 2. Map linearly: `raw_min → output_min`, `raw_max → output_max`.
    /// 3. If `reversed`, flip the output around the output midpoint.
    /// 4. Apply the centre deadzone: values within `deadzone` of the centre
    ///    output are pinned to the centre; outside values are rescaled to
    ///    preserve the full output range.
    /// 5. Final clamp to `[output_min, output_max]` to absorb f32 rounding.
    ///
    /// The returned value is always finite and within `[output_min, output_max]`.
    pub fn normalize(&self, raw: u32) -> f32 {
        let raw_min_f = self.raw_min as f32;
        let raw_max_f = self.raw_max as f32;
        let raw_range = raw_max_f - raw_min_f;

        // Degenerate range → return midpoint.
        if raw_range <= 0.0 {
            return (self.output_min + self.output_max) * 0.5;
        }

        // Step 1: clamp raw to hardware range.
        let clamped = (raw as f32).clamp(raw_min_f, raw_max_f);

        // Step 2: linear map [raw_min, raw_max] → [output_min, output_max].
        let unit = (clamped - raw_min_f) / raw_range;
        let out_range = self.output_max - self.output_min;
        let mut out = self.output_min + unit * out_range;

        // Step 3: polarity reversal.
        if self.reversed {
            out = self.output_min + self.output_max - out;
        }

        // Step 4: centre deadzone.
        //
        // Compute the output value that raw_center maps to (honouring `reversed`),
        // then apply a symmetric deadzone around that centre.
        let dz = self.deadzone.clamp(0.0, 0.9999);
        if dz > 0.0 {
            let raw_ctr = (self.raw_center as f32).clamp(raw_min_f, raw_max_f);
            let ctr_unit = (raw_ctr - raw_min_f) / raw_range;
            let mut center_out = self.output_min + ctr_unit * out_range;
            if self.reversed {
                center_out = self.output_min + self.output_max - center_out;
            }

            // Normalise `out` relative to `center_out` into [-1, 1] space.
            let half = out_range * 0.5;
            if half > 0.0 {
                let norm = (out - center_out) / half;
                if norm.abs() < dz {
                    // Inside deadzone → snap to centre.
                    out = center_out;
                } else {
                    // Outside deadzone → rescale so the edge still reaches ±1.
                    let sign = norm.signum();
                    let rescaled = sign * (norm.abs() - dz) / (1.0 - dz);
                    out = center_out + rescaled * half;
                }
            }
        }

        // Step 5: clamp to absorb f32 rounding.
        out.clamp(self.output_min, self.output_max)
    }
}
