// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! A fake game/simulator backend that consumes axis outputs and records them.

/// Connection state of the fake game backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

/// A single recorded output frame sent to the game.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedOutput {
    /// Timestamp in microseconds (caller-supplied, e.g. from DeterministicClock).
    pub timestamp_us: u64,
    /// Axis values sent to the game.
    pub axes: Vec<f64>,
}

/// A fake game backend for testing the output side of the pipeline.
///
/// Consumes axis outputs and records them for later assertion.
/// Supports configurable output acceptance rate and connection state simulation.
#[derive(Debug)]
pub struct FakeGameBackend {
    pub name: String,
    state: GameConnectionState,
    /// How many output frames to accept before dropping (0 = unlimited).
    acceptance_limit: usize,
    recorded_outputs: Vec<RecordedOutput>,
    total_sent: u64,
    total_dropped: u64,
}

impl FakeGameBackend {
    /// Create a new game backend with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            state: GameConnectionState::Disconnected,
            acceptance_limit: 0,
            recorded_outputs: Vec::new(),
            total_sent: 0,
            total_dropped: 0,
        }
    }

    /// Set the maximum number of output frames the buffer will accept.
    /// Pass `0` for unlimited.
    pub fn set_acceptance_limit(&mut self, limit: usize) {
        self.acceptance_limit = limit;
    }

    /// Transition to the `Connecting` state.
    pub fn begin_connect(&mut self) {
        self.state = GameConnectionState::Connecting;
    }

    /// Transition to the `Connected` state.
    pub fn complete_connect(&mut self) {
        self.state = GameConnectionState::Connected;
    }

    /// Transition to the `Disconnected` state.
    pub fn disconnect(&mut self) {
        self.state = GameConnectionState::Disconnected;
    }

    /// Return the current connection state.
    pub fn connection_state(&self) -> GameConnectionState {
        self.state
    }

    /// Send axis outputs to the game. Only accepted when connected and within
    /// the acceptance limit. Returns `true` if accepted, `false` if dropped.
    pub fn send_axes(&mut self, timestamp_us: u64, axes: Vec<f64>) -> bool {
        if self.state != GameConnectionState::Connected {
            self.total_dropped += 1;
            return false;
        }
        if self.acceptance_limit > 0 && self.recorded_outputs.len() >= self.acceptance_limit {
            self.total_dropped += 1;
            return false;
        }
        self.recorded_outputs
            .push(RecordedOutput { timestamp_us, axes });
        self.total_sent += 1;
        true
    }

    /// Return all recorded output frames.
    pub fn recorded_outputs(&self) -> &[RecordedOutput] {
        &self.recorded_outputs
    }

    /// Return the number of recorded output frames.
    pub fn output_count(&self) -> usize {
        self.recorded_outputs.len()
    }

    /// Return the total number of successfully sent frames.
    pub fn total_sent(&self) -> u64 {
        self.total_sent
    }

    /// Return the total number of dropped frames.
    pub fn total_dropped(&self) -> u64 {
        self.total_dropped
    }

    /// Assert that the last recorded axis value at `axis_index` is approximately `expected`.
    ///
    /// # Panics
    ///
    /// Panics if no outputs have been recorded or if the value is outside tolerance.
    pub fn assert_last_axis(&self, axis_index: usize, expected: f64, tolerance: f64) {
        let last = self
            .recorded_outputs
            .last()
            .expect("no outputs recorded to assert on");
        let actual = *last.axes.get(axis_index).unwrap_or_else(|| {
            panic!(
                "axis_index {} out of range (len={})",
                axis_index,
                last.axes.len()
            )
        });
        assert!(
            (actual - expected).abs() <= tolerance,
            "axis {axis_index}: expected ~{expected} got {actual} (tolerance {tolerance})"
        );
    }

    /// Assert that all recorded outputs have axis values within `[min, max]`.
    ///
    /// # Panics
    ///
    /// Panics if any axis value is out of range.
    pub fn assert_all_axes_in_range(&self, min: f64, max: f64) {
        for (frame_idx, output) in self.recorded_outputs.iter().enumerate() {
            for (axis_idx, &value) in output.axes.iter().enumerate() {
                assert!(
                    value >= min && value <= max,
                    "frame {frame_idx} axis {axis_idx}: value {value} outside [{min}, {max}]"
                );
            }
        }
    }

    /// Assert that at least `count` output frames were recorded.
    ///
    /// # Panics
    ///
    /// Panics if fewer than `count` frames were recorded.
    pub fn assert_min_outputs(&self, count: usize) {
        assert!(
            self.recorded_outputs.len() >= count,
            "expected at least {count} outputs, got {}",
            self.recorded_outputs.len()
        );
    }

    /// Clear all recorded outputs and counters.
    pub fn clear(&mut self) {
        self.recorded_outputs.clear();
        self.total_sent = 0;
        self.total_dropped = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_defaults() {
        let game = FakeGameBackend::new("MSFS");
        assert_eq!(game.name, "MSFS");
        assert_eq!(game.connection_state(), GameConnectionState::Disconnected);
        assert_eq!(game.output_count(), 0);
        assert_eq!(game.total_sent(), 0);
        assert_eq!(game.total_dropped(), 0);
    }

    #[test]
    fn connection_state_transitions() {
        let mut game = FakeGameBackend::new("X-Plane");
        assert_eq!(game.connection_state(), GameConnectionState::Disconnected);

        game.begin_connect();
        assert_eq!(game.connection_state(), GameConnectionState::Connecting);

        game.complete_connect();
        assert_eq!(game.connection_state(), GameConnectionState::Connected);

        game.disconnect();
        assert_eq!(game.connection_state(), GameConnectionState::Disconnected);
    }

    #[test]
    fn send_axes_when_connected() {
        let mut game = FakeGameBackend::new("DCS");
        game.complete_connect();

        assert!(game.send_axes(0, vec![0.5, -0.3]));
        assert!(game.send_axes(4000, vec![0.6, -0.2]));
        assert_eq!(game.output_count(), 2);
        assert_eq!(game.total_sent(), 2);
        assert_eq!(game.total_dropped(), 0);
    }

    #[test]
    fn send_axes_dropped_when_disconnected() {
        let mut game = FakeGameBackend::new("DCS");
        assert!(!game.send_axes(0, vec![0.5]));
        assert_eq!(game.total_dropped(), 1);
        assert_eq!(game.output_count(), 0);
    }

    #[test]
    fn send_axes_dropped_when_connecting() {
        let mut game = FakeGameBackend::new("DCS");
        game.begin_connect();
        assert!(!game.send_axes(0, vec![0.5]));
        assert_eq!(game.total_dropped(), 1);
    }

    #[test]
    fn acceptance_limit_enforced() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.set_acceptance_limit(2);

        assert!(game.send_axes(0, vec![0.1]));
        assert!(game.send_axes(1000, vec![0.2]));
        assert!(!game.send_axes(2000, vec![0.3])); // over limit
        assert_eq!(game.output_count(), 2);
        assert_eq!(game.total_dropped(), 1);
    }

    #[test]
    fn recorded_outputs_accessible() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.send_axes(0, vec![0.5, -0.5]);
        game.send_axes(4000, vec![0.6, -0.4]);

        let outputs = game.recorded_outputs();
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].timestamp_us, 0);
        assert!((outputs[0].axes[0] - 0.5).abs() < f64::EPSILON);
        assert_eq!(outputs[1].timestamp_us, 4000);
    }

    #[test]
    fn assert_last_axis_passes() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.send_axes(0, vec![0.75, -0.25]);
        game.assert_last_axis(0, 0.75, 0.001);
        game.assert_last_axis(1, -0.25, 0.001);
    }

    #[test]
    #[should_panic(expected = "axis 0")]
    fn assert_last_axis_fails() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.send_axes(0, vec![0.75]);
        game.assert_last_axis(0, 0.0, 0.001);
    }

    #[test]
    fn assert_all_axes_in_range_passes() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.send_axes(0, vec![0.0, 0.5]);
        game.send_axes(1000, vec![-1.0, 1.0]);
        game.assert_all_axes_in_range(-1.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "outside")]
    fn assert_all_axes_in_range_fails() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.send_axes(0, vec![1.5]);
        game.assert_all_axes_in_range(-1.0, 1.0);
    }

    #[test]
    fn assert_min_outputs_passes() {
        let mut game = FakeGameBackend::new("DCS");
        game.complete_connect();
        game.send_axes(0, vec![0.0]);
        game.send_axes(1000, vec![0.1]);
        game.assert_min_outputs(2);
    }

    #[test]
    #[should_panic(expected = "expected at least")]
    fn assert_min_outputs_fails() {
        let game = FakeGameBackend::new("DCS");
        game.assert_min_outputs(1);
    }

    #[test]
    fn clear_resets_everything() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.send_axes(0, vec![0.5]);
        game.clear();

        assert_eq!(game.output_count(), 0);
        assert_eq!(game.total_sent(), 0);
        assert_eq!(game.total_dropped(), 0);
    }

    #[test]
    fn unlimited_acceptance_when_zero() {
        let mut game = FakeGameBackend::new("MSFS");
        game.complete_connect();
        game.set_acceptance_limit(0);
        for i in 0..100 {
            assert!(game.send_axes(i * 1000, vec![0.0]));
        }
        assert_eq!(game.output_count(), 100);
    }
}
