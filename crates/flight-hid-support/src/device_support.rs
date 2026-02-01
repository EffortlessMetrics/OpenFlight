// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device support registry and quirk detection for common HID devices.

use crate::HidDeviceInfo;
use crate::hid_descriptor::{HidUsage, extract_usages};
use serde::Serialize;
use std::fmt;

pub const THRUSTMASTER_VENDOR_ID: u16 = 0x044F;
pub const VKB_VENDOR_ID: u16 = 0x231D;
pub const SAITEK_VENDOR_ID: u16 = 0x06A3;
pub const MAD_CATZ_VENDOR_ID: u16 = 0x0738;
pub const LOGITECH_VENDOR_ID: u16 = 0x046D;

pub const TFLIGHT_HOTAS_ONE_PID: u16 = 0xB68B;
pub const TFLIGHT_HOTAS_4_PID: u16 = 0xB67A;
/// Alternate PID for T.Flight HOTAS 4 - reported but unverified.
/// Some sources claim 0xB67B; requires lsusb/USBView artifact for confirmation.
pub const TFLIGHT_HOTAS_4_PID_ALT: u16 = 0xB67B;

// Saitek/Logitech HOTAS PIDs
// See docs/reference/hotas-claims.md for verification status
//
// X52 family (unified USB) - confidence: KNOWN
pub const X52_PID: u16 = 0x075C;
pub const X52_PRO_PID: u16 = 0x0762;

// X55 family (split USB, Saitek VID 0x06A3) - confidence: LIKELY
// Note: Some X55 units may use Mad Catz VID (0x0738) with same PIDs
pub const X55_STICK_PID: u16 = 0x2215;
pub const X55_THROTTLE_PID: u16 = 0xA215;

// X56 family - Mad Catz era (split USB, VID 0x0738) - confidence: LIKELY
// These are the "blue" X56 units from the Mad Catz acquisition period
pub const X56_MADCATZ_STICK_PID: u16 = 0x2221;
pub const X56_MADCATZ_THROTTLE_PID: u16 = 0xA221;

// X56 family - Logitech branded (split USB, VID 0x046D) - confidence: LIKELY/SUSPECT
// Stick PID 0xC229 is likely correct
// WARNING: Throttle PID 0xC22A may conflict with Logitech G110 keyboard!
// See docs/reference/hotas-claims.md - requires lsusb verification from real hardware
pub const X56_LOGITECH_STICK_PID: u16 = 0xC229;
// SUSPECT: This PID needs verification - do NOT match unknown Logitech PIDs
// pub const X56_LOGITECH_THROTTLE_PID: u16 = 0xC22A;

pub const VKB_STECS_LEFT_SPACE_MINI_PID: u16 = 0x0136;
pub const VKB_STECS_RIGHT_SPACE_MINI_PID: u16 = 0x013A;
pub const VKB_STECS_LEFT_SPACE_MINI_PLUS_PID: u16 = 0x0137;
pub const VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID: u16 = 0x013B;
pub const VKB_STECS_LEFT_SPACE_STANDARD_PID: u16 = 0x0138;
pub const VKB_STECS_RIGHT_SPACE_STANDARD_PID: u16 = 0x013C;
pub const VKB_GLADIATOR_NXT_EVO_RIGHT_PID: u16 = 0x0200;
pub const VKB_GLADIATOR_NXT_EVO_LEFT_PID: u16 = 0x0201;

pub const USAGE_PAGE_GENERIC_DESKTOP: u16 = 0x01;
pub const USAGE_PAGE_BUTTON: u16 = 0x09;

pub const USAGE_JOYSTICK: u16 = 0x04;
pub const USAGE_X: u16 = 0x30;
pub const USAGE_Y: u16 = 0x31;
pub const USAGE_Z: u16 = 0x32;
pub const USAGE_RX: u16 = 0x33;
pub const USAGE_RY: u16 = 0x34;
pub const USAGE_RZ: u16 = 0x35;
pub const USAGE_SLIDER: u16 = 0x36;
pub const USAGE_DIAL: u16 = 0x37;
pub const USAGE_WHEEL: u16 = 0x38;
pub const USAGE_HAT_SWITCH: u16 = 0x39;

pub const AXIS_MODE_WARNING: &str =
    "Rudder sources are merged. Switch to full-axis mode for separate yaw inputs.";
pub const DRIVER_NOTE: &str =
    "Missing axes or buttons? Install the Thrustmaster driver and confirm full-axis mode.";
pub const DEFAULT_MAPPING_NOTE_UNKNOWN: &str =
    "Default mapping assumes full-axis mode; verify axis mode before applying.";

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
pub enum VkbStecsVariant {
    RightSpaceThrottleGripMini,
    LeftSpaceThrottleGripMini,
    RightSpaceThrottleGripMiniPlus,
    LeftSpaceThrottleGripMiniPlus,
    RightSpaceThrottleGripStandard,
    LeftSpaceThrottleGripStandard,
}

impl VkbStecsVariant {
    pub fn name(&self) -> &'static str {
        match self {
            VkbStecsVariant::RightSpaceThrottleGripMini => {
                "VKB STECS Right Space Throttle Grip Mini"
            }
            VkbStecsVariant::LeftSpaceThrottleGripMini => "VKB STECS Left Space Throttle Grip Mini",
            VkbStecsVariant::RightSpaceThrottleGripMiniPlus => {
                "VKB STECS Right Space Throttle Grip Mini+"
            }
            VkbStecsVariant::LeftSpaceThrottleGripMiniPlus => {
                "VKB STECS Left Space Throttle Grip Mini+"
            }
            VkbStecsVariant::RightSpaceThrottleGripStandard => {
                "VKB STECS Right Space Throttle Grip Standard"
            }
            VkbStecsVariant::LeftSpaceThrottleGripStandard => {
                "VKB STECS Left Space Throttle Grip Standard"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkbGladiatorVariant {
    NxtEvoRight,
    NxtEvoLeft,
}

impl VkbGladiatorVariant {
    pub fn name(&self) -> &'static str {
        match self {
            VkbGladiatorVariant::NxtEvoRight => "VKB Gladiator NXT EVO Right",
            VkbGladiatorVariant::NxtEvoLeft => "VKB Gladiator NXT EVO Left",
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
        TFLIGHT_HOTAS_4_PID | TFLIGHT_HOTAS_4_PID_ALT => Some(TFlightModel::Hotas4),
        _ => None,
    }
}

/// Returns true if the HOTAS 4 was detected via the alternate (unverified) PID.
///
/// This allows diagnostics/UI to warn the user that their device was matched
/// via an unverified PID and request lsusb/USBView artifacts for confirmation.
pub fn is_hotas4_alt_pid(device_info: &HidDeviceInfo) -> bool {
    device_info.vendor_id == THRUSTMASTER_VENDOR_ID
        && device_info.product_id == TFLIGHT_HOTAS_4_PID_ALT
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AxisUsage {
    X,
    Y,
    Z,
    Rx,
    Ry,
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
            AxisUsage::Z => write!(f, "Z"),
            AxisUsage::Rx => write!(f, "RX"),
            AxisUsage::Ry => write!(f, "RY"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AxisControl {
    pub usage: AxisUsage,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ButtonControl {
    pub index: u8,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EncoderControl {
    pub name: &'static str,
    pub cw_button: u8,
    pub ccw_button: u8,
    pub press_button: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DeviceControlMap {
    pub schema: &'static str,
    pub axes: &'static [AxisControl],
    pub buttons: &'static [ButtonControl],
    pub encoders: &'static [EncoderControl],
    pub notes: &'static [&'static str],
}

const DESCRIPTOR_DISCOVERY_SCHEMA: &str = "flight.hid-discovery/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DescriptorCounts {
    pub axes: usize,
    pub hats: usize,
    pub buttons: usize,
    pub other: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredAxis {
    pub usage_page: u16,
    pub usage: u16,
    pub index: u8,
    pub label: String,
    pub suggested_logical: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredHat {
    pub usage_page: u16,
    pub usage: u16,
    pub index: u8,
    pub label: String,
    pub suggested_logical: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredButton {
    pub usage_page: u16,
    pub usage: u16,
    pub index: u16,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DescriptorDiscovery {
    pub schema: &'static str,
    pub counts: DescriptorCounts,
    pub usages: Vec<HidUsage>,
    pub axes: Vec<DiscoveredAxis>,
    pub hats: Vec<DiscoveredHat>,
    pub buttons: Vec<DiscoveredButton>,
    pub notes: Vec<String>,
}

const DESCRIPTOR_DISCOVERY_NOTES: [&str; 2] = [
    "Derived from HID report descriptor usage tags; treat as best-effort.",
    "Prefer logical min/max and report sizes when building authoritative maps.",
];

const VKB_DISCOVERY_NOTES: [&str; 2] = [
    "VKBDevCfg can remap hats, ministicks, and axes; do not hardcode order.",
    "GNX modules may expose multiple HID devices or collections; group by serial or arrival time.",
];

const VKB_GLADIATOR_DISCOVERY_NOTES: [&str; 2] = [
    "Omni Throttle uses the same USB PID as Gladiator NXT EVO variants.",
    "Treat default mappings as hints; prefer descriptor-first discovery.",
];

fn axis_label_for_usage(usage: u16) -> Option<&'static str> {
    match usage {
        USAGE_X => Some("X"),
        USAGE_Y => Some("Y"),
        USAGE_Z => Some("Z"),
        USAGE_RX => Some("Rx"),
        USAGE_RY => Some("Ry"),
        USAGE_RZ => Some("Rz"),
        USAGE_SLIDER => Some("Slider"),
        USAGE_DIAL => Some("Dial"),
        USAGE_WHEEL => Some("Wheel"),
        _ => None,
    }
}

fn suggested_logical_for_axis(usage: u16) -> Option<&'static str> {
    match usage {
        USAGE_X => Some("roll"),
        USAGE_Y => Some("pitch"),
        USAGE_RZ => Some("yaw_candidate"),
        USAGE_SLIDER | USAGE_DIAL | USAGE_WHEEL => Some("throttle_candidate"),
        _ => None,
    }
}

fn suggested_logical_for_hat(usage: u16) -> Option<&'static str> {
    if usage == USAGE_HAT_SWITCH {
        Some("pov")
    } else {
        None
    }
}

fn push_note_lines(target: &mut Vec<String>, notes: &[&str]) {
    for note in notes {
        target.push((*note).to_string());
    }
}

pub fn descriptor_discovery_from_usages(usages: &[HidUsage]) -> DescriptorDiscovery {
    let mut axes = Vec::new();
    let mut hats = Vec::new();
    let mut buttons = Vec::new();
    let mut axis_index: u8 = 0;
    let mut hat_index: u8 = 0;

    for usage in usages {
        if usage.usage_page == USAGE_PAGE_GENERIC_DESKTOP {
            if usage.usage == USAGE_HAT_SWITCH {
                hats.push(DiscoveredHat {
                    usage_page: usage.usage_page,
                    usage: usage.usage,
                    index: hat_index,
                    label: "Hat switch".to_string(),
                    suggested_logical: suggested_logical_for_hat(usage.usage).map(str::to_string),
                });
                hat_index = hat_index.saturating_add(1);
                continue;
            }

            if let Some(label) = axis_label_for_usage(usage.usage) {
                axes.push(DiscoveredAxis {
                    usage_page: usage.usage_page,
                    usage: usage.usage,
                    index: axis_index,
                    label: label.to_string(),
                    suggested_logical: suggested_logical_for_axis(usage.usage).map(str::to_string),
                });
                axis_index = axis_index.saturating_add(1);
                continue;
            }
        }

        if usage.usage_page == USAGE_PAGE_BUTTON {
            let index = usage.usage;
            buttons.push(DiscoveredButton {
                usage_page: usage.usage_page,
                usage: usage.usage,
                index,
                label: format!("Button {}", index),
            });
        }
    }

    let counts = DescriptorCounts {
        axes: axes.len(),
        hats: hats.len(),
        buttons: buttons.len(),
        other: usages
            .len()
            .saturating_sub(axes.len() + hats.len() + buttons.len()),
    };

    let mut notes = Vec::new();
    push_note_lines(&mut notes, &DESCRIPTOR_DISCOVERY_NOTES);

    DescriptorDiscovery {
        schema: DESCRIPTOR_DISCOVERY_SCHEMA,
        counts,
        usages: usages.to_vec(),
        axes,
        hats,
        buttons,
        notes,
    }
}

pub fn descriptor_discovery_from_descriptor(descriptor: &[u8]) -> DescriptorDiscovery {
    let usages = extract_usages(descriptor);
    descriptor_discovery_from_usages(&usages)
}

pub fn descriptor_discovery_from_device_info(
    device_info: &HidDeviceInfo,
) -> Option<DescriptorDiscovery> {
    let descriptor = device_info.report_descriptor.as_deref()?;
    let mut discovery = descriptor_discovery_from_descriptor(descriptor);

    if device_info.vendor_id == VKB_VENDOR_ID {
        push_note_lines(&mut discovery.notes, &VKB_DISCOVERY_NOTES);
    }

    if is_vkb_gladiator_device(device_info) {
        push_note_lines(&mut discovery.notes, &VKB_GLADIATOR_DISCOVERY_NOTES);
    }

    Some(discovery)
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

const VKB_STECS_CONTROL_MAP_SCHEMA: &str = "flight.device-map/1";
const VKB_STECS_NOTES: [&str; 3] = [
    "Button/axis labels are derived from Elite Dangerous buttonMap files.",
    "VKBDevCfg profiles can remap buttons, encoders, and virtual buttons.",
    "Treat this map as a baseline; prefer HID usage/descriptor for authority.",
];

const VKB_STECS_RIGHT_MINI_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS [x52z]",
    },
];

const VKB_STECS_RIGHT_MINI_BUTTONS: [ButtonControl; 29] = [
    ButtonControl {
        index: 1,
        name: "STECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 12,
        name: "STECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 13,
        name: "STECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 14,
        name: "STECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 15,
        name: "STECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 16,
        name: "STECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "STECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "STECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 19,
        name: "STECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 20,
        name: "STECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 21,
        name: "STECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 22,
        name: "STECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 23,
        name: "STECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 24,
        name: "STECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 25,
        name: "STECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "STECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 27,
        name: "STECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 28,
        name: "STECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 29,
        name: "STECS H2 [x360LThumb]",
    },
];

const VKB_STECS_LEFT_MINI_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS [x52z]",
    },
];

const VKB_STECS_LEFT_MINI_BUTTONS: [ButtonControl; 29] = [
    ButtonControl {
        index: 1,
        name: "STECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 12,
        name: "STECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 13,
        name: "STECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 14,
        name: "STECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 15,
        name: "STECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 16,
        name: "STECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "STECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "STECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 19,
        name: "STECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 20,
        name: "STECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 21,
        name: "STECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 22,
        name: "STECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 23,
        name: "STECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 24,
        name: "STECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 25,
        name: "STECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "STECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 27,
        name: "STECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 28,
        name: "STECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 29,
        name: "STECS H2 [x360LThumb]",
    },
];

const VKB_STECS_LEFT_MINI_PLUS_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "LSTECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "LSTECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "LSTECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "LSTECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "LSTECS Throttle",
    },
];

const VKB_STECS_LEFT_MINI_PLUS_BUTTONS: [ButtonControl; 42] = [
    ButtonControl {
        index: 1,
        name: "LSTECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "LSTECS Start",
    },
    ButtonControl {
        index: 3,
        name: "LSTECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "LSTECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "LSTECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "LSTECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "LSTECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "LSTECS Rot CCW",
    },
    ButtonControl {
        index: 9,
        name: "LSTECS Rot CW",
    },
    ButtonControl {
        index: 10,
        name: "LSTECS Safe",
    },
    ButtonControl {
        index: 11,
        name: "LSTECS #1",
    },
    ButtonControl {
        index: 12,
        name: "LSTECS #2",
    },
    ButtonControl {
        index: 13,
        name: "LSTECS #3",
    },
    ButtonControl {
        index: 14,
        name: "LSTECS #4",
    },
    ButtonControl {
        index: 15,
        name: "LSTECS Armed",
    },
    ButtonControl {
        index: 16,
        name: "LSTECS Rot [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "LSTECS Rot [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "LSTECS Rot [ps4PadR]",
    },
    ButtonControl {
        index: 19,
        name: "LSTECS Rot [ps4PadL]",
    },
    ButtonControl {
        index: 20,
        name: "LSTECS Rot Click",
    },
    ButtonControl {
        index: 21,
        name: "LSTECS B1",
    },
    ButtonControl {
        index: 22,
        name: "LSTECS Trigger",
    },
    ButtonControl {
        index: 23,
        name: "LSTECS B2",
    },
    ButtonControl {
        index: 24,
        name: "LSTECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 25,
        name: "LSTECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "LSTECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 27,
        name: "LSTECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 28,
        name: "LSTECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 29,
        name: "LSTECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 30,
        name: "LSTECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 31,
        name: "LSTECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 32,
        name: "LSTECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 33,
        name: "LSTECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 34,
        name: "LSTECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 35,
        name: "LSTECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 36,
        name: "LSTECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 37,
        name: "LSTECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 38,
        name: "LSTECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 39,
        name: "LSTECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 40,
        name: "LSTECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 41,
        name: "LSTECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 42,
        name: "LSTECS H2 [x360LThumb]",
    },
];

const VKB_STECS_RIGHT_MINI_PLUS_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "RSTECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "RSTECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "RSTECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "RSTECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "RSTECS Throttle",
    },
];

const VKB_STECS_RIGHT_MINI_PLUS_BUTTONS: [ButtonControl; 42] = [
    ButtonControl {
        index: 1,
        name: "RSTECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "RSTECS Start",
    },
    ButtonControl {
        index: 3,
        name: "RSTECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "RSTECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "RSTECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "RSTECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "RSTECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "RSTECS Rot CCW",
    },
    ButtonControl {
        index: 9,
        name: "RSTECS Rot CW",
    },
    ButtonControl {
        index: 10,
        name: "RSTECS Safe",
    },
    ButtonControl {
        index: 11,
        name: "RSTECS #1",
    },
    ButtonControl {
        index: 12,
        name: "RSTECS #2",
    },
    ButtonControl {
        index: 13,
        name: "RSTECS #3",
    },
    ButtonControl {
        index: 14,
        name: "RSTECS #4",
    },
    ButtonControl {
        index: 15,
        name: "RSTECS Armed",
    },
    ButtonControl {
        index: 16,
        name: "RSTECS Rot [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "RSTECS Rot [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "RSTECS Rot [ps4PadR]",
    },
    ButtonControl {
        index: 19,
        name: "RSTECS Rot [ps4PadL]",
    },
    ButtonControl {
        index: 20,
        name: "RSTECS Rot Click",
    },
    ButtonControl {
        index: 21,
        name: "RSTECS B1",
    },
    ButtonControl {
        index: 22,
        name: "RSTECS Trigger",
    },
    ButtonControl {
        index: 23,
        name: "RSTECS B2",
    },
    ButtonControl {
        index: 24,
        name: "RSTECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 25,
        name: "RSTECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "RSTECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 27,
        name: "RSTECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 28,
        name: "RSTECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 29,
        name: "RSTECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 30,
        name: "RSTECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 31,
        name: "RSTECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 32,
        name: "RSTECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 33,
        name: "RSTECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 34,
        name: "RSTECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 35,
        name: "RSTECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 36,
        name: "RSTECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 37,
        name: "RSTECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 38,
        name: "RSTECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 39,
        name: "RSTECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 40,
        name: "RSTECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 41,
        name: "RSTECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 42,
        name: "RSTECS H2 [x360LThumb]",
    },
];

const VKB_STECS_LEFT_STANDARD_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS - Space Brake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS - Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS - [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS - [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS - [x52z]",
    },
];

const VKB_STECS_LEFT_STANDARD_BUTTONS: [ButtonControl; 53] = [
    ButtonControl {
        index: 1,
        name: "STECS - Base Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS - Base Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS - Base Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS - Base Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS - Base Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS - Base Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS - Base Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS - B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS - Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS - B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS - Speed Push",
    },
    ButtonControl {
        index: 12,
        name: "STECS - Speed Up",
    },
    ButtonControl {
        index: 13,
        name: "STECS - Speed Down",
    },
    ButtonControl {
        index: 14,
        name: "STECS - Index Push",
    },
    ButtonControl {
        index: 15,
        name: "STECS - HAT1 Push",
    },
    ButtonControl {
        index: 16,
        name: "STECS - Index Fore",
    },
    ButtonControl {
        index: 17,
        name: "STECS - Index Back",
    },
    ButtonControl {
        index: 18,
        name: "STECS - Index Left",
    },
    ButtonControl {
        index: 19,
        name: "STECS - Index Right",
    },
    ButtonControl {
        index: 20,
        name: "STECS - HAT1 Back",
    },
    ButtonControl {
        index: 21,
        name: "STECS - HAT1 Fore",
    },
    ButtonControl {
        index: 22,
        name: "STECS - HAT1 Down",
    },
    ButtonControl {
        index: 23,
        name: "STECS - HAT1 Up",
    },
    ButtonControl {
        index: 24,
        name: "STECS - H1 Down",
    },
    ButtonControl {
        index: 25,
        name: "STECS - H1 Up",
    },
    ButtonControl {
        index: 26,
        name: "STECS - H1 Push",
    },
    ButtonControl {
        index: 27,
        name: "STECS - H2 Back",
    },
    ButtonControl {
        index: 28,
        name: "STECS - H2 Fore",
    },
    ButtonControl {
        index: 29,
        name: "STECS - H2 Push",
    },
    ButtonControl {
        index: 30,
        name: "STECS - STEM A1",
    },
    ButtonControl {
        index: 31,
        name: "STECS - STEM A2",
    },
    ButtonControl {
        index: 32,
        name: "STECS - STEM C1",
    },
    ButtonControl {
        index: 33,
        name: "STECS - STEM B1",
    },
    ButtonControl {
        index: 34,
        name: "STECS - STEM B2",
    },
    ButtonControl {
        index: 35,
        name: "STECS - STEM B3",
    },
    ButtonControl {
        index: 36,
        name: "STECS - STEM B4",
    },
    ButtonControl {
        index: 37,
        name: "STECS - STEM B5",
    },
    ButtonControl {
        index: 38,
        name: "STECS - STEM Sw1 Up",
    },
    ButtonControl {
        index: 39,
        name: "STECS - STEM Sw1 Mid",
    },
    ButtonControl {
        index: 40,
        name: "STECS - STEM Sw1 Down",
    },
    ButtonControl {
        index: 41,
        name: "STECS - STEM Sw2 Up",
    },
    ButtonControl {
        index: 42,
        name: "STECS - STEM Sw2 Mid",
    },
    ButtonControl {
        index: 43,
        name: "STECS - STEM Sw2 Down",
    },
    ButtonControl {
        index: 44,
        name: "STECS - STEM Tgl Up",
    },
    ButtonControl {
        index: 45,
        name: "STECS - STEM Tgl Down",
    },
    ButtonControl {
        index: 46,
        name: "STECS - STEM Enc1 CCW",
    },
    ButtonControl {
        index: 47,
        name: "STECS - STEM Enc1 CW",
    },
    ButtonControl {
        index: 48,
        name: "STECS - STEM Enc2 CCW",
    },
    ButtonControl {
        index: 49,
        name: "STECS - STEM Enc2 CW",
    },
    ButtonControl {
        index: 50,
        name: "STECS - STEM Enc1 Push",
    },
    ButtonControl {
        index: 51,
        name: "STECS - STEM Enc2 Push",
    },
    ButtonControl {
        index: 52,
        name: "STECS - STEM Flap Up",
    },
    ButtonControl {
        index: 53,
        name: "STECS - STEM Flap Down",
    },
];

const VKB_STECS_RIGHT_STANDARD_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS - Space Brake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS - Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS - [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS - [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS - [x52z]",
    },
];

const VKB_STECS_RIGHT_STANDARD_BUTTONS: [ButtonControl; 53] = [
    ButtonControl {
        index: 1,
        name: "STECS - Base Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS - Base Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS - Base Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS - Base Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS - Base Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS - Base Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS - Base Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS - B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS - Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS - B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS - Speed Push",
    },
    ButtonControl {
        index: 12,
        name: "STECS - Speed Up",
    },
    ButtonControl {
        index: 13,
        name: "STECS - Speed Down",
    },
    ButtonControl {
        index: 14,
        name: "STECS - Index Push",
    },
    ButtonControl {
        index: 15,
        name: "STECS - HAT1 Push",
    },
    ButtonControl {
        index: 16,
        name: "STECS - Index Fore",
    },
    ButtonControl {
        index: 17,
        name: "STECS - Index Back",
    },
    ButtonControl {
        index: 18,
        name: "STECS - Index Right",
    },
    ButtonControl {
        index: 19,
        name: "STECS - Index Left",
    },
    ButtonControl {
        index: 20,
        name: "STECS - HAT1 Back",
    },
    ButtonControl {
        index: 21,
        name: "STECS - HAT1 Fore",
    },
    ButtonControl {
        index: 22,
        name: "STECS - HAT1 Down",
    },
    ButtonControl {
        index: 23,
        name: "STECS - HAT1 Up",
    },
    ButtonControl {
        index: 24,
        name: "STECS - H1 Down",
    },
    ButtonControl {
        index: 25,
        name: "STECS - H1 Up",
    },
    ButtonControl {
        index: 26,
        name: "STECS - H1 Push",
    },
    ButtonControl {
        index: 27,
        name: "STECS - H2 Back",
    },
    ButtonControl {
        index: 28,
        name: "STECS - H2 Fore",
    },
    ButtonControl {
        index: 29,
        name: "STECS - H2 Push",
    },
    ButtonControl {
        index: 30,
        name: "STECS - STEM A1",
    },
    ButtonControl {
        index: 31,
        name: "STECS - STEM A2",
    },
    ButtonControl {
        index: 32,
        name: "STECS - STEM C1",
    },
    ButtonControl {
        index: 33,
        name: "STECS - STEM B1",
    },
    ButtonControl {
        index: 34,
        name: "STECS - STEM B2",
    },
    ButtonControl {
        index: 35,
        name: "STECS - STEM B3",
    },
    ButtonControl {
        index: 36,
        name: "STECS - STEM B4",
    },
    ButtonControl {
        index: 37,
        name: "STECS - STEM B5",
    },
    ButtonControl {
        index: 38,
        name: "STECS - STEM Sw1 Up",
    },
    ButtonControl {
        index: 39,
        name: "STECS - STEM Sw1 Mid",
    },
    ButtonControl {
        index: 40,
        name: "STECS - STEM Sw1 Down",
    },
    ButtonControl {
        index: 41,
        name: "STECS - STEM Sw2 Up",
    },
    ButtonControl {
        index: 42,
        name: "STECS - STEM Sw2 Mid",
    },
    ButtonControl {
        index: 43,
        name: "STECS - STEM Sw2 Down",
    },
    ButtonControl {
        index: 44,
        name: "STECS - STEM Tgl Up",
    },
    ButtonControl {
        index: 45,
        name: "STECS - STEM Tgl Down",
    },
    ButtonControl {
        index: 46,
        name: "STECS - STEM Enc1 CCW",
    },
    ButtonControl {
        index: 47,
        name: "STECS - STEM Enc1 CW",
    },
    ButtonControl {
        index: 48,
        name: "STECS - STEM Enc2 CCW",
    },
    ButtonControl {
        index: 49,
        name: "STECS - STEM Enc2 CW",
    },
    ButtonControl {
        index: 50,
        name: "STECS - STEM Enc1 Push",
    },
    ButtonControl {
        index: 51,
        name: "STECS - STEM Enc2 Push",
    },
    ButtonControl {
        index: 52,
        name: "STECS - STEM Flap Up",
    },
    ButtonControl {
        index: 53,
        name: "STECS - STEM Flap Down",
    },
];

const VKB_STECS_RIGHT_MINI_ENCODERS: [EncoderControl; 0] = [];
const VKB_STECS_LEFT_MINI_ENCODERS: [EncoderControl; 0] = [];
const VKB_STECS_LEFT_MINI_PLUS_ENCODERS: [EncoderControl; 1] = [EncoderControl {
    name: "LSTECS Rot",
    cw_button: 9,
    ccw_button: 8,
    press_button: Some(20),
}];
const VKB_STECS_RIGHT_MINI_PLUS_ENCODERS: [EncoderControl; 1] = [EncoderControl {
    name: "RSTECS Rot",
    cw_button: 9,
    ccw_button: 8,
    press_button: Some(20),
}];
const VKB_STECS_STANDARD_ENCODERS: [EncoderControl; 2] = [
    EncoderControl {
        name: "STECS - STEM Enc1",
        cw_button: 47,
        ccw_button: 46,
        press_button: Some(50),
    },
    EncoderControl {
        name: "STECS - STEM Enc2",
        cw_button: 49,
        ccw_button: 48,
        press_button: Some(51),
    },
];

const VKB_STECS_RIGHT_MINI_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_RIGHT_MINI_AXES,
    buttons: &VKB_STECS_RIGHT_MINI_BUTTONS,
    encoders: &VKB_STECS_RIGHT_MINI_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_LEFT_MINI_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_LEFT_MINI_AXES,
    buttons: &VKB_STECS_LEFT_MINI_BUTTONS,
    encoders: &VKB_STECS_LEFT_MINI_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_LEFT_MINI_PLUS_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_LEFT_MINI_PLUS_AXES,
    buttons: &VKB_STECS_LEFT_MINI_PLUS_BUTTONS,
    encoders: &VKB_STECS_LEFT_MINI_PLUS_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_RIGHT_MINI_PLUS_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_RIGHT_MINI_PLUS_AXES,
    buttons: &VKB_STECS_RIGHT_MINI_PLUS_BUTTONS,
    encoders: &VKB_STECS_RIGHT_MINI_PLUS_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_LEFT_STANDARD_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_LEFT_STANDARD_AXES,
    buttons: &VKB_STECS_LEFT_STANDARD_BUTTONS,
    encoders: &VKB_STECS_STANDARD_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_RIGHT_STANDARD_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_RIGHT_STANDARD_AXES,
    buttons: &VKB_STECS_RIGHT_STANDARD_BUTTONS,
    encoders: &VKB_STECS_STANDARD_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

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

pub fn vkb_gladiator_variant(device_info: &HidDeviceInfo) -> Option<VkbGladiatorVariant> {
    if device_info.vendor_id != VKB_VENDOR_ID {
        return None;
    }

    match device_info.product_id {
        VKB_GLADIATOR_NXT_EVO_RIGHT_PID => Some(VkbGladiatorVariant::NxtEvoRight),
        VKB_GLADIATOR_NXT_EVO_LEFT_PID => Some(VkbGladiatorVariant::NxtEvoLeft),
        _ => None,
    }
}

pub fn is_vkb_gladiator_device(device_info: &HidDeviceInfo) -> bool {
    vkb_gladiator_variant(device_info).is_some()
}

pub fn vkb_stecs_variant(device_info: &HidDeviceInfo) -> Option<VkbStecsVariant> {
    if device_info.vendor_id != VKB_VENDOR_ID {
        return None;
    }

    match device_info.product_id {
        VKB_STECS_RIGHT_SPACE_MINI_PID => Some(VkbStecsVariant::RightSpaceThrottleGripMini),
        VKB_STECS_LEFT_SPACE_MINI_PID => Some(VkbStecsVariant::LeftSpaceThrottleGripMini),
        VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID => {
            Some(VkbStecsVariant::RightSpaceThrottleGripMiniPlus)
        }
        VKB_STECS_LEFT_SPACE_MINI_PLUS_PID => Some(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus),
        VKB_STECS_RIGHT_SPACE_STANDARD_PID => Some(VkbStecsVariant::RightSpaceThrottleGripStandard),
        VKB_STECS_LEFT_SPACE_STANDARD_PID => Some(VkbStecsVariant::LeftSpaceThrottleGripStandard),
        _ => None,
    }
}

pub fn is_vkb_stecs_device(device_info: &HidDeviceInfo) -> bool {
    vkb_stecs_variant(device_info).is_some()
}

pub fn vkb_stecs_control_map(variant: VkbStecsVariant) -> &'static DeviceControlMap {
    match variant {
        VkbStecsVariant::RightSpaceThrottleGripMini => &VKB_STECS_RIGHT_MINI_CONTROL_MAP,
        VkbStecsVariant::LeftSpaceThrottleGripMini => &VKB_STECS_LEFT_MINI_CONTROL_MAP,
        VkbStecsVariant::RightSpaceThrottleGripMiniPlus => &VKB_STECS_RIGHT_MINI_PLUS_CONTROL_MAP,
        VkbStecsVariant::LeftSpaceThrottleGripMiniPlus => &VKB_STECS_LEFT_MINI_PLUS_CONTROL_MAP,
        VkbStecsVariant::RightSpaceThrottleGripStandard => &VKB_STECS_RIGHT_STANDARD_CONTROL_MAP,
        VkbStecsVariant::LeftSpaceThrottleGripStandard => &VKB_STECS_LEFT_STANDARD_CONTROL_MAP,
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

    fn vkb_device(product_id: u16) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: VKB_VENDOR_ID,
            product_id,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: "/dev/test-vkb".to_string(),
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_JOYSTICK,
            report_descriptor: None,
        }
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
    fn test_descriptor_discovery_from_usages() {
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
                usage: USAGE_HAT_SWITCH,
            },
            HidUsage {
                usage_page: USAGE_PAGE_BUTTON,
                usage: 1,
            },
            HidUsage {
                usage_page: USAGE_PAGE_BUTTON,
                usage: 2,
            },
            HidUsage {
                usage_page: 0xFF00,
                usage: 1,
            },
        ];

        let discovery = descriptor_discovery_from_usages(&usages);
        assert_eq!(discovery.counts.axes, 2);
        assert_eq!(discovery.counts.hats, 1);
        assert_eq!(discovery.counts.buttons, 2);
        assert_eq!(discovery.counts.other, 1);
        assert_eq!(discovery.axes[0].label, "X");
        assert_eq!(discovery.hats[0].label, "Hat switch");
        assert_eq!(discovery.buttons[0].label, "Button 1");
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
    fn test_vkb_stecs_variant_detection() {
        let device_info = vkb_device(VKB_STECS_RIGHT_SPACE_MINI_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::RightSpaceThrottleGripMini)
        );
        assert!(is_vkb_stecs_device(&device_info));

        let device_info = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::LeftSpaceThrottleGripMini)
        );

        let device_info = vkb_device(VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::RightSpaceThrottleGripMiniPlus)
        );

        let device_info = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PLUS_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus)
        );

        let device_info = vkb_device(VKB_STECS_RIGHT_SPACE_STANDARD_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::RightSpaceThrottleGripStandard)
        );

        let device_info = vkb_device(VKB_STECS_LEFT_SPACE_STANDARD_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::LeftSpaceThrottleGripStandard)
        );
    }

    #[test]
    fn test_vkb_gladiator_variant_detection() {
        let device_info = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        assert_eq!(
            vkb_gladiator_variant(&device_info),
            Some(VkbGladiatorVariant::NxtEvoRight)
        );
        assert!(is_vkb_gladiator_device(&device_info));

        let device_info = vkb_device(VKB_GLADIATOR_NXT_EVO_LEFT_PID);
        assert_eq!(
            vkb_gladiator_variant(&device_info),
            Some(VkbGladiatorVariant::NxtEvoLeft)
        );
    }

    #[test]
    fn test_vkb_stecs_control_map_contents() {
        let control_map = vkb_stecs_control_map(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus);
        assert_eq!(control_map.schema, "flight.device-map/1");
        assert!(
            control_map
                .axes
                .iter()
                .any(|axis| axis.usage == AxisUsage::Z && axis.name.contains("Throttle"))
        );
        assert!(
            control_map
                .buttons
                .iter()
                .any(|button| button.index == 8 && button.name.contains("Rot CCW"))
        );
        assert_eq!(control_map.encoders.len(), 1);
        assert_eq!(control_map.encoders[0].cw_button, 9);
        assert_eq!(control_map.encoders[0].ccw_button, 8);
        assert_eq!(control_map.encoders[0].press_button, Some(20));

        let control_map = vkb_stecs_control_map(VkbStecsVariant::RightSpaceThrottleGripStandard);
        assert_eq!(control_map.encoders.len(), 2);
        assert_eq!(control_map.encoders[0].cw_button, 47);
        assert_eq!(control_map.encoders[0].ccw_button, 46);
        assert_eq!(control_map.encoders[0].press_button, Some(50));
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
