// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device support registry and quirk detection for common HID devices.

use crate::HidDeviceInfo;
use crate::hid_descriptor::{HidUsage, extract_usages};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

pub const THRUSTMASTER_VENDOR_ID: u16 = 0x044F;
pub const VKB_VENDOR_ID: u16 = 0x231D;
pub const SAITEK_VENDOR_ID: u16 = 0x06A3;
pub const MAD_CATZ_VENDOR_ID: u16 = 0x0738;
pub const LOGITECH_VENDOR_ID: u16 = 0x046D;

/// USB Product ID for the Logitech Extreme 3D Pro joystick.
///
/// Confirmed: VID 0x046D (Logitech), PID 0xC215 — from linux-hardware.org (221 probes).
pub const EXTREME_3D_PRO_PID: u16 = 0xC215;

/// Returns `true` if this VID/PID combination is a Logitech Extreme 3D Pro.
pub fn is_extreme_3d_pro(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == LOGITECH_VENDOR_ID && product_id == EXTREME_3D_PRO_PID
}

/// USB Vendor ID for all Honeycomb Aeronautical products.
pub const HONEYCOMB_VENDOR_ID: u16 = 0x294B;

/// USB Product ID for the Honeycomb Alpha Flight Controls XPC (Yoke).
///
/// **Caution:** This PID is community-reported (0x0102) and has not been confirmed
/// with hardware. Verify with `lsusb` / USBView against real hardware before
/// relying on it for device matching.
pub const HONEYCOMB_ALPHA_YOKE_PID: u16 = 0x0102;

/// USB Product ID for the Honeycomb Bravo Throttle Quadrant.
///
/// Confirmed from multiple independent sources: BetterBravoLights (RoystonS),
/// FwlDynamicJoystickMapper Lua scripts, SPAD.neXt profiles, and
/// linux-hardware.org probe data.
pub const HONEYCOMB_BRAVO_PID: u16 = 0x1901;

/// USB Product ID for the T.Flight Rudder Pedals (TFRP).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB678 — from the-sz.com USB ID DB.
pub const TFRP_RUDDER_PEDALS_PID: u16 = 0xB678;

/// USB Product ID for the T-Rudder pedals.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB679 — from the-sz.com USB ID DB.
pub const T_RUDDER_PID: u16 = 0xB679;

pub const TFLIGHT_HOTAS_ONE_PID: u16 = 0xB68B;
/// Primary PID for T.Flight HOTAS 4 - verified via USBView artifact.
pub const TFLIGHT_HOTAS_4_PID: u16 = 0xB67B;
/// Legacy PID for T.Flight HOTAS 4 - may appear on older firmware versions.
pub const TFLIGHT_HOTAS_4_PID_LEGACY: u16 = 0xB67A;
/// USB Product ID for the T.Flight HOTAS X (PS4/PC combined unit).
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0xB108 — from open-siege/siege-studio device info.
pub const TFLIGHT_HOTAS_X_PID: u16 = 0xB108;

/// USB Product ID for the T.16000M FCS joystick.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB10A — from linux-hardware.org probe data.
pub const T16000M_JOYSTICK_PID: u16 = 0xB10A;

/// USB Product ID for the TWCS Throttle (sold standalone and as part of the
/// T.16000M FCS HOTAS combo).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB687 — from linux-hardware.org probe data.
pub const TWCS_THROTTLE_PID: u16 = 0xB687;

/// USB Product ID for the HOTAS Warthog Joystick.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x0402 — from linux-hardware.org probe data.
pub const WARTHOG_JOYSTICK_PID: u16 = 0x0402;

/// USB Product ID for the HOTAS Warthog Throttle.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x0404 — from linux-hardware.org probe data.
pub const WARTHOG_THROTTLE_PID: u16 = 0x0404;

/// USB Product ID for the HOTAS Cougar stick.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x0400 — from linux-hardware.org probe data
/// (ID usb:044f:0400, "ThrustMaster HOTAS Cougar").
pub const COUGAR_HOTAS_STICK_PID: u16 = 0x0400;

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

/// USB Vendor ID for VIRPIL Controls UAB.
///
/// Confirmed: [the-sz.com USB ID DB](https://www.the-sz.com/products/usbid/index.php?v=0x3344)
pub const VIRPIL_VENDOR_ID: u16 = 0x3344;

/// USB Product ID for the VIRPIL VPC Throttle CM3.
///
/// Source: Buzzec/virpil open-source Rust LED control library.
pub const VIRPIL_CM3_THROTTLE_PID: u16 = 0x0194;

/// USB Product ID for the VIRPIL VPC MongoosT-50CM3 (right stick).
///
/// Source: Buzzec/virpil open-source Rust LED control library.
pub const VIRPIL_MONGOOST_STICK_PID: u16 = 0x4130;

/// USB Product ID for the VIRPIL VPC Control Panel 1.
///
/// Source: Buzzec/virpil open-source Rust LED control library.
pub const VIRPIL_PANEL1_PID: u16 = 0x025B;

/// USB Product ID for the VIRPIL VPC Control Panel 2 (Right Panel).
///
/// Confirmed: VID 0x3344, PID 0x0259 — from Buzzec/virpil open-source Rust LED control library
/// (src/right_panel.rs, `const PID: u16 = 0x0259`).
pub const VIRPIL_PANEL2_PID: u16 = 0x0259;

/// USB Product ID for the VIRPIL VPC Shark Panel.
///
/// Confirmed: VID 0x3344, PID 0x825D — from Buzzec/virpil open-source Rust LED control library
/// (src/shark_panel.rs, `const PID: u16 = 0x825D`).
pub const VIRPIL_SHARK_PANEL_PID: u16 = 0x825D;

/// USB Vendor ID for CH Products.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h`
pub const CH_VENDOR_ID: u16 = 0x068E;

/// USB Product ID for the CH Pro Throttle.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_PRO_THROTTLE)
pub const CH_PRO_THROTTLE_PID: u16 = 0x00F1;

/// USB Product ID for the CH Pro Pedals.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_PRO_PEDALS)
pub const CH_PRO_PEDALS_PID: u16 = 0x00F2;

/// USB Product ID for the CH Fighterstick.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_FIGHTERSTICK)
pub const CH_FIGHTERSTICK_PID: u16 = 0x00F3;

/// USB Product ID for the CH Combat Stick.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_COMBATSTICK)
pub const CH_COMBAT_STICK_PID: u16 = 0x00F4;

/// USB Product ID for the CH Flight Sim Eclipse Yoke.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_ECLIPSE_YOKE)
pub const CH_ECLIPSE_YOKE_PID: u16 = 0x0051;

/// USB Product ID for the CH Flight Sim Yoke.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_YOKE_USB)
pub const CH_FLIGHT_YOKE_PID: u16 = 0x00FF;

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
pub const DRIVER_NOTE: &str = "Missing axes or buttons? Install the Thrustmaster driver, confirm PC full-axis mode, and on Linux use a corrected HID descriptor setup (for example hid-tflight4) when generic HID exposes limited axes.";
pub const PC_MODE_NOTE_HOTAS_4: &str = "If full-axis inputs are missing, switch HOTAS 4 to PC HID mode (hardware switch or hold Share+Option+PS while plugging in).";
pub const PC_MODE_NOTE_HOTAS_ONE: &str = "If full-axis inputs are missing, switch HOTAS One to PC mode (Xbox/PC selector and Guide button procedure) before plugging in.";
pub const DEFAULT_MAPPING_NOTE_UNKNOWN: &str =
    "Default mapping assumes full-axis mode; verify axis mode before applying.";

/// T.16000M FCS product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T16000mModel {
    /// T.16000M FCS Joystick (standalone). VID 0x044F, PID 0xB10A.
    Joystick,
    /// TWCS Throttle (standalone or part of T.16000M HOTAS combo).
    /// VID 0x044F, PID 0xB687.
    TwcsThrottle,
}

impl T16000mModel {
    pub fn name(&self) -> &'static str {
        match self {
            T16000mModel::Joystick => "T.16000M FCS Joystick",
            T16000mModel::TwcsThrottle => "T.16000M FCS TWCS Throttle",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known T.16000M device.
pub fn is_t16000m_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID
        && matches!(product_id, T16000M_JOYSTICK_PID | TWCS_THROTTLE_PID)
}

/// Returns the T.16000M model for a known PID, or `None` for unknown PIDs.
pub fn t16000m_model(product_id: u16) -> Option<T16000mModel> {
    match product_id {
        T16000M_JOYSTICK_PID => Some(T16000mModel::Joystick),
        TWCS_THROTTLE_PID => Some(T16000mModel::TwcsThrottle),
        _ => None,
    }
}

/// HOTAS Warthog product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarthogModel {
    /// HOTAS Warthog Joystick. VID 0x044F, PID 0x0402.
    Joystick,
    /// HOTAS Warthog Throttle. VID 0x044F, PID 0x0404.
    Throttle,
}

impl WarthogModel {
    pub fn name(&self) -> &'static str {
        match self {
            WarthogModel::Joystick => "HOTAS Warthog Joystick",
            WarthogModel::Throttle => "HOTAS Warthog Throttle",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a HOTAS Warthog device.
pub fn is_warthog_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID
        && matches!(product_id, WARTHOG_JOYSTICK_PID | WARTHOG_THROTTLE_PID)
}

/// Returns the Warthog model for a known PID, or `None` for unknown PIDs.
pub fn warthog_model(product_id: u16) -> Option<WarthogModel> {
    match product_id {
        WARTHOG_JOYSTICK_PID => Some(WarthogModel::Joystick),
        WARTHOG_THROTTLE_PID => Some(WarthogModel::Throttle),
        _ => None,
    }
}

/// HOTAS Cougar product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CougarHotasModel {
    /// HOTAS Cougar Stick. VID 0x044F, PID 0x0400.
    Stick,
}

impl CougarHotasModel {
    pub fn name(&self) -> &'static str {
        match self {
            CougarHotasModel::Stick => "HOTAS Cougar Stick",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a HOTAS Cougar device.
pub fn is_cougar_hotas_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID && product_id == COUGAR_HOTAS_STICK_PID
}

/// Returns the Cougar HOTAS model for a known PID, or `None` for unknown PIDs.
pub fn cougar_hotas_model(product_id: u16) -> Option<CougarHotasModel> {
    match product_id {
        COUGAR_HOTAS_STICK_PID => Some(CougarHotasModel::Stick),
        _ => None,
    }
}

/// VIRPIL Controls VPC product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirpilModel {
    /// VPC Throttle CM3. VID 0x3344, PID 0x0194.
    Cm3Throttle,
    /// VPC MongoosT-50CM3 (right stick). VID 0x3344, PID 0x4130.
    MongoostStick,
    /// VPC Control Panel 1 (left panel). VID 0x3344, PID 0x025B.
    ControlPanel1,
    /// VPC Control Panel 2 (right panel). VID 0x3344, PID 0x0259.
    ControlPanel2,
    /// VPC Shark Panel. VID 0x3344, PID 0x825D.
    SharkPanel,
}

impl VirpilModel {
    pub fn name(&self) -> &'static str {
        match self {
            VirpilModel::Cm3Throttle => "VPC Throttle CM3",
            VirpilModel::MongoostStick => "VPC MongoosT-50CM3 Stick",
            VirpilModel::ControlPanel1 => "VPC Control Panel 1",
            VirpilModel::ControlPanel2 => "VPC Control Panel 2",
            VirpilModel::SharkPanel => "VPC Shark Panel",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known VIRPIL device.
pub fn is_virpil_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == VIRPIL_VENDOR_ID
        && matches!(
            product_id,
            VIRPIL_CM3_THROTTLE_PID
                | VIRPIL_MONGOOST_STICK_PID
                | VIRPIL_PANEL1_PID
                | VIRPIL_PANEL2_PID
                | VIRPIL_SHARK_PANEL_PID
        )
}

/// Returns the VIRPIL model for a known PID, or `None` for unknown PIDs.
pub fn virpil_model(product_id: u16) -> Option<VirpilModel> {
    match product_id {
        VIRPIL_CM3_THROTTLE_PID => Some(VirpilModel::Cm3Throttle),
        VIRPIL_MONGOOST_STICK_PID => Some(VirpilModel::MongoostStick),
        VIRPIL_PANEL1_PID => Some(VirpilModel::ControlPanel1),
        VIRPIL_PANEL2_PID => Some(VirpilModel::ControlPanel2),
        VIRPIL_SHARK_PANEL_PID => Some(VirpilModel::SharkPanel),
        _ => None,
    }
}

/// CH Products device family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChModel {
    /// CH Pro Throttle. VID 0x068E, PID 0x00F1.
    ProThrottle,
    /// CH Pro Pedals. VID 0x068E, PID 0x00F2.
    ProPedals,
    /// CH Fighterstick. VID 0x068E, PID 0x00F3.
    Fighterstick,
    /// CH Combat Stick. VID 0x068E, PID 0x00F4.
    CombatStick,
    /// CH Flight Sim Eclipse Yoke. VID 0x068E, PID 0x0051.
    EclipseYoke,
    /// CH Flight Sim Yoke. VID 0x068E, PID 0x00FF.
    FlightYoke,
}

impl ChModel {
    pub fn name(&self) -> &'static str {
        match self {
            ChModel::ProThrottle => "CH Pro Throttle",
            ChModel::ProPedals => "CH Pro Pedals",
            ChModel::Fighterstick => "CH Fighterstick",
            ChModel::CombatStick => "CH Combat Stick",
            ChModel::EclipseYoke => "CH Flight Sim Eclipse Yoke",
            ChModel::FlightYoke => "CH Flight Sim Yoke",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known CH Products device.
pub fn is_ch_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == CH_VENDOR_ID
        && matches!(
            product_id,
            CH_PRO_THROTTLE_PID
                | CH_PRO_PEDALS_PID
                | CH_FIGHTERSTICK_PID
                | CH_COMBAT_STICK_PID
                | CH_ECLIPSE_YOKE_PID
                | CH_FLIGHT_YOKE_PID
        )
}

/// Returns the CH Products model for a known PID, or `None` for unknown PIDs.
pub fn ch_model(product_id: u16) -> Option<ChModel> {
    match product_id {
        CH_PRO_THROTTLE_PID => Some(ChModel::ProThrottle),
        CH_PRO_PEDALS_PID => Some(ChModel::ProPedals),
        CH_FIGHTERSTICK_PID => Some(ChModel::Fighterstick),
        CH_COMBAT_STICK_PID => Some(ChModel::CombatStick),
        CH_ECLIPSE_YOKE_PID => Some(ChModel::EclipseYoke),
        CH_FLIGHT_YOKE_PID => Some(ChModel::FlightYoke),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TFlightModel {
    HotasOne,
    Hotas4,
    /// T.Flight HOTAS X (PS4/PC). VID 0x044F, PID 0xB108.
    HotasX,
}

impl TFlightModel {
    pub fn name(&self) -> &'static str {
        match self {
            TFlightModel::HotasOne => "T.Flight HOTAS One",
            TFlightModel::Hotas4 => "T.Flight HOTAS 4",
            TFlightModel::HotasX => "T.Flight HOTAS X",
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

fn tflight_model_from_product_name(product_name: Option<&str>) -> Option<TFlightModel> {
    let name = product_name?.to_ascii_lowercase();
    if name.contains("hotas one") || name.contains("hotasone") {
        Some(TFlightModel::HotasOne)
    } else if name.contains("hotas 4") || name.contains("hotas4") {
        Some(TFlightModel::Hotas4)
    } else if name.contains("hotas x") || name.contains("hotasx") {
        Some(TFlightModel::HotasX)
    } else {
        None
    }
}

pub fn tflight_model(device_info: &HidDeviceInfo) -> Option<TFlightModel> {
    if device_info.vendor_id != THRUSTMASTER_VENDOR_ID {
        return None;
    }

    match device_info.product_id {
        TFLIGHT_HOTAS_ONE_PID => Some(TFlightModel::HotasOne),
        TFLIGHT_HOTAS_4_PID | TFLIGHT_HOTAS_4_PID_LEGACY => Some(TFlightModel::Hotas4),
        TFLIGHT_HOTAS_X_PID => Some(TFlightModel::HotasX),
        _ => tflight_model_from_product_name(device_info.product_name.as_deref()),
    }
}

/// Returns true if the HOTAS 4 was detected via the legacy PID.
///
/// This allows diagnostics/UI to note that the device may be running
/// older firmware. The legacy PID is still fully supported.
pub fn is_hotas4_legacy_pid(device_info: &HidDeviceInfo) -> bool {
    device_info.vendor_id == THRUSTMASTER_VENDOR_ID
        && device_info.product_id == TFLIGHT_HOTAS_4_PID_LEGACY
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

pub fn pc_mode_note(model: TFlightModel) -> &'static str {
    match model {
        TFlightModel::Hotas4 => PC_MODE_NOTE_HOTAS_4,
        TFlightModel::HotasOne => PC_MODE_NOTE_HOTAS_ONE,
        TFlightModel::HotasX => DEFAULT_MAPPING_NOTE_UNKNOWN,
    }
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

const VKB_GLADIATOR_CONTROL_MAP_SCHEMA: &str = "flight.device-map/1";
const VKB_STECS_CONTROL_MAP_SCHEMA: &str = "flight.device-map/1";
const VKB_GLADIATOR_NOTES: [&str; 5] = [
    "SCG map is descriptor-first; axis labels are semantic hints, not fixed firmware contracts.",
    "The mini-stick can toggle between POV mode and analog X/Y axes via center push.",
    "A1 hat mode behavior (8-way vs alternate 4-way) is profile-dependent in VKBDevCfg.",
    "Firmware may expose extra axes through additional HID interfaces/devices to stay within legacy DirectInput limits.",
    "Gladiator NXT EVO hardware has no force-feedback motor output channel.",
];
const VKB_STECS_NOTES: [&str; 3] = [
    "Button/axis labels are derived from Elite Dangerous buttonMap files.",
    "VKBDevCfg profiles can remap buttons, encoders, and virtual buttons.",
    "Virtual controller interfaces are exposed separately by firmware (VC0..VC2); host software should group by serial/physical path.",
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

const VKB_GLADIATOR_RIGHT_SCG_AXES: [AxisControl; 8] = [
    AxisControl {
        usage: AxisUsage::X,
        name: "RSCG Stick X (Roll)",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "RSCG Stick Y (Pitch)",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "RSCG Twist (Yaw)",
    },
    AxisControl {
        usage: AxisUsage::Slider0,
        name: "RSCG Base Throttle Wheel",
    },
    AxisControl {
        usage: AxisUsage::Rx,
        name: "RSCG Mini-stick X (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "RSCG Mini-stick Y (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Rz,
        name: "RSCG Analog Trigger 1 (Profile)",
    },
    AxisControl {
        usage: AxisUsage::Slider1,
        name: "RSCG Analog Trigger 2 (Profile)",
    },
];

const VKB_GLADIATOR_LEFT_SCG_AXES: [AxisControl; 8] = [
    AxisControl {
        usage: AxisUsage::X,
        name: "LSCG Stick X (Roll)",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "LSCG Stick Y (Pitch)",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "LSCG Twist (Yaw)",
    },
    AxisControl {
        usage: AxisUsage::Slider0,
        name: "LSCG Base Throttle Wheel",
    },
    AxisControl {
        usage: AxisUsage::Rx,
        name: "LSCG Mini-stick X (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "LSCG Mini-stick Y (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Rz,
        name: "LSCG Analog Trigger 1 (Profile)",
    },
    AxisControl {
        usage: AxisUsage::Slider1,
        name: "LSCG Analog Trigger 2 (Profile)",
    },
];

const VKB_GLADIATOR_BUTTONS: [ButtonControl; 0] = [];
const VKB_GLADIATOR_ENCODERS: [EncoderControl; 0] = [];

const VKB_GLADIATOR_RIGHT_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_GLADIATOR_CONTROL_MAP_SCHEMA,
    axes: &VKB_GLADIATOR_RIGHT_SCG_AXES,
    buttons: &VKB_GLADIATOR_BUTTONS,
    encoders: &VKB_GLADIATOR_ENCODERS,
    notes: &VKB_GLADIATOR_NOTES,
};

const VKB_GLADIATOR_LEFT_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_GLADIATOR_CONTROL_MAP_SCHEMA,
    axes: &VKB_GLADIATOR_LEFT_SCG_AXES,
    buttons: &VKB_GLADIATOR_BUTTONS,
    encoders: &VKB_GLADIATOR_ENCODERS,
    notes: &VKB_GLADIATOR_NOTES,
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

pub fn vkb_gladiator_control_map(variant: VkbGladiatorVariant) -> &'static DeviceControlMap {
    match variant {
        VkbGladiatorVariant::NxtEvoRight => &VKB_GLADIATOR_RIGHT_CONTROL_MAP,
        VkbGladiatorVariant::NxtEvoLeft => &VKB_GLADIATOR_LEFT_CONTROL_MAP,
    }
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

/// Per-interface metadata for VKB Gladiator multi-interface layouts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VkbGladiatorInterfaceMetadata {
    /// HID path for this interface.
    pub device_path: String,
    /// Stable physical device identifier (serial when available).
    pub physical_id: String,
    /// Zero-based interface index in sorted path order.
    pub interface_index: u8,
    /// Number of HID interfaces discovered for the physical device.
    pub interface_count: u8,
}

/// Per-interface metadata for VKB STECS virtual-controller layouts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VkbStecsInterfaceMetadata {
    /// HID path for this interface.
    pub device_path: String,
    /// Stable physical device identifier (serial when available).
    pub physical_id: String,
    /// Zero-based virtual-controller index inside the physical group.
    pub virtual_controller_index: u8,
    /// Number of HID interfaces discovered for the physical device.
    pub interface_count: u8,
}

fn vkb_path_group_key(device_path: &str) -> String {
    let mut normalized = if let Some((base, _)) = device_path.split_once("#if") {
        base.to_ascii_lowercase()
    } else {
        device_path.to_ascii_lowercase()
    };

    if let Some(mi_pos) = normalized.find("&mi_")
        && normalized
            .get(mi_pos + 4..mi_pos + 6)
            .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        normalized.replace_range(mi_pos..mi_pos + 6, "");
    }

    normalized
}

/// Build a stable physical-device id for Gladiator interfaces.
///
/// Serial number is preferred because it survives re-enumeration across ports.
/// If serial is unavailable, a normalized HID path stem is used.
pub fn vkb_gladiator_physical_id(device_info: &HidDeviceInfo) -> Option<String> {
    if !is_vkb_gladiator_device(device_info) {
        return None;
    }

    if let Some(serial) = device_info
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|serial| !serial.is_empty())
    {
        return Some(format!(
            "vkb-gladiator:{:04x}:{:04x}:{}",
            device_info.vendor_id,
            device_info.product_id,
            serial.to_ascii_lowercase()
        ));
    }

    Some(format!(
        "vkb-gladiator:path:{}",
        vkb_path_group_key(&device_info.device_path)
    ))
}

/// Compute Gladiator interface ordering metadata for a device set.
///
/// Interfaces are grouped by physical id and sorted by HID path to provide
/// deterministic indexing (`IF0`, `IF1`, ...).
pub fn vkb_gladiator_interface_metadata<'a, I>(devices: I) -> Vec<VkbGladiatorInterfaceMetadata>
where
    I: IntoIterator<Item = &'a HidDeviceInfo>,
{
    let mut groups: BTreeMap<String, Vec<&HidDeviceInfo>> = BTreeMap::new();

    for device in devices {
        let Some(physical_id) = vkb_gladiator_physical_id(device) else {
            continue;
        };
        groups.entry(physical_id).or_default().push(device);
    }

    let mut metadata = Vec::new();
    for (physical_id, mut interfaces) in groups {
        interfaces.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
        let interface_count = u8::try_from(interfaces.len()).unwrap_or(u8::MAX);

        for (index, interface) in interfaces.iter().enumerate() {
            metadata.push(VkbGladiatorInterfaceMetadata {
                device_path: interface.device_path.clone(),
                physical_id: physical_id.clone(),
                interface_index: u8::try_from(index).unwrap_or(u8::MAX),
                interface_count,
            });
        }
    }

    metadata.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
    metadata
}

/// Build a stable physical-device id for STECS interfaces.
///
/// Serial number is preferred because it survives re-enumeration across ports.
/// If serial is unavailable, a normalized HID path stem is used.
pub fn vkb_stecs_physical_id(device_info: &HidDeviceInfo) -> Option<String> {
    if !is_vkb_stecs_device(device_info) {
        return None;
    }

    if let Some(serial) = device_info
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|serial| !serial.is_empty())
    {
        return Some(format!(
            "vkb-stecs:{:04x}:{:04x}:{}",
            device_info.vendor_id,
            device_info.product_id,
            serial.to_ascii_lowercase()
        ));
    }

    Some(format!(
        "vkb-stecs:path:{}",
        vkb_path_group_key(&device_info.device_path)
    ))
}

/// Compute STECS virtual-controller ordering metadata for a device set.
///
/// Interfaces are grouped by physical id and sorted by HID path to provide
/// deterministic indexing (`VC0`, `VC1`, ...).
pub fn vkb_stecs_interface_metadata<'a, I>(devices: I) -> Vec<VkbStecsInterfaceMetadata>
where
    I: IntoIterator<Item = &'a HidDeviceInfo>,
{
    let mut groups: BTreeMap<String, Vec<&HidDeviceInfo>> = BTreeMap::new();

    for device in devices {
        let Some(physical_id) = vkb_stecs_physical_id(device) else {
            continue;
        };
        groups.entry(physical_id).or_default().push(device);
    }

    let mut metadata = Vec::new();
    for (physical_id, mut interfaces) in groups {
        interfaces.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
        let interface_count = u8::try_from(interfaces.len()).unwrap_or(u8::MAX);

        for (index, interface) in interfaces.iter().enumerate() {
            metadata.push(VkbStecsInterfaceMetadata {
                device_path: interface.device_path.clone(),
                physical_id: physical_id.clone(),
                virtual_controller_index: u8::try_from(index).unwrap_or(u8::MAX),
                interface_count,
            });
        }
    }

    metadata.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
    metadata
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

    fn tflight_device(product_id: u16) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: THRUSTMASTER_VENDOR_ID,
            product_id,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: "/dev/test-tflight".to_string(),
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_JOYSTICK,
            report_descriptor: None,
        }
    }

    fn tflight_device_with_name(product_id: u16, product_name: Option<&str>) -> HidDeviceInfo {
        let mut info = tflight_device(product_id);
        info.product_name = product_name.map(str::to_string);
        info
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
        let device_info = tflight_device(TFLIGHT_HOTAS_ONE_PID);
        assert_eq!(tflight_model(&device_info), Some(TFlightModel::HotasOne));
    }

    #[test]
    fn test_tflight_model_fallback_from_product_name() {
        let hotas4 = tflight_device_with_name(0xFFFF, Some("T.Flight HOTAS 4"));
        let hotas_one = tflight_device_with_name(0xABCD, Some("T.Flight HOTAS One"));
        let unknown = tflight_device_with_name(0xABCD, Some("Thrustmaster Warthog"));

        assert_eq!(tflight_model(&hotas4), Some(TFlightModel::Hotas4));
        assert_eq!(tflight_model(&hotas_one), Some(TFlightModel::HotasOne));
        assert_eq!(tflight_model(&unknown), None);
    }

    #[test]
    fn test_tflight_model_fallback_requires_thrustmaster_vendor() {
        let mut info = tflight_device_with_name(0xABCD, Some("T.Flight HOTAS 4"));
        info.vendor_id = LOGITECH_VENDOR_ID;
        assert_eq!(tflight_model(&info), None);
    }

    #[test]
    fn test_hotas4_primary_and_legacy_pid_detection() {
        let primary = tflight_device(TFLIGHT_HOTAS_4_PID);
        let legacy = tflight_device(TFLIGHT_HOTAS_4_PID_LEGACY);

        assert_eq!(tflight_model(&primary), Some(TFlightModel::Hotas4));
        assert_eq!(tflight_model(&legacy), Some(TFlightModel::Hotas4));
        assert!(!is_hotas4_legacy_pid(&primary));
        assert!(is_hotas4_legacy_pid(&legacy));
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
    fn test_vkb_gladiator_control_map_contents() {
        let control_map = vkb_gladiator_control_map(VkbGladiatorVariant::NxtEvoRight);
        assert_eq!(control_map.schema, "flight.device-map/1");
        assert_eq!(control_map.axes.len(), 8);
        assert!(
            control_map
                .axes
                .iter()
                .any(|axis| axis.usage == AxisUsage::Rx && axis.name.contains("Mini-stick X"))
        );
        assert!(
            control_map.axes.iter().any(
                |axis| axis.usage == AxisUsage::Slider0 && axis.name.contains("Throttle Wheel")
            )
        );
        assert!(control_map.buttons.is_empty());
        assert!(control_map.encoders.is_empty());
        assert!(
            control_map
                .notes
                .iter()
                .any(|note| note.contains("descriptor-first"))
        );

        let left_map = vkb_gladiator_control_map(VkbGladiatorVariant::NxtEvoLeft);
        assert!(
            left_map
                .axes
                .iter()
                .any(|axis| axis.name.starts_with("LSCG"))
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
    fn test_vkb_stecs_interface_metadata_groups_by_serial() {
        let mut vc0 = vkb_device(VKB_STECS_RIGHT_SPACE_STANDARD_PID);
        vc0.serial_number = Some("ABC123".to_string());
        vc0.device_path = r"\\?\hid#vid_231d&pid_013c&mi_00#7".to_string();

        let mut vc1 = vkb_device(VKB_STECS_RIGHT_SPACE_STANDARD_PID);
        vc1.serial_number = Some("ABC123".to_string());
        vc1.device_path = r"\\?\hid#vid_231d&pid_013c&mi_01#7".to_string();

        let metadata = vkb_stecs_interface_metadata([&vc1, &vc0]);
        assert_eq!(metadata.len(), 2);

        assert_eq!(metadata[0].virtual_controller_index, 0);
        assert_eq!(metadata[1].virtual_controller_index, 1);
        assert_eq!(metadata[0].interface_count, 2);
        assert_eq!(metadata[1].interface_count, 2);
        assert_eq!(metadata[0].physical_id, metadata[1].physical_id);
    }

    #[test]
    fn test_vkb_gladiator_interface_metadata_groups_by_serial() {
        let mut if0 = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        if0.serial_number = Some("SCG-ABC123".to_string());
        if0.device_path = r"\\?\hid#vid_231d&pid_0200&mi_00#7".to_string();

        let mut if1 = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        if1.serial_number = Some("SCG-ABC123".to_string());
        if1.device_path = r"\\?\hid#vid_231d&pid_0200&mi_01#7".to_string();

        let metadata = vkb_gladiator_interface_metadata([&if1, &if0]);
        assert_eq!(metadata.len(), 2);

        assert_eq!(metadata[0].interface_index, 0);
        assert_eq!(metadata[1].interface_index, 1);
        assert_eq!(metadata[0].interface_count, 2);
        assert_eq!(metadata[1].interface_count, 2);
        assert_eq!(metadata[0].physical_id, metadata[1].physical_id);
    }

    #[test]
    fn test_vkb_stecs_physical_id_falls_back_to_path_stem() {
        let mut device = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PLUS_PID);
        device.serial_number = None;
        device.device_path = "/dev/hidraw3#if1".to_string();

        assert_eq!(
            vkb_stecs_physical_id(&device),
            Some("vkb-stecs:path:/dev/hidraw3".to_string())
        );
    }

    #[test]
    fn test_vkb_gladiator_physical_id_falls_back_to_path_stem() {
        let mut device = vkb_device(VKB_GLADIATOR_NXT_EVO_LEFT_PID);
        device.serial_number = None;
        device.device_path = r"\\?\hid#vid_231d&pid_0201&mi_01#7".to_string();

        assert_eq!(
            vkb_gladiator_physical_id(&device),
            Some(r"vkb-gladiator:path:\\?\hid#vid_231d&pid_0201#7".to_string())
        );
    }

    #[test]
    fn test_warning_and_notes() {
        assert_eq!(axis_mode_warning(AxisMode::Merged), Some(AXIS_MODE_WARNING));
        assert!(axis_mode_warning(AxisMode::Separate).is_none());
        assert!(driver_note().contains("Thrustmaster"));
        assert!(pc_mode_note(TFlightModel::Hotas4).contains("Share+Option+PS"));
        assert!(pc_mode_note(TFlightModel::HotasOne).contains("Guide"));
        assert_eq!(
            default_mapping_note(AxisMode::Unknown),
            Some(DEFAULT_MAPPING_NOTE_UNKNOWN)
        );
    }
}
