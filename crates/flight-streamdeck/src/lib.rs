//! StreamDeck plugin for Flight Hub
//!
//! Provides local Web API for StreamDeck integration with sample profiles
//! for GA/Airbus/Helo aircraft types. Includes version compatibility management
//! and graceful degradation for unsupported StreamDeck app versions.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub mod api;
pub mod compatibility;
pub mod plugin;
pub mod profiles;
pub mod server;
pub mod verify;

pub use api::{StreamDeckApi, ApiError};
pub use compatibility::{VersionCompatibility, CompatibilityMatrix, VersionRange};
pub use plugin::{StreamDeckPlugin, PluginConfig, PluginError};
pub use profiles::{ProfileManager, SampleProfiles, AircraftType};
pub use server::{StreamDeckServer, ServerConfig, ServerError};
pub use verify::{VerifyTest, VerifyResult, EventRoundTrip};

/// StreamDeck app version information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: Option<u32>,
}

impl AppVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            build: None,
        }
    }

    pub fn with_build(major: u32, minor: u32, patch: u32, build: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            build: Some(build),
        }
    }

    pub fn from_string(version_str: &str) -> Result<Self, VersionError> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() < 3 || parts.len() > 4 {
            return Err(VersionError::InvalidFormat(version_str.to_string()));
        }

        let major = parts[0].parse().map_err(|_| VersionError::InvalidFormat(version_str.to_string()))?;
        let minor = parts[1].parse().map_err(|_| VersionError::InvalidFormat(version_str.to_string()))?;
        let patch = parts[2].parse().map_err(|_| VersionError::InvalidFormat(version_str.to_string()))?;
        
        let build = if parts.len() == 4 {
            Some(parts[3].parse().map_err(|_| VersionError::InvalidFormat(version_str.to_string()))?)
        } else {
            None
        };

        Ok(Self { major, minor, patch, build })
    }
}

impl std::fmt::Display for AppVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(build) = self.build {
            write!(f, "{}.{}.{}.{}", self.major, self.minor, self.patch, build)
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

/// StreamDeck plugin errors
#[derive(Debug, Error)]
pub enum VersionError {
    #[error("Invalid version format: {0}")]
    InvalidFormat(String),
    
    #[error("Version not supported: {version}, supported range: {min_version} - {max_version}")]
    NotSupported {
        version: String,
        min_version: String,
        max_version: String,
    },
    
    #[error("Version compatibility check failed: {0}")]
    CompatibilityCheckFailed(String),
}

/// Main StreamDeck integration struct
pub struct FlightStreamDeck {
    server: StreamDeckServer,
    plugin: StreamDeckPlugin,
    compatibility: VersionCompatibility,
    profile_manager: ProfileManager,
}

impl FlightStreamDeck {
    /// Create new StreamDeck integration with default configuration
    pub fn new() -> Result<Self> {
        let server_config = ServerConfig::default();
        let plugin_config = PluginConfig::default();
        
        Ok(Self {
            server: StreamDeckServer::new(server_config)?,
            plugin: StreamDeckPlugin::new(plugin_config)?,
            compatibility: VersionCompatibility::new(),
            profile_manager: ProfileManager::new(),
        })
    }

    /// Create new StreamDeck integration with custom configuration
    pub fn with_config(server_config: ServerConfig, plugin_config: PluginConfig) -> Result<Self> {
        Ok(Self {
            server: StreamDeckServer::new(server_config)?,
            plugin: StreamDeckPlugin::new(plugin_config)?,
            compatibility: VersionCompatibility::new(),
            profile_manager: ProfileManager::new(),
        })
    }

    /// Start the StreamDeck server and plugin
    pub async fn start(&mut self) -> Result<()> {
        // Load sample profiles
        self.profile_manager.load_sample_profiles()?;
        
        // Start the web API server
        self.server.start().await?;
        
        // Initialize the plugin
        self.plugin.initialize().await?;
        
        tracing::info!("StreamDeck integration started successfully");
        Ok(())
    }

    /// Stop the StreamDeck server and plugin
    pub async fn stop(&mut self) -> Result<()> {
        self.plugin.shutdown().await?;
        self.server.stop().await?;
        
        tracing::info!("StreamDeck integration stopped");
        Ok(())
    }

    /// Check version compatibility with StreamDeck app
    pub fn check_version_compatibility(&self, app_version: &AppVersion) -> Result<bool, VersionError> {
        self.compatibility.is_compatible(app_version)
    }

    /// Get compatibility matrix for documentation
    pub fn get_compatibility_matrix(&self) -> &CompatibilityMatrix {
        self.compatibility.get_matrix()
    }

    /// Get server port for documentation
    pub fn get_server_port(&self) -> u16 {
        self.server.get_port()
    }

    /// Get available sample profiles
    pub fn get_sample_profiles(&self) -> &HashMap<AircraftType, serde_json::Value> {
        self.profile_manager.get_profiles()
    }

    /// Run verify test for event round-trip
    pub async fn run_verify_test(&mut self) -> Result<VerifyResult> {
        self.plugin.run_verify_test().await
    }
}
