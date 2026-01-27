// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metric data types.

/// Metric snapshots collected from registries.
#[derive(Debug, Clone, PartialEq)]
pub enum Metric {
    /// Counter metric (monotonic).
    Counter { name: String, value: u64 },
    /// Gauge metric (current value).
    Gauge { name: String, value: f64 },
    /// Histogram summary.
    Histogram { name: String, summary: HistogramSummary },
}

/// Summary statistics for histogram metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct HistogramSummary {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}
