# Implementation Plan

This task list provides a series of discrete, incremental coding steps to complete and polish the simulator integration layer, force feedback protocols, and platform-specific runtime infrastructure for Flight Hub v1.

**Current Implementation Status:**
- ✅ Core BusSnapshot structure and types exist in `flight-bus`
- ✅ MSFS SimConnect adapter exists in `flight-simconnect`
- ✅ X-Plane UDP adapter exists in `flight-xplane`
- ✅ DCS Export.lua adapter exists in `flight-dcs-export`
- ✅ FFB engine framework exists in `flight-ffb`
- ✅ Real-time scheduler exists in `flight-scheduler`
- 📋 DirectInput FFB device I/O needs completion
- 📋 Windows MMCSS/high-res timers need completion
- 📋 Linux rtkit integration needs completion
- 📋 Packaging and distribution infrastructure needs implementation
- 📋 Comprehensive testing and documentation needs completion

All context documents (requirements, design) are available during implementation. Each task builds on previous tasks to ensure continuous integration and early validation of core functionality.

## Task List

- [x] 1. Review and enhance core BusSnapshot types and validation





  - Review existing BusSnapshot structure in `crates/flight-bus/src/snapshot.rs`
  - Verify all core fields are present: sim identifier, aircraft identifier, timestamp, attitude, angular rates, velocities, kinematics, aerodynamics, aircraft state, control inputs, trim state, validity flags
  - Enhance validation methods for core field range checking if needed
  - Verify unit conversion utilities exist (degrees↔radians, knots↔m/s, feet↔meters, FPM↔m/s)
  - Verify snapshot age calculation API exists
  - _Requirements: BUS-CORE-01.1, BUS-CORE-01.2, BUS-CORE-01.3, BUS-CORE-01.4, BUS-CORE-01.5, BUS-CORE-01.6, BUS-CORE-01.7, BUS-CORE-01.8, BUS-CORE-01.9, BUS-CORE-01.10, BUS-CORE-01.11, BUS-CORE-01.15_

- [x] 1.1 Write unit tests for core BusSnapshot validation


  - Test validated type construction and range enforcement for core fields
  - Test unit conversion accuracy (degrees↔radians, knots↔m/s, FPM↔m/s)
  - Test snapshot age calculation
  - Test core field validation (attitude, velocities, g-loads within ranges)
  - _Requirements: BUS-CORE-01.12, BUS-CORE-01.14_

- [x] 1.2 Review and enhance extended BusSnapshot fields


  - Review existing extended fields in BusSnapshot: engines list, fuel per tank, helicopter telemetry block, environment, navigation, autopilot, lights
  - Add any missing extended fields
  - Enhance validation for extended fields (unique engine indices, helicopter pedal ranges, extended field ranges)
  - _Requirements: BUS-EXTENDED-01.1, BUS-EXTENDED-01.2, BUS-EXTENDED-01.3, BUS-EXTENDED-01.4, BUS-EXTENDED-01.5, BUS-EXTENDED-01.6, BUS-EXTENDED-01.7, BUS-EXTENDED-01.8_

- [x] 1.3 Write unit tests for extended BusSnapshot validation


  - Test unique engine indices validation
  - Test helicopter pedal range validation (-100 to 100)
  - Test extended field range validation
  - _Requirements: BUS-EXTENDED-01.8, BUS-EXTENDED-01.9_

- [x] 2. Review and enhance MSFS SimConnect adapter connection management





  - Review existing MsfsAdapter in `crates/flight-simconnect/src/adapter.rs`
  - Verify connection state machine implementation
  - Verify local SimConnect connection works without SimConnect.cfg requirement
  - Enhance exponential backoff reconnection logic if needed (up to 30s between attempts)
  - Verify connection loss detection and state transitions
  - _Requirements: MSFS-INT-01.1, MSFS-INT-01.2, MSFS-INT-01.19_

- [x] 2.1 Write unit tests for MSFS connection management


  - Test connection state transitions
  - Test exponential backoff timing
  - Test connection loss detection
  - _Requirements: MSFS-INT-01.2, MSFS-INT-01.19_

- [x] 3. Review and enhance MSFS SimConnect data definitions and telemetry mapping





  - Review existing data definitions in MSFS adapter
  - Verify high-rate telemetry data definition with explicit units for each SimVar
  - Verify low-rate identity data definition
  - Review SimVar → BusSnapshot field mapping with unit conversions
  - Verify dispatch queue draining handles burst events correctly
  - _Requirements: MSFS-INT-01.3, MSFS-INT-01.4, MSFS-INT-01.5, MSFS-INT-01.6_

- [x] 3.1 Write unit tests for MSFS telemetry mapping


  - Test attitude conversion (degrees → radians)
  - Test velocity conversion (knots → m/s, FPM → m/s, ft/s → m/s)
  - Test angular rate mapping (already rad/s)
  - Test g-load and aero mapping
  - Use fixture data from tests/fixtures/msfs_c172_cruise.json
  - _Requirements: MSFS-INT-01.4, MSFS-INT-01.5, MSFS-INT-01.6, SIM-TEST-01.2_

- [x] 4. Review and enhance MSFS Sanity Gate state machine





  - Review existing sanity gate implementation in MSFS adapter
  - Verify state machine has all required states (Disconnected, Booting, Loading, ActiveFlight, Paused, Faulted)
  - Verify state transition logic with explicit criteria
  - Verify safe_for_ffb flag control (true only in ActiveFlight)
  - Verify NaN/Inf detection with rate-limited logging
  - Verify physically implausible jump detection
  - Verify sanity violation counter with configurable threshold
  - _Requirements: MSFS-INT-01.9, MSFS-INT-01.10, MSFS-INT-01.11, MSFS-INT-01.12, MSFS-INT-01.13, MSFS-INT-01.14, MSFS-INT-01.15, MSFS-INT-01.16_

- [x] 4.1 Write unit tests for MSFS Sanity Gate


  - Test state transitions (Booting→Loading→ActiveFlight, ActiveFlight⇄Paused, Any→Faulted)
  - Test NaN/Inf detection and violation counting
  - Test physically implausible jump detection
  - Test safe_for_ffb flag behavior in each state
  - _Requirements: MSFS-INT-01.14, MSFS-INT-01.15, MSFS-INT-01.16, SIM-TEST-01.2, SIM-TEST-01.8_

- [x] 5. Review and enhance MSFS update rate monitoring and metrics





  - Review existing update rate monitoring
  - Verify conditional 60 Hz target (when sim FPS ≥60)
  - Verify metrics for actual update rate and jitter are exposed via shared metrics system (Task 41) under sim.msfs.* namespace
  - Verify aircraft change detection via TITLE SimVar
  - _Requirements: MSFS-INT-01.7, MSFS-INT-01.8, MSFS-INT-01.17_

- [x] 5.1 Write integration tests for MSFS adapter


  - Test complete adapter lifecycle with recorded fixtures
  - Test reconnection behavior
  - Test aircraft change detection
  - _Requirements: SIM-TEST-01.1, SIM-TEST-01.5, SIM-TEST-01.7_

- [ ] 6. Create MSFS SimVar mapping documentation
  - Document all SimVar → BusSnapshot field mappings in docs/integration/msfs-simvar-mapping.md
  - Document unit conversions in code comments
  - _Requirements: MSFS-INT-01.Doc.1, MSFS-INT-01.Doc.2_

- [ ] 7. Review and enhance X-Plane UDP adapter packet parsing
  - Review existing XPlaneAdapter in `crates/flight-xplane/src/`
  - Verify DATA packet format parser (36-byte records)
  - Verify data group extraction (groups 3, 4, 16, 17, 18, 21)
  - Verify graceful handling of missing data groups
  - _Requirements: XPLANE-INT-01.1, XPLANE-INT-01.2, XPLANE-INT-01.3, XPLANE-INT-01.6_

- [ ] 7.1 Write unit tests for X-Plane packet parsing
  - Test DATA packet parsing with valid data
  - Test handling of missing data groups
  - Test malformed packet handling
  - _Requirements: XPLANE-INT-01.2, XPLANE-INT-01.6, SIM-TEST-01.3_

- [ ] 8. Review and enhance X-Plane telemetry mapping and connection monitoring
  - Review existing data group → BusSnapshot field mapping with unit conversions
  - Verify connection timeout detection (2 seconds)
  - Verify connection timeout metrics are exposed via shared metrics system (Task 41) under sim.xplane.* namespace
  - Verify aircraft identity handling for UDP-only mode (sim=XPLANE, coarse aircraft_class, identity='unknown')
  - _Requirements: XPLANE-INT-01.4, XPLANE-INT-01.5, XPLANE-INT-01.7, XPLANE-INT-01.13_

- [ ] 8.1 Write unit tests for X-Plane telemetry mapping
  - Test angle conversion (degrees → radians)
  - Test rate conversion (deg/s → rad/s)
  - Test speed conversion (knots → m/s)
  - Test connection timeout detection
  - _Requirements: XPLANE-INT-01.4, XPLANE-INT-01.5, XPLANE-INT-01.13, SIM-TEST-01.3_

- [ ] 9. Create X-Plane data group mapping documentation
  - Document all data group indices → BusSnapshot field mappings in docs/integration/xplane.md
  - Document unit conversions per data group
  - Provide setup instructions for X-Plane Data Output screen configuration
  - _Requirements: XPLANE-INT-01.Doc.1, XPLANE-INT-01.Doc.2_

- [ ] 10. Review and enhance DCS Export.lua script
  - Review existing FlightHubExport.lua in `crates/flight-dcs-export/`
  - Verify LuaExportStart/Stop/AfterNextFrame hooks
  - Verify proper chaining to existing Export.lua hooks (store previous functions, call in deterministic order)
  - Verify self-aircraft telemetry gathering using LoGet* functions
  - Verify MP integrity check compliance (whitelist self-aircraft data, annotate mp_detected flag)
  - Verify non-blocking UDP transmission to localhost
  - Verify 60Hz target rate via LuaExportActivityNextEvent
  - _Requirements: DCS-INT-01.4, DCS-INT-01.5, DCS-INT-01.6, DCS-INT-01.7, DCS-INT-01.8, DCS-INT-01.9, DCS-INT-01.10, DCS-INT-01.11, DCS-INT-01.12_

- [ ] 11. Review and enhance DCS installer and uninstaller
  - Review existing DCS installer implementation
  - Verify DCS variant detection (DCS, DCS.openbeta, DCS.openalpha)
  - Verify Export.lua backup and append logic
  - Verify FlightHubExport.lua deployment to Scripts/FlightHub/
  - Verify uninstaller with backup restoration
  - _Requirements: DCS-INT-01.1, DCS-INT-01.2, DCS-INT-01.3, DCS-INT-01.14_

- [ ] 11.1 Write unit tests for DCS installer
  - Test variant detection
  - Test Export.lua backup and append logic
  - Test uninstaller backup restoration
  - _Requirements: DCS-INT-01.1, DCS-INT-01.2, DCS-INT-01.3, DCS-INT-01.14_

- [ ] 12. Review and enhance DCS Rust adapter
  - Review existing DcsAdapter in `crates/flight-dcs-export/src/`
  - Verify JSON packet parsing
  - Verify Lua value → BusSnapshot field mapping with unit conversions
  - Verify nil handling for graceful degradation
  - Verify MP status annotation (mp_detected flag, no invalidation of self-aircraft data)
  - Verify connection timeout detection (2 seconds)
  - Verify connection timeout and MP status metrics are exposed via shared metrics system (Task 41) under sim.dcs.* namespace
  - Verify aircraft change detection via unit type
  - _Requirements: DCS-INT-01.7, DCS-INT-01.8, DCS-INT-01.11, DCS-INT-01.13, DCS-INT-01.15_

- [ ] 12.1 Write unit tests for DCS adapter
  - Test JSON parsing and field mapping
  - Test nil handling
  - Test MP status annotation behavior
  - Test connection timeout detection
  - _Requirements: DCS-INT-01.8, DCS-INT-01.11, DCS-INT-01.15, SIM-TEST-01.4_

- [ ] 13. Create DCS Lua API mapping documentation
  - Document all Lua API functions → BusSnapshot field mappings in docs/integration/dcs.md
  - Document MP integrity check compliance (whitelisted data, restrictions)
  - _Requirements: DCS-INT-01.Doc.1, DCS-INT-01.Doc.2_

- [ ] 14. Checkpoint – all adapter tests passing before starting FFB tasks
  - Verify all MSFS, X-Plane, and DCS adapter unit and integration tests pass
  - Verify all mapping documentation is complete

- [ ] 15. Complete DirectInput FFB device abstraction
  - Review existing FFB framework in `crates/flight-ffb/src/`
  - Complete DirectInputFfbDevice implementation with IDirectInputDevice8 interface
  - Complete device enumeration and connection
  - Complete capability querying (supports_pid, max_torque_nm, min_period_us)
  - Complete device acquisition and cooperative level setting
  - Note: Safety framework exists, device I/O needs completion
  - _Requirements: FFB-HID-01.1, FFB-HID-01.9_

- [ ] 15.1 Write unit tests for DirectInput device abstraction
  - Test device enumeration
  - Test capability querying
  - Test device acquisition
  - _Requirements: FFB-HID-01.1, FFB-HID-01.9_

- [ ] 16. Complete DirectInput FFB effect creation and management
  - Complete constant force effect creation for pitch and roll axes
  - Complete periodic (sine) effect creation for buffeting/vibration
  - Complete condition effects (spring/damper) for centering
  - Complete effect parameter updates via SetParameters
  - Complete effect start/stop control
  - _Requirements: FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4_

- [ ] 16.1 Write unit tests for FFB effect management
  - Test constant force effect creation
  - Test periodic effect creation
  - Test condition effect creation
  - Test effect parameter updates
  - _Requirements: FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4_

- [ ] 17. Review and enhance XInput rumble integration
  - Review existing XInput integration in FFB framework
  - Verify rumble channel mapping (low-freq and high-freq motors)
  - Verify documentation of XInput limitations (vibration only, no directional torque)
  - _Requirements: FFB-HID-01.5_

- [ ] 18. Review and enhance FFB safety envelope
  - Review existing FfbSafetyEnvelope implementation (safety framework exists)
  - Verify torque magnitude clamping to device max_torque_nm
  - Verify slew rate limiting (ΔNm/Δt ≤ configured limit)
  - Verify jerk limiting (Δ²Nm/Δt² ≤ configured limit)
  - Verify safe_for_ffb flag enforcement (zero torque when false)
  - Verify 50ms ramp-to-zero on fault with explicit fault timestamp tracking
  - _Requirements: FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4, FFB-SAFETY-01.6_

- [ ] 18.1 Write unit tests for FFB safety envelope
  - Test torque clamping
  - Test slew rate limiting
  - Test jerk limiting
  - Test safe_for_ffb enforcement
  - Test 50ms ramp-to-zero timing
  - _Requirements: FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4, FFB-SAFETY-01.6, SIM-TEST-01.10, QG-FFB-SAFETY_

- [ ] 19. Review and enhance FFB fault detection and handling
  - Review existing FaultDetector implementation (safety framework exists)
  - Verify USB OUT stall detection (≥3 frames)
  - Verify NaN/Inf detection in FFB pipeline
  - Verify device health monitoring (over-temp, over-current)
  - Verify device disconnect detection (within 100ms)
  - Verify fault categorization (hardware-critical vs transient)
  - Verify fault state latching with power cycle requirement for hardware-critical faults
  - Verify explicit "clear fault" for transient faults
  - _Requirements: FFB-SAFETY-01.5, FFB-SAFETY-01.6, FFB-SAFETY-01.7, FFB-SAFETY-01.8, FFB-SAFETY-01.9, FFB-SAFETY-01.10, FFB-SAFETY-01.11_

- [ ] 19.1 Write unit tests for FFB fault detection
  - Test USB stall detection
  - Test NaN/Inf detection
  - Test device health monitoring
  - Test disconnect detection
  - Test fault categorization and latching
  - _Requirements: FFB-SAFETY-01.5, FFB-SAFETY-01.6, FFB-SAFETY-01.7, FFB-SAFETY-01.8, FFB-SAFETY-01.9, FFB-SAFETY-01.10, FFB-SAFETY-01.11, SIM-TEST-01.10, QG-FFB-SAFETY_

- [ ] 20. Review and enhance FFB blackbox recorder
  - Review existing BlackboxRecorder implementation (safety framework exists)
  - Verify high-rate capture (≥250 Hz) of BusSnapshot, FFB setpoints, and device feedback
  - Verify 2-second pre-fault and 1-second post-fault buffering
  - Verify bounded, rotating log storage (size/age-limited)
  - _Requirements: FFB-SAFETY-01.12, FFB-SAFETY-01.13_

- [ ] 20.1 Write unit tests for blackbox recorder
  - Test high-rate capture (≥250 Hz)
  - Test pre/post-fault buffering (2s before, 1s after)
  - Test bounded storage and rotation
  - _Requirements: FFB-SAFETY-01.12, FFB-SAFETY-01.13, QG-FFB-SAFETY_

- [ ] 21. Review and enhance FFB emergency stop
  - Review existing emergency stop implementation
  - Verify UI button for emergency stop
  - Verify hardware button support (if device supports it)
  - Verify immediate FFB disable on emergency stop
  - _Requirements: FFB-SAFETY-01.14_

- [ ] 22. Checkpoint – all FFB safety tests passing before starting runtime work
  - Verify all DirectInput FFB device, effect management, safety envelope, fault detection, and blackbox recorder tests pass
  - Verify emergency stop functionality is implemented and tested

- [ ] 23. Complete Windows real-time thread configuration
  - Review existing scheduler implementation in `crates/flight-scheduler/src/`
  - Complete WindowsRtThread implementation
  - Complete thread priority elevation (THREAD_PRIORITY_TIME_CRITICAL)
  - Complete MMCSS registration with "Games" or "Pro Audio" task
  - Complete power throttling disable via PROCESS_POWER_THROTTLING_EXECUTION_SPEED
  - Verify QueryPerformanceCounter (QPC) monotonic clock
  - Note: Basic implementation exists, MMCSS integration needs completion
  - _Requirements: WIN-RT-01.1, WIN-RT-01.2, WIN-RT-01.3, WIN-RT-01.8_

- [ ] 23.1 Write platform-specific integration tests for Windows RT thread configuration
  - Test thread priority elevation (gated to #[cfg(windows)], run on hardware-backed CI runners)
  - Test MMCSS registration
  - Test power throttling disable
  - Test QPC monotonic clock
  - _Requirements: WIN-RT-01.1, WIN-RT-01.2, WIN-RT-01.3, WIN-RT-01.8, RT-TEST-01.1_

- [ ] 24. Complete Windows high-resolution timer loop
  - Review existing timer loop implementation
  - Complete CreateWaitableTimerEx with CREATE_WAITABLE_TIMER_HIGH_RESOLUTION
  - Complete fallback to timeBeginPeriod(1) if high-resolution timer unavailable
  - Verify 250 Hz tick loop with periodic timer
  - Verify final 50-80μs busy-spin using QPC
  - Note: Basic implementation exists, high-resolution timer integration needs completion
  - _Requirements: WIN-RT-01.4, WIN-RT-01.5_

- [ ] 24.1 Write platform-specific integration tests for Windows timer loop
  - Test high-resolution timer creation (gated to #[cfg(windows)], run on hardware-backed CI runners)
  - Test fallback to timeBeginPeriod(1)
  - Test 250 Hz tick timing
  - Test busy-spin final portion
  - _Requirements: WIN-RT-01.4, WIN-RT-01.5, RT-TEST-01.1_

- [ ] 25. Implement Windows power management integration
  - Implement PowerSetRequest with EXECUTION_REQUIRED and SYSTEM_REQUIRED when active
  - Implement power request clearing when idle
  - _Requirements: WIN-RT-01.6, WIN-RT-01.7_

- [ ] 25.1 Write platform-specific integration tests for Windows power management
  - Test PowerSetRequest when active (gated to #[cfg(windows)], run on hardware-backed CI runners)
  - Test power request clearing when idle
  - _Requirements: WIN-RT-01.6, WIN-RT-01.7, RT-TEST-01.6_

- [ ] 26. Implement Windows HID write optimization
  - Implement WriteFile with FILE_FLAG_OVERLAPPED for non-blocking I/O
  - Avoid HidD_SetOutputReport in hot path
  - _Requirements: WIN-RT-01.9, WIN-RT-01.10_

- [ ] 26.1 Implement HID write latency measurement harness
  - Create latency measurement harness that sends synthetic OUT reports at target rate
  - Measure end-to-end write latency into a histogram
  - Expose p99 latency as a metric
  - Run only on hardware-backed CI runners with actual HID devices
  - _Requirements: RT-TEST-01.6, QG-HID-LATENCY_

- [ ] 26.2 Write HID latency CI test
  - Create test that asserts p99 ≤ 300μs on hardware-backed runners
  - Skip test (or report-only) when hardware tag not present
  - Mark as #[ignore] by default, opt-in via CI job
  - _Requirements: RT-TEST-01.6, QG-HID-LATENCY_

- [ ] 27. Complete Linux real-time thread configuration
  - Review existing Linux scheduler implementation
  - Complete LinuxRtThread implementation
  - Verify SCHED_FIFO scheduling request via pthread_setschedparam
  - Complete rtkit D-Bus integration for privilege acquisition
  - Verify fallback to normal priority with warning
  - Verify mlockall(MCL_CURRENT | MCL_FUTURE) for memory locking
  - Verify RLIMIT_RTPRIO and RLIMIT_MEMLOCK validation
  - Note: Basic implementation exists, full rtkit integration needs completion
  - Note: Linux FFB output is out of scope for v1; this work is for timing harness + input loop only
  - _Requirements: LINUX-RT-01.1, LINUX-RT-01.2, LINUX-RT-01.3, LINUX-RT-01.4, LINUX-RT-01.7, LINUX-RT-01.11_

- [ ] 27.1 Write platform-specific integration tests for Linux RT thread configuration
  - Test SCHED_FIFO scheduling request (gated to #[cfg(target_os = "linux")], run on hardware-backed CI runners)
  - Test rtkit integration
  - Test fallback to normal priority
  - Test mlockall
  - Test limits validation
  - _Requirements: LINUX-RT-01.1, LINUX-RT-01.2, LINUX-RT-01.3, LINUX-RT-01.4, LINUX-RT-01.7, RT-TEST-01.2_

- [ ] 28. Implement Linux high-resolution timer loop
  - Implement clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME) with absolute target times
  - Implement 250 Hz tick loop
  - Implement final portion busy-spin using clock_gettime(CLOCK_MONOTONIC)
  - _Requirements: LINUX-RT-01.5, LINUX-RT-01.6_

- [ ] 28.1 Write platform-specific integration tests for Linux timer loop
  - Test clock_nanosleep with absolute times (gated to #[cfg(target_os = "linux")], run on hardware-backed CI runners)
  - Test 250 Hz tick timing
  - Test busy-spin final portion
  - _Requirements: LINUX-RT-01.5, LINUX-RT-01.6, RT-TEST-01.2_

- [ ] 29. Implement Linux RT metrics exposure
  - Expose metrics for RT scheduling status via shared metrics system (Task 41) under runtime.* namespace
  - Expose metrics for timing accuracy at normal priority
  - _Requirements: LINUX-RT-01.8_

- [ ] 30. Create Linux RT setup helper script
  - Create scripts/setup-linux-rt.sh with /etc/security/limits.conf entries
  - Document RT priority configuration in installation docs
  - _Requirements: LINUX-RT-01.10_

- [ ] 31. Implement runtime jitter measurement
  - Create JitterMeasurement struct
  - Implement tick deviation recording
  - Implement p99 jitter calculation
  - Implement 5-second warm-up period
  - _Requirements: RT-TEST-01.3_

- [ ] 31.1 Write long-running jitter tests
  - Test 250 Hz axis loop jitter on hardware-backed runners (p99 ≤0.5ms)
  - Test report-only mode on virtualized runners
  - Run for ≥10 minutes with 5s warm-up
  - Mark as #[ignore] by default, opt-in via CI job
  - _Requirements: RT-TEST-01.3, RT-TEST-01.4, RT-TEST-01.5_

- [ ] 31.2 Add hardware matrix coverage for jitter tests
  - Run jitter tests on both Intel and AMD hardware runners
  - Record results separately for each platform
  - Wire CI labels/runners for Intel vs AMD coverage
  - _Requirements: RT-TEST-01.11_

- [ ] 32. Checkpoint – all runtime tests passing before packaging
  - Verify all Windows and Linux RT thread configuration, timer loop, power management, and HID write tests pass
  - Verify jitter measurement and HID latency measurement harnesses are implemented
  - Verify metrics are exposed via shared metrics system

- [ ] 33. Implement Windows code signing infrastructure
  - Create scripts/sign-binaries.ps1 for code signing
  - Implement signing for all EXE and DLL files
  - Integrate signing into CI build process
  - Note: MSI signing will be implemented in Task 34 after MSI packaging is complete
  - _Requirements: PKG-01.1, PKG-01.2_

- [ ] 34. Implement Windows MSI installer
  - Create WiX configuration for MSI package
  - Implement core binaries installation (per-user option)
  - Implement simulator integration components as opt-in features
  - Implement elevated privileges requirement for sim integrations and virtual drivers
  - Implement product posture statement display
  - Implement EULA summary display
  - Implement MSI signing using infrastructure from Task 33
  - Note: Initial MSI may be unsigned for internal dev; final release MSI is signed
  - _Requirements: PKG-01.1, PKG-01.3, PKG-01.4, PKG-01.5, PKG-01.6, PKG-01.7, PKG-01.8_

- [ ] 35. Implement Windows uninstaller
  - Implement binary removal
  - Implement X-Plane plugin removal (if installed)
  - Implement DCS Export.lua restoration (if backed up)
  - _Requirements: PKG-01.9_

- [ ] 36. Implement Linux package formats
  - Create Debian package (.deb) with control, rules, changelog, copyright files
  - Create udev rules for device access without root
  - Create postinst script for user group management and udev reload
  - Implement at least one additional format (AppImage or .rpm) as optional stretch goal
  - Note: Linux FFB output is out of scope for v1; packaging is for timing harness + input loop only
  - _Requirements: PKG-01.10, PKG-01.11_

- [ ] 37. Create Linux installation documentation
  - Document user group requirements (input, plugdev)
  - Document RT priority configuration via limits.conf
  - Document udev rules installation
  - _Requirements: PKG-01.12_

- [ ] 38. Create third-party components inventory
  - Document all bundled libraries and drivers in third-party-components.toml
  - Include licenses, versions, and usage for each component
  - Ship required license texts with product
  - _Requirements: PKG-01.13, PKG-01.Doc.1_

- [ ] 39. Create product posture document
  - Create docs/product-posture.md with product positioning statement
  - Document what Flight Hub is and is not
  - Document simulator integration approach (official APIs only)
  - Document data handling policies
  - Reference in README, website, installer, and integration module comments
  - _Requirements: LEGAL-01.1, LEGAL-01.5, LEGAL-01.6, LEGAL-01.10_

- [ ] 40. Create "What We Touch" documentation for each simulator
  - Create docs/integration/msfs-what-we-touch.md
  - Create docs/integration/xplane-what-we-touch.md
  - Create docs/integration/dcs-what-we-touch.md
  - Document files modified, ports used, variables accessed, reversion steps
  - _Requirements: LEGAL-01.7_

- [ ] 41. Implement telemetry and metrics system
  - Implement hierarchical metric naming (sim.<sim_name>.<metric>, ffb.<metric>, runtime.<metric>, bus.<metric>)
  - Implement metric types (gauges, counters, histograms)
  - Implement Prometheus exporter (optional, HTTP endpoint at :9090/metrics)
  - Implement in-process ring buffer for UI display (60 seconds)
  - Implement log-structured metric snapshots (JSON lines)
  - _Requirements: LINUX-RT-01.8, RT-TEST-01.3, RT-TEST-01.4, RT-TEST-01.5, RT-TEST-01.6, Design: Telemetry and Metrics_

- [ ] 41.1 Write unit tests for metrics system
  - Test metric naming and scoping (per-adapter, per-device, not per-aircraft)
  - Test metric type behavior (gauges, counters, histograms)
  - Test Prometheus export format
  - Test ring buffer retention (60 seconds)
  - _Requirements: LINUX-RT-01.8, RT-TEST-01.3, QG-RT-JITTER, QG-HID-LATENCY_

- [ ] 42. Implement cargo xtask validation commands
  - Implement cargo xtask validate-msfs-telemetry
  - Implement cargo xtask validate-xplane-telemetry
  - Implement cargo xtask validate-dcs-export
  - _Requirements: SIM-TEST-01.9_

- [ ] 43. Checkpoint – all tests and quality gates passing before tagging a release
  - Verify all unit tests, integration tests, and platform-specific tests pass
  - Verify all CI quality gates pass (QG-SIM-MAPPING, QG-UNIT-CONV, QG-SANITY-GATE, QG-FFB-SAFETY, QG-RT-JITTER, QG-HID-LATENCY, QG-LEGAL-DOC)
  - Verify all documentation is complete

- [ ] 44. Create CI quality gate enforcement
  - Implement QG-SIM-MAPPING: Fail if any adapter lacks complete mapping table documentation
  - Implement QG-UNIT-CONV: Fail if unit conversion tests don't cover all v1 BusSnapshot fields
  - Implement QG-SANITY-GATE: Fail if sanity gate tests don't inject NaN/Inf and verify handling
  - Implement QG-FFB-SAFETY: Fail if FFB safety tests don't verify 50ms ramp-down on all fault types
  - Implement QG-RT-JITTER: Fail if 250Hz p99 jitter >0.5ms on hardware-backed runners; report-only on virtualized
  - Implement QG-HID-LATENCY: Fail if HID write p99 >300μs on hardware-backed runners; skip when unavailable
  - Implement QG-LEGAL-DOC: Fail if product posture document is not present or not referenced in required locations
  - _Requirements: CI Quality Gates section_

- [ ] 45. Create comprehensive integration test suite
  - Create fixture files for each simulator (MSFS, X-Plane, DCS) under tests/fixtures/
  - Implement replay testing with recorded telemetry fixtures
  - Test complete adapter lifecycle (connect, telemetry, disconnect)
  - Test reconnection with exponential backoff
  - Test sanity gate with NaN/Inf injection and implausible jumps
  - _Requirements: SIM-TEST-01.5, SIM-TEST-01.6, SIM-TEST-01.7, SIM-TEST-01.8_

- [ ] 45.1 Implement soak tests
  - Run 24-48h with synthetic telemetry
  - Verify zero missed ticks
  - Verify RSS delta <10%
  - Verify no memory leaks
  - Run on both Intel and AMD hardware runners
  - _Requirements: RT-TEST-01.8, RT-TEST-01.11_

- [ ] 46. Create user documentation
  - Create installation guide for Windows (MSI)
  - Create installation guide for Linux (.deb, AppImage)
  - Create simulator setup guides (MSFS, X-Plane Data Output, DCS Export.lua)
  - Create FFB device configuration guide
  - Create troubleshooting guide
  - _Requirements: General documentation requirements_

- [ ] 47. Final validation and release preparation
  - Run all unit tests, integration tests, and quality gates
  - Verify all documentation is complete and accurate
  - Verify code signing works correctly
  - Verify installers work on clean systems
  - Verify uninstallers properly clean up
  - Create release notes
