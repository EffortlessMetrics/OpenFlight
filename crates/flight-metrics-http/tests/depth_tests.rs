//! Depth tests for flight-metrics-http crate.
//!
//! Covers Counter, Gauge, MetricsSnapshot, MetricsCollector trait,
//! Prometheus text formatting, MetricsServerConfig, and HTTP server
//! lifecycle + request handling.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use flight_metrics_http::{
    Counter, Gauge, MetricsCollector, MetricsEntry, MetricsHttpServer, MetricsKind,
    MetricsServerConfig, MetricsSnapshot,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Allocate an ephemeral port by binding to :0 and returning the assigned port.
fn ephemeral_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    listener.local_addr().unwrap().port()
}

/// Perform a raw HTTP GET and return (status_line, headers, body).
fn http_get(addr: &str, path: &str) -> (String, Vec<String>, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, addr
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream.flush().unwrap();

    let reader = BufReader::new(&stream);
    let mut lines: Vec<String> = Vec::new();
    for line in reader.lines() {
        match line {
            Ok(l) => lines.push(l.trim_end_matches('\r').to_string()),
            Err(_) => break,
        }
    }

    let status_line = lines.first().cloned().unwrap_or_default();
    let mut header_end = 0;
    for (i, l) in lines.iter().enumerate().skip(1) {
        if l.is_empty() {
            header_end = i;
            break;
        }
    }
    let headers: Vec<String> = if header_end > 1 {
        lines[1..header_end].to_vec()
    } else {
        Vec::new()
    };
    let body = if header_end > 0 && header_end + 1 < lines.len() {
        lines[header_end + 1..].join("\n")
    } else {
        String::new()
    };

    (status_line, headers, body)
}

/// Storable entry description (Clone-friendly, unlike MetricsEntry).
struct EntryDesc {
    name: String,
    help: String,
    kind: MetricsKind,
    value: f64,
}

impl EntryDesc {
    fn to_entry(&self) -> MetricsEntry {
        MetricsEntry {
            name: self.name.clone(),
            help: self.help.clone(),
            kind: self.kind,
            value: self.value,
        }
    }
}

/// A configurable test collector.
struct StubCollector {
    descs: Vec<EntryDesc>,
}

impl StubCollector {
    fn empty() -> Self {
        Self { descs: Vec::new() }
    }

    fn with_entries(entries: Vec<MetricsEntry>) -> Self {
        let descs = entries
            .into_iter()
            .map(|e| EntryDesc {
                name: e.name,
                help: e.help,
                kind: e.kind,
                value: e.value,
            })
            .collect();
        Self { descs }
    }
}

impl MetricsCollector for StubCollector {
    fn collect(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            entries: self.descs.iter().map(EntryDesc::to_entry).collect(),
        }
    }
}

/// A collector that increments an internal call-counter on every `collect`.
struct CountingCollector {
    calls: AtomicU64,
}

impl CountingCollector {
    fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
        }
    }

    fn call_count(&self) -> u64 {
        self.calls.load(Ordering::Relaxed)
    }
}

impl MetricsCollector for CountingCollector {
    fn collect(&self) -> MetricsSnapshot {
        self.calls.fetch_add(1, Ordering::Relaxed);
        MetricsSnapshot {
            entries: vec![MetricsEntry {
                name: "collect_calls".to_string(),
                help: "number of collect invocations".to_string(),
                kind: MetricsKind::Counter,
                value: self.calls.load(Ordering::Relaxed) as f64,
            }],
        }
    }
}

/// Start a MetricsHttpServer on an ephemeral port, wait until it is accepting,
/// and return the server + address string.
fn start_server(collector: Arc<dyn MetricsCollector>) -> (MetricsHttpServer, String) {
    let port = ephemeral_port();
    let config = MetricsServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port,
    };
    let addr = format!("127.0.0.1:{}", port);
    let server = MetricsHttpServer::new(config, collector);
    server.start().expect("server start");
    // Poll until the listener is accepting connections.
    for _ in 0..50 {
        if TcpStream::connect(&addr).is_ok() {
            return (server, addr);
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("server did not become ready in time");
}

// ===========================================================================
// Counter tests
// ===========================================================================

#[test]
fn counter_starts_at_zero() {
    let c = Counter::new("c", "help");
    assert_eq!(c.get(), 0);
}

#[test]
fn counter_inc_single() {
    let c = Counter::new("c", "help");
    c.inc();
    assert_eq!(c.get(), 1);
}

#[test]
fn counter_inc_multiple() {
    let c = Counter::new("c", "help");
    for _ in 0..100 {
        c.inc();
    }
    assert_eq!(c.get(), 100);
}

#[test]
fn counter_add_zero() {
    let c = Counter::new("c", "help");
    c.add(0);
    assert_eq!(c.get(), 0);
}

#[test]
fn counter_add_large() {
    let c = Counter::new("c", "help");
    c.add(1_000_000);
    assert_eq!(c.get(), 1_000_000);
}

#[test]
fn counter_add_then_inc() {
    let c = Counter::new("c", "help");
    c.add(10);
    c.inc();
    assert_eq!(c.get(), 11);
}

#[test]
fn counter_reset_to_zero() {
    let c = Counter::new("c", "help");
    c.add(42);
    c.reset();
    assert_eq!(c.get(), 0);
}

#[test]
fn counter_reset_then_inc() {
    let c = Counter::new("c", "help");
    c.add(100);
    c.reset();
    c.inc();
    assert_eq!(c.get(), 1);
}

#[test]
fn counter_name_and_help_preserved() {
    let c = Counter::new("my_counter", "counts things");
    assert_eq!(c.name, "my_counter");
    assert_eq!(c.help, "counts things");
}

#[test]
fn counter_concurrent_increments() {
    let c = Arc::new(Counter::new("c", "help"));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let c = Arc::clone(&c);
            thread::spawn(move || {
                for _ in 0..1_000 {
                    c.inc();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(c.get(), 8_000);
}

#[test]
fn counter_wrapping_add_max() {
    let c = Counter::new("c", "help");
    c.add(u64::MAX);
    assert_eq!(c.get(), u64::MAX);
}

// ===========================================================================
// Gauge tests
// ===========================================================================

#[test]
fn gauge_starts_at_zero() {
    let g = Gauge::new("g", "help");
    assert!((g.get() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn gauge_set_positive() {
    let g = Gauge::new("g", "help");
    g.set(42.5);
    assert!((g.get() - 42.5).abs() < f64::EPSILON);
}

#[test]
fn gauge_set_negative() {
    let g = Gauge::new("g", "help");
    g.set(-7.25);
    assert!((g.get() - (-7.25)).abs() < f64::EPSILON);
}

#[test]
fn gauge_overwrite_value() {
    let g = Gauge::new("g", "help");
    g.set(1.0);
    g.set(2.0);
    assert!((g.get() - 2.0).abs() < f64::EPSILON);
}

#[test]
fn gauge_very_small_value() {
    let g = Gauge::new("g", "help");
    g.set(1e-15);
    assert!((g.get() - 1e-15).abs() < f64::EPSILON);
}

#[test]
fn gauge_very_large_value() {
    let g = Gauge::new("g", "help");
    g.set(1e18);
    assert!((g.get() - 1e18).abs() < f64::EPSILON);
}

#[test]
fn gauge_nan_roundtrip() {
    let g = Gauge::new("g", "help");
    g.set(f64::NAN);
    assert!(g.get().is_nan());
}

#[test]
fn gauge_infinity_roundtrip() {
    let g = Gauge::new("g", "help");
    g.set(f64::INFINITY);
    assert!(g.get().is_infinite() && g.get().is_sign_positive());
}

#[test]
fn gauge_neg_infinity_roundtrip() {
    let g = Gauge::new("g", "help");
    g.set(f64::NEG_INFINITY);
    assert!(g.get().is_infinite() && g.get().is_sign_negative());
}

#[test]
fn gauge_name_and_help_preserved() {
    let g = Gauge::new("latency_ms", "p99 latency");
    assert_eq!(g.name, "latency_ms");
    assert_eq!(g.help, "p99 latency");
}

// ===========================================================================
// MetricsSnapshot / Prometheus text format
// ===========================================================================

#[test]
fn snapshot_empty_produces_empty_text() {
    let snap = MetricsSnapshot {
        entries: Vec::new(),
    };
    assert!(snap.to_prometheus_text().is_empty());
}

#[test]
fn snapshot_single_counter_format() {
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: "requests_total".into(),
            help: "Total requests".into(),
            kind: MetricsKind::Counter,
            value: 100.0,
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("# HELP requests_total Total requests\n"));
    assert!(text.contains("# TYPE requests_total counter\n"));
    assert!(text.contains("requests_total 100\n"));
}

#[test]
fn snapshot_single_gauge_format() {
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: "cpu_temp".into(),
            help: "CPU temperature".into(),
            kind: MetricsKind::Gauge,
            value: 72.5,
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("# TYPE cpu_temp gauge\n"));
    assert!(text.contains("cpu_temp 72.5\n"));
}

#[test]
fn snapshot_multiple_entries_order_preserved() {
    let snap = MetricsSnapshot {
        entries: vec![
            MetricsEntry {
                name: "aaa".into(),
                help: "first".into(),
                kind: MetricsKind::Counter,
                value: 1.0,
            },
            MetricsEntry {
                name: "bbb".into(),
                help: "second".into(),
                kind: MetricsKind::Gauge,
                value: 2.0,
            },
        ],
    };
    let text = snap.to_prometheus_text();
    let pos_a = text.find("aaa 1").expect("aaa present");
    let pos_b = text.find("bbb 2").expect("bbb present");
    assert!(pos_a < pos_b, "ordering preserved");
}

#[test]
fn snapshot_value_zero_rendered() {
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: "zero_metric".into(),
            help: "always zero".into(),
            kind: MetricsKind::Counter,
            value: 0.0,
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("zero_metric 0"));
}

#[test]
fn snapshot_fractional_value_rendered() {
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: "frac".into(),
            help: "fractional".into(),
            kind: MetricsKind::Gauge,
            value: 0.333,
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("frac 0.333"));
}

#[test]
fn snapshot_help_and_type_lines_present_for_each_entry() {
    let snap = MetricsSnapshot {
        entries: vec![
            MetricsEntry {
                name: "m1".into(),
                help: "h1".into(),
                kind: MetricsKind::Counter,
                value: 1.0,
            },
            MetricsEntry {
                name: "m2".into(),
                help: "h2".into(),
                kind: MetricsKind::Gauge,
                value: 2.0,
            },
        ],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("# HELP m1 h1"));
    assert!(text.contains("# TYPE m1 counter"));
    assert!(text.contains("# HELP m2 h2"));
    assert!(text.contains("# TYPE m2 gauge"));
}

#[test]
fn snapshot_large_number_of_entries() {
    let entries: Vec<MetricsEntry> = (0..100)
        .map(|i| MetricsEntry {
            name: format!("metric_{}", i),
            help: format!("help {}", i),
            kind: if i % 2 == 0 {
                MetricsKind::Counter
            } else {
                MetricsKind::Gauge
            },
            value: i as f64,
        })
        .collect();
    let snap = MetricsSnapshot { entries };
    let text = snap.to_prometheus_text();
    assert!(text.contains("metric_0 0"));
    assert!(text.contains("metric_99 99"));
    // Each entry produces 3 lines: HELP, TYPE, value
    let line_count = text.lines().count();
    assert_eq!(line_count, 300);
}

// ===========================================================================
// MetricsKind
// ===========================================================================

#[test]
fn metrics_kind_debug_counter() {
    assert_eq!(format!("{:?}", MetricsKind::Counter), "Counter");
}

#[test]
fn metrics_kind_debug_gauge() {
    assert_eq!(format!("{:?}", MetricsKind::Gauge), "Gauge");
}

#[test]
fn metrics_kind_clone() {
    let k = MetricsKind::Counter;
    let k2 = k;
    assert!(matches!(k2, MetricsKind::Counter));
}

// ===========================================================================
// MetricsServerConfig
// ===========================================================================

#[test]
fn config_default_values() {
    let c = MetricsServerConfig::default();
    assert_eq!(c.bind_addr, "127.0.0.1");
    assert_eq!(c.port, 9898);
}

#[test]
fn config_debug_impl() {
    let c = MetricsServerConfig::default();
    let dbg = format!("{:?}", c);
    assert!(dbg.contains("127.0.0.1"));
    assert!(dbg.contains("9898"));
}

#[test]
fn config_clone() {
    let c = MetricsServerConfig {
        bind_addr: "0.0.0.0".to_string(),
        port: 8080,
    };
    let c2 = c.clone();
    assert_eq!(c2.bind_addr, "0.0.0.0");
    assert_eq!(c2.port, 8080);
}

// ===========================================================================
// MetricsHttpServer — unit level
// ===========================================================================

#[test]
fn server_not_running_before_start() {
    let c = Arc::new(StubCollector::empty());
    let server = MetricsHttpServer::new(MetricsServerConfig::default(), c);
    assert!(!server.is_running());
}

#[test]
fn server_bind_address_format() {
    let config = MetricsServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port: 1234,
    };
    let c = Arc::new(StubCollector::empty());
    let server = MetricsHttpServer::new(config, c);
    assert_eq!(server.bind_address(), "127.0.0.1:1234");
}

#[test]
fn server_is_running_after_start() {
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::empty());
    let (server, _addr) = start_server(c);
    assert!(server.is_running());
    server.stop();
}

#[test]
fn server_stop_clears_running_flag() {
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::empty());
    let (server, _addr) = start_server(c);
    server.stop();
    assert!(!server.is_running());
}

// ===========================================================================
// MetricsHttpServer — HTTP integration
// ===========================================================================

#[test]
fn http_health_endpoint_returns_ok() {
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::empty());
    let (server, addr) = start_server(c);

    let (status, _headers, body) = http_get(&addr, "/health");
    assert!(status.contains("200 OK"));
    assert!(body.contains("ok"));

    server.stop();
}

#[test]
fn http_metrics_endpoint_returns_prometheus_text() {
    let entries = vec![MetricsEntry {
        name: "up".into(),
        help: "instance up".into(),
        kind: MetricsKind::Gauge,
        value: 1.0,
    }];
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::with_entries(entries));
    let (server, addr) = start_server(c);

    let (status, headers, body) = http_get(&addr, "/metrics");
    assert!(status.contains("200 OK"));
    assert!(
        headers
            .iter()
            .any(|h| h.contains("text/plain; version=0.0.4")),
        "content-type header"
    );
    assert!(body.contains("# TYPE up gauge"));
    assert!(body.contains("up 1"));

    server.stop();
}

#[test]
fn http_unknown_path_returns_404() {
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::empty());
    let (server, addr) = start_server(c);

    let (status, _headers, body) = http_get(&addr, "/nonexistent");
    assert!(status.contains("404"));
    assert!(body.contains("Not Found"));

    server.stop();
}

#[test]
fn http_root_path_returns_404() {
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::empty());
    let (server, addr) = start_server(c);

    let (status, _headers, _body) = http_get(&addr, "/");
    assert!(status.contains("404"));

    server.stop();
}

#[test]
fn http_metrics_empty_collector() {
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::empty());
    let (server, addr) = start_server(c);

    let (status, _headers, body) = http_get(&addr, "/metrics");
    assert!(status.contains("200 OK"));
    // Empty snapshot produces empty body.
    assert!(
        body.trim().is_empty(),
        "body should be empty for zero entries"
    );

    server.stop();
}

#[test]
fn http_metrics_with_many_entries() {
    let entries: Vec<MetricsEntry> = (0..20)
        .map(|i| MetricsEntry {
            name: format!("m_{}", i),
            help: format!("metric {}", i),
            kind: MetricsKind::Counter,
            value: i as f64,
        })
        .collect();
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::with_entries(entries));
    let (server, addr) = start_server(c);

    let (_status, _headers, body) = http_get(&addr, "/metrics");
    for i in 0..20 {
        assert!(
            body.contains(&format!("m_{} {}", i, i)),
            "missing metric m_{}",
            i
        );
    }

    server.stop();
}

#[test]
fn http_content_length_header_present() {
    let entries = vec![MetricsEntry {
        name: "x".into(),
        help: "x".into(),
        kind: MetricsKind::Counter,
        value: 1.0,
    }];
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::with_entries(entries));
    let (server, addr) = start_server(c);

    let (_status, headers, _body) = http_get(&addr, "/metrics");
    assert!(
        headers.iter().any(|h| h.starts_with("Content-Length:")),
        "Content-Length header present"
    );

    server.stop();
}

#[test]
fn http_connection_close_header_present() {
    let c: Arc<dyn MetricsCollector> = Arc::new(StubCollector::empty());
    let (server, addr) = start_server(c);

    let (_status, headers, _body) = http_get(&addr, "/health");
    assert!(
        headers.iter().any(|h| h.contains("Connection: close")),
        "Connection: close header present"
    );

    server.stop();
}

#[test]
fn http_collector_invoked_each_scrape() {
    let c = Arc::new(CountingCollector::new());
    let (server, addr) = start_server(c.clone() as Arc<dyn MetricsCollector>);

    // Three sequential scrapes.
    for _ in 0..3 {
        let _ = http_get(&addr, "/metrics");
    }

    assert_eq!(c.call_count(), 3);
    server.stop();
}

#[test]
fn http_health_does_not_invoke_collector() {
    let c = Arc::new(CountingCollector::new());
    let (server, addr) = start_server(c.clone() as Arc<dyn MetricsCollector>);

    let _ = http_get(&addr, "/health");

    assert_eq!(c.call_count(), 0);
    server.stop();
}

#[test]
fn http_404_does_not_invoke_collector() {
    let c = Arc::new(CountingCollector::new());
    let (server, addr) = start_server(c.clone() as Arc<dyn MetricsCollector>);

    let _ = http_get(&addr, "/bogus");

    assert_eq!(c.call_count(), 0);
    server.stop();
}

#[test]
fn http_concurrent_scrapes() {
    let c = Arc::new(CountingCollector::new());
    let (server, addr) = start_server(c.clone() as Arc<dyn MetricsCollector>);

    let threads: Vec<_> = (0..4)
        .map(|_| {
            let a = addr.clone();
            thread::spawn(move || {
                for _ in 0..5 {
                    let (status, _, _) = http_get(&a, "/metrics");
                    assert!(status.contains("200"), "concurrent scrape ok");
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(c.call_count(), 20);
    server.stop();
}

// ===========================================================================
// MetricsCollector trait: custom implementations
// ===========================================================================

struct DynamicCollector {
    value: AtomicU64,
}

impl MetricsCollector for DynamicCollector {
    fn collect(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            entries: vec![MetricsEntry {
                name: "dynamic".into(),
                help: "changes over time".into(),
                kind: MetricsKind::Gauge,
                value: self.value.load(Ordering::Relaxed) as f64,
            }],
        }
    }
}

#[test]
fn http_dynamic_collector_reflects_state_changes() {
    let c = Arc::new(DynamicCollector {
        value: AtomicU64::new(0),
    });
    let (server, addr) = start_server(c.clone() as Arc<dyn MetricsCollector>);

    let (_, _, body1) = http_get(&addr, "/metrics");
    assert!(body1.contains("dynamic 0"));

    c.value.store(99, Ordering::Relaxed);
    let (_, _, body2) = http_get(&addr, "/metrics");
    assert!(body2.contains("dynamic 99"));

    server.stop();
}

// ===========================================================================
// Counter + Gauge integration with Prometheus text
// ===========================================================================

#[test]
fn counter_value_in_snapshot() {
    let c = Counter::new("req_total", "total requests");
    c.add(42);
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: c.name.to_string(),
            help: c.help.to_string(),
            kind: MetricsKind::Counter,
            value: c.get() as f64,
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("req_total 42"));
}

#[test]
fn gauge_value_in_snapshot() {
    let g = Gauge::new("temp", "temperature");
    g.set(36.6);
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: g.name.to_string(),
            help: g.help.to_string(),
            kind: MetricsKind::Gauge,
            value: g.get(),
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("temp 36.6"));
}

// ===========================================================================
// Edge cases in Prometheus formatting
// ===========================================================================

#[test]
fn snapshot_negative_value_rendered() {
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: "drift".into(),
            help: "clock drift".into(),
            kind: MetricsKind::Gauge,
            value: -0.5,
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.contains("drift -0.5"));
}

#[test]
fn snapshot_entry_lines_end_with_newline() {
    let snap = MetricsSnapshot {
        entries: vec![MetricsEntry {
            name: "x".into(),
            help: "x".into(),
            kind: MetricsKind::Counter,
            value: 0.0,
        }],
    };
    let text = snap.to_prometheus_text();
    assert!(text.ends_with('\n'), "text should end with newline");
}

#[test]
fn snapshot_no_blank_lines_between_entries() {
    let snap = MetricsSnapshot {
        entries: vec![
            MetricsEntry {
                name: "a".into(),
                help: "a".into(),
                kind: MetricsKind::Counter,
                value: 1.0,
            },
            MetricsEntry {
                name: "b".into(),
                help: "b".into(),
                kind: MetricsKind::Gauge,
                value: 2.0,
            },
        ],
    };
    let text = snap.to_prometheus_text();
    assert!(!text.contains("\n\n"), "no blank lines between entries");
}
