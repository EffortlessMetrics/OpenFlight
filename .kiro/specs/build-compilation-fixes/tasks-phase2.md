# Implementation Plan - Phase 2: Remaining Compilation Fixes

This task list addresses compilation errors discovered during verification (task 5.6) that were not covered in the initial implementation phase.

## Background

After completing tasks 1.1-5.6, `cargo check --workspace` revealed additional compilation errors in:
- **flight-hub-examples** package (examples with outdated APIs, missing dependencies)
- **flight-simconnect** crate (missing serde derives, borrow conflicts, async safety)

These issues require fixes to bring the workspace to a fully compiling state.

## Strategy

- **No public API changes** - only fix call sites and internal implementation
- **Add missing dependencies** - resolve E0433 errors by adding flight_bus, flight_ffb, tracing deps
- **Mechanical API updates** - update call sites to match current BlackboxWriter/Reader APIs
- **Fix async safety** - resolve non-Send future and borrow checker issues in flight-simconnect
- **Bisectable commits** - each task should result in a compilable state

---

## Tasks

- [x] 1. Fix flight-hub-examples compilation errors





  - Add missing dependencies to resolve E0433 errors
  - Update BlackboxWriter/Reader API usage to match current implementation
  - Add missing async runtime attributes to resolve E0752 errors
  - Fix BlackboxRecord variant construction to resolve E0223 errors
  - _Requirements: BC-06, BC-09_

- [x] 1.1 Add missing dependencies and async runtime setup


  - **File**: `flight-hub-examples/Cargo.toml`
  - Add dependencies:
    ```toml
    anyhow = "1"
    tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
    flight-core = { path = "../crates/flight-core", features = ["serde"] }
    flight-bus = { path = "../crates/flight-bus" }
    flight-ffb = { path = "../crates/flight-ffb" }
    ```
  - **Files**: `examples/capture_replay_demo.rs`, `examples/streamdeck_panel_demo.rs`, `examples/telemetry_synth_demo.rs`
  - Add async main attribute and logging initialization:
    ```rust
    #[tokio::main(flavor = "current_thread")]
    async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
        // ... existing code ...
        Ok(())
    }
    ```
  - Verify: `cargo check -p flight-hub-examples` resolves E0433 (unresolved crate) and E0752 (async main) errors
  - _Requirements: BC-06.1, BC-06.5_

- [x] 1.2 Update BlackboxWriter API calls


  - **Files**: All examples using BlackboxWriter (capture_replay_demo.rs, etc.)
  - Update `start_recording()` calls (E0061 - wrong arity):
    ```rust
    // Before: writer.start_recording().await?;
    // After:
    writer.start_recording(
        "openflight-demo".into(),
        "msfs".into(),
        "C172".into()
    ).await?;
    ```
  - Update `write_record()` to `write()` (E0599 - method not found):
    ```rust
    // Before: writer.write_record(record).await?;
    // After: writer.write(record).await?;
    ```
  - Fix BlackboxRecord variant construction (E0223 - ambiguous associated type):
    ```rust
    // Before: BlackboxRecord::AxisFrame { timestamp, frame }
    // After: BlackboxRecord::Axis(axis_frame)
    
    // Before: BlackboxRecord::BusSnapshot { timestamp, snapshot }
    // After: BlackboxRecord::BusSnapshot(snapshot)
    
    // Before: BlackboxRecord::Event { timestamp, event }
    // After: BlackboxRecord::Event(event)
    ```
  - Verify: `cargo check -p flight-hub-examples` resolves E0061, E0599, E0223 errors
  - _Requirements: BC-06.2, BC-06.3_

- [x] 1.3 Update BlackboxReader API calls and stats access

  - **Files**: All examples using BlackboxReader
  - Replace constructor (E0599 - method not found):
    ```rust
    // Before: let mut reader = BlackboxReader::new(&recording_path)?;
    // After: let mut reader = BlackboxReader::open(&recording_path)?;
    ```
  - Update stats field access (E0609 - no field):
    ```rust
    // Before:
    // println!("  Axis frames: {}", stats.axis_frames_written);
    // println!("  Bus snapshots: {}", stats.bus_snapshots_written);
    // println!("  Events: {}", stats.events_written);
    // println!("  Compression ratio: {:.1}%", stats.compression_ratio * 100.0);
    
    // After:
    println!("  Records written: {}", stats.records_written);
    println!("  Bytes written:   {}", stats.bytes_written);
    println!("  Chunks written:  {}", stats.chunks_written);
    ```
  - Verify: `cargo check -p flight-hub-examples` passes without errors
  - _Requirements: BC-06.2_

- [x] 1.4 Add platform-specific gates for Windows-only examples (optional)

  - **Files**: Any Windows-specific examples
  - Add `#[cfg(windows)]` attribute at top of file if example uses Windows-only APIs
  - Alternative: Feature-gate in Cargo.toml with `[target.'cfg(windows)'.dependencies]`
  - Verify: `cargo check -p flight-hub-examples` passes on both Windows and Linux
  - _Requirements: BC-04.6_

- [x] 2. Fix flight-simconnect compilation errors





  - Add serde derives to SessionFixture to resolve E0277 trait bound errors
  - Fix non-Send future in tokio::spawn by taking receiver ownership
  - Resolve borrow checker conflicts in mapping.rs with scoped borrows
  - _Requirements: BC-03, BC-02_

- [x] 2.1 Add serde derives to SessionFixture






  - **File**: `crates/flight-simconnect/src/fixtures.rs` (or wherever SessionFixture is defined)
  - Add serde imports and derives:
    ```rust
    use serde::{Serialize, Deserialize};
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SessionFixture {
        // ... fields
    }
    ```
  - Ensure all nested types in SessionFixture also derive Serialize and Deserialize
  - Verify: `cargo check -p flight-simconnect` resolves E0277 (trait bound not satisfied) errors
  - _Requirements: BC-02.1, BC-02.2_


- [x] 2.2 Fix non-Send future in tokio::spawn


  - **File**: `crates/flight-simconnect/src/adapter.rs`
  - Take receiver ownership before spawning to avoid holding MutexGuard across await:
    ```rust
    use tokio::sync::{Mutex, mpsc};
    
    // Struct field should be:
    // event_receiver: Mutex<Option<mpsc::UnboundedReceiver<SessionEvent>>>
    
    // In spawn code:
    let mut guard = self.event_receiver.lock().await;
    let mut rx = guard
        .take()
        .expect("receiver should be initialized before spawn");
    drop(guard); // Explicitly drop guard before spawn
    
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            // handle event...
        }
    });
    ```
  - Verify: `cargo check -p flight-simconnect` resolves "future cannot be sent between threads safely" error
  - _Requirements: BC-03.3_



- [x] 2.3 Fix borrow checker conflicts in mapping.rs

  - **File**: `crates/flight-simconnect/src/mapping.rs`
  - Refactor `setup_data_definitions` to end immutable borrow before mutable calls:
    ```rust
    pub fn setup_data_definitions(
        &mut self,
        api: &SimConnectApi,
        handle: HSIMCONNECT,
        aircraft_id: AircraftId,
    ) -> Result<(), MappingError> {
        // Clone all needed data in a scoped block to end immutable borrow
        let (kin, cfg, engs, env, nav, helo) = {
            let m = self.get_aircraft_mapping(aircraft_id);
            (
                m.kinematics.clone(),
                m.config.clone(),
                m.engines.clone(),
                m.environment.clone(),
                m.navigation.clone(),
                m.helicopter.clone(),
            )
        }; // immutable borrow ends here
        
        // Now safe to call &mut self methods
        self.setup_kinematics_definition(api, handle, &kin)?;
        self.setup_config_definition(api, handle, &cfg)?;
        for e in &engs {
            self.setup_engine_definition(api, handle, e)?;
        }
        self.setup_environment_definition(api, handle, &env)?;
        self.setup_navigation_definition(api, handle, &nav)?;
        if let Some(h) = helo.as_ref() {
            self.setup_helicopter_definition(api, handle, h)?;
        }
        Ok(())
    }
    ```
  - Verify: `cargo check -p flight-simconnect` resolves E0502 (cannot borrow as mutable) errors
  - _Requirements: BC-03.4_

- [x] 3. Verify all fixes with comprehensive checks





  - Run validation commands to ensure workspace compiles
  - Verify examples package compiles independently
  - Test serde feature combinations
  - Validate cross-platform compatibility
  - _Requirements: All BC requirements verification_


- [x] 3.1 Run workspace and package-specific checks

  - Execute verification commands:
    ```bash
    # Examples compilation
    cargo check -p flight-hub-examples
    
    # SimConnect compilation
    cargo check -p flight-simconnect
    
    # Full workspace check
    cargo check --workspace
    
    # Serde feature check (if gated)
    cargo check -p flight-simconnect --features serde
    ```
  - All commands should pass without errors
  - Warnings about unused items are acceptable
  - _Requirements: BC-06.6, BC-03.7, BC-04.6_


- [x] 3.2 Add regression prevention checks

  - Add clippy lint for async lock holding:
    ```bash
    cargo clippy -p flight-simconnect -- -W clippy::await_holding_lock
    ```
  - Add grep checks for common API drift patterns:
    ```bash
    # Should return 0 hits (old API usage)
    git grep -n "write_record(" examples/
    git grep -n "BlackboxReader::new(" examples/
    git grep -n "start_recording().await" examples/
    ```
  - Document these checks in CI or verification scripts
  - _Requirements: NFR-B, NFR-C_

---

## Commit Strategy (for bisectability)

Each commit should result in a compilable state:

1. **Commit 1**: Examples Cargo.toml + async mains (task 1.1)
2. **Commit 2**: BlackboxWriter call-site updates (task 1.2)
3. **Commit 3**: BlackboxReader call-site updates (task 1.3)
4. **Commit 4**: SimConnect SessionFixture derives (task 2.1)
5. **Commit 5**: SimConnect spawn Send-safety (task 2.2)
6. **Commit 6**: SimConnect mapping borrow refactor (task 2.3)
7. **Commit 7**: Verification and regression checks (task 3.1-3.2)

## Notes

- **No public API changes** - all fixes are call-site updates or internal refactoring
- **Add dependencies, don't remove** - resolves the flight_bus/flight_ffb contradiction
- **Mechanical updates** - each change has a clear before/after pattern
- **Platform-aware** - Windows-specific code is properly gated
- These tasks build on completed Phase 1 tasks (1.1-5.6)
- Focus on bringing workspace to fully compiling state with `cargo check --workspace`
