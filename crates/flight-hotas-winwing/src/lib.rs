// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! WinWing HOTAS driver for Flight Hub.
//!
//! Supports the **Orion 2 Throttle**, **Orion 2 F/A-18C Stick**,
//! **TFRP Rudder Pedals**, **F-16EX Grip**, and **SuperTaurus Dual Throttle**
//! via USB HID.
//!
//! # USB Identifiers
//!
//! | Product | VID    | PID    |
//! |---------|--------|--------|
//! | Orion 2 Throttle      | 0x4098 | 0xBE62 |
//! | Orion 2 F/A-18C Stick | 0x4098 | 0xBE63 |
//! | TFRP Rudder Pedals    | 0x4098 | 0xBE64 |
//! | F-16EX Grip           | 0x4098 | 0xBEA8 |
//! | SuperTaurus Dual Throttle | 0x4098 | 0xBD64 |
//! | UFC1 + HUD1 Panel     | 0x4098 | 0xBEDE |
//! | Skywalker Metal Rudder Pedals | 0x4098 | 0xBEF0 |
//!
//! # Quick start
//!
//! ```no_run
//! use flight_hotas_winwing::input::{parse_throttle_report, THROTTLE_REPORT_LEN};
//!
//! let raw = [0u8; THROTTLE_REPORT_LEN];
//! // raw[0] = 0x01;
//! let state = parse_throttle_report(&raw).unwrap();
//! let combined = state.axes.throttle_combined;
//! ```

pub mod detent_system;
pub mod f16ex_stick;
pub mod health;
pub mod input;
pub mod led_control;
pub mod orion2_stick;
pub mod orion2_throttle;
pub mod orion_joystick;
pub mod presets;
pub mod profiles;
pub mod protocol;
pub mod skywalker_rudder;
pub mod super_taurus;
pub mod tfrp;
pub mod ufc_panel;

/// Shared error type for WinWing simple device parsers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WinWingError {
    #[error("Report too short: need {need} bytes, got {got}")]
    ReportTooShort { need: usize, got: usize },
    #[error("Unknown report ID: {0:#04x}")]
    UnknownReportId(u8),
}

pub use detent_system::{ActiveDetent, MagneticDetent, WinwingDetentConfig, detect_detent};
pub use f16ex_stick::{
    BUTTON_COUNT as F16EX_BUTTON_COUNT, F16EX_STICK_PID, F16ExAxes, F16ExButtons, F16ExInputState,
    F16ExParseError, MIN_REPORT_BYTES as F16EX_REPORT_LEN, parse_f16ex_stick_report,
};
pub use health::{WinWingDevice, WinWingHealthMonitor, WinWingHealthStatus};
pub use input::{
    ORION2_F18_STICK_PID, ORION2_THROTTLE_PID, RUDDER_REPORT_LEN, RudderAxes, STICK_REPORT_LEN,
    StickAxes, StickButtons, StickInputState, TFRP_RUDDER_PID, THROTTLE_REPORT_LEN, ThrottleAxes,
    ThrottleButtons, ThrottleInputState, WINWING_VENDOR_ID, WinWingParseError, parse_rudder_report,
    parse_stick_report, parse_throttle_report,
};
pub use led_control::{DisplayCommand, LedCommand, build_display_report, build_led_report};
pub use orion_joystick::{
    ORION_JOYSTICK_MIN_REPORT_BYTES, ORION_JOYSTICK_PID, OrionJoystickState, URSA_MINOR_L_PID,
    parse_orion_joystick,
};
pub use orion2_stick::{
    BUTTON_COUNT as ORION2_STICK_BUTTON_COUNT, MIN_REPORT_BYTES as ORION2_STICK_REPORT_LEN,
    ORION2_STICK_PID, Orion2StickAxes, Orion2StickButtons, Orion2StickInputState,
    Orion2StickParseError, parse_orion2_stick_report,
};
pub use orion2_throttle::{
    BUTTON_COUNT as ORION2_THROTTLE_BUTTON_COUNT, ENCODER_COUNT as ORION2_THROTTLE_ENCODER_COUNT,
    MIN_REPORT_BYTES as ORION2_THROTTLE_REPORT_BYTES, ORION2_THROTTLE_MIN_REPORT_BYTES,
    Orion2ThrottleAxes, Orion2ThrottleButtons, Orion2ThrottleInputState, Orion2ThrottleParseError,
    Orion2ThrottleState, normalize_axis_16bit, normalize_throttle_16bit, parse_orion2_throttle,
    parse_orion2_throttle_report,
};
pub use presets::{orion2_stick_config, orion2_throttle_config, tfrp_rudder_config};
pub use profiles::{
    ButtonGroupDescriptor, DetentDescriptor, DeviceProfile, DisplayFieldDescriptor,
    EncoderDescriptor, HatDescriptor, a10_grip_profile, all_profiles, combat_ready_panel_profile,
    efis_panel_profile, f16ex_grip_profile, f18_grip_profile, fcu_panel_profile,
    orion2_base_profile, orion2_throttle_profile, profile_by_pid, take_off_panel_profile,
};
pub use protocol::{
    BacklightSubCommand, CommandCategory, DetentName, DetentPosition, DetentReport,
    DetentSubCommand, DeviceType, DisplaySubCommand, FeatureReportFrame, ParsedFrame,
    ProtocolError, WinwingProtocol, build_backlight_all_command, build_backlight_all_rgb_command,
    build_backlight_single_command, build_backlight_single_rgb_command, build_detent_query_command,
    build_detent_set_command, build_display_brightness_command, build_display_clear_command,
    build_display_segment_command, build_display_text_command, parse_detent_response,
    parse_feature_report,
};
pub use skywalker_rudder::{
    MIN_REPORT_BYTES as SKYWALKER_RUDDER_REPORT_LEN, SKYWALKER_RUDDER_PID, SkywalkerAxes,
    SkywalkerParseError, SkywalkerRudderInputState, parse_skywalker_rudder_report,
};
pub use super_taurus::{
    BUTTON_COUNT as SUPER_TAURUS_BUTTON_COUNT, MIN_REPORT_BYTES as SUPER_TAURUS_REPORT_LEN,
    SUPER_TAURUS_PID, SuperTaurusAxes, SuperTaurusButtons, SuperTaurusInputState,
    SuperTaurusParseError, parse_super_taurus_report,
};
pub use tfrp::{
    MIN_REPORT_BYTES as TFRP_REPORT_BYTES, TfrpAxes, TfrpInputState, TfrpParseError,
    parse_tfrp_report,
};
pub use ufc_panel::{
    HUD_BUTTON_COUNT as UFC_HUD_BUTTON_COUNT, MIN_REPORT_BYTES as UFC_PANEL_REPORT_LEN,
    TOTAL_BUTTON_COUNT as UFC_TOTAL_BUTTON_COUNT, UFC_BUTTON_COUNT, UFC_PANEL_PID, UfcButtons,
    UfcPanelInputState, UfcPanelParseError, parse_ufc_panel_report,
};

/// WinWing USB Vendor ID.
pub const WINWING_VID: u16 = 0x4098;

/// All known WinWing PIDs covered by this crate.
pub const WINWING_PIDS: &[u16] = &[
    ORION2_THROTTLE_PID,
    ORION2_F18_STICK_PID,
    TFRP_RUDDER_PID,
    F16EX_STICK_PID,
    SUPER_TAURUS_PID,
    UFC_PANEL_PID,
    SKYWALKER_RUDDER_PID,
];
