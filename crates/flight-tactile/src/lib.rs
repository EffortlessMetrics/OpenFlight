// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Tactile Bridge - Channel routing for tactile feedback effects
//!
//! Provides a rate-limited bridge to SimShaker-class applications for touchdown,
//! rumble, and stall effects. Operates independently from the real-time axis/FFB
//! loops to prevent jitter regression.

pub mod bridge;
pub mod channel;
pub mod effects;
pub mod simshaker;

use flight_core::Result;
use std::sync::Arc;
use parking_lot::RwLock;

// Re-export main types for convenience
pub use bridge::{TactileBridge, TactileConfig, TactileStats};
pub use channel::{ChannelRouter, ChannelMapping, ChannelId};
pub use effects::{EffectType, EffectIntensity, EffectEvent, EffectProcessor};
pub use simshaker::{SimShakerBridge, SimShakerConfig, SimShakerStatus};

/// Main tactile manager for coordinating all tactile feedback
pub struct TactileManager {
    bridge: Option<TactileBridge>,
    config: Arc<RwLock<TactileConfig>>,
    enabled: Arc<RwLock<bool>>,
}

impl TactileManager {
    /// Create a new tactile manager
    pub fn new() -> Self {
        Self {
            bridge: None,
            config: Arc::new(RwLock::new(TactileConfig::default())),
            enabled: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize the tactile bridge with configuration
    pub fn initialize(&mut self, config: TactileConfig) -> Result<()> {
        *self.config.write() = config.clone();
        
        let bridge = TactileBridge::new(config, self.enabled.clone())?;
        self.bridge = Some(bridge);
        
        Ok(())
    }

    /// Start the tactile bridge
    pub fn start(&mut self) -> Result<()> {
        if let Some(bridge) = &mut self.bridge {
            bridge.start()?;
        }
        Ok(())
    }

    /// Stop the tactile bridge
    pub fn stop(&mut self) -> Result<()> {
        if let Some(bridge) = &mut self.bridge {
            bridge.stop()?;
        }
        Ok(())
    }

    /// Enable or disable tactile feedback
    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.write() = enabled;
    }

    /// Check if tactile feedback is enabled
    pub fn is_enabled(&self) -> bool {
        *self.enabled.read()
    }

    /// Update configuration
    pub fn update_config(&self, config: TactileConfig) -> Result<()> {
        *self.config.write() = config.clone();
        
        if let Some(bridge) = &self.bridge {
            bridge.update_config(config)?;
        }
        
        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> TactileConfig {
        self.config.read().clone()
    }

    /// Process telemetry data for tactile effects
    pub fn process_telemetry(&self, snapshot: &flight_bus::BusSnapshot) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        if let Some(bridge) = &self.bridge {
            bridge.process_telemetry(snapshot)?;
        }
        
        Ok(())
    }

    /// Get tactile bridge statistics
    pub fn get_stats(&self) -> Option<TactileStats> {
        self.bridge.as_ref().map(|bridge| bridge.get_stats())
    }

    /// Test tactile output with a specific effect
    pub fn test_effect(&self, effect_type: EffectType, intensity: f32) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            bridge.test_effect(effect_type, intensity)?;
        }
        Ok(())
    }
}

impl Default for TactileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TactileManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}