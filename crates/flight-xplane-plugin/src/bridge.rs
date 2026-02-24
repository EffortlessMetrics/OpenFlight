// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! TCP bridge: connects to the Flight Hub plugin interface (localhost:52000).
//!
//! Runs in a detached background thread to avoid blocking X-Plane's main loop.
//! Implements the same newline-delimited JSON protocol as `flight-xplane/src/plugin.rs`.

use crate::protocol::{PluginMessage, PluginResponse};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use thiserror::Error;

const FLIGHT_HUB_ADDR: &str = "127.0.0.1:52000";
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors that can occur in the bridge
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum BridgeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Handshake timeout")]
    Timeout,
}

/// Manages the background thread that talks to Flight Hub.
pub struct Bridge {
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Bridge {
    /// Spawn the bridge background thread.
    pub fn start() -> Result<Self, BridgeError> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = thread::Builder::new()
            .name("flighthub-bridge".to_string())
            .spawn(move || {
                run_bridge_loop(running_clone);
            })
            .map_err(BridgeError::Io)?;

        Ok(Self {
            running,
            thread: Some(handle),
        })
    }

    /// Signal the background thread to stop and wait for it to exit.
    pub fn shutdown(mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Main reconnect loop — keeps retrying until `running` is cleared.
fn run_bridge_loop(running: Arc<AtomicBool>) {
    while running.load(Ordering::Acquire) {
        match connect_and_run(&running) {
            Ok(()) => {
                eprintln!("[FlightHub] Bridge disconnected normally");
            }
            Err(e) => {
                eprintln!("[FlightHub] Bridge error: {}; retrying in {:?}", e, RECONNECT_DELAY);
            }
        }

        if running.load(Ordering::Acquire) {
            thread::sleep(RECONNECT_DELAY);
        }
    }
}

/// Establish one TCP connection and drive the message loop until disconnected.
fn connect_and_run(running: &AtomicBool) -> Result<(), BridgeError> {
    eprintln!("[FlightHub] Connecting to {}", FLIGHT_HUB_ADDR);
    let stream = TcpStream::connect_timeout(
        &FLIGHT_HUB_ADDR.parse().unwrap(),
        HANDSHAKE_TIMEOUT,
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut write_stream = stream.try_clone()?;

    let mut reader = BufReader::new(stream);
    eprintln!("[FlightHub] Connected, waiting for handshake…");

    // Read Flight Hub's handshake offer
    let handshake_req = read_message(&mut reader)?;
    match handshake_req {
        PluginMessage::Handshake { version, capabilities } => {
            eprintln!(
                "[FlightHub] Handshake received: version={}, caps={:?}",
                version, capabilities
            );
        }
        other => {
            return Err(BridgeError::Protocol(format!(
                "Expected Handshake, got {:?}",
                other
            )));
        }
    }

    // Send HandshakeAck
    let ack = PluginResponse::HandshakeAck {
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec![
            "read_datarefs".to_string(),
            "write_datarefs".to_string(),
            "execute_commands".to_string(),
            "aircraft_info".to_string(),
        ],
        status: "ready".to_string(),
    };
    send_message(&mut write_stream, &ack)?;
    eprintln!("[FlightHub] Handshake complete");

    // Message loop
    while running.load(Ordering::Acquire) {
        let msg = match read_message(&mut reader) {
            Ok(m) => m,
            Err(BridgeError::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => return Err(e),
        };

        if let Some(response) = handle_message(msg) {
            send_message(&mut write_stream, &response)?;
        }
    }

    Ok(())
}

/// Handle an incoming request from Flight Hub and return an optional response.
fn handle_message(msg: PluginMessage) -> Option<PluginResponse> {
    match msg {
        PluginMessage::GetAircraftInfo { id } => {
            // TODO: call XPLM DataRef APIs once SDK is linked
            Some(PluginResponse::AircraftInfo {
                id,
                icao: "UNKN".to_string(),
                title: String::new(),
                author: String::new(),
                file_path: String::new(),
            })
        }
        PluginMessage::Ping { id, timestamp } => Some(PluginResponse::Pong { id, timestamp }),
        PluginMessage::GetDataRef { id, name } => {
            // Stub: XPLM DataRef reads will be wired here once the SDK is linked.
            eprintln!("[FlightHub] GetDataRef: {} (XPLM stub, returning 0)", name);
            Some(PluginResponse::DataRefValue {
                id,
                name,
                value: serde_json::json!(0.0),
                timestamp: 0,
            })
        }
        PluginMessage::SetDataRef { id, name, value } => {
            // Stub: XPLM DataRef writes will be wired here once the SDK is linked.
            eprintln!("[FlightHub] SetDataRef: {} = {} (XPLM stub)", name, value);
            Some(PluginResponse::CommandResult {
                id,
                success: true,
                message: None,
            })
        }
        PluginMessage::Command { id, name } => {
            // Stub: XPLMCommandRef execution will be wired here once the SDK is linked.
            eprintln!("[FlightHub] Command: {} (XPLM stub)", name);
            Some(PluginResponse::CommandResult {
                id,
                success: true,
                message: None,
            })
        }
        _ => {
            // Handshake, Subscribe, Unsubscribe — no response needed here
            None
        }
    }
}

fn send_message(stream: &mut TcpStream, msg: &PluginResponse) -> Result<(), BridgeError> {
    let json = serde_json::to_string(msg)?;
    writeln!(stream, "{}", json)?;
    Ok(())
}

fn read_message(reader: &mut BufReader<TcpStream>) -> Result<PluginMessage, BridgeError> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(BridgeError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "connection closed",
        )));
    }
    Ok(serde_json::from_str(line.trim())?)
}
