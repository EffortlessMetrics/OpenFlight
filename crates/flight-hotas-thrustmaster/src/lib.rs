// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Thrustmaster T.Flight HOTAS and T.16000M FCS driver for OpenFlight.
//!
//! This crate provides support for:
//! - T.Flight HOTAS 4 and HOTAS One controllers
//! - T.16000M FCS joystick and TWCS Throttle

pub mod detents;
pub mod health;
pub mod input;
pub mod pc_mode;
pub mod presets;
pub mod t16000m;
pub mod tfrp;
pub mod tpr;
pub mod warthog;

pub use detents::{DetentEvent, ThrottleDetentConfig, ThrottleDetentTracker};
pub use flight_hid_support::device_support::{
    AxisMode, T_RUDDER_PID, T16000M_JOYSTICK_PID, T16000mModel, TCA_QUADRANT_BOEING_ENG12_PID,
    TCA_QUADRANT_BOEING_ENG34_PID, TCA_YOKE_BOEING_PID, TFLIGHT_HOTAS_4_PID,
    TFLIGHT_HOTAS_4_PID_LEGACY, TFLIGHT_HOTAS_ONE_PID, TFLIGHT_HOTAS_X_PID, TFRP_RUDDER_PEDALS_PID,
    TFlightModel, THRUSTMASTER_VENDOR_ID, TPR_PENDULAR_RUDDER_BULK_PID, TPR_PENDULAR_RUDDER_PID,
    TWCS_THROTTLE_PID, TcaBoeingModel, WARTHOG_JOYSTICK_PID, WARTHOG_THROTTLE_PID, WarthogModel,
    is_hotas4_legacy_pid, is_t16000m_device, is_tca_boeing_device, is_tflight_device,
    is_warthog_device, t16000m_model, tca_boeing_model, tflight_model, warthog_model,
};
pub use health::{TFlightHealthMonitor, TFlightHealthStatus};
pub use input::{
    TFlightAxes, TFlightButtons, TFlightInputHandler, TFlightInputState, TFlightParseError,
    TFlightYawPolicy, TFlightYawResolution, TFlightYawSource,
};
pub use pc_mode::{
    PC_MODE_HANDSHAKE_INSTRUCTIONS, PC_MODE_MIN_REPORT_LEN, PcModeDetector, PcModeStatus,
};
pub use presets::{RecommendedAxisConfig, recommended_axis_config};
pub use t16000m::{
    T16000mAxes, T16000mButtons, T16000mInputState, T16000mParseError, TwcsAxes, TwcsButtons,
    TwcsInputState, parse_t16000m_report, parse_twcs_report,
};
pub use tfrp::{
    TFRP_MIN_REPORT_BYTES, TfrpAxes, TfrpInputState, TfrpParseError, parse_tfrp_report,
};
pub use tpr::{TPR_MIN_REPORT_BYTES, TprAxes, TprInputState, TprParseError, parse_tpr_report};
pub use warthog::{
    WARTHOG_STICK_MIN_REPORT_BYTES, WARTHOG_THROTTLE_MIN_REPORT_BYTES, WarthogHat,
    WarthogParseError, WarthogStickAxes, WarthogStickButtons, WarthogStickInputState,
    WarthogThrottleAxes, WarthogThrottleButtons, WarthogThrottleInputState, parse_warthog_stick,
    parse_warthog_throttle,
};
