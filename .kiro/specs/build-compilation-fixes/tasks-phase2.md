# Implementation Plan - Phase 2: Final Workspace Compilation Fixes

This task list addresses the remaining compilation errors blocking `cargo check --workspace` success.

## Background

After completing Phase 1 (tasks 1.1-5.6), the workspace still has compilation failures in:
- **flight-hub-examples** (examples package) - API drift, missing dependencies, heavy outdated API usage
- **flight-simconnect-sys** - clippy FFI lint errors

## Strategy Overview

Following the minimal, no-public-API fix approach:

1. **Feature-isolate examples** - Gate failing demos behind declared features with fallback mains
2. **Surgical example fixes** - Port high-value demos to new APIs (optional but low-effort)
3. **Clippy FFI suppression** - Allow FFI-specific lints in sys crate per BC-10

This gets the workspace green immediately while allowing selective demo re-enablement.

---

## Tasks

- [x] 1. Feature-isolate examples package (Step 1 - Makes workspace green)




  - Declare features in examples/Cargo.toml to silence "unexpected cfg" warnings
  - Make external deps optional and map features to deps
  - Gate failing examples with fallback mains
  - _Requirements: BC-06_

- [x] 1.1 Update examples/Cargo.toml with feature declarations





  - **File**: `examples/Cargo.toml`
  - Add features section:
    ```toml
    [features]
    default = []
    flight-service    = ["dep:flight-service"]
    flight-simconnect = ["dep:flight-simconnect"]
    flight-panels     = ["dep:flight-panels"]
    flight-streamdeck = ["dep:flight-streamdeck"]
    panels            = ["flight-panels"]
    simconnect        = ["flight-simconnect"]
    streamdeck        = ["flight-streamdeck"]
    windows           = []
    integration       = []
    axis              = []  # legacy gate for axis_demo.rs
    replay            = []  # legacy gate for replay_demo.rs
    ```
  - Add base dependencies (always available):
    ```toml
    anyhow = "1"
    tokio = { version = "1", features = ["rt-multi-thread","macros","time","sync"] }
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["fmt","env-filter"] }
    bincode = "1"
    flight-core = { path = "../crates/flight-core" }
    flight-bus = { path = "../crates/flight-bus" }
    flight-ffb = { path = "../crates/flight-ffb" }
    ```
  - Make failing deps optional:
    ```toml
    flight-replay     = { path = "../crates/flight-replay", optional = true }
    flight-service    = { path = "../crates/flight-service", optional = true }
    flight-simconnect = { path = "../crates/flight-simconnect", optional = true }
    flight-streamdeck = { path = "../crates/flight-streamdeck", optional = true }
    flight-panels     = { path = "../crates/flight-panels", optional = true }
    flight-hid        = { path = "../crates/flight-hid", optional = true }
    tempfile          = { version = "3", optional = true }
    ```
  - Verify: Features declared match cfg gates used in example files
  - _Requirements: BC-06.1_

- [x] 1.2 Gate capability_demo.rs with fallback main





  - **File**: `examples/capability_demo.rs`
  - Add at top of file:
    ```rust
    #![cfg_attr(not(feature = "flight-service"), allow(dead_code, unused_imports))]
    
    #[cfg(feature = "flight-service")]
    #[tokio::main(flavor = "current_thread")]
    async fn main() -> anyhow::Result<()> {
        // existing demo code
        Ok(())
    }
    
    #[cfg(not(feature = "flight-service"))]
    fn main() {
        eprintln!("Enable `--features flight-service` to build this example.");
    }
    ```
  - Verify: `cargo check -p examples` passes (example compiles with fallback)
  - Verify: `cargo check -p examples --features flight-service` shows missing dep error (expected until deps added)
  - _Requirements: BC-06.1_

- [x] 1.3 Gate simconnect_usage_demo.rs with fallback main





  - **File**: `examples/simconnect_usage_demo.rs`
  - Add at top of file:
    ```rust
    #![cfg_attr(not(feature = "simconnect"), allow(dead_code, unused_imports))]
    
    #[cfg(feature = "simconnect")]
    #[tokio::main(flavor = "current_thread")]
    async fn main() -> anyhow::Result<()> {
        // existing demo code
        Ok(())
    }
    
    #[cfg(not(feature = "simconnect"))]
    fn main() {
        eprintln!("Enable `--features simconnect` to build this example.");
    }
    ```
  - Verify: `cargo check -p examples` passes
  - _Requirements: BC-06.1_

- [x] 1.4 Gate streamdeck_panel_demo.rs with fallback main





  - **File**: `examples/streamdeck_panel_demo.rs`
  - Add at top of file:
    ```rust
    #![cfg_attr(not(all(feature = "streamdeck", feature = "panels")), allow(dead_code, unused_imports))]
    
    #[cfg(all(feature = "streamdeck", feature = "panels"))]
    #[tokio::main(flavor = "current_thread")]
    async fn main() -> anyhow::Result<()> {
        // existing demo code
        Ok(())
    }
    
    #[cfg(not(all(feature = "streamdeck", feature = "panels")))]
    fn main() {
        eprintln!("Enable `--features streamdeck,panels` to build this example.");
    }
    ```
  - Verify: `cargo check -p examples` passes
  - _Requirements: BC-06.1_

- [x] 1.5 Gate watchdog_integration_demo.rs with fallback main





  - **File**: `examples/watchdog_integration_demo.rs`
  - Add at top of file:
    ```rust
    #![cfg_attr(not(feature = "flight-hid"), allow(dead_code, unused_imports))]
    
    #[cfg(feature = "flight-hid")]
    #[tokio::main(flavor = "current_thread")]
    async fn main() -> anyhow::Result<()> {
        // existing demo code
        Ok(())
    }
    
    #[cfg(not(feature = "flight-hid"))]
    fn main() {
        eprintln!("Enable `--features flight-hid` to build this example.");
    }
    ```
  - Verify: `cargo check -p examples` passes
  - _Requirements: BC-06.1_

- [x] 1.6 Gate pipeline_compilation_demo.rs with fallback main


  - **File**: `examples/pipeline_compilation_demo.rs`
  - Add at top of file:
    ```rust
    #![cfg_attr(not(feature = "integration"), allow(dead_code, unused_imports))]
    
    #[cfg(feature = "integration")]
    #[tokio::main(flavor = "current_thread")]
    async fn main() -> anyhow::Result<()> {
        // existing demo code
        Ok(())
    }
    
    #[cfg(not(feature = "integration"))]
    fn main() {
        eprintln!("Enable `--features integration` to build this example (requires API porting).");
    }
    ```
  - Note: This demo has heavy API drift (old PipelineBuilder API), gate until ported
  - Verify: `cargo check -p examples` passes
  - _Requirements: BC-06.1_


- [x] 1.7 Gate capture_replay_demo.rs with fallback main

  - **File**: `examples/capture_replay_demo.rs`
  - Add at top of file:
    ```rust
    #![cfg_attr(not(feature = "replay"), allow(dead_code, unused_imports))]
    
    #[cfg(feature = "replay")]
    #[tokio::main(flavor = "current_thread")]
    async fn main() -> anyhow::Result<()> {
        // existing demo code
        Ok(())
    }
    
    #[cfg(not(feature = "replay"))]
    fn main() {
        eprintln!("Enable `--features replay` to build this example (requires API porting).");
    }
    ```
  - Note: This demo uses old ReplayEngine/ReplayFrame/ReplayStats API, gate until ported
  - Verify: `cargo check -p examples` passes
  - _Requirements: BC-06.1_


- [x] 1.8 Verify workspace compiles with gated examples

  - Run: `cargo check --workspace`
  - Should pass with all examples compiling to fallback mains
  - Run: `cargo check -p examples --all-features`
  - Will show errors for gated examples (expected - they need API porting)
  - Verify: No "unexpected cfg" warnings
  - _Requirements: BC-06.6_

- [x] 2. Suppress clippy FFI lints in sys crate (Step 3)




  - Fix clippy errors in flight-simconnect-sys
  - _Requirements: BC-10_

- [x] 2.1 Add FFI lint suppressions to flight-simconnect-sys

  - **File**: `crates/flight-simconnect-sys/src/lib.rs`
  - Add at crate root (top of file):
    ```rust
    #![allow(
        clippy::not_unsafe_ptr_arg_deref,
        clippy::missing_transmute_annotations
    )]
    ```
  - These lints are typical for FFI thunks calling function pointers with raw handles
  - Suppression is appropriate for -sys crates per BC-10
  - Verify: `cargo clippy -p flight-simconnect-sys` passes without FFI lint errors
  - _Requirements: BC-10.1, BC-10.2, BC-10.6_

- [x] 3. Optional: Surgical fixes for high-value examples (Step 2)





  - Port specific examples to new APIs for demonstration purposes
  - These are optional - workspace is green after Step 1
  - _Requirements: BC-06_

- [x] 3.1 Fix streamdeck_panel_demo.rs helper signatures (optional)


  - **File**: `examples/streamdeck_panel_demo.rs`
  - Change helper function signatures from `Result<_, Box<dyn Error>>` to `anyhow::Result<()>`
  - Add missing import: `use flight_bus::AutopilotState;`
  - This fixes ? conversion errors with anyhow::Result
  - Verify: `cargo check -p examples --features streamdeck,panels` passes
  - _Requirements: BC-06.2_

- [x] 3.2 Fix simconnect_usage_demo.rs API updates (optional)


  - **File**: `examples/simconnect_usage_demo.rs`
  - Update method calls:
    - `as_knots()` → `to_knots()`
    - `as_degrees()` → `to_degrees()`
    - Replace `as_percentage()` with new API (often raw f32 or `as_f32()` * 100)
  - Update AutoSwitchConfig construction with available fields:
    - `max_switch_time`, `profile_paths`, `enable_pof`, `pof_hysteresis`, `capability_context`
  - Update DetectedAircraft construction: `{ sim, aircraft_id, process_name }`
  - Add `.await` to metrics access (it's a Future)
  - Fix type literals: `Some(3500.0f32)` instead of bare `3500.0`
  - Verify: `cargo check -p examples --features simconnect` passes
  - _Requirements: BC-06.2_

- [x] 3.3 Fix telemetry_synth_demo.rs borrow checker (optional)


  - **File**: `examples/telemetry_synth_demo.rs`
  - Fix borrow conflict by scoping the first mutable borrow:
    ```rust
    // Before (holds synth_engine mutably while calling ffb_engine method):
    // let synth = ffb_engine.get_telemetry_synth_mut().unwrap();
    // synth.tuning_mut().set_global_intensity(1.5);
    // let reduced = ffb_engine.update_telemetry_synthesis(&snapshot)?;
    
    // After (scope the first borrow):
    if let Some(synth) = ffb_engine.get_telemetry_synth_mut() {
        synth.tuning_mut().set_global_intensity(1.5);
    } // borrow ends here
    let reduced = ffb_engine.update_telemetry_synthesis(&snapshot)?;
    ```
  - Verify: `cargo check -p examples` passes
  - _Requirements: BC-06.2_

- [ ] 4. Verify final workspace state
  - Run comprehensive checks to ensure green build
  - _Requirements: All BC requirements verification_

- [ ] 4.1 Run core verification commands
  - Execute:
    ```bash
    # Primary goal - workspace compiles
    cargo check --workspace
    
    # Sys crate clippy passes
    cargo clippy -p flight-simconnect-sys
    
    # Examples compile with fallback mains
    cargo check -p examples
    
    # Specific feature checks (optional demos)
    cargo check -p examples --features simconnect
    cargo check -p examples --features streamdeck,panels
    ```
  - All commands should pass without errors
  - Gated examples show helpful "enable --features" messages
  - _Requirements: BC-06.6, BC-10.6_

- [ ] 4.2 Verify cross-platform compatibility
  - Run on both Windows and Linux CI:
    ```bash
    cargo check --workspace
    ```
  - Ensure no platform-specific compilation errors
  - Windows-only examples properly gated
  - _Requirements: BC-04.6, NFR-C_

- [ ] 4.3 Document example feature usage
  - Add comment to examples/Cargo.toml explaining feature system:
    ```toml
    # Examples are feature-gated to avoid pulling in heavyweight dependencies by default.
    # To build a specific example:
    #   cargo run -p examples --example <name> --features <feature>
    # 
    # Available features:
    #   simconnect        - SimConnect integration examples
    #   streamdeck,panels - StreamDeck panel examples
    #   flight-hid        - HID device examples
    #   integration       - Multi-crate integration examples (may need API porting)
    #   replay            - Replay system examples (may need API porting)
    ```
  - _Requirements: BC-06.7_

---

## Summary

This phase achieves a green `cargo check --workspace` through:

1. **Feature isolation** (tasks 1.1-1.8) - Gate failing examples behind features with fallback mains
2. **Clippy fixes** (task 2.1) - Suppress appropriate FFI lints in sys crate
3. **Optional porting** (tasks 3.1-3.3) - Selective API updates for high-value demos
4. **Verification** (tasks 4.1-4.3) - Comprehensive checks and documentation

## Key Benefits

- **Workspace compiles by default** - No heavyweight features pulled in
- **No public API changes** - All fixes are internal or call-site updates
- **Selective enablement** - Demos can be re-enabled with `--features` as needed
- **Clear migration path** - Gated demos show what needs porting
- **Bisectable** - Each task results in a compilable state

## Notes

- Pattern already used in axis_demo.rs and replay_demo.rs - extending to all failing examples
- Feature declarations silence "unexpected cfg" warnings
- Fallback mains provide helpful error messages
- Optional surgical fixes (Step 2) can be done incrementally
- Focus is on getting workspace green, not porting all examples immediately
