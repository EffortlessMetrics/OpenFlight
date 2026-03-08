// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Telemetry-based force feedback effect synthesis
//!
//! This module implements force feedback effects derived from flight telemetry data,
//! including stall buffet, touchdown impulse, ground roll effects, gear warnings,
//! and helicopter rotor effects. All effects are rate-limited and run off the RT thread.

use crate::{Result, TimeSource};
use flight_bus::{BusSnapshot, HeloData, Kinematics};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

/// Configuration for telemetry synthesis effects
#[derive(Debug, Clone)]
pub struct TelemetrySynthConfig {
    /// Stall buffet configuration
    pub stall_buffet: StallBuffetConfig,
    /// Touchdown impulse configuration
    pub touchdown: TouchdownConfig,
    /// Ground roll effects configuration
    pub ground_roll: GroundRollConfig,
    /// Gear warning configuration
    pub gear_warning: GearWarningConfig,
    /// Helicopter rotor effects configuration
    pub rotor_effects: RotorEffectsConfig,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitingConfig,
}

/// Stall buffet effect configuration
#[derive(Debug, Clone)]
pub struct StallBuffetConfig {
    /// Angle of attack threshold for stall buffet onset (degrees)
    pub aoa_threshold_deg: f32,
    /// Maximum buffet intensity (0.0 to 1.0)
    pub max_intensity: f32,
    /// Buffet frequency at maximum intensity (Hz)
    pub max_frequency_hz: f32,
    /// Buffet ramp rate (intensity per degree AoA)
    pub ramp_rate: f32,
    /// Enable stall buffet effect
    pub enabled: bool,
}

/// Touchdown impulse effect configuration
#[derive(Debug, Clone)]
pub struct TouchdownConfig {
    /// Vertical speed threshold for touchdown detection (ft/min, negative)
    pub vs_threshold_fpm: f32,
    /// Maximum impulse magnitude (Nm)
    pub max_impulse_nm: f32,
    /// Impulse duration (milliseconds)
    pub duration_ms: u32,
    /// Ground proximity threshold (feet AGL)
    pub ground_proximity_ft: f32,
    /// Enable touchdown impulse effect
    pub enabled: bool,
}

/// Ground roll effects configuration
#[derive(Debug, Clone)]
pub struct GroundRollConfig {
    /// Ground speed threshold for ground roll effects (knots)
    pub speed_threshold_kt: f32,
    /// Maximum rumble intensity (0.0 to 1.0)
    pub max_intensity: f32,
    /// Rumble frequency (Hz)
    pub frequency_hz: f32,
    /// Surface roughness multiplier
    pub roughness_multiplier: f32,
    /// Enable ground roll effects
    pub enabled: bool,
}

/// Gear warning effect configuration
#[derive(Debug, Clone)]
pub struct GearWarningConfig {
    /// Airspeed threshold for gear warning (knots)
    pub speed_threshold_kt: f32,
    /// Warning pulse intensity (0.0 to 1.0)
    pub pulse_intensity: f32,
    /// Warning pulse frequency (Hz)
    pub pulse_frequency_hz: f32,
    /// Altitude threshold for gear warning (feet AGL)
    pub altitude_threshold_ft: f32,
    /// Enable gear warning effect
    pub enabled: bool,
}

/// Helicopter rotor effects configuration
#[derive(Debug, Clone)]
pub struct RotorEffectsConfig {
    /// Nr (main rotor) low threshold (percentage)
    pub nr_low_threshold: f32,
    /// Np (power turbine) low threshold (percentage)
    pub np_low_threshold: f32,
    /// Low rotor warning intensity (0.0 to 1.0)
    pub warning_intensity: f32,
    /// Rotor vibration base frequency (Hz)
    pub base_frequency_hz: f32,
    /// Torque feedback scaling factor
    pub torque_scaling: f32,
    /// Enable rotor effects
    pub enabled: bool,
}

/// Rate limiting configuration for effect updates
#[derive(Debug, Clone)]
pub struct RateLimitingConfig {
    /// Maximum update rate (Hz)
    pub max_update_rate_hz: f32,
    /// Minimum interval between updates (milliseconds)
    pub min_interval_ms: u32,
    /// Effect smoothing factor (0.0 to 1.0)
    pub smoothing_factor: f32,
}

/// Telemetry synthesis effect engine
pub struct TelemetrySynthEngine {
    config: TelemetrySynthConfig,
    last_update: Instant,
    last_snapshot: Option<BusSnapshot>,
    effect_state: EffectState,
    rate_limiter: RateLimiter,
    blackbox_markers: VecDeque<BlackboxMarker>,
    user_tuning: UserTuningInterface,
    time_source: Arc<dyn TimeSource>,
}

/// Internal state for all effects
#[derive(Debug, Default)]
pub struct EffectState {
    pub stall_buffet: StallBuffetState,
    pub touchdown: TouchdownState,
    pub ground_roll: GroundRollState,
    pub gear_warning: GearWarningState,
    pub rotor_effects: RotorEffectsState,
}

/// Stall buffet effect state
#[derive(Debug, Default)]
pub struct StallBuffetState {
    pub current_intensity: f32,
    pub current_frequency: f32,
    pub phase: f32,
    pub last_aoa: f32,
}

/// Touchdown impulse effect state
#[derive(Debug, Default)]
pub struct TouchdownState {
    pub impulse_active: bool,
    pub impulse_start: Option<Instant>,
    pub impulse_magnitude: f32,
    pub last_vs: f32,
    pub last_altitude: f32,
    pub touchdown_detected: bool,
}

/// Ground roll effects state
#[derive(Debug, Default)]
pub struct GroundRollState {
    pub current_intensity: f32,
    pub phase: f32,
    pub on_ground: bool,
    pub last_ground_speed: f32,
}

/// Gear warning effect state
#[derive(Debug, Default)]
pub struct GearWarningState {
    pub warning_active: bool,
    pub pulse_phase: f32,
    pub last_gear_state: bool,
    pub last_airspeed: f32,
    pub last_altitude: f32,
}

/// Helicopter rotor effects state
#[derive(Debug, Default)]
pub struct RotorEffectsState {
    pub nr_warning_active: bool,
    pub np_warning_active: bool,
    pub vibration_phase: f32,
    pub torque_feedback: f32,
    pub last_nr: f32,
    pub last_np: f32,
    pub last_torque: f32,
}

/// Rate limiter for effect updates
struct RateLimiter {
    last_update: Instant,
    min_interval: Duration,
    smoothing_factor: f32,
    last_output: f32,
}

impl RateLimiter {
    /// Check if update should proceed based on rate limit
    fn check(&mut self, now: Instant) -> bool {
        if now.duration_since(self.last_update) < self.min_interval {
            false
        } else {
            self.last_update = now;
            true
        }
    }

    /// Apply smoothing to the output value
    fn apply_smoothing(&mut self, current: f32) -> f32 {
        let smoothed = self.last_output * (1.0 - self.smoothing_factor)
            + current * self.smoothing_factor;
        self.last_output = smoothed;
        smoothed
    }
}

/// Blackbox marker for effect events
#[derive(Debug, Clone)]
pub struct BlackboxMarker {
    pub timestamp: Instant,
    pub effect_type: String,
    pub event: String,
    pub parameters: std::collections::HashMap<String, f32>,
}

/// User tuning interface for real-time effect adjustment
pub struct UserTuningInterface {
    stall_buffet_intensity: f32,
    touchdown_sensitivity: f32,
    ground_roll_intensity: f32,
    gear_warning_sensitivity: f32,
    rotor_sensitivity: f32,
    global_intensity: f32,
}

/// Combined effect output
#[derive(Debug, Clone, Default)]
pub struct EffectOutput {
    /// Total torque contribution (Nm)
    pub torque_nm: f32,
    /// Effect frequency (Hz)
    pub frequency_hz: f32,
    /// Effect intensity (0.0 to 1.0)
    pub intensity: f32,
    /// Active effect types
    pub active_effects: Vec<String>,
}

impl Default for TelemetrySynthConfig {
    fn default() -> Self {
        Self {
            stall_buffet: StallBuffetConfig {
                aoa_threshold_deg: 12.0,
                max_intensity: 0.8,
                max_frequency_hz: 15.0,
                ramp_rate: 0.1,
                enabled: true,
            },
            touchdown: TouchdownConfig {
                vs_threshold_fpm: -200.0,
                max_impulse_nm: 5.0,
                duration_ms: 150,
                ground_proximity_ft: 50.0,
                enabled: true,
            },
            ground_roll: GroundRollConfig {
                speed_threshold_kt: 5.0,
                max_intensity: 0.4,
                frequency_hz: 8.0,
                roughness_multiplier: 1.0,
                enabled: true,
            },
            gear_warning: GearWarningConfig {
                speed_threshold_kt: 120.0,
                pulse_intensity: 0.6,
                pulse_frequency_hz: 2.0,
                altitude_threshold_ft: 1000.0,
                enabled: true,
            },
            rotor_effects: RotorEffectsConfig {
                nr_low_threshold: 95.0,
                np_low_threshold: 95.0,
                warning_intensity: 0.7,
                base_frequency_hz: 12.0,
                torque_scaling: 0.3,
                enabled: true,
            },
            rate_limiting: RateLimitingConfig {
                max_update_rate_hz: 60.0,
                min_interval_ms: 16, // ~60Hz
                smoothing_factor: 0.1,
            },
        }
    }
}

impl TelemetrySynthEngine {
    /// Create a new telemetry synthesis engine
    pub fn new(config: TelemetrySynthConfig) -> Self {
        Self::with_time_source(config, Arc::new(crate::DefaultTimeSource))
    }

    /// Create a new telemetry synthesis engine with time source
    pub fn with_time_source(config: TelemetrySynthConfig, time_source: Arc<dyn TimeSource>) -> Self {
        let min_interval = Duration::from_millis(config.rate_limiting.min_interval_ms as u64);
        let smoothing_factor = config.rate_limiting.smoothing_factor;
        let now = time_source.now();

        Self {
            config,
            last_update: now,
            last_snapshot: None,
            effect_state: EffectState::default(),
            rate_limiter: RateLimiter {
                last_update: now,
                min_interval,
                smoothing_factor,
                last_output: 0.0,
            },
            blackbox_markers: VecDeque::new(),
            user_tuning: UserTuningInterface::default(),
            time_source,
        }
    }

    /// Update effects based on telemetry snapshot
    pub fn update(&mut self, snapshot: &BusSnapshot) -> Result<EffectOutput> {
        let now = self.time_source.now();
        let dt = now.duration_since(self.last_update).as_secs_f32();

        // Rate limiting check
        if !self.rate_limiter.check(now) {
            // Return last computed output if rate limited
            return Ok(self.compute_current_output());
        }

        self.last_update = now;

        // Update individual effects
        self.update_stall_buffet(&snapshot.kinematics, dt)?;
        self.update_touchdown_impulse(&snapshot.kinematics)?;
        self.update_ground_roll(&snapshot.kinematics, dt)?;

        // Clone helo data to avoid borrowing issues
        let helo_data = snapshot.helo.clone();
        self.update_gear_warning(snapshot, dt)?;

        // Update helicopter effects if applicable
        if let Some(ref helo_data) = helo_data {
            self.update_rotor_effects(helo_data, dt)?;
        }

        // Store snapshot for next update
        self.last_snapshot = Some(snapshot.clone());

        // Compute combined output
        let output = self.compute_combined_output();

        // Apply rate limiting and smoothing
        let smoothed_output = self.apply_rate_limiting(output);

        Ok(smoothed_output)
    }

    /// Update stall buffet effect based on angle of attack
    fn update_stall_buffet(&mut self, kinematics: &Kinematics, dt: f32) -> Result<()> {
        if !self.config.stall_buffet.enabled {
            self.effect_state.stall_buffet.current_intensity = 0.0;
            return Ok(());
        }

        let aoa_deg = kinematics.aoa.value();
        let threshold = self.config.stall_buffet.aoa_threshold_deg;

        // Calculate buffet intensity based on AoA
        let intensity = if aoa_deg > threshold {
            let excess_aoa = aoa_deg - threshold;
            let raw_intensity = excess_aoa * self.config.stall_buffet.ramp_rate;
            (raw_intensity * self.user_tuning.stall_buffet_intensity)
                .min(self.config.stall_buffet.max_intensity)
        } else {
            0.0
        };

        // Update state
        let state = &mut self.effect_state.stall_buffet;
        let intensity_changed = (state.current_intensity - intensity).abs() > 0.01;

        state.current_intensity = intensity;
        state.current_frequency = intensity * self.config.stall_buffet.max_frequency_hz;
        state.last_aoa = aoa_deg;

        // Update phase for oscillation
        if intensity > 0.0 {
            state.phase += state.current_frequency * dt * 2.0 * std::f32::consts::PI;
            state.phase = state.phase % (2.0 * std::f32::consts::PI);
        }

        // Record blackbox marker for significant changes
        if intensity_changed && intensity > 0.1 {
            let marker = BlackboxMarker {
                timestamp: self.time_source.now(),
                effect_type: "stall_buffet".to_string(),
                event: "intensity_change".to_string(),
                parameters: [
                    ("aoa_deg".to_string(), aoa_deg),
                    ("intensity".to_string(), intensity),
                    ("frequency_hz".to_string(), state.current_frequency),
                ]
                .into(),
            };
            self.blackbox_markers.push_back(marker);

            // Keep only recent markers (last 1000)
            while self.blackbox_markers.len() > 1000 {
                self.blackbox_markers.pop_front();
            }
        }

        trace!(
            "Stall buffet: AoA={:.1}°, intensity={:.2}, freq={:.1}Hz",
            aoa_deg, intensity, state.current_frequency
        );

        Ok(())
    }

    /// Update touchdown impulse effect
    fn update_touchdown_impulse(&mut self, kinematics: &Kinematics) -> Result<()> {
        if !self.config.touchdown.enabled {
            self.effect_state.touchdown.impulse_active = false;
            return Ok(());
        }

        let vs_fpm = kinematics.vertical_speed;
        let altitude_ft = 0.0; // Would need to get from environment data

        let state = &mut self.effect_state.touchdown;

        // Detect touchdown conditions
        let approaching_ground = altitude_ft < self.config.touchdown.ground_proximity_ft;
        let touchdown_transition = state.last_vs < self.config.touchdown.vs_threshold_fpm
            && vs_fpm >= self.config.touchdown.vs_threshold_fpm
            && approaching_ground;

        if touchdown_transition && !state.touchdown_detected {
            // Trigger touchdown impulse
            let impulse_magnitude = (vs_fpm.abs() / 500.0).min(1.0)
                * self.config.touchdown.max_impulse_nm
                * self.user_tuning.touchdown_sensitivity;

            state.impulse_active = true;
            state.impulse_start = Some(self.time_source.now());
            state.impulse_magnitude = impulse_magnitude;
            state.touchdown_detected = true;

            let marker = BlackboxMarker {
                timestamp: self.time_source.now(),
                effect_type: "touchdown".to_string(),
                event: "impulse_triggered".to_string(),
                parameters: [
                    ("vs_fpm".to_string(), vs_fpm),
                    ("magnitude_nm".to_string(), impulse_magnitude),
                    ("altitude_ft".to_string(), altitude_ft),
                ]
                .into(),
            };
            self.blackbox_markers.push_back(marker);

            // Keep only recent markers (last 1000)
            while self.blackbox_markers.len() > 1000 {
                self.blackbox_markers.pop_front();
            }

            debug!(
                "Touchdown impulse triggered: VS={:.0} fpm, magnitude={:.2} Nm",
                vs_fpm, impulse_magnitude
            );
        }

        // Update impulse state
        if let Some(start_time) = state.impulse_start {
            let elapsed = self.time_source.now().duration_since(start_time);
            let duration = Duration::from_millis(self.config.touchdown.duration_ms as u64);

            if elapsed > duration {
                state.impulse_active = false;
                state.impulse_start = None;
                state.impulse_magnitude = 0.0;
            }
        }

        // Reset touchdown detection when airborne
        if altitude_ft > self.config.touchdown.ground_proximity_ft * 2.0 {
            state.touchdown_detected = false;
        }

        state.last_vs = vs_fpm;
        state.last_altitude = altitude_ft;

        Ok(())
    }

    /// Update ground roll effects
    fn update_ground_roll(&mut self, kinematics: &Kinematics, dt: f32) -> Result<()> {
        if !self.config.ground_roll.enabled {
            self.effect_state.ground_roll.current_intensity = 0.0;
            return Ok(());
        }

        let ground_speed_kt = kinematics.ground_speed.value();
        let on_ground = ground_speed_kt > 0.0 && kinematics.g_force.value() > 0.8; // Simplified ground detection

        let state = &mut self.effect_state.ground_roll;

        // Calculate ground roll intensity
        let intensity = if on_ground && ground_speed_kt > self.config.ground_roll.speed_threshold_kt
        {
            let speed_factor = (ground_speed_kt / 100.0).min(1.0); // Normalize to 100 knots
            let surface_factor = self.config.ground_roll.roughness_multiplier;
            speed_factor
                * surface_factor
                * self.config.ground_roll.max_intensity
                * self.user_tuning.ground_roll_intensity
        } else {
            0.0
        };

        state.current_intensity = intensity;
        state.on_ground = on_ground;
        state.last_ground_speed = ground_speed_kt;

        // Update phase for rumble effect
        if intensity > 0.0 {
            state.phase += self.config.ground_roll.frequency_hz * dt * 2.0 * std::f32::consts::PI;
            state.phase = state.phase % (2.0 * std::f32::consts::PI);
        }

        trace!(
            "Ground roll: speed={:.1}kt, on_ground={}, intensity={:.2}",
            ground_speed_kt, on_ground, intensity
        );

        Ok(())
    }

    /// Update gear warning effect
    fn update_gear_warning(&mut self, snapshot: &BusSnapshot, dt: f32) -> Result<()> {
        if !self.config.gear_warning.enabled {
            self.effect_state.gear_warning.warning_active = false;
            return Ok(());
        }

        let airspeed_kt = snapshot.kinematics.ias.value();
        let gear_down = snapshot.config.gear.all_down();
        let altitude_ft = snapshot.environment.altitude; // Simplified - should use AGL

        let state = &mut self.effect_state.gear_warning;

        // Determine if gear warning should be active
        let should_warn = !gear_down
            && airspeed_kt < self.config.gear_warning.speed_threshold_kt
            && altitude_ft < self.config.gear_warning.altitude_threshold_ft;

        if should_warn != state.warning_active {
            state.warning_active = should_warn;

            if should_warn {
                let marker = BlackboxMarker {
                    timestamp: self.time_source.now(),
                    effect_type: "gear_warning".to_string(),
                    event: "activated".to_string(),
                    parameters: [
                        ("airspeed_kt".to_string(), airspeed_kt),
                        ("altitude_ft".to_string(), altitude_ft),
                        ("gear_down".to_string(), if gear_down { 1.0 } else { 0.0 }),
                    ]
                    .into(),
                };
                self.blackbox_markers.push_back(marker);

                // Keep only recent markers (last 1000)
                while self.blackbox_markers.len() > 1000 {
                    self.blackbox_markers.pop_front();
                }

                debug!(
                    "Gear warning activated: speed={:.0}kt, alt={:.0}ft, gear={}",
                    airspeed_kt,
                    altitude_ft,
                    if gear_down { "DOWN" } else { "UP" }
                );
            }
        }

        // Update pulse phase
        if state.warning_active {
            state.pulse_phase +=
                self.config.gear_warning.pulse_frequency_hz * dt * 2.0 * std::f32::consts::PI;
            state.pulse_phase = state.pulse_phase % (2.0 * std::f32::consts::PI);
        }

        state.last_gear_state = gear_down;
        state.last_airspeed = airspeed_kt;
        state.last_altitude = altitude_ft;

        Ok(())
    }

    /// Update helicopter rotor effects
    fn update_rotor_effects(&mut self, helo_data: &HeloData, dt: f32) -> Result<()> {
        if !self.config.rotor_effects.enabled {
            return Ok(());
        }

        let nr_pct = helo_data.nr.value();
        let np_pct = helo_data.np.value();
        let torque_pct = helo_data.torque.value();

        let state = &mut self.effect_state.rotor_effects;

        // Check for low rotor warnings
        let nr_warning = nr_pct < self.config.rotor_effects.nr_low_threshold;
        let np_warning = np_pct < self.config.rotor_effects.np_low_threshold;

        // Update warning states
        if nr_warning != state.nr_warning_active {
            state.nr_warning_active = nr_warning;
            if nr_warning {
                let marker = BlackboxMarker {
                    timestamp: self.time_source.now(),
                    effect_type: "rotor_effects".to_string(),
                    event: "nr_low_warning".to_string(),
                    parameters: [
                        ("nr_pct".to_string(), nr_pct),
                        (
                            "threshold".to_string(),
                            self.config.rotor_effects.nr_low_threshold,
                        ),
                    ]
                    .into(),
                };
                self.blackbox_markers.push_back(marker);

                // Keep only recent markers (last 1000)
                while self.blackbox_markers.len() > 1000 {
                    self.blackbox_markers.pop_front();
                }

                warn!(
                    "Low Nr warning: {:.1}% (threshold: {:.1}%)",
                    nr_pct, self.config.rotor_effects.nr_low_threshold
                );
            }
        }

        if np_warning != state.np_warning_active {
            state.np_warning_active = np_warning;
            if np_warning {
                let marker = BlackboxMarker {
                    timestamp: self.time_source.now(),
                    effect_type: "rotor_effects".to_string(),
                    event: "np_low_warning".to_string(),
                    parameters: [
                        ("np_pct".to_string(), np_pct),
                        (
                            "threshold".to_string(),
                            self.config.rotor_effects.np_low_threshold,
                        ),
                    ]
                    .into(),
                };
                self.blackbox_markers.push_back(marker);

                // Keep only recent markers (last 1000)
                while self.blackbox_markers.len() > 1000 {
                    self.blackbox_markers.pop_front();
                }

                warn!(
                    "Low Np warning: {:.1}% (threshold: {:.1}%)",
                    np_pct, self.config.rotor_effects.np_low_threshold
                );
            }
        }

        // Calculate torque feedback
        state.torque_feedback = torque_pct / 100.0
            * self.config.rotor_effects.torque_scaling
            * self.user_tuning.rotor_sensitivity;

        // Update vibration phase based on rotor speed
        let rotor_freq = self.config.rotor_effects.base_frequency_hz * (nr_pct / 100.0);
        state.vibration_phase += rotor_freq * dt * 2.0 * std::f32::consts::PI;
        state.vibration_phase = state.vibration_phase % (2.0 * std::f32::consts::PI);

        state.last_nr = nr_pct;
        state.last_np = np_pct;
        state.last_torque = torque_pct;

        trace!(
            "Rotor effects: Nr={:.1}%, Np={:.1}%, torque={:.1}%, feedback={:.2}",
            nr_pct, np_pct, torque_pct, state.torque_feedback
        );

        Ok(())
    }

    /// Compute combined effect output
    fn compute_combined_output(&self) -> EffectOutput {
        let mut output = EffectOutput::default();
        let mut active_effects = Vec::new();

        // Stall buffet contribution
        if self.effect_state.stall_buffet.current_intensity > 0.0 {
            let buffet_torque = self.effect_state.stall_buffet.current_intensity
                * self.effect_state.stall_buffet.phase.sin()
                * 2.0;
            output.torque_nm += buffet_torque;
            output.frequency_hz = output
                .frequency_hz
                .max(self.effect_state.stall_buffet.current_frequency);
            output.intensity = output
                .intensity
                .max(self.effect_state.stall_buffet.current_intensity);
            active_effects.push("stall_buffet".to_string());
        }

        // Touchdown impulse contribution
        if self.effect_state.touchdown.impulse_active {
            output.torque_nm += self.effect_state.touchdown.impulse_magnitude;
            output.intensity = output.intensity.max(0.8);
            active_effects.push("touchdown_impulse".to_string());
        }

        // Ground roll contribution
        if self.effect_state.ground_roll.current_intensity > 0.0 {
            let rumble_torque = self.effect_state.ground_roll.current_intensity
                * self.effect_state.ground_roll.phase.sin()
                * 1.0;
            output.torque_nm += rumble_torque;
            output.frequency_hz = output
                .frequency_hz
                .max(self.config.ground_roll.frequency_hz);
            output.intensity = output
                .intensity
                .max(self.effect_state.ground_roll.current_intensity);
            active_effects.push("ground_roll".to_string());
        }

        // Gear warning contribution
        if self.effect_state.gear_warning.warning_active {
            let pulse_torque = self.config.gear_warning.pulse_intensity
                * self.effect_state.gear_warning.pulse_phase.sin()
                * self.user_tuning.gear_warning_sensitivity
                * 1.5;
            output.torque_nm += pulse_torque;
            output.frequency_hz = output
                .frequency_hz
                .max(self.config.gear_warning.pulse_frequency_hz);
            output.intensity = output
                .intensity
                .max(self.config.gear_warning.pulse_intensity);
            active_effects.push("gear_warning".to_string());
        }

        // Rotor effects contribution
        if self.effect_state.rotor_effects.nr_warning_active
            || self.effect_state.rotor_effects.np_warning_active
        {
            let warning_torque = self.config.rotor_effects.warning_intensity
                * self.effect_state.rotor_effects.vibration_phase.sin()
                * 1.2;
            output.torque_nm += warning_torque;
            output.frequency_hz = output
                .frequency_hz
                .max(self.config.rotor_effects.base_frequency_hz);
            output.intensity = output
                .intensity
                .max(self.config.rotor_effects.warning_intensity);
            active_effects.push("rotor_warning".to_string());
        }

        // Add torque feedback
        output.torque_nm += self.effect_state.rotor_effects.torque_feedback;

        // Apply global intensity scaling
        output.torque_nm *= self.user_tuning.global_intensity;
        output.intensity *= self.user_tuning.global_intensity;

        output.active_effects = active_effects;
        output
    }

    /// Get current output without updating
    fn compute_current_output(&self) -> EffectOutput {
        self.compute_combined_output()
    }

    /// Apply rate limiting and smoothing to output
    fn apply_rate_limiting(&mut self, output: EffectOutput) -> EffectOutput {
        let smoothed_torque = self.rate_limiter.apply_smoothing(output.torque_nm);

        EffectOutput {
            torque_nm: smoothed_torque,
            frequency_hz: output.frequency_hz,
            intensity: output.intensity,
            active_effects: output.active_effects,
        }
    }

    /// Get recent blackbox markers
    pub fn get_blackbox_markers(&self) -> &VecDeque<BlackboxMarker> {
        &self.blackbox_markers
    }

    /// Clear blackbox markers
    pub fn clear_blackbox_markers(&mut self) {
        self.blackbox_markers.clear();
    }

    /// Get user tuning interface
    pub fn get_user_tuning(&self) -> &UserTuningInterface {
        &self.user_tuning
    }

    /// Get mutable user tuning interface
    pub fn get_user_tuning_mut(&mut self) -> &mut UserTuningInterface {
        &mut self.user_tuning
    }

    /// Update configuration
    pub fn update_config(&mut self, config: TelemetrySynthConfig) {
        self.config = config;
        self.rate_limiter.min_interval =
            Duration::from_millis(self.config.rate_limiting.min_interval_ms as u64);
        self.rate_limiter.smoothing_factor = self.config.rate_limiting.smoothing_factor;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &TelemetrySynthConfig {
        &self.config
    }

    /// Get current effect state (for diagnostics)
    pub fn get_effect_state(&self) -> &EffectState {
        &self.effect_state
    }
}

impl Default for UserTuningInterface {
    fn default() -> Self {
        Self {
            stall_buffet_intensity: 1.0,
            touchdown_sensitivity: 1.0,
            ground_roll_intensity: 1.0,
            gear_warning_sensitivity: 1.0,
            rotor_sensitivity: 1.0,
            global_intensity: 1.0,
        }
    }
}

impl UserTuningInterface {
    /// Set stall buffet intensity multiplier (0.0 to 2.0)
    pub fn set_stall_buffet_intensity(&mut self, intensity: f32) {
        self.stall_buffet_intensity = intensity.clamp(0.0, 2.0);
    }

    /// Set touchdown sensitivity multiplier (0.0 to 2.0)
    pub fn set_touchdown_sensitivity(&mut self, sensitivity: f32) {
        self.touchdown_sensitivity = sensitivity.clamp(0.0, 2.0);
    }

    /// Set ground roll intensity multiplier (0.0 to 2.0)
    pub fn set_ground_roll_intensity(&mut self, intensity: f32) {
        self.ground_roll_intensity = intensity.clamp(0.0, 2.0);
    }

    /// Set gear warning sensitivity multiplier (0.0 to 2.0)
    pub fn set_gear_warning_sensitivity(&mut self, sensitivity: f32) {
        self.gear_warning_sensitivity = sensitivity.clamp(0.0, 2.0);
    }

    /// Set rotor effects sensitivity multiplier (0.0 to 2.0)
    pub fn set_rotor_sensitivity(&mut self, sensitivity: f32) {
        self.rotor_sensitivity = sensitivity.clamp(0.0, 2.0);
    }

    /// Set global intensity multiplier (0.0 to 2.0)
    pub fn set_global_intensity(&mut self, intensity: f32) {
        self.global_intensity = intensity.clamp(0.0, 2.0);
    }

    /// Get all tuning values as a map
    pub fn get_all_values(&self) -> std::collections::HashMap<String, f32> {
        [
            (
                "stall_buffet_intensity".to_string(),
                self.stall_buffet_intensity,
            ),
            (
                "touchdown_sensitivity".to_string(),
                self.touchdown_sensitivity,
            ),
            (
                "ground_roll_intensity".to_string(),
                self.ground_roll_intensity,
            ),
            (
                "gear_warning_sensitivity".to_string(),
                self.gear_warning_sensitivity,
            ),
            ("rotor_sensitivity".to_string(), self.rotor_sensitivity),
            ("global_intensity".to_string(), self.global_intensity),
        ]
        .into()
    }

    /// Reset all values to defaults
    pub fn reset_to_defaults(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::{AircraftId, BusSnapshot, Percentage, SimId, ValidatedAngle, ValidatedSpeed};

    fn create_test_snapshot() -> BusSnapshot {
        BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
    }

    #[test]
    fn test_telemetry_synth_engine_creation() {
        let config = TelemetrySynthConfig::default();
        let engine = TelemetrySynthEngine::new(config);

        assert_eq!(engine.effect_state.stall_buffet.current_intensity, 0.0);
        assert!(!engine.effect_state.touchdown.impulse_active);
        assert!(!engine.effect_state.gear_warning.warning_active);
    }

    #[test]
    fn test_stall_buffet_activation() {
        let mut config = TelemetrySynthConfig::default();
        config.rate_limiting.min_interval_ms = 0; // Disable rate limiting for test
        let mut engine = TelemetrySynthEngine::new(config);

        let mut snapshot = create_test_snapshot();

        // Set AoA below threshold
        snapshot.kinematics.aoa = ValidatedAngle::new_degrees(10.0).unwrap();
        let result = engine.update(&snapshot).unwrap();
        assert_eq!(result.torque_nm, 0.0);
        assert!(!result.active_effects.contains(&"stall_buffet".to_string()));

        // Wait a bit to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));

        // Set AoA above threshold
        snapshot.kinematics.aoa = ValidatedAngle::new_degrees(15.0).unwrap();
        let result = engine.update(&snapshot).unwrap();
        assert!(result.torque_nm.abs() > 0.0);
        assert!(result.active_effects.contains(&"stall_buffet".to_string()));
    }

    #[test]
    fn test_ground_roll_effects() {
        let mut config = TelemetrySynthConfig::default();
        config.rate_limiting.min_interval_ms = 0; // Disable rate limiting for test
        let mut engine = TelemetrySynthEngine::new(config);

        let mut snapshot = create_test_snapshot();

        // Set conditions for ground roll
        snapshot.kinematics.ground_speed = ValidatedSpeed::new_knots(30.0).unwrap();
        snapshot.kinematics.g_force = flight_bus::GForce::new(1.0).unwrap();

        let result = engine.update(&snapshot).unwrap();
        assert!(result.active_effects.contains(&"ground_roll".to_string()));
        assert!(result.torque_nm.abs() > 0.0);
    }

    #[test]
    fn test_gear_warning() {
        let mut config = TelemetrySynthConfig::default();
        config.rate_limiting.min_interval_ms = 0; // Disable rate limiting for test
        let mut engine = TelemetrySynthEngine::new(config);

        let mut snapshot = create_test_snapshot();

        // Set conditions for gear warning (low speed, gear up, low altitude)
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(100.0).unwrap();
        snapshot.config.gear.nose = flight_bus::types::GearPosition::Up;
        snapshot.config.gear.left = flight_bus::types::GearPosition::Up;
        snapshot.config.gear.right = flight_bus::types::GearPosition::Up;
        snapshot.environment.altitude = 500.0;

        let result = engine.update(&snapshot).unwrap();
        assert!(result.active_effects.contains(&"gear_warning".to_string()));
    }

    #[test]
    fn test_rotor_effects() {
        let mut config = TelemetrySynthConfig::default();
        config.rate_limiting.min_interval_ms = 0; // Disable rate limiting for test
        let mut engine = TelemetrySynthEngine::new(config);

        let mut snapshot = create_test_snapshot();

        // Add helicopter data with low Nr
        snapshot.helo = Some(HeloData {
            nr: Percentage::new(90.0).unwrap(), // Below threshold
            np: Percentage::new(100.0).unwrap(),
            torque: Percentage::new(75.0).unwrap(),
            collective: Percentage::new(50.0).unwrap(),
            pedals: 0.0,
        });

        let result = engine.update(&snapshot).unwrap();
        assert!(result.active_effects.contains(&"rotor_warning".to_string()));
    }

    #[test]
    fn test_user_tuning_interface() {
        let mut tuning = UserTuningInterface::default();

        // Test setting values within range
        tuning.set_stall_buffet_intensity(1.5);
        assert_eq!(tuning.stall_buffet_intensity, 1.5);

        // Test clamping
        tuning.set_global_intensity(3.0);
        assert_eq!(tuning.global_intensity, 2.0);

        tuning.set_touchdown_sensitivity(-1.0);
        assert_eq!(tuning.touchdown_sensitivity, 0.0);

        // Test reset
        tuning.reset_to_defaults();
        assert_eq!(tuning.global_intensity, 1.0);
    }

    #[test]
    fn test_rate_limiting() {
        let mut config = TelemetrySynthConfig::default();
        config.rate_limiting.min_interval_ms = 100; // 10Hz max

        let mut engine = TelemetrySynthEngine::new(config);
        let snapshot = create_test_snapshot();

        // First update should work
        let result1 = engine.update(&snapshot).unwrap();

        // Immediate second update should be rate limited
        let result2 = engine.update(&snapshot).unwrap();

        // Results should be identical due to rate limiting
        assert_eq!(result1.torque_nm, result2.torque_nm);
        assert_eq!(result1.active_effects, result2.active_effects);
    }

    #[test]
    fn test_blackbox_markers() {
        let mut config = TelemetrySynthConfig::default();
        config.rate_limiting.min_interval_ms = 0; // Disable rate limiting for test
        let mut engine = TelemetrySynthEngine::new(config);

        let mut snapshot = create_test_snapshot();

        // First update with low AoA
        snapshot.kinematics.aoa = ValidatedAngle::new_degrees(10.0).unwrap();
        engine.update(&snapshot).unwrap();

        // Wait a bit to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));

        // Second update with high AoA to trigger stall buffet
        snapshot.kinematics.aoa = ValidatedAngle::new_degrees(15.0).unwrap();
        engine.update(&snapshot).unwrap();

        let markers = engine.get_blackbox_markers();
        assert!(!markers.is_empty());

        let stall_marker = markers.iter().find(|m| m.effect_type == "stall_buffet");
        assert!(stall_marker.is_some());
    }
}
