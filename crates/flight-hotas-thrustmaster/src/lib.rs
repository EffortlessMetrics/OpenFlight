// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Thrustmaster T.Flight HOTAS driver for OpenFlight.
//!
//! This crate provides support for T.Flight HOTAS 4 and HOTAS One controllers.
//!
//! # Architecture
//!
//! - **Input path** (axes/buttons): Standard HID - handles both Merged and Separate axis modes
//! - **Noise filtering**: Specialized handling for B104 potentiometer jitter
//!
//! # Axis Modes
//!
//! The T.Flight HOTAS 4 can operate in two axis modes:
//!
//! - **Merged mode**: Twist and rocker are combined into a single RZ axis (8 bytes)
//! - **Separate mode**: Twist and rocker are separate axes (9 bytes)
//!
//! Mode can be detected from the HID report descriptor or report size.
//!
//! # Hardware Notes
//!
//! - Uses B104 (100kΩ linear) potentiometers prone to jitter
//! - PC mode (Green LED) required for Windows/Linux operation
//! - "Secret handshake" (Share+Option+PS at plugin) forces PC mode

pub mod health;
pub mod input;
pub mod presets;

pub use flight_hid_support::device_support::{
    AxisMode, TFLIGHT_HOTAS_4_PID, TFLIGHT_HOTAS_4_PID_LEGACY, TFLIGHT_HOTAS_ONE_PID, TFlightModel,
    THRUSTMASTER_VENDOR_ID, is_hotas4_legacy_pid, is_tflight_device, tflight_model,
};
pub use health::{TFlightHealthMonitor, TFlightHealthStatus};
pub use input::{TFlightAxes, TFlightButtons, TFlightInputHandler, TFlightInputState};
pub use presets::{RecommendedAxisConfig, recommended_axis_config};
