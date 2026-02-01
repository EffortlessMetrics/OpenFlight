// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Channel routing for tactile effects

use crate::effects::{EffectEvent, EffectIntensity, EffectType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Unique identifier for a tactile channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub u8);

impl ChannelId {
    /// Create a new channel ID
    pub fn new(id: u8) -> Self {
        Self(id)
    }

    /// Get the channel ID value
    pub fn value(&self) -> u8 {
        self.0
    }
}

/// Mapping configuration for routing effects to channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMapping {
    /// Effect type to channel mappings
    pub mappings: HashMap<EffectType, ChannelId>,
    /// Channel intensity multipliers (0.0 to 1.0)
    pub channel_gains: HashMap<ChannelId, f32>,
    /// Channel enable/disable state
    pub channel_enabled: HashMap<ChannelId, bool>,
}

impl ChannelMapping {
    /// Create a new channel mapping with defaults
    pub fn new() -> Self {
        let mut mappings = HashMap::new();
        let mut channel_gains = HashMap::new();
        let mut channel_enabled = HashMap::new();

        // Default channel assignments
        mappings.insert(EffectType::Touchdown, ChannelId::new(0));
        mappings.insert(EffectType::GroundRoll, ChannelId::new(1));
        mappings.insert(EffectType::StallBuffet, ChannelId::new(2));
        mappings.insert(EffectType::EngineVibration, ChannelId::new(3));
        mappings.insert(EffectType::GearWarning, ChannelId::new(4));
        mappings.insert(EffectType::RotorVibration, ChannelId::new(5));

        // Default gains (all at 100%)
        for i in 0..8 {
            let channel_id = ChannelId::new(i);
            channel_gains.insert(channel_id, 1.0);
            channel_enabled.insert(channel_id, true);
        }

        Self {
            mappings,
            channel_gains,
            channel_enabled,
        }
    }

    /// Set effect to channel mapping
    pub fn set_mapping(&mut self, effect_type: EffectType, channel_id: ChannelId) {
        self.mappings.insert(effect_type, channel_id);
    }

    /// Get channel for effect type
    pub fn get_channel(&self, effect_type: EffectType) -> Option<ChannelId> {
        self.mappings.get(&effect_type).copied()
    }

    /// Set channel gain (0.0 to 1.0)
    pub fn set_channel_gain(&mut self, channel_id: ChannelId, gain: f32) -> Result<(), String> {
        if (0.0..=1.0).contains(&gain) {
            self.channel_gains.insert(channel_id, gain);
            Ok(())
        } else {
            Err(format!(
                "Channel gain must be between 0.0 and 1.0, got {}",
                gain
            ))
        }
    }

    /// Get channel gain
    pub fn get_channel_gain(&self, channel_id: ChannelId) -> f32 {
        self.channel_gains.get(&channel_id).copied().unwrap_or(1.0)
    }

    /// Enable or disable a channel
    pub fn set_channel_enabled(&mut self, channel_id: ChannelId, enabled: bool) {
        self.channel_enabled.insert(channel_id, enabled);
    }

    /// Check if channel is enabled
    pub fn is_channel_enabled(&self, channel_id: ChannelId) -> bool {
        self.channel_enabled
            .get(&channel_id)
            .copied()
            .unwrap_or(true)
    }

    /// Get all configured channels
    pub fn get_all_channels(&self) -> Vec<ChannelId> {
        let mut channels: Vec<_> = self.channel_gains.keys().copied().collect();
        channels.sort_by_key(|c| c.0);
        channels
    }
}

impl Default for ChannelMapping {
    fn default() -> Self {
        Self::new()
    }
}

/// Output data for a single channel
#[derive(Debug, Clone)]
pub struct ChannelOutput {
    pub channel_id: ChannelId,
    pub intensity: EffectIntensity,
    pub timestamp: Instant,
}

impl ChannelOutput {
    /// Create a new channel output
    pub fn new(channel_id: ChannelId, intensity: EffectIntensity) -> Self {
        Self {
            channel_id,
            intensity,
            timestamp: Instant::now(),
        }
    }

    /// Create zero intensity output for channel
    pub fn zero(channel_id: ChannelId) -> Self {
        Self::new(channel_id, EffectIntensity::zero())
    }
}

/// Routes effect events to appropriate channels with gain and enable control
pub struct ChannelRouter {
    mapping: ChannelMapping,
    active_effects: HashMap<EffectType, EffectEvent>,
    last_update: Instant,
}

impl ChannelRouter {
    /// Create a new channel router
    pub fn new(mapping: ChannelMapping) -> Self {
        Self {
            mapping,
            active_effects: HashMap::new(),
            last_update: Instant::now(),
        }
    }

    /// Update the channel mapping
    pub fn update_mapping(&mut self, mapping: ChannelMapping) {
        self.mapping = mapping;
    }

    /// Get current channel mapping
    pub fn get_mapping(&self) -> &ChannelMapping {
        &self.mapping
    }

    /// Process effect events and generate channel outputs
    pub fn process_events(&mut self, events: Vec<EffectEvent>) -> Vec<ChannelOutput> {
        let now = Instant::now();
        self.last_update = now;

        // Update active effects with new events
        for event in events {
            self.active_effects.insert(event.effect_type, event);
        }

        // Remove expired effects
        self.active_effects.retain(|_, event| !event.is_expired());

        // Generate channel outputs
        let mut outputs = Vec::new();
        let mut channel_intensities: HashMap<ChannelId, f32> = HashMap::new();

        // Accumulate intensities for each channel
        for (effect_type, event) in &self.active_effects {
            if let Some(channel_id) = self.mapping.get_channel(*effect_type)
                && self.mapping.is_channel_enabled(channel_id)
            {
                let gain = self.mapping.get_channel_gain(channel_id);
                let intensity = event.intensity.value() * gain;

                // Accumulate intensities (max of all effects on this channel)
                let current = channel_intensities.get(&channel_id).copied().unwrap_or(0.0);
                channel_intensities.insert(channel_id, current.max(intensity));
            }
        }

        // Create outputs for all configured channels
        for channel_id in self.mapping.get_all_channels() {
            let intensity = channel_intensities.get(&channel_id).copied().unwrap_or(0.0);
            let intensity =
                EffectIntensity::new(intensity.min(1.0)).unwrap_or(EffectIntensity::zero());
            outputs.push(ChannelOutput::new(channel_id, intensity));
        }

        outputs
    }

    /// Get currently active effects
    pub fn get_active_effects(&self) -> &HashMap<EffectType, EffectEvent> {
        &self.active_effects
    }

    /// Clear all active effects
    pub fn clear_active_effects(&mut self) {
        self.active_effects.clear();
    }

    /// Test a specific effect on its mapped channel
    pub fn test_effect(
        &mut self,
        effect_type: EffectType,
        intensity: f32,
    ) -> Result<Vec<ChannelOutput>, String> {
        let intensity = EffectIntensity::new(intensity)?;
        let event = EffectEvent::new(effect_type, intensity);

        // Temporarily add the test effect
        let old_effect = self.active_effects.insert(effect_type, event);

        // Generate outputs
        let outputs = self.process_events(Vec::new());

        // Restore previous state
        if let Some(old) = old_effect {
            self.active_effects.insert(effect_type, old);
        } else {
            self.active_effects.remove(&effect_type);
        }

        Ok(outputs)
    }

    /// Get time since last update
    pub fn time_since_last_update(&self) -> std::time::Duration {
        self.last_update.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectEvent;
    use std::time::Duration;

    #[test]
    fn test_channel_mapping_creation() {
        let mapping = ChannelMapping::new();

        // Check default mappings exist
        assert!(mapping.get_channel(EffectType::Touchdown).is_some());
        assert!(mapping.get_channel(EffectType::StallBuffet).is_some());

        // Check default gains
        let channel_id = ChannelId::new(0);
        assert_eq!(mapping.get_channel_gain(channel_id), 1.0);
        assert!(mapping.is_channel_enabled(channel_id));
    }

    #[test]
    fn test_channel_mapping_modification() {
        let mut mapping = ChannelMapping::new();
        let channel_id = ChannelId::new(7);

        // Set custom mapping
        mapping.set_mapping(EffectType::Touchdown, channel_id);
        assert_eq!(mapping.get_channel(EffectType::Touchdown), Some(channel_id));

        // Set custom gain
        assert!(mapping.set_channel_gain(channel_id, 0.5).is_ok());
        assert_eq!(mapping.get_channel_gain(channel_id), 0.5);

        // Invalid gain should fail
        assert!(mapping.set_channel_gain(channel_id, 1.5).is_err());

        // Disable channel
        mapping.set_channel_enabled(channel_id, false);
        assert!(!mapping.is_channel_enabled(channel_id));
    }

    #[test]
    fn test_channel_router_processing() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);

        // Create test events
        let touchdown_event =
            EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(0.8).unwrap());
        let stall_event =
            EffectEvent::new(EffectType::StallBuffet, EffectIntensity::new(0.6).unwrap());

        let events = vec![touchdown_event, stall_event];
        let outputs = router.process_events(events);

        // Should have outputs for all configured channels
        assert!(!outputs.is_empty());

        // Find touchdown channel output
        let touchdown_channel = router.mapping.get_channel(EffectType::Touchdown).unwrap();
        let touchdown_output = outputs
            .iter()
            .find(|o| o.channel_id == touchdown_channel)
            .unwrap();

        assert!(touchdown_output.intensity.value() > 0.0);
    }

    #[test]
    fn test_channel_router_gain_application() {
        let mut mapping = ChannelMapping::new();
        let channel_id = ChannelId::new(0);

        // Set 50% gain
        mapping.set_channel_gain(channel_id, 0.5).unwrap();
        mapping.set_mapping(EffectType::Touchdown, channel_id);

        let mut router = ChannelRouter::new(mapping);

        // Create full intensity event
        let event = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());

        let outputs = router.process_events(vec![event]);
        let output = outputs.iter().find(|o| o.channel_id == channel_id).unwrap();

        // Should be reduced by gain
        assert!((output.intensity.value() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_channel_router_disabled_channel() {
        let mut mapping = ChannelMapping::new();
        let channel_id = ChannelId::new(0);

        // Disable channel
        mapping.set_channel_enabled(channel_id, false);
        mapping.set_mapping(EffectType::Touchdown, channel_id);

        let mut router = ChannelRouter::new(mapping);

        // Create event
        let event = EffectEvent::new(EffectType::Touchdown, EffectIntensity::new(1.0).unwrap());

        let outputs = router.process_events(vec![event]);
        let output = outputs.iter().find(|o| o.channel_id == channel_id).unwrap();

        // Should be zero due to disabled channel
        assert_eq!(output.intensity.value(), 0.0);
    }

    #[test]
    fn test_effect_expiration() {
        let mapping = ChannelMapping::new();
        let mut router = ChannelRouter::new(mapping);

        // Create short-duration event
        let event = EffectEvent::with_duration(
            EffectType::Touchdown,
            EffectIntensity::new(1.0).unwrap(),
            Duration::from_millis(50),
        );

        router.process_events(vec![event]);
        assert_eq!(router.active_effects.len(), 1);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(100));

        // Process again to trigger cleanup
        router.process_events(Vec::new());
        assert_eq!(router.active_effects.len(), 0);
    }
}
