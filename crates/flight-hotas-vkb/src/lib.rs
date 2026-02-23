// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! VKB STECS HOTAS driver for OpenFlight.
//!
//! This crate provides support for VKB STECS throttle variants and virtual
//! controller report aggregation.

pub mod health;
pub mod input;

pub use flight_hid_support::device_support::{
    VKB_STECS_LEFT_SPACE_MINI_PID, VKB_STECS_LEFT_SPACE_MINI_PLUS_PID,
    VKB_STECS_LEFT_SPACE_STANDARD_PID, VKB_STECS_RIGHT_SPACE_MINI_PID,
    VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID, VKB_STECS_RIGHT_SPACE_STANDARD_PID, VKB_VENDOR_ID,
    VkbStecsInterfaceMetadata, VkbStecsVariant, is_vkb_stecs_device, vkb_stecs_interface_metadata,
    vkb_stecs_physical_id, vkb_stecs_variant,
};

pub use health::{StecsHealthMonitor, StecsHealthStatus};
pub use input::{
    STECS_BUTTONS_PER_VIRTUAL_CONTROLLER, STECS_MAX_BUTTONS, STECS_MAX_VIRTUAL_CONTROLLERS,
    StecsAxes, StecsInputAggregator, StecsInputHandler, StecsInputState, StecsInterfaceState,
    StecsParseError,
};
