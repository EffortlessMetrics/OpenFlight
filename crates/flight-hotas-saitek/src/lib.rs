// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek/Logitech HOTAS driver for OpenFlight.
//!
//! This crate provides support for X52, X52 Pro, X55, and X56 HOTAS controllers.
//!
//! # Architecture
//!
//! - **Input path** (axes/buttons): Standard HID - implemented in RT-aware code
//! - **Output path** (MFD/LED/RGB): Vendor-specific - experimental, feature-gated
//!
//! # Feature Flags
//!
//! Output protocols are unverified and require opt-in:
//!
//! - `x52-mfd-experimental`: X52 Pro MFD display support
//! - `x52-led-experimental`: X52/X52 Pro LED control
//! - `x56-rgb-experimental`: X56 RGB lighting control
//!
//! See `docs/reference/hotas-claims.md` for protocol verification status.

pub mod health;
pub mod input;
pub mod policy;
pub mod traits;

#[cfg(feature = "x52-mfd-experimental")]
pub mod mfd;

#[cfg(feature = "x52-led-experimental")]
pub mod led;

#[cfg(feature = "x56-rgb-experimental")]
pub mod rgb;

pub use flight_hid_support::saitek_hotas::{SaitekHotasFamily, SaitekHotasType, is_saitek_hotas};
pub use health::HotasHealthMonitor;
pub use input::HotasInputHandler;
pub use policy::allow_device_io;
pub use traits::{HotasError, HotasResult};
