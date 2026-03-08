// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metrics export in Prometheus exposition and JSON formats.
//!
//! [`MetricsExporter`] converts a slice of [`LabeledMetric`] values into
//! Prometheus text or JSON strings. Each metric carries optional key-value
//! labels that appear in the output.

/// A single metric with optional labels and help text.
#[derive(Debug, Clone)]
pub struct LabeledMetric {
    pub name: String,
    pub help: String,
    pub labels: Vec<(String, String)>,
    pub value: MetricValue,
}

/// The payload of a labeled metric.
#[derive(Debug, Clone)]
pub enum MetricValue {
    /// Monotonically increasing counter.
    Counter(u64),
    /// Point-in-time gauge.
    Gauge(f64),
    /// Histogram with cumulative bucket counts.
    Histogram {
        buckets: Vec<(f64, u64)>,
        count: u64,
        sum: f64,
    },
}

/// Stateless exporter that renders [`LabeledMetric`] slices.
pub struct MetricsExporter;

impl MetricsExporter {
    /// Render metrics in the Prometheus exposition text format.
    pub fn format_prometheus(metrics: &[LabeledMetric]) -> String {
        let mut out = String::new();
        for m in metrics {
            out.push_str(&format!("# HELP {} {}\n", m.name, m.help));
            let type_str = match &m.value {
                MetricValue::Counter(_) => "counter",
                MetricValue::Gauge(_) => "gauge",
                MetricValue::Histogram { .. } => "histogram",
            };
            out.push_str(&format!("# TYPE {} {type_str}\n", m.name));

            let labels_str = format_labels(&m.labels);

            match &m.value {
                MetricValue::Counter(v) => {
                    out.push_str(&format!("{}{labels_str} {v}\n", m.name));
                }
                MetricValue::Gauge(v) => {
                    out.push_str(&format!("{}{labels_str} {}\n", m.name, format_f64(*v)));
                }
                MetricValue::Histogram {
                    buckets,
                    count,
                    sum,
                } => {
                    for (le, c) in buckets {
                        let le_str = if le.is_infinite() {
                            "+Inf".to_string()
                        } else {
                            format_f64(*le)
                        };
                        let mut bucket_labels = m.labels.clone();
                        bucket_labels.push(("le".to_string(), le_str));
                        let bl = format_labels(&bucket_labels);
                        out.push_str(&format!("{}_bucket{bl} {c}\n", m.name));
                    }
                    out.push_str(&format!(
                        "{}_sum{labels_str} {}\n",
                        m.name,
                        format_f64(*sum)
                    ));
                    out.push_str(&format!("{}_count{labels_str} {count}\n", m.name));
                }
            }
        }
        out
    }

    /// Render metrics as a JSON array string.
    pub fn format_json(metrics: &[LabeledMetric]) -> String {
        let entries: Vec<String> = metrics
            .iter()
            .map(|m| {
                let labels_json = format_labels_json(&m.labels);
                let (type_str, value_json) = match &m.value {
                    MetricValue::Counter(v) => ("counter", format!("{v}")),
                    MetricValue::Gauge(v) => ("gauge", format_f64(*v)),
                    MetricValue::Histogram {
                        buckets,
                        count,
                        sum,
                    } => {
                        let bucket_entries: Vec<String> = buckets
                            .iter()
                            .map(|(le, c)| {
                                let le_str = if le.is_infinite() {
                                    "\"+Inf\"".to_string()
                                } else {
                                    format_f64(*le)
                                };
                                format!("{{\"le\":{le_str},\"count\":{c}}}")
                            })
                            .collect();
                        (
                            "histogram",
                            format!(
                                "{{\"buckets\":[{}],\"count\":{count},\"sum\":{}}}",
                                bucket_entries.join(","),
                                format_f64(*sum)
                            ),
                        )
                    }
                };
                format!(
                    "{{\"name\":\"{}\",\"help\":\"{}\",\"type\":\"{type_str}\",\"labels\":{{{labels_json}}},\"value\":{value_json}}}",
                    escape_json(&m.name),
                    escape_json(&m.help),
                )
            })
            .collect();
        format!("[{}]", entries.join(","))
    }
}

fn format_labels(labels: &[(String, String)]) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let pairs: Vec<String> = labels
        .iter()
        .map(|(k, v)| format!("{k}=\"{}\"", escape_prometheus_label(v)))
        .collect();
    format!("{{{}}}", pairs.join(","))
}

fn format_labels_json(labels: &[(String, String)]) -> String {
    labels
        .iter()
        .map(|(k, v)| format!("\"{}\":\"{}\"", escape_json(k), escape_json(v)))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_f64(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

fn escape_prometheus_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter(name: &str, help: &str, value: u64) -> LabeledMetric {
        LabeledMetric {
            name: name.to_string(),
            help: help.to_string(),
            labels: vec![],
            value: MetricValue::Counter(value),
        }
    }

    fn gauge(name: &str, help: &str, value: f64) -> LabeledMetric {
        LabeledMetric {
            name: name.to_string(),
            help: help.to_string(),
            labels: vec![],
            value: MetricValue::Gauge(value),
        }
    }

    fn labeled_counter(
        name: &str,
        help: &str,
        labels: &[(&str, &str)],
        value: u64,
    ) -> LabeledMetric {
        LabeledMetric {
            name: name.to_string(),
            help: help.to_string(),
            labels: labels
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            value: MetricValue::Counter(value),
        }
    }

    fn histogram(
        name: &str,
        help: &str,
        buckets: &[(f64, u64)],
        count: u64,
        sum: f64,
    ) -> LabeledMetric {
        LabeledMetric {
            name: name.to_string(),
            help: help.to_string(),
            labels: vec![],
            value: MetricValue::Histogram {
                buckets: buckets.to_vec(),
                count,
                sum,
            },
        }
    }

    // ── Prometheus format ──────────────────────────────────────────────────

    #[test]
    fn prometheus_counter() {
        let text = MetricsExporter::format_prometheus(&[counter("req_total", "Total reqs", 42)]);
        assert!(text.contains("# HELP req_total Total reqs"));
        assert!(text.contains("# TYPE req_total counter"));
        assert!(text.contains("req_total 42"));
    }

    #[test]
    fn prometheus_gauge() {
        let text =
            MetricsExporter::format_prometheus(&[gauge("temperature", "Current temp", 36.6)]);
        assert!(text.contains("# TYPE temperature gauge"));
        assert!(text.contains("temperature 36.6"));
    }

    #[test]
    fn prometheus_histogram() {
        let text = MetricsExporter::format_prometheus(&[histogram(
            "latency",
            "Latency ms",
            &[(1.0, 1), (5.0, 3), (f64::INFINITY, 4)],
            4,
            12.5,
        )]);
        assert!(text.contains("# TYPE latency histogram"));
        assert!(text.contains("latency_bucket{le=\"1.0\"} 1"));
        assert!(text.contains("latency_bucket{le=\"5.0\"} 3"));
        assert!(text.contains("latency_bucket{le=\"+Inf\"} 4"));
        assert!(text.contains("latency_sum 12.5"));
        assert!(text.contains("latency_count 4"));
    }

    #[test]
    fn prometheus_labels() {
        let text = MetricsExporter::format_prometheus(&[labeled_counter(
            "http_requests",
            "HTTP requests",
            &[("method", "GET"), ("status", "200")],
            10,
        )]);
        assert!(text.contains("http_requests{method=\"GET\",status=\"200\"} 10"));
    }

    #[test]
    fn prometheus_empty_slice() {
        let text = MetricsExporter::format_prometheus(&[]);
        assert!(text.is_empty());
    }

    // ── JSON format ────────────────────────────────────────────────────────

    #[test]
    fn json_counter() {
        let json = MetricsExporter::format_json(&[counter("events", "Events", 99)]);
        assert!(json.contains("\"name\":\"events\""));
        assert!(json.contains("\"type\":\"counter\""));
        assert!(json.contains("\"value\":99"));
    }

    #[test]
    fn json_gauge() {
        let json = MetricsExporter::format_json(&[gauge("mem", "Memory", 1024.0)]);
        assert!(json.contains("\"type\":\"gauge\""));
        assert!(json.contains("\"value\":1024.0"));
    }

    #[test]
    fn json_labels() {
        let json = MetricsExporter::format_json(&[labeled_counter(
            "hits",
            "Hit count",
            &[("path", "/api")],
            5,
        )]);
        assert!(json.contains("\"path\":\"/api\""));
    }

    #[test]
    fn json_empty_slice() {
        let json = MetricsExporter::format_json(&[]);
        assert_eq!(json, "[]");
    }

    #[test]
    fn json_escapes_special_chars() {
        let json = MetricsExporter::format_json(&[counter("c", "desc with \"quotes\"", 1)]);
        assert!(json.contains("desc with \\\"quotes\\\""));
    }

    #[test]
    fn json_histogram() {
        let json =
            MetricsExporter::format_json(&[histogram("lat", "Latency", &[(5.0, 2)], 3, 7.5)]);
        assert!(json.contains("\"type\":\"histogram\""));
        assert!(json.contains("\"count\":3"));
        assert!(json.contains("\"sum\":7.5"));
    }

    #[test]
    fn json_histogram_includes_buckets() {
        let json = MetricsExporter::format_json(&[histogram(
            "lat",
            "Latency",
            &[(1.0, 1), (5.0, 3), (f64::INFINITY, 4)],
            4,
            12.5,
        )]);
        assert!(json.contains("\"buckets\":["));
        assert!(json.contains("{\"le\":1.0,\"count\":1}"));
        assert!(json.contains("{\"le\":5.0,\"count\":3}"));
        assert!(json.contains("{\"le\":\"+Inf\",\"count\":4}"));
    }

    #[test]
    fn prometheus_label_escaping() {
        let m = LabeledMetric {
            name: "test_metric".to_string(),
            help: "test".to_string(),
            labels: vec![
                ("path".to_string(), "val with \"quotes\"".to_string()),
                ("info".to_string(), "back\\slash".to_string()),
                ("multi".to_string(), "line1\nline2".to_string()),
            ],
            value: MetricValue::Counter(1),
        };
        let text = MetricsExporter::format_prometheus(&[m]);
        assert!(text.contains(r#"path="val with \"quotes\"""#));
        assert!(text.contains(r#"info="back\\slash""#));
        assert!(text.contains(r#"multi="line1\nline2""#));
    }
}
