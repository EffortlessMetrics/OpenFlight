// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Rotary encoder state tracking for VIRPIL VPC devices.
//!
//! VIRPIL devices with rotary encoders (CM3 Throttle, Rotor TCS Plus, Control
//! Panel 2) report encoder turns as momentary button presses: one button for
//! clockwise (CW) and one for counter-clockwise (CCW). The firmware pulses the
//! button for one report cycle per detent.
//!
//! This module provides [`EncoderBank`] to track multiple encoders and convert
//! CW/CCW button pulses into signed delta values.
//!
//! # Example
//!
//! ```
//! use flight_hotas_virpil::encoder::{EncoderBank, EncoderPair};
//!
//! let pairs = [
//!     EncoderPair { cw_button: 65, ccw_button: 66 },
//!     EncoderPair { cw_button: 67, ccw_button: 68 },
//! ];
//! let bank = EncoderBank::new(&pairs);
//! // CW button 65 pressed → encoder 0 returns +1
//! assert_eq!(bank.delta(0, 65, true, 66, false), 1);
//! // CCW button 66 pressed → encoder 0 returns -1
//! assert_eq!(bank.delta(0, 65, false, 66, true), -1);
//! // Neither pressed → 0
//! assert_eq!(bank.delta(0, 65, false, 66, false), 0);
//! ```

/// A CW/CCW button pair that represents one rotary encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderPair {
    /// 1-indexed button number for clockwise rotation.
    pub cw_button: u8,
    /// 1-indexed button number for counter-clockwise rotation.
    pub ccw_button: u8,
}

/// A bank of rotary encoders decoded from button presses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderBank {
    pairs: Vec<EncoderPair>,
}

impl EncoderBank {
    /// Create a new encoder bank from a slice of CW/CCW button pairs.
    pub fn new(pairs: &[EncoderPair]) -> Self {
        Self {
            pairs: pairs.to_vec(),
        }
    }

    /// Number of encoders in this bank.
    pub fn count(&self) -> usize {
        self.pairs.len()
    }

    /// Get the encoder pair at the given index.
    pub fn pair(&self, index: usize) -> Option<&EncoderPair> {
        self.pairs.get(index)
    }

    /// Compute the delta for encoder at `index` given the current button states.
    ///
    /// Returns `+1` if CW is pressed, `-1` if CCW is pressed, `0` if neither
    /// or both are pressed.
    pub fn delta(
        &self,
        index: usize,
        cw_button_id: u8,
        cw_pressed: bool,
        ccw_button_id: u8,
        ccw_pressed: bool,
    ) -> i8 {
        if let Some(pair) = self.pairs.get(index) {
            if cw_button_id == pair.cw_button
                && ccw_button_id == pair.ccw_button
                && cw_pressed
                && !ccw_pressed
            {
                return 1;
            }
            if cw_button_id == pair.cw_button
                && ccw_button_id == pair.ccw_button
                && !cw_pressed
                && ccw_pressed
            {
                return -1;
            }
        }
        0
    }

    /// Extract deltas for all encoders using a button-query function.
    ///
    /// The `is_pressed` closure takes a 1-indexed button number and returns
    /// whether it is currently pressed.
    pub fn deltas(&self, is_pressed: impl Fn(u8) -> bool) -> Vec<i8> {
        self.pairs
            .iter()
            .map(|pair| {
                let cw = is_pressed(pair.cw_button);
                let ccw = is_pressed(pair.ccw_button);
                match (cw, ccw) {
                    (true, false) => 1,
                    (false, true) => -1,
                    _ => 0,
                }
            })
            .collect()
    }
}

/// Pre-defined encoder pairs for the VPC Throttle CM3.
///
/// The CM3 Throttle has 4 rotary encoders mapped to button pairs 65–72:
/// - Encoder 0: buttons 65 (CW) / 66 (CCW)
/// - Encoder 1: buttons 67 (CW) / 68 (CCW)
/// - Encoder 2: buttons 69 (CW) / 70 (CCW)
/// - Encoder 3: buttons 71 (CW) / 72 (CCW)
pub const CM3_ENCODER_PAIRS: &[EncoderPair] = &[
    EncoderPair {
        cw_button: 65,
        ccw_button: 66,
    },
    EncoderPair {
        cw_button: 67,
        ccw_button: 68,
    },
    EncoderPair {
        cw_button: 69,
        ccw_button: 70,
    },
    EncoderPair {
        cw_button: 71,
        ccw_button: 72,
    },
];

/// Pre-defined encoder pair for the VPC Rotor TCS Plus.
///
/// The Rotor TCS has 1 rotary encoder mapped to button pairs 21/22.
pub const ROTOR_TCS_ENCODER_PAIRS: &[EncoderPair] = &[EncoderPair {
    cw_button: 21,
    ccw_button: 22,
}];

/// Create an [`EncoderBank`] pre-configured for the VPC Throttle CM3.
pub fn cm3_encoder_bank() -> EncoderBank {
    EncoderBank::new(CM3_ENCODER_PAIRS)
}

/// Create an [`EncoderBank`] pre-configured for the VPC Rotor TCS Plus.
pub fn rotor_tcs_encoder_bank() -> EncoderBank {
    EncoderBank::new(ROTOR_TCS_ENCODER_PAIRS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cm3_bank_has_four_encoders() {
        let bank = cm3_encoder_bank();
        assert_eq!(bank.count(), 4);
    }

    #[test]
    fn rotor_tcs_bank_has_one_encoder() {
        let bank = rotor_tcs_encoder_bank();
        assert_eq!(bank.count(), 1);
    }

    #[test]
    fn delta_cw_returns_positive() {
        let bank = cm3_encoder_bank();
        assert_eq!(bank.delta(0, 65, true, 66, false), 1);
    }

    #[test]
    fn delta_ccw_returns_negative() {
        let bank = cm3_encoder_bank();
        assert_eq!(bank.delta(0, 65, false, 66, true), -1);
    }

    #[test]
    fn delta_both_pressed_returns_zero() {
        let bank = cm3_encoder_bank();
        assert_eq!(bank.delta(0, 65, true, 66, true), 0);
    }

    #[test]
    fn delta_neither_pressed_returns_zero() {
        let bank = cm3_encoder_bank();
        assert_eq!(bank.delta(0, 65, false, 66, false), 0);
    }

    #[test]
    fn delta_wrong_index_returns_zero() {
        let bank = cm3_encoder_bank();
        assert_eq!(bank.delta(99, 65, true, 66, false), 0);
    }

    #[test]
    fn delta_wrong_buttons_returns_zero() {
        let bank = cm3_encoder_bank();
        // Buttons don't match pair at index 0
        assert_eq!(bank.delta(0, 67, true, 68, false), 0);
    }

    #[test]
    fn deltas_all_idle() {
        let bank = cm3_encoder_bank();
        let d = bank.deltas(|_| false);
        assert_eq!(d, vec![0, 0, 0, 0]);
    }

    #[test]
    fn deltas_encoder_0_cw() {
        let bank = cm3_encoder_bank();
        let d = bank.deltas(|n| n == 65);
        assert_eq!(d, vec![1, 0, 0, 0]);
    }

    #[test]
    fn deltas_encoder_2_ccw() {
        let bank = cm3_encoder_bank();
        let d = bank.deltas(|n| n == 70);
        assert_eq!(d, vec![0, 0, -1, 0]);
    }

    #[test]
    fn deltas_multiple_encoders_active() {
        let bank = cm3_encoder_bank();
        let d = bank.deltas(|n| matches!(n, 65 | 68));
        assert_eq!(d, vec![1, -1, 0, 0]);
    }

    #[test]
    fn pair_accessor() {
        let bank = cm3_encoder_bank();
        let p = bank.pair(2).unwrap();
        assert_eq!(p.cw_button, 69);
        assert_eq!(p.ccw_button, 70);
    }

    #[test]
    fn pair_out_of_range_is_none() {
        let bank = cm3_encoder_bank();
        assert!(bank.pair(10).is_none());
    }

    #[test]
    fn empty_bank() {
        let bank = EncoderBank::new(&[]);
        assert_eq!(bank.count(), 0);
        assert_eq!(bank.deltas(|_| true), Vec::<i8>::new());
    }
}
