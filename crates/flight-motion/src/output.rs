// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! UDP output driver for SimTools-compatible motion software.
//!
//! [SimTools](https://www.xsimulator.net/community/faq/simtools.7/) is widely
//! used motion control software that accepts UDP datagrams in the format:
//!
//! ```text
//! A{surge}B{sway}C{heave}D{roll}E{pitch}F{yaw}\n
//! ```
//!
//! where each value is an integer in the range -100..100.
//!
//! This module provides [`SimToolsUdpOutput`] which sends [`MotionFrame`] updates
//! to a SimTools UDP listener.

use crate::frame::MotionFrame;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::net::UdpSocket;

/// Errors from the SimTools UDP output driver.
#[derive(Debug, Error)]
pub enum OutputError {
    #[error("Socket error: {0}")]
    Socket(#[from] std::io::Error),
    #[error("Send failed: sent {sent} of {expected} bytes")]
    SendIncomplete { sent: usize, expected: usize },
}

/// Configuration for SimTools UDP output.
#[derive(Debug, Clone)]
pub struct SimToolsConfig {
    /// Remote address to send datagrams to (default: `127.0.0.1:4123`).
    pub remote_addr: SocketAddr,
    /// Local bind address (default: `0.0.0.0:0`, ephemeral port).
    pub local_addr: SocketAddr,
}

impl Default for SimToolsConfig {
    fn default() -> Self {
        Self {
            remote_addr: "127.0.0.1:4123".parse().unwrap(),
            local_addr: "0.0.0.0:0".parse().unwrap(),
        }
    }
}

/// Sends [`MotionFrame`] data to a SimTools UDP listener.
///
/// ```no_run
/// use flight_motion::output::{SimToolsUdpOutput, SimToolsConfig};
/// use flight_motion::MotionFrame;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut output = SimToolsUdpOutput::bind(SimToolsConfig::default()).await?;
/// output.send(MotionFrame::NEUTRAL).await?;
/// # Ok(())
/// # }
/// ```
pub struct SimToolsUdpOutput {
    socket: UdpSocket,
    config: SimToolsConfig,
}

impl SimToolsUdpOutput {
    /// Bind a UDP socket and return a new output driver.
    pub async fn bind(config: SimToolsConfig) -> Result<Self, OutputError> {
        let socket = UdpSocket::bind(config.local_addr).await?;
        socket.connect(config.remote_addr).await?;
        Ok(Self { socket, config })
    }

    /// Send a motion frame as a SimTools UDP datagram.
    pub async fn send(&self, frame: MotionFrame) -> Result<(), OutputError> {
        let msg = frame.to_simtools_string();
        let bytes = msg.as_bytes();
        let sent = self.socket.send(bytes).await?;
        if sent != bytes.len() {
            return Err(OutputError::SendIncomplete {
                sent,
                expected: bytes.len(),
            });
        }
        tracing::trace!("SimTools UDP sent: {}", msg.trim());
        Ok(())
    }

    /// Returns the configured remote address.
    pub fn remote_addr(&self) -> SocketAddr {
        self.config.remote_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_simtools_send_receive() {
        // Bind a listener to receive the frame
        let listener = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let listener_addr = listener.local_addr().unwrap();

        let config = SimToolsConfig {
            remote_addr: listener_addr,
            local_addr: "127.0.0.1:0".parse().unwrap(),
        };

        let output = SimToolsUdpOutput::bind(config).await.unwrap();

        let frame = MotionFrame {
            surge: 0.5,
            sway: -0.25,
            heave: 1.0,
            roll: 0.0,
            pitch: 0.0,
            yaw: -0.5,
        };

        output.send(frame).await.unwrap();

        let mut buf = [0u8; 64];
        let (n, _) = listener.recv_from(&mut buf).await.unwrap();
        let msg = std::str::from_utf8(&buf[..n]).unwrap();

        assert_eq!(msg, "A50B-25C100D0E0F-50\n");
    }

    #[test]
    fn test_default_remote_addr() {
        let config = SimToolsConfig::default();
        assert_eq!(config.remote_addr.port(), 4123);
    }
}
