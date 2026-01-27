// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek/Logitech panel support and verify matrix logic.

pub use flight_panels_core::led;

pub mod saitek;
pub mod verify_matrix;

pub use saitek::{
    PanelHealthStatus, PanelInfo, PanelType, SaitekPanelWriter, VerifyStep, VerifyStepResult,
    VerifyTestResult,
};
pub use verify_matrix::{DriftAction, DriftAnalysis, MatrixTestResult, VerifyMatrix};
