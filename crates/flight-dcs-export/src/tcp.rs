// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! TCP connection mode for DCS Export adapter.
//!
//! Provides a TCP server that accepts connections from the DCS export script,
//! as an alternative to UDP mode.

use std::io::{self, BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Configuration for TCP mode.
#[derive(Debug, Clone)]
pub struct TcpConfig {
    /// Bind address (default: "127.0.0.1:9789").
    pub bind_address: String,
    /// Connection timeout.
    pub accept_timeout: Duration,
    /// Read timeout per connected client.
    pub read_timeout: Duration,
    /// Maximum number of reconnections before giving up.
    pub max_reconnects: Option<u32>,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:9789".to_string(),
            accept_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(2),
            max_reconnects: None,
        }
    }
}

/// Statistics for the TCP connection.
#[derive(Debug, Default)]
pub struct TcpStats {
    /// Total number of connections accepted.
    pub connections_accepted: AtomicU64,
    /// Total number of lines received.
    pub lines_received: AtomicU64,
    /// Total number of reconnection events.
    pub reconnects: AtomicU64,
    /// Number of parse errors.
    pub parse_errors: AtomicU64,
}

impl TcpStats {
    /// Create new zeroed stats.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

/// Parses a single DCS export line into key-value pair.
///
/// DCS export lines have the format: `key=value`
/// Returns `None` for malformed lines.
pub fn parse_dcs_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let eq = line.find('=')?;
    let key = &line[..eq];
    let val = &line[eq + 1..];
    Some((key, val))
}

/// TCP server for DCS export data.
pub struct DcsTcpServer {
    config: TcpConfig,
    stats: Arc<TcpStats>,
    running: Arc<AtomicBool>,
}

impl DcsTcpServer {
    /// Create a new TCP server.
    pub fn new(config: TcpConfig) -> Self {
        Self {
            config,
            stats: TcpStats::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Statistics for this server.
    pub fn stats(&self) -> Arc<TcpStats> {
        Arc::clone(&self.stats)
    }

    /// Returns true if the server is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Stop the server.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Read lines from a TCP connection, calling `on_line` for each.
    ///
    /// Returns when the connection is closed or an error occurs.
    pub fn read_connection<F>(&self, stream: TcpStream, mut on_line: F) -> io::Result<()>
    where
        F: FnMut(&str, &str),
    {
        stream.set_read_timeout(Some(self.config.read_timeout))?;
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            if !self.running.load(Ordering::Acquire) {
                break;
            }
            match line {
                Ok(l) => {
                    self.stats.lines_received.fetch_add(1, Ordering::Relaxed);
                    if let Some((key, val)) = parse_dcs_line(&l) {
                        on_line(key, val);
                    } else {
                        self.stats.parse_errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    // read timeout — check if still running
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Accept one connection from the TCP listener.
    pub fn accept_connection(&self, listener: &TcpListener) -> io::Result<TcpStream> {
        listener.set_nonblocking(false)?;
        let (stream, _addr) = listener.accept()?;
        self.stats
            .connections_accepted
            .fetch_add(1, Ordering::Relaxed);
        Ok(stream)
    }

    /// Create a listener bound to the configured address.
    pub fn bind(&self) -> io::Result<TcpListener> {
        self.running.store(true, Ordering::Release);
        TcpListener::bind(&self.config.bind_address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normal_line() {
        let (k, v) = parse_dcs_line("speed=350.5").unwrap();
        assert_eq!(k, "speed");
        assert_eq!(v, "350.5");
    }

    #[test]
    fn parse_ignores_blank_lines() {
        assert!(parse_dcs_line("   ").is_none());
        assert!(parse_dcs_line("").is_none());
    }

    #[test]
    fn parse_ignores_comments() {
        assert!(parse_dcs_line("# comment").is_none());
    }

    #[test]
    fn parse_ignores_lines_without_equals() {
        assert!(parse_dcs_line("noequalssign").is_none());
    }

    #[test]
    fn parse_value_can_contain_equals() {
        let (k, v) = parse_dcs_line("key=a=b=c").unwrap();
        assert_eq!(k, "key");
        assert_eq!(v, "a=b=c");
    }

    #[test]
    fn parse_trims_whitespace() {
        // trim() is applied to the whole line before splitting, so inner
        // spaces around '=' are preserved as-is in key and value.
        let (k, v) = parse_dcs_line("  pitch = 12.5  ").unwrap();
        assert_eq!(k, "pitch ");
        assert_eq!(v, " 12.5");
    }

    #[test]
    fn tcp_config_default() {
        let c = TcpConfig::default();
        assert!(c.bind_address.contains("9789"));
        assert!(c.max_reconnects.is_none());
    }

    #[test]
    fn tcp_server_starts_not_running() {
        let server = DcsTcpServer::new(TcpConfig::default());
        assert!(!server.is_running());
    }

    #[test]
    fn tcp_server_stop_sets_not_running() {
        let server = DcsTcpServer::new(TcpConfig::default());
        server.running.store(true, Ordering::Release);
        assert!(server.is_running());
        server.stop();
        assert!(!server.is_running());
    }

    #[test]
    fn tcp_stats_start_at_zero() {
        let stats = TcpStats::new();
        assert_eq!(stats.connections_accepted.load(Ordering::Relaxed), 0);
        assert_eq!(stats.lines_received.load(Ordering::Relaxed), 0);
    }
}
