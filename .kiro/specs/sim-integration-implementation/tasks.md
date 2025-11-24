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
- [x] All three mapping docs exist and match BusSnapshot schema





- [x] Unit tests for unit conversions pass (deg↔rad, kt↔m/s, ft↔m, fpm↔m/s)




- [x] QG-SIM-MAPPING enabled (checks for file presence)






- [-] QG-UNIT-CONV enabled (checks test coverage of all BusSnapshot fields)





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

- [x] P0.3 Enable Phase 0 CI quality gates








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
- [x] All adapter unit and integration tests pass


- [x] Adapters can run in harness that logs BusSnapshots with no NaN/Inf under normal use


- [x] Sanity violations only occur when deliberately injecting nonsense




- [x] QG-SANITY-GATE enabled and passing (tests inject NaN/Inf and implausible jumps)





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

- [x] P1.4 Enable Phase 1 CI quality gate




  **Phase:** 1 - Adapters
  **Status:** Not started
  - Implement QG-SANITY-GATE: Tests must inject NaN/Inf and implausible jumps, verify safe_for_ffb goes false
  - Wire gate into CI pipeline
  - Verify gate passes for all three adapters
  - _Requirements: CI Quality Gates_

- [x] P1.5 Phase 1 Checkpoint




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

## Phase 3 – FFB safety, faults, and observability

**Goal:** FFB fault detection, blackbox recording, and emergency stop are fully implemented and tested.

**Exit Criteria:**
- [ ] All FFB safety tests passing (fault detection, 50 ms ramp, estop)
- [ ] Blackbox recorder exercised in tests and visible in logs
- [ ] QG-FFB-SAFETY gate wired in CI

- [ ] 19. Implement FFB fault detection and handling (FFB-SAFETY-02, FFB-SAFETY-03)
  - [ ] 19.1 Add `FaultReason` enum and fault state machine to `flight-ffb`
  - [ ] 19.2 Detect USB OUT stalls (N consecutive failed writes → `UsbStall`)
  - [ ] 19.3 Detect NaN/Inf in FFB pipeline inputs → `NaNInPipeline`
  - [ ] 19.4 Integrate device health (over-temp/over-current) where available → hardware-critical faults
  - [ ] 19.5 Detect device disconnects from HID/DirectInput return codes
  - [ ] 19.6 Implement `clear_fault()` semantics (transient vs hardware-critical)
  - [ ]* 19.7 Unit tests for all fault paths
  - _Requirements: FFB-SAFETY-02, FFB-SAFETY-03_

- [ ] 20. Implement FFB blackbox recorder (FFB-BLACKBOX-01)
  - [ ] 20.1 Add `BlackboxSample` and `BlackboxRecorder` ring buffer (≥3 s window @ ≥250 Hz)
  - [ ] 20.2 Wire recorder into FFB loop (pre- and post-fault sampling)
  - [ ] 20.3 Implement export of 3 s window (2 s pre + 1 s post) to compressed file
  - [ ] 20.4 Add log rotation (max N files or total size) under `logs/blackbox`
  - [ ]* 20.5 Unit tests for ring buffer, export window, and rotation
  - _Requirements: FFB-BLACKBOX-01_

- [ ] 21. Implement FFB emergency stop (FFB-SAFETY-04)
  - [ ] 21.1 Add `FfbController::emergency_stop()` / `clear_emergency_stop()` APIs
  - [ ] 21.2 Wire emergency stop into safety state machine (`FaultReason::UserEStop`)
  - [ ] 21.3 Reuse 50 ms ramp-to-zero path for estop starting from current torque
  - [ ] 21.4 Bind estop to UI action (big red button)
  - [ ]* 21.5 (Optional) Bind hardware E-stop input if available via HID
  - _Requirements: FFB-SAFETY-04_

- [ ] 22. Checkpoint – FFB safety & observability
  - [ ] 22.1 All FFB safety tests passing (fault detection, 50 ms ramp, estop)
  - [ ] 22.2 Blackbox recorder exercised in tests and visible in logs
  - [ ] 22.3 Wire `QG-FFB-SAFETY` gate in CI (unit tests + basic blackbox check)

---

## Phase 4 – Runtime scheduling and timing

**Goal:** Windows and Linux real-time paths complete with jitter measurement and HID latency validation.

**Exit Criteria:**
- [ ] Windows RT + timers + power integration verified
- [ ] Linux RT + timers + metrics verified
- [ ] Jitter tests green on hardware matrix
- [ ] HID latency test green on HID runner
- [ ] QG-RT-JITTER and QG-HID-LATENCY wired into CI

### Windows real-time path

- [ ] 23. Complete Windows real-time thread configuration (RT-WIN-01)
  - [ ] 23.1 Implement `WindowsRtThread` abstraction:
    - [ ] 23.1.1 MMCSS registration via `AvSetMmThreadCharacteristicsW` ("Games" or "Pro Audio")
    - [ ] 23.1.2 Elevate thread priority (`SetThreadPriority`)
    - [ ] 23.1.3 Disable process power throttling (`SetProcessInformation`)
    - [ ] 23.1.4 RAII for registration / restoration on drop
  - [ ]* 23.2 Platform-specific integration tests:
    - [ ]* 23.2.1 Verify non-zero MMCSS handle
    - [ ]* 23.2.2 Verify priority change via `GetThreadPriority`
  - _Requirements: RT-WIN-01_

- [ ] 24. Complete Windows high-resolution timer loop (RT-WIN-02)
  - [ ] 24.1 Implement 250 Hz loop using high-resolution waitable timers:
    - [ ] 24.1.1 `CreateWaitableTimerExW` with high-res flag (where supported)
    - [ ] 24.1.2 Fallback to `timeBeginPeriod(1)` + standard waitable timer
    - [ ] 24.1.3 Busy-spin final ~50–80 µs based on QPC
  - [ ]* 24.2 Platform-specific timer tests:
    - [ ]* 24.2.1 No-op tick harness measuring intervals via QPC
    - [ ]* 24.2.2 Compute p99 jitter and export as metrics (feeds QG-RT-JITTER)
  - _Requirements: RT-WIN-02_

- [ ] 25. Implement Windows power management integration (RT-WIN-03)
  - [ ] 25.1 Implement `PowerManager` wrapper:
    - [ ] 25.1.1 Use `PowerCreateRequest` + `PowerSetRequest` for EXECUTION/SYSTEM_REQUIRED
    - [ ] 25.1.2 Tie lifetime to "active sim + active FFB device" state
    - [ ] 25.1.3 Clear requests when idle
  - [ ]* 25.2 Platform-specific tests:
    - [ ]* 25.2.1 Harness asserts request handles exist when active and are cleared when idle
  - _Requirements: RT-WIN-03_

- [ ] 26. Implement Windows HID write optimization and latency harness (RT-WIN-04)
  - [ ] 26.1 Optimize HID writes:
    - [ ] 26.1.1 Open devices with `FILE_FLAG_OVERLAPPED`
    - [ ] 26.1.2 Implement async `WriteFile` using an OVERLAPPED struct pool
    - [ ] 26.1.3 Avoid `HidD_SetOutputReport` in FFB hot path
  - [ ] 26.2 Implement HID latency measurement harness:
    - [ ] 26.2.1 `flight-hid-bench` tool sends synthetic OUT reports at 1 kHz
    - [ ] 26.2.2 Measure submit → completion latency via QPC; build histogram
  - [ ]* 26.3 Add HID latency CI test:
    - [ ]* 26.3.1 Hardware-tagged job runs harness; asserts p99 ≤ configured budget
    - [ ]* 26.3.2 Non-tagged jobs skip gracefully but still report metrics
  - _Requirements: RT-WIN-04_

### Linux real-time path

- [ ] 27. Complete Linux real-time thread configuration (RT-LINUX-01)
  - [ ] 27.1 Implement `LinuxRtThread` abstraction:
    - [ ] 27.1.1 Prefer rtkit DBus (`MakeThreadRealtime(u64, u32)`)
    - [ ] 27.1.2 Fallback to `sched_setscheduler` with `SCHED_FIFO`
    - [ ] 27.1.3 Call `mlockall(MCL_CURRENT|MCL_FUTURE)` on success
    - [ ] 27.1.4 Validate `RLIMIT_RTPRIO` and `RLIMIT_MEMLOCK`; emit warnings/metrics
  - [ ]* 27.2 Platform-specific tests:
    - [ ]* 27.2.1 Test under allowed RT limits (policy/priority as expected)
    - [ ]* 27.2.2 Test without RT privileges (correct fallback + warning)
  - _Requirements: RT-LINUX-01_

- [ ] 28. Implement Linux high-resolution timer loop (RT-LINUX-02)
  - [ ] 28.1 Implement 250 Hz loop via `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, ...)`
  - [ ] 28.2 Busy-spin final ~50 µs with `clock_gettime`
  - [ ]* 28.3 Platform-specific timer tests:
    - [ ]* 28.3.1 No-op tick harness measuring intervals; compute jitter
  - _Requirements: RT-LINUX-02_

- [ ] 29. Implement Linux RT metrics exposure (RT-LINUX-03)
  - [ ] 29.1 Export metrics:
    - [ ] 29.1.1 `runtime.linux.rt_enabled`
    - [ ] 29.1.2 `runtime.linux.sched_policy`, `runtime.linux.priority`
    - [ ] 29.1.3 `runtime.linux.mlockall_success`
  - [ ] 29.2 Hook into central metrics system (Task 41)
  - _Requirements: RT-LINUX-03_

- [ ] 30. Create Linux RT setup helper script (RT-LINUX-04)
  - [ ] 30.1 `scripts/setup-linux-rt.sh`:
    - [ ] 30.1.1 Configure `/etc/security/limits.conf` for `rtprio`/`memlock`
    - [ ] 30.1.2 Print instructions for logout/login and group membership
  - [ ] 30.2 Document script usage in Linux install docs
  - _Requirements: RT-LINUX-04_

### Cross-platform jitter & checkpoints

- [ ] 31. Implement runtime jitter measurement (RT-JITTER-01)
  - [ ] 31.1 Implement `JitterMeasurement` helper:
    - [ ] 31.1.1 Accept target Hz; record deviation vs ideal period (skip warmup)
    - [ ] 31.1.2 Compute p50/p95/p99; export as metrics
  - [ ]* 31.2 Long-running jitter tests:
    - [ ]* 31.2.1 10-minute synthetic loop @ 250 Hz on hardware runners
    - [ ]* 31.2.2 Assert p99 ≤ configured budget on "realtime-capable" runners
    - [ ]* 31.2.3 Report-only mode on virtualized runners
  - [ ]* 31.3 Add hardware matrix coverage:
    - [ ]* 31.3.1 `windows-intel`, `windows-amd`, `linux-intel`, `linux-amd` jobs
  - _Requirements: RT-JITTER-01_

- [ ] 32. Checkpoint – runtime & timing
  - [ ] 32.1 Windows RT + timers + power integration verified
  - [ ] 32.2 Linux RT + timers + metrics verified
  - [ ] 32.3 Jitter tests green on hardware matrix
  - [ ] 32.4 HID latency test green on HID runner
  - [ ] 32.5 `QG-RT-JITTER` and `QG-HID-LATENCY` wired into CI

---

## Phase 5 – Packaging and distribution

**Goal:** Installers and packages for Windows and Linux that can be deployed to end users.

**Exit Criteria:**
- [ ] Installers tested on clean Win10/11 and at least one Linux distro
- [ ] Uninstallers restore sims to original state
- [ ] All artifacts signed and include license inventory

- [ ] 33. Implement Windows code signing infrastructure (PKG-WIN-01)
  - [ ] 33.1 Integrate `signtool` into CI; sign all EXE/DLL/MSI artifacts
  - [ ] 33.2 Fail "release" jobs if any artifact is unsigned
  - _Requirements: PKG-WIN-01_

- [ ] 34. Implement Windows MSI installer (PKG-WIN-02)
  - [ ] 34.1 Create WiX project:
    - [ ] 34.1.1 Features: core, sim integrations (MSFS/X-Plane/DCS)
    - [ ] 34.1.2 Correct install scope (per-user vs per-machine) by feature
  - [ ] 34.2 Custom actions:
    - [ ] 34.2.1 Display product posture summary / EULA excerpt
    - [ ] 34.2.2 Write sim config files in a reversible way
  - _Requirements: PKG-WIN-02_

- [ ] 35. Implement Windows uninstaller (PKG-WIN-03)
  - [ ] 35.1 Remove all installed binaries
  - [ ] 35.2 Restore `Export.lua` from `.flighthub_backup` if present
  - [ ] 35.3 Remove X-Plane plugin(s) and any integration stubs
  - [ ] 35.4 Leave sims in original state if possible
  - _Requirements: PKG-WIN-03_

- [ ] 36. Implement Linux package formats (PKG-LINUX-01)
  - [ ] 36.1 Build `.deb`:
    - [ ] 36.1.1 Ship binaries in `/usr/bin`
    - [ ] 36.1.2 Include udev rules for `/dev/hidraw*`
  - [ ]* 36.2 Optionally build AppImage or `.rpm` for other distros
  - [ ] 36.3 Postinst script:
    - [ ] 36.3.1 Add user to relevant groups
    - [ ] 36.3.2 Reload udev rules
  - _Requirements: PKG-LINUX-01_

- [ ] 37. Create Linux installation documentation (PKG-LINUX-02)
  - [ ] 37.1 Document package installation
  - [ ] 37.2 Document RT setup script usage (Task 30)
  - [ ] 37.3 Document group membership and log locations
  - _Requirements: PKG-LINUX-02_

- [ ] 38. Create third-party components inventory (PKG-LICENSE-01)
  - [ ] 38.1 Generate `third-party-components.toml` from dependencies
  - [ ] 38.2 Collect and ship licenses for all redistributed components
  - [ ] 38.3 Link inventory from installer and docs
  - _Requirements: PKG-LICENSE-01_

---

## Phase 6 – Legal posture, metrics, CI, and docs

**Goal:** Complete observability, legal documentation, CI quality gates, and user documentation.

**Exit Criteria:**
- [ ] All QG-* checks have CI jobs and are passing
- [ ] Product posture and "What We Touch" docs exist
- [ ] User documentation complete
- [ ] Full test matrix green

- [ ] 39. Create product posture document (LEGAL-01)
  - [ ] 39.1 Write `docs/product-posture.md` (accessory posture, not certified training)
  - [ ] 39.2 Include export-control/EULA reminders from sim vendors
  - [ ] 39.3 Link from README and installer
  - _Requirements: LEGAL-01_

- [ ] 40. Create "What We Touch" documentation for each simulator (LEGAL-02)
  - [ ] 40.1 For MSFS: files, APIs, SimVars, ports
  - [ ] 40.2 For X-Plane: plugins, datarefs, UDP ports
  - [ ] 40.3 For DCS: `Export.lua`, data, ports
  - [ ] 40.4 Document how to revert all changes
  - _Requirements: LEGAL-02_

- [ ] 41. Implement telemetry and metrics system (METRICS-01)
  - [ ] 41.1 Implement metrics core (counters, gauges, histograms)
  - [ ] 41.2 Define namespaces: `sim.*`, `ffb.*`, `runtime.*`, `bus.*`
  - [ ] 41.3 Implement in-process exporter (for UI)
  - [ ]* 41.4 Optional Prometheus exporter
  - [ ]* 41.5 Unit tests for metrics creation, updates, and export
  - _Requirements: METRICS-01_

- [ ] 42. Implement `cargo xtask` validation commands (CI-TOOLS-01)
  - [ ] 42.1 `cargo xtask validate-msfs-telemetry`
  - [ ] 42.2 `cargo xtask validate-xplane-telemetry`
  - [ ] 42.3 `cargo xtask validate-dcs-export`
  - [ ] 42.4 Each xtask replays fixtures and checks mapping + sanity behaviour
  - _Requirements: CI-TOOLS-01_

- [ ] 43. Checkpoint – all tests and quality gates passing before release (CI-GATES-01)
  - [ ] 43.1 Ensure all QG-* checks have CI jobs:
    - [ ] QG-SIM-MAPPING
    - [ ] QG-UNIT-CONV
    - [ ] QG-SANITY-GATE
    - [ ] QG-FFB-SAFETY
    - [ ] QG-RT-JITTER
    - [ ] QG-HID-LATENCY
    - [ ] QG-LEGAL-DOC
  - _Requirements: CI-GATES-01_

- [ ] 44. Create CI quality gate enforcement (CI-GATES-02)
  - [ ] 44.1 Wire QG-* jobs as required checks on main/release branches
  - [ ] 44.2 Document gates in `CONTRIBUTING.md`
  - _Requirements: CI-GATES-02_

- [ ] 45. Create comprehensive integration test suite (TEST-INTEG-01)
  - [ ] 45.1 Fixture-based tests for each sim (connect → stream → disconnect → reconnect)
  - [ ] 45.2 End-to-end test: sim fixture → bus → FFB → safety → no faults under normal conditions
  - _Requirements: TEST-INTEG-01_

- [ ]* 46. Implement soak tests (TEST-INTEG-02)
  - [ ]* 46.1 24–48h synthetic telemetry + FFB loop on hardware
  - [ ]* 46.2 Assert no missed ticks beyond threshold; RSS stable; blackbox present on any faults
  - _Requirements: TEST-INTEG-02_

- [ ] 47. Create user documentation (DOCS-USER-01)
  - [ ] 47.1 Install guides (Windows, Linux)
  - [ ] 47.2 Per-sim setup guides
  - [ ] 47.3 FFB device configuration and safety guidelines
  - [ ] 47.4 Troubleshooting (RT not enabled, no FFB, perms, etc.)
  - _Requirements: DOCS-USER-01_

- [ ] 48. Final validation and release preparation (RELEASE-01)
  - [ ] 48.1 Run full test matrix (unit, integration, RT, HID, soak)
  - [ ] 48.2 Verify installers on clean Win10/11 and at least one Linux distro
  - [ ] 48.3 Check all CI quality gates green
  - [ ] 48.4 Tag release and archive artifacts (binaries, installers, docs)
  - _Requirements: RELEASE-01_

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
