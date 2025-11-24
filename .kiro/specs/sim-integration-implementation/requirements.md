# Simulator Integration Implementation Requirements

## Introduction

This specification defines the concrete implementation requirements for Flight Hub's simulator integration layer, force feedback protocol surface, OS-specific runtime scheduling, and packaging/distribution infrastructure. While the parent flight-hub spec establishes the high-level architecture and contracts, this spec provides the detailed technical requirements for implementing the actual bindings to MSFS SimConnect, X-Plane DataRefs/Plugin SDK, DCS Export.lua, HID/DirectInput FFB protocols, and platform-specific real-time scheduling primitives.

The goal is to transform the abstract adapter interfaces into production-ready implementations with well-defined connection models, data mappings, safety gates, and packaging requirements that respect each simulator's licensing constraints and ecosystem norms.

### Version Scope

This specification defines the target for Flight Hub v1:

**In-scope:**
- MSFS SimConnect adapter (read-only)
- X-Plane UDP adapter; plugin-based adapter planned but not required for v1
- DCS Export.lua adapter (self-telemetry only)
- Windows FFB output (DirectInput + OFP-1 raw torque)
- Windows real-time loop + basic Linux timing test harness

**Out-of-scope for v1 (MAY be partially prototyped):**
- X-Plane plugin for direct DataRef access
- Linux FFB output
- Kernel drivers of any kind

## Glossary

- **SimConnect**: Microsoft Flight Simulator's official SDK interface for external applications to read simulation variables and send events
- **SimVar**: Simulation variable exposed by MSFS via SimConnect (e.g., PLANE_PITCH_DEGREES, INDICATED_AIRSPEED)
- **DataRef**: X-Plane's named variable system for accessing flight model and aircraft state
- **Export.lua**: DCS World's Lua scripting hook for exporting telemetry data to external applications
- **HID PID**: USB HID Physical Interface Device usage page (0x0F) for force feedback device control
- **DirectInput FFB**: Windows API for force feedback effect management on game controllers
- **BusSnapshot**: Flight Hub's normalized telemetry structure that all sim adapters populate
- **MMCSS**: Windows Multimedia Class Scheduler Service for real-time thread priority management
- **rtkit**: Linux D-Bus service for acquiring real-time scheduling privileges without root
- **OFP-1**: Open Force Protocol v1, Flight Hub's raw torque streaming protocol for FFB devices

## Requirements

### Requirement 1: MSFS SimConnect Integration (MSFS-INT-01)

**User Story:** As a Flight Hub developer, I want a robust SimConnect adapter implementation, so that MSFS telemetry flows reliably into the normalized bus with proper connection management and safety gates.

#### Acceptance Criteria

1. WHEN connecting to MSFS THEN the adapter SHALL attempt local SimConnect connection without requiring SimConnect.cfg configuration
2. WHEN SimConnect is unavailable THEN the adapter SHALL retry connection with exponential backoff up to 30 seconds between attempts
3. WHEN registering data definitions THEN the adapter SHALL explicitly specify units for each SimVar to ensure consistent data interpretation
4. WHEN receiving telemetry THEN the adapter SHALL populate BusSnapshot fields using the mapping: PLANE_PITCH_DEGREES → attitude.pitch (converted to radians), PLANE_BANK_DEGREES → attitude.roll (radians), PLANE_HEADING_DEGREES_TRUE → attitude.yaw (radians)
5. WHEN receiving telemetry THEN the adapter SHALL map ROTATION_VELOCITY_BODY_X/Y/Z → angular_rates.p/q/r (rad/s), INDICATED_AIRSPEED → velocities.ias (m/s), VERTICAL_SPEED → velocities.vs (m/s)
6. WHEN receiving telemetry THEN the adapter SHALL map G_FORCE → kinematics.nz_g, INCIDENCE_ALPHA → aero.alpha (rad), INCIDENCE_BETA → aero.beta (rad)
7. WHEN receiving telemetry THEN the adapter SHALL target a minimum effective BusSnapshot update rate of 60 Hz with jitter p99 ≤ 10ms
8. WHEN sim state changes THEN the adapter SHALL implement a Sanity Gate with states: Disconnected, Booting, Loading, ActiveFlight, Paused, Faulted
9. WHEN in Sanity Gate state THEN the adapter SHALL only set BusSnapshot.safe_for_ffb = true in ActiveFlight state
10. WHEN telemetry values are NaN or Inf THEN the adapter SHALL mark those fields as invalid, transition to Faulted on repeated violations (configurable threshold), and log at most once per 5 seconds
11. WHEN attitude or velocity values change by more than physically plausible amounts in one frame THEN the adapter SHALL drop that packet and increment a sanity_violation counter
12. WHEN aircraft changes THEN the adapter SHALL detect via TITLE SimVar and trigger profile switching within 500ms
13. WHEN implementing THEN the adapter SHALL be read-only with no event injection to minimize legal and safety risk
14. WHEN connection is lost THEN the adapter SHALL mark all BusSnapshot fields as invalid and transition to Disconnected state

#### Documentation

1. WHEN documenting THEN the adapter SHALL maintain a mapping table in docs/ listing each SimVar → BusSnapshot field mapping with units
2. WHEN implementing unit conversions THEN the adapter SHALL document conversions in code comments (degrees to radians, feet to meters, knots to m/s)

### Requirement 2: X-Plane Integration (XPLANE-INT-01)

**User Story:** As a Flight Hub developer, I want X-Plane integration via UDP data output initially and plugin SDK for future enhancement, so that X-Plane users can use Flight Hub without complex setup.

#### Acceptance Criteria

1. WHEN implementing THEN the adapter SHALL support UDP data output mode where users configure X-Plane's "Data Output" screen to send to Flight Hub's listening port
2. WHEN receiving UDP packets THEN the adapter SHALL parse the DATA packet format with 36-byte records per data group (4-byte index + 8×4-byte floats)
3. WHEN mapping data groups THEN the adapter SHALL support: group 3 (speeds: IAS, TAS, GS), group 4 (Mach, VVI, g-load), group 16 (angular velocities P/Q/R), group 17 (pitch/roll/heading), group 18 (alpha/beta), group 21 (body velocities)
4. WHEN mapping data groups THEN the adapter SHALL convert: group 17 angles from degrees to ValidatedAngle, group 16 rates from deg/s to rad/s, group 3 speeds from knots to ValidatedSpeed
5. WHEN mapping DataRefs THEN the adapter SHALL explicitly document unit conversions per DataRef in code comments and mapping docs, using BusSnapshot typed fields
6. WHEN a DataRef is missing from UDP output THEN the adapter SHALL gracefully handle missing groups without crashing
7. WHEN implementing UDP-only mode THEN the adapter SHALL document that aircraft identity may be unavailable or inferred poorly, with true identity-based profile switching requiring the plugin
8. WHEN implementing future plugin THEN it SHALL register a flight loop callback via XPLMRegisterFlightLoopCallback at maximum rate (period = 0)
9. WHEN implementing future plugin THEN the flight loop SHALL read required DataRefs, write to lock-free queue, and return quickly with no blocking I/O or allocations
10. WHEN implementing future plugin THEN it SHALL communicate with Flight Hub via UDP or named pipe with binary packet format containing version header and timestamp
11. WHEN implementing future plugin THEN the plugin adapter SHALL populate the same BusSnapshot fields as the UDP adapter and SHALL be a drop-in replacement at the bus boundary
12. WHEN aircraft changes THEN the adapter SHALL detect via aircraft path/name (plugin mode) or heuristics (UDP mode) and trigger profile switching
13. WHEN connection is lost or no packets received for 2 seconds THEN the adapter SHALL mark BusSnapshot as invalid and transition to disconnected state
14. WHEN implementing THEN the adapter SHALL provide web API integration for querying X-Plane state via HTTP endpoints

#### Documentation

1. WHEN documenting THEN the adapter SHALL maintain a mapping table in docs/integration/xplane.md listing each data group index → BusSnapshot field mapping with units
2. WHEN documenting THEN the system SHALL provide setup instructions for configuring X-Plane's Data Output screen with required indices and rates

### Requirement 3: DCS World Integration (DCS-INT-01)

**User Story:** As a Flight Hub developer, I want a DCS Export.lua integration that respects multiplayer integrity checks and community norms, so that DCS users can safely use Flight Hub in both single-player and multiplayer environments.

#### Acceptance Criteria

1. WHEN installing THEN the installer SHALL detect all installed DCS variants (DCS, DCS.openbeta, DCS.openalpha) under Saved Games and offer per-variant installation
2. WHEN installing THEN the system SHALL check for existing Export.lua in Saved Games\DCS\Scripts\ and append a dofile reference rather than overwriting
3. WHEN no Export.lua exists THEN the installer SHALL create a minimal one that dofiles Flight Hub's script and preserves compatibility with future tools
4. WHEN implementing the export script THEN it SHALL define LuaExportStart, LuaExportStop, and LuaExportAfterNextFrame hooks
5. WHEN exporting data THEN the script SHALL use only self-aircraft functions: LoGetSelfData, LoGetIndicatedAirSpeed, LoGetAccelerationUnits, LoGetAngleOfAttack, LoGetTrueAirSpeed, LoGetAltitudeAboveGroundLevel
6. WHEN exporting data THEN the adapter SHALL normalize all values to the canonical BusSnapshot typed fields and document any non-obvious unit conversions in code comments
7. WHEN export functions return nil THEN the script SHALL handle gracefully by marking fields invalid without spamming logs or crashing
8. WHEN in multiplayer with integrity check enabled THEN the adapter SHALL annotate MP status via mp_detected flag but SHALL NOT invalidate self-aircraft telemetry
9. WHEN MP restrictions prevent access to world objects THEN the adapter SHALL continue exporting self-aircraft data (attitude, velocities, g-load, IAS/TAS, AoA) which are allowed by integrity check
10. WHEN exporting data THEN the script SHALL NOT export world objects, RWR data, or tactical information that could provide unfair advantage
11. WHEN sending data THEN the script SHALL use non-blocking UDP to localhost with target rate of 60Hz via LuaExportActivityNextEvent
12. WHEN aircraft changes THEN the adapter SHALL detect via unit type (self_data.Name) and trigger profile switching
13. WHEN uninstalling THEN the system SHALL restore the backed-up original Export.lua if one existed (with .flighthub_backup extension)
14. WHEN connection is lost or no packets received for 2 seconds THEN the adapter SHALL mark BusSnapshot as invalid and log the disconnection
15. WHEN implementing the Lua script THEN it SHALL properly chain to existing Export.lua hooks by storing previous hook functions and calling them before/after Flight Hub logic

#### Documentation

1. WHEN documenting THEN the adapter SHALL maintain a mapping table in docs/integration/dcs.md listing each Lua API function → BusSnapshot field mapping with units
2. WHEN documenting THEN the system SHALL maintain MP integrity check compliance documentation explaining which data is exported and why it's allowed

### Requirement 4: Normalized Telemetry Bus (BUS-01)

**User Story:** As a Flight Hub developer, I want a canonical BusSnapshot structure with clear unit conventions, so that all simulator adapters feed a consistent data model to the FFB and profile systems.

#### Acceptance Criteria

1. WHEN defining BusSnapshot THEN it SHALL include: sim (SimId enum), aircraft (AircraftId with ICAO and variant), timestamp (monotonic nanoseconds)
2. WHEN defining kinematics THEN it SHALL use validated types: ValidatedSpeed for ias/tas/ground_speed, ValidatedAngle for aoa/sideslip/bank/pitch/heading, GForce for g-forces, Mach for mach number
3. WHEN defining kinematics THEN it SHALL include: vertical_speed (feet per minute), g_force (vertical), g_lateral, g_longitudinal
4. WHEN defining aircraft configuration THEN it SHALL include: GearState (per-gear positions), flaps/spoilers (Percentage), AutopilotState, ap_altitude/ap_heading/ap_speed targets, LightsConfig, fuel (HashMap per tank)
5. WHEN defining helicopter data THEN it SHALL include optional HeloData: nr/np/torque/collective (Percentage), pedals (-100 to 100)
6. WHEN defining engine data THEN it SHALL include Vec<EngineData>: index, running, rpm, manifold_pressure, egt, cht, fuel_flow, oil_pressure, oil_temperature
7. WHEN defining environment THEN it SHALL include: altitude/pressure_altitude (feet), oat (Celsius), wind_speed/wind_direction, visibility (statute miles), cloud_coverage
8. WHEN defining navigation THEN it SHALL include: latitude/longitude (degrees), ground_track, distance_to_dest/time_to_dest, active_waypoint
9. WHEN using typed values THEN the system SHALL enforce validation at construction: Percentage (0-100), GForce (-20 to 20), Mach (0-5), ValidatedSpeed (0-1000 knots or 0-500 m/s), ValidatedAngle (-180 to 180 degrees or -π to π radians)
10. WHEN normalizing units THEN adapters SHALL use the validated types which handle unit conversions: ValidatedSpeed.to_knots(), ValidatedAngle.to_degrees()
11. WHEN defining coordinate frames THEN all adapters SHALL populate fields using standard aerospace conventions: bank/pitch/heading for attitude, positive values per standard definitions
12. WHEN the bus schema evolves THEN it SHALL use additive-only changes with version field to maintain backward compatibility
13. WHEN validating snapshots THEN the system SHALL check: unique engine indices, helicopter pedals in range (-100 to 100), all typed fields within their defined ranges
14. WHEN querying snapshot age THEN the system SHALL provide age_ms() method returning milliseconds since snapshot creation

#### Documentation

1. WHEN documenting THEN the BusSnapshot structure SHALL be fully documented in docs/ with field definitions, units, coordinate frames, and sign conventions
2. WHEN implementing THEN the system SHALL use Rust's type system to enforce unit safety and validation at compile time where possible

### Requirement 5: Force Feedback HID Protocol (FFB-HID-01)

**User Story:** As a Flight Hub developer, I want to implement DirectInput FFB on Windows and evdev FFB on Linux, so that force feedback devices receive properly formatted effects without requiring kernel drivers.

#### Acceptance Criteria

1. WHEN implementing Windows FFB THEN the system SHALL use DirectInput 8 (IDirectInputDevice8) for effect creation and management
2. WHEN creating effects THEN the system SHALL support: constant force for sustained loads, periodic (sine) for buffeting/vibration, condition effects (spring/damper) for centering
3. WHEN implementing effects THEN the system SHALL use IDirectInputDevice8::CreateEffect with appropriate DIEFFECT structures specifying magnitude, duration, and envelope
4. WHEN implementing effects THEN the system SHALL use IDirectInputEffect::Start/Stop/SetParameters for runtime control
5. WHEN presenting a virtual XInput controller THEN the system SHALL map its two rumble channels into the FFB synthesis pipeline as coarse vibration inputs only and SHALL NOT attempt to model full stick torque through XInput
6. WHEN implementing Linux FFB THEN the system SHOULD use evdev FF_* ioctls on /dev/input/event* devices (MAY be deferred to v2)
7. WHEN implementing Linux FFB THEN the system SHOULD upload effects via EVIOCSFF and trigger via EV_FF events (MAY be deferred to v2)
8. WHEN implementing OFP-1 raw torque mode THEN the system SHALL stream constant force updates at 500-1000Hz for devices that support it
9. WHEN a device connects THEN the system SHALL query capabilities to determine: supports_pid, supports_raw_torque, max_torque_nm, min_period_us, has_health_stream
10. WHEN selecting FFB mode THEN the system SHALL prefer: DirectInput pass-through where sims implement rich FFB, raw torque when device supports OFP-1, telemetry synthesis as fallback
11. WHEN implementing THEN the system SHALL target Windows for FFB output with Linux support as optional enhancement
12. WHEN implementing THEN the system SHALL stay entirely in user-mode with no kernel driver requirements

### Requirement 6: FFB Safety Envelope (FFB-SAFETY-01)

**User Story:** As a Flight Hub developer, I want comprehensive FFB safety checks and fault handling, so that force feedback never causes harm even when telemetry is invalid or devices malfunction.

#### Acceptance Criteria

1. WHEN BusSnapshot.safe_for_ffb is false THEN the FFB engine SHALL output zero torque regardless of telemetry values
2. WHEN torque magnitude exceeds device max_torque_nm THEN the system SHALL clamp to safe limits
3. WHEN configuring torque limits THEN the configured slew and jerk limits SHALL be bounded by device-rated capabilities and SHALL default to conservative values documented per device
4. WHEN changing torque setpoints THEN the system SHALL rate-limit changes to prevent steps: ΔNm/Δt ≤ configured slew rate, Δ²Nm/Δt² ≤ configured jerk limit
5. WHEN USB OUT stall is detected for ≥3 frames THEN the system SHALL ramp torque to zero within 50ms, emit audible cue, and latch to SafeTorque state
6. WHEN NaN or Inf appears in FFB pipeline THEN the system SHALL trigger fault handler and ramp to zero within 50ms
7. WHEN device reports over-temp or over-current THEN the system SHALL immediately disable FFB and latch fault state
8. WHEN device disconnects THEN the system SHALL detect within 100ms and ensure outputs were ramped to safe within 50ms
9. WHEN in SafeTorque/Faulted state THEN the system SHALL stop issuing any non-zero torque commands, continue to process inputs and telemetry for UI/debugging, and surface a latched fault indicator to UI/telemetry until power cycle or explicit "clear fault" command
10. WHEN fault occurs THEN the blackbox recorder SHALL capture at least: BusSnapshot at ≥250 Hz, FFB setpoints and actual device feedback, for 2 seconds before and 1 second after the fault trigger
11. WHEN in SafeTorque state THEN the system SHALL require power cycle to clear latched faults and re-enable high-torque mode
12. WHEN implementing emergency stop THEN the system SHALL provide both UI button and hardware button (if supported) to immediately disable FFB

### Requirement 7: Windows Runtime Scheduling (WIN-RT-01)

**User Story:** As a Flight Hub developer, I want to use Windows real-time scheduling APIs correctly, so that the axis and FFB loops meet their timing guarantees on Windows systems.

#### Acceptance Criteria

1. WHEN creating the RT axis thread THEN the system SHALL set thread priority using SetThreadPriority with THREAD_PRIORITY_HIGHEST or THREAD_PRIORITY_TIME_CRITICAL
2. WHEN creating RT threads THEN the system SHALL register with MMCSS via AvSetMmThreadCharacteristics using task name "Games" or "Pro Audio"
3. WHEN the process starts THEN the system SHALL disable power throttling via PROCESS_POWER_THROTTLING_EXECUTION_SPEED flag
4. WHEN implementing the tick loop THEN the runtime SHALL use high-resolution timers for its 250 Hz loop; implementations SHOULD prefer CreateWaitableTimerEx with CREATE_WAITABLE_TIMER_HIGH_RESOLUTION but MAY call timeBeginPeriod(1) as a fallback on systems where empirical testing shows unacceptable jitter without it
5. WHEN finishing each tick THEN the system SHALL busy-spin for the final 50-80μs using QueryPerformanceCounter to minimize jitter
6. WHEN at least one sim is connected and FFB device is active THEN the system SHALL call PowerSetRequest with EXECUTION_REQUIRED and SYSTEM_REQUIRED to prevent sleep
7. WHEN idle with no active sim or FFB THEN the system SHALL clear power requests to allow normal power management
8. WHEN measuring timing THEN the system SHALL use QueryPerformanceCounter (QPC) as the monotonic clock source
9. WHEN implementing HID writes THEN the system SHALL use WriteFile on handles opened with FILE_FLAG_OVERLAPPED for non-blocking I/O
10. WHEN implementing HID writes THEN the system SHALL avoid HidD_SetOutputReport in the hot path due to performance characteristics

### Requirement 8: Linux Runtime Scheduling (LINUX-RT-01)

**User Story:** As a Flight Hub developer, I want to use Linux real-time scheduling correctly with proper fallbacks, so that Flight Hub works well on Linux systems with and without RT privileges.

#### Acceptance Criteria

1. WHEN creating RT threads THEN the system SHOULD request SCHED_FIFO scheduling policy via pthread_setschedparam with priority in range 1-49
2. WHEN RT scheduling is unavailable THEN the system SHOULD attempt to acquire privileges via rtkit D-Bus interface
3. WHEN rtkit is unavailable or denies request THEN the system SHALL fall back to normal priority and warn user about potential jitter
4. WHEN running with RT priority THEN the system SHOULD call mlockall(MCL_CURRENT | MCL_FUTURE) to prevent page faults in RT threads
5. WHEN implementing the tick loop THEN the system SHOULD use clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME) with absolute target times
6. WHEN finishing each tick THEN the system SHOULD busy-spin for the final portion using clock_gettime(CLOCK_MONOTONIC) to minimize jitter
7. WHEN starting THEN the system SHALL validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK limits and warn if insufficient
8. WHEN RT scheduling fails THEN the system SHALL expose metrics so users can verify if timing is acceptable at normal priority
9. WHEN implementing HID writes THEN the system SHOULD use non-blocking hidraw writes with write coalescing and error recovery
10. WHEN documenting THEN the system SHALL provide /etc/security/limits.conf entries needed for RT operation and ship a helper script to apply them
11. WHEN implementing v1 THEN Linux RT support MAY be limited to a timing test harness and non-FFB input loop; full parity with Windows FFB loops is deferred

### Requirement 9: Packaging and Code Signing (PKG-01)

**User Story:** As a Flight Hub developer, I want proper packaging and code signing infrastructure, so that users can install and trust Flight Hub binaries on their systems.

#### Acceptance Criteria

1. WHEN distributing Windows binaries THEN all EXE and DLL files SHALL be signed with an OV or EV code signing certificate
2. WHEN building releases THEN CI SHALL automatically sign all binaries before packaging
3. WHEN creating Windows installer THEN it SHALL be packaged as MSI and signed with the same certificate
4. WHEN installing on Windows THEN the installer SHALL: install main EXE and libraries, optionally deploy X-Plane plugin to configured path, optionally install DCS Export.lua shim with backup of original
5. WHEN installing THEN the installer SHALL display product posture statement and EULA summary
6. WHEN installing THEN sim integration components SHALL be opt-in toggles, not installed by default
7. WHEN uninstalling on Windows THEN the uninstaller SHALL: remove installed binaries, remove X-Plane plugin if installed, restore backed-up Export.lua if it existed
8. WHEN distributing Linux binaries THEN the system SHALL provide AppImage, .deb, and .rpm packages
9. WHEN installing on Linux THEN packages SHALL include udev rules for device access without root
10. WHEN documenting Linux install THEN instructions SHALL cover: adding user to required groups (input, plugdev), optional RT priority configuration via limits.conf
11. WHEN implementing THEN the system SHALL NOT ship custom kernel-mode drivers in v1; where virtual devices are required on Windows, the system MAY depend on third-party signed drivers (e.g., ViGEmBus), provided their licenses are included in installer documentation

#### Documentation

1. WHEN distributing THEN the project SHALL maintain a third-party components inventory (licenses, versions, and usage) for all bundled libraries and drivers, and SHALL ship required license texts with the product

### Requirement 10: Legal and Licensing Compliance (LEGAL-01)

**User Story:** As a Flight Hub product owner, I want clear legal guardrails and compliance documentation, so that Flight Hub respects simulator EULAs and operates within established ecosystem norms.

#### Acceptance Criteria

1. WHEN documenting product positioning THEN it SHALL state: "Flight Hub is an accessory/input manager that requires MSFS/X-Plane/DCS; it does not emulate or replace any simulator"
2. WHEN implementing MSFS integration THEN the system SHALL NOT: bypass DRM, modify simulator binaries, train ML models on MSFS data, create competing simulator products
3. WHEN implementing X-Plane integration THEN the system SHALL NOT: bundle Laminar assets, claim to be official X-Plane software, modify X-Plane binaries
4. WHEN implementing DCS integration THEN the system SHALL NOT: modify DCS core files, export tactical data that provides unfair advantage, enable automation that triggers weapons
5. WHEN collecting telemetry data THEN it SHALL be runtime-only by default with logs ring-buffered and local
6. WHEN implementing analytics THEN it SHALL be opt-in only with clear data export and delete options
7. WHEN documenting THEN the system SHALL maintain a "What We Touch" document per sim listing: exact files modified, ports used, variables accessed, how to revert changes
8. WHEN used for training THEN documentation SHALL state "not for certified training devices" unless formal certification is pursued
9. WHEN implementing THEN the system SHALL use only official SDK interfaces: SimConnect for MSFS, plugin SDK/UDP for X-Plane, Export.lua for DCS
10. WHEN distributing THEN the system SHALL include product posture document referenced in: README, website, installer, and integration module comments

### Requirement 11: Simulator Adapter Testing (SIM-TEST-01)

**User Story:** As a Flight Hub developer, I want comprehensive adapter testing infrastructure, so that simulator integrations are validated against known-good data and edge cases.

#### Acceptance Criteria

1. WHEN testing adapters THEN each SHALL have a unit test suite that feeds known raw values and asserts correct BusSnapshot content
2. WHEN testing MSFS adapter THEN tests SHALL verify: unit conversions (degrees to radians, feet to meters, knots to m/s), state machine transitions, sanity gate behavior with NaN/Inf values
3. WHEN testing X-Plane adapter THEN tests SHALL verify: UDP packet parsing, DataRef mapping, handling of missing DataRefs
4. WHEN testing DCS adapter THEN tests SHALL verify: Lua value parsing, nil handling, MP-safe mode restrictions
5. WHEN implementing integration tests THEN the system SHALL use recorded telemetry fixtures from each sim for replay testing
6. WHEN implementing fixtures THEN each simulator adapter's unit tests SHALL include at least one recorded fixture file per sim version family (e.g., MSFS 2020, X-Plane 12, DCS stable) stored under tests/fixtures
7. WHEN testing connection handling THEN tests SHALL verify: reconnection with exponential backoff, graceful handling of sim process start/stop, proper cleanup on disconnect
8. WHEN testing sanity gates THEN tests SHALL inject: NaN/Inf values, physically implausible jumps, rapid state transitions
9. WHEN implementing cmd: tests THEN the system SHALL provide: cargo xtask validate-msfs-telemetry, cargo xtask validate-xplane-telemetry, cargo xtask validate-dcs-export
10. WHEN testing FFB safety THEN tests SHALL verify: torque clamping, rate limiting, fault detection and response, safe ramp-down on disconnect

#### Documentation

1. WHEN testing THEN each adapter SHALL have a mapping table document checked into docs/ listing source variable → BusSnapshot field mappings

### Requirement 12: Platform Runtime Testing (RT-TEST-01)

**User Story:** As a Flight Hub developer, I want runtime scheduling and timing validation, so that real-time performance is verified on both Windows and Linux.

#### Acceptance Criteria

1. WHEN testing Windows RT THEN tests SHALL verify: MMCSS registration succeeds, thread priority is elevated, power throttling is disabled, QPC provides monotonic timestamps
2. WHEN testing Linux RT THEN tests SHOULD verify: SCHED_FIFO acquisition (when permitted), mlockall succeeds, clock_nanosleep provides accurate timing, fallback to normal priority works
3. WHEN measuring jitter THEN tests SHALL run for ≥10 minutes and compute p99 of tick interval error with first 5s warm-up discarded
4. WHEN testing on hardware-backed CI runners THEN the system SHALL verify 250Hz axis loop achieves p99 jitter ≤0.5ms; on virtualized runners, the jitter test MAY report metrics without failing the build
5. WHEN testing HID writes on hardware-backed CI runners THEN the system SHALL measure and verify p99 latency ≤300μs; when a tagged hardware runner is unavailable, the test MAY be skipped
6. WHEN testing power management THEN tests SHALL verify: PowerSetRequest is called when active, requests are cleared when idle, system can sleep when Flight Hub is idle
7. WHEN testing FFB timing THEN tests SHALL verify: torque ramp to zero completes within 50ms on fault, effect updates occur at target rate (500-1000Hz for raw torque)
8. WHEN implementing soak tests THEN the system SHALL run 24-48h with synthetic telemetry and verify: zero missed ticks, RSS delta <10%, no memory leaks
9. WHEN testing THEN CI SHALL include quality gates on hardware-backed runners that fail builds if: 250Hz p99 jitter >0.5ms, HID write p99 >300μs, any blackbox drops in 10-minute capture
10. WHEN testing cross-platform THEN the system SHALL run timing tests on both Intel and AMD processors to catch platform-specific issues

## CI Quality Gates

The following quality gates are enforced as build requirements:

**QG-SIM-MAPPING (MUST):** Fail if any simulator adapter lacks complete mapping table documentation.

**QG-UNIT-CONV (MUST):** Fail if unit conversion tests don't cover all BusSnapshot fields populated by each adapter.

**QG-SANITY-GATE (MUST):** Fail if sanity gate tests don't inject NaN/Inf and verify proper handling.

**QG-FFB-SAFETY (MUST):** Fail if FFB safety tests don't verify torque ramp-down within 50ms on all fault types.

**QG-RT-JITTER (MUST):** Fail if 250Hz p99 jitter >0.5ms on CI runners.

**QG-HID-LATENCY (MUST):** Fail if HID write p99 >300μs on physical test hardware.

**QG-LEGAL-DOC (MUST):** Fail if product posture document is not present or not referenced in required locations.
