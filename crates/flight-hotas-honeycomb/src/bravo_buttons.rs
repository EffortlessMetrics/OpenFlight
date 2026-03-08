// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Named button variants for the Honeycomb Bravo Throttle Quadrant.
//!
//! Each variant maps to a 1-indexed button number in the Bravo HID report.
//! Use [`BravoButton::is_active`] to test against a [`BravoButtons`](crate::bravo::BravoButtons) mask.

use crate::bravo::BravoButtons;

/// Named buttons for the Honeycomb Bravo Throttle Quadrant.
///
/// Button numbers are 1-indexed to match HID report conventions.
/// Bit index in the mask = button_number − 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BravoButton {
    /// HDG autopilot mode — button 1 (bit 0).
    ApHdg = 1,
    /// NAV autopilot mode — button 2 (bit 1).
    ApNav = 2,
    /// APR (approach) autopilot mode — button 3 (bit 2).
    ApApr = 3,
    /// REV (back course) autopilot mode — button 4 (bit 3).
    ApRev = 4,
    /// ALT autopilot mode — button 5 (bit 4).
    ApAlt = 5,
    /// VS autopilot mode — button 6 (bit 5).
    ApVs = 6,
    /// IAS autopilot mode — button 7 (bit 6).
    ApIas = 7,
    /// AP Master (CMD) — button 8 (bit 7).
    ApMaster = 8,
    /// Throttle 2 reverse handle — button 9 (bit 8).
    Throttle2Reverse = 9,
    /// Throttle 3 reverse handle — button 10 (bit 9).
    Throttle3Reverse = 10,
    /// Throttle 4 reverse handle — button 11 (bit 10).
    Throttle4Reverse = 11,
    /// Throttle 5 reverse handle — button 12 (bit 11).
    Throttle5Reverse = 12,
    /// Encoder increment (CW) — button 13 (bit 12).
    EncoderCw = 13,
    /// Encoder decrement (CCW) — button 14 (bit 13).
    EncoderCcw = 14,
    /// Flaps down — button 15 (bit 14).
    FlapsDown = 15,
    /// Flaps up — button 16 (bit 15).
    FlapsUp = 16,
    /// AP mode select: IAS — button 17 (bit 16).
    ApModeIas = 17,
    /// AP mode select: CRS — button 18 (bit 17).
    ApModeCrs = 18,
    /// AP mode select: HDG — button 19 (bit 18).
    ApModeHdg = 19,
    /// AP mode select: VS — button 20 (bit 19).
    ApModeVs = 20,
    /// AP mode select: ALT — button 21 (bit 20).
    ApModeAlt = 21,
    /// Trim down — button 22 (bit 21).
    TrimDown = 22,
    /// Trim up — button 23 (bit 22).
    TrimUp = 23,
    /// Throttle 1 reverse zone — button 24 (bit 23).
    Throttle1ReverseZone = 24,
    /// Throttle 2 reverse zone — button 25 (bit 24).
    Throttle2ReverseZone = 25,
    /// Throttle 3 reverse zone — button 26 (bit 25).
    Throttle3ReverseZone = 26,
    /// Throttle 4 reverse zone — button 27 (bit 26).
    Throttle4ReverseZone = 27,
    /// Throttle 5 reverse zone — button 28 (bit 27).
    Throttle5ReverseZone = 28,
    /// Throttle 1 reverse handle — button 29 (bit 28).
    Throttle1Reverse = 29,
    /// Throttle 3 2nd function — button 30 (bit 29).
    Throttle3SecondFn = 30,
    /// Gear UP — button 31 (bit 30).
    GearUp = 31,
    /// Gear DOWN — button 32 (bit 31).
    GearDown = 32,
    /// Throttle 6 reverse zone — button 33 (bit 32).
    Throttle6ReverseZone = 33,
    /// Toggle switch 1 UP — button 34 (bit 33).
    Toggle1Up = 34,
    /// Toggle switch 1 DOWN — button 35 (bit 34).
    Toggle1Down = 35,
    /// Toggle switch 2 UP — button 36 (bit 35).
    Toggle2Up = 36,
    /// Toggle switch 2 DOWN — button 37 (bit 36).
    Toggle2Down = 37,
    /// Toggle switch 3 UP — button 38 (bit 37).
    Toggle3Up = 38,
    /// Toggle switch 3 DOWN — button 39 (bit 38).
    Toggle3Down = 39,
    /// Toggle switch 4 UP — button 40 (bit 39).
    Toggle4Up = 40,
    /// Toggle switch 4 DOWN — button 41 (bit 40).
    Toggle4Down = 41,
    /// Toggle switch 5 UP — button 42 (bit 41).
    Toggle5Up = 42,
    /// Toggle switch 5 DOWN — button 43 (bit 42).
    Toggle5Down = 43,
    /// Toggle switch 6 UP — button 44 (bit 43).
    Toggle6Up = 44,
    /// Toggle switch 6 DOWN — button 45 (bit 44).
    Toggle6Down = 45,
    /// Toggle switch 7 UP — button 46 (bit 45).
    Toggle7Up = 46,
    /// Toggle switch 7 DOWN — button 47 (bit 46).
    Toggle7Down = 47,
    /// Throttle 4 2nd function — button 48 (bit 47).
    Throttle4SecondFn = 48,
}

impl BravoButton {
    /// Returns the 1-indexed button number.
    pub fn number(self) -> u8 {
        self as u8
    }

    /// Returns `true` if this button is active in the given button state.
    pub fn is_active(self, buttons: &BravoButtons) -> bool {
        buttons.is_pressed(self as u8)
    }

    /// Returns all named Bravo buttons.
    pub fn all() -> &'static [BravoButton] {
        &[
            BravoButton::ApHdg,
            BravoButton::ApNav,
            BravoButton::ApApr,
            BravoButton::ApRev,
            BravoButton::ApAlt,
            BravoButton::ApVs,
            BravoButton::ApIas,
            BravoButton::ApMaster,
            BravoButton::Throttle2Reverse,
            BravoButton::Throttle3Reverse,
            BravoButton::Throttle4Reverse,
            BravoButton::Throttle5Reverse,
            BravoButton::EncoderCw,
            BravoButton::EncoderCcw,
            BravoButton::FlapsDown,
            BravoButton::FlapsUp,
            BravoButton::ApModeIas,
            BravoButton::ApModeCrs,
            BravoButton::ApModeHdg,
            BravoButton::ApModeVs,
            BravoButton::ApModeAlt,
            BravoButton::TrimDown,
            BravoButton::TrimUp,
            BravoButton::Throttle1ReverseZone,
            BravoButton::Throttle2ReverseZone,
            BravoButton::Throttle3ReverseZone,
            BravoButton::Throttle4ReverseZone,
            BravoButton::Throttle5ReverseZone,
            BravoButton::Throttle1Reverse,
            BravoButton::Throttle3SecondFn,
            BravoButton::GearUp,
            BravoButton::GearDown,
            BravoButton::Throttle6ReverseZone,
            BravoButton::Toggle1Up,
            BravoButton::Toggle1Down,
            BravoButton::Toggle2Up,
            BravoButton::Toggle2Down,
            BravoButton::Toggle3Up,
            BravoButton::Toggle3Down,
            BravoButton::Toggle4Up,
            BravoButton::Toggle4Down,
            BravoButton::Toggle5Up,
            BravoButton::Toggle5Down,
            BravoButton::Toggle6Up,
            BravoButton::Toggle6Down,
            BravoButton::Toggle7Up,
            BravoButton::Toggle7Down,
            BravoButton::Throttle4SecondFn,
        ]
    }

    /// Human-readable label for this button.
    pub fn label(self) -> &'static str {
        match self {
            BravoButton::ApHdg => "AP HDG",
            BravoButton::ApNav => "AP NAV",
            BravoButton::ApApr => "AP APR",
            BravoButton::ApRev => "AP REV",
            BravoButton::ApAlt => "AP ALT",
            BravoButton::ApVs => "AP VS",
            BravoButton::ApIas => "AP IAS",
            BravoButton::ApMaster => "AP Master",
            BravoButton::Throttle2Reverse => "Throttle 2 Reverse",
            BravoButton::Throttle3Reverse => "Throttle 3 Reverse",
            BravoButton::Throttle4Reverse => "Throttle 4 Reverse",
            BravoButton::Throttle5Reverse => "Throttle 5 Reverse",
            BravoButton::EncoderCw => "Encoder CW",
            BravoButton::EncoderCcw => "Encoder CCW",
            BravoButton::FlapsDown => "Flaps Down",
            BravoButton::FlapsUp => "Flaps Up",
            BravoButton::ApModeIas => "AP Mode IAS",
            BravoButton::ApModeCrs => "AP Mode CRS",
            BravoButton::ApModeHdg => "AP Mode HDG",
            BravoButton::ApModeVs => "AP Mode VS",
            BravoButton::ApModeAlt => "AP Mode ALT",
            BravoButton::TrimDown => "Trim Down",
            BravoButton::TrimUp => "Trim Up",
            BravoButton::Throttle1ReverseZone => "Throttle 1 Reverse Zone",
            BravoButton::Throttle2ReverseZone => "Throttle 2 Reverse Zone",
            BravoButton::Throttle3ReverseZone => "Throttle 3 Reverse Zone",
            BravoButton::Throttle4ReverseZone => "Throttle 4 Reverse Zone",
            BravoButton::Throttle5ReverseZone => "Throttle 5 Reverse Zone",
            BravoButton::Throttle1Reverse => "Throttle 1 Reverse",
            BravoButton::Throttle3SecondFn => "Throttle 3 2nd Fn",
            BravoButton::GearUp => "Gear UP",
            BravoButton::GearDown => "Gear DOWN",
            BravoButton::Throttle6ReverseZone => "Throttle 6 Reverse Zone",
            BravoButton::Toggle1Up => "Toggle 1 UP",
            BravoButton::Toggle1Down => "Toggle 1 DOWN",
            BravoButton::Toggle2Up => "Toggle 2 UP",
            BravoButton::Toggle2Down => "Toggle 2 DOWN",
            BravoButton::Toggle3Up => "Toggle 3 UP",
            BravoButton::Toggle3Down => "Toggle 3 DOWN",
            BravoButton::Toggle4Up => "Toggle 4 UP",
            BravoButton::Toggle4Down => "Toggle 4 DOWN",
            BravoButton::Toggle5Up => "Toggle 5 UP",
            BravoButton::Toggle5Down => "Toggle 5 DOWN",
            BravoButton::Toggle6Up => "Toggle 6 UP",
            BravoButton::Toggle6Down => "Toggle 6 DOWN",
            BravoButton::Toggle7Up => "Toggle 7 UP",
            BravoButton::Toggle7Down => "Toggle 7 DOWN",
            BravoButton::Throttle4SecondFn => "Throttle 4 2nd Fn",
        }
    }

    /// Returns all autopilot mode buttons.
    pub fn ap_buttons() -> &'static [BravoButton] {
        &[
            BravoButton::ApHdg,
            BravoButton::ApNav,
            BravoButton::ApApr,
            BravoButton::ApRev,
            BravoButton::ApAlt,
            BravoButton::ApVs,
            BravoButton::ApIas,
            BravoButton::ApMaster,
        ]
    }

    /// Returns all toggle switch buttons (UP and DOWN pairs).
    pub fn toggle_buttons() -> &'static [BravoButton] {
        &[
            BravoButton::Toggle1Up,
            BravoButton::Toggle1Down,
            BravoButton::Toggle2Up,
            BravoButton::Toggle2Down,
            BravoButton::Toggle3Up,
            BravoButton::Toggle3Down,
            BravoButton::Toggle4Up,
            BravoButton::Toggle4Down,
            BravoButton::Toggle5Up,
            BravoButton::Toggle5Down,
            BravoButton::Toggle6Up,
            BravoButton::Toggle6Down,
            BravoButton::Toggle7Up,
            BravoButton::Toggle7Down,
        ]
    }
}

impl std::fmt::Display for BravoButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_buttons_have_unique_numbers() {
        let all = BravoButton::all();
        let mut seen = std::collections::HashSet::new();
        for b in all {
            assert!(
                seen.insert(b.number()),
                "duplicate button number: {}",
                b.number()
            );
        }
    }

    #[test]
    fn all_buttons_in_valid_range() {
        for b in BravoButton::all() {
            assert!(
                (1..=64).contains(&b.number()),
                "button {} out of range",
                b.number()
            );
        }
    }

    #[test]
    fn button_active_check() {
        let buttons = BravoButtons { mask: 1 << 7 }; // AP Master (bit 7 = button 8)
        assert!(BravoButton::ApMaster.is_active(&buttons));
        assert!(!BravoButton::ApHdg.is_active(&buttons));
    }

    #[test]
    fn ap_buttons_count() {
        assert_eq!(BravoButton::ap_buttons().len(), 8);
    }

    #[test]
    fn toggle_buttons_count() {
        assert_eq!(BravoButton::toggle_buttons().len(), 14);
    }

    #[test]
    fn all_buttons_have_nonempty_labels() {
        for b in BravoButton::all() {
            assert!(!b.label().is_empty());
        }
    }

    #[test]
    fn display_impl_matches_label() {
        for b in BravoButton::all() {
            assert_eq!(format!("{b}"), b.label());
        }
    }

    #[test]
    fn gear_buttons_correct_numbers() {
        assert_eq!(BravoButton::GearUp.number(), 31);
        assert_eq!(BravoButton::GearDown.number(), 32);
    }

    #[test]
    fn encoder_buttons_correct_numbers() {
        assert_eq!(BravoButton::EncoderCw.number(), 13);
        assert_eq!(BravoButton::EncoderCcw.number(), 14);
    }
}
