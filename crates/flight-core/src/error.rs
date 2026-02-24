// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Error types for Flight Hub

use thiserror::Error;

/// Flight Hub error types
#[derive(Error, Debug)]
pub enum FlightError {
    /// Rules validation failed (e.g. invalid DSL syntax or constraint violation)
    #[error("Rules validation error: {0}")]
    RulesValidation(String),

    /// I/O operation failed
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failed
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid configuration value or missing required field
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Sim variable writer operation failed
    #[error("Writer error: {0}")]
    Writer(String),

    /// Hardware communication error
    #[error("Hardware error: {0}")]
    Hardware(String),

    /// Profile schema validation or merge error
    #[error("Profile error: {0}")]
    Profile(#[from] flight_profile::ProfileError),

    /// Process detection subsystem error
    #[error("Process detection error: {0}")]
    ProcessDetection(#[from] flight_process_detection::ProcessDetectionError),

    /// Curve conflict detection or resolution error
    #[error("Writers error: {0}")]
    Writers(#[from] flight_writers::CurveConflictError),

    /// Security capability enforcement error
    #[error("Security error: {0}")]
    Security(#[from] flight_security::SecurityError),

    /// Blackbox recording or playback error
    #[error("Blackbox error: {0}")]
    Blackbox(#[from] flight_blackbox::BlackboxError),

    /// Watchdog timer or health-check error
    #[error("Watchdog error: {0}")]
    Watchdog(#[from] flight_watchdog::WatchdogError),

    /// Rules engine evaluation error
    #[error("Rules error: {0}")]
    Rules(#[from] flight_rules::RulesError),

    /// Aircraft session or auto-switch error
    #[error("Session error: {0}")]
    Session(#[from] flight_session::SessionError),
}

/// Result type alias for Flight Hub operations
pub type Result<T> = std::result::Result<T, FlightError>;
