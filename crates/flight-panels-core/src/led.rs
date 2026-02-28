// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! LED controller for panel hardware

use flight_core::Result;
use flight_core::rules::Action;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// LED target identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LedTarget {
    Panel(String),
    Indexer,
    Custom(String),
}

/// LED state
#[derive(Debug, Clone)]
pub struct LedState {
    pub on: bool,
    pub brightness: f32,
    pub blink_rate: Option<f32>,
    pub last_update: Instant,
}

/// Seam for the actual hardware write path.
///
/// The controller drives rate limiting, state tracking, and latency accounting.
/// Implementations provide the low-level device write (HID report, USB control
/// transfer, etc.).  The no-op default is used when no physical device is
/// attached (tests, simulator-only mode).
pub trait LedBackend: Send {
    /// Write a single LED state to the device.
    ///
    /// The implementation should be non-blocking (or at most a few µs).
    /// Transient errors may be logged but should not propagate here — the
    /// controller will retry on the next [`LedController::execute_actions`]
    /// call once the rate-limit window expires.
    fn write(&mut self, target: &LedTarget, state: &LedState) -> Result<()>;
}

/// No-op LED backend — logs state changes without driving hardware.
///
/// Used as the default when no physical panel device is attached.
pub struct NoopLedBackend;

impl LedBackend for NoopLedBackend {
    fn write(&mut self, target: &LedTarget, state: &LedState) -> Result<()> {
        tracing::debug!(
            "LED {:?}: on={}, brightness={:.2}, blink_rate={:?} (noop)",
            target,
            state.on,
            state.brightness,
            state.blink_rate,
        );
        Ok(())
    }
}

/// LED controller with rate limiting and latency tracking
pub struct LedController {
    backend: Box<dyn LedBackend>,
    led_states: HashMap<LedTarget, LedState>,
    last_write: HashMap<LedTarget, Instant>,
    min_interval: Duration,
    latency_samples: Vec<Duration>,
    max_latency_samples: usize,
}

impl LedController {
    /// Create a new LED controller backed by `NoopLedBackend`.
    pub fn new() -> Self {
        Self::with_backend(Box::new(NoopLedBackend))
    }

    /// Create a new LED controller with a custom hardware backend.
    pub fn with_backend(backend: Box<dyn LedBackend>) -> Self {
        Self {
            backend,
            led_states: HashMap::new(),
            last_write: HashMap::new(),
            min_interval: Duration::from_millis(8), // ≥8ms min interval per requirements
            latency_samples: Vec::new(),
            max_latency_samples: 1000, // Keep last 1000 samples for analysis
        }
    }

    /// Execute a list of actions with rate limiting
    pub fn execute_actions(&mut self, actions: &[Action]) -> Result<()> {
        let now = Instant::now();

        for action in actions {
            let target = self.action_to_target(action);

            // Check rate limiting
            if let Some(&last_write) = self.last_write.get(&target)
                && now.duration_since(last_write) < self.min_interval
            {
                continue; // Skip this update due to rate limiting
            }

            self.execute_action(action, now)?;
            self.last_write.insert(target, now);
        }

        Ok(())
    }

    fn execute_action(&mut self, action: &Action, now: Instant) -> Result<()> {
        match action {
            Action::LedOn { target } => {
                let led_target = LedTarget::Panel(target.clone());
                // Update the state directly
                self.led_states
                    .entry(led_target.clone())
                    .and_modify(|state| {
                        state.on = true;
                        state.blink_rate = None;
                        state.last_update = now;
                    })
                    .or_insert(LedState {
                        on: true,
                        brightness: 1.0,
                        blink_rate: None,
                        last_update: now,
                    });

                let state_clone = self.led_states.get(&led_target).unwrap().clone();
                self.write_led_state(&led_target, &state_clone)?;
            }
            Action::LedOff { target } => {
                let led_target = LedTarget::Panel(target.clone());
                // Update the state directly
                self.led_states
                    .entry(led_target.clone())
                    .and_modify(|state| {
                        state.on = false;
                        state.blink_rate = None;
                        state.last_update = now;
                    })
                    .or_insert_with(|| LedState {
                        on: false,
                        brightness: 1.0,
                        blink_rate: None,
                        last_update: now,
                    });

                let state_clone = self.led_states.get(&led_target).unwrap().clone();
                self.write_led_state(&led_target, &state_clone)?;
            }
            Action::LedBlink { target, rate_hz } => {
                let led_target = if target == "indexer" {
                    LedTarget::Indexer
                } else {
                    LedTarget::Panel(target.clone())
                };

                // Update the state directly
                self.led_states
                    .entry(led_target.clone())
                    .and_modify(|state| {
                        state.blink_rate = Some(*rate_hz);
                        state.last_update = now;
                    })
                    .or_insert(LedState {
                        on: false,
                        brightness: 1.0,
                        blink_rate: Some(*rate_hz),
                        last_update: now,
                    });

                let state_clone = self.led_states.get(&led_target).unwrap().clone();
                self.write_led_state(&led_target, &state_clone)?;
            }
            Action::LedBrightness { target, brightness } => {
                let led_target = LedTarget::Panel(target.clone());
                // Update the state directly
                self.led_states
                    .entry(led_target.clone())
                    .and_modify(|state| {
                        state.brightness = brightness.clamp(0.0, 1.0);
                        state.last_update = now;
                    })
                    .or_insert(LedState {
                        on: false,
                        brightness: brightness.clamp(0.0, 1.0),
                        blink_rate: None,
                        last_update: now,
                    });

                let state_clone = self.led_states.get(&led_target).unwrap().clone();
                self.write_led_state(&led_target, &state_clone)?;
            }
        }

        Ok(())
    }

    fn action_to_target(&self, action: &Action) -> LedTarget {
        match action {
            Action::LedOn { target }
            | Action::LedOff { target }
            | Action::LedBrightness { target, .. } => LedTarget::Panel(target.clone()),
            Action::LedBlink { target, .. } => {
                if target == "indexer" {
                    LedTarget::Indexer
                } else {
                    LedTarget::Panel(target.clone())
                }
            }
        }
    }

    fn write_led_state(&mut self, target: &LedTarget, state: &LedState) -> Result<()> {
        let write_start = Instant::now();

        self.backend.write(target, state)?;

        let write_latency = write_start.elapsed();

        // Track latency for validation
        self.latency_samples.push(write_latency);
        if self.latency_samples.len() > self.max_latency_samples {
            self.latency_samples.remove(0);
        }

        // Validate latency requirement (≤20ms)
        if write_latency > Duration::from_millis(20) {
            tracing::warn!(
                "LED write latency exceeded 20ms: {:?} for target {:?}",
                write_latency,
                target
            );
        }

        Ok(())
    }

    /// Update blinking LEDs (should be called regularly)
    pub fn update_blink_states(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut updates = Vec::new();

        for (target, state) in &mut self.led_states {
            if let Some(rate_hz) = state.blink_rate {
                let period = Duration::from_secs_f32(1.0 / rate_hz);
                let elapsed = now.duration_since(state.last_update);

                if elapsed >= period / 2 {
                    state.on = !state.on;
                    state.last_update = now;
                    updates.push((target.clone(), state.clone()));
                }
            }
        }

        // Apply rate limiting to blink updates
        for (target, state) in updates {
            if let Some(&last_write) = self.last_write.get(&target) {
                if now.duration_since(last_write) >= self.min_interval {
                    self.write_led_state(&target, &state)?;
                    self.last_write.insert(target, now);
                }
            } else {
                self.write_led_state(&target, &state)?;
                self.last_write.insert(target, now);
            }
        }

        Ok(())
    }

    /// Get current LED state
    pub fn get_led_state(&self, target: &LedTarget) -> Option<&LedState> {
        self.led_states.get(target)
    }

    /// Set minimum interval for rate limiting
    pub fn set_min_interval(&mut self, interval: Duration) {
        self.min_interval = interval;
    }

    /// Get latency statistics for validation
    pub fn get_latency_stats(&self) -> Option<LatencyStats> {
        if self.latency_samples.is_empty() {
            return None;
        }

        let mut samples: Vec<_> = self.latency_samples.iter().map(|d| d.as_nanos()).collect();
        samples.sort_unstable();

        let len = samples.len();
        let mean = samples.iter().sum::<u128>() / len as u128;
        let p99_index = (len as f64 * 0.99) as usize;
        let p99 = samples.get(p99_index).copied().unwrap_or(samples[len - 1]);
        let max = samples[len - 1];

        Some(LatencyStats {
            mean_ns: mean,
            p99_ns: p99,
            max_ns: max,
            sample_count: len,
        })
    }

    /// Clear latency samples
    pub fn clear_latency_stats(&mut self) {
        self.latency_samples.clear();
    }
}

/// LED write latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub mean_ns: u128,
    pub p99_ns: u128,
    pub max_ns: u128,
    pub sample_count: usize,
}

impl Default for LedController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_core::rules::Action;

    #[test]
    fn test_led_on_off() {
        let mut controller = LedController::new();
        // Disable rate limiting for testing
        controller.set_min_interval(Duration::from_millis(0));
        let target = LedTarget::Panel("GEAR".to_string());

        // Turn LED on
        let action = Action::LedOn {
            target: "GEAR".to_string(),
        };
        controller.execute_actions(&[action]).unwrap();

        let state = controller.get_led_state(&target).unwrap();
        assert!(state.on);
        assert!(state.blink_rate.is_none());

        // Turn LED off
        let action = Action::LedOff {
            target: "GEAR".to_string(),
        };
        controller.execute_actions(&[action]).unwrap();

        let state = controller.get_led_state(&target).unwrap();
        assert!(!state.on);
    }

    #[test]
    fn test_led_blink() {
        let mut controller = LedController::new();
        controller.set_min_interval(Duration::from_millis(0));
        let target = LedTarget::Indexer;

        let action = Action::LedBlink {
            target: "indexer".to_string(),
            rate_hz: 6.0,
        };
        controller.execute_actions(&[action]).unwrap();

        let state = controller.get_led_state(&target).unwrap();
        assert_eq!(state.blink_rate, Some(6.0));
    }

    #[test]
    fn test_led_brightness() {
        let mut controller = LedController::new();
        controller.set_min_interval(Duration::from_millis(0));
        let target = LedTarget::Panel("TEST".to_string());

        let action = Action::LedBrightness {
            target: "TEST".to_string(),
            brightness: 0.5,
        };
        controller.execute_actions(&[action]).unwrap();

        let state = controller.get_led_state(&target).unwrap();
        assert_eq!(state.brightness, 0.5);
    }

    #[test]
    fn test_rate_limiting() {
        let mut controller = LedController::new();
        controller.set_min_interval(Duration::from_millis(100)); // Longer interval for testing

        let actions = vec![
            Action::LedOn {
                target: "TEST".to_string(),
            },
            Action::LedOff {
                target: "TEST".to_string(),
            },
        ];

        // Both actions should be processed, but second might be rate limited
        controller.execute_actions(&actions).unwrap();

        // In a real scenario, we'd verify that hardware writes were rate limited
        // For now, just ensure no errors occurred
    }

    #[test]
    fn test_latency_tracking() {
        let mut controller = LedController::new();
        controller.set_min_interval(Duration::from_millis(0)); // No rate limiting for test

        // Execute several actions to generate latency samples
        for i in 0..10 {
            let action = Action::LedOn {
                target: format!("TEST_{}", i),
            };
            controller.execute_actions(&[action]).unwrap();
        }

        // Check latency statistics
        let stats = controller.get_latency_stats().unwrap();
        assert_eq!(stats.sample_count, 10);

        // Verify latency is reasonable (should be very fast in test)
        assert!(
            stats.mean_ns < 50_000_000,
            "Mean latency too high: {} ns",
            stats.mean_ns
        ); // <50ms
        assert!(
            stats.p99_ns < 50_000_000,
            "P99 latency too high: {} ns",
            stats.p99_ns
        ); // <50ms
        assert!(
            stats.max_ns < 50_000_000,
            "Max latency too high: {} ns",
            stats.max_ns
        ); // <50ms
    }

    #[test]
    fn test_latency_requirement_validation() {
        let mut controller = LedController::new();
        controller.set_min_interval(Duration::from_millis(0));

        // Execute many actions to get good statistics
        for i in 0..100 {
            let action = Action::LedBlink {
                target: format!("LED_{}", i % 10),
                rate_hz: 4.0,
            };
            controller.execute_actions(&[action]).unwrap();
        }

        let stats = controller.get_latency_stats().unwrap();

        // Validate against requirements: LED latency ≤20ms
        assert!(
            stats.p99_ns <= 20_000_000,
            "LED latency requirement violated: P99 = {} ns (>20ms)",
            stats.p99_ns
        );

        // Also check that we're well under the limit in test environment
        assert!(
            stats.mean_ns < 10_000_000,
            "Mean latency should be much better than requirement in test: {} ns",
            stats.mean_ns
        );
    }

    #[test]
    fn test_min_interval_enforcement() {
        let mut controller = LedController::new();
        let min_interval = Duration::from_millis(10);
        controller.set_min_interval(min_interval);

        let target = "RATE_TEST";

        // First write turns LED on
        let on_action = Action::LedOn {
            target: target.to_string(),
        };
        controller.execute_actions(&[on_action]).unwrap();

        // Immediate second write tries to turn LED off — should be rate-limited (skipped)
        let off_action = Action::LedOff {
            target: target.to_string(),
        };
        controller.execute_actions(&[off_action]).unwrap();

        // Verify LED is still ON (the off write was rate-limited)
        let led_target = LedTarget::Panel(target.to_string());
        let state = controller.get_led_state(&led_target);
        assert!(
            state.is_some_and(|s| s.on),
            "LED should still be ON — immediate off write should be rate-limited"
        );

        // After min_interval elapses, the off write should execute
        std::thread::sleep(min_interval + Duration::from_millis(1));
        let off_action2 = Action::LedOff {
            target: target.to_string(),
        };
        controller.execute_actions(&[off_action2]).unwrap();

        let state = controller.get_led_state(&led_target);
        assert!(
            state.is_some_and(|s| !s.on),
            "LED should be OFF after min_interval has elapsed"
        );
    }
}
