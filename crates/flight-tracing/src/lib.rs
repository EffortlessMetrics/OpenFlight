#![cfg_attr(
    test,
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_parens,
        dead_code
    )
)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub Tracing and Performance Counters
//!
//! Provides platform-specific tracing infrastructure for real-time performance monitoring:
//! - ETW (Event Tracing for Windows) on Windows
//! - Tracepoints on Linux
//! - Performance counter collection for CI gates
//! - Regression detection and alerting
//!
//! Key events traced:
//! - TickStart/TickEnd: RT loop timing
//! - HidWrite: USB output operations
//! - DeadlineMiss: Timing violations
//! - WriterDrop: Buffer overflow events

#[cfg(windows)]
mod etw;
#[cfg(unix)]
mod tracepoints;

pub mod counters;
pub mod events;
pub mod regression;
pub mod structured_log;

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::time::Instant;

pub use counters::{CounterSnapshot, PerfCounters};
pub use events::{EventData, TraceEvent};
pub use regression::{Alert, RegressionDetector, Threshold};

/// Global performance counters
static PERF_COUNTERS: Lazy<PerfCounters> = Lazy::new(PerfCounters::new);

/// Global tracing provider
static TRACE_PROVIDER: RwLock<Option<Box<dyn TraceProvider + Send + Sync>>> = RwLock::new(None);

/// Platform-agnostic tracing provider trait
pub trait TraceProvider {
    /// Initialize the tracing provider
    fn initialize(&mut self) -> Result<(), TraceError>;

    /// Emit a trace event
    fn emit_event(&self, event: &TraceEvent) -> Result<(), TraceError>;

    /// Shutdown the provider
    fn shutdown(&mut self) -> Result<(), TraceError>;

    /// Check if provider is enabled
    fn is_enabled(&self) -> bool;
}

/// Tracing errors
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("Provider not initialized")]
    NotInitialized,

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Event serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Initialize tracing system
pub fn initialize() -> Result<(), TraceError> {
    let mut provider = TRACE_PROVIDER.write();

    #[cfg(windows)]
    {
        let mut etw_provider = etw::EtwProvider::new();
        etw_provider.initialize()?;
        *provider = Some(Box::new(etw_provider));
    }

    #[cfg(unix)]
    {
        let mut tp_provider = tracepoints::TracepointProvider::new();
        tp_provider.initialize()?;
        *provider = Some(Box::new(tp_provider));
    }

    tracing::info!("Flight Hub tracing initialized");
    Ok(())
}

/// Shutdown tracing system
pub fn shutdown() -> Result<(), TraceError> {
    let mut provider = TRACE_PROVIDER.write();
    if let Some(ref mut p) = provider.as_mut() {
        p.shutdown()?;
    }
    *provider = None;

    tracing::info!("Flight Hub tracing shutdown");
    Ok(())
}

/// Emit a trace event
pub fn emit_event(event: TraceEvent) -> Result<(), TraceError> {
    // Update performance counters
    PERF_COUNTERS.record_event(&event);

    // Emit to platform provider
    let provider = TRACE_PROVIDER.read();
    if let Some(p) = provider.as_ref()
        && p.is_enabled()
    {
        p.emit_event(&event)?;
    }

    Ok(())
}

/// Get current performance counters
pub fn get_counters() -> CounterSnapshot {
    PERF_COUNTERS.snapshot()
}

/// Reset performance counters
pub fn reset_counters() {
    PERF_COUNTERS.reset();
}

/// Check if tracing is enabled
pub fn is_enabled() -> bool {
    let provider = TRACE_PROVIDER.read();
    provider.as_ref().is_some_and(|p| p.is_enabled())
}

/// High-level tracing macros for common events
#[macro_export]
macro_rules! trace_tick_start {
    ($tick_number:expr) => {
        let _ = $crate::emit_event($crate::TraceEvent::tick_start($tick_number));
    };
}

#[macro_export]
macro_rules! trace_tick_end {
    ($tick_number:expr, $duration_ns:expr, $jitter_ns:expr) => {
        let _ = $crate::emit_event($crate::TraceEvent::tick_end(
            $tick_number,
            $duration_ns,
            $jitter_ns,
        ));
    };
}

#[macro_export]
macro_rules! trace_hid_write {
    ($device_id:expr, $bytes:expr, $duration_ns:expr) => {
        let _ = $crate::emit_event($crate::TraceEvent::hid_write(
            $device_id,
            $bytes,
            $duration_ns,
        ));
    };
}

#[macro_export]
macro_rules! trace_deadline_miss {
    ($tick_number:expr, $miss_duration_ns:expr) => {
        let _ = $crate::emit_event($crate::TraceEvent::deadline_miss(
            $tick_number,
            $miss_duration_ns,
        ));
    };
}

#[macro_export]
macro_rules! trace_writer_drop {
    ($stream_id:expr, $dropped_count:expr) => {
        let _ = $crate::emit_event($crate::TraceEvent::writer_drop($stream_id, $dropped_count));
    };
}

/// Scoped timing helper for automatic tick tracing
pub struct TickTracer {
    tick_number: u64,
    start_time: Instant,
}

impl TickTracer {
    /// Start tracing a tick
    pub fn start(tick_number: u64) -> Self {
        trace_tick_start!(tick_number);
        Self {
            tick_number,
            start_time: Instant::now(),
        }
    }

    /// End tracing with jitter measurement
    pub fn end_with_jitter(self, jitter_ns: i64) {
        let duration_ns = self.start_time.elapsed().as_nanos() as u64;
        trace_tick_end!(self.tick_number, duration_ns, jitter_ns);
    }
}

impl Drop for TickTracer {
    fn drop(&mut self) {
        let duration_ns = self.start_time.elapsed().as_nanos() as u64;
        trace_tick_end!(self.tick_number, duration_ns, 0);
    }
}

/// Scoped HID write tracer
pub struct HidWriteTracer {
    device_id: u32,
    bytes: usize,
    start_time: Instant,
}

impl HidWriteTracer {
    /// Start tracing HID write
    pub fn start(device_id: u32, bytes: usize) -> Self {
        Self {
            device_id,
            bytes,
            start_time: Instant::now(),
        }
    }
}

impl Drop for HidWriteTracer {
    fn drop(&mut self) {
        let duration_ns = self.start_time.elapsed().as_nanos() as u64;
        trace_hid_write!(self.device_id, self.bytes, duration_ns);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_tracing_initialization() {
        // Should initialize without error
        assert!(initialize().is_ok());

        // Should be enabled after init
        assert!(is_enabled());

        // Should shutdown cleanly
        assert!(shutdown().is_ok());

        // Should not be enabled after shutdown
        assert!(!is_enabled());
    }

    #[test]
    fn test_event_emission() {
        initialize().unwrap();

        let initial = get_counters();

        // Emit some events
        trace_tick_start!(1);
        trace_tick_end!(1, 1000000, 500);
        trace_hid_write!(0x1234, 64, 250000);
        trace_deadline_miss!(2, 2000000);
        trace_writer_drop!("axis", 5);

        // Check counters increased from their initial values
        let counters = get_counters();
        assert!(counters.total_ticks > initial.total_ticks);
        assert!(counters.total_hid_writes > initial.total_hid_writes);
        assert!(counters.deadline_misses > initial.deadline_misses);
        assert!(counters.writer_drops > initial.writer_drops);

        shutdown().unwrap();
    }

    #[test]
    fn test_tick_tracer() {
        initialize().unwrap();

        let initial_ticks = get_counters().total_ticks;

        {
            let _tracer = TickTracer::start(42);
            thread::sleep(Duration::from_millis(1));
        } // Auto-drop should emit tick_end

        let counters = get_counters();
        assert!(
            counters.total_ticks > initial_ticks,
            "expected ticks > {}, got {}",
            initial_ticks,
            counters.total_ticks
        );

        shutdown().unwrap();
    }

    #[test]
    fn test_hid_write_tracer() {
        initialize().unwrap();

        let initial_hid_writes = get_counters().total_hid_writes;

        {
            let _tracer = HidWriteTracer::start(0x5678, 128);
            thread::sleep(Duration::from_micros(100));
        } // Auto-drop should emit hid_write

        let counters = get_counters();
        assert!(
            counters.total_hid_writes > initial_hid_writes,
            "expected hid_writes > {}, got {}",
            initial_hid_writes,
            counters.total_hid_writes
        );

        shutdown().unwrap();
    }
}
