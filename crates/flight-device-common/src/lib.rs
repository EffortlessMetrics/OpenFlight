// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared device-layer primitives used across OpenFlight crates.

pub mod device;
pub mod health;
pub mod manager;
pub mod metrics;

pub use device::DeviceId;
pub use health::DeviceHealth;
pub use manager::{DeviceManager, IdentifiedDevice};
pub use metrics::DeviceMetrics;
