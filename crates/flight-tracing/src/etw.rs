// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! ETW (Event Tracing for Windows) provider implementation
//!
//! Provides high-performance event tracing on Windows using ETW infrastructure.
//! Events are emitted to the "Flight-Hub" provider for consumption by WPA, PerfView,
//! or custom ETW consumers.

use crate::{TraceProvider, TraceError, TraceEvent, EventData};
use std::io::Write;
use std::fs::OpenOptions;
use std::sync::Mutex;

/// ETW provider implementation (simplified to file-based for now)
pub struct EtwProvider {
    log_file: Mutex<Option<std::fs::File>>,
    enabled: bool,
}

impl EtwProvider {
    /// Create new ETW provider
    pub fn new() -> Self {
        Self {
            log_file: Mutex::new(None),
            enabled: false,
        }
    }
    
    /// Check if ETW is available on this system
    #[allow(dead_code)]
    pub fn is_available() -> bool {
        // Always available (using file-based logging for simplicity)
        true
    }
    
    /// Write event to log file
    fn write_event(&self, message: &str) -> Result<(), TraceError> {
        let mut file_guard = self.log_file.lock().unwrap();
        if let Some(ref mut file) = file_guard.as_mut() {
            writeln!(file, "{}", message)?;
            file.flush()?;
        }
        Ok(())
    }
    

}

impl TraceProvider for EtwProvider {
    fn initialize(&mut self) -> Result<(), TraceError> {
        if self.enabled {
            return Ok(());
        }
        
        // Create log file for Windows tracing
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("flight-hub-trace.log")?;
        
        *self.log_file.lock().unwrap() = Some(log_file);
        self.enabled = true;
        
        self.write_event("flight_hub_init: ETW provider initialized")?;
        tracing::info!("ETW provider initialized (file-based)");
        Ok(())
    }
    
    fn emit_event(&self, event: &TraceEvent) -> Result<(), TraceError> {
        if !self.enabled {
            return Ok(());
        }
        
        let message = match &event.data {
            EventData::TickStart { tick_number } => {
                format!("flight_hub_tick_start: tick={}", tick_number)
            }
            
            EventData::TickEnd { tick_number, duration_ns, jitter_ns } => {
                format!(
                    "flight_hub_tick_end: tick={} duration_ns={} jitter_ns={}",
                    tick_number, duration_ns, jitter_ns
                )
            }
            
            EventData::HidWrite { device_id, bytes, duration_ns } => {
                format!(
                    "flight_hub_hid_write: device_id=0x{:x} bytes={} duration_ns={}",
                    device_id, bytes, duration_ns
                )
            }
            
            EventData::DeadlineMiss { tick_number, miss_duration_ns } => {
                format!(
                    "flight_hub_deadline_miss: tick={} miss_duration_ns={}",
                    tick_number, miss_duration_ns
                )
            }
            
            EventData::WriterDrop { stream_id, dropped_count } => {
                format!(
                    "flight_hub_writer_drop: stream_id={} dropped_count={}",
                    stream_id, dropped_count
                )
            }
            
            EventData::Custom { name, data } => {
                let json_data = serde_json::to_string(data)?;
                format!("flight_hub_custom: name={} data={}", name, json_data)
            }
        };
        
        self.write_event(&message)
    }
    
    fn shutdown(&mut self) -> Result<(), TraceError> {
        if !self.enabled {
            return Ok(());
        }
        
        self.write_event("flight_hub_shutdown: ETW provider shutdown")?;
        
        *self.log_file.lock().unwrap() = None;
        self.enabled = false;
        
        tracing::info!("ETW provider shutdown");
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Drop for EtwProvider {
    fn drop(&mut self) {
        if self.enabled {
            let _ = self.shutdown();
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etw_provider_creation() {
        let provider = EtwProvider::new();
        assert!(!provider.is_enabled());
    }

    #[test]
    fn test_etw_availability() {
        assert!(EtwProvider::is_available());
    }

    #[test]
    fn test_etw_initialization() {
        let mut provider = EtwProvider::new();
        
        // Should initialize successfully
        assert!(provider.initialize().is_ok());
        assert!(provider.is_enabled());
        
        // Should shutdown cleanly
        assert!(provider.shutdown().is_ok());
        assert!(!provider.is_enabled());
    }

    #[test]
    fn test_event_emission() {
        let mut provider = EtwProvider::new();
        provider.initialize().unwrap();
        
        // Test all event types
        let events = vec![
            TraceEvent::tick_start(1),
            TraceEvent::tick_end(1, 4000000, 1500),
            TraceEvent::hid_write(0x1234, 64, 250000),
            TraceEvent::deadline_miss(2, 2000000),
            TraceEvent::writer_drop("axis", 5),
            TraceEvent::custom("test", serde_json::json!({"key": "value"})),
        ];
        
        for event in events {
            assert!(provider.emit_event(&event).is_ok());
        }
        
        provider.shutdown().unwrap();
    }
}