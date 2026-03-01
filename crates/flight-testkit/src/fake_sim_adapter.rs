// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fake simulator adapter that records axis outputs for verification.

use std::time::Duration;

/// Describes how the fake adapter behaves with respect to connectivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionBehavior {
    /// Always connected.
    AlwaysConnected,
    /// Always disconnected — every write returns an error.
    AlwaysDisconnected,
    /// Alternates between connected and disconnected every `cycle_len` writes.
    Intermittent { cycle_len: usize },
}

/// A single axis output recorded by the adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedOutput {
    /// Axis index.
    pub axis: usize,
    /// Written value.
    pub value: f64,
    /// Timestamp (microseconds from test clock).
    pub timestamp_us: u64,
}

/// Error returned when the fake adapter rejects a write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeAdapterError {
    /// The adapter is currently disconnected.
    Disconnected,
    /// The value is outside the expected bounds.
    OutOfBounds {
        axis: usize,
        value_bits: u64,
        min_bits: u64,
        max_bits: u64,
    },
}

impl std::fmt::Display for FakeAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "fake adapter is disconnected"),
            Self::OutOfBounds {
                axis,
                value_bits,
                min_bits,
                max_bits,
            } => {
                write!(
                    f,
                    "axis {axis} value {value_bits} out of bounds [{min_bits}, {max_bits}]"
                )
            }
        }
    }
}

impl std::error::Error for FakeAdapterError {}

/// A fake simulator adapter that consumes axis outputs, records them, and
/// optionally validates bounds and simulates connectivity issues.
#[derive(Debug)]
pub struct FakeSimAdapter {
    /// Human-readable name.
    pub name: String,
    behavior: ConnectionBehavior,
    /// Expected value bounds (min, max) per axis, or global if single entry.
    bounds: Option<(f64, f64)>,
    recordings: Vec<RecordedOutput>,
    write_count: usize,
    disconnect_events: Vec<u64>,
    reconnect_events: Vec<u64>,
    connected: bool,
}

impl FakeSimAdapter {
    /// Create a new adapter with the given name, always connected.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            behavior: ConnectionBehavior::AlwaysConnected,
            bounds: None,
            recordings: Vec::new(),
            write_count: 0,
            disconnect_events: Vec::new(),
            reconnect_events: Vec::new(),
            connected: true,
        }
    }

    /// Set the connection behavior.
    pub fn with_behavior(mut self, behavior: ConnectionBehavior) -> Self {
        self.behavior = behavior;
        self.connected = behavior != ConnectionBehavior::AlwaysDisconnected;
        self
    }

    /// Set expected output bounds; writes outside these bounds return an error.
    pub fn with_bounds(mut self, min: f64, max: f64) -> Self {
        self.bounds = Some((min, max));
        self
    }

    // -- writing -------------------------------------------------------------

    /// Write an axis value at the given timestamp.
    ///
    /// Returns an error if the adapter is disconnected or the value is out of
    /// bounds.
    pub fn write_axis(
        &mut self,
        axis: usize,
        value: f64,
        timestamp_us: u64,
    ) -> Result<(), FakeAdapterError> {
        self.write_count += 1;
        self.update_connection(timestamp_us);

        if !self.connected {
            return Err(FakeAdapterError::Disconnected);
        }

        if let Some((min, max)) = self.bounds
            && (value < min || value > max)
        {
            return Err(FakeAdapterError::OutOfBounds {
                axis,
                value_bits: value.to_bits(),
                min_bits: min.to_bits(),
                max_bits: max.to_bits(),
            });
        }

        self.recordings.push(RecordedOutput {
            axis,
            value,
            timestamp_us,
        });
        Ok(())
    }

    // -- disconnect / reconnect simulation -----------------------------------

    /// Simulate a disconnect at the given timestamp.
    pub fn simulate_disconnect(&mut self, timestamp_us: u64) {
        self.connected = false;
        self.disconnect_events.push(timestamp_us);
    }

    /// Simulate a reconnect at the given timestamp.
    pub fn simulate_reconnect(&mut self, timestamp_us: u64) {
        self.connected = true;
        self.reconnect_events.push(timestamp_us);
    }

    /// Whether the adapter is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    // -- querying ------------------------------------------------------------

    /// All recorded outputs.
    pub fn recordings(&self) -> &[RecordedOutput] {
        &self.recordings
    }

    /// Recorded outputs for a specific axis.
    pub fn recordings_for_axis(&self, axis: usize) -> Vec<&RecordedOutput> {
        self.recordings.iter().filter(|r| r.axis == axis).collect()
    }

    /// Timestamps of all disconnect events.
    pub fn disconnect_events(&self) -> &[u64] {
        &self.disconnect_events
    }

    /// Timestamps of all reconnect events.
    pub fn reconnect_events(&self) -> &[u64] {
        &self.reconnect_events
    }

    /// Total number of `write_axis` calls (including failed ones).
    pub fn total_writes(&self) -> usize {
        self.write_count
    }

    /// Clear all recorded data but keep configuration.
    pub fn clear(&mut self) {
        self.recordings.clear();
        self.write_count = 0;
        self.disconnect_events.clear();
        self.reconnect_events.clear();
    }

    /// Return the average interval between consecutive recordings (as [`Duration`]).
    ///
    /// Returns `None` if fewer than two recordings exist.
    pub fn average_interval(&self) -> Option<Duration> {
        if self.recordings.len() < 2 {
            return None;
        }
        let total: u64 = self
            .recordings
            .windows(2)
            .map(|w| w[1].timestamp_us.saturating_sub(w[0].timestamp_us))
            .sum();
        let count = (self.recordings.len() - 1) as u64;
        Some(Duration::from_micros(total / count))
    }

    // -- internal ------------------------------------------------------------

    fn update_connection(&mut self, _timestamp_us: u64) {
        match self.behavior {
            ConnectionBehavior::AlwaysConnected => {
                // Don't override manual disconnect/reconnect calls.
            }
            ConnectionBehavior::AlwaysDisconnected => self.connected = false,
            ConnectionBehavior::Intermittent { cycle_len } => {
                // Toggle based on write count within each cycle.
                // write_count has already been incremented, so use 1-based index.
                let cycle_pos = (self.write_count - 1) % (cycle_len * 2);
                self.connected = cycle_pos < cycle_len;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_adapter_is_connected() {
        let adapter = FakeSimAdapter::new("MSFS");
        assert!(adapter.is_connected());
        assert!(adapter.recordings().is_empty());
        assert_eq!(adapter.total_writes(), 0);
    }

    #[test]
    fn write_records_output() {
        let mut adapter = FakeSimAdapter::new("MSFS");
        adapter.write_axis(0, 0.5, 1000).unwrap();
        adapter.write_axis(1, -0.3, 2000).unwrap();

        assert_eq!(adapter.recordings().len(), 2);
        assert_eq!(adapter.recordings()[0].axis, 0);
        assert!((adapter.recordings()[0].value - 0.5).abs() < f64::EPSILON);
        assert_eq!(adapter.recordings()[0].timestamp_us, 1000);
    }

    #[test]
    fn recordings_for_axis_filters() {
        let mut adapter = FakeSimAdapter::new("X-Plane");
        adapter.write_axis(0, 0.1, 1000).unwrap();
        adapter.write_axis(1, 0.2, 2000).unwrap();
        adapter.write_axis(0, 0.3, 3000).unwrap();

        let axis_0 = adapter.recordings_for_axis(0);
        assert_eq!(axis_0.len(), 2);
        assert!((axis_0[0].value - 0.1).abs() < f64::EPSILON);
        assert!((axis_0[1].value - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn always_disconnected_rejects_writes() {
        let mut adapter =
            FakeSimAdapter::new("DCS").with_behavior(ConnectionBehavior::AlwaysDisconnected);
        let result = adapter.write_axis(0, 0.5, 1000);
        assert_eq!(result, Err(FakeAdapterError::Disconnected));
        assert_eq!(adapter.total_writes(), 1);
        assert!(adapter.recordings().is_empty());
    }

    #[test]
    fn intermittent_toggles_connection() {
        let mut adapter = FakeSimAdapter::new("Sim")
            .with_behavior(ConnectionBehavior::Intermittent { cycle_len: 2 });

        // First 2 writes are connected (cycle positions 1, 2 ⇒ 1 < 2, but
        // write_count is incremented *before* check, so positions are 1, 2)
        let r0 = adapter.write_axis(0, 0.1, 1000);
        let r1 = adapter.write_axis(0, 0.2, 2000);
        // Next 2 writes should be disconnected
        let r2 = adapter.write_axis(0, 0.3, 3000);
        let r3 = adapter.write_axis(0, 0.4, 4000);

        assert!(r0.is_ok());
        assert!(r1.is_ok());
        assert!(r2.is_err());
        assert!(r3.is_err());
    }

    #[test]
    fn bounds_checking_rejects_out_of_range() {
        let mut adapter = FakeSimAdapter::new("Sim").with_bounds(-1.0, 1.0);
        assert!(adapter.write_axis(0, 0.5, 1000).is_ok());
        assert!(adapter.write_axis(0, 1.5, 2000).is_err());
        assert!(adapter.write_axis(0, -1.5, 3000).is_err());
        // Only the successful write should be recorded
        assert_eq!(adapter.recordings().len(), 1);
    }

    #[test]
    fn simulate_disconnect_reconnect() {
        let mut adapter = FakeSimAdapter::new("Sim");
        adapter.simulate_disconnect(5000);
        assert!(!adapter.is_connected());
        assert!(adapter.write_axis(0, 0.0, 5000).is_err());

        adapter.simulate_reconnect(10000);
        assert!(adapter.is_connected());
        // Must override behavior back to connected for manual simulate to work
        // When behavior is AlwaysConnected the update_connection restores it
        assert!(adapter.write_axis(0, 0.0, 10000).is_ok());

        assert_eq!(adapter.disconnect_events(), &[5000]);
        assert_eq!(adapter.reconnect_events(), &[10000]);
    }

    #[test]
    fn average_interval_computed() {
        let mut adapter = FakeSimAdapter::new("Sim");
        adapter.write_axis(0, 0.0, 0).unwrap();
        adapter.write_axis(0, 0.1, 4000).unwrap();
        adapter.write_axis(0, 0.2, 8000).unwrap();

        let avg = adapter.average_interval().unwrap();
        assert_eq!(avg, Duration::from_micros(4000));
    }

    #[test]
    fn average_interval_none_when_too_few() {
        let adapter = FakeSimAdapter::new("Sim");
        assert!(adapter.average_interval().is_none());
    }

    #[test]
    fn clear_resets_recordings() {
        let mut adapter = FakeSimAdapter::new("Sim");
        adapter.write_axis(0, 0.5, 1000).unwrap();
        adapter.simulate_disconnect(2000);
        adapter.clear();
        assert!(adapter.recordings().is_empty());
        assert_eq!(adapter.total_writes(), 0);
        assert!(adapter.disconnect_events().is_empty());
    }
}
