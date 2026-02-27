// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Prometheus-compatible metrics export.

use std::collections::BTreeMap;
use std::fmt;

/// A Prometheus-compatible metric.
#[derive(Debug, Clone)]
pub struct PrometheusMetric {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    pub labels: BTreeMap<String, String>,
    pub value: f64,
}

/// Prometheus metric types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

impl fmt::Display for MetricType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Counter => f.write_str("counter"),
            Self::Gauge => f.write_str("gauge"),
            Self::Histogram => f.write_str("histogram"),
            Self::Summary => f.write_str("summary"),
        }
    }
}

/// Registry of metrics that can be exported in Prometheus exposition format.
pub struct PrometheusRegistry {
    metrics: Vec<PrometheusMetric>,
}

impl PrometheusRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
        }
    }

    /// Register a counter metric.
    pub fn register_counter(
        &mut self,
        name: &str,
        help: &str,
        labels: BTreeMap<String, String>,
        value: f64,
    ) {
        self.metrics.push(PrometheusMetric {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Counter,
            labels,
            value,
        });
    }

    /// Register a gauge metric.
    pub fn register_gauge(
        &mut self,
        name: &str,
        help: &str,
        labels: BTreeMap<String, String>,
        value: f64,
    ) {
        self.metrics.push(PrometheusMetric {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Gauge,
            labels,
            value,
        });
    }

    /// Increment a counter by `delta`. Returns `true` if the counter was found.
    pub fn increment_counter(&mut self, name: &str, delta: f64) -> bool {
        let mut found = false;
        for metric in &mut self.metrics {
            if metric.name == name && metric.metric_type == MetricType::Counter {
                metric.value += delta;
                found = true;
            }
        }
        found
    }

    /// Set a gauge value. Returns `true` if the gauge was found.
    pub fn set_gauge(&mut self, name: &str, value: f64) -> bool {
        let mut found = false;
        for metric in &mut self.metrics {
            if metric.name == name && metric.metric_type == MetricType::Gauge {
                metric.value = value;
                found = true;
            }
        }
        found
    }

    /// Look up a metric by name.
    pub fn get_metric(&self, name: &str) -> Option<&PrometheusMetric> {
        self.metrics.iter().find(|m| m.name == name)
    }

    /// Render all metrics in the Prometheus exposition text format.
    pub fn export_prometheus(&self) -> String {
        let mut out = String::new();
        for metric in &self.metrics {
            out.push_str(&format!("# HELP {} {}\n", metric.name, metric.help));
            out.push_str(&format!("# TYPE {} {}\n", metric.name, metric.metric_type));
            out.push_str(&metric.name);
            if !metric.labels.is_empty() {
                out.push('{');
                let pairs: Vec<String> = metric
                    .labels
                    .iter()
                    .map(|(k, v)| format!("{k}=\"{v}\""))
                    .collect();
                out.push_str(&pairs.join(","));
                out.push('}');
            }
            out.push_str(&format!(" {}\n", format_value(metric.value)));
        }
        out
    }

    /// Render all metrics as a JSON array string.
    pub fn export_json(&self) -> String {
        let entries: Vec<String> = self
            .metrics
            .iter()
            .map(|m| {
                let labels_json: Vec<String> = m
                    .labels
                    .iter()
                    .map(|(k, v)| format!("\"{}\":\"{}\"", escape_json(k), escape_json(v)))
                    .collect();
                format!(
                    "{{\"name\":\"{}\",\"help\":\"{}\",\"type\":\"{}\",\"labels\":{{{}}},\"value\":{}}}",
                    escape_json(&m.name),
                    escape_json(&m.help),
                    m.metric_type,
                    labels_json.join(","),
                    format_value(m.value),
                )
            })
            .collect();
        format!("[{}]", entries.join(","))
    }

    /// Return the number of registered metrics.
    pub fn metric_count(&self) -> usize {
        self.metrics.len()
    }

    /// Remove all metrics from the registry.
    pub fn clear(&mut self) {
        self.metrics.clear();
    }
}

impl Default for PrometheusRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Format an f64 value, ensuring whole numbers get a `.0` suffix.
fn format_value(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// Minimal JSON string escaping (backslash and double-quote).
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn test_register_counter() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("requests_total", "Total requests", BTreeMap::new(), 1.0);
        assert_eq!(reg.metric_count(), 1);
        let m = reg.get_metric("requests_total").unwrap();
        assert_eq!(m.metric_type, MetricType::Counter);
        assert_eq!(m.value, 1.0);
    }

    #[test]
    fn test_register_gauge() {
        let mut reg = PrometheusRegistry::new();
        reg.register_gauge("temperature", "Current temp", BTreeMap::new(), 36.6);
        assert_eq!(reg.metric_count(), 1);
        let m = reg.get_metric("temperature").unwrap();
        assert_eq!(m.metric_type, MetricType::Gauge);
        assert_eq!(m.value, 36.6);
    }

    #[test]
    fn test_increment_counter() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("hits", "Hit count", BTreeMap::new(), 5.0);
        assert!(reg.increment_counter("hits", 3.0));
        assert_eq!(reg.get_metric("hits").unwrap().value, 8.0);
    }

    #[test]
    fn test_increment_counter_missing_returns_false() {
        let mut reg = PrometheusRegistry::new();
        assert!(!reg.increment_counter("missing", 1.0));
    }

    #[test]
    fn test_set_gauge_value() {
        let mut reg = PrometheusRegistry::new();
        reg.register_gauge("cpu", "CPU usage", BTreeMap::new(), 0.0);
        assert!(reg.set_gauge("cpu", 75.5));
        assert_eq!(reg.get_metric("cpu").unwrap().value, 75.5);
    }

    #[test]
    fn test_set_gauge_missing_returns_false() {
        let mut reg = PrometheusRegistry::new();
        assert!(!reg.set_gauge("missing", 1.0));
    }

    #[test]
    fn test_export_prometheus_format() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("req_total", "Total reqs", BTreeMap::new(), 42.0);
        let output = reg.export_prometheus();
        assert!(output.contains("# HELP req_total Total reqs"));
        assert!(output.contains("# TYPE req_total counter"));
        assert!(output.contains("req_total 42.0"));
    }

    #[test]
    fn test_export_json_format() {
        let mut reg = PrometheusRegistry::new();
        reg.register_gauge("temp", "Temperature", BTreeMap::new(), 20.0);
        let json = reg.export_json();
        assert!(json.contains("\"name\":\"temp\""));
        assert!(json.contains("\"type\":\"gauge\""));
        assert!(json.contains("\"value\":20.0"));
    }

    #[test]
    fn test_labels_in_prometheus_output() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter(
            "http_requests",
            "HTTP requests",
            labels(&[("method", "GET"), ("status", "200")]),
            10.0,
        );
        let output = reg.export_prometheus();
        assert!(output.contains("http_requests{method=\"GET\",status=\"200\"} 10.0"));
    }

    #[test]
    fn test_multiple_metrics_exported() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("a", "A help", BTreeMap::new(), 1.0);
        reg.register_gauge("b", "B help", BTreeMap::new(), 2.0);
        reg.register_counter("c", "C help", BTreeMap::new(), 3.0);
        let output = reg.export_prometheus();
        assert!(output.contains("# HELP a A help"));
        assert!(output.contains("# HELP b B help"));
        assert!(output.contains("# HELP c C help"));
        assert_eq!(reg.metric_count(), 3);
    }

    #[test]
    fn test_metric_types_correct_in_output() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("c", "counter", BTreeMap::new(), 0.0);
        reg.register_gauge("g", "gauge", BTreeMap::new(), 0.0);
        let output = reg.export_prometheus();
        assert!(output.contains("# TYPE c counter"));
        assert!(output.contains("# TYPE g gauge"));
    }

    #[test]
    fn test_clear_empties_registry() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("x", "x", BTreeMap::new(), 1.0);
        reg.register_gauge("y", "y", BTreeMap::new(), 2.0);
        assert_eq!(reg.metric_count(), 2);
        reg.clear();
        assert_eq!(reg.metric_count(), 0);
        assert!(reg.export_prometheus().is_empty());
    }

    #[test]
    fn test_get_metric_returns_correct_metric() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("alpha", "First", BTreeMap::new(), 10.0);
        reg.register_gauge("beta", "Second", BTreeMap::new(), 20.0);
        let m = reg.get_metric("beta").unwrap();
        assert_eq!(m.name, "beta");
        assert_eq!(m.help, "Second");
        assert_eq!(m.metric_type, MetricType::Gauge);
        assert_eq!(m.value, 20.0);
    }

    #[test]
    fn test_get_metric_missing_returns_none() {
        let reg = PrometheusRegistry::new();
        assert!(reg.get_metric("nonexistent").is_none());
    }

    #[test]
    fn test_labels_in_json_output() {
        let mut reg = PrometheusRegistry::new();
        reg.register_gauge(
            "disk",
            "Disk usage",
            labels(&[("mount", "/data")]),
            85.0,
        );
        let json = reg.export_json();
        assert!(json.contains("\"mount\":\"/data\""));
    }
}
