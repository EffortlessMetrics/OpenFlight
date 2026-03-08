// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Unified rudder pedals input model for OpenFlight.
//!
//! Every supported pedal device (Thrustmaster TFRP/TPR, MFG Crosswind,
//! Slaw RX Viper, VKB T-Rudder, Saitek/Logitech Pro Flight) is parsed
//! into a shared [`PedalsAxes`] struct with three normalised `f32` axes:
//!
//! - **`rudder`** — combined yaw deflection, `0.0` = full-left, `1.0` = full-right.
//! - **`left_toe_brake`** — independent left pedal, `0.0` = released, `1.0` = fully pressed.
//! - **`right_toe_brake`** — independent right pedal, `0.0` = released, `1.0` = fully pressed.
//!
//! Vendor-specific parsers normalise byte order, resolution, and axis
//! inversion into this common representation.  [`Calibration`] allows
//! per-device min/max overrides for worn potentiometers or Hall sensors.

pub mod calibration;
pub mod mfg_crosswind;
pub mod saitek_pro_flight;
pub mod slaw_rx_viper;
pub mod thrustmaster_tfrp;
pub mod thrustmaster_tpr;
pub mod vkb_t_rudder;

pub use calibration::{AxisCalibration, Calibration};
pub use mfg_crosswind::{
    MFG_CROSSWIND_MIN_REPORT_BYTES, MfgCrosswdParseError, parse_mfg_crosswind_report,
};
pub use saitek_pro_flight::{
    SAITEK_PEDALS_MIN_REPORT_BYTES, SaitekPedalsParseError, parse_saitek_pedals_report,
};
pub use slaw_rx_viper::{
    SLAW_VIPER_MIN_REPORT_BYTES, SlawViperParseError, parse_slaw_viper_report,
};
pub use thrustmaster_tfrp::{TFRP_MIN_REPORT_BYTES, TfrpParseError, parse_tfrp_report};
pub use thrustmaster_tpr::{TPR_MIN_REPORT_BYTES, TprParseError, parse_tpr_report};
pub use vkb_t_rudder::{
    VKB_TRUDDER_MIN_REPORT_BYTES, VkbTRudderParseError, parse_vkb_trudder_report,
};

// ─── Unified types ───────────────────────────────────────────────────────────

/// Normalised axes shared by all pedal devices.
///
/// All values are in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PedalsAxes {
    /// Combined rudder yaw.  0.0 = full-left, 0.5 ≈ centre, 1.0 = full-right.
    pub rudder: f32,
    /// Left toe brake.  0.0 = released, 1.0 = fully pressed.
    pub left_toe_brake: f32,
    /// Right toe brake.  0.0 = released, 1.0 = fully pressed.
    pub right_toe_brake: f32,
}

/// Full parsed input state from any pedal device.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PedalsInputState {
    /// Which vendor/model produced this state.
    pub vendor: PedalVendor,
    /// Normalised axis values.
    pub axes: PedalsAxes,
}

/// Identifies the pedal hardware family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PedalVendor {
    #[default]
    Unknown,
    ThrustmasterTfrp,
    ThrustmasterTpr,
    MfgCrosswind,
    SlawRxViper,
    VkbTRudder,
    SaitekProFlight,
}

// ─── USB identifiers ─────────────────────────────────────────────────────────

/// Thrustmaster VID.
pub const THRUSTMASTER_VID: u16 = 0x044F;
/// Thrustmaster TFRP PID.
pub const TFRP_PID: u16 = 0xB678;
/// Thrustmaster T-Rudder PID.
pub const T_RUDDER_PID: u16 = 0xB679;
/// Thrustmaster TPR (standard) PID.
pub const TPR_PID: u16 = 0xB68F;
/// Thrustmaster TPR (bulk) PID.
pub const TPR_BULK_PID: u16 = 0xB68E;

/// MFG vendor ID.
pub const MFG_VID: u16 = 0x1551;
/// MFG Crosswind V3 PID (community estimate).
pub const MFG_CROSSWIND_V3_PID: u16 = 0x0003;

/// STMicroelectronics VID (used by Slaw Device STM32 firmware).
pub const SLAW_VID: u16 = 0x0483;
/// Slaw RX Viper PID (community estimate).
pub const SLAW_RX_VIPER_PID: u16 = 0x5746;

/// VKB VID.
pub const VKB_VID: u16 = 0x231D;
/// VKB T-Rudder Mk.IV PID.
pub const VKB_T_RUDDER_MK4_PID: u16 = 0x0126;

/// Saitek / Logitech VID.
pub const SAITEK_VID: u16 = 0x06A3;
/// Saitek Pro Flight Rudder Pedals PID.
pub const SAITEK_PRO_FLIGHT_PEDALS_PID: u16 = 0x0763;
