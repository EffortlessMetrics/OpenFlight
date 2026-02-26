// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! VKB device support helpers for OpenFlight.
//!
//! This crate provides support for VKB STECS throttle variants, VKB Gladiator
//! NXT EVO sticks, and VKB virtual-interface metadata utilities.

pub mod health;
pub mod input;
pub mod stecs_modern;

pub use flight_hid_support::device_support::{
    VKB_GLADIATOR_NXT_EVO_LEFT_PID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID, VKB_STECS_LEFT_SPACE_MINI_PID,
    VKB_STECS_LEFT_SPACE_MINI_PLUS_PID, VKB_STECS_LEFT_SPACE_STANDARD_PID,
    VKB_STECS_RIGHT_SPACE_MINI_PID, VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID,
    VKB_STECS_RIGHT_SPACE_STANDARD_PID, VKB_STECS_MODERN_THROTTLE_MINI_PID,
    VKB_STECS_MODERN_THROTTLE_MAX_PID, VKB_VENDOR_ID, VkbGladiatorInterfaceMetadata,
    VkbGladiatorVariant, VkbStecsInterfaceMetadata, VkbStecsVariant, is_vkb_gladiator_device,
    is_vkb_stecs_device, vkb_gladiator_control_map, vkb_gladiator_interface_metadata,
    vkb_gladiator_physical_id, vkb_gladiator_variant, vkb_stecs_control_map,
    vkb_stecs_interface_metadata, vkb_stecs_physical_id, vkb_stecs_variant,
};

pub use health::{StecsHealthMonitor, StecsHealthStatus};
pub use input::{
    GLADIATOR_MAX_BUTTONS, GLADIATOR_MAX_HATS, GladiatorAxes, GladiatorInputHandler,
    GladiatorInputState, GladiatorParseError, HatDirection, STECS_BUTTONS_PER_VIRTUAL_CONTROLLER,
    STECS_MAX_BUTTONS, STECS_MAX_VIRTUAL_CONTROLLERS, StecsAxes, StecsInputAggregator,
    StecsInputHandler, StecsInputState, StecsInterfaceState, StecsParseError,
};
pub use stecs_modern::{
    VKC_STECS_MT_MAX_BUTTONS, VKC_STECS_MT_MIN_REPORT_BYTES, VkcStecsMtAxes, VkcStecsMtButtons,
    VkcStecsMtInputState, StecsMtParseError, StecsMtVariant, parse_stecs_mt_report,
};
