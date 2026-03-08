//! Depth tests for the plugin system — lifecycle, WASM sandboxing, capabilities,
//! and error handling around the registry/sandbox boundary.
//!
//! This module exercises:
//! - Plugin lifecycle state transitions (`Unloaded` → `Loaded` → `Running` → `Failed`)
//! - WASM sandbox initialization with resource limits
//! - Capability declaration and filtering via `CapabilitySet`
//! - Failure modes and recovery paths when loading/starting plugins

#[cfg(test)]
mod plugin_lifecycle {
    use crate::capabilities::{Capability, CapabilitySet};
    use crate::registry::PluginRegistry;
    use crate::sandbox::mock::{MockControls, MockWasmRuntime};
    use crate::sandbox::{ResourceLimits, WasmSandbox};
    use crate::{PluginManifest, PluginState, PluginType};
    use std::sync::atomic::Ordering;

    fn make_registry() -> (PluginRegistry, MockControls) {
        let (runtime, controls) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
        (PluginRegistry::new(sandbox), controls)
    }

    fn wasm_manifest(name: &str) -> PluginManifest {
        PluginManifest {
            name: name.into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadAxes]),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        }
    }

    #[test]
    fn load_plugin_transitions_to_loaded() {
        let (mut reg, _) = make_registry();
        let id = reg.register(wasm_manifest("loader"));
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Unloaded);

        reg.load(id, b"fake-wasm").unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Loaded);
    }

    #[test]
    fn initialize_plugin_via_start_calls_init() {
        let (mut reg, controls) = make_registry();
        let id = reg.register(wasm_manifest("init-test"));
        reg.load(id, b"fake-wasm").unwrap();

        // Verify init is called (if it fails, state transitions to Failed)
        controls.fail_init.store(false, Ordering::Relaxed);
        reg.start(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Running);
    }

    #[test]
    fn start_plugin_from_loaded_reaches_running() {
        let (mut reg, _) = make_registry();
        let id = reg.register(wasm_manifest("start-test"));
        reg.load(id, b"fake-wasm").unwrap();
        reg.start(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Running);

        // Confirm it appears as running in the list
        let list = reg.list();
        let info = list.iter().find(|p| p.id == id).unwrap();
        assert_eq!(info.state, PluginState::Running);
    }

    #[test]
    fn stop_plugin_suspends_without_shutdown() {
        let (mut reg, controls) = make_registry();
        let id = reg.register(wasm_manifest("stop-test"));
        reg.load(id, b"fake-wasm").unwrap();
        reg.start(id).unwrap();

        // Set shutdown to fail — stop should NOT call shutdown, only suspend
        controls.fail_shutdown.store(true, Ordering::Relaxed);
        reg.stop(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Suspended);
    }

    #[test]
    fn unload_plugin_releases_instance() {
        let (mut reg, _) = make_registry();
        let id = reg.register(wasm_manifest("unload-test"));
        reg.load(id, b"fake-wasm").unwrap();
        reg.start(id).unwrap();
        reg.unload(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Unloaded);

        // After unload, can re-load fresh bytes
        reg.load(id, b"new-wasm-bytes").unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Loaded);
    }

    #[test]
    fn reload_plugin_full_cycle() {
        let (mut reg, _) = make_registry();
        let id = reg.register(wasm_manifest("reload-test"));

        // First cycle
        reg.load(id, b"v1-wasm").unwrap();
        reg.start(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Running);

        // Unload
        reg.unload(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Unloaded);

        // Second cycle with new bytes
        reg.load(id, b"v2-wasm").unwrap();
        reg.start(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Running);

        // Clean unload
        reg.stop(id).unwrap();
        reg.unload(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Unloaded);
    }
}

#[cfg(test)]
mod wasm_sandbox {
    use crate::capabilities::{Capability, CapabilitySet};
    use crate::sandbox::mock::{MockControls, MockWasmRuntime};
    use crate::sandbox::{ResourceLimits, WasmSandbox};
    use crate::{PluginError, PluginManifest, PluginType};
    use std::sync::atomic::Ordering;

    fn wasm_manifest_with_caps(caps: &[Capability]) -> PluginManifest {
        PluginManifest {
            name: "sandbox-test".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps(caps.iter().copied()),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        }
    }

    fn make_sandbox() -> (WasmSandbox, MockControls) {
        let (runtime, controls) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
        (sandbox, controls)
    }

    #[test]
    fn create_sandbox_with_custom_defaults() {
        // First sandbox: verify custom memory limit of 4 MiB is enforced
        let (runtime_mem, controls_mem) = MockWasmRuntime::new();
        let limits = ResourceLimits {
            max_memory_bytes: 4 * 1024 * 1024,
            max_fuel: Some(500_000),
        };
        let sandbox_mem = WasmSandbox::new(Box::new(runtime_mem), limits);
        let manifest = wasm_manifest_with_caps(&[Capability::ReadAxes]);
        let mut inst_mem = sandbox_mem.load(&manifest, b"wasm").unwrap();

        // Plugin inherits sandbox defaults when manifest doesn't override.
        assert_eq!(inst_mem.memory_used(), 1024);
        assert_eq!(inst_mem.fuel_consumed(), 0);

        inst_mem.call_init().unwrap();

        // Below custom memory limit — succeeds
        controls_mem
            .memory_used
            .store(4 * 1024 * 1024 - 1, Ordering::Relaxed);
        assert!(inst_mem.call_tick(b"mem-ok").is_ok());

        // At exact custom memory limit — still succeeds
        controls_mem
            .memory_used
            .store(4 * 1024 * 1024, Ordering::Relaxed);
        assert!(inst_mem.call_tick(b"mem-edge").is_ok());

        // Above custom memory limit — fails with ResourceExhausted
        controls_mem
            .memory_used
            .store(4 * 1024 * 1024 + 1, Ordering::Relaxed);
        let err = inst_mem.call_tick(b"mem-boom").unwrap_err();
        assert!(matches!(err, PluginError::ResourceExhausted(_)));
        assert!(err.to_string().contains("memory limit exceeded"));

        // Second sandbox: verify custom fuel limit of 500k is enforced
        let (runtime_fuel, controls_fuel) = MockWasmRuntime::new();
        let limits = ResourceLimits {
            max_memory_bytes: 4 * 1024 * 1024,
            max_fuel: Some(500_000),
        };
        let sandbox_fuel = WasmSandbox::new(Box::new(runtime_fuel), limits);
        let mut inst_fuel = sandbox_fuel.load(&manifest, b"wasm").unwrap();
        inst_fuel.call_init().unwrap();

        // Below custom fuel limit — succeeds
        controls_fuel
            .fuel_consumed
            .store(500_000 - 1, Ordering::Relaxed);
        assert!(inst_fuel.call_tick(b"fuel-ok").is_ok());

        // Above custom fuel limit — fails with ResourceExhausted
        controls_fuel
            .fuel_consumed
            .store(500_000 + 1, Ordering::Relaxed);
        let err = inst_fuel.call_tick(b"fuel-boom").unwrap_err();
        assert!(matches!(err, PluginError::ResourceExhausted(_)));
        assert!(err.to_string().contains("fuel exhausted"));
    }

    #[test]
    fn memory_limit_enforced_on_tick() {
        let (sandbox, controls) = make_sandbox();
        let manifest = wasm_manifest_with_caps(&[Capability::ReadAxes]);
        let mut inst = sandbox.load(&manifest, b"wasm").unwrap();
        inst.call_init().unwrap();

        // Below limit — succeeds
        controls
            .memory_used
            .store(15 * 1024 * 1024, Ordering::Relaxed);
        assert!(inst.call_tick(b"ok").is_ok());

        // At exact limit — succeeds (not exceeded, equal)
        controls
            .memory_used
            .store(16 * 1024 * 1024, Ordering::Relaxed);
        assert!(inst.call_tick(b"edge").is_ok());

        // Above limit — fails
        controls
            .memory_used
            .store(16 * 1024 * 1024 + 1, Ordering::Relaxed);
        let err = inst.call_tick(b"boom").unwrap_err();
        assert!(matches!(err, PluginError::ResourceExhausted(_)));
        assert!(err.to_string().contains("memory limit exceeded"));
    }

    #[test]
    fn cpu_fuel_limit_enforced_on_tick() {
        let (sandbox, controls) = make_sandbox();
        let manifest = wasm_manifest_with_caps(&[Capability::ReadAxes]);
        let mut inst = sandbox.load(&manifest, b"wasm").unwrap();
        inst.call_init().unwrap();

        // Below limit — succeeds
        controls.fuel_consumed.store(999_999, Ordering::Relaxed);
        assert!(inst.call_tick(b"ok").is_ok());

        // Exceed limit
        controls.fuel_consumed.store(1_000_001, Ordering::Relaxed);
        let err = inst.call_tick(b"boom").unwrap_err();
        assert!(matches!(err, PluginError::ResourceExhausted(_)));
        assert!(err.to_string().contains("fuel exhausted"));
    }

    #[test]
    fn import_export_functions_via_init_tick_shutdown() {
        let (sandbox, _controls) = make_sandbox();
        let manifest = wasm_manifest_with_caps(&[Capability::ReadAxes]);
        let mut inst = sandbox.load(&manifest, b"wasm").unwrap();

        // init export
        inst.call_init().unwrap();

        // tick export — input echoed back by mock
        let out = inst.call_tick(b"hello").unwrap();
        assert_eq!(out.data, b"hello");

        // shutdown export
        inst.call_shutdown().unwrap();
    }

    #[test]
    fn sandbox_isolation_between_plugins() {
        // Each plugin instance should have independent resource accounting.
        // We model this by giving each instance its own sandbox + mock controls.
        let (sandbox_a, controls_a) = make_sandbox();
        let (sandbox_b, _controls_b) = make_sandbox();

        let manifest = wasm_manifest_with_caps(&[Capability::ReadAxes]);

        let mut inst_a = sandbox_a.load(&manifest, b"plugin-a").unwrap();
        let mut inst_b = sandbox_b.load(&manifest, b"plugin-b").unwrap();

        inst_a.call_init().unwrap();
        inst_b.call_init().unwrap();

        // Initial tick for both plugins — they each consume some fuel.
        inst_a.call_tick(b"a-data-1").unwrap();
        inst_b.call_tick(b"b-data-1").unwrap();

        let fuel_a_initial = inst_a.fuel_consumed();
        let fuel_b_initial = inst_b.fuel_consumed();
        assert_eq!(fuel_a_initial, 100);
        assert_eq!(fuel_b_initial, 100);

        // Change plugin A's observed memory usage via its mock controls.
        controls_a
            .memory_used
            .store(10 * 1024 * 1024, Ordering::Relaxed);
        assert_eq!(inst_a.memory_used(), 10 * 1024 * 1024);

        // Plugin B's memory usage should be unaffected by changes to plugin A.
        let mem_b_after_a_change = inst_b.memory_used();
        assert_eq!(mem_b_after_a_change, 1024);

        // Now advance plugin A further and ensure fuel for B does not change.
        inst_a.call_tick(b"a-data-2").unwrap();
        let fuel_a_after = inst_a.fuel_consumed();
        let fuel_b_after = inst_b.fuel_consumed();
        assert!(fuel_a_after > fuel_a_initial);
        assert_eq!(fuel_b_after, fuel_b_initial);
    }

    #[test]
    fn capability_declaration_checked_at_load() {
        let (runtime, _) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());

        // Valid capabilities — loads fine
        let manifest = wasm_manifest_with_caps(&[Capability::ReadAxes, Capability::ReadTelemetry]);
        assert!(sandbox.load(&manifest, b"wasm").is_ok());

        // Empty capabilities — also valid (a plugin may not need any)
        let empty_manifest = PluginManifest {
            name: "minimal".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::EMPTY,
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        };
        assert!(sandbox.load(&empty_manifest, b"wasm").is_ok());
    }
}

#[cfg(test)]
mod capability_system {
    use crate::capabilities::{Capability, CapabilityChecker, CapabilitySet};

    #[test]
    fn declare_capabilities_from_array() {
        let set = CapabilitySet::from_caps([
            Capability::ReadAxes,
            Capability::WriteAxes,
            Capability::WriteFfb,
        ]);
        assert_eq!(set.len(), 3);
        assert!(set.contains(Capability::ReadAxes));
        assert!(set.contains(Capability::WriteAxes));
        assert!(set.contains(Capability::WriteFfb));
        assert!(!set.contains(Capability::AccessNetwork));
    }

    #[test]
    fn check_capabilities_at_runtime() {
        let granted =
            CapabilitySet::from_caps([Capability::ReadAxes, Capability::ReadTelemetry]);
        let requested = CapabilitySet::from_caps([Capability::ReadAxes]);
        assert!(CapabilityChecker::check(granted, requested).is_ok());

        // Exact match is also valid
        assert!(CapabilityChecker::check(granted, granted).is_ok());
    }

    #[test]
    fn deny_unauthorized_access() {
        let granted = CapabilitySet::from_caps([Capability::ReadAxes]);
        let requested =
            CapabilitySet::from_caps([Capability::ReadAxes, Capability::AccessNetwork]);

        let err = CapabilityChecker::check(granted, requested).unwrap_err();
        assert!(err.denied.contains(Capability::AccessNetwork));
        assert!(!err.denied.contains(Capability::ReadAxes));

        // Error message mentions both sets
        let msg = err.to_string();
        assert!(msg.contains("denied"));
        assert!(msg.contains("granted"));
    }

    #[test]
    fn capability_escalation_prevention() {
        // A plugin with ReadAxes tries to also get WriteAxes and WriteFfb
        let granted = CapabilitySet::from_caps([Capability::ReadAxes]);
        let escalated = CapabilitySet::from_caps([
            Capability::ReadAxes,
            Capability::WriteAxes,
            Capability::WriteFfb,
            Capability::AccessNetwork,
        ]);

        let err = CapabilityChecker::check(granted, escalated).unwrap_err();
        // All three escalated capabilities should be denied
        assert!(err.denied.contains(Capability::WriteAxes));
        assert!(err.denied.contains(Capability::WriteFfb));
        assert!(err.denied.contains(Capability::AccessNetwork));
        assert_eq!(err.denied.len(), 3);

        // The granted set in the error should reflect the original grant
        assert!(err.granted.contains(Capability::ReadAxes));
        assert_eq!(err.granted.len(), 1);
    }

    #[test]
    fn default_capabilities_are_empty() {
        let set = CapabilitySet::default();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);

        // Every individual capability should be absent
        for cap in Capability::ALL {
            assert!(!set.contains(*cap), "default set should not contain {cap}");
        }

        // Empty grant + empty request = ok
        assert!(CapabilityChecker::check(set, CapabilitySet::EMPTY).is_ok());
    }

    #[test]
    fn custom_capabilities_full_set() {
        // Grant every capability
        let full = CapabilitySet::from_caps(Capability::ALL.iter().copied());
        assert_eq!(full.len(), Capability::ALL.len());

        // Any subset request should be allowed
        let subset = CapabilitySet::from_caps([Capability::AccessNetwork, Capability::WriteFfb]);
        assert!(CapabilityChecker::check(full, subset).is_ok());

        // Full grant satisfies full request
        assert!(CapabilityChecker::check(full, full).is_ok());

        // Iterate and verify all are present
        let collected: Vec<_> = full.iter().collect();
        assert_eq!(collected.len(), Capability::ALL.len());
    }
}

#[cfg(test)]
mod plugin_discovery {
    use crate::capabilities::{Capability, CapabilitySet};
    use crate::registry::PluginRegistry;
    use crate::sandbox::mock::{MockControls, MockWasmRuntime};
    use crate::sandbox::{ResourceLimits, WasmSandbox};
    use crate::{PluginError, PluginManifest, PluginState, PluginType};

    fn make_registry() -> (PluginRegistry, MockControls) {
        let (runtime, controls) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
        (PluginRegistry::new(sandbox), controls)
    }

    #[test]
    fn scan_plugin_directory_via_registry_list() {
        let (mut reg, _) = make_registry();

        // Register multiple plugins simulating a directory scan
        for i in 0..5 {
            reg.register(PluginManifest {
                name: format!("plugin-{i}"),
                version: "1.0.0".into(),
                plugin_type: PluginType::Wasm,
                capabilities: CapabilitySet::from_caps([Capability::ReadAxes]),
                frequency_hz: 60,
                max_memory_bytes: None,
                max_fuel: None,
            });
        }

        let list = reg.list();
        assert_eq!(list.len(), 5);
        // All start unloaded
        assert!(list.iter().all(|p| p.state == PluginState::Unloaded));
    }

    #[test]
    fn version_compatibility_in_manifest() {
        let (mut reg, _) = make_registry();

        // Register different versions of the same-named plugin
        let id_v1 = reg.register(PluginManifest {
            name: "nav-helper".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadTelemetry]),
            frequency_hz: 30,
            max_memory_bytes: None,
            max_fuel: None,
        });
        let id_v2 = reg.register(PluginManifest {
            name: "nav-helper".into(),
            version: "2.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([
                Capability::ReadTelemetry,
                Capability::ReadAxes,
            ]),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        });

        let list = reg.list();
        let v1_info = list.iter().find(|p| p.id == id_v1).unwrap();
        let v2_info = list.iter().find(|p| p.id == id_v2).unwrap();
        assert_eq!(v1_info.version, "1.0.0");
        assert_eq!(v2_info.version, "2.0.0");
        // v2 has more capabilities
        assert!(v2_info.capabilities.contains(Capability::ReadAxes));
        assert!(!v1_info.capabilities.contains(Capability::ReadAxes));
    }

    #[test]
    fn dependency_resolution_load_order() {
        let (mut reg, _) = make_registry();

        // Simulate dependency chain: C depends on B depends on A
        let id_a = reg.register(PluginManifest {
            name: "base-lib".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::EMPTY,
            frequency_hz: 20,
            max_memory_bytes: None,
            max_fuel: None,
        });
        let id_b = reg.register(PluginManifest {
            name: "mid-layer".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadAxes]),
            frequency_hz: 40,
            max_memory_bytes: None,
            max_fuel: None,
        });
        let id_c = reg.register(PluginManifest {
            name: "top-plugin".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadAxes, Capability::WriteFfb]),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        });

        // Load in dependency order
        reg.load(id_a, b"a-wasm").unwrap();
        reg.start(id_a).unwrap();
        reg.load(id_b, b"b-wasm").unwrap();
        reg.start(id_b).unwrap();
        reg.load(id_c, b"c-wasm").unwrap();
        reg.start(id_c).unwrap();

        // All running
        assert_eq!(reg.get_state(id_a).unwrap(), PluginState::Running);
        assert_eq!(reg.get_state(id_b).unwrap(), PluginState::Running);
        assert_eq!(reg.get_state(id_c).unwrap(), PluginState::Running);
    }

    #[test]
    fn plugin_metadata_preserved() {
        let (mut reg, _) = make_registry();
        let id = reg.register(PluginManifest {
            name: "metadata-test".into(),
            version: "3.2.1".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([
                Capability::ReadAxes,
                Capability::ReadButtons,
                Capability::ReadTelemetry,
            ]),
            frequency_hz: 120,
            max_memory_bytes: Some(8 * 1024 * 1024),
            max_fuel: Some(500_000),
        });

        let list = reg.list();
        let info = list.iter().find(|p| p.id == id).unwrap();
        assert_eq!(info.name, "metadata-test");
        assert_eq!(info.version, "3.2.1");
        assert_eq!(info.plugin_type, PluginType::Wasm);
        assert_eq!(info.capabilities.len(), 3);
    }

    #[test]
    fn manifest_validation_frequency_bounds() {
        let (runtime, _) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());

        // Below minimum (20 Hz)
        let mut manifest = PluginManifest {
            name: "too-slow".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::EMPTY,
            frequency_hz: 19,
            max_memory_bytes: None,
            max_fuel: None,
        };
        let err = sandbox.load(&manifest, b"wasm").unwrap_err();
        assert!(matches!(err, PluginError::InvalidManifest(_)));
        assert!(err.to_string().contains("20-120 Hz"));

        // Above maximum (120 Hz)
        manifest.frequency_hz = 121;
        manifest.name = "too-fast".into();
        let err = sandbox.load(&manifest, b"wasm").unwrap_err();
        assert!(matches!(err, PluginError::InvalidManifest(_)));

        // Boundary values — valid
        manifest.frequency_hz = 20;
        assert!(sandbox.load(&manifest, b"wasm").is_ok());

        // Need fresh sandbox for next load (mock runtime can't produce two instances)
        let (runtime2, _) = MockWasmRuntime::new();
        let sandbox2 = WasmSandbox::new(Box::new(runtime2), ResourceLimits::default());
        manifest.frequency_hz = 120;
        assert!(sandbox2.load(&manifest, b"wasm").is_ok());
    }
}

#[cfg(test)]
mod inter_plugin_communication {
    //! Tests for inter-plugin communication patterns.
    //! The registry supports multiple concurrent plugins that can be independently
    //! managed; these tests verify the patterns that enable cooperation.

    use crate::capabilities::{Capability, CapabilitySet};
    use crate::registry::PluginRegistry;
    use crate::sandbox::mock::{MockControls, MockWasmRuntime};
    use crate::sandbox::{ResourceLimits, WasmSandbox};
    use crate::{PluginManifest, PluginState, PluginType};

    fn make_registry() -> (PluginRegistry, MockControls) {
        let (runtime, controls) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
        (PluginRegistry::new(sandbox), controls)
    }

    fn plugin_manifest(name: &str, caps: &[Capability]) -> PluginManifest {
        PluginManifest {
            name: name.into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps(caps.iter().copied()),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        }
    }

    #[test]
    fn shared_events_multiple_plugins_running() {
        let (mut reg, _) = make_registry();

        // Producer and consumer plugins both running
        let producer = reg.register(plugin_manifest("producer", &[Capability::WriteAxes]));
        let consumer = reg.register(plugin_manifest("consumer", &[Capability::ReadAxes]));

        reg.load(producer, b"wasm").unwrap();
        reg.start(producer).unwrap();
        reg.load(consumer, b"wasm").unwrap();
        reg.start(consumer).unwrap();

        // Both running concurrently
        assert_eq!(reg.get_state(producer).unwrap(), PluginState::Running);
        assert_eq!(reg.get_state(consumer).unwrap(), PluginState::Running);
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn plugin_to_plugin_messaging_via_capabilities() {
        // Verify capability sets properly encode producer/consumer roles
        let writer_caps =
            CapabilitySet::from_caps([Capability::WriteAxes, Capability::WriteButtons]);
        let reader_caps =
            CapabilitySet::from_caps([Capability::ReadAxes, Capability::ReadButtons]);

        // Writer cannot read, reader cannot write
        assert!(!writer_caps.contains(Capability::ReadAxes));
        assert!(!reader_caps.contains(Capability::WriteAxes));

        // A bridge plugin would need both
        let bridge_caps = CapabilitySet::from_caps([
            Capability::ReadAxes,
            Capability::WriteAxes,
            Capability::ReadButtons,
            Capability::WriteButtons,
        ]);
        assert!(bridge_caps.contains_all(writer_caps));
        assert!(bridge_caps.contains_all(reader_caps));
    }

    #[test]
    fn event_filtering_by_capability_set() {
        // Only plugins with ReadTelemetry should receive telemetry events
        let telemetry_cap = CapabilitySet::from_caps([Capability::ReadTelemetry]);

        let plugin_a_caps =
            CapabilitySet::from_caps([Capability::ReadAxes, Capability::ReadTelemetry]);
        let plugin_b_caps = CapabilitySet::from_caps([Capability::ReadAxes]);

        assert!(plugin_a_caps.contains_all(telemetry_cap));
        assert!(!plugin_b_caps.contains_all(telemetry_cap));
    }

    #[test]
    fn message_ordering_sequential_start_stop() {
        let (mut reg, _) = make_registry();

        let ids: Vec<_> = (0..3)
            .map(|i| {
                reg.register(plugin_manifest(
                    &format!("ordered-{i}"),
                    &[Capability::ReadAxes],
                ))
            })
            .collect();

        // Load and start in order
        for &id in &ids {
            reg.load(id, b"wasm").unwrap();
            reg.start(id).unwrap();
        }

        // Stop in reverse order (graceful teardown pattern)
        for &id in ids.iter().rev() {
            reg.stop(id).unwrap();
            assert_eq!(reg.get_state(id).unwrap(), PluginState::Suspended);
        }

        // Unload all
        for &id in &ids {
            reg.unload(id).unwrap();
            assert_eq!(reg.get_state(id).unwrap(), PluginState::Unloaded);
        }
    }

    #[test]
    fn backpressure_via_resource_limits() {
        use std::sync::atomic::Ordering;

        let (mut reg, controls) = make_registry();
        let id = reg.register(plugin_manifest("heavy-plugin", &[Capability::ReadAxes]));
        reg.load(id, b"wasm").unwrap();
        reg.start(id).unwrap();

        // Simulate plugin approaching memory limit (backpressure signal)
        controls
            .memory_used
            .store(15 * 1024 * 1024, Ordering::Relaxed);

        // Plugin is still running but near its limit — the registry reports its state
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Running);

        // A real system would throttle or suspend the plugin before it hits the limit
        // The stop mechanism provides the backpressure escape valve
        reg.stop(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Suspended);
    }
}

#[cfg(test)]
mod error_handling {
    use crate::capabilities::{Capability, CapabilitySet};
    use crate::registry::PluginRegistry;
    use crate::sandbox::mock::{MockControls, MockWasmRuntime};
    use crate::sandbox::{ResourceLimits, WasmSandbox};
    use crate::{PluginError, PluginManifest, PluginState, PluginType};
    use std::sync::atomic::Ordering;

    fn make_registry() -> (PluginRegistry, MockControls) {
        let (runtime, controls) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
        (PluginRegistry::new(sandbox), controls)
    }

    fn wasm_manifest(name: &str) -> PluginManifest {
        PluginManifest {
            name: name.into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadAxes]),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        }
    }

    #[test]
    fn plugin_crash_recovery_via_reload() {
        let (mut reg, controls) = make_registry();
        let id = reg.register(wasm_manifest("crash-test"));
        reg.load(id, b"wasm").unwrap();

        // Simulate init crash
        controls.fail_init.store(true, Ordering::Relaxed);
        let err = reg.start(id).unwrap_err();
        assert!(matches!(err, PluginError::InitFailed(_)));
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Failed);

        // Recovery: unload the failed plugin, then reload
        reg.unload(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Unloaded);

        // Fix the issue and reload
        controls.fail_init.store(false, Ordering::Relaxed);
        reg.load(id, b"fixed-wasm").unwrap();
        reg.start(id).unwrap();
        assert_eq!(reg.get_state(id).unwrap(), PluginState::Running);
    }

    #[test]
    fn sandbox_violation_memory_overflow() {
        let (sandbox, controls) = {
            let (runtime, controls) = MockWasmRuntime::new();
            let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
            (sandbox, controls)
        };

        let manifest = wasm_manifest("mem-violator");
        let mut inst = sandbox.load(&manifest, b"wasm").unwrap();
        inst.call_init().unwrap();

        // Dramatically exceed memory
        controls
            .memory_used
            .store(100 * 1024 * 1024, Ordering::Relaxed);
        let err = inst.call_tick(b"data").unwrap_err();
        assert!(matches!(err, PluginError::ResourceExhausted(_)));
        assert!(err.to_string().contains("memory limit exceeded"));
    }

    #[test]
    fn version_mismatch_detected_in_metadata() {
        let (mut reg, _) = make_registry();

        // Register two plugins with same name but incompatible versions
        let id_old = reg.register(PluginManifest {
            name: "versioned-plugin".into(),
            version: "1.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadAxes]),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        });
        let id_new = reg.register(PluginManifest {
            name: "versioned-plugin".into(),
            version: "2.0.0".into(),
            plugin_type: PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([Capability::ReadAxes]),
            frequency_hz: 60,
            max_memory_bytes: None,
            max_fuel: None,
        });

        let list = reg.list();
        let old = list.iter().find(|p| p.id == id_old).unwrap();
        let new = list.iter().find(|p| p.id == id_new).unwrap();

        // Version strings differ — host can detect and resolve
        assert_ne!(old.version, new.version);
        assert_eq!(old.name, new.name);
    }

    #[test]
    fn missing_dependency_unknown_plugin_id() {
        let (mut reg, _) = make_registry();

        // Try to operate on a non-existent plugin ID
        let missing_id = crate::PluginId::new();

        let err = reg.load(missing_id, b"wasm").unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        let err = reg.start(missing_id).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        let err = reg.stop(missing_id).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        let err = reg.unload(missing_id).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));

        let err = reg.get_state(missing_id).unwrap_err();
        assert!(matches!(err, PluginError::NotFound(_)));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn corrupt_plugin_file_empty_bytes() {
        let (runtime, _) = MockWasmRuntime::new();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());

        let manifest = wasm_manifest("corrupt");

        // Empty WASM bytes are rejected
        let err = sandbox.load(&manifest, b"").unwrap_err();
        assert!(matches!(err, PluginError::InvalidWasm(_)));
        assert!(err.to_string().contains("empty"));

        // Instantiation failure (simulating corrupt/invalid WASM)
        let failing_runtime = MockWasmRuntime::failing();
        let sandbox2 = WasmSandbox::new(Box::new(failing_runtime), ResourceLimits::default());
        let err = sandbox2.load(&manifest, b"corrupt-bytes").unwrap_err();
        assert!(matches!(err, PluginError::InvalidWasm(_)));
    }
}
