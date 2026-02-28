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
        Arc,
        atomic::{AtomicBool, Ordering},
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
                eprintln!(
                    "[FlightHub] Bridge error: {}; retrying in {:?}",
                    e, RECONNECT_DELAY
                );
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
    let stream = TcpStream::connect_timeout(&FLIGHT_HUB_ADDR.parse().unwrap(), HANDSHAKE_TIMEOUT)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut write_stream = stream.try_clone()?;

    let mut reader = BufReader::new(stream);
    eprintln!("[FlightHub] Connected, waiting for handshake…");

    // Read Flight Hub's handshake offer
    let handshake_req = read_message(&mut reader)?;
    match handshake_req {
        PluginMessage::Handshake {
            version,
            capabilities,
        } => {
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
    use crate::xplm;

    match msg {
        PluginMessage::GetAircraftInfo { id } => {
            let icao = xplm::read_dataref_string("sim/aircraft/view/acf_ICAO")
                .unwrap_or_else(|| "UNKN".to_string());
            let title =
                xplm::read_dataref_string("sim/aircraft/view/acf_descrip").unwrap_or_default();
            let author =
                xplm::read_dataref_string("sim/aircraft/view/acf_author").unwrap_or_default();
            let file_path = xplm::read_dataref_string("sim/aircraft/view/acf_relative_path")
                .unwrap_or_default();
            Some(PluginResponse::AircraftInfo {
                id,
                icao,
                title,
                author,
                file_path,
            })
        }
        PluginMessage::Ping { id, timestamp } => Some(PluginResponse::Pong { id, timestamp }),
        PluginMessage::GetDataRef { id, name } => match xplm::read_dataref(&name) {
            Some(value) => Some(PluginResponse::DataRefValue {
                id,
                name,
                value,
                timestamp: xplm::timestamp_ms(),
            }),
            None => Some(PluginResponse::Error {
                id: Some(id),
                error: format!("DataRef not found: {}", name),
                details: None,
            }),
        },
        PluginMessage::SetDataRef { id, name, value } => {
            let success = xplm::write_dataref(&name, &value);
            Some(PluginResponse::CommandResult {
                id,
                success,
                message: if success {
                    None
                } else {
                    Some(format!("Failed to write DataRef: {}", name))
                },
            })
        }
        PluginMessage::Command { id, name } => {
            let success = xplm::execute_command(&name);
            Some(PluginResponse::CommandResult {
                id,
                success,
                message: if success {
                    None
                } else {
                    Some(format!("Command not found: {}", name))
                },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xplm::{self, MockValue};

    #[test]
    fn get_dataref_returns_float_value() {
        xplm::clear_mocks();
        xplm::set_mock_dataref("sim/cockpit/autopilot/altitude", MockValue::Float(35000.0));

        let msg = PluginMessage::GetDataRef {
            id: 1,
            name: "sim/cockpit/autopilot/altitude".to_string(),
        };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::DataRefValue {
                id, name, value, ..
            } => {
                assert_eq!(id, 1);
                assert_eq!(name, "sim/cockpit/autopilot/altitude");
                assert!((value.as_f64().unwrap() - 35000.0).abs() < 0.1);
            }
            other => panic!("Expected DataRefValue, got {:?}", other),
        }
    }

    #[test]
    fn get_dataref_returns_error_when_not_found() {
        xplm::clear_mocks();

        let msg = PluginMessage::GetDataRef {
            id: 2,
            name: "sim/nonexistent/dataref".to_string(),
        };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::Error { id, error, .. } => {
                assert_eq!(id, Some(2));
                assert!(error.contains("not found"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn set_dataref_succeeds_when_dataref_exists() {
        xplm::clear_mocks();
        xplm::set_mock_dataref("sim/test/writable", MockValue::Float(0.0));

        let msg = PluginMessage::SetDataRef {
            id: 3,
            name: "sim/test/writable".to_string(),
            value: serde_json::json!(123.4),
        };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::CommandResult {
                id,
                success,
                message,
            } => {
                assert_eq!(id, 3);
                assert!(success);
                assert!(message.is_none());
            }
            other => panic!("Expected CommandResult, got {:?}", other),
        }
    }

    #[test]
    fn set_dataref_fails_when_not_found() {
        xplm::clear_mocks();

        let msg = PluginMessage::SetDataRef {
            id: 4,
            name: "sim/nonexistent/dataref".to_string(),
            value: serde_json::json!(1.0),
        };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::CommandResult {
                id,
                success,
                message,
            } => {
                assert_eq!(id, 4);
                assert!(!success);
                assert!(message.is_some());
            }
            other => panic!("Expected CommandResult, got {:?}", other),
        }
    }

    #[test]
    fn command_executes_and_records() {
        xplm::clear_mocks();

        let msg = PluginMessage::Command {
            id: 5,
            name: "sim/autopilot/heading_sync".to_string(),
        };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::CommandResult {
                id,
                success,
                message,
            } => {
                assert_eq!(id, 5);
                assert!(success);
                assert!(message.is_none());
            }
            other => panic!("Expected CommandResult, got {:?}", other),
        }
        let cmds = xplm::executed_commands();
        assert_eq!(cmds, vec!["sim/autopilot/heading_sync"]);
    }

    #[test]
    fn get_aircraft_info_reads_datarefs() {
        xplm::clear_mocks();
        xplm::set_mock_dataref(
            "sim/aircraft/view/acf_ICAO",
            MockValue::Data(b"B738\0".to_vec()),
        );
        xplm::set_mock_dataref(
            "sim/aircraft/view/acf_descrip",
            MockValue::Data(b"Boeing 737-800\0".to_vec()),
        );
        xplm::set_mock_dataref(
            "sim/aircraft/view/acf_author",
            MockValue::Data(b"Zibo\0".to_vec()),
        );
        xplm::set_mock_dataref(
            "sim/aircraft/view/acf_relative_path",
            MockValue::Data(b"Aircraft/B738/B738.acf\0".to_vec()),
        );

        let msg = PluginMessage::GetAircraftInfo { id: 6 };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::AircraftInfo {
                id,
                icao,
                title,
                author,
                file_path,
            } => {
                assert_eq!(id, 6);
                assert_eq!(icao, "B738");
                assert_eq!(title, "Boeing 737-800");
                assert_eq!(author, "Zibo");
                assert_eq!(file_path, "Aircraft/B738/B738.acf");
            }
            other => panic!("Expected AircraftInfo, got {:?}", other),
        }
    }

    #[test]
    fn get_aircraft_info_defaults_when_datarefs_missing() {
        xplm::clear_mocks();

        let msg = PluginMessage::GetAircraftInfo { id: 7 };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::AircraftInfo {
                id,
                icao,
                title,
                author,
                file_path,
            } => {
                assert_eq!(id, 7);
                assert_eq!(icao, "UNKN");
                assert!(title.is_empty());
                assert!(author.is_empty());
                assert!(file_path.is_empty());
            }
            other => panic!("Expected AircraftInfo, got {:?}", other),
        }
    }

    #[test]
    fn ping_returns_pong() {
        let msg = PluginMessage::Ping {
            id: 8,
            timestamp: 1234567890,
        };
        let response = handle_message(msg).unwrap();
        match response {
            PluginResponse::Pong { id, timestamp } => {
                assert_eq!(id, 8);
                assert_eq!(timestamp, 1234567890);
            }
            other => panic!("Expected Pong, got {:?}", other),
        }
    }

    #[test]
    fn handshake_returns_none() {
        let msg = PluginMessage::Handshake {
            version: "1.0".to_string(),
            capabilities: vec![],
        };
        assert!(handle_message(msg).is_none());
    }

    #[test]
    fn get_dataref_timestamp_is_nonzero() {
        xplm::clear_mocks();
        xplm::set_mock_dataref("sim/test/ts", MockValue::Int(42));

        let msg = PluginMessage::GetDataRef {
            id: 9,
            name: "sim/test/ts".to_string(),
        };
        let response = handle_message(msg).unwrap();
        if let PluginResponse::DataRefValue { timestamp, .. } = response {
            assert!(timestamp > 0, "timestamp should be non-zero");
        } else {
            panic!("Expected DataRefValue");
        }
    }
}
