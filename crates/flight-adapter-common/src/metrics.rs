// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Adapter performance metrics.

use std::time::{Duration, Instant};

/// Adapter metrics for monitoring update rate and jitter.
#[derive(Debug, Clone)]
pub struct AdapterMetrics {
    /// Total number of telemetry updates received.
    pub total_updates: u64,
    /// Last update timestamp.
    pub last_update_time: Option<Instant>,
    /// Update intervals (for jitter calculation).
    pub update_intervals: Vec<Duration>,
    /// Maximum interval buffer size.
    pub max_interval_samples: usize,
    /// Actual update rate (Hz) - calculated from recent intervals.
    pub actual_update_rate: f32,
    /// Update jitter (p99 in milliseconds).
    pub update_jitter_p99_ms: f32,
    /// Last aircraft title (for change detection).
    pub last_aircraft_title: Option<String>,
    /// Aircraft change count.
    pub aircraft_changes: u64,
}

impl Default for AdapterMetrics {
    fn default() -> Self {
        Self {
            total_updates: 0,
            last_update_time: None,
            update_intervals: Vec::new(),
            max_interval_samples: 100,
            actual_update_rate: 0.0,
            update_jitter_p99_ms: 0.0,
            last_aircraft_title: None,
            aircraft_changes: 0,
        }
    }
}

impl AdapterMetrics {
    /// Create new metrics with default buffer size.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a telemetry update.
    pub fn record_update(&mut self) {
        self.total_updates += 1;
        let now = Instant::now();

        if let Some(last_time) = self.last_update_time {
            let interval = now.duration_since(last_time);
            self.update_intervals.push(interval);

            if self.update_intervals.len() > self.max_interval_samples {
                self.update_intervals.remove(0);
            }

            if !self.update_intervals.is_empty() {
                let avg_interval: Duration = self.update_intervals.iter().sum::<Duration>()
                    / self.update_intervals.len() as u32;
                self.actual_update_rate = 1.0 / avg_interval.as_secs_f32();

                let mut sorted_intervals = self.update_intervals.clone();
                sorted_intervals.sort();
                let p99_index = (sorted_intervals.len() as f32 * 0.99) as usize;
                if p99_index < sorted_intervals.len() {
                    self.update_jitter_p99_ms = sorted_intervals[p99_index].as_secs_f32() * 1000.0;
                }
            }
        }

        self.last_update_time = Some(now);
    }

    /// Record aircraft change.
    pub fn record_aircraft_change(&mut self, title: String) {
        if self.last_aircraft_title.as_ref() != Some(&title) {
            self.aircraft_changes += 1;
            self.last_aircraft_title = Some(title);
        }
    }

    /// Get metrics summary for logging/monitoring.
    pub fn summary(&self) -> String {
        format!(
            "Updates: {}, Rate: {:.1} Hz, Jitter p99: {:.2} ms, Aircraft changes: {}",
            self.total_updates,
            self.actual_update_rate,
            self.update_jitter_p99_ms,
            self.aircraft_changes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_update_tracks_samples() {
        let mut metrics = AdapterMetrics::new();
        metrics.record_update();
        metrics.record_update();

        assert_eq!(metrics.total_updates, 2);
        assert!(metrics.last_update_time.is_some());
        assert!(!metrics.update_intervals.is_empty());
    }

    #[test]
    fn test_record_aircraft_change() {
        let mut metrics = AdapterMetrics::new();
        metrics.record_aircraft_change("C172".to_string());
        metrics.record_aircraft_change("C172".to_string());
        metrics.record_aircraft_change("A320".to_string());

        assert_eq!(metrics.aircraft_changes, 2);
    }
}
