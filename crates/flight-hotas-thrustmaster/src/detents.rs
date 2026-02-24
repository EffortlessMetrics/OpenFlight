// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Throttle detent detection for Thrustmaster T.Flight HOTAS devices.
//!
//! The T.Flight HOTAS 4 and HOTAS One throttle unit contains a physical
//! resistance notch ("detent") near the idle position. Pulling the
//! throttle up through the notch requires a small extra force, giving
//! tactile feedback for the idle/afterburner gate.
//!
//! Because the detent is mechanical only — no separate HID event is
//! generated — OpenFlight detects detent crossings in software by
//! monitoring the throttle axis value as it passes through a configured
//! zone.
//!
//! # Default HOTAS 4 Detent
//!
//! The factory notch sits at approximately **5 %** throttle (0.05 in
//! normalised 0.0–1.0 range). [`ThrottleDetentConfig::hotas4_idle`]
//! provides the recommended configuration with:
//!
//! - `position`: 0.05  
//! - `half_width` (one-sided hysteresis): 0.02  
//!
//! The active zone is therefore `[0.03, 0.07]`. The throttle must leave
//! this zone before the same event can fire again (hysteresis prevents
//! rapid chatter near the boundary).
//!
//! # Example
//!
//! ```
//! use flight_hotas_thrustmaster::detents::{
//!     DetentEvent, ThrottleDetentConfig, ThrottleDetentTracker,
//! };
//!
//! let config = ThrottleDetentConfig::hotas4_idle();
//! let mut tracker = ThrottleDetentTracker::new(vec![config]);
//!
//! // Move throttle into the detent zone
//! let events = tracker.update(0.05);
//! assert!(events.iter().any(|e| matches!(e, DetentEvent::Entered { .. })));
//!
//! // Moving clearly below the zone exits it
//! let events = tracker.update(0.00);
//! assert!(events.iter().any(|e| matches!(e, DetentEvent::Exited { .. })));
//! ```

/// A configured throttle detent zone.
///
/// The active zone spans `[position - half_width, position + half_width]`.
#[derive(Debug, Clone)]
pub struct ThrottleDetentConfig {
    /// Human-readable name for this detent (e.g. `"idle"`, `"afterburner"`).
    pub name: &'static str,
    /// Index used to identify this detent in [`DetentEvent`].
    pub index: usize,
    /// Centre position of the detent in normalised 0.0–1.0 throttle range.
    pub position: f32,
    /// One-sided half-width of the hysteresis zone.
    ///
    /// The active zone is `[position - half_width, position + half_width]`.
    /// The throttle must leave this zone before `Entered` fires again.
    pub half_width: f32,
}

impl ThrottleDetentConfig {
    /// Default idle-gate detent for HOTAS 4 (~5 % with ±2 % hysteresis).
    pub const fn hotas4_idle() -> Self {
        Self {
            name: "idle",
            index: 0,
            position: 0.05,
            half_width: 0.02,
        }
    }

    /// Returns the lower bound of the detent zone.
    pub fn lower(&self) -> f32 {
        (self.position - self.half_width).max(0.0)
    }

    /// Returns the upper bound of the detent zone.
    pub fn upper(&self) -> f32 {
        (self.position + self.half_width).min(1.0)
    }

    /// Returns `true` if `value` falls within the detent zone.
    pub fn contains(&self, value: f32) -> bool {
        value >= self.lower() && value <= self.upper()
    }
}

/// An event emitted by [`ThrottleDetentTracker`] when the throttle
/// crosses a detent boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum DetentEvent {
    /// Throttle entered the detent zone.
    Entered {
        /// Index of the detent that was crossed (matches [`ThrottleDetentConfig::index`]).
        detent_index: usize,
        /// Throttle value at the moment of entry.
        value: f32,
    },
    /// Throttle left the detent zone (either side).
    Exited {
        /// Index of the detent that was left.
        detent_index: usize,
        /// Throttle value at the moment of exit.
        value: f32,
    },
}

/// Per-detent tracking state.
#[derive(Debug, Clone)]
struct DetentState {
    config: ThrottleDetentConfig,
    /// Whether the throttle is currently inside this detent zone.
    inside: bool,
}

/// Tracks throttle position and emits [`DetentEvent`]s as the throttle
/// crosses configured detent zones.
///
/// Create a tracker with one or more [`ThrottleDetentConfig`]s, then call
/// [`update`](Self::update) on each incoming throttle sample. Any detent
/// crossing events are returned from that call.
#[derive(Debug, Clone)]
pub struct ThrottleDetentTracker {
    detents: Vec<DetentState>,
}

impl ThrottleDetentTracker {
    /// Create a tracker for the given set of detent configurations.
    pub fn new(configs: Vec<ThrottleDetentConfig>) -> Self {
        Self {
            detents: configs
                .into_iter()
                .map(|c| DetentState {
                    config: c,
                    inside: false,
                })
                .collect(),
        }
    }

    /// Create a tracker pre-configured with the standard HOTAS 4 idle detent.
    pub fn hotas4_default() -> Self {
        Self::new(vec![ThrottleDetentConfig::hotas4_idle()])
    }

    /// Process a throttle sample and return any crossing events.
    ///
    /// `throttle` must be in the range 0.0–1.0.
    pub fn update(&mut self, throttle: f32) -> Vec<DetentEvent> {
        let mut events = Vec::new();
        for ds in &mut self.detents {
            let now_inside = ds.config.contains(throttle);
            if now_inside && !ds.inside {
                events.push(DetentEvent::Entered {
                    detent_index: ds.config.index,
                    value: throttle,
                });
                ds.inside = true;
            } else if !now_inside && ds.inside {
                events.push(DetentEvent::Exited {
                    detent_index: ds.config.index,
                    value: throttle,
                });
                ds.inside = false;
            }
        }
        events
    }

    /// Returns `true` if the throttle is currently inside any detent zone.
    pub fn any_active(&self) -> bool {
        self.detents.iter().any(|d| d.inside)
    }

    /// Returns `true` if the throttle is inside the detent at `index`.
    pub fn is_active(&self, index: usize) -> bool {
        self.detents
            .iter()
            .find(|d| d.config.index == index)
            .is_some_and(|d| d.inside)
    }

    /// Reset all detent states (clears the `inside` flag for all detents).
    ///
    /// The next call to [`update`](Self::update) will re-detect crossings from
    /// a fresh baseline.
    pub fn reset(&mut self) {
        for ds in &mut self.detents {
            ds.inside = false;
        }
    }

    /// Iterate over the configured detents.
    pub fn detents(&self) -> impl Iterator<Item = &ThrottleDetentConfig> {
        self.detents.iter().map(|d| &d.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ThrottleDetentConfig ────────────────────────────────────────────────

    #[test]
    fn test_default_hotas4_idle_detent() {
        let d = ThrottleDetentConfig::hotas4_idle();
        assert_eq!(d.position, 0.05);
        assert_eq!(d.half_width, 0.02);
        assert!((d.lower() - 0.03).abs() < 1e-6, "lower={}", d.lower());
        assert!((d.upper() - 0.07).abs() < 1e-6, "upper={}", d.upper());
    }

    #[test]
    fn test_contains_inside() {
        let d = ThrottleDetentConfig::hotas4_idle();
        assert!(d.contains(0.05));
        // Use slightly inward bounds to avoid f32 boundary precision issues
        assert!(d.contains(0.035));
        assert!(d.contains(0.065));
    }

    #[test]
    fn test_contains_outside() {
        let d = ThrottleDetentConfig::hotas4_idle();
        assert!(!d.contains(0.00));
        assert!(!d.contains(0.10));
    }

    #[test]
    fn test_lower_clamped_at_zero() {
        let d = ThrottleDetentConfig {
            name: "low",
            index: 0,
            position: 0.01,
            half_width: 0.05,
        };
        assert_eq!(d.lower(), 0.0);
    }

    #[test]
    fn test_upper_clamped_at_one() {
        let d = ThrottleDetentConfig {
            name: "high",
            index: 0,
            position: 0.99,
            half_width: 0.05,
        };
        assert_eq!(d.upper(), 1.0);
    }

    // ── ThrottleDetentTracker ───────────────────────────────────────────────

    #[test]
    fn test_entered_event_when_throttle_enters_zone() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        // Start outside
        t.update(0.20);
        // Move into zone
        let events = t.update(0.05);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DetentEvent::Entered { detent_index: 0, .. }));
    }

    #[test]
    fn test_exited_event_when_throttle_leaves_zone() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        t.update(0.05); // enter
        let events = t.update(0.00); // exit low side
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DetentEvent::Exited { detent_index: 0, .. }));
    }

    #[test]
    fn test_no_event_while_inside_zone() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        t.update(0.05); // enter — fires Entered
        let e1 = t.update(0.04);
        let e2 = t.update(0.06);
        let e3 = t.update(0.05);
        assert!(e1.is_empty());
        assert!(e2.is_empty());
        assert!(e3.is_empty());
    }

    #[test]
    fn test_entered_fires_again_after_exit_and_reenter() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        t.update(0.05); // enter
        t.update(0.20); // exit
        let events = t.update(0.05); // enter again
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DetentEvent::Entered { .. }));
    }

    #[test]
    fn test_no_events_when_never_in_zone() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        for v in [0.0, 0.20, 0.50, 1.0] {
            let e = t.update(v);
            assert!(e.is_empty(), "unexpected events at {v}: {e:?}");
        }
    }

    #[test]
    fn test_any_active_true_when_inside() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        t.update(0.05);
        assert!(t.any_active());
    }

    #[test]
    fn test_any_active_false_when_outside() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        t.update(0.20);
        assert!(!t.any_active());
    }

    #[test]
    fn test_reset_clears_state() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        t.update(0.05); // enter
        assert!(t.any_active());
        t.reset();
        assert!(!t.any_active());
        // After reset, entering the zone should fire Entered again
        let events = t.update(0.05);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DetentEvent::Entered { .. }));
    }

    #[test]
    fn test_multiple_detents_independent() {
        let configs = vec![
            ThrottleDetentConfig { name: "idle", index: 0, position: 0.05, half_width: 0.02 },
            ThrottleDetentConfig {
                name: "mil",
                index: 1,
                position: 0.85,
                half_width: 0.03,
            },
        ];
        let mut t = ThrottleDetentTracker::new(configs);

        let e1 = t.update(0.05); // enter detent 0
        assert_eq!(e1.len(), 1);
        assert!(matches!(e1[0], DetentEvent::Entered { detent_index: 0, .. }));

        let e2 = t.update(0.85); // exit 0, enter 1
        // Exit from detent 0, Enter detent 1
        assert_eq!(e2.len(), 2);
    }

    #[test]
    fn test_event_value_matches_throttle() {
        let mut t = ThrottleDetentTracker::hotas4_default();
        t.update(0.20);
        let events = t.update(0.05);
        if let DetentEvent::Entered { value, .. } = &events[0] {
            assert!((value - 0.05).abs() < 1e-6);
        } else {
            panic!("expected Entered event");
        }
    }
}
