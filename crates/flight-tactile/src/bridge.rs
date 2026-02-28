// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Main tactile bridge implementation with rate-limited thread

use crate::channel::{ChannelMapping, ChannelRouter};
use crate::effects::{EffectProcessor, EffectType};
use crate::simshaker::{SimShakerBridge, SimShakerConfig, SimShakerError};
use flight_bus::BusSnapshot;
use flight_core::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Configuration for the tactile bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TactileConfig {
    /// SimShaker bridge configuration
    pub simshaker: SimShakerConfig,
    /// Channel mapping configuration
    pub channel_mapping: ChannelMapping,
    /// Update rate for the bridge thread (Hz)
    pub update_rate_hz: f32,
    /// Maximum queue size for telemetry data
    pub max_queue_size: usize,
    /// Enable/disable individual effect types
    pub effect_enabled: std::collections::HashMap<EffectType, bool>,
}

impl Default for TactileConfig {
    fn default() -> Self {
        let mut effect_enabled = std::collections::HashMap::new();
        effect_enabled.insert(EffectType::Touchdown, true);
        effect_enabled.insert(EffectType::GroundRoll, true);
        effect_enabled.insert(EffectType::StallBuffet, true);
        effect_enabled.insert(EffectType::EngineVibration, true);
        effect_enabled.insert(EffectType::GearWarning, true);
        effect_enabled.insert(EffectType::RotorVibration, true);

        Self {
            simshaker: SimShakerConfig::default(),
            channel_mapping: ChannelMapping::default(),
            update_rate_hz: 60.0, // 60 Hz update rate
            max_queue_size: 100,
            effect_enabled,
        }
    }
}

/// Statistics for the tactile bridge
#[derive(Debug, Clone)]
pub struct TactileStats {
    /// Number of telemetry snapshots processed
    pub snapshots_processed: u64,
    /// Number of effect events generated
    pub effects_generated: u64,
    /// Number of channel outputs sent
    pub outputs_sent: u64,
    /// Number of dropped telemetry snapshots (queue full)
    pub snapshots_dropped: u64,
    /// Average processing time per snapshot (microseconds)
    pub avg_processing_time_us: f32,
    /// SimShaker bridge statistics
    pub simshaker_stats: Option<crate::simshaker::SimShakerStats>,
    /// Thread running status
    pub thread_running: bool,
    /// Last update timestamp
    pub last_update: Option<Instant>,
    /// Queue utilization (0.0 to 1.0)
    pub queue_utilization: f32,
}

impl Default for TactileStats {
    fn default() -> Self {
        Self {
            snapshots_processed: 0,
            effects_generated: 0,
            outputs_sent: 0,
            snapshots_dropped: 0,
            avg_processing_time_us: 0.0,
            simshaker_stats: None,
            thread_running: false,
            last_update: None,
            queue_utilization: 0.0,
        }
    }
}

/// Errors that can occur in the tactile bridge
#[derive(Debug, Error)]
pub enum TactileBridgeError {
    #[error("SimShaker error: {0}")]
    SimShaker(#[from] SimShakerError),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Thread error: {0}")]
    Thread(String),
    #[error("Channel error: {0}")]
    Channel(String),
}

/// Commands sent to the bridge thread
#[derive(Debug)]
enum BridgeCommand {
    /// Process telemetry snapshot
    ProcessTelemetry(BusSnapshot),
    /// Update configuration
    UpdateConfig(TactileConfig),
    /// Test specific effect
    TestEffect(EffectType, f32),
    /// Stop the bridge
    Stop,
}

/// Main tactile bridge with rate-limited processing thread
pub struct TactileBridge {
    config: Arc<RwLock<TactileConfig>>,
    enabled: Arc<RwLock<bool>>,
    stats: Arc<RwLock<TactileStats>>,
    command_sender: Option<Sender<BridgeCommand>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl TactileBridge {
    /// Create a new tactile bridge
    pub fn new(config: TactileConfig, enabled: Arc<RwLock<bool>>) -> Result<Self> {
        // Validate configuration
        if config.update_rate_hz <= 0.0 || config.update_rate_hz > 1000.0 {
            return Err(flight_core::FlightError::Configuration(
                "Update rate must be between 0 and 1000 Hz".to_string(),
            ));
        }

        if config.max_queue_size == 0 {
            return Err(flight_core::FlightError::Configuration(
                "Max queue size must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            enabled,
            stats: Arc::new(RwLock::new(TactileStats::default())),
            command_sender: None,
            thread_handle: None,
        })
    }

    /// Start the tactile bridge thread
    pub fn start(&mut self) -> Result<()> {
        if self.thread_handle.is_some() {
            return Ok(()); // Already running
        }

        info!("Starting tactile bridge");

        let (command_sender, command_receiver) = mpsc::channel();

        let config = self.config.clone();
        let enabled = self.enabled.clone();
        let stats = self.stats.clone();

        let thread_handle = thread::Builder::new()
            .name("tactile-bridge".to_string())
            .spawn(move || {
                Self::bridge_thread_main(config, enabled, stats, command_receiver);
            })
            .map_err(|e| {
                flight_core::FlightError::Configuration(format!(
                    "Failed to start tactile bridge thread: {}",
                    e
                ))
            })?;

        self.command_sender = Some(command_sender);
        self.thread_handle = Some(thread_handle);

        // Update stats
        self.stats.write().thread_running = true;

        info!("Tactile bridge started successfully");
        Ok(())
    }

    /// Stop the tactile bridge thread
    pub fn stop(&mut self) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            let _ = sender.send(BridgeCommand::Stop);
        }

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        self.command_sender = None;
        self.stats.write().thread_running = false;

        info!("Tactile bridge stopped");
        Ok(())
    }

    /// Process telemetry data
    pub fn process_telemetry(&self, snapshot: &BusSnapshot) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            match sender.send(BridgeCommand::ProcessTelemetry(snapshot.clone())) {
                Ok(()) => Ok(()),
                Err(_) => Err(flight_core::FlightError::Configuration(
                    "Tactile bridge thread disconnected".to_string(),
                )),
            }
        } else {
            Ok(()) // Bridge not started
        }
    }

    /// Update configuration
    pub fn update_config(&self, config: TactileConfig) -> Result<()> {
        *self.config.write() = config.clone();

        if let Some(sender) = &self.command_sender {
            let _ = sender.send(BridgeCommand::UpdateConfig(config));
        }

        Ok(())
    }

    /// Test a specific effect
    pub fn test_effect(&self, effect_type: EffectType, intensity: f32) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            let _ = sender.send(BridgeCommand::TestEffect(effect_type, intensity));
        }
        Ok(())
    }

    /// Get current statistics
    pub fn get_stats(&self) -> TactileStats {
        self.stats.read().clone()
    }

    /// Check if bridge is running
    pub fn is_running(&self) -> bool {
        self.thread_handle.is_some() && self.stats.read().thread_running
    }

    /// Main thread function for the tactile bridge
    fn bridge_thread_main(
        config: Arc<RwLock<TactileConfig>>,
        enabled: Arc<RwLock<bool>>,
        stats: Arc<RwLock<TactileStats>>,
        command_receiver: Receiver<BridgeCommand>,
    ) {
        debug!("Tactile bridge thread started");

        let mut effect_processor = EffectProcessor::new();
        let mut channel_router = ChannelRouter::new(config.read().channel_mapping.clone());
        let mut simshaker_bridge = None;

        // Initialize SimShaker bridge
        match SimShakerBridge::new(config.read().simshaker.clone()) {
            Ok(mut bridge) => {
                if let Err(e) = bridge.start() {
                    error!("Failed to start SimShaker bridge: {}", e);
                } else {
                    simshaker_bridge = Some(bridge);
                    debug!("SimShaker bridge initialized");
                }
            }
            Err(e) => {
                error!("Failed to create SimShaker bridge: {}", e);
            }
        }

        let update_interval = Duration::from_secs_f32(1.0 / config.read().update_rate_hz);
        let mut last_update = Instant::now();
        let mut running = true;

        while running {
            let loop_start = Instant::now();

            // Process commands with timeout
            let timeout = update_interval.saturating_sub(last_update.elapsed());
            match command_receiver.recv_timeout(timeout) {
                Ok(command) => {
                    match command {
                        BridgeCommand::ProcessTelemetry(snapshot) => {
                            if *enabled.read() {
                                Self::process_telemetry_snapshot(
                                    &snapshot,
                                    &config.read(),
                                    &mut effect_processor,
                                    &mut channel_router,
                                    &mut simshaker_bridge,
                                    &stats,
                                );
                            }
                        }
                        BridgeCommand::UpdateConfig(new_config) => {
                            debug!("Updating tactile bridge configuration");
                            channel_router.update_mapping(new_config.channel_mapping.clone());

                            // Update SimShaker bridge if needed
                            if let Some(ref mut bridge) = simshaker_bridge
                                && let Err(e) = bridge.update_config(new_config.simshaker.clone())
                            {
                                warn!("Failed to update SimShaker config: {}", e);
                            }
                        }
                        BridgeCommand::TestEffect(effect_type, intensity) => {
                            debug!(
                                "Testing effect: {:?} at intensity {}",
                                effect_type, intensity
                            );
                            Self::test_effect_internal(
                                effect_type,
                                intensity,
                                &mut channel_router,
                                &mut simshaker_bridge,
                            );
                        }
                        BridgeCommand::Stop => {
                            debug!("Received stop command");
                            running = false;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Normal timeout, continue processing
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    debug!("Command channel disconnected, stopping bridge thread");
                    running = false;
                }
            }

            // Rate-limited updates
            let now = Instant::now();
            if now.duration_since(last_update) >= update_interval {
                // Update SimShaker bridge with current channel state
                if let Some(ref mut bridge) = simshaker_bridge {
                    let outputs = channel_router.process_events(Vec::new()); // Get current state
                    if let Err(e) = bridge.update(&outputs) {
                        warn!("SimShaker bridge update failed: {}", e);
                    }
                }

                // Update statistics
                let mut stats_guard = stats.write();
                stats_guard.last_update = Some(now);
                if let Some(ref bridge) = simshaker_bridge {
                    stats_guard.simshaker_stats = Some(bridge.get_stats());
                }

                last_update = now;
            }

            // Calculate processing time
            let processing_time = loop_start.elapsed();
            let processing_time_us = processing_time.as_micros() as f32;

            // Update average processing time (simple moving average)
            let mut stats_guard = stats.write();
            if stats_guard.avg_processing_time_us == 0.0 {
                stats_guard.avg_processing_time_us = processing_time_us;
            } else {
                stats_guard.avg_processing_time_us =
                    stats_guard.avg_processing_time_us * 0.95 + processing_time_us * 0.05;
            }
        }

        // Cleanup
        if let Some(mut bridge) = simshaker_bridge {
            bridge.stop();
        }

        debug!("Tactile bridge thread stopped");
    }

    /// Process a single telemetry snapshot
    fn process_telemetry_snapshot(
        snapshot: &BusSnapshot,
        config: &TactileConfig,
        effect_processor: &mut EffectProcessor,
        channel_router: &mut ChannelRouter,
        simshaker_bridge: &mut Option<SimShakerBridge>,
        stats: &Arc<RwLock<TactileStats>>,
    ) {
        let process_start = Instant::now();

        // Generate effect events
        let mut events = effect_processor.process(snapshot);

        // Filter events based on configuration
        events.retain(|event| {
            config
                .effect_enabled
                .get(&event.effect_type)
                .copied()
                .unwrap_or(true)
        });

        // Route events to channels
        let outputs = channel_router.process_events(events.clone());

        // Send to SimShaker bridge
        if let Some(bridge) = simshaker_bridge
            && let Err(e) = bridge.update(&outputs)
        {
            warn!("Failed to update SimShaker bridge: {}", e);
        }

        // Update statistics
        let mut stats_guard = stats.write();
        stats_guard.snapshots_processed += 1;
        stats_guard.effects_generated += events.len() as u64;
        stats_guard.outputs_sent += outputs.len() as u64;

        let processing_time = process_start.elapsed();
        let processing_time_us = processing_time.as_micros() as f32;

        // Update average processing time
        if stats_guard.avg_processing_time_us == 0.0 {
            stats_guard.avg_processing_time_us = processing_time_us;
        } else {
            stats_guard.avg_processing_time_us =
                stats_guard.avg_processing_time_us * 0.9 + processing_time_us * 0.1;
        }
    }

    /// Test a specific effect internally
    fn test_effect_internal(
        effect_type: EffectType,
        intensity: f32,
        channel_router: &mut ChannelRouter,
        simshaker_bridge: &mut Option<SimShakerBridge>,
    ) {
        if let Ok(outputs) = channel_router.test_effect(effect_type, intensity)
            && let Some(bridge) = simshaker_bridge
            && let Err(e) = bridge.update(&outputs)
        {
            warn!("Failed to send test effect to SimShaker: {}", e);
        }
    }
}

impl Drop for TactileBridge {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::RwLock;
    use std::sync::Arc;

    #[test]
    fn test_tactile_bridge_creation() {
        let config = TactileConfig::default();
        let enabled = Arc::new(RwLock::new(true));

        let bridge = TactileBridge::new(config, enabled);
        assert!(bridge.is_ok());
    }

    #[test]
    fn test_tactile_bridge_invalid_config() {
        let config = TactileConfig {
            update_rate_hz: 0.0, // Invalid
            ..Default::default()
        };

        let enabled = Arc::new(RwLock::new(true));
        let bridge = TactileBridge::new(config, enabled);
        assert!(bridge.is_err());
    }

    #[test]
    fn test_tactile_config_defaults() {
        let config = TactileConfig::default();

        assert_eq!(config.update_rate_hz, 60.0);
        assert_eq!(config.max_queue_size, 100);
        assert!(
            config
                .effect_enabled
                .get(&EffectType::Touchdown)
                .copied()
                .unwrap_or(false)
        );
        assert!(
            config
                .effect_enabled
                .get(&EffectType::StallBuffet)
                .copied()
                .unwrap_or(false)
        );
    }

    #[test]
    fn test_tactile_stats_defaults() {
        let stats = TactileStats::default();

        assert_eq!(stats.snapshots_processed, 0);
        assert_eq!(stats.effects_generated, 0);
        assert_eq!(stats.outputs_sent, 0);
        assert!(!stats.thread_running);
    }

    #[test]
    fn test_bridge_lifecycle() {
        let config = TactileConfig::default();
        let enabled = Arc::new(RwLock::new(true));

        let bridge = TactileBridge::new(config, enabled).unwrap();

        // Should not be running initially
        assert!(!bridge.is_running());

        // Note: We can't easily test start/stop without mocking the SimShaker bridge
        // as it requires network operations. In a real test environment, we would
        // use dependency injection or mocking frameworks.
    }
}
