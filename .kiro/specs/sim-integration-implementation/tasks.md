# Implementation Plan

This task list provides a series of discrete, incremental coding steps to implement the simulator integration layer, force feedback protocols, and platform-specific runtime infrastructure for Flight Hub v1.

All context documents (requirements, design) are available during implementation. Each task builds on previous tasks to ensure continuous integration and early validation of core functionality.

## Task List

- [ ] 1. Implement core BusSnapshot types and validation
  - Create type-safe BusSnapshot structure with validated types (ValidatedSpeed, ValidatedAngle, Percentage, GForce, Mach)
  - Implement validation methods for range checking and field consistency
  - Add unit conversion utilities (degrees↔radians, knots↔m/s, feet↔meters)
  - Implement snapshot age calculation API
  - _Requirements: BUS-CORE-01.1, BUS-CORE-01.2, BUS-CORE-01.3, BUS-CORE-01.4, BUS-CORE-01.5, BUS-CORE-01.6, BUS-CORE-01.7, BUS-CORE-01.15_

- [ ] 1.1 Write unit tests for BusSnapshot validation
  - Test validated type construction and range enforcement
  - Test unit conversion accuracy
  - Test snapshot age calculation
  - Test field validation (unique engine indices, helicopter pedal ranges)
  - _Requirements: BUS-CORE-01.9, BUS-EXTENDED-01.8_

- [ ] 2. Implement MSFS SimConnect adapter connection management
  - Create MsfsAdapter struct with connection state machine
  - Implement local SimConnect connection without SimConnect.cfg requirement
  - Implement exponential backoff reconnection logic (up to 30s between attempts)
  - Implement connection loss detection and state transitions
  - _Requirements: MSFS-INT-01.1, MSFS-INT-01.2, MSFS-INT-01.19_

- [ ] 2.1 Write unit tests for MSFS connection management
  - Test connection state transitions
  - Test exponential backoff timing
  - Test connection loss detection
  - _Requirements: MSFS-INT-01.2, MSFS-INT-01.19_

- [ ] 3. Implement MSFS SimConnect data definitions and telemetry mapping
  - Register high-rate telemetry data definition with explicit units for each SimVar
  - Register low-rate identity data definition
  - Implement SimVar → BusSnapshot field mapping with unit conversions
  - Implement dispatch queue draining to handle burst events
  - _Requirements: MSFS-INT-01.3, MSFS-INT-01.4, MSFS-INT-01.5, MSFS-INT-01.6_

- [ ] 3.1 Write unit tests for MSFS telemetry mapping
  - Test attitude conversion (degrees → radians)
  - Test velocity conversion (knots → m/s, FPM → m/s, ft/s → m/s)
  - Test angular rate mapping (already rad/s)
  - Test g-load and aero mapping
  - Use fixture data from tests/fixtures/msfs_c172_cruise.json
  - _Requirements: MSFS-INT-01.4, MSFS-INT-01.5, MSFS-INT-01.6, SIM-TEST-01.2_

- [ ] 4. Implement MSFS Sanity Gate state machine
  - Create SanityGate struct with state machine (Disconnected, Booting, Loading, ActiveFlight, Paused, Faulted)
  - Implement state transition logic with explicit criteria
  - Implement safe_for_ffb flag control (true only in ActiveFlight)
  - Implement NaN/Inf detection with rate-limited logging
  - Implement physically implausible jump detection
  - Implement sanity violation counter with configurable threshold
  - _Requirements: MSFS-INT-01.9, MSFS-INT-01.10, MSFS-INT-01.11, MSFS-INT-01.12, MSFS-INT-01.13, MSFS-INT-01.14, MSFS-INT-01.15, MSFS-INT-01.16_

- [ ] 4.1 Write unit tests for MSFS Sanity Gate
  - Test state transitions (Booting→Loading→ActiveFlight, ActiveFlight⇄Paused, Any→Faulted)
  - Test NaN/Inf detection and violation counting
  - Test physically implausible jump detection
  - Test safe_for_ffb flag behavior in each state
  - _Requirements: MSFS-INT-01.14, MSFS-INT-01.15, MSFS-INT-01.16, SIM-TEST-01.2, SIM-TEST-01.8_

- [ ] 5. Implement MSFS update rate monitoring and metrics
  - Implement conditional 60 Hz target (when sim FPS ≥60)
  - Expose metrics for actual update rate and jitter
  - Implement aircraft change detection via TITLE SimVar
  - _Requirements: MSFS-INT-01.7, MSFS-INT-01.8, MSFS-INT-01.17_

- [ ] 5.1 Write integration tests for MSFS adapter
  - Test complete adapter lifecycle with recorded fixtures
  - Test reconnection behavior
  - Test aircraft change detection
  - _Requirements: SIM-TEST-01.1, SIM-TEST-01.5, SIM-TEST-01.7_

- [ ] 6. Create MSFS SimVar mapping documentation
  - Document all SimVar → BusSnapshot field mappings in docs/integration/msfs-simvar-mapping.md
  - Document unit conversions in code comments
  - _Requirements: MSFS-INT-01.Doc.1, MSFS-INT-01.Doc.2_

- [ ] 7. Implement X-Plane UDP adapter packet parsing
  - Create XPlaneAdapter struct with UDP socket
  - Implement DATA packet format parser (36-byte records)
  - Implement data group extraction (groups 3, 4, 16, 17, 18, 21)
  - Implement graceful handling of missing data groups
  - _Requirements: XPLANE-INT-01.1, XPLANE-INT-01.2, XPLANE-INT-01.3, XPLANE-INT-01.6_

- [ ] 7.1 Write unit tests for X-Plane packet parsing
  - Test DATA packet parsing with valid data
  - Test handling of missing data groups
  - Test malformed packet handling
  - _Requirements: XPLANE-INT-01.2, XPLANE-INT-01.6, SIM-TEST-01.3_

- [ ] 8. Implement X-Plane telemetry mapping and connection monitoring
  - Implement data group → BusSnapshot field mapping with unit conversions
  - Implement connection timeout detection (2 seconds)
  - Implement aircraft identity handling for UDP-only mode (sim=XPLANE, coarse aircraft_class, identity='unknown')
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

- [ ] 10. Implement DCS Export.lua script
  - Create FlightHubExport.lua with LuaExportStart/Stop/AfterNextFrame hooks
  - Implement proper chaining to existing Export.lua hooks (store previous functions, call in deterministic order)
  - Implement self-aircraft telemetry gathering using LoGet* functions
  - Implement MP integrity check compliance (whitelist self-aircraft data, annotate mp_detected flag)
  - Implement non-blocking UDP transmission to localhost
  - Implement 60Hz target rate via LuaExportActivityNextEvent
  - _Requirements: DCS-INT-01.4, DCS-INT-01.5, DCS-INT-01.6, DCS-INT-01.7, DCS-INT-01.8, DCS-INT-01.9, DCS-INT-01.10, DCS-INT-01.11, DCS-INT-01.12_

- [ ] 11. Implement DCS installer and uninstaller
  - Implement DCS variant detection (DCS, DCS.openbeta, DCS.openalpha)
  - Implement Export.lua backup and append logic
  - Implement FlightHubExport.lua deployment to Scripts/FlightHub/
  - Implement uninstaller with backup restoration
  - _Requirements: DCS-INT-01.1, DCS-INT-01.2, DCS-INT-01.3, DCS-INT-01.14_

- [ ] 11.1 Write unit tests for DCS installer
  - Test variant detection
  - Test Export.lua backup and append logic
  - Test uninstaller backup restoration
  - _Requirements: DCS-INT-01.1, DCS-INT-01.2, DCS-INT-01.3, DCS-INT-01.14_

- [ ] 12. Implement DCS Rust adapter
  - Create DcsAdapter struct with UDP socket
  - Implement JSON packet parsing
  - Implement Lua value → BusSnapshot field mapping with unit conversions
  - Implement nil handling for graceful degradation
  - Implement MP status annotation (mp_detected flag, no invalidation of self-aircraft data)
  - Implement connection timeout detection (2 seconds)
  - Implement aircraft change detection via unit type
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

- [ ] 14. Checkpoint - Ensure all adapter tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 15. Implement DirectInput FFB device abstraction
  - Create DirectInputFfbDevice struct with IDirectInputDevice8 interface
  - Implement device enumeration and connection
  - Implement capability querying (supports_pid, max_torque_nm, min_period_us)
  - Implement device acquisition and cooperative level setting
  - _Requirements: FFB-HID-01.1, FFB-HID-01.9_

- [ ] 15.1 Write unit tests for DirectInput device abstraction
  - Test device enumeration
  - Test capability querying
  - Test device acquisition
  - _Requirements: FFB-HID-01.1, FFB-HID-01.9_

- [ ] 16. Implement DirectInput FFB effect creation and management
  - Implement constant force effect creation for pitch and roll axes
  - Implement periodic (sine) effect creation for buffeting/vibration
  - Implement condition effects (spring/damper) for centering
  - Implement effect parameter updates via SetParameters
  - Implement effect start/stop control
  - _Requirements: FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4_

- [ ] 16.1 Write unit tests for FFB effect management
  - Test constant force effect creation
  - Test periodic effect creation
  - Test condition effect creation
  - Test effect parameter updates
  - _Requirements: FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4_

- [ ] 17. Implement XInput rumble integration
  - Create XInputRumbleDevice struct
  - Implement rumble channel mapping (low-freq and high-freq motors)
  - Document XInput limitations (vibration only, no directional torque)
  - _Requirements: FFB-HID-01.5_

- [ ] 18. Implement FFB safety envelope
  - Create FfbSafetyEnvelope struct with torque limits
  - Implement torque magnitude clamping to device max_torque_nm
  - Implement slew rate limiting (ΔNm/Δt ≤ configured limit)
  - Implement jerk limiting (Δ²Nm/Δt² ≤ configured limit)
  - Implement safe_for_ffb flag enforcement (zero torque when false)
  - Implement 50ms ramp-to-zero on fault with explicit fault timestamp tracking
  - _Requirements: FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4, FFB-SAFETY-01.6_

- [ ] 18.1 Write unit tests for FFB safety envelope
  - Test torque clamping
  - Test slew rate limiting
  - Test jerk limiting
  - Test safe_for_ffb enforcement
  - Test 50ms ramp-to-zero timing
  - _Requirements: FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4, FFB-SAFETY-01.6, SIM-TEST-01.10_

- [ ] 19. Implement FFB fault detection and handling
  - Create FaultDetector struct
  - Implement USB OUT stall detection (≥3 frames)
  - Implement NaN/Inf detection in FFB pipeline
  - Implement device health monitoring (over-temp, over-current)
  - Implement device disconnect detection (within 100ms)
  - Implement fault categorization (hardware-critical vs transient)
  - Implement fault state latching with power cycle requirement for hardware-critical faults
  - Implement explicit "clear fault" for transient faults
  - _Requirements: FFB-SAFETY-01.5, FFB-SAFETY-01.6, FFB-SAFETY-01.7, FFB-SAFETY-01.8, FFB-SAFETY-01.9, FFB-SAFETY-01.10, FFB-SAFETY-01.11_

- [ ] 19.1 Write unit tests for FFB fault detection
  - Test USB stall detection
  - Test NaN/Inf detection
  - Test device health monitoring
  - Test disconnect detection
  - Test fault categorization and latching
  - _Requirements: FFB-SAFETY-01.5, FFB-SAFETY-01.6, FFB-SAFETY-01.7, FFB-SAFETY-01.8, FFB-SAFETY-01.9, FFB-SAFETY-01.10, FFB-SAFETY-01.11, SIM-TEST-01.10_

- [ ] 20. Implement FFB blackbox recorder
  - Create BlackboxRecorder struct
  - Implement high-rate capture (≥250 Hz) of BusSnapshot, FFB setpoints, and device feedback
  - Implement 2-second pre-fault and 1-second post-fault buffering
  - Implement bounded, rotating log storage (size/age-limited)
  - _Requirements: FFB-SAFETY-01.12, FFB-SAFETY-01.13_

- [ ] 20.1 Write unit tests for blackbox recorder
  - Test high-rate capture
  - Test pre/post-fault buffering
  - Test bounded storage and rotation
  - _Requirements: FFB-SAFETY-01.12, FFB-SAFETY-01.13_

- [ ] 21. Implement FFB emergency stop
  - Implement UI button for emergency stop
  - Implement hardware button support (if device supports it)
  - Implement immediate FFB disable on emergency stop
  - _Requirements: FFB-SAFETY-01.14_

- [ ] 22. Checkpoint - Ensure all FFB safety tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 23. Implement Windows real-time thread configuration
  - Create WindowsRtThread struct
  - Implement thread priority elevation (THREAD_PRIORITY_TIME_CRITICAL)
  - Implement MMCSS registration with "Games" or "Pro Audio" task
  - Implement power throttling disable via PROCESS_POWER_THROTTLING_EXECUTION_SPEED
  - Implement QueryPerformanceCounter (QPC) monotonic clock
  - _Requirements: WIN-RT-01.1, WIN-RT-01.2, WIN-RT-01.3, WIN-RT-01.8_

- [ ] 23.1 Write unit tests for Windows RT thread configuration
  - Test thread priority elevation
  - Test MMCSS registration
  - Test power throttling disable
  - Test QPC monotonic clock
  - _Requirements: WIN-RT-01.1, WIN-RT-01.2, WIN-RT-01.3, WIN-RT-01.8, RT-TEST-01.1_

- [ ] 24. Implement Windows high-resolution timer loop
  - Implement CreateWaitableTimerEx with CREATE_WAITABLE_TIMER_HIGH_RESOLUTION
  - Implement fallback to timeBeginPeriod(1) if high-resolution timer unavailable
  - Implement 250 Hz tick loop with periodic timer
  - Implement final 50-80μs busy-spin using QPC
  - _Requirements: WIN-RT-01.4, WIN-RT-01.5_

- [ ] 24.1 Write unit tests for Windows timer loop
  - Test high-resolution timer creation
  - Test fallback to timeBeginPeriod(1)
  - Test 250 Hz tick timing
  - Test busy-spin final portion
  - _Requirements: WIN-RT-01.4, WIN-RT-01.5, RT-TEST-01.1_

- [ ] 25. Implement Windows power management integration
  - Implement PowerSetRequest with EXECUTION_REQUIRED and SYSTEM_REQUIRED when active
  - Implement power request clearing when idle
  - _Requirements: WIN-RT-01.6, WIN-RT-01.7_

- [ ] 25.1 Write unit tests for Windows power management
  - Test PowerSetRequest when active
  - Test power request clearing when idle
  - _Requirements: WIN-RT-01.6, WIN-RT-01.7, RT-TEST-01.6_

- [ ] 26. Implement Windows HID write optimization
  - Implement WriteFile with FILE_FLAG_OVERLAPPED for non-blocking I/O
  - Avoid HidD_SetOutputReport in hot path
  - _Requirements: WIN-RT-01.9, WIN-RT-01.10_

- [ ] 27. Implement Linux real-time thread configuration
  - Create LinuxRtThread struct
  - Implement SCHED_FIFO scheduling request via pthread_setschedparam
  - Implement rtkit D-Bus integration for privilege acquisition
  - Implement fallback to normal priority with warning
  - Implement mlockall(MCL_CURRENT | MCL_FUTURE) for memory locking
  - Implement RLIMIT_RTPRIO and RLIMIT_MEMLOCK validation
  - _Requirements: LINUX-RT-01.1, LINUX-RT-01.2, LINUX-RT-01.3, LINUX-RT-01.4, LINUX-RT-01.7_

- [ ] 27.1 Write unit tests for Linux RT thread configuration
  - Test SCHED_FIFO scheduling request
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

- [ ] 28.1 Write unit tests for Linux timer loop
  - Test clock_nanosleep with absolute times
  - Test 250 Hz tick timing
  - Test busy-spin final portion
  - _Requirements: LINUX-RT-01.5, LINUX-RT-01.6, RT-TEST-01.2_

- [ ] 29. Implement Linux RT metrics exposure
  - Expose metrics for RT scheduling status
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
  - _Requirements: RT-TEST-01.3, RT-TEST-01.4, RT-TEST-01.5_

- [ ] 32. Checkpoint - Ensure all runtime tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 33. Implement Windows MSI installer
  - Create WiX configuration for MSI package
  - Implement core binaries installation (per-user option)
  - Implement simulator integration components as opt-in features
  - Implement elevated privileges requirement for sim integrations and virtual drivers
  - Implement product posture statement display
  - Implement EULA summary display
  - _Requirements: PKG-01.1, PKG-01.3, PKG-01.4, PKG-01.5, PKG-01.6, PKG-01.7, PKG-01.8_

- [ ] 34. Implement Windows code signing
  - Create scripts/sign-binaries.ps1 for code signing
  - Implement signing for all EXE and DLL files
  - Implement MSI signing
  - Integrate signing into CI build process
  - _Requirements: PKG-01.1, PKG-01.2, PKG-01.3_

- [ ] 35. Implement Windows uninstaller
  - Implement binary removal
  - Implement X-Plane plugin removal (if installed)
  - Implement DCS Export.lua restoration (if backed up)
  - _Requirements: PKG-01.9_

- [ ] 36. Implement Linux package formats
  - Create Debian package (.deb) with control, rules, changelog, copyright files
  - Create udev rules for device access without root
  - Create postinst script for user group management and udev reload
  - Implement at least one additional format (AppImage or .rpm) as optional
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
  - _Requirements: Telemetry and Metrics section in design_

- [ ] 41.1 Write unit tests for metrics system
  - Test metric naming and scoping
  - Test metric type behavior
  - Test Prometheus export format
  - Test ring buffer retention

- [ ] 42. Implement cargo xtask validation commands
  - Implement cargo xtask validate-msfs-telemetry
  - Implement cargo xtask validate-xplane-telemetry
  - Implement cargo xtask validate-dcs-export
  - _Requirements: SIM-TEST-01.9_

- [ ] 43. Final Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

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
  - _Requirements: RT-TEST-01.8_

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
