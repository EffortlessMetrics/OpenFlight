// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS-BIOS binary protocol support for cockpit builders.
//!
//! This crate implements the DCS-BIOS protocol used by cockpit builders to
//! interface with DCS World's clickable cockpit controls through a serial/UDP
//! binary protocol.
//!
//! # Modules
//!
//! - [`protocol`] — Frame parsing and sync detection
//! - [`controls`] — Control type definitions and module loading
//! - [`state`] — Cockpit state memory map with change detection
//! - [`commands`] — Import protocol command builder
//! - [`modules`] — Pre-built aircraft module definitions

pub mod commands;
pub mod controls;
pub mod modules;
pub mod protocol;
pub mod state;

pub use commands::DcsBiosCommand;
pub use controls::{DcsBiosControl, DcsBiosModule};
pub use protocol::{DcsBiosUpdate, ParseError, parse_frame};
pub use state::DcsBiosState;
