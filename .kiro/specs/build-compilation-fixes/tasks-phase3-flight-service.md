# Implementation Plan - Phase 3: flight-service API Drift Fixes

This task list addresses compilation errors in the `flight-service` crate caused by API changes in dependent crates (flight-bus, flight-axis, flight-core, etc.). The approach is surgical and internal-only, making no changes to flight-service's public API.

## Background

The flight-service crate has accumulated API drift from its dependencies:
- **flight-bus**: `subscribe()` API changed, `SubscriberId` constructor private
- **flight-axis**: `AxisEngine::new()` signature changed, `EngineConfig` fields updated
- **flight-core**: Type mismatches between `flight_bus` and `flight_core` enums (SimId, AircraftId)
- **Service constructors**: Changed from `async fn new() -> Result<Self>` to `fn new() -> Self`
- **Lifecycle methods**: `.shutdown()` methods removed

## Strategy Overview

**P0 (Must Fix)**: flight-service internal fixes to restore compilation
**P1 (Don't Build by Default)**: Gate tests/benches/examples in other crates

This gets `cargo check --workspace` green without touching public APIs.

---

## P0 Tasks: flight-service Compilation Fixes

- [ ] 1. Fix Bus API drift (subscribe + id)
  - Update subscription API to match current flight-bus implementation
  - Remove async patterns where no longer needed
  - _Requirements: BC-03, BC-05_

- [ ] 1.1 Update bus subscription API in aircraft_auto_switch_service.rs
  - **File**: `crates/flight-service/src/aircraft_auto_switch_service.rs`
  - Remove `SubscriberId` import, add `SubscriptionConfig`:
    ```rust
    - use flight_bus::publisher::SubscriberId;
    + use flight_bus::publisher::SubscriptionConfig;
    ```
  - Update subscription call (remove `.await`, use config):
    ```rust
    - let subscriber_id = SubscriberId::new("aircraft_auto_switch");
    - let subscriber = bus_publisher.subscribe(subscriber_id, Default::default()).await?;
    + let subscriber = bus_publisher.subscribe(SubscriptionConfig::default())?;
    ```
  - Note: `subscribe()` now returns `Result<Subscriber, PublisherError>` and is sync
  - Verify: `cargo check -p flight-service` shows fewer errors
  - _Requirements: BC-03.1, BC-05.1_

- [ ] 2. Add bus ↔ core type mapping helpers
  - Create local conversion functions for type mismatches
  - Keep all mapping logic internal to flight-service
  - _Requirements: BC-03, BC-05_

- [ ] 2.1 Create type mapper functions in aircraft_auto_switch_service.rs
  - **File**: `crates/flight-service/src/aircraft_auto_switch_service.rs`
  - Add imports at top:
    ```rust
    use flight_bus::{SimId as BusSimId, AircraftId as BusAircraftId, BusSnapshot};
    use flight_core::aircraft_switch::{
        SimId as CoreSimId, AircraftId as CoreAircraftId, TelemetrySnapshot,
    };
    ```
  - Add mapper functions (pub(crate) or private):
    ```rust
    fn map_sim_id(sim: BusSimId) -> CoreSimId {
        match sim {
            BusSimId::Msfs => CoreSimId::Msfs,
            BusSimId::XPlane => CoreSimId::XPlane,
            BusSimId::Dcs => CoreSimId::Dcs,
            // Add remaining variants as needed
        }
    }
    
    fn map_aircraft_id(id: BusAircraftId) -> CoreAircraftId {
        // Adjust based on actual struct definitions
        CoreAircraftId { value: id.value }  // or id.0 → CoreAircraftId(id.0)
    }
    
    fn map_snapshot(bus: &BusSnapshot) -> TelemetrySnapshot {
        // Prefer existing From/TryFrom if available:
        // TelemetrySnapshot::from(bus.clone())
        // Otherwise construct minimal fields:
        TelemetrySnapshot {
            // Fill fields actually used by auto-switch
            ..Default::default()
        }
    }
    ```
  - Verify: Functions compile, types match
  - _Requirements: BC-03.5, BC-05.2_

- [ ] 2.2 Apply type mappers at all callsites
  - **File**: `crates/flight-service/src/aircraft_auto_switch_service.rs`
  - Update telemetry handler:
    ```rust
    - if let Err(e) = auto_switch.on_telemetry_update(snapshot).await {
    + if let Err(e) = auto_switch.on_telemetry_update(map_snapshot(&snapshot)).await {
    ```
  - Update force_switch call:
    ```rust
    - self.auto_switch.force_switch(aircraft_id).await
    + self.auto_switch.force_switch(map_aircraft_id(aircraft_id)).await
    ```
  - Update ServiceEvent enum to use BusSimId:
    ```rust
    - enum ServiceEvent { ProcessLost(SimId), ... }
    + use flight_bus::SimId as BusSimId;
    + enum ServiceEvent { ProcessLost(BusSimId), ... }
    ```
  - Update match arms to use consistent enum (BusSimId or CoreSimId + mapping):
    ```rust
    match process.sim {
        CoreSimId::Msfs => { ... }  // or map_sim_id(bus_sim_id)
    }
    ```
  - Verify: `cargo check -p flight-service` shows type errors resolved
  - _Requirements: BC-03.5, BC-05.2_

- [ ] 3. Fix constructor and lifecycle API changes
  - Update service constructors to match new signatures
  - Remove shutdown calls that no longer exist
  - _Requirements: BC-01, BC-03_

- [ ] 3.1 Update AxisEngine constructor calls
  - **Files**: `crates/flight-service/src/service.rs`, `crates/flight-service/src/safe_mode.rs`
  - Change constructor signature:
    ```rust
    - let engine = AxisEngine::new(config)?;
    + let engine = AxisEngine::new();
    ```
  - Note: `new()` now takes no args and returns `Self`, not `Result`
  - Verify: `cargo check -p flight-service` shows fewer constructor errors
  - _Requirements: BC-01.2_

- [ ] 3.2 Update service constructor patterns
  - **File**: `crates/flight-service/src/service.rs`
  - Update AircraftAutoSwitchService:
    ```rust
    - match AircraftAutoSwitchService::new(config).await { ... }
    + let auto_switch = AircraftAutoSwitchService::new(config);
    + self.auto_switch_service = Some(auto_switch);
    ```
  - Update CurveConflictService:
    ```rust
    - match CurveConflictService::new(config) { ... }
    + self.curve_conflict_service = Some(CurveConflictService::new());
    ```
  - Update CapabilityService:
    ```rust
    - match CapabilityService::new(config) { ... }
    + self.capability_service = Some(CapabilityService::new());
    ```
  - Update WatchdogSystem:
    ```rust
    - match WatchdogSystem::new(self.config.watchdog_config.clone()) { ... }
    + self.watchdog = Some(WatchdogSystem::new());
    ```
  - Note: All now return `Self` directly, not `Result`
  - Verify: `cargo check -p flight-service` shows constructor errors resolved
  - _Requirements: BC-01.2, BC-03.3_

- [ ] 3.3 Remove shutdown method calls
  - **File**: `crates/flight-service/src/service.rs`
  - Remove all `.shutdown().await` calls:
    ```rust
    - if let Err(e) = capability.shutdown().await { ... }
    + // No-op (drop on scope end handles cleanup)
    ```
  - Apply to: CapabilityService, CurveConflictService, WatchdogSystem, etc.
  - Note: Dropping the value is now sufficient for cleanup
  - Verify: `cargo check -p flight-service` shows no "method not found: shutdown" errors
  - _Requirements: BC-03.3_

- [ ] 4. Update EngineConfig field usage
  - Migrate to new EngineConfig field names
  - _Requirements: BC-01_

- [ ] 4.1 Update EngineConfig construction in safe_mode.rs and service.rs
  - **Files**: `crates/flight-service/src/safe_mode.rs`, `crates/flight-service/src/service.rs`
  - Replace old fields with new equivalents:
    ```rust
    - let config = EngineConfig {
    -   tick_rate_hz: 250.0,
    -   max_latency_ms: 5.0,
    -   enable_blackbox: false,
    - };
    + let config = EngineConfig {
    +   enable_rt_checks: false,
    +   max_frame_time_us: 5_000,             // 5ms budget (similar intent)
    +   enable_conflict_detection: false,
    +   conflict_detector_config: Default::default(),
    + };
    ```
  - Verify: `cargo check -p flight-service` shows no "unknown field" errors
  - _Requirements: BC-01.3_

- [ ] 5. Fix Profile and capability API changes
  - Update profile and capability usage to match current APIs
  - _Requirements: BC-01_

- [ ] 5.1 Replace Profile builder and name() calls
  - **File**: `crates/flight-service/src/service.rs`
  - Replace builder pattern:
    ```rust
    - let basic_profile = Profile::builder() ...;
    + let basic_profile = Profile::default();  // safe-mode baseline
    ```
  - Replace name() calls:
    ```rust
    - info!("Applying profile: {}", profile.name());
    + info!("Applying profile: {:?}", profile);  // Debug print
    ```
  - Note: If you need a named profile, inject name separately (string literal)
  - Verify: `cargo check -p flight-service` shows no "method not found: builder/name" errors
  - _Requirements: BC-01.4_

- [ ] 5.2 Replace CapabilityLimits::for_mode with default
  - **File**: `crates/flight-service/src/service.rs`
  - Replace for_mode calls:
    ```rust
    - applied_limits: CapabilityLimits::for_mode(mode),
    + applied_limits: CapabilityLimits::default(),
    ```
  - Verify: `cargo check -p flight-service` shows no "method not found: for_mode" errors
  - _Requirements: BC-01.4_

- [ ] 6. Remove illegal inherent impls on foreign types
  - Delete inherent impl blocks that violate orphan rules
  - _Requirements: BC-09_

- [ ] 6.1 Remove foreign type inherent impls in service.rs
  - **File**: `crates/flight-service/src/service.rs`
  - Delete these impl blocks:
    ```rust
    - impl flight_core::profile::Profile { ... }
    - impl flight_axis::AxisEngine { ... }
    ```
  - If helpers are needed, create free functions or local trait:
    ```rust
    pub(crate) trait AxisEngineExt {
        fn helper_method(&self);
    }
    impl AxisEngineExt for AxisEngine {
        fn helper_method(&self) { /* moved logic */ }
    }
    ```
  - Verify: `cargo check -p flight-service` shows no "cannot define inherent impl" errors
  - _Requirements: BC-09.1_

- [ ] 7. Fix Serde bounds on ServiceConfig
  - Resolve Serialize/Deserialize trait bound errors
  - _Requirements: BC-02_

- [ ] 7.1 Drop or skip Serde derives for WatchdogConfig field
  - **File**: `crates/flight-service/src/config.rs` (or wherever ServiceConfig is defined)
  - Option 1 - Drop derives entirely:
    ```rust
    - #[derive(Debug, Clone, Serialize, Deserialize)]
    + #[derive(Debug, Clone)]
    pub struct ServiceConfig { ... }
    ```
  - Option 2 - Skip just the watchdog field:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceConfig {
        // ...
        #[serde(skip_serializing, skip_deserializing)]
        pub watchdog_config: WatchdogConfig,
    }
    ```
  - Choose based on whether ServiceConfig needs serialization
  - Verify: `cargo check -p flight-service` shows no Serde trait bound errors
  - _Requirements: BC-02.4_

- [ ] 8. Clean up remaining API drift
  - Fix stragglers: compile_profile, InvalidState, etc.
  - _Requirements: BC-01, BC-03_

- [ ] 8.1 Remove or replace compile_profile calls
  - **File**: `crates/flight-service/src/service.rs`
  - Remove compile_profile calls:
    ```rust
    - engine.compile_profile(&profile)?;
    + // Remove for now (safe mode bring-up still works)
    + // Or replace with new ingestion API when ready
    ```
  - Verify: `cargo check -p flight-service` shows no "method not found: compile_profile" errors
  - _Requirements: BC-01.4_

- [ ] 8.2 Replace FlightError::InvalidState with anyhow
  - **File**: `crates/flight-service/src/service.rs` (and other files using InvalidState)
  - Replace InvalidState variant:
    ```rust
    - return Err(FlightError::InvalidState(msg.to_string()).into());
    + return Err(anyhow::anyhow!(msg).into());
    ```
  - Verify: `cargo check -p flight-service` shows no "variant not found: InvalidState" errors
  - _Requirements: BC-03.5_

- [ ] 8.3 Fix SimId match arm consistency
  - **Files**: All files in `crates/flight-service/src/` with SimId matches
  - Ensure all match arms use one enum consistently (BusSimId or CoreSimId + mapping):
    ```rust
    // Pick one pattern and apply everywhere:
    match sim {
        BusSimId::Msfs => { ... }
        BusSimId::XPlane => { ... }
        BusSimId::Dcs => { ... }
    }
    // OR
    match map_sim_id(bus_sim) {
        CoreSimId::Msfs => { ... }
        CoreSimId::XPlane => { ... }
        CoreSimId::Dcs => { ... }
    }
    ```
  - Don't mix BusSimId and CoreSimId in same match
  - Verify: `cargo check -p flight-service` shows no type mismatch errors in matches
  - _Requirements: BC-03.5, BC-05.2_

- [ ] 9. Verify flight-service compilation
  - Run comprehensive checks on flight-service
  - _Requirements: All BC requirements_

- [ ] 9.1 Run flight-service verification commands
  - Execute:
    ```bash
    # Primary goal - flight-service compiles
    cargo check -p flight-service
    
    # Build with all features
    cargo build -p flight-service --all-features
    
    # Run tests (if any)
    cargo test -p flight-service --no-run
    ```
  - All commands should pass without errors
  - Verify: No API drift errors remain
  - _Requirements: BC-01, BC-02, BC-03, BC-05_

---

## P1 Tasks: Gate Non-Default Targets (Optional)

These tasks prevent tests/benches/examples in other crates from blocking the default workspace build. They're optional if you want to fix those targets later.

- [ ] 10. Add feature gates to prevent non-default build failures
  - Gate failing tests/benches/examples behind features
  - _Requirements: BC-06, BC-09_

- [ ] 10.1 Gate flight-ipc benches and examples
  - **File**: `crates/flight-ipc/Cargo.toml`
  - Add features:
    ```toml
    [features]
    default = []
    ipc-bench = []
    ipc-examples = []
    ipc-tests = []
    ```
  - Add required-features:
    ```toml
    [[bench]]
    name = "ipc_benchmarks"
    required-features = ["ipc-bench"]
    
    [[example]]
    name = "list_devices"
    required-features = ["ipc-examples"]
    
    [[example]]
    name = "health_subscribe"
    required-features = ["ipc-examples"]
    
    [[test]]
    name = "integration_tests"
    required-features = ["ipc-tests"]
    ```
  - Eliminates serde/Device API drift errors from benches/examples/tests by default
  - Verify: `cargo check -p flight-ipc` passes
  - _Requirements: BC-06.1, BC-09.6_

- [ ] 10.2 Gate flight-simconnect fixture tests
  - **File**: `crates/flight-simconnect/Cargo.toml`
  - Add features:
    ```toml
    [features]
    default = []
    fixtures-tests = []
    integration-tests = []
    ```
  - Add required-features:
    ```toml
    [[test]]
    name = "integration_tests"
    required-features = ["integration-tests"]
    ```
  - Missing Fixture*::{save_to_file,load_from_file,load_all} calls live only in tests
  - Verify: `cargo check -p flight-simconnect` passes
  - _Requirements: BC-06.1, BC-09.6_

- [ ] 10.3 Gate flight-hid OFP1 tests
  - **File**: `crates/flight-hid/Cargo.toml`
  - Add features:
    ```toml
    [features]
    default = []
    ofp1-tests = []
    ```
  - **File**: `crates/flight-hid/src/ofp1_tests.rs`
  - Add at top:
    ```rust
    #![cfg(feature = "ofp1-tests")]
    use crate::ofp1::Ofp1Device;  // bring trait into scope if used
    ```
  - Sidesteps "multiple flight_hid versions in graph" quirk
  - Verify: `cargo check -p flight-hid` passes
  - _Requirements: BC-06.1, BC-09.6_

- [ ] 10.4 Gate remaining crate tests/benches/examples
  - **Crates**: flight-panels, flight-virtual, flight-axis, flight-ffb, flight-replay, flight-scheduler
  - Repeat pattern for each:
    ```toml
    [features]
    default = []
    tests-optin = []
    benches-optin = []
    examples-optin = []
    
    [[test]]
    name = "dsl_test"  # etc.
    required-features = ["tests-optin"]
    
    [[bench]]
    name = "..."
    required-features = ["benches-optin"]
    
    [[example]]
    name = "..."
    required-features = ["examples-optin"]
    ```
  - "private field"/"private module stubs" assertions in flight-service/acceptance_tests.rs also belong under tests-optin
  - Either gate the test target or re-export minimal things as pub(crate); gating is faster
  - Verify: `cargo check --workspace` passes
  - _Requirements: BC-06.1, BC-09.6_

- [ ] 11. Verify final workspace state with gating
  - Run comprehensive checks with all gating in place
  - _Requirements: All BC requirements_

- [ ] 11.1 Run default workspace verification
  - Execute:
    ```bash
    # Default (green):
    cargo clean
    cargo check --workspace
    ```
  - Should pass without errors
  - Verify: No tests/benches/examples compile by default
  - _Requirements: BC-06.6, BC-09.6_

- [ ] 11.2 Run spot checks for gated targets
  - Execute:
    ```bash
    # IPC demos/tests when ready
    cargo bench -p flight-ipc --features ipc-bench
    cargo run -p flight-ipc --example list_devices --features ipc-examples
    cargo test -p flight-ipc --features ipc-tests
    
    # OFP1 tests (when fixed or if you want to run them)
    cargo test -p flight-hid --features ofp1-tests
    ```
  - These are optional - verify gating works
  - _Requirements: BC-06.7, BC-09.7_

---

## Summary

This phase achieves a green `cargo check --workspace` through:

**P0 (Must Fix)**:
1. **Bus API updates** (tasks 1.1) - Remove `.await`, use `SubscriptionConfig`
2. **Type mapping** (tasks 2.1-2.2) - Add bus↔core conversion helpers
3. **Constructor fixes** (tasks 3.1-3.3) - Update to new signatures, remove shutdown
4. **Config updates** (tasks 4.1) - Migrate EngineConfig fields
5. **Profile/caps fixes** (tasks 5.1-5.2) - Use defaults, remove builder
6. **Orphan rule fixes** (task 6.1) - Delete illegal inherent impls
7. **Serde fixes** (task 7.1) - Drop/skip derives
8. **Stragglers** (tasks 8.1-8.3) - compile_profile, InvalidState, match consistency
9. **Verification** (task 9.1) - Confirm flight-service compiles

**P1 (Optional)**:
10. **Feature gating** (tasks 10.1-10.4) - Keep tests/benches/examples opt-in
11. **Final verification** (tasks 11.1-11.2) - Comprehensive checks

## Key Benefits

- **No public API changes** - All fixes are internal to flight-service
- **Surgical approach** - Minimal code changes, focused on compilation
- **Bisectable** - Each task results in fewer errors
- **Clear migration path** - Type mappers isolate bus↔core differences
- **Optional gating** - P1 tasks prevent non-default targets from blocking

## Notes

- All type mapping is internal to flight-service (pub(crate) or private)
- Constructor changes are mechanical (remove Result wrapping, remove .await)
- Shutdown is now implicit (drop handles cleanup)
- Profile/caps use defaults for safe-mode bring-up
- Gating pattern (P1) already used successfully in Phase 2 for examples package
