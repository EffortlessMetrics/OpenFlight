//! Minimal Prometheus-format HTTP metrics server.
//!
//! Provides an embedded metrics endpoint for OpenFlight services.
//! Uses only std library for zero additional dependencies.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;

/// A single named counter metric.
pub struct Counter {
    pub name: &'static str,
    pub help: &'static str,
    value: AtomicU64,
}

impl Counter {
    pub const fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            value: AtomicU64::new(0),
        }
    }
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// A single named gauge metric.
pub struct Gauge {
    pub name: &'static str,
    pub help: &'static str,
    value: AtomicU64, // stores f64 bits
}

impl Gauge {
    pub const fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            value: AtomicU64::new(0),
        }
    }
    pub fn set(&self, v: f64) {
        self.value.store(v.to_bits(), Ordering::Relaxed);
    }
    pub fn get(&self) -> f64 {
        f64::from_bits(self.value.load(Ordering::Relaxed))
    }
}

/// Collection of metrics for Prometheus export.
pub struct MetricsSnapshot {
    pub entries: Vec<MetricsEntry>,
}

/// A single metrics entry in Prometheus format.
pub struct MetricsEntry {
    pub name: String,
    pub help: String,
    pub kind: MetricsKind,
    pub value: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum MetricsKind {
    Counter,
    Gauge,
}

impl MetricsSnapshot {
    pub fn to_prometheus_text(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            let kind_str = match entry.kind {
                MetricsKind::Counter => "counter",
                MetricsKind::Gauge => "gauge",
            };
            out.push_str(&format!(
                "# HELP {} {}\n# TYPE {} {}\n{} {}\n",
                entry.name, entry.help, entry.name, kind_str, entry.name, entry.value
            ));
        }
        out
    }
}

/// Callback for collecting metrics snapshots.
pub trait MetricsCollector: Send + Sync {
    fn collect(&self) -> MetricsSnapshot;
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

/// Minimal HTTP server that exposes `/metrics` and `/health` endpoints.
pub struct MetricsHttpServer {
    config: MetricsServerConfig,
    collector: Arc<dyn MetricsCollector>,
    running: Arc<AtomicBool>,
}

impl MetricsHttpServer {
    pub fn new(config: MetricsServerConfig, collector: Arc<dyn MetricsCollector>) -> Self {
        Self {
            config,
            collector,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) -> std::io::Result<()> {
        let addr = format!("{}:{}", self.config.bind_addr, self.config.port);
        let listener = TcpListener::bind(&addr)?;
        self.running.store(true, Ordering::Relaxed);
        let collector = Arc::clone(&self.collector);
        let running = Arc::clone(&self.running);

        thread::spawn(move || {
            for stream in listener.incoming() {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(stream) = stream {
                    let c = Arc::clone(&collector);
                    thread::spawn(move || handle_metrics_request(stream, c.as_ref()));
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

fn handle_metrics_request(mut stream: TcpStream, collector: &dyn MetricsCollector) {
    let buf = BufReader::new(&stream);
    let line = buf
        .lines()
        .next()
        .unwrap_or(Ok(String::new()))
        .unwrap_or_default();
    let (status, body, content_type) = if line.starts_with("GET /metrics") {
        let snap = collector.collect();
        let text = snap.to_prometheus_text();
        ("200 OK", text, "text/plain; version=0.0.4")
    } else if line.starts_with("GET /health") {
        ("200 OK", "ok\n".to_string(), "text/plain")
    } else {
        ("404 Not Found", "Not Found\n".to_string(), "text/plain")
    };
    let resp = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        content_type,
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCollector {
        ticks: u64,
        errors: u64,
    }

    impl MetricsCollector for TestCollector {
        fn collect(&self) -> MetricsSnapshot {
            MetricsSnapshot {
                entries: vec![
                    MetricsEntry {
                        name: "test_ticks".to_string(),
                        help: "test".to_string(),
                        kind: MetricsKind::Counter,
                        value: self.ticks as f64,
                    },
                    MetricsEntry {
                        name: "test_errors".to_string(),
                        help: "test".to_string(),
                        kind: MetricsKind::Gauge,
                        value: self.errors as f64,
                    },
                ],
            }
        }
    }

    #[test]
    fn test_counter_increment() {
        let c = Counter::new("test", "help");
        c.inc();
        c.inc();
        assert_eq!(c.get(), 2);
    }

    #[test]
    fn test_counter_add() {
        let c = Counter::new("test", "help");
        c.add(10);
        assert_eq!(c.get(), 10);
    }

    #[test]
    fn test_counter_reset() {
        let c = Counter::new("test", "help");
        c.add(5);
        c.reset();
        assert_eq!(c.get(), 0);
    }

    #[test]
    fn test_gauge_set_get() {
        let g = Gauge::new("test", "help");
        g.set(3.125);
        let v = g.get();
        assert!((v - 3.125).abs() < 0.001);
    }

    #[test]
    fn test_metrics_snapshot_to_prometheus() {
        let c = TestCollector {
            ticks: 42,
            errors: 1,
        };
        let snap = c.collect();
        let text = snap.to_prometheus_text();
        assert!(text.contains("test_ticks 42"));
        assert!(text.contains("# TYPE test_ticks counter"));
        assert!(text.contains("test_errors 1"));
    }

    #[test]
    fn test_default_config() {
        let config = MetricsServerConfig::default();
        assert_eq!(config.port, 9898);
        assert_eq!(config.bind_addr, "127.0.0.1");
    }

    #[test]
    fn test_server_not_running_initially() {
        let c = Arc::new(TestCollector {
            ticks: 0,
            errors: 0,
        });
        let server = MetricsHttpServer::new(Default::default(), c);
        assert!(!server.is_running());
    }

    #[test]
    fn test_prometheus_counter_type_label() {
        let c = TestCollector {
            ticks: 1,
            errors: 0,
        };
        let snap = c.collect();
        let text = snap.to_prometheus_text();
        assert!(text.contains("# TYPE test_ticks counter"));
        assert!(text.contains("# HELP test_ticks test"));
    }

    #[test]
    fn test_prometheus_gauge_type_label() {
        let c = TestCollector {
            ticks: 0,
            errors: 5,
        };
        let snap = c.collect();
        let text = snap.to_prometheus_text();
        assert!(text.contains("# TYPE test_errors gauge"));
    }
}
