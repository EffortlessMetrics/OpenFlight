// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metrics registry and collection utilities.

pub mod collector;
pub mod common;
pub mod dashboard;
pub mod export;
pub mod exporter;
pub mod histogram;
pub mod prometheus_export;
pub mod registry;
pub mod types;

pub use collector::{AtomicCollector, MetricsCollector};
pub use dashboard::{
    AxisMetrics, BusMetrics, DashboardSnapshot, DeviceMetrics, FfbMetrics, MetricsDashboard,
    RtMetrics, SimMetrics, WatchdogMetrics,
};
pub use exporter::{LabeledMetric, MetricValue, MetricsExporter};
pub use histogram::{FixedBucketHistogram, latency_buckets, size_buckets};
pub use registry::{MetricFamily, MetricFamilyType, MetricsRegistry};
pub use types::{HistogramSummary, Metric};
