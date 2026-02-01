// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Thrustmaster Cougar MFD panel support.

pub use flight_panels_core::led;

pub mod cougar;

pub use cougar::{
    CougarMfdHealthStatus, CougarMfdType, CougarMfdWriter, CougarVerifyStep,
    CougarVerifyStepResult, CougarVerifyTestResult, MfdInfo, MfdLedState,
};
