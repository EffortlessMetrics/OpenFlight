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
pub mod adapter_state;
pub mod aircraft_db;
pub mod aircraft_detection;
pub mod auto_deploy;
pub mod control_injection;
pub mod export_lua;
pub mod installer;
pub mod lua_bridge;
pub mod mission_state;
pub mod mp_detection;
pub mod protocol;
pub mod socket_bridge;
pub mod tcp;

pub use adapter::{DcsAdapter, DcsAdapterConfig, DcsAdapterError};
pub use adapter_state::{
    DcsAdapterEvent, DcsAdapterState, DcsAdapterStateMachine, DcsTransitionError,
};
pub use aircraft_db::{AircraftCategory, AxesProfile, DcsAircraftInfo};
pub use aircraft_detection::{
    AircraftDetection, CockpitSeat, ModuleFidelity, detect_aircraft, detect_axes_profile,
    detect_category,
};
pub use auto_deploy::{DeployResult, deploy_export_script, find_dcs_install};
pub use control_injection::{
    DcsCommandDef, DcsControlCommand, DcsControlInjector, DcsDevice, DcsUdpSender,
};
pub use export_lua::{ExportLuaConfig, ExportLuaGenerator};
pub use flight_adapter_common::{AdapterMetrics, AdapterState};
pub use installer::{DcsInstaller, InstallResult, InstallStatus};
pub use lua_bridge::{HookAction, LuaBridgeConfig, SnippetStatus};
pub use mission_state::{MissionPhase, MissionStateMachine, MissionTelemetry};
pub use mp_detection::{MpSession, SessionType};
pub use protocol::{DcsExportEntry, DcsFlightData, DcsTelemetryPacket, ParseError};
pub use socket_bridge::{DcsMessage, ProtocolVersion, SocketBridge, SocketBridgeConfig};
