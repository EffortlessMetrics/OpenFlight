# Flight Hub Requirements Document

## Introduction

Flight Hub is a comprehensive PC flight simulation input management system that provides a unified control plane for flight controls, panels, and force feedback devices across multiple simulators. The system features a deterministic 250Hz axis processing pipeline, per-aircraft auto-profiles, rule-based panel management, and robust diagnostics capabilities. It aims to eliminate the complexity of managing multiple input devices and configurations across different flight simulators while maintaining real-time performance and safety standards.

## Definitions & Measurements

**Def-01 Latency (MUST):** "Processing latency" is input sample timestamp → HID OUT write completion measured with a monotonic clock; exclude device firmware time.

**Def-02 Jitter p99 (MUST):** p99 of tick interval error over ≥10 minutes, discarding the first 5s warm-up.

**Def-03 Missed tick (MUST):** A tick whose start time slips >1.5× the nominal period (≥6ms at 250Hz). Missed ticks are counted and logged.

**Def-04 Fault (MUST):** Any of {USB OUT stall ≥3 frames, endpoint error, encoder invalid/NaN, over-temp/current, plugin overrun}.

**Def-05 Session (MUST):** Process lifetime or until device power cycle; high-torque consent resets on power cycle.

## Requirements

### Requirement 1: Real-Time Axis Processing (AX-01)

**User Story:** As a flight simulation enthusiast, I want consistent and responsive control input processing across all my flight controls, so that I can have precise aircraft control without latency or jitter issues.

#### Acceptance Criteria

1. WHEN the system processes axis inputs THEN the processing latency SHALL be ≤ 5ms p99
2. WHEN the axis scheduler runs THEN the jitter SHALL be ≤ 0.5ms p99 at 250Hz
3. WHEN processing axis data THEN the system SHALL use zero allocations and locks on the hot path
4. WHEN an axis tick is processed THEN the processing time SHALL be ≤ 0.5ms p99
5. IF the system detects missed ticks THEN it SHALL log the event and maintain stability
6. WHEN a profile is applied THEN the system SHALL compile the pipeline off-thread and swap at a tick boundary with an acknowledgment; partial applies are disallowed
7. WHEN given identical inputs THEN the pipeline SHALL produce the same effective profile hash and outputs within FP tolerance
8. WHEN measuring jitter THEN it SHALL be computed over a continuous ≥10-minute run with warm-up excluded per Def-02

### Requirement 2: Multi-Simulator Support (GI-01)

**User Story:** As a pilot who uses multiple flight simulators, I want seamless integration with MSFS, X-Plane, and DCS, so that I can use the same control setup across all my preferred simulators.

#### Acceptance Criteria

1. WHEN connecting to MSFS THEN the system SHALL use SimConnect and Input Events for communication
2. WHEN connecting to X-Plane THEN the system SHALL use DataRefs via UDP or plugin interface
3. WHEN connecting to DCS THEN the system SHALL use user-installed Export.lua with minimal footprint
4. WHEN switching between simulators THEN aircraft profiles SHALL auto-apply within 500ms
5. WHEN configuring a simulator THEN the system SHALL apply versioned, table-driven JSON diffs with golden tests and a one-click rollback
6. WHEN sim updates occur THEN the system SHALL offer a Verify/Repair matrix that evaluates expected behaviors and applies minimum diffs to restore function
7. WHEN publishing support THEN a coverage matrix SHALL list which variables/events per sim are normalized; CI SHALL reject regressions

### Requirement 3: Device Management and Hot-Plug Support (DM-01)

**User Story:** As a user with multiple flight control devices, I want automatic device detection and stable operation when devices are connected or disconnected, so that I don't have to restart the system or reconfigure settings.

#### Acceptance Criteria

1. WHEN a device is connected THEN it SHALL be detected and classified within 300ms
2. WHEN a device is disconnected THEN it SHALL be detected within 100ms and outputs stopped safely
3. WHEN devices are reordered or moved between USB ports THEN bindings SHALL remain stable using VID/PID/serial/path
4. WHEN calibrating devices THEN axis min/max/center and detent zones SHALL persist across restarts
5. IF device health data is available THEN it SHALL be monitored at 10-20Hz
6. WHEN disconnect is detected THEN outputs SHALL ramp to safe within 50ms
7. WHEN testing identity stability THEN bindings SHALL remain stable across USB hubs/ports using VID/PID/serial/device path with conformance testing

### Requirement 4: Force Feedback Safety and Control (FFB-01)

**User Story:** As a user with force feedback devices, I want safe and controlled force feedback operation with proper safety interlocks, so that I can enjoy realistic feedback without risk of injury or equipment damage.

#### Acceptance Criteria

1. WHEN force feedback is enabled THEN it SHALL require both UI consent and physical button combination
2. WHEN a fault is detected (USB stall, NaN, over-temp) THEN torque SHALL ramp to zero within 50ms
3. WHEN a fault occurs THEN an audible cue SHALL be provided and the state latched
4. WHEN using non-FFB devices THEN force-trim SHALL provide recentre illusion without fighting spring
5. IF high torque mode is active THEN it SHALL persist until power cycle
6. WHEN a force-feedback device connects THEN the system SHALL negotiate DirectInput pass-through, raw-torque protocol (if supported), or telemetry-synth per device/sim policy
7. WHEN unlocking high-torque THEN it SHALL require UI consent and device button-combo ACK (hold two inputs ≥2s), validated with rolling token
8. WHEN changing FFB setpoints THEN they SHALL be rate-limited to avoid torque steps; non-FFB trim-hold SHALL freeze springs and re-enable with ramp

### Requirement 5: Profile Management and Aircraft Auto-Switching (PRF-01)

**User Story:** As a pilot who flies different aircraft types, I want automatic profile switching based on the aircraft I'm flying, so that each aircraft has appropriate control curves and settings without manual intervention.

#### Acceptance Criteria

1. WHEN an aircraft change is detected THEN the appropriate profile SHALL be applied within 500ms
2. WHEN profiles are merged THEN the hierarchy SHALL be Global → Sim → Aircraft → Phase of Flight
3. WHEN profile conflicts exist THEN the system SHALL use last-writer-wins for scalars and keyed-merge for arrays
4. WHEN profiles are validated THEN JSON Schema flight.profile/1 SHALL be enforced with line/column errors
5. IF Phase of Flight changes THEN overrides SHALL apply with hysteresis to prevent flapping
6. WHEN validating curves THEN profile curves SHALL be monotonic; non-monotonic definitions are rejected with line/column schema errors
7. WHEN detecting conflicts THEN if sim curves/vendor tools are active, the system SHALL present one-click disable path or gain compensation
8. WHEN merging profiles THEN it SHALL be deterministic: scalars last-writer-wins; arrays keyed-merge by identity fields with documented tie-breaks

### Requirement 6: Panel and StreamDeck Integration (PNL-01)

**User Story:** As a user with flight panels and StreamDeck devices, I want rule-based LED and display control that responds to flight conditions, so that my panels provide accurate flight status information.

#### Acceptance Criteria

1. WHEN flight conditions change THEN panel LEDs SHALL respond within 20ms
2. WHEN rules are compiled THEN they SHALL use IF/THEN logic with hysteresis to prevent flicker
3. WHEN panels are verified THEN the system SHALL show pass/fail status and offer repair options
4. WHEN using StreamDeck THEN it SHALL integrate via local API with sample profiles for GA/Airbus/Helo
5. IF panel configuration drifts THEN Verify/Repair SHALL detect and fix the issues
6. WHEN evaluating rules THEN the engine SHALL allocate zero memory at runtime (post-compile)
7. WHEN using StreamDeck THEN the plugin SHALL document supported app versions and degrade gracefully when out-of-range

### Requirement 7: Diagnostics and Blackbox Recording (DIAG-01)

**User Story:** As a user experiencing issues or a support technician, I want comprehensive diagnostic capabilities and flight data recording, so that problems can be quickly identified and resolved.

#### Acceptance Criteria

1. WHEN recording THEN captures SHALL use binary format with header (FBB1, endian, app ver, timebase, sim/aircraft IDs), streams (A: 250Hz axis frames; B: 60Hz bus snapshots; C: events), index every 100ms, and CRC32C footer
2. WHEN recording on SSD THEN the recorder SHALL sustain ≥10 minutes with zero drops; drop count >0 fails acceptance
3. WHEN replaying a capture THEN it SHALL reproduce engine outputs within FP tolerance
4. WHEN generating support bundles THEN they SHALL include .fbb, logs, profiles, and device maps under 30MB
5. WHEN errors occur THEN they SHALL have stable error codes linked to knowledge base articles
6. WHEN tracing is enabled THEN ETW/tracepoints SHALL include TickStart/End, HidWrite, DeadlineMiss; enabling SHALL not affect RT numbers per NFR-01

### Requirement 8: Security and Privacy (SEC-01)

**User Story:** As a security-conscious user, I want the system to operate with minimal permissions and no unauthorized data collection, so that my privacy is protected and my system remains secure.

#### Acceptance Criteria

1. WHEN the system operates THEN IPC SHALL be local-only using Pipes/UDS with OS ACLs
2. WHEN plugins are loaded THEN WASM plugins SHALL be sandboxed with no file/network access by default
3. WHEN native plugins run THEN they SHALL execute in isolated helper processes with watchdog protection
4. WHEN analytics are collected THEN it SHALL require explicit user opt-in with data export/delete options
5. IF binaries are distributed THEN they SHALL be signed and signature status shown in UI
6. WHEN integrating with sims THEN the system SHALL not inject code into sim processes; all integration uses SimConnect/DataRefs/Export.lua per sim policy
7. WHEN starting IPC THEN it SHALL bind only to local Pipes/UDS; no network listeners are started unless explicitly enabled
8. WHEN displaying plugins THEN the UI SHALL display plugin signature state (signed/unsigned) and capability manifest

### Requirement 9: Cross-Platform Support (XPLAT-01)

**User Story:** As a user on Windows or Linux, I want native performance and proper OS integration, so that the system works optimally on my preferred operating system.

#### Acceptance Criteria

1. WHEN running on Windows THEN it SHALL use HID/raw input, MMCSS "Games", and waitable timers
2. WHEN running on Linux THEN it SHALL use hidraw + udev, SCHED_FIFO via rtkit, and clock_nanosleep
3. WHEN packaging THEN it SHALL provide MSI for Windows and systemd user units for Linux
4. WHEN updating THEN it SHALL use signed delta updates with rollback capability
5. IF virtual devices are needed THEN they SHALL be opt-in with proper rollback support
6. WHEN optimizing performance THEN on Windows, process power-throttling SHALL be disabled and MMCSS "Games" used for RT threads; on Linux, mlockall and SCHED_FIFO via rtkit SHALL be required for RT operation
7. WHEN running after install THEN the service SHALL run without admin/root; Linux uses systemd user; Windows uses user-level startup

### Requirement 10: Plugin System and Extensibility (PLUG-01)

**User Story:** As a developer or advanced user, I want to extend the system with custom plugins, so that I can add specialized functionality while maintaining system stability and security.

#### Acceptance Criteria

1. WHEN WASM plugins execute THEN they SHALL run at 20-120Hz with capability manifests
2. WHEN native plugins execute THEN they SHALL have ≤100μs budget with watchdog protection
3. WHEN plugins overrun or crash THEN they SHALL be quarantined for the session while the engine continues
4. WHEN IPC versions change THEN plugins SHALL negotiate compatibility and refuse ABI mismatches
5. IF plugin capabilities are restricted THEN the manifest SHALL clearly indicate permissions
6. WHEN WASM plugins load THEN they SHALL declare required capabilities; any undeclared use (file/network) SHALL be denied
7. WHEN native plugins run THEN they SHALL execute in separate helper process with SHM queues; exceeding ≤100μs budget or crashing SHALL quarantine plugin for session and raise PLUG-OVERRUN event

### Requirement 11: User Interface and Experience (UX-01)

**User Story:** As a new user, I want a guided setup process and intuitive interface, so that I can get from installation to flying quickly without technical expertise.

#### Acceptance Criteria

1. WHEN first running the system THEN a wizard SHALL guide through calibration, sim selection, and verification
2. WHEN conflicts are detected THEN the system SHALL provide one-click resolution options
3. WHEN viewing detents THEN a live bar SHALL show labeled zones with hysteresis bands
4. WHEN using CLI THEN it SHALL have parity with UI functionality and provide JSON output
5. IF accessibility is needed THEN the system SHALL support high-contrast UI and color-blind palettes
6. WHEN completing first-run wizard THEN it SHALL enable a working flight within ≤10 minutes from install on supported sim/aircraft
7. WHEN troubleshooting THEN a startup flag SHALL disable plugins/panels/tactile and load minimal axis pipeline for Safe Mode

### Requirement 12: Performance and Resource Management (NFR-01)

**User Story:** As a performance-conscious user, I want the system to use minimal system resources while maintaining real-time performance, so that it doesn't impact my flight simulation experience.

#### Acceptance Criteria

1. WHEN the system is running THEN CPU usage SHALL be <3% of one mid-range core in cruise
2. WHEN memory is allocated THEN RSS SHALL be <150MB during normal operation
3. WHEN running for extended periods THEN the system SHALL maintain stability for 24-48 hours
4. WHEN processing HID writes THEN latency SHALL be ≤300μs p99, measured over ≥10 minutes under normal load
5. IF resource limits are exceeded THEN the system SHALL gracefully degrade and log the condition
6. WHEN running soak tests THEN the service SHALL run 24-48h with synthetic telemetry without missed ticks or leaks (RSS delta <10%)
7. WHEN CPU/RAM budgets are exceeded THEN non-RT work (panels/tactile/plugins) SHALL shed load first; RT loop MUST remain within p99 jitter
### Req
uirement 13: Release, Packaging, and Channels (REL-01)

**User Story:** As a user, I want reliable software updates and the ability to choose my update cadence, so that I can balance stability with new features according to my needs.

#### Acceptance Criteria

1. WHEN choosing update channels THEN the system SHALL provide stable/beta/canary channels; writers (sim diffs) are releasable independently of the core
2. WHEN updating THEN updates SHALL be signed; rollback on startup crash; keep the last two versions on disk
3. WHEN checking compatibility THEN the system SHALL publish and ship a compatibility matrix by sim version; installer surfaces current state

### Requirement 14: Interop & Legal (LEG-01)

**User Story:** As a user with existing flight sim tools, I want the system to work alongside my current setup while respecting simulator licensing terms, so that I don't have to abandon my existing investments.

#### Acceptance Criteria

1. WHEN managing curves THEN the system owns axis curves; it interops with AAO/SPAD/FSUIPC for panels; no exclusive lockouts
2. WHEN using DCS THEN the adapter SHALL be limited to user-installed Export.lua in Saved Games; the UI SHALL label MP-blocked features
3. WHEN used for training THEN the system SHALL state "not for certified training devices" unless/until pursued formally

## CI Quality Gates

The following quality gates are enforced as build requirements:

**QG-AX-Jitter (MUST):** Fail build if 250Hz p99 jitter >0.5ms (virtual + one physical runner).

**QG-HID-Latency (MUST):** Fail if HID write p99 >300μs.

**QG-Writers (MUST):** Fail on golden test mismatches.

**QG-Schema (MUST):** Fail on IPC/Profile breaking changes without version bump/migrator.

**QG-Blackbox (MUST):** Fail if any drops in a 10-minute capture.

**QG-Soft-Stop (MUST):** Fail if fault→torque→0 >50ms.