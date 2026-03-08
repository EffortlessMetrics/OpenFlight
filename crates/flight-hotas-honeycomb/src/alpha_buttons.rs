// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Named button variants for the Honeycomb Alpha Flight Controls XPC (Yoke).
//!
//! Each variant maps to a 1-indexed button number in the Alpha HID report.
//! Use [`AlphaButton::is_active`] to test against an [`AlphaButtons`](crate::alpha::AlphaButtons) mask.

use crate::alpha::AlphaButtons;

/// Named buttons for the Honeycomb Alpha Yoke.
///
/// Button numbers are 1-indexed to match HID report conventions.
/// Not all 36 buttons are named here — only the ones with well-known
/// assignments from community documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AlphaButton {
    /// Push-to-talk (PTT) — button 1.
    Ptt = 1,
    /// Autopilot disconnect — button 2.
    ApDisconnect = 2,
    /// Left-side trigger — button 3.
    TriggerLeft = 3,
    /// Right-side trigger — button 4.
    TriggerRight = 4,
    /// Left rocker up — button 5.
    LeftRockerUp = 5,
    /// Left rocker down — button 6.
    LeftRockerDown = 6,
    /// Right rocker up — button 7.
    RightRockerUp = 7,
    /// Right rocker down — button 8.
    RightRockerDown = 8,
    /// Flap switch position 1 (up) — button 9.
    FlapUp = 9,
    /// Flap switch position 2 — button 10.
    FlapPos2 = 10,
    /// Flap switch position 3 — button 11.
    FlapPos3 = 11,
    /// Flap switch position 4 (down) — button 12.
    FlapDown = 12,
    /// Trim wheel up — button 13.
    TrimUp = 13,
    /// Trim wheel down — button 14.
    TrimDown = 14,
    /// Left toggle switch up — button 15.
    LeftToggleUp = 15,
    /// Left toggle switch down — button 16.
    LeftToggleDown = 16,
    /// Right toggle switch up — button 17.
    RightToggleUp = 17,
    /// Right toggle switch down — button 18.
    RightToggleDown = 18,
    /// Master battery — button 19.
    MasterBattery = 19,
    /// Master alternator — button 20.
    MasterAlternator = 20,
    /// Avionics bus 1 — button 21.
    AvionicsBus1 = 21,
    /// Avionics bus 2 — button 22.
    AvionicsBus2 = 22,
    /// Beacon light — button 23.
    BeaconLight = 23,
    /// Landing light — button 24.
    LandingLight = 24,
    /// Magneto A (right) — button 25.
    MagnetoRight = 25,
    /// Magneto B (left) — button 26.
    MagnetoLeft = 26,
    /// Starter (spring-return momentary) — button 27.
    Starter = 27,
    /// Navigation light — button 28.
    NavLight = 28,
    /// Strobe light — button 29.
    StrobeLight = 29,
    /// Taxi light — button 30.
    TaxiLight = 30,
}

impl AlphaButton {
    /// Returns the 1-indexed button number.
    pub fn number(self) -> u8 {
        self as u8
    }

    /// Returns `true` if this button is active in the given button state.
    pub fn is_active(self, buttons: &AlphaButtons) -> bool {
        buttons.is_pressed(self as u8)
    }

    /// Returns all named Alpha buttons.
    pub fn all() -> &'static [AlphaButton] {
        &[
            AlphaButton::Ptt,
            AlphaButton::ApDisconnect,
            AlphaButton::TriggerLeft,
            AlphaButton::TriggerRight,
            AlphaButton::LeftRockerUp,
            AlphaButton::LeftRockerDown,
            AlphaButton::RightRockerUp,
            AlphaButton::RightRockerDown,
            AlphaButton::FlapUp,
            AlphaButton::FlapPos2,
            AlphaButton::FlapPos3,
            AlphaButton::FlapDown,
            AlphaButton::TrimUp,
            AlphaButton::TrimDown,
            AlphaButton::LeftToggleUp,
            AlphaButton::LeftToggleDown,
            AlphaButton::RightToggleUp,
            AlphaButton::RightToggleDown,
            AlphaButton::MasterBattery,
            AlphaButton::MasterAlternator,
            AlphaButton::AvionicsBus1,
            AlphaButton::AvionicsBus2,
            AlphaButton::BeaconLight,
            AlphaButton::LandingLight,
            AlphaButton::MagnetoRight,
            AlphaButton::MagnetoLeft,
            AlphaButton::Starter,
            AlphaButton::NavLight,
            AlphaButton::StrobeLight,
            AlphaButton::TaxiLight,
        ]
    }

    /// Human-readable label for this button.
    pub fn label(self) -> &'static str {
        match self {
            AlphaButton::Ptt => "PTT",
            AlphaButton::ApDisconnect => "AP Disconnect",
            AlphaButton::TriggerLeft => "Trigger Left",
            AlphaButton::TriggerRight => "Trigger Right",
            AlphaButton::LeftRockerUp => "Left Rocker Up",
            AlphaButton::LeftRockerDown => "Left Rocker Down",
            AlphaButton::RightRockerUp => "Right Rocker Up",
            AlphaButton::RightRockerDown => "Right Rocker Down",
            AlphaButton::FlapUp => "Flap Up",
            AlphaButton::FlapPos2 => "Flap Position 2",
            AlphaButton::FlapPos3 => "Flap Position 3",
            AlphaButton::FlapDown => "Flap Down",
            AlphaButton::TrimUp => "Trim Up",
            AlphaButton::TrimDown => "Trim Down",
            AlphaButton::LeftToggleUp => "Left Toggle Up",
            AlphaButton::LeftToggleDown => "Left Toggle Down",
            AlphaButton::RightToggleUp => "Right Toggle Up",
            AlphaButton::RightToggleDown => "Right Toggle Down",
            AlphaButton::MasterBattery => "Master Battery",
            AlphaButton::MasterAlternator => "Master Alternator",
            AlphaButton::AvionicsBus1 => "Avionics Bus 1",
            AlphaButton::AvionicsBus2 => "Avionics Bus 2",
            AlphaButton::BeaconLight => "Beacon Light",
            AlphaButton::LandingLight => "Landing Light",
            AlphaButton::MagnetoRight => "Magneto Right",
            AlphaButton::MagnetoLeft => "Magneto Left",
            AlphaButton::Starter => "Starter",
            AlphaButton::NavLight => "Nav Light",
            AlphaButton::StrobeLight => "Strobe Light",
            AlphaButton::TaxiLight => "Taxi Light",
        }
    }
}

impl std::fmt::Display for AlphaButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_buttons_have_unique_numbers() {
        let all = AlphaButton::all();
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
        for b in AlphaButton::all() {
            assert!(
                (1..=36).contains(&b.number()),
                "button {} out of range",
                b.number()
            );
        }
    }

    #[test]
    fn button_active_check() {
        let buttons = AlphaButtons {
            mask: 1 << 0,
            hat: 0,
        }; // button 1 pressed
        assert!(AlphaButton::Ptt.is_active(&buttons));
        assert!(!AlphaButton::ApDisconnect.is_active(&buttons));
    }

    #[test]
    fn all_buttons_have_nonempty_labels() {
        for b in AlphaButton::all() {
            assert!(!b.label().is_empty());
        }
    }

    #[test]
    fn display_impl_matches_label() {
        for b in AlphaButton::all() {
            assert_eq!(format!("{b}"), b.label());
        }
    }

    #[test]
    fn magneto_buttons_correct_numbers() {
        assert_eq!(AlphaButton::MagnetoRight.number(), 25);
        assert_eq!(AlphaButton::MagnetoLeft.number(), 26);
        assert_eq!(AlphaButton::Starter.number(), 27);
    }
}
