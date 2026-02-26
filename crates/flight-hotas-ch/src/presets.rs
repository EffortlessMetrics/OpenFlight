// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Recommended axis presets for CH Products devices.

use flight_hid_support::device_support::ChModel;

/// Recommended axis configuration for a CH Products device.
#[derive(Debug, Clone, PartialEq)]
pub struct ChAxisPreset {
    /// The device model this preset applies to.
    pub device: ChModel,
    /// Recommended deadzone radius (0.0–1.0, normalized axis units).
    pub deadzone: f32,
    /// Recommended exponential curve factor (0.0 = linear, higher = more curved).
    pub expo: f32,
    /// Whether the throttle axis should be inverted by default.
    pub invert_throttle: bool,
}

/// Returns the recommended axis preset for a given CH Products device model.
pub fn recommended_preset(model: ChModel) -> ChAxisPreset {
    match model {
        ChModel::Fighterstick | ChModel::CombatStick => ChAxisPreset {
            device: model,
            deadzone: 0.02,
            expo: 0.15,
            invert_throttle: false,
        },
        ChModel::ProThrottle => ChAxisPreset {
            device: model,
            deadzone: 0.01,
            expo: 0.0,
            invert_throttle: false,
        },
        ChModel::ProPedals => ChAxisPreset {
            device: model,
            deadzone: 0.03,
            expo: 0.1,
            invert_throttle: false,
        },
        ChModel::EclipseYoke | ChModel::FlightYoke => ChAxisPreset {
            device: model,
            deadzone: 0.02,
            expo: 0.1,
            invert_throttle: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn all_models() -> [ChModel; 6] {
        [
            ChModel::ProThrottle,
            ChModel::ProPedals,
            ChModel::Fighterstick,
            ChModel::CombatStick,
            ChModel::EclipseYoke,
            ChModel::FlightYoke,
        ]
    }

    #[test]
    fn all_models_have_preset() {
        for model in all_models() {
            let preset = recommended_preset(model);
            assert_eq!(preset.device, model);
        }
    }

    #[test]
    fn deadzone_in_range() {
        for model in all_models() {
            let preset = recommended_preset(model);
            assert!(
                (0.0..=0.1).contains(&preset.deadzone),
                "{model:?} deadzone {:.4} out of [0.0, 0.1]",
                preset.deadzone
            );
        }
    }

    #[test]
    fn expo_in_range() {
        for model in all_models() {
            let preset = recommended_preset(model);
            assert!(
                (0.0..=0.5).contains(&preset.expo),
                "{model:?} expo {:.4} out of [0.0, 0.5]",
                preset.expo
            );
        }
    }

    #[test]
    fn yokes_invert_throttle() {
        for model in [ChModel::EclipseYoke, ChModel::FlightYoke] {
            assert!(
                recommended_preset(model).invert_throttle,
                "{model:?} should invert throttle"
            );
        }
    }

    #[test]
    fn sticks_do_not_invert_throttle() {
        for model in [
            ChModel::Fighterstick,
            ChModel::CombatStick,
            ChModel::ProThrottle,
            ChModel::ProPedals,
        ] {
            assert!(
                !recommended_preset(model).invert_throttle,
                "{model:?} should not invert throttle"
            );
        }
    }

    proptest! {
        /// Deadzone is always in [0.0, 0.1] regardless of which model variant is
        /// selected by the fuzzer (we drive selection via an index).
        #[test]
        fn prop_deadzone_valid(idx in 0usize..6) {
            let preset = recommended_preset(all_models()[idx]);
            prop_assert!((0.0f32..=0.1).contains(&preset.deadzone));
        }

        #[test]
        fn prop_expo_valid(idx in 0usize..6) {
            let preset = recommended_preset(all_models()[idx]);
            prop_assert!((0.0f32..=0.5).contains(&preset.expo));
        }
    }
}
