// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Common adapter types shared across simulator adapters.

pub mod config;
pub mod error;
pub mod metrics;
pub mod reconnection;
pub mod state;

pub use config::AdapterConfig;
pub use error::AdapterError;
pub use metrics::AdapterMetrics;
pub use reconnection::ReconnectionStrategy;
pub use state::AdapterState;
