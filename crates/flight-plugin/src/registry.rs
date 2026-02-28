//! Plugin registry — tracks plugin lifecycle and state.
//!
//! The registry owns all loaded plugins and manages transitions between states.

use std::collections::HashMap;

use crate::capabilities::CapabilitySet;
use crate::sandbox::{PluginInstance, WasmSandbox};
use crate::{PluginError, PluginId, PluginManifest, PluginState, PluginType};

/// Summary information about a registered plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub plugin_type: PluginType,
    pub state: PluginState,
    pub capabilities: CapabilitySet,
}

/// Entry in the registry for a single plugin.
struct PluginEntry {
    manifest: PluginManifest,
    state: PluginState,
    instance: Option<PluginInstance>,
}

/// Manages the set of registered plugins and their lifecycles.
pub struct PluginRegistry {
    plugins: HashMap<PluginId, PluginEntry>,
    sandbox: WasmSandbox,
}

impl PluginRegistry {
    /// Create a new registry backed by the given WASM sandbox.
    pub fn new(sandbox: WasmSandbox) -> Self {
        Self {
            plugins: HashMap::new(),
            sandbox,
        }
    }

    /// Register a plugin manifest and return its unique ID.
    ///
    /// The plugin starts in the [`PluginState::Unloaded`] state.
    pub fn register(&mut self, manifest: PluginManifest) -> PluginId {
        let id = PluginId::new();
        tracing::info!(id = %id.0, name = %manifest.name, "plugin registered");
        self.plugins.insert(
            id,
            PluginEntry {
                manifest,
                state: PluginState::Unloaded,
                instance: None,
            },
        );
        id
    }

    /// Load WASM bytes for a registered plugin (transitions Unloaded → Loaded).
    pub fn load(&mut self, id: PluginId, wasm_bytes: &[u8]) -> Result<(), PluginError> {
        let entry = self.plugins.get_mut(&id).ok_or(PluginError::NotFound(id))?;

        if entry.state != PluginState::Unloaded {
            return Err(PluginError::InvalidStateTransition {
                from: entry.state,
                to: PluginState::Loaded,
            });
        }

        let instance = self.sandbox.load(&entry.manifest, wasm_bytes)?;
        entry.instance = Some(instance);
        entry.state = PluginState::Loaded;
        tracing::info!(id = %id.0, "plugin loaded");
        Ok(())
    }

    /// Start a loaded plugin (transitions Loaded|Suspended → Running).
    ///
    /// If the plugin is in Loaded state, `call_init()` is invoked. If it was
    /// Suspended, it resumes without re-initialising.
    pub fn start(&mut self, id: PluginId) -> Result<(), PluginError> {
        let entry = self.plugins.get_mut(&id).ok_or(PluginError::NotFound(id))?;

        match entry.state {
            PluginState::Loaded => {
                if let Some(ref mut instance) = entry.instance {
                    match instance.call_init() {
                        Ok(()) => {
                            entry.state = PluginState::Running;
                            tracing::info!(id = %id.0, "plugin started");
                            Ok(())
                        }
                        Err(e) => {
                            entry.state = PluginState::Failed;
                            tracing::error!(id = %id.0, error = %e, "plugin init failed");
                            Err(e)
                        }
                    }
                } else {
                    Err(PluginError::InvalidStateTransition {
                        from: entry.state,
                        to: PluginState::Running,
                    })
                }
            }
            PluginState::Suspended => {
                entry.state = PluginState::Running;
                tracing::info!(id = %id.0, "plugin resumed");
                Ok(())
            }
            _ => Err(PluginError::InvalidStateTransition {
                from: entry.state,
                to: PluginState::Running,
            }),
        }
    }

    /// Stop a running plugin (transitions Running → Suspended).
    pub fn stop(&mut self, id: PluginId) -> Result<(), PluginError> {
        let entry = self.plugins.get_mut(&id).ok_or(PluginError::NotFound(id))?;

        if entry.state != PluginState::Running {
            return Err(PluginError::InvalidStateTransition {
                from: entry.state,
                to: PluginState::Suspended,
            });
        }

        entry.state = PluginState::Suspended;
        tracing::info!(id = %id.0, "plugin stopped");
        Ok(())
    }

    /// Unload a plugin, calling shutdown if running (→ Unloaded).
    pub fn unload(&mut self, id: PluginId) -> Result<(), PluginError> {
        let entry = self.plugins.get_mut(&id).ok_or(PluginError::NotFound(id))?;

        // Attempt graceful shutdown if running
        if entry.state == PluginState::Running
            && let Some(ref mut instance) = entry.instance
            && let Err(e) = instance.call_shutdown()
        {
            tracing::warn!(id = %id.0, error = %e, "plugin shutdown failed during unload");
        }

        entry.instance = None;
        entry.state = PluginState::Unloaded;
        tracing::info!(id = %id.0, "plugin unloaded");
        Ok(())
    }

    /// List all registered plugins.
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins
            .iter()
            .map(|(id, entry)| PluginInfo {
                id: *id,
                name: entry.manifest.name.clone(),
                version: entry.manifest.version.clone(),
                plugin_type: entry.manifest.plugin_type,
                state: entry.state,
                capabilities: entry.manifest.capabilities,
            })
            .collect()
    }

    /// Get the current state of a plugin.
    pub fn get_state(&self, id: PluginId) -> Result<PluginState, PluginError> {
        self.plugins
            .get(&id)
            .map(|e| e.state)
            .ok_or(PluginError::NotFound(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::Capability;
    use crate::sandbox::ResourceLimits;
    use crate::sandbox::mock::MockWasmRuntime;
    use std::sync::atomic::Ordering;

    fn make_registry() -> (PluginRegistry, crate::sandbox::mock::MockControls) {
        let (runtime, controls) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
        (PluginRegistry::new(sandbox), controls)
    }

    fn test_manifest() -> PluginManifest {
        PluginManifest {
            name: "test-plugin".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadAxes]),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        }
    }

    #[test]
    fn register_returns_unique_ids() {
        let (mut registry, _) = make_registry();
        let id1 = registry.register(test_manifest());
        let id2 = registry.register(test_manifest());
        assert_ne!(id1, id2);
    }

    #[test]
    fn register_starts_unloaded() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Unloaded);
    }

    #[test]
    fn full_lifecycle_through_registry() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());

        // Unloaded → Loaded
        registry.load(id, b"fake-wasm").unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Loaded);

        // Loaded → Running
        registry.start(id).unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Running);

        // Running → Suspended
        registry.stop(id).unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Suspended);

        // Suspended → Running
        registry.start(id).unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Running);

        // Running → Unloaded
        registry.unload(id).unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Unloaded);
    }

    #[test]
    fn cannot_load_already_loaded() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        let err = registry.load(id, b"fake-wasm").unwrap_err();
        assert!(matches!(err, PluginError::InvalidStateTransition { .. }));
    }

    #[test]
    fn cannot_start_unloaded() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        let err = registry.start(id).unwrap_err();
        assert!(matches!(err, PluginError::InvalidStateTransition { .. }));
    }

    #[test]
    fn cannot_stop_non_running() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        let err = registry.stop(id).unwrap_err();
        assert!(matches!(err, PluginError::InvalidStateTransition { .. }));
    }

    #[test]
    fn not_found_errors() {
        let (mut registry, _) = make_registry();
        let fake_id = PluginId::new();
        assert!(matches!(
            registry.load(fake_id, b"wasm"),
            Err(PluginError::NotFound(_))
        ));
        assert!(matches!(
            registry.start(fake_id),
            Err(PluginError::NotFound(_))
        ));
        assert!(matches!(
            registry.stop(fake_id),
            Err(PluginError::NotFound(_))
        ));
        assert!(matches!(
            registry.unload(fake_id),
            Err(PluginError::NotFound(_))
        ));
        assert!(matches!(
            registry.get_state(fake_id),
            Err(PluginError::NotFound(_))
        ));
    }

    #[test]
    fn list_returns_all_plugins() {
        let (mut registry, _) = make_registry();
        let mut m1 = test_manifest();
        m1.name = "alpha".into();
        let mut m2 = test_manifest();
        m2.name = "beta".into();

        registry.register(m1);
        registry.register(m2);

        let list = registry.list();
        assert_eq!(list.len(), 2);
        let names: Vec<_> = list.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn list_reflects_state() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        registry.start(id).unwrap();

        let list = registry.list();
        let info = list.iter().find(|p| p.id == id).unwrap();
        assert_eq!(info.state, PluginState::Running);
    }

    #[test]
    fn failed_init_sets_failed_state() {
        let (mut registry, controls) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        controls.fail_init.store(true, Ordering::Relaxed);
        let err = registry.start(id).unwrap_err();
        assert!(matches!(err, PluginError::InitFailed(_)));
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Failed);
    }

    #[test]
    fn unload_from_suspended() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        registry.start(id).unwrap();
        registry.stop(id).unwrap();
        registry.unload(id).unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Unloaded);
    }

    #[test]
    fn unload_from_loaded() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        registry.unload(id).unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Unloaded);
    }

    #[test]
    fn unload_running_calls_shutdown() {
        let (mut registry, controls) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        registry.start(id).unwrap();
        // Even if shutdown fails, unload still succeeds
        controls.fail_shutdown.store(true, Ordering::Relaxed);
        registry.unload(id).unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Unloaded);
    }

    #[test]
    fn can_reload_after_unload() {
        let (mut registry, _) = make_registry();
        let id = registry.register(test_manifest());
        registry.load(id, b"fake-wasm").unwrap();
        registry.start(id).unwrap();
        registry.unload(id).unwrap();
        // Can reload
        registry.load(id, b"fake-wasm-v2").unwrap();
        assert_eq!(registry.get_state(id).unwrap(), PluginState::Loaded);
    }
}
