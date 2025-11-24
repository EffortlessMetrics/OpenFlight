// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Offline engine implementations for replay

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

use flight_axis::{AxisEngine, AxisFrame, EngineConfig as AxisEngineConfig};
use flight_bus::BusSnapshot;
use flight_ffb::{FfbConfig, FfbEngine, FfbMode, SafetyState};

use crate::replay_config::ReplayConfig;

/// State of an offline engine during replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    /// Current timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Number of frames processed
    pub frames_processed: u64,
    /// Current axis outputs (device_id -> output)
    pub axis_outputs: HashMap<String, f32>,
    /// Current FFB outputs (device_id -> torque_nm)
    pub ffb_outputs: HashMap<String, f32>,
    /// Engine health status
    pub is_healthy: bool,
    /// Last error message (if any)
    pub last_error: Option<String>,
}

/// Offline axis engine for replay
pub struct OfflineAxisEngine {
    engine: AxisEngine,
    config: ReplayConfig,
    state: EngineState,
    device_configs: HashMap<String, AxisEngineConfig>,
    last_frames: HashMap<String, AxisFrame>,
}

impl OfflineAxisEngine {
    /// Create a new offline axis engine
    pub fn new(config: ReplayConfig) -> Result<Self> {
        let engine = AxisEngine::new();

        let state = EngineState {
            timestamp_ns: 0,
            frames_processed: 0,
            axis_outputs: HashMap::new(),
            ffb_outputs: HashMap::new(),
            is_healthy: true,
            last_error: None,
        };

        Ok(Self {
            engine,
            config,
            state,
            device_configs: HashMap::new(),
            last_frames: HashMap::new(),
        })
    }

    /// Add a device configuration for replay
    pub fn add_device(&mut self, device_id: String, config: AxisEngineConfig) -> Result<()> {
        self.device_configs.insert(device_id.clone(), config);
        self.state.axis_outputs.insert(device_id.clone(), 0.0);
        debug!("Added device {} to offline axis engine", device_id);
        Ok(())
    }

    /// Process an axis frame through the offline engine
    pub fn process_frame(&mut self, device_id: &str, mut frame: AxisFrame) -> Result<f32> {
        // Update derivative if we have a previous frame
        if let Some(prev_frame) = self.last_frames.get(device_id) {
            frame.update_derivative(prev_frame);
        }

        // Process through axis engine (simplified for offline replay)
        let output = match self.engine.process(&mut frame) {
            Ok(()) => {
                self.state.is_healthy = true;
                self.state.last_error = None;
                frame.out
            }
            Err(e) => {
                self.state.is_healthy = false;
                self.state.last_error = Some(e.to_string());
                tracing::warn!("Axis engine error for device {}: {}", device_id, e);
                frame.out // Fallback to pass-through
            }
        };

        // Update state
        self.state.timestamp_ns = frame.ts_mono_ns;
        self.state.frames_processed += 1;
        self.state
            .axis_outputs
            .insert(device_id.to_string(), output);
        self.last_frames.insert(device_id.to_string(), frame);

        trace!(
            "Processed axis frame for {}: {} -> {}",
            device_id, frame.in_raw, output
        );
        Ok(output)
    }

    /// Get current engine state
    pub fn get_state(&self) -> &EngineState {
        &self.state
    }

    /// Reset engine state for new replay
    pub fn reset(&mut self) {
        self.state = EngineState {
            timestamp_ns: 0,
            frames_processed: 0,
            axis_outputs: HashMap::new(),
            ffb_outputs: HashMap::new(),
            is_healthy: true,
            last_error: None,
        };
        self.last_frames.clear();

        // Re-initialize axis outputs for configured devices
        for device_id in self.device_configs.keys() {
            self.state.axis_outputs.insert(device_id.clone(), 0.0);
        }
    }

    /// Get axis engine for advanced operations
    pub fn get_engine(&self) -> &AxisEngine {
        &self.engine
    }

    /// Get mutable axis engine for configuration
    pub fn get_engine_mut(&mut self) -> &mut AxisEngine {
        &mut self.engine
    }
}

/// Offline FFB engine for replay
pub struct OfflineFfbEngine {
    engine: FfbEngine,
    config: ReplayConfig,
    state: EngineState,
    device_configs: HashMap<String, FfbConfig>,
    last_update: Instant,
}

impl OfflineFfbEngine {
    /// Create a new offline FFB engine
    pub fn new(config: ReplayConfig) -> Result<Self> {
        let ffb_config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: false,     // Disabled for offline replay
            mode: FfbMode::TelemetrySynth, // Use telemetry synthesis for replay
            device_path: None,
        };

        let engine = FfbEngine::new(ffb_config).context("Failed to create FFB engine")?;

        let state = EngineState {
            timestamp_ns: 0,
            frames_processed: 0,
            axis_outputs: HashMap::new(),
            ffb_outputs: HashMap::new(),
            is_healthy: true,
            last_error: None,
        };

        Ok(Self {
            engine,
            config,
            state,
            device_configs: HashMap::new(),
            last_update: Instant::now(),
        })
    }

    /// Add a device configuration for replay
    pub fn add_device(&mut self, device_id: String, config: FfbConfig) -> Result<()> {
        self.device_configs.insert(device_id.clone(), config);
        self.state.ffb_outputs.insert(device_id.clone(), 0.0);
        debug!("Added device {} to offline FFB engine", device_id);
        Ok(())
    }

    /// Process telemetry data and generate FFB output
    pub fn process_telemetry(
        &mut self,
        device_id: &str,
        snapshot: &BusSnapshot,
        timestamp_ns: u64,
    ) -> Result<f32> {
        // Update engine with telemetry data
        let torque_output =
            if let Some(effect_output) = self.engine.update_telemetry_synthesis(snapshot)? {
                effect_output.torque_nm
            } else {
                0.0 // No telemetry synthesis enabled
            };

        // Update engine state
        if let Err(e) = self.engine.update() {
            self.state.is_healthy = false;
            self.state.last_error = Some(e.to_string());
            tracing::warn!("FFB engine update error: {}", e);
        } else {
            self.state.is_healthy = true;
            self.state.last_error = None;
        }

        // Update state
        self.state.timestamp_ns = timestamp_ns;
        self.state.frames_processed += 1;
        self.state
            .ffb_outputs
            .insert(device_id.to_string(), torque_output);
        self.last_update = Instant::now();

        trace!(
            "Processed FFB telemetry for {}: {} Nm",
            device_id, torque_output
        );
        Ok(torque_output)
    }

    /// Process axis frame for FFB (for axis-driven effects)
    pub fn process_axis_frame(
        &mut self,
        device_id: &str,
        frame: AxisFrame,
        axis_output: f32,
    ) -> Result<f32> {
        // Record axis frame in FFB engine
        self.engine.record_axis_frame(
            device_id.to_string(),
            frame.in_raw,
            axis_output,
            0.0, // Torque will be calculated by telemetry synthesis
        )?;

        // Update trim controller
        let trim_output = self.engine.update_trim_controller();
        let torque_output = match trim_output {
            flight_ffb::TrimOutput::ForceFeedback { setpoint_nm, .. } => setpoint_nm,
            flight_ffb::TrimOutput::SpringCentered { .. } => 0.0, // No torque for spring-centered
        };

        // Update state
        self.state.timestamp_ns = frame.ts_mono_ns;
        self.state.frames_processed += 1;
        self.state
            .ffb_outputs
            .insert(device_id.to_string(), torque_output);

        trace!(
            "Processed FFB axis frame for {}: {} Nm",
            device_id, torque_output
        );
        Ok(torque_output)
    }

    /// Get current engine state
    pub fn get_state(&self) -> &EngineState {
        &self.state
    }

    /// Reset engine state for new replay
    pub fn reset(&mut self) {
        self.state = EngineState {
            timestamp_ns: 0,
            frames_processed: 0,
            axis_outputs: HashMap::new(),
            ffb_outputs: HashMap::new(),
            is_healthy: true,
            last_error: None,
        };

        // Re-initialize FFB outputs for configured devices
        for device_id in self.device_configs.keys() {
            self.state.ffb_outputs.insert(device_id.clone(), 0.0);
        }

        self.last_update = Instant::now();
    }

    /// Get FFB engine for advanced operations
    pub fn get_engine(&self) -> &FfbEngine {
        &self.engine
    }

    /// Get mutable FFB engine for configuration
    pub fn get_engine_mut(&mut self) -> &mut FfbEngine {
        &mut self.engine
    }

    /// Get current safety state
    pub fn get_safety_state(&self) -> SafetyState {
        self.engine.safety_state()
    }

    /// Check if engine is healthy
    pub fn is_healthy(&self) -> bool {
        self.state.is_healthy && self.engine.is_healthy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_axis::AxisFrame;
    use flight_bus::{AircraftConfig, BusSnapshot, Kinematics};

    #[test]
    fn test_offline_axis_engine_creation() {
        let config = ReplayConfig::default();
        let engine = OfflineAxisEngine::new(config).unwrap();

        assert_eq!(engine.get_state().frames_processed, 0);
        assert!(engine.get_state().is_healthy);
    }

    #[test]
    fn test_offline_axis_engine_device_management() {
        let config = ReplayConfig::default();
        let mut engine = OfflineAxisEngine::new(config).unwrap();

        let device_config = AxisEngineConfig::default();
        engine
            .add_device("test_device".to_string(), device_config)
            .unwrap();

        assert!(engine.get_state().axis_outputs.contains_key("test_device"));
    }

    #[test]
    fn test_offline_axis_engine_frame_processing() {
        let config = ReplayConfig::default();
        let mut engine = OfflineAxisEngine::new(config).unwrap();

        let device_config = AxisEngineConfig::default();
        engine
            .add_device("test_device".to_string(), device_config)
            .unwrap();

        let frame = AxisFrame::new(0.5, 1000000);
        let output = engine.process_frame("test_device", frame).unwrap();

        assert_eq!(engine.get_state().frames_processed, 1);
        assert_eq!(engine.get_state().axis_outputs["test_device"], output);
    }

    #[test]
    fn test_offline_ffb_engine_creation() {
        let config = ReplayConfig::default();
        let engine = OfflineFfbEngine::new(config).unwrap();

        assert_eq!(engine.get_state().frames_processed, 0);
        assert!(engine.get_state().is_healthy);
        assert_eq!(engine.get_safety_state(), SafetyState::SafeTorque);
    }

    #[test]
    fn test_offline_ffb_engine_telemetry_processing() {
        let config = ReplayConfig::default();
        let mut engine = OfflineFfbEngine::new(config).unwrap();

        let ffb_config = FfbConfig {
            max_torque_nm: 10.0,
            fault_timeout_ms: 50,
            interlock_required: false,
            mode: FfbMode::TelemetrySynth,
            device_path: None,
        };
        engine
            .add_device("test_device".to_string(), ffb_config)
            .unwrap();

        let snapshot = BusSnapshot {
            sim: flight_bus::SimId::Unknown,
            aircraft: flight_bus::AircraftId::new("test_aircraft"),
            timestamp: 1000000,
            kinematics: flight_bus::Kinematics {
                ias: flight_bus::ValidatedSpeed::new_knots(120.0).unwrap(),
                tas: flight_bus::ValidatedSpeed::new_knots(125.0).unwrap(),
                ground_speed: flight_bus::ValidatedSpeed::new_knots(120.0).unwrap(),
                aoa: flight_bus::ValidatedAngle::new_degrees(5.0).unwrap(),
                sideslip: flight_bus::ValidatedAngle::new_degrees(0.0).unwrap(),
                bank: flight_bus::ValidatedAngle::new_degrees(0.0).unwrap(),
                pitch: flight_bus::ValidatedAngle::new_degrees(5.0).unwrap(),
                heading: flight_bus::ValidatedAngle::new_degrees(0.0).unwrap(),
                g_force: flight_bus::GForce::new(1.0).unwrap(),
                g_lateral: flight_bus::GForce::new(0.0).unwrap(),
                g_longitudinal: flight_bus::GForce::new(0.0).unwrap(),
                mach: flight_bus::Mach::new(0.18).unwrap(),
                vertical_speed: 0.0,
            },
            angular_rates: flight_bus::snapshot::AngularRates {
                p: 0.0,
                q: 0.0,
                r: 0.0,
            },
            config: flight_bus::AircraftConfig {
                gear: flight_bus::GearState {
                    nose: flight_bus::GearPosition::Up,
                    left: flight_bus::GearPosition::Up,
                    right: flight_bus::GearPosition::Up,
                },
                flaps: flight_bus::Percentage::new(0.0).unwrap(),
                spoilers: flight_bus::Percentage::new(0.0).unwrap(),
                ap_state: flight_bus::AutopilotState::Off,
                ap_altitude: None,
                ap_heading: None,
                ap_speed: None,
                lights: flight_bus::LightsConfig::default(),
                fuel: std::collections::HashMap::new(),
            },
            control_inputs: flight_bus::snapshot::ControlInputs {
                pitch: 0.0,
                roll: 0.0,
                yaw: 0.0,
                throttle: vec![0.75],
            },
            trim_state: flight_bus::snapshot::TrimState {
                elevator: 0.0,
                aileron: 0.0,
                rudder: 0.0,
            },
            helo: None,
            engines: Vec::new(),
            environment: flight_bus::Environment::default(),
            navigation: flight_bus::Navigation::default(),
            validity: flight_bus::snapshot::ValidityFlags {
                safe_for_ffb: true,
                attitude_valid: true,
                angular_rates_valid: true,
                velocities_valid: true,
                kinematics_valid: true,
                aero_valid: true,
                position_valid: true,
            },
        };

        let output = engine
            .process_telemetry("test_device", &snapshot, 1000000)
            .unwrap();

        assert_eq!(engine.get_state().frames_processed, 1);
        assert!(engine.get_state().ffb_outputs.contains_key("test_device"));
    }

    #[test]
    fn test_engine_reset() {
        let config = ReplayConfig::default();
        let mut engine = OfflineAxisEngine::new(config).unwrap();

        let device_config = AxisEngineConfig::default();
        engine
            .add_device("test_device".to_string(), device_config)
            .unwrap();

        let frame = AxisFrame::new(0.5, 1000000);
        engine.process_frame("test_device", frame).unwrap();

        assert_eq!(engine.get_state().frames_processed, 1);

        engine.reset();

        assert_eq!(engine.get_state().frames_processed, 0);
        assert_eq!(engine.get_state().timestamp_ns, 0);
        assert!(engine.get_state().axis_outputs.contains_key("test_device"));
        assert_eq!(engine.get_state().axis_outputs["test_device"], 0.0);
    }
}
