// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Plugin registry for managing plugin lifecycle (REQ-810).
//!
//! Provides thread-safe registration, lifecycle management, and
//! tick dispatch for all loaded plugins.

use std::time::Instant;

use parking_lot::RwLock;

use crate::plugin::{Plugin, PluginError, PluginErrorKind, PluginState};

/// A registered plugin together with its runtime metadata.
struct PluginEntry {
    plugin: Box<dyn Plugin>,
    state: PluginState,
    registered_at: Instant,
}

/// Thread-safe registry that owns all loaded plugins.
pub struct PluginRegistry {
    entries: RwLock<Vec<PluginEntry>>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }

    /// Register a plugin. Transitions it through Loading → Active (or Error).
    ///
    /// Returns `Ok(())` if the plugin loaded successfully, or `Err` with the
    /// original `PluginError` if `on_load` failed (the plugin is still kept in
    /// the registry in `Error` state).
    pub fn register(&self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let id = plugin.id().to_owned();

        // Reject duplicates.
        {
            let entries = self.entries.read();
            if entries.iter().any(|e| e.plugin.id() == id) {
                return Err(PluginError {
                    message: format!("plugin '{id}' is already registered"),
                    kind: PluginErrorKind::LoadFailed,
                });
            }
        }

        let mut entry = PluginEntry {
            plugin,
            state: PluginState::Loading,
            registered_at: Instant::now(),
        };

        match entry.plugin.on_load() {
            Ok(()) => {
                entry.state = PluginState::Active;
                self.entries.write().push(entry);
                Ok(())
            }
            Err(e) => {
                entry.state = PluginState::Error(e.message.clone());
                self.entries.write().push(entry);
                Err(e)
            }
        }
    }

    /// Unregister a plugin by id. Calls `on_unload` before removal.
    ///
    /// Returns `true` if the plugin was found and removed.
    pub fn unregister(&self, id: &str) -> bool {
        let mut entries = self.entries.write();
        if let Some(pos) = entries.iter().position(|e| e.plugin.id() == id) {
            let entry = &mut entries[pos];
            // Best-effort unload; ignore errors.
            let _ = entry.plugin.on_unload();
            entry.state = PluginState::Unloaded;
            entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Tick all active plugins. Plugins that fail are moved to `Error` state.
    pub fn tick_all(&self, tick: u64) {
        let mut entries = self.entries.write();
        for entry in entries.iter_mut() {
            if entry.state == PluginState::Active
                && let Err(e) = entry.plugin.on_tick(tick)
            {
                entry.state = PluginState::Error(e.message);
            }
        }
    }

    /// Return a snapshot of `(id, name, state)` for every registered plugin.
    #[must_use]
    pub fn list(&self) -> Vec<(String, String, PluginState)> {
        self.entries
            .read()
            .iter()
            .map(|e| {
                (
                    e.plugin.id().to_owned(),
                    e.plugin.name().to_owned(),
                    e.state.clone(),
                )
            })
            .collect()
    }

    /// Look up a plugin's state by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<PluginState> {
        self.entries
            .read()
            .iter()
            .find(|e| e.plugin.id() == id)
            .map(|e| e.state.clone())
    }

    /// Return how long ago a plugin was registered.
    #[must_use]
    pub fn registered_since(&self, id: &str) -> Option<std::time::Duration> {
        self.entries
            .read()
            .iter()
            .find(|e| e.plugin.id() == id)
            .map(|e| e.registered_at.elapsed())
    }

    /// Number of currently registered plugins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginError, PluginErrorKind, PluginTier};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Minimal test plugin that succeeds on every lifecycle call.
    struct StubPlugin {
        id: &'static str,
        tick_count: Arc<AtomicU64>,
    }

    impl StubPlugin {
        fn new(id: &'static str) -> (Self, Arc<AtomicU64>) {
            let count = Arc::new(AtomicU64::new(0));
            (
                Self {
                    id,
                    tick_count: Arc::clone(&count),
                },
                count,
            )
        }
    }

    impl Plugin for StubPlugin {
        fn id(&self) -> &str {
            self.id
        }
        fn name(&self) -> &str {
            "Stub"
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn tier(&self) -> PluginTier {
            PluginTier::Service
        }
        fn on_load(&mut self) -> Result<(), PluginError> {
            Ok(())
        }
        fn on_unload(&mut self) -> Result<(), PluginError> {
            Ok(())
        }
        fn on_tick(&mut self, _tick: u64) -> Result<(), PluginError> {
            self.tick_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    /// A plugin whose `on_load` always fails.
    struct FailLoadPlugin;

    impl Plugin for FailLoadPlugin {
        fn id(&self) -> &str {
            "fail-load"
        }
        fn name(&self) -> &str {
            "FailLoad"
        }
        fn version(&self) -> &str {
            "0.0.1"
        }
        fn tier(&self) -> PluginTier {
            PluginTier::Wasm
        }
        fn on_load(&mut self) -> Result<(), PluginError> {
            Err(PluginError {
                message: "boom".into(),
                kind: PluginErrorKind::LoadFailed,
            })
        }
        fn on_unload(&mut self) -> Result<(), PluginError> {
            Ok(())
        }
        fn on_tick(&mut self, _tick: u64) -> Result<(), PluginError> {
            Ok(())
        }
    }

    /// A plugin whose `on_tick` fails on the first call.
    struct FailTickPlugin;

    impl Plugin for FailTickPlugin {
        fn id(&self) -> &str {
            "fail-tick"
        }
        fn name(&self) -> &str {
            "FailTick"
        }
        fn version(&self) -> &str {
            "0.0.1"
        }
        fn tier(&self) -> PluginTier {
            PluginTier::Native
        }
        fn on_load(&mut self) -> Result<(), PluginError> {
            Ok(())
        }
        fn on_unload(&mut self) -> Result<(), PluginError> {
            Ok(())
        }
        fn on_tick(&mut self, _tick: u64) -> Result<(), PluginError> {
            Err(PluginError {
                message: "tick failed".into(),
                kind: PluginErrorKind::TickFailed,
            })
        }
    }

    #[test]
    fn plugin_register_and_unregister() {
        let registry = PluginRegistry::new();
        let (plugin, _count) = StubPlugin::new("test-1");

        assert!(registry.is_empty());
        registry.register(Box::new(plugin)).unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get("test-1"), Some(PluginState::Active));

        assert!(registry.unregister("test-1"));
        assert!(registry.is_empty());
        assert_eq!(registry.get("test-1"), None);
    }

    #[test]
    fn plugin_lifecycle_load_tick_unload() {
        let registry = PluginRegistry::new();
        let (plugin, tick_count) = StubPlugin::new("lifecycle");

        registry.register(Box::new(plugin)).unwrap();
        assert_eq!(registry.get("lifecycle"), Some(PluginState::Active));

        registry.tick_all(1);
        registry.tick_all(2);
        registry.tick_all(3);
        assert_eq!(tick_count.load(Ordering::Relaxed), 3);

        assert!(registry.unregister("lifecycle"));
    }

    #[test]
    fn plugin_load_failure_does_not_crash_registry() {
        let registry = PluginRegistry::new();
        let result = registry.register(Box::new(FailLoadPlugin));
        assert!(result.is_err());

        // Plugin is kept in Error state.
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.get("fail-load"),
            Some(PluginState::Error("boom".into()))
        );

        // Registry is still usable.
        let (good, _count) = StubPlugin::new("good");
        registry.register(Box::new(good)).unwrap();
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn plugin_tick_failure_moves_to_error_state() {
        let registry = PluginRegistry::new();
        registry.register(Box::new(FailTickPlugin)).unwrap();
        assert_eq!(registry.get("fail-tick"), Some(PluginState::Active));

        registry.tick_all(1);
        assert_eq!(
            registry.get("fail-tick"),
            Some(PluginState::Error("tick failed".into()))
        );

        // Subsequent ticks skip errored plugins.
        registry.tick_all(2);
        assert_eq!(
            registry.get("fail-tick"),
            Some(PluginState::Error("tick failed".into()))
        );
    }

    #[test]
    fn plugin_list_returns_correct_states() {
        let registry = PluginRegistry::new();
        let (p1, _) = StubPlugin::new("a");
        let (p2, _) = StubPlugin::new("b");

        registry.register(Box::new(p1)).unwrap();
        registry.register(Box::new(p2)).unwrap();

        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|(_, _, s)| *s == PluginState::Active));
    }

    #[test]
    fn plugin_duplicate_registration_rejected() {
        let registry = PluginRegistry::new();
        let (p1, _) = StubPlugin::new("dup");
        let (p2, _) = StubPlugin::new("dup");

        registry.register(Box::new(p1)).unwrap();
        let result = registry.register(Box::new(p2));
        assert!(result.is_err());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn plugin_unregister_nonexistent_returns_false() {
        let registry = PluginRegistry::new();
        assert!(!registry.unregister("no-such-plugin"));
    }

    #[test]
    fn plugin_default_trait() {
        let registry = PluginRegistry::default();
        assert!(registry.is_empty());
    }
}
