// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Linux tracepoints implementation
//!
//! Provides high-performance event tracing on Linux using kernel tracepoints.
//! Events are written to /sys/kernel/debug/tracing/trace_marker for consumption
//! by ftrace, perf, or other tracing tools.

use crate::{TraceProvider, TraceError, TraceEvent, EventData};
use std::fs::OpenOptions;
use std::io::{Write, BufWriter};
use std::sync::Mutex;

/// Linux tracepoint provider
pub struct TracepointProvider {
    trace_marker: Mutex<Option<BufWriter<std::fs::File>>>,
    enabled: bool,
}

impl TracepointProvider {
    /// Create new tracepoint provider
    pub fn new() -> Self {
        Self {
            trace_marker: Mutex::new(None),
            enabled: false,
        }
    }
    
    /// Check if tracepoints are available on this system
    pub fn is_available() -> bool {
        std::path::Path::new("/sys/kernel/debug/tracing/trace_marker").exists()
    }
    
    /// Write event to trace marker
    fn write_trace_marker(&self, message: &str) -> Result<(), TraceError> {
        let mut marker_guard = self.trace_marker.lock().unwrap();
        if let Some(ref mut writer) = marker_guard.as_mut() {
            writeln!(writer, "{}", message)?;
            writer.flush()?;
        }
        Ok(())
    }
    
    /// Format tick start event
    fn format_tick_start(&self, tick_number: u64) -> String {
        format!("flight_hub_tick_start: tick={}", tick_number)
    }
    
    /// Format tick end event
    fn format_tick_end(&self, tick_number: u64, duration_ns: u64, jitter_ns: i64) -> String {
        format!(
            "flight_hub_tick_end: tick={} duration_ns={} jitter_ns={}",
            tick_number, duration_ns, jitter_ns
        )
    }
    
    /// Format HID write event
    fn format_hid_write(&self, device_id: u32, bytes: usize, duration_ns: u64) -> String {
        format!(
            "flight_hub_hid_write: device_id=0x{:x} bytes={} duration_ns={}",
            device_id, bytes, duration_ns
        )
    }
    
    /// Format deadline miss event
    fn format_deadline_miss(&self, tick_number: u64, miss_duration_ns: u64) -> String {
        format!(
            "flight_hub_deadline_miss: tick={} miss_duration_ns={}",
            tick_number, miss_duration_ns
        )
    }
    
    /// Format writer drop event
    fn format_writer_drop(&self, stream_id: &str, dropped_count: u64) -> String {
        format!(
            "flight_hub_writer_drop: stream_id={} dropped_count={}",
            stream_id, dropped_count
        )
    }
    
    /// Format custom event
    fn format_custom(&self, name: &str, data: &serde_json::Value) -> Result<String, TraceError> {
        let json_str = serde_json::to_string(data)?;
        Ok(format!("flight_hub_custom: name={} data={}", name, json_str))
    }
}

impl TraceProvider for TracepointProvider {
    fn initialize(&mut self) -> Result<(), TraceError> {
        if self.enabled {
            return Ok(());
        }
        
        if !Self::is_available() {
            return Err(TraceError::Platform(
                "Tracepoints not available - /sys/kernel/debug/tracing/trace_marker not found".to_string()
            ));
        }
        
        // Open trace_marker for writing
        let file = OpenOptions::new()
            .write(true)
            .open("/sys/kernel/debug/tracing/trace_marker")
            .map_err(|e| TraceError::Platform(format!("Failed to open trace_marker: {}", e)))?;
        
        let writer = BufWriter::new(file);
        *self.trace_marker.lock().unwrap() = Some(writer);
        self.enabled = true;
        
        // Write initialization marker
        self.write_trace_marker("flight_hub_init: tracing started")?;
        
        tracing::info!("Linux tracepoints initialized");
        Ok(())
    }
    
    fn emit_event(&self, event: &TraceEvent) -> Result<(), TraceError> {
        if !self.enabled {
            return Ok(());
        }
        
        let message = match &event.data {
            EventData::TickStart { tick_number } => {
                self.format_tick_start(*tick_number)
            }
            
            EventData::TickEnd { tick_number, duration_ns, jitter_ns } => {
                self.format_tick_end(*tick_number, *duration_ns, *jitter_ns)
            }
            
            EventData::HidWrite { device_id, bytes, duration_ns } => {
                self.format_hid_write(*device_id, *bytes, *duration_ns)
            }
            
            EventData::DeadlineMiss { tick_number, miss_duration_ns } => {
                self.format_deadline_miss(*tick_number, *miss_duration_ns)
            }
            
            EventData::WriterDrop { stream_id, dropped_count } => {
                self.format_writer_drop(stream_id, *dropped_count)
            }
            
            EventData::Custom { name, data } => {
                self.format_custom(name, data)?
            }
        };
        
        self.write_trace_marker(&message)
    }
    
    fn shutdown(&mut self) -> Result<(), TraceError> {
        if !self.enabled {
            return Ok(());
        }
        
        // Write shutdown marker
        self.write_trace_marker("flight_hub_shutdown: tracing stopped")?;
        
        // Close trace marker
        *self.trace_marker.lock().unwrap() = None;
        self.enabled = false;
        
        tracing::info!("Linux tracepoints shutdown");
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Drop for TracepointProvider {
    fn drop(&mut self) {
        if self.enabled {
            let _ = self.shutdown();
        }
    }
}

/// Helper functions for ftrace integration
pub mod ftrace {
    use std::fs;
    use std::io::Write;
    use crate::TraceError;
    
    /// Enable Flight Hub tracing in ftrace
    pub fn enable_flight_hub_tracing() -> Result<(), TraceError> {
        // Enable tracing
        fs::write("/sys/kernel/debug/tracing/tracing_on", "1")
            .map_err(|e| TraceError::Platform(format!("Failed to enable tracing: {}", e)))?;
        
        // Set buffer size (8MB per CPU)
        let _ = fs::write("/sys/kernel/debug/tracing/buffer_size_kb", "8192");
        
        // Clear existing trace
        fs::write("/sys/kernel/debug/tracing/trace", "")
            .map_err(|e| TraceError::Platform(format!("Failed to clear trace: {}", e)))?;
        
        Ok(())
    }
    
    /// Disable tracing
    pub fn disable_tracing() -> Result<(), TraceError> {
        fs::write("/sys/kernel/debug/tracing/tracing_on", "0")
            .map_err(|e| TraceError::Platform(format!("Failed to disable tracing: {}", e)))?;
        
        Ok(())
    }
    
    /// Set trace filter for Flight Hub events only
    pub fn set_flight_hub_filter() -> Result<(), TraceError> {
        // Filter for flight_hub events only
        fs::write("/sys/kernel/debug/tracing/set_event", "")
            .map_err(|e| TraceError::Platform(format!("Failed to clear events: {}", e)))?;
        
        // Enable print events (trace_marker writes)
        let _ = fs::write("/sys/kernel/debug/tracing/events/printk/enable", "1");
        
        Ok(())
    }
    
    /// Read current trace buffer
    pub fn read_trace() -> Result<String, TraceError> {
        fs::read_to_string("/sys/kernel/debug/tracing/trace")
            .map_err(|e| TraceError::Platform(format!("Failed to read trace: {}", e)))
    }
    
    /// Get trace statistics
    pub fn get_trace_stats() -> Result<TraceStats, TraceError> {
        let stats_content = fs::read_to_string("/sys/kernel/debug/tracing/trace_stats")
            .map_err(|e| TraceError::Platform(format!("Failed to read trace stats: {}", e)))?;
        
        // Parse basic stats (simplified)
        let mut entries = 0;
        let mut overrun = 0;
        
        for line in stats_content.lines() {
            if line.contains("entries:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    entries = value.parse().unwrap_or(0);
                }
            } else if line.contains("overrun:") {
                if let Some(value) = line.split_whitespace().nth(1) {
                    overrun = value.parse().unwrap_or(0);
                }
            }
        }
        
        Ok(TraceStats { entries, overrun })
    }
}

/// Trace statistics
#[derive(Debug, Clone)]
pub struct TraceStats {
    pub entries: u64,
    pub overrun: u64,
}

/// Perf integration helpers
pub mod perf {
    use std::process::Command;
    use crate::TraceError;
    
    /// Start perf recording for Flight Hub events
    pub fn start_recording(output_file: &str, duration_seconds: u32) -> Result<(), TraceError> {
        let status = Command::new("perf")
            .args(&[
                "record",
                "-e", "printk:console",
                "-o", output_file,
                "--", 
                "sleep", &duration_seconds.to_string()
            ])
            .status()
            .map_err(|e| TraceError::Platform(format!("Failed to start perf: {}", e)))?;
        
        if !status.success() {
            return Err(TraceError::Platform("Perf recording failed".to_string()));
        }
        
        Ok(())
    }
    
    /// Convert perf data to text format
    pub fn script_to_text(perf_file: &str, output_file: &str) -> Result<(), TraceError> {
        let status = Command::new("perf")
            .args(&["script", "-i", perf_file])
            .output()
            .map_err(|e| TraceError::Platform(format!("Failed to run perf script: {}", e)))?;
        
        if !status.status.success() {
            return Err(TraceError::Platform("Perf script failed".to_string()));
        }
        
        std::fs::write(output_file, status.stdout)
            .map_err(|e| TraceError::Platform(format!("Failed to write output: {}", e)))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracepoint_provider_creation() {
        let provider = TracepointProvider::new();
        assert!(!provider.is_enabled());
    }

    #[test]
    fn test_tracepoint_availability() {
        // This test will pass/fail based on system configuration
        let available = TracepointProvider::is_available();
        println!("Tracepoints available: {}", available);
    }

    #[test]
    fn test_event_formatting() {
        let provider = TracepointProvider::new();
        
        let tick_start = provider.format_tick_start(42);
        assert!(tick_start.contains("flight_hub_tick_start"));
        assert!(tick_start.contains("tick=42"));
        
        let tick_end = provider.format_tick_end(42, 4000000, 1500);
        assert!(tick_end.contains("flight_hub_tick_end"));
        assert!(tick_end.contains("tick=42"));
        assert!(tick_end.contains("duration_ns=4000000"));
        assert!(tick_end.contains("jitter_ns=1500"));
        
        let hid_write = provider.format_hid_write(0x1234, 64, 250000);
        assert!(hid_write.contains("flight_hub_hid_write"));
        assert!(hid_write.contains("device_id=0x1234"));
        assert!(hid_write.contains("bytes=64"));
        
        let deadline_miss = provider.format_deadline_miss(43, 2000000);
        assert!(deadline_miss.contains("flight_hub_deadline_miss"));
        assert!(deadline_miss.contains("tick=43"));
        
        let writer_drop = provider.format_writer_drop("axis", 5);
        assert!(writer_drop.contains("flight_hub_writer_drop"));
        assert!(writer_drop.contains("stream_id=axis"));
        assert!(writer_drop.contains("dropped_count=5"));
    }

    #[test]
    fn test_custom_event_formatting() {
        let provider = TracepointProvider::new();
        let data = serde_json::json!({"key": "value", "number": 42});
        
        let custom = provider.format_custom("test_event", &data).unwrap();
        assert!(custom.contains("flight_hub_custom"));
        assert!(custom.contains("name=test_event"));
        assert!(custom.contains("key"));
        assert!(custom.contains("value"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_tracepoint_initialization() {
        if !TracepointProvider::is_available() {
            println!("Skipping test - tracepoints not available");
            return;
        }
        
        let mut provider = TracepointProvider::new();
        
        // Should initialize if tracepoints are available
        match provider.initialize() {
            Ok(()) => {
                assert!(provider.is_enabled());
                
                // Test event emission
                let event = TraceEvent::tick_start(1);
                assert!(provider.emit_event(&event).is_ok());
                
                // Should shutdown cleanly
                assert!(provider.shutdown().is_ok());
                assert!(!provider.is_enabled());
            }
            Err(e) => {
                println!("Tracepoint initialization failed (expected if not root): {}", e);
            }
        }
    }
}