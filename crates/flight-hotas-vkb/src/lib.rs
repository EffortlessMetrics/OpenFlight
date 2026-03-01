// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! VKB device support helpers for OpenFlight.
//!
//! This crate provides support for VKB STECS throttle variants, VKB Gladiator
//! NXT EVO sticks, and VKB virtual-interface metadata utilities.

pub mod axis_mapping;
pub mod configuration;
pub mod health;
pub mod input;
pub mod profiles;
pub mod protocol;
pub mod stecs_modern;

pub use flight_hid_support::device_support::{
    VKB_GLADIATOR_NXT_EVO_LEFT_PID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID, VKB_STECS_LEFT_SPACE_MINI_PID,
    VKB_STECS_LEFT_SPACE_MINI_PLUS_PID, VKB_STECS_LEFT_SPACE_STANDARD_PID,
    VKB_STECS_MODERN_THROTTLE_MAX_PID, VKB_STECS_MODERN_THROTTLE_MINI_PID,
    VKB_STECS_RIGHT_SPACE_MINI_PID, VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID,
    VKB_STECS_RIGHT_SPACE_STANDARD_PID, VKB_VENDOR_ID, VkbGladiatorInterfaceMetadata,
    VkbGladiatorVariant, VkbStecsInterfaceMetadata, VkbStecsVariant, is_vkb_gladiator_device,
    is_vkb_stecs_device, vkb_gladiator_control_map, vkb_gladiator_interface_metadata,
    vkb_gladiator_physical_id, vkb_gladiator_variant, vkb_stecs_control_map,
    vkb_stecs_interface_metadata, vkb_stecs_physical_id, vkb_stecs_variant,
};

pub use axis_mapping::{
    AxisMapEntry, AxisResolveError, GLADIATOR_AXIS_MAP, GUNFIGHTER_AXIS_MAP, VkbAxis,
    axis_map_for_family, resolve_axis, resolve_axis_by_name,
};
pub use configuration::{
    ConfigParseError, ConfigProfile, CurveType, VKB_CONFIG_MIN_REPORT_BYTES, VKB_CONFIG_REPORT_ID,
    VKB_MAX_CONFIG_AXES, VkbConfig, read_config_from_report,
};
pub use health::{StecsHealthMonitor, StecsHealthStatus};
pub use input::{
    GLADIATOR_MAX_BUTTONS, GLADIATOR_MAX_HATS, GladiatorAxes, GladiatorInputHandler,
    GladiatorInputState, GladiatorParseError, HatDirection, STECS_BUTTONS_PER_VIRTUAL_CONTROLLER,
    STECS_MAX_BUTTONS, STECS_MAX_VIRTUAL_CONTROLLERS, StecsAxes, StecsInputAggregator,
    StecsInputHandler, StecsInputState, StecsInterfaceState, StecsParseError,
};
pub use profiles::{
    AxisMapping, AxisNormMode, ButtonKind, ButtonMapping, HatKind, HatMapping, VkbDeviceProfile,
    all_profiles, gladiator_nxt_evo_profile, gunfighter_mcg_profile, profile_for_pid,
    sem_thq_profile, stecs_throttle_profile, t_rudder_profile,
};
pub use protocol::{
    GLADIATOR_NXT_EVO_SHIFT, GUNFIGHTER_MAX_BUTTONS, GUNFIGHTER_MAX_HATS, GUNFIGHTER_MCG_SHIFT,
    GunfighterAxes, GunfighterInputHandler, GunfighterInputState, GunfighterParseError,
    GunfighterVariant, SemThqAxes, SemThqInputHandler, SemThqInputState, SemThqParseError,
    StickAxes, StickState, VKB_AXIS_16BIT, VKB_JOYSTICK_STANDARD_LAYOUT, VKB_LED_REPORT_ID,
    VKB_SEM_THQ_LAYOUT, VkbAxisResolution, VkbDeviceFamily, VkbJoystickReportLayout, VkbLedColor,
    VkbLedIndex, VkbProtocol, VkbProtocolParseError, VkbShiftMode, build_led_command,
    is_vkb_joystick, report_layout_for_family, vkb_device_family,
};
pub use stecs_modern::{
    StecsMtParseError, StecsMtVariant, VKC_STECS_MT_MAX_BUTTONS, VKC_STECS_MT_MIN_REPORT_BYTES,
    VkcStecsMtAxes, VkcStecsMtButtons, VkcStecsMtInputState, parse_stecs_mt_report,
};
