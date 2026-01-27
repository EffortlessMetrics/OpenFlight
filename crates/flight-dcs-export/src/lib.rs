// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS Export Integration
//!
//! Provides DCS World integration through user-installed Export.lua scripts.
//! Implements MP-safe telemetry extraction with clear MP/SP feature boundaries.
//!
//! # Architecture
//!
//! - User installs Export.lua in DCS Saved Games directory
//! - Export.lua opens socket connection to Flight Hub
//! - Adapter validates MP session and blocks restricted features
//! - Telemetry published to normalized bus at 30-60Hz
//!
//! # MP Integrity
//!
//! - Export.lua declares MP-safe vs blocked features
//! - Adapter refuses blocked features in MP sessions
//! - Clear UI messaging when features are unavailable

pub mod adapter;
pub mod export_lua;
pub mod installer;
pub mod mp_detection;
pub mod socket_bridge;

pub use adapter::{DcsAdapter, DcsAdapterConfig, DcsAdapterError};
pub use flight_adapter_common::{AdapterMetrics, AdapterState};
pub use export_lua::{ExportLuaConfig, ExportLuaGenerator};
pub use installer::{DcsInstaller, InstallResult, InstallStatus};
pub use mp_detection::{MpSession, SessionType};
pub use socket_bridge::{ProtocolVersion, SocketBridge, SocketBridgeConfig};
