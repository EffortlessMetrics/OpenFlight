// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Update channels (stable/beta/canary) management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Update channel types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Stable releases - thoroughly tested, recommended for production
    #[serde(alias = "Stable")]
    Stable,
    /// Beta releases - feature complete, undergoing final testing
    #[serde(alias = "Beta")]
    Beta,
    /// Canary releases - latest features, may be unstable
    #[serde(alias = "Canary")]
    Canary,
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Channel::Stable => write!(f, "stable"),
            Channel::Beta => write!(f, "beta"),
            Channel::Canary => write!(f, "canary"),
        }
    }
}

impl std::str::FromStr for Channel {
    type Err = crate::UpdateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "canary" => Ok(Channel::Canary),
            _ => Err(crate::UpdateError::ChannelNotFound(s.to_string())),
        }
    }
}

/// Channel configuration and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel type
    pub channel: Channel,
    /// Update check frequency in hours
    pub check_frequency_hours: u64,
    /// Automatic update installation
    pub auto_install: bool,
    /// Pre-release acceptance
    pub accept_prerelease: bool,
    /// Update server URL
    pub update_url: String,
    /// Public key for signature verification
    pub public_key: String,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            channel: Channel::Stable,
            check_frequency_hours: 24,
            auto_install: false,
            accept_prerelease: false,
            update_url: "https://updates.flight-hub.dev".to_string(),
            public_key: String::new(), // Will be set during build
        }
    }
}

/// Channel manager for handling multiple update channels
#[derive(Debug)]
pub struct ChannelManager {
    configs: HashMap<Channel, ChannelConfig>,
    current_channel: Channel,
}

impl ChannelManager {
    /// Create a new channel manager with default configurations
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        // Stable channel
        configs.insert(
            Channel::Stable,
            ChannelConfig {
                channel: Channel::Stable,
                check_frequency_hours: 24,
                auto_install: false,
                accept_prerelease: false,
                update_url: "https://updates.flight-hub.dev/stable".to_string(),
                public_key: include_str!("../keys/stable.pub").to_string(),
            },
        );

        // Beta channel
        configs.insert(
            Channel::Beta,
            ChannelConfig {
                channel: Channel::Beta,
                check_frequency_hours: 12,
                auto_install: false,
                accept_prerelease: true,
                update_url: "https://updates.flight-hub.dev/beta".to_string(),
                public_key: include_str!("../keys/beta.pub").to_string(),
            },
        );

        // Canary channel
        configs.insert(
            Channel::Canary,
            ChannelConfig {
                channel: Channel::Canary,
                check_frequency_hours: 6,
                auto_install: false,
                accept_prerelease: true,
                update_url: "https://updates.flight-hub.dev/canary".to_string(),
                public_key: include_str!("../keys/canary.pub").to_string(),
            },
        );

        Self {
            configs,
            current_channel: Channel::Stable,
        }
    }

    /// Get configuration for a specific channel
    pub fn get_config(&self, channel: Channel) -> Option<&ChannelConfig> {
        self.configs.get(&channel)
    }

    /// Get current active channel
    pub fn current_channel(&self) -> Channel {
        self.current_channel
    }

    /// Switch to a different channel
    pub fn switch_channel(&mut self, channel: Channel) -> crate::Result<()> {
        if !self.configs.contains_key(&channel) {
            return Err(crate::UpdateError::ChannelNotFound(channel.to_string()));
        }

        tracing::info!(
            "Switching from {} to {} channel",
            self.current_channel,
            channel
        );
        self.current_channel = channel;
        Ok(())
    }

    /// Update configuration for a channel
    pub fn update_config(&mut self, channel: Channel, config: ChannelConfig) {
        self.configs.insert(channel, config);
    }

    /// Get all available channels
    pub fn available_channels(&self) -> Vec<Channel> {
        self.configs.keys().copied().collect()
    }

    /// Validate channel configuration
    pub fn validate_config(&self, channel: Channel) -> crate::Result<()> {
        let config = self
            .get_config(channel)
            .ok_or_else(|| crate::UpdateError::ChannelNotFound(channel.to_string()))?;

        // Validate URL format
        if config.update_url.is_empty() {
            return Err(crate::UpdateError::VersionValidation(
                "Update URL cannot be empty".to_string(),
            ));
        }

        // Validate public key
        if config.public_key.is_empty() {
            return Err(crate::UpdateError::VersionValidation(
                "Public key cannot be empty".to_string(),
            ));
        }

        // Validate check frequency (minimum 1 hour)
        if config.check_frequency_hours == 0 {
            return Err(crate::UpdateError::VersionValidation(
                "Check frequency must be at least 1 hour".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_display() {
        assert_eq!(Channel::Stable.to_string(), "stable");
        assert_eq!(Channel::Beta.to_string(), "beta");
        assert_eq!(Channel::Canary.to_string(), "canary");
    }

    #[test]
    fn test_channel_from_str() {
        assert_eq!("stable".parse::<Channel>().unwrap(), Channel::Stable);
        assert_eq!("beta".parse::<Channel>().unwrap(), Channel::Beta);
        assert_eq!("canary".parse::<Channel>().unwrap(), Channel::Canary);
        assert!("invalid".parse::<Channel>().is_err());
    }

    #[test]
    fn test_channel_manager_creation() {
        let manager = ChannelManager::new();
        assert_eq!(manager.current_channel(), Channel::Stable);
        assert_eq!(manager.available_channels().len(), 3);
    }

    #[test]
    fn test_channel_switching() {
        let mut manager = ChannelManager::new();

        assert!(manager.switch_channel(Channel::Beta).is_ok());
        assert_eq!(manager.current_channel(), Channel::Beta);

        assert!(manager.switch_channel(Channel::Canary).is_ok());
        assert_eq!(manager.current_channel(), Channel::Canary);
    }

    #[test]
    fn test_config_validation() {
        let manager = ChannelManager::new();

        // Default configs should be valid
        assert!(manager.validate_config(Channel::Stable).is_ok());
        assert!(manager.validate_config(Channel::Beta).is_ok());
        assert!(manager.validate_config(Channel::Canary).is_ok());
    }

    /// An unrecognised channel name must return a ChannelNotFound error with the
    /// offending name preserved in the payload.
    #[test]
    fn test_unknown_channel_name_returns_channel_not_found_error() {
        let result = "nightly".parse::<Channel>();
        assert!(result.is_err(), "unknown channel must be an error");
        match result.unwrap_err() {
            crate::UpdateError::ChannelNotFound(name) => {
                assert_eq!(name, "nightly");
            }
            e => panic!("expected ChannelNotFound, got: {:?}", e),
        }
    }

    /// Stable channel must point at the stable update endpoint and have
    /// conservative settings (no pre-release, 24-hour check frequency).
    #[test]
    fn test_stable_channel_has_correct_url_and_metadata() {
        let manager = ChannelManager::new();
        let config = manager.get_config(Channel::Stable).unwrap();
        assert_eq!(config.update_url, "https://updates.flight-hub.dev/stable");
        assert!(
            !config.accept_prerelease,
            "stable must not accept pre-releases"
        );
        assert_eq!(config.check_frequency_hours, 24);
    }

    /// Beta channel must point at the beta endpoint and accept pre-release builds,
    /// with a more frequent check interval than stable.
    #[test]
    fn test_beta_channel_has_correct_url_and_metadata() {
        let manager = ChannelManager::new();
        let config = manager.get_config(Channel::Beta).unwrap();
        assert_eq!(config.update_url, "https://updates.flight-hub.dev/beta");
        assert!(config.accept_prerelease, "beta must accept pre-releases");
        assert_eq!(config.check_frequency_hours, 12);
    }

    /// Canary channel must have the most aggressive check frequency and accept
    /// pre-release builds.
    #[test]
    fn test_canary_channel_has_correct_url_and_metadata() {
        let manager = ChannelManager::new();
        let config = manager.get_config(Channel::Canary).unwrap();
        assert_eq!(config.update_url, "https://updates.flight-hub.dev/canary");
        assert!(config.accept_prerelease, "canary must accept pre-releases");
        assert_eq!(config.check_frequency_hours, 6);
    }

    /// validate_config must reject a configuration with check_frequency_hours == 0.
    #[test]
    fn test_validate_config_rejects_zero_check_frequency() {
        let mut manager = ChannelManager::new();
        let mut config = manager.get_config(Channel::Stable).unwrap().clone();
        config.check_frequency_hours = 0;
        manager.update_config(Channel::Stable, config);
        assert!(
            manager.validate_config(Channel::Stable).is_err(),
            "zero check frequency must be rejected"
        );
    }

    /// validate_config must reject a configuration with an empty update URL.
    #[test]
    fn test_validate_config_rejects_empty_url() {
        let mut manager = ChannelManager::new();
        let mut config = manager.get_config(Channel::Stable).unwrap().clone();
        config.update_url = String::new();
        manager.update_config(Channel::Stable, config);
        assert!(
            manager.validate_config(Channel::Stable).is_err(),
            "empty update URL must be rejected"
        );
    }
}
