// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Device operation metrics helpers.

use flight_metrics::MetricsRegistry;
use flight_metrics::common::DeviceMetricNames;
use std::time::Instant;

/// Aggregated device operation metrics.
#[derive(Debug, Clone, Default)]
pub struct DeviceMetrics {
    pub operations_total: u64,
    pub operations_failed: u64,
    pub bytes_transferred: u64,
    pub last_operation_time: Option<Instant>,
    pub last_operation_latency_ms: Option<f64>,
}

impl DeviceMetrics {
    /// Record one operation locally.
    pub fn record_operation(&mut self, bytes_transferred: u64, latency_ms: f64, success: bool) {
        self.operations_total += 1;
        if !success {
            self.operations_failed += 1;
        }
        self.bytes_transferred += bytes_transferred;
        self.last_operation_time = Some(Instant::now());
        if latency_ms.is_finite() && latency_ms >= 0.0 {
            self.last_operation_latency_ms = Some(latency_ms);
        }
    }

    /// Record one operation and mirror it into the shared metrics registry.
    pub fn record_operation_with_registry(
        &mut self,
        registry: &MetricsRegistry,
        names: DeviceMetricNames,
        bytes_transferred: u64,
        latency_ms: f64,
        success: bool,
    ) {
        self.record_operation(bytes_transferred, latency_ms, success);
        registry.inc_counter(names.operations_total, 1);
        if !success {
            registry.inc_counter(names.errors_total, 1);
        }
        registry.observe(names.operation_latency_ms, latency_ms);
    }

    /// Failure rate in percentage.
    pub fn error_rate_percent(&self) -> f64 {
        if self.operations_total == 0 {
            return 0.0;
        }
        (self.operations_failed as f64 / self.operations_total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceMetrics;
    use flight_metrics::Metric;
    use flight_metrics::MetricsRegistry;
    use flight_metrics::common::DEVICE_METRICS_SHARED;

    #[test]
    fn test_record_operation_updates_totals() {
        let mut metrics = DeviceMetrics::default();
        metrics.record_operation(64, 1.5, true);
        metrics.record_operation(32, 2.0, false);

        assert_eq!(metrics.operations_total, 2);
        assert_eq!(metrics.operations_failed, 1);
        assert_eq!(metrics.bytes_transferred, 96);
        assert_eq!(metrics.last_operation_latency_ms, Some(2.0));
    }

    #[test]
    fn test_record_operation_with_registry() {
        let mut metrics = DeviceMetrics::default();
        let registry = MetricsRegistry::new();
        metrics.record_operation_with_registry(&registry, DEVICE_METRICS_SHARED, 128, 0.9, true);

        let snapshot = registry.snapshot();
        let has_counter = snapshot.iter().any(|metric| match metric {
            Metric::Counter { name, value } => {
                name == DEVICE_METRICS_SHARED.operations_total && *value == 1
            }
            _ => false,
        });
        let has_histogram = snapshot.iter().any(|metric| match metric {
            Metric::Histogram { name, .. } => name == DEVICE_METRICS_SHARED.operation_latency_ms,
            _ => false,
        });

        assert!(has_counter);
        assert!(has_histogram);
    }
}
