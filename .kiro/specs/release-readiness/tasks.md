# Implementation Plan: Release Readiness

## Overview

This task list provides a phased implementation plan for bringing Flight Hub to production readiness. Tasks are organized into five phases: Platform Runtime, Packaging, Testing, Documentation, and Release Preparation. Each phase builds on the previous, with quality gates enforced throughout.

**Implementation Philosophy:**
- Platform-native APIs (MMCSS, rtkit) over workarounds
- Fail-safe defaults with graceful degradation
- Observable behavior via metrics
- Reversible installations

**Legend:**
- Tasks marked with `*` are optional and can be skipped for faster MVP
- Property tests reference design document properties

---

## Phase 1: Platform Runtime

**Goal:** Complete Windows and Linux real-time scheduling with validated jitter.

**Exit Criteria:**
- [ ] Windows MMCSS + high-res timers working
- [ ] Linux rtkit + clock_nanosleep working
- [ ] Jitter tests passing on hardware runners

### Windows Real-Time

- [ ] 1. Implement Windows RT thread configuration
  - [ ] 1.1 Create `WindowsRtThread` struct in `flight-scheduler`
    - Implement MMCSS registration via `AvSetMmThreadCharacteristicsW`
    - Implement thread priority elevation via `SetThreadPriority`
    - Implement RAII cleanup via `Drop` trait
    - _Requirements: 1.1, 1.2, 1.5_
  
  - [ ] 1.2 Implement power throttling disable
    - Use `SetProcessInformation` with `PROCESS_POWER_THROTTLING_EXECUTION_SPEED`
    - Call on process start when sim/FFB active
    - _Requirements: 1.3_
  
  - [ ] 1.3 Write property test for MMCSS lifecycle
    - **Property 2: MMCSS Lifecycle**
    - **Validates: Requirements 1.1, 1.2, 1.5**

- [ ] 2. Implement Windows high-resolution timer loop
  - [ ] 2.1 Create `WindowsTimerLoop` struct
    - Implement `CreateWaitableTimerExW` with high-res flag
    - Implement fallback to `timeBeginPeriod(1)` + standard timer
    - Implement busy-spin finish using QPC
    - _Requirements: 2.1, 2.2, 2.3_
  
  - [ ] 2.2 Write jitter test for Windows timer
    - **Property 1: Timer Loop Jitter**
    - **Validates: Requirements 2.5**
    - 10-minute test, assert p99 ≤0.5ms

- [ ] 3. Implement Windows power management
  - [ ] 3.1 Create `PowerManager` struct
    - Implement `PowerCreateRequest` / `PowerSetRequest`
    - Implement activate/deactivate based on sim/FFB state
    - Implement RAII cleanup
    - _Requirements: 3.1, 3.2_

- [ ] 4. Implement Windows HID optimization
  - [ ] 4.1 Create `HidWriter` with overlapped I/O
    - Open devices with `FILE_FLAG_OVERLAPPED`
    - Implement async `WriteFile` with OVERLAPPED pool
    - Avoid `HidD_SetOutputReport` in hot path
    - _Requirements: 4.1, 4.2_
  
  - [ ] 4.2 Write HID latency benchmark
    - **Property 4: HID Write Latency**
    - **Validates: Requirements 4.3**
    - 10-minute test, assert p99 ≤300μs
  
  - [ ] 4.3 Implement HID fault detection
    - Detect USB OUT stalls (≥3 consecutive failures)
    - Trigger fault handler within 3 frames
    - _Requirements: 4.4_
  
  - [ ] 4.4 Write property test for fault detection
    - **Property 5: HID Fault Detection**
    - **Validates: Requirements 4.4**

### Linux Real-Time

- [ ] 5. Implement Linux RT thread configuration
  - [ ] 5.1 Create `LinuxRtThread` struct in `flight-scheduler`
    - Implement rtkit D-Bus integration
    - Implement fallback to `sched_setscheduler`
    - Implement `mlockall` on RT success
    - Validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_
  
  - [ ] 5.2 Implement RT metrics exposure
    - Expose `runtime.linux.rt_enabled`, `sched_policy`, `priority`, `mlockall_success`
    - _Requirements: 7.1_
  
  - [ ] 5.3 Write property test for RT metrics
    - **Property 3: RT Metrics Exposure**
    - **Validates: Requirements 5.3, 7.1**

- [ ] 6. Implement Linux high-resolution timer loop
  - [ ] 6.1 Create `LinuxTimerLoop` struct
    - Implement `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)`
    - Implement busy-spin finish using `clock_gettime`
    - _Requirements: 6.1, 6.2_
  
  - [ ] 6.2 Write jitter test for Linux timer
    - **Property 1: Timer Loop Jitter**
    - **Validates: Requirements 6.3**
    - 10-minute test, assert p99 ≤0.5ms

- [ ] 7. Create Linux RT setup helper
  - [ ] 7.1 Create `scripts/setup-linux-rt.sh`
    - Configure `/etc/security/limits.conf` for rtprio/memlock
    - Print instructions for group membership
    - _Requirements: 7.2, 7.3_

### Cross-Platform Jitter

- [ ] 8. Implement jitter measurement helper
  - [ ] 8.1 Create `JitterMeasurement` struct
    - Record deviation from ideal period
    - Compute p50/p95/p99 statistics
    - Support warmup period exclusion
    - _Requirements: 8.1_
  
  - [ ] 8.2 Write property test for measurement accuracy
    - **Property 10: Jitter Measurement Accuracy**
    - **Validates: Requirements 8.1**

- [ ] 9. Checkpoint - Platform runtime complete
  - Ensure Windows RT + timers working
  - Ensure Linux RT + timers working
  - Ensure jitter tests pass on hardware

---

## Phase 2: Packaging and Distribution

**Goal:** Create signed installers for Windows and Linux.

**Exit Criteria:**
- [ ] Windows MSI builds and installs correctly
- [ ] Linux .deb builds and installs correctly
- [ ] All artifacts signed
- [ ] Third-party license inventory complete

### Windows Installer

- [ ] 10. Implement Windows MSI installer
  - [ ] 10.1 Create WiX project structure
    - Define features: Core, MSFS, X-Plane, DCS
    - Configure per-user install scope for core
    - _Requirements: 9.1, 9.2, 9.3_
  
  - [ ] 10.2 Implement DCS installer helper
    - Detect DCS variants (DCS, DCS.openbeta, DCS.openalpha)
    - Implement Export.lua backup and restore
    - Implement FlightHubExport.lua deployment
    - _Requirements: 9.5, 9.6_
  
  - [ ] 10.3 Write property test for uninstall reversibility
    - **Property 9: Uninstall Reversibility**
    - **Validates: Requirements 9.6**
  
  - [ ] 10.4 Implement custom actions
    - Display product posture on install
    - Backup/restore Export.lua
    - _Requirements: 9.4_

- [ ] 11. Implement Windows code signing
  - [ ] 11.1 Integrate signtool into CI
    - Sign all EXE, DLL, MSI artifacts
    - Fail release builds if unsigned
    - _Requirements: 10.1, 10.2_

### Linux Installer

- [ ] 12. Implement Linux .deb package
  - [ ] 12.1 Create debian package structure
    - Create debian/control with dependencies
    - Ship binaries to /usr/bin
    - _Requirements: 11.1_
  
  - [ ] 12.2 Create udev rules
    - Allow input group access to hidraw devices
    - _Requirements: 11.2_
  
  - [ ] 12.3 Create postinst script
    - Add user to input group
    - Reload udev rules
    - _Requirements: 11.3_

### License Inventory

- [ ] 13. Implement third-party components inventory
  - [ ] 13.1 Create inventory generator
    - Parse Cargo.lock for dependencies
    - Fetch license info from crates.io
    - Generate third-party-components.toml
    - _Requirements: 12.1_
  
  - [ ] 13.2 Collect license texts
    - Download license files for all dependencies
    - Ship with installer
    - _Requirements: 12.2_
  
  - [ ] 13.3 Write property test for inventory completeness
    - **Property 8: License Inventory Completeness**
    - **Validates: Requirements 12.1, 12.2**

- [ ] 14. Checkpoint - Packaging complete
  - Verify MSI installs on clean Win10/11
  - Verify .deb installs on Ubuntu/Debian
  - Verify all artifacts signed
  - Verify license inventory complete

---

## Phase 3: Testing Infrastructure

**Goal:** Implement soak tests and integration tests.

**Exit Criteria:**
- [ ] Soak test framework working
- [ ] Integration tests for all adapters
- [ ] End-to-end test passing

### Soak Tests

- [ ] 15. Implement soak test framework
  - [ ] 15.1 Create `SoakTest` struct
    - Implement synthetic telemetry generator
    - Implement 24-48h test loop
    - Track missed ticks, RSS, faults
    - _Requirements: 13.1_
  
  - [ ] 15.2 Implement soak test assertions
    - Assert no missed ticks
    - Assert RSS delta < 10%
    - Assert blackbox present on faults
    - _Requirements: 13.2_
  
  - [ ] 15.3 Write property test for soak stability
    - **Property 6: Soak Test Stability**
    - **Validates: Requirements 13.2**
  
  - [ ] 15.4 Implement diagnostic output on failure
    - Log tick timing, memory profile, fault details
    - _Requirements: 13.3_

### Integration Tests

- [ ] 16. Implement adapter integration tests
  - [ ] 16.1 Create `AdapterIntegrationTest` framework
    - Test connect → stream → disconnect → reconnect
    - Validate no NaN/Inf in snapshots
    - _Requirements: 14.1_
  
  - [ ] 16.2 Create fixtures for each adapter
    - MSFS SimConnect fixture
    - X-Plane UDP fixture
    - DCS Export.lua fixture
    - _Requirements: 14.1_
  
  - [ ] 16.3 Write property test for adapter lifecycle
    - **Property 7: Adapter Lifecycle**
    - **Validates: Requirements 14.1, 14.2**

- [ ] 17. Implement end-to-end test
  - [ ] 17.1 Create `EndToEndTest` framework
    - Test sim fixture → bus → FFB → safety
    - Assert no safety violations under normal conditions
    - _Requirements: 14.2_
  
  - [ ] 17.2 Implement diagnostic output on failure
    - Log failure point, snapshot state, FFB state
    - _Requirements: 14.3_

- [ ] 18. Checkpoint - Testing infrastructure complete
  - Ensure soak test framework working
  - Ensure all adapter integration tests pass
  - Ensure end-to-end test passes

---

## Phase 4: Documentation

**Goal:** Create all required documentation.

**Exit Criteria:**
- [ ] Product posture document exists
- [ ] "What We Touch" docs for all sims
- [ ] User documentation complete

### Legal Documentation

- [ ] 19. Create product posture document
  - [ ] 19.1 Write docs/product-posture.md
    - State accessory/input manager positioning
    - Include export-control reminders
    - Include EULA reminders from sim vendors
    - _Requirements: 15.1, 15.2_
  
  - [ ] 19.2 Link from README and installer
    - Add link to README.md
    - Add link to installer UI
    - _Requirements: 15.3_

- [ ] 20. Create "What We Touch" documentation
  - [ ] 20.1 Create docs/integration/msfs-what-we-touch.md
    - List files, APIs, SimVars, ports
    - Include revert instructions
    - _Requirements: 16.1, 16.4_
  
  - [ ] 20.2 Create docs/integration/xplane-what-we-touch.md
    - List plugins, DataRefs, UDP ports
    - Include revert instructions
    - _Requirements: 16.2, 16.4_
  
  - [ ] 20.3 Create docs/integration/dcs-what-we-touch.md
    - List Export.lua modifications, data, ports
    - Include revert instructions
    - _Requirements: 16.3, 16.4_

### User Documentation

- [ ] 21. Create user documentation
  - [ ] 21.1 Create install guides
    - Windows install guide
    - Linux install guide
    - _Requirements: 17.1_
  
  - [ ] 21.2 Create per-sim setup guides
    - MSFS setup guide
    - X-Plane setup guide
    - DCS setup guide
    - _Requirements: 17.2_
  
  - [ ] 21.3 Create FFB documentation
    - Device configuration guide
    - Safety guidelines
    - _Requirements: 17.3_
  
  - [ ] 21.4 Create troubleshooting guide
    - RT not enabled
    - No FFB detected
    - Permission issues
    - _Requirements: 17.4_

- [ ] 22. Checkpoint - Documentation complete
  - Verify all docs exist and are linked
  - Verify product posture in README and installer

---

## Phase 5: Release Preparation

**Goal:** Enforce quality gates and prepare release.

**Exit Criteria:**
- [ ] All QG-* checks passing
- [ ] Release tagged and artifacts archived

### CI Quality Gates

- [ ] 23. Implement CI quality gate jobs
  - [ ] 23.1 Create QG-RT-JITTER job
    - Run on hardware runners
    - Assert p99 ≤0.5ms
    - _Requirements: 18.1_
  
  - [ ] 23.2 Create QG-HID-LATENCY job
    - Run on hardware runners with HID device
    - Assert p99 ≤300μs
    - _Requirements: 18.1_
  
  - [ ] 23.3 Create QG-LEGAL-DOC job
    - Check all required docs exist
    - _Requirements: 18.1_
  
  - [ ] 23.4 Wire gates as required checks
    - Block merge on main/release if gates fail
    - _Requirements: 18.2_
  
  - [ ] 23.5 Document gates in CONTRIBUTING.md
    - List all QG-* checks
    - Explain how to run locally
    - _Requirements: 18.3_

### Release Process

- [ ] 24. Implement release preparation
  - [ ] 24.1 Create release checklist script
    - Run full test matrix
    - Verify installers on clean systems
    - Check all quality gates green
    - _Requirements: 19.1, 19.2, 19.3_
  
  - [ ] 24.2 Create release tagging workflow
    - Tag release with version
    - Archive artifacts (binaries, installers, docs)
    - Generate release notes from changelog
    - _Requirements: 19.4, 19.5_

- [ ] 25. Final checkpoint - Release ready
  - All quality gates passing
  - Installers verified on clean systems
  - Documentation complete
  - Release tagged and artifacts archived

---

## Notes

- All tasks are required for comprehensive release readiness
- Property tests reference design document properties for traceability
- Hardware-backed tests (jitter, HID latency) require self-hosted runners
- Soak tests (24-48h) should be run manually or in scheduled CI jobs
