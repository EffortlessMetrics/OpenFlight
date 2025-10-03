// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SimConnect session management
//!
//! Provides connection management, message dispatching, and error handling
//! for SimConnect communication with Microsoft Flight Simulator.

use flight_simconnect_sys::{
    constants::*, SimConnectApi, SimConnectError, HSIMCONNECT, SIMCONNECT_RECV,
    SIMCONNECT_RECV_EXCEPTION, SIMCONNECT_RECV_OPEN, SIMCONNECT_RECV_SIMOBJECT_DATA,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use windows::Win32::Foundation::{HANDLE, HWND};

/// Configuration for SimConnect session
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SessionConfig {
    /// Application name for SimConnect
    pub app_name: String,
    /// SimConnect configuration index (0 for default)
    pub config_index: u32,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Message polling interval
    pub poll_interval: Duration,
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Reconnection delay
    pub reconnect_delay: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            app_name: "Flight Hub".to_string(),
            config_index: 0,
            connect_timeout: Duration::from_secs(10),
            poll_interval: Duration::from_millis(16), // ~60Hz
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(2),
        }
    }
}

/// SimConnect session events
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Connection established
    Connected {
        app_name: String,
        app_version: (u32, u32, u32, u32),
        simconnect_version: (u32, u32, u32, u32),
    },
    /// Connection lost
    Disconnected,
    /// Exception occurred
    Exception {
        exception: u32,
        send_id: u32,
        index: u32,
    },
    /// Data received
    DataReceived {
        request_id: u32,
        object_id: u32,
        define_id: u32,
        data: Vec<u8>,
    },
    /// Event received
    EventReceived {
        group_id: u32,
        event_id: u32,
        data: u32,
    },
}

/// SimConnect session error types
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("SimConnect API error: {0}")]
    SimConnect(#[from] SimConnectError),
    #[error("Connection timeout")]
    Timeout,
    #[error("Not connected")]
    NotConnected,
    #[error("Connection lost")]
    ConnectionLost,
    #[error("Invalid message format")]
    InvalidMessage,
    #[error("Channel error: {0}")]
    Channel(String),
}

/// SimConnect session manager
pub struct SimConnectSession {
    api: Arc<SimConnectApi>,
    config: SessionConfig,
    handle: Option<HSIMCONNECT>,
    event_sender: mpsc::UnboundedSender<SessionEvent>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<SessionEvent>>>,
    connected: bool,
    last_poll: Instant,
    reconnect_attempts: u32,
}

impl SimConnectSession {
    /// Create a new SimConnect session
    pub fn new(config: SessionConfig) -> Result<Self, SessionError> {
        let api = Arc::new(SimConnectApi::new()?);
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Ok(Self {
            api,
            config,
            handle: None,
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            connected: false,
            last_poll: Instant::now(),
            reconnect_attempts: 0,
        })
    }

    /// Connect to SimConnect
    pub async fn connect(&mut self) -> Result<(), SessionError> {
        if self.connected {
            return Ok(());
        }

        info!("Connecting to SimConnect...");

        let start_time = Instant::now();
        while start_time.elapsed() < self.config.connect_timeout {
            match self.try_connect() {
                Ok(handle) => {
                    self.handle = Some(handle);
                    self.connected = true;
                    self.reconnect_attempts = 0;
                    info!("Connected to SimConnect successfully");
                    return Ok(());
                }
                Err(e) => {
                    debug!("Connection attempt failed: {}", e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        Err(SessionError::Timeout)
    }

    /// Disconnect from SimConnect
    pub fn disconnect(&mut self) -> Result<(), SessionError> {
        if let Some(handle) = self.handle.take() {
            self.api.close(handle)?;
            self.connected = false;
            info!("Disconnected from SimConnect");
            
            // Send disconnect event
            let _ = self.event_sender.send(SessionEvent::Disconnected);
        }
        Ok(())
    }

    /// Check if connected to SimConnect
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get event receiver for session events
    pub fn event_receiver(&self) -> Arc<Mutex<mpsc::UnboundedReceiver<SessionEvent>>> {
        self.event_receiver.clone()
    }

    /// Poll for SimConnect messages
    pub async fn poll(&mut self) -> Result<(), SessionError> {
        if !self.connected {
            return Err(SessionError::NotConnected);
        }

        // Rate limit polling
        let now = Instant::now();
        if now.duration_since(self.last_poll) < self.config.poll_interval {
            return Ok(());
        }
        self.last_poll = now;

        let handle = self.handle.ok_or(SessionError::NotConnected)?;

        // Process all available messages
        loop {
            match self.api.get_next_dispatch(handle) {
                Ok(Some(data)) => {
                    if let Err(e) = self.process_message(data) {
                        warn!("Error processing SimConnect message: {}", e);
                    }
                }
                Ok(None) => {
                    // No more messages
                    break;
                }
                Err(SimConnectError::ApiError(val)) if val == 0x80004005u32 as i32 => {
                    // E_FAIL - connection lost
                    error!("SimConnect connection lost");
                    self.handle = None;
                    self.connected = false;
                    let _ = self.event_sender.send(SessionEvent::Disconnected);
                    return Err(SessionError::ConnectionLost);
                }
                Err(e) => {
                    error!("SimConnect polling error: {}", e);
                    return Err(SessionError::SimConnect(e));
                }
            }
        }

        Ok(())
    }

    /// Attempt reconnection if disconnected
    pub async fn try_reconnect(&mut self) -> Result<(), SessionError> {
        if self.connected || self.reconnect_attempts >= self.config.max_reconnect_attempts {
            return Ok(());
        }

        self.reconnect_attempts += 1;
        info!("Attempting reconnection {} of {}", self.reconnect_attempts, self.config.max_reconnect_attempts);

        tokio::time::sleep(self.config.reconnect_delay).await;
        
        match self.connect().await {
            Ok(()) => {
                info!("Reconnection successful");
                Ok(())
            }
            Err(e) => {
                warn!("Reconnection attempt {} failed: {}", self.reconnect_attempts, e);
                Err(e)
            }
        }
    }

    /// Get SimConnect handle for direct API access
    pub fn handle(&self) -> Option<HSIMCONNECT> {
        self.handle
    }

    /// Get SimConnect API reference
    pub fn api(&self) -> &SimConnectApi {
        &self.api
    }

    fn try_connect(&self) -> Result<HSIMCONNECT, SessionError> {
        self.api.open(
            &self.config.app_name,
            HWND::default(),
            0,
            HANDLE::default(),
            self.config.config_index,
        ).map_err(SessionError::SimConnect)
    }

    fn process_message(&self, data: Vec<u8>) -> Result<(), SessionError> {
        if data.len() < std::mem::size_of::<SIMCONNECT_RECV>() {
            return Err(SessionError::InvalidMessage);
        }

        let recv = unsafe { &*(data.as_ptr() as *const SIMCONNECT_RECV) };
        
        match recv.dwID {
            id if id == SIMCONNECT_RECV_ID::OPEN as u32 => {
                self.handle_open_message(&data)?;
            }
            id if id == SIMCONNECT_RECV_ID::EXCEPTION as u32 => {
                self.handle_exception_message(&data)?;
            }
            id if id == SIMCONNECT_RECV_ID::SIMOBJECT_DATA as u32 => {
                self.handle_data_message(&data)?;
            }
            id if id == SIMCONNECT_RECV_ID::EVENT as u32 => {
                self.handle_event_message(&data)?;
            }
            id if id == SIMCONNECT_RECV_ID::QUIT as u32 => {
                info!("SimConnect quit message received");
                let _ = self.event_sender.send(SessionEvent::Disconnected);
            }
            _ => {
                debug!("Unhandled SimConnect message type: {}", recv.dwID);
            }
        }

        Ok(())
    }

    fn handle_open_message(&self, data: &[u8]) -> Result<(), SessionError> {
        if data.len() < std::mem::size_of::<SIMCONNECT_RECV_OPEN>() {
            return Err(SessionError::InvalidMessage);
        }

        let open_msg = unsafe { &*(data.as_ptr() as *const SIMCONNECT_RECV_OPEN) };
        
        // Extract application name (null-terminated)
        let app_name = unsafe {
            std::ffi::CStr::from_ptr(open_msg.szApplicationName.as_ptr())
                .to_string_lossy()
                .to_string()
        };

        let event = SessionEvent::Connected {
            app_name,
            app_version: (
                open_msg.dwApplicationVersionMajor,
                open_msg.dwApplicationVersionMinor,
                open_msg.dwApplicationBuildMajor,
                open_msg.dwApplicationBuildMinor,
            ),
            simconnect_version: (
                open_msg.dwSimConnectVersionMajor,
                open_msg.dwSimConnectVersionMinor,
                open_msg.dwSimConnectBuildMajor,
                open_msg.dwSimConnectBuildMinor,
            ),
        };

        let _ = self.event_sender.send(event);
        Ok(())
    }

    fn handle_exception_message(&self, data: &[u8]) -> Result<(), SessionError> {
        if data.len() < std::mem::size_of::<SIMCONNECT_RECV_EXCEPTION>() {
            return Err(SessionError::InvalidMessage);
        }

        let exception_msg = unsafe { &*(data.as_ptr() as *const SIMCONNECT_RECV_EXCEPTION) };
        
        let event = SessionEvent::Exception {
            exception: exception_msg.dwException,
            send_id: exception_msg.dwSendID,
            index: exception_msg.dwIndex,
        };

        let _ = self.event_sender.send(event);
        Ok(())
    }

    fn handle_data_message(&self, data: &[u8]) -> Result<(), SessionError> {
        if data.len() < std::mem::size_of::<SIMCONNECT_RECV_SIMOBJECT_DATA>() {
            return Err(SessionError::InvalidMessage);
        }

        let data_msg = unsafe { &*(data.as_ptr() as *const SIMCONNECT_RECV_SIMOBJECT_DATA) };
        
        // Extract the actual data payload
        let data_offset = std::mem::size_of::<SIMCONNECT_RECV_SIMOBJECT_DATA>();
        let payload = if data.len() > data_offset {
            data[data_offset..].to_vec()
        } else {
            Vec::new()
        };

        let event = SessionEvent::DataReceived {
            request_id: data_msg.dwRequestID,
            object_id: data_msg.dwObjectID,
            define_id: data_msg.dwDefineID,
            data: payload,
        };

        let _ = self.event_sender.send(event);
        Ok(())
    }

    fn handle_event_message(&self, data: &[u8]) -> Result<(), SessionError> {
        if data.len() < std::mem::size_of::<flight_simconnect_sys::SIMCONNECT_RECV_EVENT>() {
            return Err(SessionError::InvalidMessage);
        }

        let event_msg = unsafe { &*(data.as_ptr() as *const flight_simconnect_sys::SIMCONNECT_RECV_EVENT) };
        
        let event = SessionEvent::EventReceived {
            group_id: event_msg.uGroupID,
            event_id: event_msg.uEventID,
            data: event_msg.dwData,
        };

        let _ = self.event_sender.send(event);
        Ok(())
    }
}

impl Drop for SimConnectSession {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert_eq!(config.app_name, "Flight Hub");
        assert_eq!(config.config_index, 0);
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.poll_interval, Duration::from_millis(16));
    }

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        
        // This test will only pass if SimConnect.dll is available
        match SimConnectSession::new(config) {
            Ok(session) => {
                assert!(!session.is_connected());
                assert!(session.handle().is_none());
            }
            Err(SessionError::SimConnect(SimConnectError::LibraryNotFound)) => {
                // Expected on systems without MSFS/SimConnect
                println!("SimConnect library not found - this is expected on systems without MSFS");
            }
            Err(e) => {
                panic!("Unexpected error creating session: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_session_connect_timeout() {
        let mut config = SessionConfig::default();
        config.connect_timeout = Duration::from_millis(100);
        
        match SimConnectSession::new(config) {
            Ok(mut session) => {
                // This should timeout since MSFS is likely not running
                let result = session.connect().await;
                match result {
                    Err(SessionError::Timeout) => {
                        // Expected behavior
                    }
                    Err(SessionError::SimConnect(SimConnectError::LibraryNotFound)) => {
                        // Also expected on systems without SimConnect
                        println!("SimConnect library not found");
                    }
                    Ok(()) => {
                        // Unexpected success - MSFS must be running
                        println!("Unexpected successful connection - MSFS is running");
                        let _ = session.disconnect();
                    }
                    Err(e) => {
                        println!("Other connection error: {}", e);
                    }
                }
            }
            Err(SessionError::SimConnect(SimConnectError::LibraryNotFound)) => {
                println!("SimConnect library not found");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }
}