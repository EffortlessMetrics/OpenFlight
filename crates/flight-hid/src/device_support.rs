// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device support registry and quirk detection for common HID devices.

use crate::hid_descriptor::{HidUsage, extract_usages};
use crate::HidDeviceInfo;
use std::fmt;

pub const THRUSTMASTER_VENDOR_ID: u16 = 0x044F;

pub const TFLIGHT_HOTAS_ONE_PID: u16 = 0xB68B;
pub const TFLIGHT_HOTAS_4_PID: u16 = 0xB67A;

pub const USAGE_PAGE_GENERIC_DESKTOP: u16 = 0x01;

pub const USAGE_JOYSTICK: u16 = 0x04;
pub const USAGE_X: u16 = 0x30;
pub const USAGE_Y: u16 = 0x31;
pub const USAGE_RZ: u16 = 0x35;
pub const USAGE_SLIDER: u16 = 0x36;
pub const USAGE_DIAL: u16 = 0x37;

pub const AXIS_MODE_WARNING: &str =
    "Rudder + throttle are merged. Switch to 5/8 axis mode for full mapping.";
pub const DRIVER_NOTE: &str =
    "Missing axes or buttons? Install the Thrustmaster driver and confirm 5/8 axis mode.";
pub const DEFAULT_MAPPING_NOTE_UNKNOWN: &str =
    "Default mapping assumes 5/8 axis mode; verify axis mode before applying.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TFlightModel {
    HotasOne,
    Hotas4,
}

impl TFlightModel {
    pub fn name(&self) -> &'static str {
        match self {
            TFlightModel::HotasOne => "T.Flight HOTAS One",
            TFlightModel::Hotas4 => "T.Flight HOTAS 4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisMode {
    Merged,
    Separate,
    Unknown,
}

impl AxisMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AxisMode::Merged => "merged",
            AxisMode::Separate => "separate",
            AxisMode::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisUsageSummary {
    pub has_x: bool,
    pub has_y: bool,
    pub has_rz: bool,
    pub slider_like_count: u8,
}

impl AxisUsageSummary {
    pub fn from_usages(usages: &[HidUsage]) -> Self {
        let mut summary = AxisUsageSummary {
            has_x: false,
            has_y: false,
            has_rz: false,
            slider_like_count: 0,
        };

        for usage in usages {
            if usage.usage_page != USAGE_PAGE_GENERIC_DESKTOP {
                continue;
            }

            match usage.usage {
                USAGE_X => summary.has_x = true,
                USAGE_Y => summary.has_y = true,
                USAGE_RZ => summary.has_rz = true,
                USAGE_SLIDER | USAGE_DIAL => {
                    summary.slider_like_count = summary.slider_like_count.saturating_add(1);
                }
                _ => {}
            }
        }

        summary
    }
}

pub fn axis_mode_from_summary(summary: &AxisUsageSummary) -> AxisMode {
    if !(summary.has_x && summary.has_y && summary.has_rz) {
        return AxisMode::Unknown;
    }

    if summary.slider_like_count >= 2 {
        AxisMode::Separate
    } else if summary.slider_like_count == 0 {
        AxisMode::Merged
    } else {
        AxisMode::Unknown
    }
}

pub fn axis_mode_from_usages(usages: &[HidUsage]) -> AxisMode {
    let summary = AxisUsageSummary::from_usages(usages);
    axis_mode_from_summary(&summary)
}

pub fn axis_mode_from_descriptor(descriptor: &[u8]) -> AxisMode {
    let usages = extract_usages(descriptor);
    axis_mode_from_usages(&usages)
}

pub fn axis_mode_from_device_info(device_info: &HidDeviceInfo) -> AxisMode {
    match device_info.report_descriptor.as_deref() {
        Some(descriptor) => axis_mode_from_descriptor(descriptor),
        None => AxisMode::Unknown,
    }
}

pub fn tflight_model(device_info: &HidDeviceInfo) -> Option<TFlightModel> {
    if device_info.vendor_id != THRUSTMASTER_VENDOR_ID {
        return None;
    }

    match device_info.product_id {
        TFLIGHT_HOTAS_ONE_PID => Some(TFlightModel::HotasOne),
        TFLIGHT_HOTAS_4_PID => Some(TFlightModel::Hotas4),
        _ => None,
    }
}

pub fn is_tflight_device(device_info: &HidDeviceInfo) -> bool {
    tflight_model(device_info).is_some()
}

pub fn axis_mode_warning(axis_mode: AxisMode) -> Option<&'static str> {
    if axis_mode == AxisMode::Merged {
        Some(AXIS_MODE_WARNING)
    } else {
        None
    }
}

pub fn driver_note() -> &'static str {
    DRIVER_NOTE
}

pub fn default_mapping_note(axis_mode: AxisMode) -> Option<&'static str> {
    if axis_mode == AxisMode::Unknown {
        Some(DEFAULT_MAPPING_NOTE_UNKNOWN)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisUsage {
    X,
    Y,
    Rz,
    Slider0,
    Slider1,
    RzCombined,
}

impl fmt::Display for AxisUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AxisUsage::X => write!(f, "X"),
            AxisUsage::Y => write!(f, "Y"),
            AxisUsage::Rz => write!(f, "RZ"),
            AxisUsage::Slider0 => write!(f, "Slider0"),
            AxisUsage::Slider1 => write!(f, "Slider1"),
            AxisUsage::RzCombined => write!(f, "RZ (combined)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalControl {
    Axis(AxisUsage),
    Hat,
}

impl fmt::Display for PhysicalControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhysicalControl::Axis(axis) => write!(f, "{}", axis),
            PhysicalControl::Hat => write!(f, "Hat"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalControl {
    Roll,
    Pitch,
    Yaw,
    Throttle,
    Pov,
}

impl fmt::Display for LogicalControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicalControl::Roll => write!(f, "Roll"),
            LogicalControl::Pitch => write!(f, "Pitch"),
            LogicalControl::Yaw => write!(f, "Yaw"),
            LogicalControl::Throttle => write!(f, "Throttle"),
            LogicalControl::Pov => write!(f, "POV"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlBinding {
    pub physical: PhysicalControl,
    pub logical: LogicalControl,
    pub note: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultMapping {
    pub bindings: &'static [ControlBinding],
}

impl DefaultMapping {
    pub fn as_hint_string(&self) -> String {
        let mut out = String::new();
        for (idx, binding) in self.bindings.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("{}->{}", binding.physical, binding.logical));
            if let Some(note) = binding.note {
                out.push_str(" (");
                out.push_str(note);
                out.push(')');
            }
        }
        out
    }
}

const TFLIGHT_MAPPING_SEPARATE: [ControlBinding; 6] = [
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::X),
        logical: LogicalControl::Roll,
        note: None,
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Y),
        logical: LogicalControl::Pitch,
        note: Some("invert optional"),
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Slider0),
        logical: LogicalControl::Throttle,
        note: None,
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Rz),
        logical: LogicalControl::Yaw,
        note: Some("primary"),
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Slider1),
        logical: LogicalControl::Yaw,
        note: Some("alternate"),
    },
    ControlBinding {
        physical: PhysicalControl::Hat,
        logical: LogicalControl::Pov,
        note: None,
    },
];

const TFLIGHT_MAPPING_MERGED: [ControlBinding; 4] = [
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::X),
        logical: LogicalControl::Roll,
        note: None,
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Y),
        logical: LogicalControl::Pitch,
        note: Some("invert optional"),
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::RzCombined),
        logical: LogicalControl::Yaw,
        note: Some("combined"),
    },
    ControlBinding {
        physical: PhysicalControl::Hat,
        logical: LogicalControl::Pov,
        note: None,
    },
];

pub fn tflight_default_mapping(axis_mode: AxisMode) -> DefaultMapping {
    match axis_mode {
        AxisMode::Merged => DefaultMapping {
            bindings: &TFLIGHT_MAPPING_MERGED,
        },
        AxisMode::Separate | AxisMode::Unknown => DefaultMapping {
            bindings: &TFLIGHT_MAPPING_SEPARATE,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load_hex_fixture(name: &str) -> Vec<u8> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("fixtures");
        path.push(name);
        let content = std::fs::read_to_string(path).expect("fixture should exist");
        content
            .split_whitespace()
            .filter_map(|token| {
                let token = token.trim_start_matches("0x");
                u8::from_str_radix(token, 16).ok()
            })
            .collect()
    }

    #[test]
    fn test_axis_mode_from_usages() {
        let usages = vec![
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_X,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_Y,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_RZ,
            },
        ];

        assert_eq!(axis_mode_from_usages(&usages), AxisMode::Merged);

        let usages = vec![
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_X,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_Y,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_RZ,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_SLIDER,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_SLIDER,
            },
        ];

        assert_eq!(axis_mode_from_usages(&usages), AxisMode::Separate);
    }

    #[test]
    fn test_axis_mode_from_descriptor_fixtures() {
        let merged = load_hex_fixture("tflight_merged.hex");
        let separate = load_hex_fixture("tflight_separate.hex");

        assert_eq!(axis_mode_from_descriptor(&merged), AxisMode::Merged);
        assert_eq!(axis_mode_from_descriptor(&separate), AxisMode::Separate);
    }

    #[test]
    fn test_default_mapping_hint() {
        let mapping = tflight_default_mapping(AxisMode::Separate);
        let hint = mapping.as_hint_string();
        assert!(hint.contains("X->Roll"));
        assert!(hint.contains("Slider0->Throttle"));
        assert!(hint.contains("RZ->Yaw"));
    }

    #[test]
    fn test_tflight_model_detection() {
        let device_info = HidDeviceInfo {
            vendor_id: THRUSTMASTER_VENDOR_ID,
            product_id: TFLIGHT_HOTAS_ONE_PID,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: "/dev/test".to_string(),
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_JOYSTICK,
            report_descriptor: None,
        };

        assert_eq!(tflight_model(&device_info), Some(TFlightModel::HotasOne));
    }

    #[test]
    fn test_warning_and_notes() {
        assert_eq!(axis_mode_warning(AxisMode::Merged), Some(AXIS_MODE_WARNING));
        assert!(axis_mode_warning(AxisMode::Separate).is_none());
        assert!(driver_note().contains("Thrustmaster"));
        assert_eq!(
            default_mapping_note(AxisMode::Unknown),
            Some(DEFAULT_MAPPING_NOTE_UNKNOWN)
        );
    }
}
