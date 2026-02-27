// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Throttle zone processing.
//!
//! Adds named zones to throttle axes: cut zone, saturation zone, and event zones
//! (military power, afterburner, custom).
//!
//! # Example
//!
//! ```rust
//! use flight_axis::throttle_zone::{ThrottleZoneConfig, ThrottleZoneProcessor, ZoneEvent, ZoneName};
//!
//! let config = ThrottleZoneConfig::new().with_cut(0.05);
//! let mut proc = ThrottleZoneProcessor::new(config);
//! let placeholder = ZoneEvent { zone: ZoneName::Cut, entered: false, value: 0.0 };
//! let mut events = [placeholder.clone(), placeholder.clone(), placeholder.clone(), placeholder];
//! let (output, _n_events) = proc.process(0.03, &mut events);
//! assert_eq!(output, 0.0); // below cut threshold → forced to 0.0
//! ```

use thiserror::Error;

/// A named threshold zone on a throttle axis.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrottleZone {
    pub name: ZoneName,
    /// Normalized position in `[0.0, 1.0]`.
    pub threshold: f32,
    pub enabled: bool,
}

/// Names for throttle zones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneName {
    /// Below threshold → output forced to 0.0.
    Cut,
    /// Above threshold → output saturated to 1.0.
    Max,
    /// Crossing this threshold emits a MilPower event.
    MilPower,
    /// Crossing this threshold emits an Afterburner event.
    Afterburner,
    /// User-defined zone, index 0–7.
    Custom(u8),
}

/// Event emitted when a zone threshold is crossed.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneEvent {
    pub zone: ZoneName,
    /// `true` = entered the zone, `false` = exited the zone.
    pub entered: bool,
    /// Axis value at the moment of crossing.
    pub value: f32,
}

/// Configuration for throttle zone processing.
#[derive(Debug, Clone, Default)]
pub struct ThrottleZoneConfig {
    pub zones: Vec<ThrottleZone>,
}

impl ThrottleZoneConfig {
    /// Creates an empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an enabled Cut zone at `threshold`.
    pub fn with_cut(mut self, threshold: f32) -> Self {
        self.zones.push(ThrottleZone {
            name: ZoneName::Cut,
            threshold,
            enabled: true,
        });
        self
    }

    /// Adds an enabled Max (saturation) zone at `threshold`.
    pub fn with_max(mut self, threshold: f32) -> Self {
        self.zones.push(ThrottleZone {
            name: ZoneName::Max,
            threshold,
            enabled: true,
        });
        self
    }

    /// Adds an enabled MilPower event zone at `threshold`.
    pub fn with_mil_power(mut self, threshold: f32) -> Self {
        self.zones.push(ThrottleZone {
            name: ZoneName::MilPower,
            threshold,
            enabled: true,
        });
        self
    }

    /// Adds an enabled Afterburner event zone at `threshold`.
    pub fn with_afterburner(mut self, threshold: f32) -> Self {
        self.zones.push(ThrottleZone {
            name: ZoneName::Afterburner,
            threshold,
            enabled: true,
        });
        self
    }

    /// Validates the configuration.
    ///
    /// Returns an error if any enabled Cut or Max threshold is outside `[0.0, 1.0]`,
    /// or if the Cut threshold is not strictly less than the Max threshold.
    pub fn validate(&self) -> Result<(), ZoneError> {
        let cut = self
            .zones
            .iter()
            .find(|z| z.name == ZoneName::Cut && z.enabled);
        let max = self
            .zones
            .iter()
            .find(|z| z.name == ZoneName::Max && z.enabled);

        if let Some(c) = cut {
            if !(0.0..=1.0).contains(&c.threshold) {
                return Err(ZoneError::InvalidCutThreshold(c.threshold));
            }
        }
        if let Some(m) = max {
            if !(0.0..=1.0).contains(&m.threshold) {
                return Err(ZoneError::InvalidMaxThreshold(m.threshold));
            }
        }
        if let (Some(c), Some(m)) = (cut, max) {
            if c.threshold >= m.threshold {
                return Err(ZoneError::CutAboveMax {
                    cut: c.threshold,
                    max: m.threshold,
                });
            }
        }
        Ok(())
    }
}

/// Errors returned by [`ThrottleZoneConfig::validate`].
#[derive(Debug, PartialEq, Error)]
pub enum ZoneError {
    #[error("Cut threshold {0} out of range [0.0, 1.0]")]
    InvalidCutThreshold(f32),
    #[error("Max threshold {0} out of range [0.0, 1.0]")]
    InvalidMaxThreshold(f32),
    #[error("Cut threshold {cut} must be less than max threshold {max}")]
    CutAboveMax { cut: f32, max: f32 },
}

/// Processes a throttle axis value through configured zones.
///
/// Maintains the previous axis value to detect zone crossings for event emission.
pub struct ThrottleZoneProcessor {
    config: ThrottleZoneConfig,
    prev_value: f32,
}

impl ThrottleZoneProcessor {
    /// Creates a new processor with the given configuration.
    pub fn new(config: ThrottleZoneConfig) -> Self {
        Self {
            config,
            prev_value: 0.0,
        }
    }

    /// Processes `input` through all configured zones.
    ///
    /// Input is clamped to `[0.0, 1.0]` before processing. Returns the output value
    /// and the number of events written into `events`. Up to 4 events are emitted per call;
    /// events are only generated for `MilPower`, `Afterburner`, and `Custom` zones.
    pub fn process(&mut self, input: f32, events: &mut [ZoneEvent; 4]) -> (f32, usize) {
        let clamped = input.clamp(0.0, 1.0);
        let prev = self.prev_value;
        let mut output = clamped;

        // Apply Cut zone: values at or below the threshold are forced to 0.0.
        if let Some(cut) = self
            .config
            .zones
            .iter()
            .find(|z| z.name == ZoneName::Cut && z.enabled)
        {
            if clamped <= cut.threshold {
                output = 0.0;
            }
        }

        // Apply Max zone: values at or above the threshold are saturated to 1.0.
        if let Some(max) = self
            .config
            .zones
            .iter()
            .find(|z| z.name == ZoneName::Max && z.enabled)
        {
            if clamped >= max.threshold {
                output = 1.0;
            }
        }

        // Emit crossing events for MilPower, Afterburner, and Custom zones.
        let mut n = 0usize;
        for zone in &self.config.zones {
            if n >= 4 {
                break;
            }
            if !zone.enabled {
                continue;
            }
            if !matches!(
                zone.name,
                ZoneName::MilPower | ZoneName::Afterburner | ZoneName::Custom(_)
            ) {
                continue;
            }
            let t = zone.threshold;
            if prev < t && clamped >= t {
                events[n] = ZoneEvent {
                    zone: zone.name.clone(),
                    entered: true,
                    value: clamped,
                };
                n += 1;
            } else if clamped < t && prev >= t {
                events[n] = ZoneEvent {
                    zone: zone.name.clone(),
                    entered: false,
                    value: clamped,
                };
                n += 1;
            }
        }

        self.prev_value = clamped;
        (output, n)
    }

    /// Returns a reference to the zone configuration.
    pub fn config(&self) -> &ThrottleZoneConfig {
        &self.config
    }

    /// Resets the previous axis value to 0.0.
    pub fn reset(&mut self) {
        self.prev_value = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_events() -> [ZoneEvent; 4] {
        let placeholder = ZoneEvent {
            zone: ZoneName::Cut,
            entered: false,
            value: 0.0,
        };
        [
            placeholder.clone(),
            placeholder.clone(),
            placeholder.clone(),
            placeholder,
        ]
    }

    #[test]
    fn test_no_zones_passthrough() {
        let mut proc = ThrottleZoneProcessor::new(ThrottleZoneConfig::new());
        let mut ev = make_events();
        let (out, n) = proc.process(0.5, &mut ev);
        assert_eq!(out, 0.5);
        assert_eq!(n, 0);
    }

    #[test]
    fn test_cut_zone_below_threshold() {
        let config = ThrottleZoneConfig::new().with_cut(0.1);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();
        let (out, _) = proc.process(0.05, &mut ev);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn test_cut_zone_above_threshold() {
        let config = ThrottleZoneConfig::new().with_cut(0.1);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();
        let (out, _) = proc.process(0.5, &mut ev);
        assert_eq!(out, 0.5);
    }

    #[test]
    fn test_cut_zone_at_threshold() {
        let config = ThrottleZoneConfig::new().with_cut(0.1);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();
        // exactly at threshold → cut applies (input <= threshold)
        let (out, _) = proc.process(0.1, &mut ev);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn test_max_zone_saturation() {
        let config = ThrottleZoneConfig::new().with_max(0.9);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();
        let (out, _) = proc.process(0.95, &mut ev);
        assert_eq!(out, 1.0);
    }

    #[test]
    fn test_max_zone_below_threshold() {
        let config = ThrottleZoneConfig::new().with_max(0.9);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();
        let (out, _) = proc.process(0.5, &mut ev);
        assert_eq!(out, 0.5);
    }

    #[test]
    fn test_milpower_zone_event_on_entry() {
        let config = ThrottleZoneConfig::new().with_mil_power(0.85);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();

        // First call: below threshold, no event
        let (_, n) = proc.process(0.8, &mut ev);
        assert_eq!(n, 0);

        // Second call: crosses threshold upward → entered event
        let (_, n) = proc.process(0.9, &mut ev);
        assert_eq!(n, 1);
        assert_eq!(ev[0].zone, ZoneName::MilPower);
        assert!(ev[0].entered);
        assert_eq!(ev[0].value, 0.9);
    }

    #[test]
    fn test_milpower_zone_event_on_exit() {
        let config = ThrottleZoneConfig::new().with_mil_power(0.85);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();

        // Start above threshold
        proc.process(0.9, &mut ev);

        // Cross threshold downward → exited event
        let (_, n) = proc.process(0.8, &mut ev);
        assert_eq!(n, 1);
        assert_eq!(ev[0].zone, ZoneName::MilPower);
        assert!(!ev[0].entered);
        assert_eq!(ev[0].value, 0.8);
    }

    #[test]
    fn test_afterburner_zone_event() {
        let config = ThrottleZoneConfig::new().with_afterburner(0.95);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();

        proc.process(0.9, &mut ev);
        let (_, n) = proc.process(1.0, &mut ev);
        assert_eq!(n, 1);
        assert_eq!(ev[0].zone, ZoneName::Afterburner);
        assert!(ev[0].entered);
    }

    #[test]
    fn test_combined_cut_and_milpower() {
        let config = ThrottleZoneConfig::new()
            .with_cut(0.05)
            .with_mil_power(0.85);
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();

        // Input below cut → output 0.0, no event
        let (out, n) = proc.process(0.03, &mut ev);
        assert_eq!(out, 0.0);
        assert_eq!(n, 0);

        // Input crosses milpower threshold → milpower event, no cut applied
        let (out, n) = proc.process(0.9, &mut ev);
        assert_eq!(out, 0.9);
        assert_eq!(n, 1);
        assert_eq!(ev[0].zone, ZoneName::MilPower);
        assert!(ev[0].entered);
    }

    #[test]
    fn test_validate_invalid_cut_threshold() {
        let config = ThrottleZoneConfig::new().with_cut(1.5);
        assert_eq!(config.validate(), Err(ZoneError::InvalidCutThreshold(1.5)));
    }

    #[test]
    fn test_validate_cut_above_max_error() {
        let config = ThrottleZoneConfig::new().with_cut(0.5).with_max(0.3);
        assert_eq!(
            config.validate(),
            Err(ZoneError::CutAboveMax { cut: 0.5, max: 0.3 })
        );
    }

    #[test]
    fn test_process_clamped_input_above_one() {
        let config = ThrottleZoneConfig::new();
        let mut proc = ThrottleZoneProcessor::new(config);
        let mut ev = make_events();
        let (out, n) = proc.process(1.5, &mut ev);
        assert_eq!(out, 1.0);
        assert_eq!(n, 0);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ThrottleZoneConfig::new()
            .with_cut(0.05)
            .with_max(0.95)
            .with_mil_power(0.85)
            .with_afterburner(0.97);
        assert_eq!(config.zones.len(), 4);
        assert_eq!(config.zones[0].name, ZoneName::Cut);
        assert_eq!(config.zones[0].threshold, 0.05);
        assert_eq!(config.zones[1].name, ZoneName::Max);
        assert_eq!(config.zones[2].name, ZoneName::MilPower);
        assert_eq!(config.zones[3].name, ZoneName::Afterburner);
        assert!(config.zones.iter().all(|z| z.enabled));
    }

    proptest! {
        /// Output is always in [0.0, 1.0] for any input, regardless of zone configuration.
        #[test]
        fn prop_output_always_in_range(input in -1_000_000.0f32..=1_000_000.0f32) {
            let config = ThrottleZoneConfig::new()
                .with_cut(0.05)
                .with_max(0.95)
                .with_mil_power(0.85)
                .with_afterburner(0.97);
            let mut proc = ThrottleZoneProcessor::new(config);
            let mut ev = make_events();
            let (out, _) = proc.process(input, &mut ev);
            prop_assert!(
                out >= 0.0 && out <= 1.0,
                "output {} out of [0, 1] for input {}",
                out, input
            );
        }

        /// Event count never exceeds 4 regardless of configuration or input sequence.
        #[test]
        fn prop_event_count_never_exceeds_4(
            a in 0.0f32..=1.0f32,
            b in 0.0f32..=1.0f32,
        ) {
            let config = ThrottleZoneConfig::new()
                .with_mil_power(0.2)
                .with_mil_power(0.4)
                .with_afterburner(0.6)
                .with_afterburner(0.8);
            let mut proc = ThrottleZoneProcessor::new(config);
            let mut ev = make_events();
            proc.process(a, &mut ev);
            let (_, n) = proc.process(b, &mut ev);
            prop_assert!(n <= 4, "event count {} exceeds 4", n);
        }
    }
}
