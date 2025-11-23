// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Transport layer for SimConnect communication
//!
//! Provides transport abstractions for SimConnect data exchange.

use thiserror::Error;

/// Transport layer error types
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Timeout error: {0}")]
    Timeout(String),
}
