// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Transport layer for SimConnect communication
//!
//! Provides transport abstractions for SimConnect data exchange.

use thiserror::Error;

/// Transport layer error types
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Timeout error: {0}")]
    Timeout(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_display() {
        let err = TransportError::Connection("host unreachable".to_string());
        assert_eq!(err.to_string(), "Connection error: host unreachable");
    }

    #[test]
    fn test_protocol_error_display() {
        let err = TransportError::Protocol("unexpected packet header".to_string());
        assert_eq!(err.to_string(), "Protocol error: unexpected packet header");
    }

    #[test]
    fn test_timeout_error_display() {
        let err = TransportError::Timeout("read timed out after 5s".to_string());
        assert_eq!(err.to_string(), "Timeout error: read timed out after 5s");
    }

    #[test]
    fn test_io_error_from_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err = TransportError::from(io_err);
        assert!(
            err.to_string().starts_with("IO error:"),
            "Expected 'IO error:' prefix, got: {}",
            err
        );
    }

    #[test]
    fn test_io_error_broken_pipe_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe");
        let err = TransportError::Io(io_err);
        let msg = err.to_string();
        assert!(msg.starts_with("IO error:"), "got: {}", msg);
    }

    #[test]
    fn test_error_variants_are_debug() {
        let err = TransportError::Connection("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Connection"));

        let err = TransportError::Protocol("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Protocol"));

        let err = TransportError::Timeout("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Timeout"));
    }
}
