// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tactile effect types and processing

use flight_bus::BusSnapshot;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Types of tactile effects that can be generated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectType {
    /// Touchdown effect when landing
    Touchdown,
    /// Ground roll rumble during taxi/takeoff/landing
    GroundRoll,
    /// Stall buffet effect
    StallBuffet,
    /// Engine vibration
    EngineVibration,
    /// Gear warning vibration
    GearWarning,
    /// Rotor vibration (helicopters)
    RotorVibration,
}

/// Intensity level for tactile effects (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EffectIntensity(f32);

impl EffectIntensity {
    /// Create a new effect intensity
    pub fn new(value: f32) -> Result<Self, String> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "Effect intensity must be between 0.0 and 1.0, got {}",
                value
            ))
        }
    }

    /// Get the intensity value
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Create zero intensity
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// Create maximum intensity
    pub fn max() -> Self {
        Self(1.0)
    }
}

/// A tactile effect event with timing information
#[derive(Debug, Clone)]
pub struct EffectEvent {
    pub effect_type: EffectType,
    pub intensity: EffectIntensity,
    pub duration: Option<Duration>,
    pub timestamp: Instant,
}

impl EffectEvent {
    /// Create a new effect event
    pub fn new(effect_type: EffectType, intensity: EffectIntensity) -> Self {
        Self {
            effect_type,
            intensity,
            duration: None,
            timestamp: Instant::now(),
        }
    }

    /// Create a new effect event with duration
    pub fn with_duration(
        effect_type: EffectType,
        intensity: EffectIntensity,
        duration: Duration,
    ) -> Self {
        Self {
            effect_type,
            intensity,
            duration: Some(duration),
            timestamp: Instant::now(),
        }
    }

    /// Check if this effect has expired
    pub fn is_expired(&self) -> bool {
        if let Some(duration) = self.duration {
            self.timestamp.elapsed() > duration
        } else {
            false
        }
    }

    /// Get remaining duration
    pub fn remaining_duration(&self) -> Option<Duration> {
        self.duration
            .and_then(|d| d.checked_sub(self.timestamp.elapsed()))
    }
}

/// Processes telemetry data to generate tactile effects
pub struct EffectProcessor {
    last_ground_contact: bool,
    last_altitude: f32,
    last_vertical_speed: f32,
    last_ground_speed: f32,
    stall_threshold: f32,
    touchdown_threshold: f32,
    ground_roll_threshold: f32,
}

impl EffectProcessor {
    /// Create a new effect processor
    pub fn new() -> Self {
        Self {
            last_ground_contact: false,
            last_altitude: 0.0,
            last_vertical_speed: 0.0,
            last_ground_speed: 0.0,
            stall_threshold: 18.0,       // degrees AoA
            touchdown_threshold: -200.0, // fpm descent rate
            ground_roll_threshold: 5.0,  // knots ground speed
        }
    }

    /// Process telemetry snapshot and generate effect events
    pub fn process(&mut self, snapshot: &BusSnapshot) -> Vec<EffectEvent> {
        let mut events = Vec::new();

        // Check for touchdown effect
        if let Some(touchdown_event) = self.check_touchdown(snapshot) {
            events.push(touchdown_event);
        }

        // Check for ground roll effect
        if let Some(ground_roll_event) = self.check_ground_roll(snapshot) {
            events.push(ground_roll_event);
        }

        // Check for stall buffet effect
        if let Some(stall_event) = self.check_stall_buffet(snapshot) {
            events.push(stall_event);
        }

        // Check for engine vibration effect
        if let Some(engine_event) = self.check_engine_vibration(snapshot) {
            events.push(engine_event);
        }

        // Check for gear warning effect
        if let Some(gear_event) = self.check_gear_warning(snapshot) {
            events.push(gear_event);
        }

        // Check for rotor vibration (helicopters)
        if let Some(rotor_event) = self.check_rotor_vibration(snapshot) {
            events.push(rotor_event);
        }

        // Update state for next iteration
        self.update_state(snapshot);

        events
    }

    /// Check for touchdown effect
    fn check_touchdown(&self, snapshot: &BusSnapshot) -> Option<EffectEvent> {
        let on_ground = snapshot.config.gear.all_down() && snapshot.environment.altitude < 50.0;
        let vertical_speed = snapshot.kinematics.vertical_speed;

        // Touchdown detected: transition from air to ground with significant descent rate
        if !self.last_ground_contact && on_ground && vertical_speed < self.touchdown_threshold {
            let intensity = (vertical_speed.abs() / 500.0).min(1.0); // Scale by descent rate
            let intensity = EffectIntensity::new(intensity).unwrap_or(EffectIntensity::zero());

            Some(EffectEvent::with_duration(
                EffectType::Touchdown,
                intensity,
                Duration::from_millis(500), // 500ms touchdown effect
            ))
        } else {
            None
        }
    }

    /// Check for ground roll effect
    fn check_ground_roll(&self, snapshot: &BusSnapshot) -> Option<EffectEvent> {
        let on_ground = snapshot.config.gear.all_down() && snapshot.environment.altitude < 50.0;
        let ground_speed = snapshot.kinematics.ground_speed.value();

        if on_ground && ground_speed > self.ground_roll_threshold {
            // Intensity based on ground speed
            let intensity = (ground_speed / 100.0).min(1.0); // Scale up to 100 knots
            let intensity =
                EffectIntensity::new(intensity * 0.3).unwrap_or(EffectIntensity::zero()); // Moderate intensity

            Some(EffectEvent::new(EffectType::GroundRoll, intensity))
        } else {
            None
        }
    }

    /// Check for stall buffet effect
    fn check_stall_buffet(&self, snapshot: &BusSnapshot) -> Option<EffectEvent> {
        let aoa = snapshot.kinematics.aoa.value();
        let airspeed = snapshot.kinematics.ias.value();

        // Stall buffet occurs at high AoA with sufficient airspeed
        if aoa > self.stall_threshold && airspeed > 40.0 {
            let intensity = ((aoa - self.stall_threshold) / 10.0).min(1.0); // Scale beyond threshold
            let intensity = EffectIntensity::new(intensity).unwrap_or(EffectIntensity::zero());

            Some(EffectEvent::new(EffectType::StallBuffet, intensity))
        } else {
            None
        }
    }

    /// Check for engine vibration effect
    fn check_engine_vibration(&self, snapshot: &BusSnapshot) -> Option<EffectEvent> {
        if snapshot.engines.is_empty() {
            return None;
        }

        // Calculate average engine RPM
        let total_rpm: f32 = snapshot
            .engines
            .iter()
            .filter(|e| e.running)
            .map(|e| e.rpm.value())
            .sum();

        let running_engines = snapshot.engines.iter().filter(|e| e.running).count();

        if running_engines > 0 {
            let avg_rpm = total_rpm / running_engines as f32;
            let intensity = (avg_rpm / 100.0).min(1.0) * 0.2; // Low intensity engine vibration
            let intensity = EffectIntensity::new(intensity).unwrap_or(EffectIntensity::zero());

            Some(EffectEvent::new(EffectType::EngineVibration, intensity))
        } else {
            None
        }
    }

    /// Check for gear warning effect
    fn check_gear_warning(&self, snapshot: &BusSnapshot) -> Option<EffectEvent> {
        let airspeed = snapshot.kinematics.ias.value();
        let altitude = snapshot.environment.altitude;
        let gear_down = snapshot.config.gear.all_down();

        // Gear warning: low altitude, low speed, gear not down
        if altitude < 1000.0 && airspeed < 150.0 && !gear_down {
            let intensity = EffectIntensity::new(0.8).unwrap(); // High intensity warning

            Some(EffectEvent::with_duration(
                EffectType::GearWarning,
                intensity,
                Duration::from_millis(200), // Short pulse
            ))
        } else {
            None
        }
    }

    /// Check for rotor vibration (helicopters)
    fn check_rotor_vibration(&self, snapshot: &BusSnapshot) -> Option<EffectEvent> {
        if let Some(helo) = &snapshot.helo {
            let nr = helo.nr.value();
            let torque = helo.torque.value();

            // Rotor vibration based on Nr and torque
            if nr > 90.0 {
                let intensity = (torque / 100.0) * 0.4; // Scale with torque, moderate intensity
                let intensity = EffectIntensity::new(intensity).unwrap_or(EffectIntensity::zero());

                Some(EffectEvent::new(EffectType::RotorVibration, intensity))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Update internal state for next iteration
    fn update_state(&mut self, snapshot: &BusSnapshot) {
        self.last_ground_contact =
            snapshot.config.gear.all_down() && snapshot.environment.altitude < 50.0;
        self.last_altitude = snapshot.environment.altitude;
        self.last_vertical_speed = snapshot.kinematics.vertical_speed;
        self.last_ground_speed = snapshot.kinematics.ground_speed.value();
    }

    /// Configure stall threshold
    pub fn set_stall_threshold(&mut self, threshold: f32) {
        self.stall_threshold = threshold;
    }

    /// Configure touchdown threshold
    pub fn set_touchdown_threshold(&mut self, threshold: f32) {
        self.touchdown_threshold = threshold;
    }

    /// Configure ground roll threshold
    pub fn set_ground_roll_threshold(&mut self, threshold: f32) {
        self.ground_roll_threshold = threshold;
    }
}

impl Default for EffectProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::{AircraftId, BusSnapshot, SimId};

    fn create_test_snapshot() -> BusSnapshot {
        BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
    }

    #[test]
    fn test_effect_intensity_validation() {
        assert!(EffectIntensity::new(0.0).is_ok());
        assert!(EffectIntensity::new(1.0).is_ok());
        assert!(EffectIntensity::new(0.5).is_ok());
        assert!(EffectIntensity::new(-0.1).is_err());
        assert!(EffectIntensity::new(1.1).is_err());
    }

    #[test]
    fn test_effect_event_expiration() {
        let event = EffectEvent::with_duration(
            EffectType::Touchdown,
            EffectIntensity::new(0.5).unwrap(),
            Duration::from_millis(100),
        );

        assert!(!event.is_expired());
        std::thread::sleep(Duration::from_millis(150));
        assert!(event.is_expired());
    }

    #[test]
    fn test_touchdown_detection() {
        let mut processor = EffectProcessor::new();
        let mut snapshot = create_test_snapshot();

        // Set up for touchdown: in air with descent rate
        snapshot.environment.altitude = 100.0;
        snapshot.kinematics.vertical_speed = -300.0;
        processor.update_state(&snapshot);

        // Now touchdown: on ground
        snapshot.environment.altitude = 10.0;
        let events = processor.process(&snapshot);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].effect_type, EffectType::Touchdown);
        assert!(events[0].intensity.value() > 0.0);
    }

    #[test]
    fn test_stall_buffet_detection() {
        let mut processor = EffectProcessor::new();
        let mut snapshot = create_test_snapshot();

        // Set up stall conditions
        snapshot.kinematics.aoa = flight_bus::ValidatedAngle::new_degrees(20.0).unwrap();
        snapshot.kinematics.ias = flight_bus::ValidatedSpeed::new_knots(50.0).unwrap();

        let events = processor.process(&snapshot);

        let stall_events: Vec<_> = events
            .iter()
            .filter(|e| e.effect_type == EffectType::StallBuffet)
            .collect();

        assert_eq!(stall_events.len(), 1);
        assert!(stall_events[0].intensity.value() > 0.0);
    }

    #[test]
    fn test_ground_roll_detection() {
        let mut processor = EffectProcessor::new();
        let mut snapshot = create_test_snapshot();

        // Set up ground roll conditions
        snapshot.environment.altitude = 10.0;
        snapshot.kinematics.ground_speed = flight_bus::ValidatedSpeed::new_knots(30.0).unwrap();

        let events = processor.process(&snapshot);

        let ground_roll_events: Vec<_> = events
            .iter()
            .filter(|e| e.effect_type == EffectType::GroundRoll)
            .collect();

        assert_eq!(ground_roll_events.len(), 1);
        assert!(ground_roll_events[0].intensity.value() > 0.0);
    }
}
