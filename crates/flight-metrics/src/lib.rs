// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metrics registry and collection utilities.

pub mod collector;
pub mod common;
pub mod registry;
pub mod types;

pub use collector::MetricsCollector;
pub use registry::MetricsRegistry;
pub use types::{HistogramSummary, Metric};
