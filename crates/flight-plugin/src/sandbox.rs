//! WASM sandbox for plugin execution (ADR-003 Tier 1).
//!
//! Uses a trait-based backend so tests can use a mock WASM runtime while
//! production code can use wasmtime or another engine.

use crate::capabilities::{CapabilityChecker, CapabilitySet};
use crate::{PluginError, PluginManifest};

/// Resource limits applied to a WASM plugin instance.
#[derive(Debug, Clone, Copy)]
pub struct ResourceLimits {
    /// Maximum memory in bytes (default 16 MiB).
    pub max_memory_bytes: usize,
    /// Maximum fuel (execution budget). `None` means unlimited.
    pub max_fuel: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024, // 16 MiB
            max_fuel: Some(1_000_000),
        }
    }
}

/// Output data returned from a plugin tick.
#[derive(Debug, Clone, Default)]
pub struct TickOutput {
    /// Serialised output bytes produced by the plugin.
    pub data: Vec<u8>,
}

/// Trait abstracting the WASM runtime engine.
///
/// A mock implementation is used in tests; in production this would be backed
/// by wasmtime or a similar engine.
pub trait WasmRuntime: Send + Sync {
    /// Instantiate a WASM module from raw bytes with given resource limits.
    fn instantiate(
        &self,
        wasm_bytes: &[u8],
        limits: ResourceLimits,
        capabilities: CapabilitySet,
    ) -> Result<Box<dyn WasmInstance>, PluginError>;
}

/// Trait abstracting a single instantiated WASM module.
pub trait WasmInstance: Send + Sync {
    /// Call the plugin's `init` export.
    fn call_init(&mut self) -> Result<(), PluginError>;

    /// Call the plugin's `tick` export with input data, returning output.
    fn call_tick(&mut self, input: &[u8]) -> Result<TickOutput, PluginError>;

    /// Call the plugin's `shutdown` export.
    fn call_shutdown(&mut self) -> Result<(), PluginError>;

    /// Current memory usage in bytes.
    fn memory_used(&self) -> usize;

    /// Total fuel consumed since instantiation.
    fn fuel_consumed(&self) -> u64;
}

/// The WASM sandbox that loads and manages plugin instances.
pub struct WasmSandbox {
    runtime: Box<dyn WasmRuntime>,
    default_limits: ResourceLimits,
}

impl WasmSandbox {
    /// Create a new sandbox with the given runtime backend and default limits.
    pub fn new(runtime: Box<dyn WasmRuntime>, default_limits: ResourceLimits) -> Self {
        Self {
            runtime,
            default_limits,
        }
    }

    /// Load a plugin from its manifest and WASM bytes.
    ///
    /// Validates capabilities against the manifest before instantiation.
    pub fn load(
        &self,
        manifest: &PluginManifest,
        wasm_bytes: &[u8],
    ) -> Result<PluginInstance, PluginError> {
        if wasm_bytes.is_empty() {
            return Err(PluginError::InvalidWasm("empty WASM bytes".into()));
        }

        // Validate that frequency is in range
        if manifest.frequency_hz < 20 || manifest.frequency_hz > 120 {
            return Err(PluginError::InvalidManifest(format!(
                "WASM plugin frequency must be 20-120 Hz, got {}",
                manifest.frequency_hz
            )));
        }

        // Check the capability request against what is declared
        CapabilityChecker::check(manifest.capabilities, manifest.capabilities)?;

        let limits = ResourceLimits {
            max_memory_bytes: manifest
                .max_memory_bytes
                .unwrap_or(self.default_limits.max_memory_bytes),
            max_fuel: manifest.max_fuel.or(self.default_limits.max_fuel),
        };

        let instance = self
            .runtime
            .instantiate(wasm_bytes, limits, manifest.capabilities)?;

        tracing::info!(
            plugin = %manifest.name,
            version = %manifest.version,
            "WASM plugin loaded"
        );

        Ok(PluginInstance {
            instance,
            limits,
            manifest_name: manifest.name.clone(),
        })
    }
}

/// A loaded WASM plugin instance with lifecycle methods.
pub struct PluginInstance {
    instance: Box<dyn WasmInstance>,
    limits: ResourceLimits,
    manifest_name: String,
}

impl std::fmt::Debug for PluginInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginInstance")
            .field("manifest_name", &self.manifest_name)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl PluginInstance {
    /// Initialise the plugin.
    pub fn call_init(&mut self) -> Result<(), PluginError> {
        tracing::debug!(plugin = %self.manifest_name, "calling plugin init");
        self.instance.call_init()
    }

    /// Execute one tick with input data.
    pub fn call_tick(&mut self, input: &[u8]) -> Result<TickOutput, PluginError> {
        self.check_resource_limits()?;
        self.instance.call_tick(input)
    }

    /// Shut down the plugin.
    pub fn call_shutdown(&mut self) -> Result<(), PluginError> {
        tracing::debug!(plugin = %self.manifest_name, "calling plugin shutdown");
        self.instance.call_shutdown()
    }

    /// Current memory usage in bytes.
    pub fn memory_used(&self) -> usize {
        self.instance.memory_used()
    }

    /// Total fuel consumed.
    pub fn fuel_consumed(&self) -> u64 {
        self.instance.fuel_consumed()
    }

    /// Check resource limits and return an error if exceeded.
    fn check_resource_limits(&self) -> Result<(), PluginError> {
        let mem = self.instance.memory_used();
        if mem > self.limits.max_memory_bytes {
            return Err(PluginError::ResourceExhausted(format!(
                "memory limit exceeded: {} > {} bytes",
                mem, self.limits.max_memory_bytes
            )));
        }
        if let Some(max_fuel) = self.limits.max_fuel {
            let consumed = self.instance.fuel_consumed();
            if consumed > max_fuel {
                return Err(PluginError::ResourceExhausted(format!(
                    "fuel exhausted: {} > {} units",
                    consumed, max_fuel
                )));
            }
        }
        Ok(())
    }
}

// ── Mock runtime for testing ───────────────────────────────────────────

#[cfg(test)]
pub(crate) mod mock {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

    /// Control knobs for the mock instance.
    #[derive(Debug, Clone)]
    pub struct MockControls {
        pub memory_used: Arc<AtomicUsize>,
        pub fuel_consumed: Arc<AtomicU64>,
        pub fail_init: Arc<AtomicBool>,
        pub fail_tick: Arc<AtomicBool>,
        pub fail_shutdown: Arc<AtomicBool>,
    }

    impl Default for MockControls {
        fn default() -> Self {
            Self {
                memory_used: Arc::new(AtomicUsize::new(1024)),
                fuel_consumed: Arc::new(AtomicU64::new(0)),
                fail_init: Arc::new(AtomicBool::new(false)),
                fail_tick: Arc::new(AtomicBool::new(false)),
                fail_shutdown: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    pub struct MockWasmRuntime {
        pub controls: MockControls,
        pub fail_instantiate: bool,
    }

    impl MockWasmRuntime {
        pub fn new() -> (Self, MockControls) {
            let controls = MockControls::default();
            let runtime = Self {
                controls: controls.clone(),
                fail_instantiate: false,
            };
            (runtime, controls)
        }

        pub fn failing() -> Self {
            Self {
                controls: MockControls::default(),
                fail_instantiate: true,
            }
        }
    }

    impl WasmRuntime for MockWasmRuntime {
        fn instantiate(
            &self,
            _wasm_bytes: &[u8],
            _limits: ResourceLimits,
            _capabilities: CapabilitySet,
        ) -> Result<Box<dyn WasmInstance>, PluginError> {
            if self.fail_instantiate {
                return Err(PluginError::InvalidWasm(
                    "mock instantiation failure".into(),
                ));
            }
            Ok(Box::new(MockWasmInstance {
                controls: self.controls.clone(),
                tick_count: 0,
            }))
        }
    }

    pub struct MockWasmInstance {
        controls: MockControls,
        tick_count: u64,
    }

    impl WasmInstance for MockWasmInstance {
        fn call_init(&mut self) -> Result<(), PluginError> {
            if self.controls.fail_init.load(Ordering::Relaxed) {
                return Err(PluginError::InitFailed("mock init failure".into()));
            }
            Ok(())
        }

        fn call_tick(&mut self, input: &[u8]) -> Result<TickOutput, PluginError> {
            if self.controls.fail_tick.load(Ordering::Relaxed) {
                return Err(PluginError::TickFailed("mock tick failure".into()));
            }
            self.tick_count += 1;
            // Increment fuel consumed per tick
            self.controls
                .fuel_consumed
                .fetch_add(100, Ordering::Relaxed);
            Ok(TickOutput {
                data: input.to_vec(),
            })
        }

        fn call_shutdown(&mut self) -> Result<(), PluginError> {
            if self.controls.fail_shutdown.load(Ordering::Relaxed) {
                return Err(PluginError::ShutdownFailed("mock shutdown failure".into()));
            }
            Ok(())
        }

        fn memory_used(&self) -> usize {
            self.controls.memory_used.load(Ordering::Relaxed)
        }

        fn fuel_consumed(&self) -> u64 {
            self.controls.fuel_consumed.load(Ordering::Relaxed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::*;
    use super::*;
    use crate::PluginManifest;
    use crate::capabilities::Capability;
    use std::sync::atomic::Ordering;

    fn test_manifest() -> PluginManifest {
        PluginManifest {
            name: "test-plugin".into(),
            version: "1.0.0".into(),
            plugin_type: crate::PluginType::Wasm,
            capabilities: CapabilitySet::from_caps([
                Capability::ReadAxes,
                Capability::ReadTelemetry,
            ]),
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
    fn load_and_init_plugin() {
        let (sandbox, _controls) = make_sandbox();
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        assert!(instance.call_init().is_ok());
    }

    #[test]
    fn full_lifecycle() {
        let (sandbox, _controls) = make_sandbox();
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        instance.call_init().unwrap();
        let output = instance.call_tick(b"input-data").unwrap();
        assert_eq!(output.data, b"input-data");
        instance.call_shutdown().unwrap();
    }

    #[test]
    fn reject_empty_wasm() {
        let (sandbox, _controls) = make_sandbox();
        let manifest = test_manifest();
        let err = sandbox.load(&manifest, b"").unwrap_err();
        assert!(matches!(err, PluginError::InvalidWasm(_)));
    }

    #[test]
    fn reject_invalid_frequency_low() {
        let (sandbox, _controls) = make_sandbox();
        let mut manifest = test_manifest();
        manifest.frequency_hz = 10;
        let err = sandbox.load(&manifest, b"fake-wasm").unwrap_err();
        assert!(matches!(err, PluginError::InvalidManifest(_)));
    }

    #[test]
    fn reject_invalid_frequency_high() {
        let (sandbox, _controls) = make_sandbox();
        let mut manifest = test_manifest();
        manifest.frequency_hz = 200;
        let err = sandbox.load(&manifest, b"fake-wasm").unwrap_err();
        assert!(matches!(err, PluginError::InvalidManifest(_)));
    }

    #[test]
    fn instantiation_failure() {
        let runtime = MockWasmRuntime::failing();
        let sandbox = WasmSandbox::new(Box::new(runtime), ResourceLimits::default());
        let manifest = test_manifest();
        let err = sandbox.load(&manifest, b"fake-wasm").unwrap_err();
        assert!(matches!(err, PluginError::InvalidWasm(_)));
    }

    #[test]
    fn init_failure() {
        let (sandbox, controls) = make_sandbox();
        controls.fail_init.store(true, Ordering::Relaxed);
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        let err = instance.call_init().unwrap_err();
        assert!(matches!(err, PluginError::InitFailed(_)));
    }

    #[test]
    fn tick_failure() {
        let (sandbox, controls) = make_sandbox();
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        instance.call_init().unwrap();
        controls.fail_tick.store(true, Ordering::Relaxed);
        let err = instance.call_tick(b"data").unwrap_err();
        assert!(matches!(err, PluginError::TickFailed(_)));
    }

    #[test]
    fn shutdown_failure() {
        let (sandbox, controls) = make_sandbox();
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        instance.call_init().unwrap();
        controls.fail_shutdown.store(true, Ordering::Relaxed);
        let err = instance.call_shutdown().unwrap_err();
        assert!(matches!(err, PluginError::ShutdownFailed(_)));
    }

    #[test]
    fn memory_limit_exceeded() {
        let (sandbox, controls) = make_sandbox();
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        instance.call_init().unwrap();
        // Exceed the 16 MiB default limit
        controls
            .memory_used
            .store(20 * 1024 * 1024, Ordering::Relaxed);
        let err = instance.call_tick(b"data").unwrap_err();
        assert!(matches!(err, PluginError::ResourceExhausted(_)));
    }

    #[test]
    fn fuel_limit_exceeded() {
        let (sandbox, controls) = make_sandbox();
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        instance.call_init().unwrap();
        // Exceed the 1M fuel default
        controls.fuel_consumed.store(2_000_000, Ordering::Relaxed);
        let err = instance.call_tick(b"data").unwrap_err();
        assert!(matches!(err, PluginError::ResourceExhausted(_)));
    }

    #[test]
    fn memory_and_fuel_tracking() {
        let (sandbox, controls) = make_sandbox();
        let manifest = test_manifest();
        let mut instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        instance.call_init().unwrap();
        assert_eq!(instance.memory_used(), 1024);
        assert_eq!(instance.fuel_consumed(), 0);

        instance.call_tick(b"tick1").unwrap();
        assert_eq!(instance.fuel_consumed(), 100);

        instance.call_tick(b"tick2").unwrap();
        assert_eq!(instance.fuel_consumed(), 200);

        controls.memory_used.store(4096, Ordering::Relaxed);
        assert_eq!(instance.memory_used(), 4096);
    }

    #[test]
    fn custom_resource_limits_from_manifest() {
        let (sandbox, _controls) = make_sandbox();
        let mut manifest = test_manifest();
        manifest.max_memory_bytes = Some(8 * 1024 * 1024);
        manifest.max_fuel = Some(500_000);
        let instance = sandbox.load(&manifest, b"fake-wasm").unwrap();
        assert_eq!(instance.limits.max_memory_bytes, 8 * 1024 * 1024);
        assert_eq!(instance.limits.max_fuel, Some(500_000));
    }

    #[test]
    fn default_resource_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_bytes, 16 * 1024 * 1024);
        assert_eq!(limits.max_fuel, Some(1_000_000));
    }
}
