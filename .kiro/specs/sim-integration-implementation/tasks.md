# Implementation Plan

This task list provides a phased, dependency-aware implementation plan for Flight Hub v1. Tasks are organized into phases with clear exit criteria and integrated CI quality gates.

**Implementation Philosophy:**
- **Phases build on each other**: Adapters → FFB → Runtime → Packaging → CI
- **CI gates are first-class**: Quality gates are acceptance criteria for phases, not afterthoughts
- **Test as you build**: Unit tests, property tests, and integration tests are part of each phase
- **Hardware validation matters**: Jitter and latency tests run on real hardware, not just VMs

**Current Implementation Status:**
- ✅ Core BusSnapshot structure exists (needs finalization per Phase 0)
- ✅ MSFS/X-Plane/DCS adapters exist (need completion per Phase 1)
- � FFBu engine framework exists (API + stubs, integration pending per Phase 2)
- � Reala-time scheduler exists (basic implementation, MMCSS/rtkit pending per Phase 3)
- 📋 DirectInput FFB device I/O (stubs exist, real COM calls pending)
- 📋 Packaging and distribution (not started)

**Legend:**
- ✅ Complete and tested
- 🔄 In progress (API exists, integration or real implementation pending)
- 📋 Not started
- 🚧 Blocked (waiting on dependency)

---

## Phase 0: Baseline Schema and Docs

**Goal:** Lock down canonical BusSnapshot schema and mapping docs. All adapters know exactly what they're targeting.

**Exit Criteria:**
- [ ] All three mapping docs exist and match BusSnapshot schema
- [ ] Unit tests for unit conversions pass (deg↔rad, kt↔m/s, ft↔m, fpm↔m/s)
- [ ] QG-SIM-MAPPING enabled (checks for file presence)
- [ ] QG-UNIT-CONV enabled (checks test coverage of all BusSnapshot fields)

### Task List

- [x] P0.1 Finalize BusSnapshot schema
  **Phase:** 0 - Baseline
  **Status:** Complete
  - Lock SI unit conventions (meters, radians, m/s)
  - Ensure structure matches design doc: attitude, velocities, aero, altitude_msl/agl, controls, config, valid, safe_for_ffb
  - Implement `has_nan_or_inf()` method
  - Implement `age_ms()` method
  - Implement structural `validate()` (unique engine indices, helo pedal ranges)
  - _Requirements: BUS-CORE-01.*, BUS-EXTENDED-01.*_

- [x] P0.1.1 Write unit tests for BusSnapshot
  **Phase:** 0 - Baseline
  **Status:** Complete
  - Test unit conversion utilities (deg↔rad, kt↔m/s, ft↔m, fpm↔m/s)
  - Test `has_nan_or_inf()` detection
  - Test `age_ms()` calculation
  - Test structural validation (unique indices, ranges)
  - _Requirements: BUS-CORE-01.12, BUS-CORE-01.14, BUS-EXTENDED-01.8_

- [x] P1.1 Complete MSFS SimConnect adapter
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - Connection state machine: Disconnected → Connecting → Booting → Loading → ActiveFlight ⇄ Paused → Faulted
  - Data definitions with explicit units for each SimVar
  - SimVar → BusSnapshot mapping with unit conversions (deg→rad, kt→m/s, ft→m, fpm→m/s)
  - Dispatch queue draining for burst events
  - Exponential backoff reconnection (up to 30s)
  - Aircraft change detection via TITLE SimVar
  - _Requirements: MSFS-INT-01.*_

- [x] P1.1.1 Complete MSFS Sanity Gate
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - NaN/Inf detection via `snapshot.has_nan_or_inf()`
  - Physically implausible jump detection with wrap-around for heading
  - Configurable thresholds (`max_attitude_rate_rad_per_s` from aircraft profile)
  - State machine transitions with explicit criteria
  - `safe_for_ffb` only true in ActiveFlight
  - Rate-limited logging (max once per 5s)
  - _Requirements: MSFS-INT-01.9-16_

- [x] P1.1.2 Write MSFS adapter tests
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - Unit tests for all unit conversions
  - Unit tests for state transitions
  - Unit tests for NaN/Inf detection and violation counting
  - Unit tests for implausible jump detection
  - Fixture-based integration tests with recorded telemetry
  - Test reconnection behavior
  - Test aircraft change detection
  - _Requirements: SIM-TEST-01.1, SIM-TEST-01.2, SIM-TEST-01.5, SIM-TEST-01.7, SIM-TEST-01.8_

- [x] P0.2 Create simulator mapping documentation
  **Phase:** 0 - Baseline
  **Status:** Complete
  - Create `docs/integration/msfs-simvar-mapping.md` with complete SimVar → BusSnapshot mapping
  - Create `docs/integration/xplane-data-groups.md` with DATA group → BusSnapshot mapping
  - Create `docs/integration/dcs-export-api.md` with LoGet* → BusSnapshot mapping
  - Document unit conversions, coordinate frames, and sign conventions for each
  - _Requirements: MSFS-INT-01.Doc.*, XPLANE-INT-01.Doc.*, DCS-INT-01.Doc.*_

- [ ] P0.3 Enable Phase 0 CI quality gates
  **Phase:** 0 - Baseline
  **Status:** Not started
  - Implement QG-SIM-MAPPING: Fail if any adapter lacks complete mapping table documentation
  - Implement QG-UNIT-CONV: Fail if unit conversion tests don't cover all BusSnapshot fields
  - Wire gates into CI pipeline
  - Verify gates pass
  - _Requirements: CI Quality Gates_

---

## Phase 1: Adapters and Sanity Gates

**Goal:** All three sims feed clean, sane BusSnapshots into the bus. FFB still disabled.

**Exit Criteria:**
- [ ] All adapter unit and integration tests pass
- [ ] Adapters can run in harness that logs BusSnapshots with no NaN/Inf under normal use
- [ ] Sanity violations only occur when deliberately injecting nonsense
- [ ] QG-SANITY-GATE enabled and passing (tests inject NaN/Inf and implausible jumps)

- [x] P1.2 Complete X-Plane UDP adapter
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - DATA packet parser (5-byte header + 36-byte records)
  - Data group extraction (groups 3, 4, 16, 17, 18, 21)
  - Graceful handling of missing groups
  - Data group → BusSnapshot mapping with unit conversions (deg→rad, deg/s→rad/s, kt→m/s)
  - Connection timeout detection (2s no packets → Disconnected)
  - Aircraft identity handling for UDP-only mode (limited info)
  - _Requirements: XPLANE-INT-01.*_

- [x] P1.2.1 Write X-Plane adapter tests
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - Unit tests for DATA packet parsing
  - Unit tests for missing/malformed packets
  - Unit tests for all unit conversions
  - Unit tests for connection timeout
  - _Requirements: SIM-TEST-01.3_

- [x] P1.3 Complete DCS Export.lua and adapter
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - Export.lua with proper hook chaining
  - Self-aircraft telemetry only (LoGet* functions)
  - MP integrity check compliance (`mp_detected` flag, no world objects)
  - Non-blocking UDP to localhost at 60Hz
  - Rust adapter: JSON parsing, nil handling, unit conversions
  - Connection timeout detection (2s)
  - Aircraft change detection via unit type
  - _Requirements: DCS-INT-01.*_

- [x] P1.3.1 Complete DCS installer
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - DCS variant detection (DCS, DCS.openbeta, DCS.openalpha)
  - Export.lua backup and append logic
  - FlightHubExport.lua deployment
  - Uninstaller with backup restoration
  - _Requirements: DCS-INT-01.1-3, DCS-INT-01.14_

- [x] P1.3.2 Write DCS adapter and installer tests
  **Phase:** 1 - Adapters
  **Status:** Complete (needs verification)
  - Unit tests for JSON parsing and field mapping
  - Unit tests for nil handling
  - Unit tests for MP status annotation
  - Unit tests for installer (variant detection, backup/restore)
  - _Requirements: SIM-TEST-01.4_

- [ ] P1.4 Enable Phase 1 CI quality gate
  **Phase:** 1 - Adapters
  **Status:** Not started
  - Implement QG-SANITY-GATE: Tests must inject NaN/Inf and implausible jumps, verify safe_for_ffb goes false
  - Wire gate into CI pipeline
  - Verify gate passes for all three adapters
  - _Requirements: CI Quality Gates_

- [ ] P1.5 Phase 1 Checkpoint
  **Phase:** 1 - Adapters
  **Status:** Not started
  - Run all adapter tests: `cargo test -p flight-simconnect -p flight-xplane -p flight-dcs-export`
  - Verify all tests pass
  - Run adapters in harness, verify no NaN/Inf under normal use
  - Verify sanity violations only when injecting nonsense
  - Verify QG-SANITY-GATE passes
  - **Exit criteria met before proceeding to Phase 2**

---

## Phase 2: FFB Engine, Safety, and Backends

**Goal:** FFB pipeline exists, is safe by design, and is integrated with DirectInput/XInput. Can be disabled globally while hardening runtime.

**Exit Criteria:**
- [ ] Pure-Rust tests for SafetyEnvelope and mapping logic pass
- [ ] Sim-disabled harness can feed synthetic snapshots into FFB engine
- [ ] No safety thresholds violated in tests
- [ ] Faults produce blackbox dumps and latched indicators
- [ ] QG-FFB-SAFETY enabled and passing (50ms ramp-down verified on all fault types)

- [ ] P2.1 Complete DirectInput device abstraction
  **Phase:** 2 - FFB
  **Status:** In progress (API + stubs exist, real COM calls pending)
  - RAII wrapper for IDirectInputDevice8 and IDirectInputEffect
  - Real CreateEffect calls (not stubs) for:
    - Constant force (decide: one per axis or multi-axis with direction)
    - Sine periodic
    - Spring and damper condition effects
  - Error mapping from HRESULT to local error enum
  - Device enumeration and acquisition
  - Capability querying (or load from config file per device)
  - _Requirements: FFB-HID-01.1, FFB-HID-01.9_
  - **Blocked by:** Locked test binaries (see troubleshooting)

- [ ] P2.1.1 Decide and document per-axis FFB topology
  **Phase:** 2 - FFB
  **Status:** Not started
  - Decide: one effect per axis (pitch, roll) OR one multi-axis effect with direction vector
  - Document decision in design.md
  - Implement API: `set_constant_force_pitch_nm(f32)` / `set_constant_force_roll_nm(f32)` OR `set_constant_force_xy(pitch_nm, roll_nm)`
  - _Requirements: FFB-HID-01.2_

- [ ] P2.1.2 Write DirectInput device tests
  **Phase:** 2 - FFB
  **Status:** Not started
  - Unit tests via mocks or fake DirectInput under cfg(test)
  - Integration test that acquires device and creates effects (hardware-gated, optional)
  - _Requirements: FFB-HID-01.1, FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4_

- [ ] P2.2 Wire SafetyEnvelope into FFB pipeline
  **Phase:** 2 - FFB
  **Status:** In progress (SafetyEnvelope type exists, integration pending)
  - Pipeline: raw_torque → SafetyEnvelope::apply_limits(safe_for_ffb) → DirectInput/XInput/OFP-1
  - Guarantee:
    - Torque never exceeds device max_torque_nm
    - Slew and jerk limits enforced per axis
    - When safe_for_ffb == false, ramp to zero in ≤50ms from fault detection
    - Hardware-critical faults (over-temp/current) latch, require power cycle
  - Capture `fault_initial_torque` at fault detection (not `last_setpoint`)
  - _Requirements: FFB-SAFETY-01.1-6_

- [ ] P2.2.1 Write SafetyEnvelope integration tests
  **Phase:** 2 - FFB
  **Status:** Not started
  - Pure Rust tests for clamping, slew, jerk
  - Test 50ms ramp from arbitrary starting torque (use `fault_initial_torque`)
  - Test SafeTorque mode (30% envelope) vs HighTorque (100%) vs Faulted (0%)
  - _Requirements: FFB-SAFETY-01.1-6, QG-FFB-SAFETY_

- [ ] P2.3 Complete XInput rumble backend
  **Phase:** 2 - FFB
  **Status:** In progress (module exists, mode negotiation wiring pending)
  - Implement `XInputRumbleDevice::apply_vibration(low: f32, high: f32)`
  - Side-effect free mapping from FFB synthesis → two rumble channels
  - Wire into mode negotiation:
    - DirectInput FFB when FFB device present
    - XInput rumble when only XInput available
    - Off when nothing available
  - _Requirements: FFB-HID-01.5_

- [ ] P2.3.1 Write XInput rumble tests
  **Phase:** 2 - FFB
  **Status:** Not started
  - Unit tests for mapping logic (mock XInputSetState)
  - No requirement for real controller in CI
  - _Requirements: FFB-HID-01.5_

- [ ] P2.4 Complete fault detection and blackbox
  **Phase:** 2 - FFB
  **Status:** Not started
  - Wire fault detection to:
    - USB OUT stalls (≥3 writes failing/timeouts)
    - NaNs in pipeline before SafetyEnvelope
    - Device health stream (where supported)
  - Blackbox ring buffer:
    - Capture BusSnapshot (≥250 Hz), FFB setpoints, device status
    - 2s pre-fault + 1s post-fault dump on fault
    - Bounded, rotating log (size/age-limited)
  - Emergency stop:
    - UI button and optional hardware button
    - Bypasses everything, jumps to ramp-down
  - _Requirements: FFB-SAFETY-01.5-14_

- [ ] P2.4.1 Write fault detection and blackbox tests
  **Phase:** 2 - FFB
  **Status:** Not started
  - Test USB stall detection
  - Test NaN/Inf detection in pipeline
  - Test device health monitoring
  - Test disconnect detection (within 100ms)
  - Test fault categorization (hardware-critical vs transient)
  - Test blackbox capture rate and buffering
  - Test emergency stop
  - _Requirements: FFB-SAFETY-01.5-14, QG-FFB-SAFETY_

- [ ] P2.5 Enable Phase 2 CI quality gate
  **Phase:** 2 - FFB
  **Status:** Not started
  - Implement QG-FFB-SAFETY: Verify 50ms ramp-down on all fault types
  - Wire gate into CI pipeline
  - Verify gate passes
  - _Requirements: CI Quality Gates_

- [ ] P2.6 Phase 2 Checkpoint
  **Phase:** 2 - FFB
  **Status:** Not started
  - Run all FFB tests: `cargo test -p flight-ffb`
  - Verify pure-Rust tests for SafetyEnvelope pass
  - Run sim-disabled harness with synthetic snapshots
  - Verify no safety thresholds violated
  - Verify faults produce blackbox dumps
  - Verify QG-FFB-SAFETY passes
  - **Exit criteria met before proceeding to Phase 3**

---

## Phase 3: Runtime Scheduling & Timing

**Goal:** 250 Hz axis loop and 500-1000 Hz raw torque loop behave as specified on real hardware. FFB can be enabled with high confidence.

**Exit Criteria:**
- [ ] Hardware-backed CI jobs exist and are green for 250 Hz jitter and HID latency
- [ ] Metrics around loop timing and HID latency visible
- [ ] Soak tests pass (24-48h, no missed ticks, RSS <10% growth, no blackbox drops)

- [ ] P3.1 Complete Windows RT scheduling
  **Phase:** 3 - Runtime
  **Status:** In progress (basic scheduler exists, MMCSS pending)
  - MMCSS registration (AvSetMmThreadCharacteristicsW with "Games" or "Pro Audio")
  - SetThreadPriority(THREAD_PRIORITY_TIME_CRITICAL)
  - High-res timer (CreateWaitableTimerEx + 50-80μs busy-spin)
  - Power throttling disable (PROCESS_POWER_THROTTLING_EXECUTION_SPEED)
  - HID writes via WriteFile with overlapped I/O (no HidD_SetOutputReport in hot path)
  - PowerSetRequest when active, cleared when idle
  - _Requirements: WIN-RT-01.*_

- [ ] P3.1.1 Write Windows RT tests (hardware-backed)
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Test thread priority elevation (#[cfg(windows)])
  - Test MMCSS registration
  - Test high-res timer creation and fallback
  - Test 250 Hz tick timing
  - Test PowerSetRequest behavior
  - **Requires hardware-backed CI runners**
  - _Requirements: WIN-RT-01.*, RT-TEST-01.1_

- [ ] P3.2 Complete Linux RT harness (v1-limited)
  **Phase:** 3 - Runtime
  **Status:** In progress (basic scheduler exists, rtkit pending)
  - SCHED_FIFO via pthread_setschedparam or rtkit D-Bus
  - clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME) loop
  - mlockall when RT active
  - Limits validation and warnings
  - Note: v1 only needs timing harness + input loop, not FFB output
  - _Requirements: LINUX-RT-01.*_

- [ ] P3.2.1 Write Linux RT tests (hardware-backed)
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Test SCHED_FIFO scheduling (#[cfg(target_os = "linux")])
  - Test rtkit integration
  - Test fallback to normal priority
  - Test clock_nanosleep timing
  - **Requires hardware-backed CI runners**
  - _Requirements: LINUX-RT-01.*, RT-TEST-01.2_

- [ ] P3.2.2 Create Linux RT setup helper
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Create scripts/setup-linux-rt.sh with limits.conf entries
  - Document RT priority configuration
  - _Requirements: LINUX-RT-01.10_

- [ ] P3.3 Implement jitter measurement harness
  **Phase:** 3 - Runtime
  **Status:** Not started
  - JitterMeasurement struct with tick deviation recording
  - p99 jitter calculation
  - 5s warm-up period, ≥10 minute run
  - Intel + AMD hardware runners
  - _Requirements: RT-TEST-01.3, RT-TEST-01.11_

- [ ] P3.3.1 Write jitter CI tests
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Test 250 Hz p99 ≤0.5ms on hardware-backed runners
  - Report-only mode on virtualized runners
  - Mark as #[ignore], opt-in via CI job
  - _Requirements: RT-TEST-01.3, RT-TEST-01.4, RT-TEST-01.5_

- [ ] P3.4 Implement HID latency measurement harness
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Measure end-to-end HID write latency
  - Histogram with p99 metric
  - Run only on hardware-backed runners with actual devices
  - _Requirements: RT-TEST-01.6, QG-HID-LATENCY_

- [ ] P3.4.1 Write HID latency CI test
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Assert p99 ≤300μs on hardware-backed runners
  - Skip when hardware tag not present
  - Mark as #[ignore], opt-in via CI job
  - _Requirements: RT-TEST-01.6, QG-HID-LATENCY_

- [ ] P3.5 Implement soak tests
  **Phase:** 3 - Runtime
  **Status:** Not started
  - 24-48h run with synthetic telemetry
  - Verify zero missed ticks
  - Verify RSS delta <10%
  - Verify no blackbox drops
  - Run on Intel + AMD hardware
  - _Requirements: RT-TEST-01.8, RT-TEST-01.11_

- [ ] P3.6 Enable Phase 3 CI quality gates
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Implement QG-RT-JITTER: Fail if 250Hz p99 >0.5ms on hardware, report-only on VMs
  - Implement QG-HID-LATENCY: Fail if HID write p99 >300μs on hardware, skip when unavailable
  - Wire gates into CI pipeline
  - Verify gates pass
  - _Requirements: CI Quality Gates_

- [ ] P3.7 Phase 3 Checkpoint
  **Phase:** 3 - Runtime
  **Status:** Not started
  - Hardware-backed CI jobs green for jitter and HID latency
  - Metrics visible for loop timing and HID latency
  - Soak tests pass
  - QG-RT-JITTER and QG-HID-LATENCY pass
  - **Exit criteria met before proceeding to Phase 4**

---

## Phase 4: Packaging, Installers, and Legal

**Goal:** Binaries that normal people can install/uninstall without breaking sims, with clear legal posture.

**Exit Criteria:**
- [ ] Installers tested on clean Win10/11 and mainstream Linux distro
- [ ] Uninstall leaves sims in original state
- [ ] QG-LEGAL-DOC passes (posture/docs exist and referenced)

- [ ] P4.1 Implement Windows code signing and MSI installer
  **Phase:** 4 - Packaging
  **Status:** Not started
  - Code signing infrastructure (scripts/sign-binaries.ps1)
  - Sign all EXE and DLL files
  - WiX configuration for MSI
  - Per-user core binaries, opt-in sim integrations
  - Elevated privileges for sim integrations
  - Product posture and EULA display
  - MSI signing
  - _Requirements: PKG-01.1-8_

- [ ] P4.2 Implement Windows uninstaller
  **Phase:** 4 - Packaging
  **Status:** Not started
  - Binary removal
  - X-Plane plugin removal (if installed)
  - DCS Export.lua restoration (if backed up)
  - _Requirements: PKG-01.9_

- [ ] P4.3 Implement Linux packages
  **Phase:** 4 - Packaging
  **Status:** Not started
  - .deb package with udev rules
  - postinst script for group management
  - Optional: AppImage or .rpm
  - Note: v1 Linux is timing harness + input loop only, no FFB output
  - _Requirements: PKG-01.10-12_

- [ ] P4.4 Create legal and compliance documentation
  **Phase:** 4 - Packaging
  **Status:** Not started
  - Product posture document (docs/product-posture.md)
  - "What We Touch" docs per sim
  - Third-party components inventory
  - Ship license texts
  - Reference posture in README, installer, integration modules
  - _Requirements: LEGAL-01.*, PKG-01.13_

- [ ] P4.5 Enable Phase 4 CI quality gate
  **Phase:** 4 - Packaging
  **Status:** Not started
  - Implement QG-LEGAL-DOC: Fail if posture doc missing or not referenced
  - Wire gate into CI pipeline
  - Verify gate passes
  - _Requirements: CI Quality Gates_

- [ ] P4.6 Phase 4 Checkpoint
  **Phase:** 4 - Packaging
  **Status:** Not started
  - Installers tested on clean Win10/11 and Linux distro
  - Uninstall leaves sims in original state
  - QG-LEGAL-DOC passes
  - **Exit criteria met before proceeding to Phase 5**

---

## Phase 5: Metrics, CI Gates, and Ship It

**Goal:** Observability in place, CI enforces all guarantees, candidate release passes all gates.

**Exit Criteria:**
- [ ] All CI quality gates enabled and passing
- [ ] Metrics visible for any given run
- [ ] Candidate release build passes all gates
- [ ] User documentation complete

- [ ] P5.1 Implement metrics system
  **Phase:** 5 - CI & Metrics
  **Status:** Not started
  - Hierarchical naming (sim.*, ffb.*, runtime.*, bus.*)
  - Metric types (gauges, counters, histograms)
  - Optional Prometheus exporter (:9090/metrics)
  - In-process ring buffer for UI (60s)
  - Log-structured snapshots (JSON lines)
  - _Requirements: Design: Telemetry and Metrics_

- [ ] P5.1.1 Write metrics system tests
  **Phase:** 5 - CI & Metrics
  **Status:** Not started
  - Test naming and scoping
  - Test metric types
  - Test Prometheus export format
  - Test ring buffer retention
  - _Requirements: LINUX-RT-01.8, RT-TEST-01.3_

- [ ] P5.2 Implement cargo xtask validation commands
  **Phase:** 5 - CI & Metrics
  **Status:** Not started
  - cargo xtask validate-msfs-telemetry
  - cargo xtask validate-xplane-telemetry
  - cargo xtask validate-dcs-export
  - _Requirements: SIM-TEST-01.9_

- [ ] P5.3 Create comprehensive integration test suite
  **Phase:** 5 - CI & Metrics
  **Status:** Not started
  - Fixture files for each sim (tests/fixtures/)
  - Replay testing with recorded telemetry
  - Complete adapter lifecycle tests
  - Reconnection with exponential backoff
  - Sanity gate with NaN/Inf injection
  - _Requirements: SIM-TEST-01.5-8_

- [ ] P5.4 Create user documentation
  **Phase:** 5 - CI & Metrics
  **Status:** Not started
  - Installation guides (Windows MSI, Linux .deb/AppImage)
  - Simulator setup guides (MSFS, X-Plane, DCS)
  - FFB device configuration guide
  - Troubleshooting guide
  - _Requirements: General documentation_

- [ ] P5.5 Final validation and release preparation
  **Phase:** 5 - CI & Metrics
  **Status:** Not started
  - Run all tests and quality gates
  - Verify documentation complete
  - Verify code signing works
  - Verify installers on clean systems
  - Verify uninstallers clean up properly
  - Create release notes
  - **Ship v1!**

---

## Troubleshooting

### Locked Test Binaries (LNK1104)

**Problem:** Tests fail to run with "LNK1104: cannot open file" because previous test run left binaries locked.

**Solution:**
1. Kill all cargo/test processes: `taskkill /F /IM cargo.exe /IM *.exe`
2. Delete target/debug/deps/*.pdb files
3. Re-run tests

**Prevention:** Add to CONTRIBUTING.md so contributors know the workaround.

### Hardware-Backed CI Runners

**Problem:** Jitter and HID latency tests require real hardware, not VMs.

**Solution:**
- Tag CI runners with `hardware: true`
- Gate tests with `#[cfg_attr(not(feature = "hardware"), ignore)]`
- Run in report-only mode on VMs (don't fail builds)

### DirectInput COM Calls

**Problem:** DirectInput tests may fail without real FFB device attached.

**Solution:**
- Use mocks or fake DirectInput under `cfg(test)`
- Mark hardware integration tests as `#[ignore]`, opt-in via CI job
- Document device requirements in test comments
