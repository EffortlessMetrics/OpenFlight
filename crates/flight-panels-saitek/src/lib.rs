// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek/Logitech panel support and verify matrix logic.

pub use flight_panels_core::led;

pub mod bip;
pub mod fip;
pub mod multi_panel;
pub mod saitek;
pub mod verify_matrix;

pub use multi_panel::{
    LcdDisplay, MULTI_PANEL_INPUT_MIN_BYTES, MULTI_PANEL_OUTPUT_BYTES, MultiPanelButtonState,
    MultiPanelLedMask, MultiPanelState, encode_segment, led_bits, parse_multi_panel_input,
};
pub use saitek::{
    PanelHealthStatus, PanelInfo, PanelType, SaitekPanelWriter, VerifyStep, VerifyStepResult,
    VerifyTestResult,
};
pub use verify_matrix::{DriftAction, DriftAnalysis, MatrixTestResult, VerifyMatrix};
