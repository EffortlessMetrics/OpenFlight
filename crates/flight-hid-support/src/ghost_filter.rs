// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ghost input filtering for HOTAS devices.
//!
//! Some HOTAS hardware (particularly X55/X56 mini-sticks) is known to generate
//! spurious button presses. This module provides filtering to mitigate these issues.
//!
//! # Ghost Input Types
//!
//! - **Bounce**: Rapid on/off transitions faster than humanly possible
//! - **Impossible states**: Multiple mutually exclusive buttons pressed simultaneously
//! - **Stuck buttons**: Button appears held when physically released

use std::time::{Duration, Instant};

/// Default debounce threshold for button inputs.
pub const DEFAULT_DEBOUNCE_MS: u64 = 20;

/// Maximum buttons tracked by the filter (derived from u32 bitmask width).
pub const MAX_TRACKED_BUTTONS: usize = u32::BITS as usize;

/// Ghost input filter combining debouncing and impossible state detection.
#[derive(Debug)]
pub struct GhostInputFilter {
    debouncer: ButtonDebouncer,
    impossible_detector: ImpossibleStateDetector,
    stats: GhostFilterStats,
}

impl GhostInputFilter {
    /// Create a new ghost input filter with default settings.
    pub fn new() -> Self {
        Self::with_config(GhostFilterConfig::default())
    }

    /// Create a ghost input filter with custom configuration.
    pub fn with_config(config: GhostFilterConfig) -> Self {
        Self {
            debouncer: ButtonDebouncer::new(config.debounce_threshold),
            impossible_detector: ImpossibleStateDetector::new(config.impossible_masks.clone()),
            stats: GhostFilterStats::default(),
        }
    }

    /// Filter a raw button state bitmask, returning the filtered state.
    ///
    /// This applies both debouncing and impossible state detection.
    pub fn filter(&mut self, raw: u32) -> u32 {
        let debounced = self.debouncer.filter(raw);
        let filtered = self.impossible_detector.filter(debounced);

        // Track statistics
        if raw != filtered {
            self.stats.total_filtered += 1;
        }
        if raw != debounced {
            self.stats.debounce_filtered += 1;
        }
        if debounced != filtered {
            self.stats.impossible_filtered += 1;
        }
        self.stats.total_samples += 1;

        filtered
    }

    /// Get the current ghost detection rate (0.0 to 1.0).
    pub fn ghost_rate(&self) -> f64 {
        if self.stats.total_samples == 0 {
            0.0
        } else {
            self.stats.total_filtered as f64 / self.stats.total_samples as f64
        }
    }

    /// Get detailed filter statistics.
    pub fn stats(&self) -> &GhostFilterStats {
        &self.stats
    }

    /// Reset filter state and statistics.
    pub fn reset(&mut self) {
        self.debouncer.reset();
        self.impossible_detector.reset();
        self.stats = GhostFilterStats::default();
    }
}

impl Default for GhostInputFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for ghost input filtering.
#[derive(Debug, Clone)]
pub struct GhostFilterConfig {
    /// Minimum time between button state changes.
    pub debounce_threshold: Duration,
    /// Bitmasks of mutually exclusive button combinations.
    pub impossible_masks: Vec<u32>,
}

impl Default for GhostFilterConfig {
    fn default() -> Self {
        Self {
            debounce_threshold: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            impossible_masks: Vec::new(),
        }
    }
}

/// Statistics from ghost input filtering.
#[derive(Debug, Clone, Default)]
pub struct GhostFilterStats {
    /// Total number of samples processed.
    pub total_samples: u64,
    /// Number of samples that were modified by any filter.
    pub total_filtered: u64,
    /// Number of samples modified by debouncing.
    pub debounce_filtered: u64,
    /// Number of samples modified by impossible state detection.
    pub impossible_filtered: u64,
}

/// Button debouncer using per-button timing.
#[derive(Debug)]
pub struct ButtonDebouncer {
    threshold: Duration,
    last_state: u32,
    last_change: [Option<Instant>; MAX_TRACKED_BUTTONS],
    output_state: u32,
}

impl ButtonDebouncer {
    /// Create a new debouncer with the specified threshold.
    pub fn new(threshold: Duration) -> Self {
        Self {
            threshold,
            last_state: 0,
            last_change: [None; MAX_TRACKED_BUTTONS],
            output_state: 0,
        }
    }

    /// Filter a raw button state, applying debounce logic.
    pub fn filter(&mut self, raw: u32) -> u32 {
        let now = Instant::now();
        let changed = raw ^ self.last_state;

        for i in 0..MAX_TRACKED_BUTTONS {
            let mask = 1u32 << i;
            if changed & mask != 0 {
                // Button state changed
                self.last_change[i] = Some(now);
            }

            // Check if enough time has passed to accept the new state
            if let Some(change_time) = self.last_change[i]
                && now.duration_since(change_time) >= self.threshold
            {
                // Accept the new state
                if raw & mask != 0 {
                    self.output_state |= mask;
                } else {
                    self.output_state &= !mask;
                }
            }
        }

        self.last_state = raw;
        self.output_state
    }

    /// Reset the debouncer state.
    pub fn reset(&mut self) {
        self.last_state = 0;
        self.last_change = [None; MAX_TRACKED_BUTTONS];
        self.output_state = 0;
    }
}

/// Detector for impossible button state combinations.
#[derive(Debug)]
pub struct ImpossibleStateDetector {
    /// Each mask represents buttons that cannot all be pressed simultaneously.
    impossible_masks: Vec<u32>,
    last_valid_state: u32,
}

impl ImpossibleStateDetector {
    /// Create a new detector with the specified impossible state masks.
    ///
    /// Each mask defines a set of buttons that cannot physically be pressed together.
    /// If all bits in a mask are set in the input, the state is considered impossible.
    pub fn new(impossible_masks: Vec<u32>) -> Self {
        Self {
            impossible_masks,
            last_valid_state: 0,
        }
    }

    /// Filter a button state, replacing impossible states with the last valid state.
    pub fn filter(&mut self, state: u32) -> u32 {
        if self.is_impossible(state) {
            // Return last known valid state
            self.last_valid_state
        } else {
            self.last_valid_state = state;
            state
        }
    }

    /// Check if a button state is impossible.
    pub fn is_impossible(&self, state: u32) -> bool {
        for mask in &self.impossible_masks {
            // If all bits in the mask are set, this is an impossible state
            if state & mask == *mask && mask.count_ones() > 1 {
                return true;
            }
        }
        false
    }

    /// Reset the detector state.
    pub fn reset(&mut self) {
        self.last_valid_state = 0;
    }
}

/// Pre-configured ghost filters for known devices.
pub mod presets {
    use super::*;

    /// Ghost filter configured for X55/X56 mini-stick issues.
    ///
    /// The mini-sticks on X55/X56 throttles are known to generate ghost inputs,
    /// particularly when multiple directions appear pressed simultaneously.
    pub fn x55_x56_ministick() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(25),
            // Mini-stick cannot physically press opposite directions
            impossible_masks: vec![
                0b0011, // Up + Down impossible
                0b1100, // Left + Right impossible
            ],
        }
    }

    /// Ghost filter with aggressive debouncing for noisy hardware.
    pub fn aggressive() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(50),
            impossible_masks: Vec::new(),
        }
    }

    /// Ghost filter configured for T.Flight HOTAS 4 HAT switch.
    ///
    /// The T.Flight HOTAS 4 HAT switch can occasionally report impossible
    /// opposite directions simultaneously. This preset filters those states.
    pub fn tflight_hotas4() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(30),
            // HAT switch cannot physically press opposite directions
            impossible_masks: vec![
                0b0101, // Up + Down impossible
                0b1010, // Left + Right impossible
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debouncer_stable_state() {
        let mut debouncer = ButtonDebouncer::new(Duration::from_millis(10));

        // Initial state should pass through after threshold
        assert_eq!(debouncer.filter(0b0001), 0);
        std::thread::sleep(Duration::from_millis(15));
        assert_eq!(debouncer.filter(0b0001), 0b0001);
    }

    #[test]
    fn test_debouncer_rejects_bounce() {
        let mut debouncer = ButtonDebouncer::new(Duration::from_millis(50));

        // Rapid changes should be rejected
        debouncer.filter(0b0001);
        debouncer.filter(0b0000);
        debouncer.filter(0b0001);

        // Should still be 0 since not enough time passed
        assert_eq!(debouncer.filter(0b0001), 0);
    }

    #[test]
    fn test_impossible_state_detection() {
        let mut detector = ImpossibleStateDetector::new(vec![0b0011]); // bits 0 and 1 can't both be set

        // Valid states pass through
        assert_eq!(detector.filter(0b0001), 0b0001);
        assert_eq!(detector.filter(0b0010), 0b0010);
        assert_eq!(detector.filter(0b0100), 0b0100);

        // Impossible state returns last valid
        assert_eq!(detector.filter(0b0011), 0b0100); // Returns last valid (0b0100)
    }

    #[test]
    fn test_ghost_filter_stats() {
        let mut filter = GhostInputFilter::new();

        // Process some samples
        filter.filter(0);
        filter.filter(0);
        filter.filter(0);

        assert_eq!(filter.stats().total_samples, 3);
        assert_eq!(filter.ghost_rate(), 0.0);
    }

    #[test]
    fn test_preset_configs() {
        let config = presets::x55_x56_ministick();
        assert_eq!(config.debounce_threshold, Duration::from_millis(25));
        assert!(!config.impossible_masks.is_empty());
    }
}
