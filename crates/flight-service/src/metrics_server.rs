//! Lightweight Prometheus-format metrics HTTP server.
//!
//! Exposes a `/metrics` endpoint using a minimal raw TCP/HTTP implementation
//! to avoid adding a full web framework dependency. This is only enabled when
//! configured.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;

/// Snapshot of service metrics for Prometheus export.
#[derive(Debug, Default, Clone)]
pub struct ServiceMetricsSnapshot {
    pub axis_ticks_total: u64,
    pub axis_errors_total: u64,
    pub profiles_loaded: u64,
    pub adapter_reconnects: u64,
    pub uptime_seconds: u64,
}

/// Shared metrics counters (thread-safe).
#[derive(Debug, Default)]
pub struct PrometheusMetrics {
    pub axis_ticks_total: AtomicU64,
    pub axis_errors_total: AtomicU64,
    pub profiles_loaded: AtomicU64,
    pub adapter_reconnects: AtomicU64,
}

impl PrometheusMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn snapshot(&self) -> ServiceMetricsSnapshot {
        ServiceMetricsSnapshot {
            axis_ticks_total: self.axis_ticks_total.load(Ordering::Relaxed),
            axis_errors_total: self.axis_errors_total.load(Ordering::Relaxed),
            profiles_loaded: self.profiles_loaded.load(Ordering::Relaxed),
            adapter_reconnects: self.adapter_reconnects.load(Ordering::Relaxed),
            uptime_seconds: 0,
        }
    }

    pub fn to_prometheus_text(&self, uptime_secs: u64) -> String {
        let snap = self.snapshot();
        format!(
            "# HELP openflight_axis_ticks_total Total axis processing ticks\n\
             # TYPE openflight_axis_ticks_total counter\n\
             openflight_axis_ticks_total {}\n\
             # HELP openflight_axis_errors_total Total axis processing errors\n\
             # TYPE openflight_axis_errors_total counter\n\
             openflight_axis_errors_total {}\n\
             # HELP openflight_profiles_loaded_total Total profiles loaded\n\
             # TYPE openflight_profiles_loaded_total counter\n\
             openflight_profiles_loaded_total {}\n\
             # HELP openflight_adapter_reconnects_total Total adapter reconnects\n\
             # TYPE openflight_adapter_reconnects_total counter\n\
             openflight_adapter_reconnects_total {}\n\
             # HELP openflight_uptime_seconds Service uptime in seconds\n\
             # TYPE openflight_uptime_seconds gauge\n\
             openflight_uptime_seconds {}\n",
            snap.axis_ticks_total,
            snap.axis_errors_total,
            snap.profiles_loaded,
            snap.adapter_reconnects,
            uptime_secs,
        )
    }
}

/// Configuration for the metrics HTTP server.
#[derive(Debug, Clone)]
pub struct MetricsServerConfig {
    pub bind_addr: String,
    pub port: u16,
}

impl Default for MetricsServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1".to_string(),
            port: 9898,
        }
    }
}

/// Minimal HTTP metrics server.
///
/// Listens on the configured address and responds to GET /metrics with
/// Prometheus text format. All other paths return 404.
pub struct MetricsServer {
    config: MetricsServerConfig,
    metrics: Arc<PrometheusMetrics>,
    running: Arc<AtomicBool>,
}

impl MetricsServer {
    pub fn new(config: MetricsServerConfig, metrics: Arc<PrometheusMetrics>) -> Self {
        Self {
            config,
            metrics,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) -> std::io::Result<()> {
        let addr = format!("{}:{}", self.config.bind_addr, self.config.port);
        let listener = TcpListener::bind(&addr)?;
        listener.set_nonblocking(false)?;

        self.running.store(true, Ordering::Relaxed);
        let metrics = Arc::clone(&self.metrics);
        let running = Arc::clone(&self.running);

        thread::spawn(move || {
            for stream in listener.incoming() {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(stream) = stream {
                    let metrics = Arc::clone(&metrics);
                    thread::spawn(move || handle_connection(stream, &metrics));
                }
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.bind_addr, self.config.port)
    }
}

fn handle_connection(mut stream: TcpStream, metrics: &PrometheusMetrics) {
    let first_line = {
        let reader = BufReader::new(&stream);
        reader
            .lines()
            .next()
            .unwrap_or_else(|| Ok(String::new()))
            .unwrap_or_default()
    };

    let (status, body) = if first_line.starts_with("GET /metrics") {
        let body = metrics.to_prometheus_text(0);
        ("200 OK", body)
    } else if first_line.starts_with("GET /health") {
        ("200 OK", "ok\n".to_string())
    } else {
        ("404 Not Found", "Not Found\n".to_string())
    };

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_default_config() {
        let config = MetricsServerConfig::default();
        assert_eq!(config.port, 9898);
        assert_eq!(config.bind_addr, "127.0.0.1");
    }

    #[test]
    fn test_metrics_snapshot_default_zeros() {
        let m = PrometheusMetrics::default();
        let snap = m.snapshot();
        assert_eq!(snap.axis_ticks_total, 0);
        assert_eq!(snap.axis_errors_total, 0);
        assert_eq!(snap.profiles_loaded, 0);
        assert_eq!(snap.adapter_reconnects, 0);
    }

    #[test]
    fn test_metrics_increment_counters() {
        let m = PrometheusMetrics::default();
        m.axis_ticks_total.fetch_add(100, Ordering::Relaxed);
        m.axis_errors_total.fetch_add(2, Ordering::Relaxed);
        let snap = m.snapshot();
        assert_eq!(snap.axis_ticks_total, 100);
        assert_eq!(snap.axis_errors_total, 2);
    }

    #[test]
    fn test_prometheus_text_format() {
        let m = PrometheusMetrics::default();
        m.axis_ticks_total.fetch_add(42, Ordering::Relaxed);
        let text = m.to_prometheus_text(100);
        assert!(text.contains("openflight_axis_ticks_total 42"));
        assert!(text.contains("openflight_uptime_seconds 100"));
        assert!(text.contains("# TYPE openflight_axis_ticks_total counter"));
    }

    #[test]
    fn test_prometheus_text_contains_all_metrics() {
        let m = PrometheusMetrics::default();
        let text = m.to_prometheus_text(0);
        assert!(text.contains("openflight_axis_ticks_total"));
        assert!(text.contains("openflight_axis_errors_total"));
        assert!(text.contains("openflight_profiles_loaded_total"));
        assert!(text.contains("openflight_adapter_reconnects_total"));
        assert!(text.contains("openflight_uptime_seconds"));
    }

    #[test]
    fn test_metrics_server_not_running_initially() {
        let config = MetricsServerConfig::default();
        let metrics = PrometheusMetrics::new();
        let server = MetricsServer::new(config, metrics);
        assert!(!server.is_running());
    }

    #[test]
    fn test_metrics_server_bind_address() {
        let config = MetricsServerConfig {
            bind_addr: "0.0.0.0".to_string(),
            port: 12345,
        };
        let metrics = PrometheusMetrics::new();
        let server = MetricsServer::new(config, metrics);
        assert_eq!(server.bind_address(), "0.0.0.0:12345");
    }

    #[test]
    fn test_metrics_arc_is_shared() {
        let metrics = PrometheusMetrics::new();
        let m2 = Arc::clone(&metrics);
        metrics.axis_ticks_total.fetch_add(5, Ordering::Relaxed);
        assert_eq!(m2.axis_ticks_total.load(Ordering::Relaxed), 5);
    }
}
