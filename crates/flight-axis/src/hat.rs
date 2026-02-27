// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Hat switch decoder — converts discrete direction values to normalized (x, y) axes.
//!
//! Hat switches report discrete direction values (0–8 or 0–15 depending on the device).
//! This module decodes them into a normalized `(x, y)` pair where:
//! - `x`: `-1.0` = left (W), `0.0` = center, `+1.0` = right (E)
//! - `y`: `-1.0` = up (N), `0.0` = center, `+1.0` = down (S)
//!
//! # Supported encodings
//!
//! | Variant     | Encoding                                           |
//! |-------------|----------------------------------------------------|
//! | `FourWay`   | N=0, E=1, S=2, W=3, neutral=0xFF                  |
//! | `EightWay`  | N=0, NE=1, E=2, SE=3, S=4, SW=5, W=6, NW=7, neutral=8 |
//! | `Pov16`     | POV angle in hundredths of degrees (0–35999), neutral=0xFFFF |
//!
//! Diagonal vectors (`EightWay` and `Pov16`) are normalized to unit length using
//! `1/√2 ≈ 0.70711`.

use thiserror::Error;

/// Precomputed `1 / √2` for normalizing diagonal hat-switch vectors.
const SQRT2_RECIP: f32 = std::f32::consts::FRAC_1_SQRT_2;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Hat switch resolution — selects the decoding table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HatResolution {
    /// 4-way hat: N=0, E=1, S=2, W=3, neutral=0xFF.
    FourWay,
    /// 8-way hat: N=0, NE=1, E=2, SE=3, S=4, SW=5, W=6, NW=7, neutral=8.
    EightWay,
    /// 16-way POV reported in hundredths of degrees (e.g. 9000 = 90.00°), neutral=0xFFFF.
    Pov16,
}

/// Output of a single hat-switch decode operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HatOutput {
    /// Horizontal axis: `-1.0` = left (W), `0.0` = center, `+1.0` = right (E).
    pub x: f32,
    /// Vertical axis: `-1.0` = up (N), `0.0` = center, `+1.0` = down (S).
    pub y: f32,
    /// `true` when any direction is active (i.e. not neutral).
    pub pressed: bool,
}

impl HatOutput {
    /// Neutral (center) position — no direction pressed.
    #[inline]
    pub fn neutral() -> Self {
        HatOutput {
            x: 0.0,
            y: 0.0,
            pressed: false,
        }
    }
}

/// Error type for hat switch decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HatError {
    /// The raw value is outside the valid range for the chosen resolution.
    #[error("invalid hat value {raw:#06x} for resolution {resolution:?}")]
    InvalidValue { raw: u16, resolution: HatResolution },
}

// ---------------------------------------------------------------------------
// HatDecoder
// ---------------------------------------------------------------------------

/// Decodes a raw hat switch value into a normalized `(x, y)` [`HatOutput`].
pub struct HatDecoder {
    /// Resolution/encoding used by this decoder.
    pub resolution: HatResolution,
}

impl HatDecoder {
    /// Creates a new `HatDecoder` for the given [`HatResolution`].
    pub fn new(resolution: HatResolution) -> Self {
        Self { resolution }
    }

    /// Decodes a raw hat value to `(x, y)` output.
    ///
    /// Returns [`HatError::InvalidValue`] when `raw` is outside the valid range for
    /// the configured [`HatResolution`].
    pub fn decode(&self, raw: u16) -> Result<HatOutput, HatError> {
        match self.resolution {
            HatResolution::EightWay => self.decode_eight_way(raw),
            HatResolution::FourWay => self.decode_four_way(raw),
            HatResolution::Pov16 => self.decode_pov16(raw),
        }
    }

    /// Decodes an 8-way hat value.  Diagonals are scaled to unit length (`1/√2`).
    fn decode_eight_way(&self, raw: u16) -> Result<HatOutput, HatError> {
        // neutral=8; values 0–7 are valid directions.
        let out = match raw {
            0 => HatOutput {
                x: 0.0,
                y: -1.0,
                pressed: true,
            }, // N
            1 => HatOutput {
                x: SQRT2_RECIP,
                y: -SQRT2_RECIP,
                pressed: true,
            }, // NE
            2 => HatOutput {
                x: 1.0,
                y: 0.0,
                pressed: true,
            }, // E
            3 => HatOutput {
                x: SQRT2_RECIP,
                y: SQRT2_RECIP,
                pressed: true,
            }, // SE
            4 => HatOutput {
                x: 0.0,
                y: 1.0,
                pressed: true,
            }, // S
            5 => HatOutput {
                x: -SQRT2_RECIP,
                y: SQRT2_RECIP,
                pressed: true,
            }, // SW
            6 => HatOutput {
                x: -1.0,
                y: 0.0,
                pressed: true,
            }, // W
            7 => HatOutput {
                x: -SQRT2_RECIP,
                y: -SQRT2_RECIP,
                pressed: true,
            }, // NW
            8 => HatOutput::neutral(),
            _ => {
                return Err(HatError::InvalidValue {
                    raw,
                    resolution: HatResolution::EightWay,
                });
            }
        };
        Ok(out)
    }

    /// Decodes a 4-way hat value.
    fn decode_four_way(&self, raw: u16) -> Result<HatOutput, HatError> {
        // neutral=0xFF; values 0–3 are valid directions.
        let out = match raw {
            0x00 => HatOutput {
                x: 0.0,
                y: -1.0,
                pressed: true,
            }, // N
            0x01 => HatOutput {
                x: 1.0,
                y: 0.0,
                pressed: true,
            }, // E
            0x02 => HatOutput {
                x: 0.0,
                y: 1.0,
                pressed: true,
            }, // S
            0x03 => HatOutput {
                x: -1.0,
                y: 0.0,
                pressed: true,
            }, // W
            0xFF => HatOutput::neutral(),
            _ => {
                return Err(HatError::InvalidValue {
                    raw,
                    resolution: HatResolution::FourWay,
                });
            }
        };
        Ok(out)
    }

    /// Decodes a 16-way POV value (hundredths of degrees, 0–35999), neutral=0xFFFF.
    ///
    /// Uses `x = sin(θ)`, `y = -cos(θ)` so that 0° → N (x=0, y=-1) and 90° → E (x=1, y=0).
    fn decode_pov16(&self, raw: u16) -> Result<HatOutput, HatError> {
        if raw == 0xFFFF {
            return Ok(HatOutput::neutral());
        }
        if raw > 35999 {
            return Err(HatError::InvalidValue {
                raw,
                resolution: HatResolution::Pov16,
            });
        }
        let degrees = raw as f32 / 100.0;
        let radians = degrees * std::f32::consts::PI / 180.0;
        Ok(HatOutput {
            x: radians.sin(),
            y: -radians.cos(),
            pressed: true,
        })
    }
}

// ---------------------------------------------------------------------------
// HatBank
// ---------------------------------------------------------------------------

/// Manages a bank of hat switches, each with its own [`HatDecoder`] and cached output.
pub struct HatBank {
    decoders: Vec<HatDecoder>,
    outputs: Vec<HatOutput>,
}

impl HatBank {
    /// Creates a new `HatBank` from a list of [`HatResolution`]s, one per hat switch.
    pub fn new(resolutions: Vec<HatResolution>) -> Self {
        let outputs = vec![HatOutput::neutral(); resolutions.len()];
        let decoders = resolutions.into_iter().map(HatDecoder::new).collect();
        Self { decoders, outputs }
    }

    /// Updates the hat at `hat_index` with a new raw value, caches the result, and
    /// returns it.
    ///
    /// Returns [`HatError::InvalidValue`] if `raw` is out of range, or
    /// `None`-equivalent panic if `hat_index >= len()`.
    pub fn update(&mut self, hat_index: usize, raw: u16) -> Result<HatOutput, HatError> {
        let output = self.decoders[hat_index].decode(raw)?;
        self.outputs[hat_index] = output;
        Ok(output)
    }

    /// Returns the last cached output for `hat_index`, or `None` if out of range.
    pub fn output(&self, hat_index: usize) -> Option<HatOutput> {
        self.outputs.get(hat_index).copied()
    }

    /// Returns the number of hat switches in the bank.
    pub fn len(&self) -> usize {
        self.decoders.len()
    }

    /// Returns `true` if the bank contains no hat switches.
    pub fn is_empty(&self) -> bool {
        self.decoders.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn eight_way() -> HatDecoder {
        HatDecoder::new(HatResolution::EightWay)
    }
    fn four_way() -> HatDecoder {
        HatDecoder::new(HatResolution::FourWay)
    }
    fn pov16() -> HatDecoder {
        HatDecoder::new(HatResolution::Pov16)
    }

    const EPS: f32 = 0.001;

    // ---- EightWay cardinal ------------------------------------------------

    #[test]
    fn test_eight_way_north() {
        let out = eight_way().decode(0).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: 0.0,
                y: -1.0,
                pressed: true
            }
        );
    }

    #[test]
    fn test_eight_way_east() {
        let out = eight_way().decode(2).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: 1.0,
                y: 0.0,
                pressed: true
            }
        );
    }

    #[test]
    fn test_eight_way_south() {
        let out = eight_way().decode(4).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: 0.0,
                y: 1.0,
                pressed: true
            }
        );
    }

    #[test]
    fn test_eight_way_west() {
        let out = eight_way().decode(6).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: -1.0,
                y: 0.0,
                pressed: true
            }
        );
    }

    // ---- EightWay diagonals -----------------------------------------------

    #[test]
    fn test_eight_way_northeast() {
        let out = eight_way().decode(1).unwrap();
        assert!((out.x - SQRT2_RECIP).abs() < EPS, "x={}", out.x);
        assert!((out.y - (-SQRT2_RECIP)).abs() < EPS, "y={}", out.y);
        assert!(out.pressed);
    }

    #[test]
    fn test_eight_way_southeast() {
        let out = eight_way().decode(3).unwrap();
        assert!((out.x - SQRT2_RECIP).abs() < EPS);
        assert!((out.y - SQRT2_RECIP).abs() < EPS);
        assert!(out.pressed);
    }

    #[test]
    fn test_eight_way_southwest() {
        let out = eight_way().decode(5).unwrap();
        assert!((out.x - (-SQRT2_RECIP)).abs() < EPS);
        assert!((out.y - SQRT2_RECIP).abs() < EPS);
        assert!(out.pressed);
    }

    #[test]
    fn test_eight_way_northwest() {
        let out = eight_way().decode(7).unwrap();
        assert!((out.x - (-SQRT2_RECIP)).abs() < EPS);
        assert!((out.y - (-SQRT2_RECIP)).abs() < EPS);
        assert!(out.pressed);
    }

    // ---- EightWay neutral / invalid --------------------------------------

    #[test]
    fn test_eight_way_neutral() {
        assert_eq!(eight_way().decode(8).unwrap(), HatOutput::neutral());
    }

    #[test]
    fn test_eight_way_invalid() {
        assert!(matches!(
            eight_way().decode(9),
            Err(HatError::InvalidValue {
                raw: 9,
                resolution: HatResolution::EightWay
            })
        ));
    }

    // ---- FourWay ----------------------------------------------------------

    #[test]
    fn test_four_way_north() {
        let out = four_way().decode(0).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: 0.0,
                y: -1.0,
                pressed: true
            }
        );
    }

    #[test]
    fn test_four_way_east() {
        let out = four_way().decode(1).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: 1.0,
                y: 0.0,
                pressed: true
            }
        );
    }

    #[test]
    fn test_four_way_south() {
        let out = four_way().decode(2).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: 0.0,
                y: 1.0,
                pressed: true
            }
        );
    }

    #[test]
    fn test_four_way_west() {
        let out = four_way().decode(3).unwrap();
        assert_eq!(
            out,
            HatOutput {
                x: -1.0,
                y: 0.0,
                pressed: true
            }
        );
    }

    #[test]
    fn test_four_way_neutral() {
        assert_eq!(four_way().decode(0xFF).unwrap(), HatOutput::neutral());
    }

    #[test]
    fn test_four_way_invalid() {
        assert!(matches!(
            four_way().decode(4),
            Err(HatError::InvalidValue {
                raw: 4,
                resolution: HatResolution::FourWay
            })
        ));
    }

    // ---- Pov16 ------------------------------------------------------------

    #[test]
    fn test_pov16_north() {
        let out = pov16().decode(0).unwrap();
        assert!((out.x).abs() < EPS, "x={}", out.x);
        assert!((out.y - (-1.0)).abs() < EPS, "y={}", out.y);
        assert!(out.pressed);
    }

    #[test]
    fn test_pov16_east() {
        // 90.00° → raw=9000
        let out = pov16().decode(9000).unwrap();
        assert!((out.x - 1.0).abs() < EPS, "x={}", out.x);
        assert!((out.y).abs() < EPS, "y={}", out.y);
        assert!(out.pressed);
    }

    #[test]
    fn test_pov16_south() {
        // 180.00° → raw=18000
        let out = pov16().decode(18000).unwrap();
        assert!((out.x).abs() < EPS, "x={}", out.x);
        assert!((out.y - 1.0).abs() < EPS, "y={}", out.y);
        assert!(out.pressed);
    }

    #[test]
    fn test_pov16_west() {
        // 270.00° → raw=27000
        let out = pov16().decode(27000).unwrap();
        assert!((out.x - (-1.0)).abs() < EPS, "x={}", out.x);
        assert!((out.y).abs() < EPS, "y={}", out.y);
        assert!(out.pressed);
    }

    #[test]
    fn test_pov16_neutral() {
        assert_eq!(pov16().decode(0xFFFF).unwrap(), HatOutput::neutral());
    }

    #[test]
    fn test_pov16_invalid() {
        // 36000 (360.00°) is out of range
        assert!(matches!(
            pov16().decode(36000),
            Err(HatError::InvalidValue {
                raw: 36000,
                resolution: HatResolution::Pov16
            })
        ));
    }

    // ---- HatBank ----------------------------------------------------------

    #[test]
    fn test_hat_bank_multiple() {
        let mut bank = HatBank::new(vec![HatResolution::EightWay, HatResolution::FourWay]);
        assert_eq!(bank.len(), 2);

        let out0 = bank.update(0, 0).unwrap(); // EightWay North
        let out1 = bank.update(1, 1).unwrap(); // FourWay East

        assert_eq!(
            out0,
            HatOutput {
                x: 0.0,
                y: -1.0,
                pressed: true
            }
        );
        assert_eq!(
            out1,
            HatOutput {
                x: 1.0,
                y: 0.0,
                pressed: true
            }
        );

        // Outputs are independent
        assert_eq!(bank.output(0).unwrap(), out0);
        assert_eq!(bank.output(1).unwrap(), out1);
    }

    #[test]
    fn test_hat_bank_empty() {
        let bank = HatBank::new(vec![]);
        assert!(bank.is_empty());
        assert_eq!(bank.len(), 0);
        assert_eq!(bank.output(0), None);
    }

    #[test]
    fn test_hat_bank_cached_output_after_update() {
        let mut bank = HatBank::new(vec![HatResolution::EightWay]);
        assert_eq!(bank.output(0).unwrap(), HatOutput::neutral());
        bank.update(0, 4).unwrap(); // South
        assert_eq!(
            bank.output(0).unwrap(),
            HatOutput {
                x: 0.0,
                y: 1.0,
                pressed: true
            }
        );
    }

    // ---- Proptests --------------------------------------------------------

    proptest! {
        /// All valid 8-way hat values produce x and y in [-1.0, 1.0].
        #[test]
        fn prop_eight_way_axes_in_range(raw in 0u16..=8u16) {
            let out = eight_way().decode(raw).unwrap();
            prop_assert!(out.x >= -1.0 && out.x <= 1.0, "x={} out of range", out.x);
            prop_assert!(out.y >= -1.0 && out.y <= 1.0, "y={} out of range", out.y);
        }

        /// Output vector magnitude is always ≤ √2 for any valid 8-way value.
        #[test]
        fn prop_eight_way_magnitude_bounded(raw in 0u16..=8u16) {
            let out = eight_way().decode(raw).unwrap();
            let mag = (out.x * out.x + out.y * out.y).sqrt();
            prop_assert!(
                mag <= 2f32.sqrt() + 1e-5,
                "magnitude {} > sqrt(2) for raw={}",
                mag, raw
            );
        }
    }
}
