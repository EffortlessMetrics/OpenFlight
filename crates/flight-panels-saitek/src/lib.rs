// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek/Logitech panel support and verify matrix logic.

pub use flight_panels_core::led;

pub mod bip;
pub mod fip;
pub mod multi_panel;
pub mod radio_panel;
pub mod saitek;
pub mod switch_panel;
pub mod verify_matrix;

pub use fip::{FipPageManager, FipProtocol, FipScrollWheel, FipSoftKeys};
pub use multi_panel::{
    LcdDisplay, MULTI_PANEL_INPUT_MIN_BYTES, MULTI_PANEL_OUTPUT_BYTES, MULTI_PANEL_PID,
    MULTI_PANEL_VID, ModeStateMachine, MultiPanelButtonState, MultiPanelLedMask, MultiPanelMode,
    MultiPanelProtocol, MultiPanelState, encode_segment, led_bits, parse_multi_panel_input,
};
pub use radio_panel::{
    EncoderDelta, RADIO_PANEL_INPUT_MIN_BYTES, RADIO_PANEL_OUTPUT_BYTES, RadioDisplay, RadioMode,
    RadioPanelButtonState, RadioPanelProtocol, RadioPanelState, parse_radio_panel_input,
};
pub use saitek::{
    PanelHealthStatus, PanelInfo, PanelType, SaitekPanelWriter, VerifyStep, VerifyStepResult,
    VerifyTestResult,
};
pub use switch_panel::{
    GearLedColor, MagnetoPosition, SWITCH_PANEL_INPUT_MIN_BYTES, SWITCH_PANEL_OUTPUT_BYTES,
    SwitchDebounce, SwitchPanelGearLeds, SwitchPanelProtocol, SwitchPanelState,
    SwitchPanelSwitchState, gear_led_bits, parse_switch_panel_input,
};
pub use verify_matrix::{DriftAction, DriftAnalysis, MatrixTestResult, VerifyMatrix};
