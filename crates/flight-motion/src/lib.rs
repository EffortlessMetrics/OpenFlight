// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! `flight-motion` — 6DOF motion platform support for OpenFlight.
//!
//! This crate implements the motion platform pipeline:
//!
//! ```text
//! BusSnapshot (kinematics, angular rates)
//!        │
//!        ▼
//!   MotionMapper
//!   ├── Translational: g_longitudinal / g_lateral / g_force → high-pass washout
//!   └── Angular: bank / pitch (low-pass) + yaw rate (high-pass)
//!        │
//!        ▼
//!   MotionFrame (-1.0 to +1.0 per DoF)
//!        │
//!        ▼
//!   SimToolsUdpOutput → motion software (SimTools, SFX-100, etc.)
//! ```
//!
//! ## Quick start
//!
//! ```
//! use flight_motion::{MotionConfig, MotionMapper, MotionFrame};
//! use flight_bus::BusSnapshot;
//!
//! let config = MotionConfig::default();
//! let mut mapper = MotionMapper::new(config, 1.0 / 60.0);  // 60 Hz
//!
//! let snapshot = BusSnapshot::default();
//! let frame: MotionFrame = mapper.process(&snapshot);
//! println!("{}", frame.to_simtools_string());
//! ```
//!
//! ## Architecture Decision (ADR-001 alignment)
//!
//! The mapper is designed to be called from the non-RT thread, receiving
//! `BusSnapshot` values published by the RT spine. The 250Hz RT core is not
//! required — the mapper works at any update rate (typically 30–60 Hz for
//! motion platforms).

pub mod config;
pub mod frame;
pub mod mapper;
pub mod output;
pub mod washout;

pub use config::{DoFConfig, MotionConfig, WashoutConfig};
pub use frame::MotionFrame;
pub use mapper::MotionMapper;
pub use output::{OutputError, SimToolsConfig, SimToolsUdpOutput};
pub use washout::{HighPassFilter, LowPassFilter, WashoutFilter};

use thiserror::Error;

/// Errors produced by the motion platform subsystem.
#[derive(Debug, Error)]
pub enum MotionError {
    #[error("Output error: {0}")]
    Output(#[from] OutputError),
    #[error("Invalid configuration: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, MotionError>;
