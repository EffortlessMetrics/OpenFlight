// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Error types for Flight Hub

use thiserror::Error;

/// Flight Hub error types
#[derive(Error, Debug)]
pub enum FlightError {
    #[error("Profile validation error: {0}")]
    ProfileValidation(String),

    #[error("Rules validation error: {0}")]
    RulesValidation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Writer error: {0}")]
    Writer(String),

    #[error("Aircraft auto-switch error: {0}")]
    AutoSwitch(String),

    #[error("Hardware error: {0}")]
    Hardware(String),

    #[error("Profile error: {0}")]
    Profile(#[from] flight_profile::ProfileError),

    #[error("Process detection error: {0}")]
    ProcessDetection(#[from] flight_process_detection::ProcessDetectionError),

    #[error("Writers error: {0}")]
    Writers(#[from] flight_writers::CurveConflictError),

    #[error("Security error: {0}")]
    Security(#[from] flight_security::SecurityError),

    #[error("Blackbox error: {0}")]
    Blackbox(#[from] flight_blackbox::BlackboxError),

    #[error("Watchdog error: {0}")]
    Watchdog(#[from] flight_watchdog::WatchdogError),

    #[error("Rules error: {0}")]
    Rules(#[from] flight_rules::RulesError),
}

/// Result type alias for Flight Hub operations
pub type Result<T> = std::result::Result<T, FlightError>;
