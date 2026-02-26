// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! CH Products device support helpers for OpenFlight.
//!
//! CH Products devices (Fighterstick, Combat Stick, Pro Throttle, Pro Pedals,
//! Eclipse Yoke, Flight Yoke) use standard OS-mediated HID, so no raw byte
//! parser is needed — the OS HID stack delivers axis and button data directly.
//!
//! This crate provides:
//! - Recommended axis presets per device model ([`presets`])
//! - Device health/connectivity status types ([`health`])

pub mod health;
pub mod presets;

pub use flight_hid_support::device_support::{
    CH_COMBAT_STICK_PID, CH_ECLIPSE_YOKE_PID, CH_FIGHTERSTICK_PID, CH_FLIGHT_YOKE_PID,
    CH_PRO_PEDALS_PID, CH_PRO_THROTTLE_PID, CH_VENDOR_ID, ChModel, ch_model, is_ch_device,
};

pub use health::{ChHealthMonitor, ChHealthStatus};
pub use presets::{ChAxisPreset, recommended_preset};
