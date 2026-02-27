// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin trait interface for OpenFlight extensions (REQ-810).
//!
//! Defines the lifecycle trait that all plugins implement, along with
//! tier, state, and error types. See ADR-003 for tier definitions.

use std::fmt;

/// Plugin lifecycle trait for OpenFlight extensions.
/// Plugins register at startup and receive lifecycle callbacks.
pub trait Plugin: Send + Sync {
    /// Unique plugin identifier (e.g. `"com.example.my-plugin"`).
    fn id(&self) -> &str;
    /// Human-readable display name.
    fn name(&self) -> &str;
    /// Semantic version string (e.g. `"1.0.0"`).
    fn version(&self) -> &str;
    /// Plugin tier (WASM, Native, Service) — see ADR-003.
    fn tier(&self) -> PluginTier;
    /// Called when the plugin is loaded into the registry.
    fn on_load(&mut self) -> Result<(), PluginError>;
    /// Called when the plugin is unloaded from the registry.
    fn on_unload(&mut self) -> Result<(), PluginError>;
    /// Called every tick for active plugins.
    fn on_tick(&mut self, tick: u64) -> Result<(), PluginError>;
}

/// Plugin execution tier per ADR-003.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTier {
    /// Sandboxed WASM, 20–120 Hz.
    Wasm,
    /// Isolated helper process, shared-memory SPSC.
    Native,
    /// Managed thread, full access with user consent.
    Service,
}

impl fmt::Display for PluginTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wasm => f.write_str("WASM"),
            Self::Native => f.write_str("Native"),
            Self::Service => f.write_str("Service"),
        }
    }
}

/// Current state of a plugin in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Discovered but not yet loaded.
    Discovered,
    /// Currently loading.
    Loading,
    /// Successfully loaded and receiving ticks.
    Active,
    /// An error occurred; carries the error message.
    Error(String),
    /// Plugin has been unloaded.
    Unloaded,
}

impl fmt::Display for PluginState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovered => f.write_str("Discovered"),
            Self::Loading => f.write_str("Loading"),
            Self::Active => f.write_str("Active"),
            Self::Error(msg) => write!(f, "Error({msg})"),
            Self::Unloaded => f.write_str("Unloaded"),
        }
    }
}

/// Error returned by plugin lifecycle methods.
#[derive(Debug)]
pub struct PluginError {
    /// Human-readable error description.
    pub message: String,
    /// Categorised error kind.
    pub kind: PluginErrorKind,
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for PluginError {}

/// Categorised plugin error kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginErrorKind {
    /// Plugin failed to load.
    LoadFailed,
    /// Plugin failed during initialisation.
    InitFailed,
    /// Plugin failed during a tick callback.
    TickFailed,
    /// Plugin failed to unload cleanly.
    UnloadFailed,
}

impl fmt::Display for PluginErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed => f.write_str("LoadFailed"),
            Self::InitFailed => f.write_str("InitFailed"),
            Self::TickFailed => f.write_str("TickFailed"),
            Self::UnloadFailed => f.write_str("UnloadFailed"),
        }
    }
}
