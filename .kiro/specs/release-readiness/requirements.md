# Requirements Document

## Introduction

This specification consolidates the remaining work needed to bring Flight Hub from its current development state to a production-ready v1 release. It covers five key areas: platform runtime and timing (Windows MMCSS, Linux rtkit, jitter validation), packaging and distribution (Windows MSI, Linux .deb, auto-update), final integration testing (soak tests, hardware validation), documentation and legal compliance (product posture, "What We Touch" docs, user guides), and release preparation (CI gates, artifact signing, release tagging).

The goal is to ensure Flight Hub meets all quality gates (QG-RT-JITTER, QG-HID-LATENCY, QG-FFB-SAFETY, QG-LEGAL-DOC) and can be confidently shipped to end users with proper installers, documentation, and support infrastructure.

### Current State Summary

**Completed:**
- Core 250Hz axis processing pipeline
- Simulator adapters (MSFS, X-Plane, DCS)
- FFB safety systems (SafetyEnvelope, fault detection, blackbox)
- Basic real-time scheduler with PLL phase correction
- CLI parity and safe mode
- Build, lint, and supply chain fixes

**Remaining (this spec):**
- Windows MMCSS registration and high-resolution timers
- Linux rtkit integration for unprivileged RT scheduling
- Hardware-backed jitter validation CI gates
- Windows MSI and Linux .deb packaging
- Auto-update system with rollback
- Soak tests and hardware validation
- Product posture and legal documentation
- User documentation and guides
- Release CI gates and artifact signing

## Glossary

- **MMCSS**: Windows Multimedia Class Scheduler Service for real-time thread priority management
- **rtkit**: Linux D-Bus service for acquiring real-time scheduling privileges without root
- **QPC**: QueryPerformanceCounter, Windows high-resolution monotonic clock
- **Soak_Test**: Extended duration test (24-48h) validating stability under continuous load
- **MSI**: Microsoft Installer package format for Windows software distribution
- **WiX**: Windows Installer XML toolset for building MSI packages
- **Product_Posture**: Legal document defining Flight Hub's relationship to simulators (accessory, not replacement)
- **What_We_Touch**: Per-simulator documentation of files, APIs, and ports accessed by Flight Hub
- **Quality_Gate**: CI check that must pass before release; failure blocks the build

## Requirements

### Requirement 1: Windows Real-Time Thread Configuration

**User Story:** As a Flight Hub developer, I want Windows real-time thread configuration using MMCSS, so that the axis and FFB loops meet their timing guarantees on Windows systems.

#### Acceptance Criteria

1. WHEN creating the RT axis thread THEN the System SHALL register with MMCSS via AvSetMmThreadCharacteristicsW using task name "Games" or "Pro Audio"
2. WHEN MMCSS registration succeeds THEN the System SHALL elevate thread priority using SetThreadPriority with THREAD_PRIORITY_TIME_CRITICAL
3. WHEN the process starts with active sim or FFB device THEN the System SHALL disable process power throttling via SetProcessInformation with PROCESS_POWER_THROTTLING_EXECUTION_SPEED flag
4. WHEN MMCSS registration fails THEN the System SHALL log a warning with the HRESULT error code and continue with elevated priority only
5. WHEN the RT thread terminates THEN the System SHALL call AvRevertMmThreadCharacteristics to release MMCSS registration

### Requirement 2: Windows High-Resolution Timer Loop

**User Story:** As a Flight Hub developer, I want a high-resolution 250Hz timer loop on Windows, so that axis processing achieves sub-millisecond jitter.

#### Acceptance Criteria

1. WHEN implementing the 250Hz loop THEN the System SHALL use CreateWaitableTimerExW with CREATE_WAITABLE_TIMER_HIGH_RESOLUTION flag where supported
2. WHEN high-resolution timers are unavailable THEN the System SHALL fall back to timeBeginPeriod(1) with standard waitable timers
3. WHEN finishing each tick THEN the System SHALL busy-spin for the final 50-80μs using QueryPerformanceCounter to minimize jitter
4. WHEN measuring timing THEN the System SHALL use QueryPerformanceCounter as the monotonic clock source for all interval calculations
5. WHEN the timer loop runs THEN the p99 jitter SHALL be ≤0.5ms measured over ≥10 minutes with warm-up excluded

### Requirement 3: Windows Power Management Integration

**User Story:** As a Flight Hub developer, I want proper power management integration on Windows, so that the system prevents sleep and throttling during active operation.

#### Acceptance Criteria

1. WHEN at least one sim is connected and FFB device is active THEN the System SHALL call PowerCreateRequest and PowerSetRequest with EXECUTION_REQUIRED and SYSTEM_REQUIRED
2. WHEN idle with no active sim or FFB THEN the System SHALL clear power requests via PowerClearRequest to allow normal power management
3. WHEN power request creation fails THEN the System SHALL log a warning and continue operation without power management integration

### Requirement 4: Windows HID Write Optimization

**User Story:** As a Flight Hub developer, I want optimized HID writes on Windows, so that FFB output achieves ≤300μs p99 latency.

#### Acceptance Criteria

1. WHEN opening HID devices for FFB output THEN the System SHALL use CreateFile with FILE_FLAG_OVERLAPPED for non-blocking I/O
2. WHEN writing HID reports THEN the System SHALL use async WriteFile with an OVERLAPPED struct pool instead of HidD_SetOutputReport
3. WHEN measuring HID latency THEN the p99 write latency SHALL be ≤300μs measured over ≥10 minutes under normal load
4. WHEN HID writes fail THEN the System SHALL detect USB OUT stalls within 3 frames and trigger the fault handler

### Requirement 5: Linux Real-Time Thread Configuration

**User Story:** As a Flight Hub developer, I want Linux real-time thread configuration using rtkit, so that Flight Hub works well on Linux systems with and without RT privileges.

#### Acceptance Criteria

1. WHEN creating RT threads THEN the System SHALL first attempt to acquire privileges via rtkit D-Bus interface using MakeThreadRealtime
2. WHEN rtkit is unavailable or denies request THEN the System SHALL fall back to sched_setscheduler with SCHED_FIFO and priority 1-49
3. WHEN RT scheduling is unavailable THEN the System SHALL fall back to normal priority, log a warning, and expose metrics for timing validation
4. WHEN running with RT priority THEN the System SHALL call mlockall(MCL_CURRENT | MCL_FUTURE) to prevent page faults in RT threads
5. WHEN starting THEN the System SHALL validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK limits and warn if insufficient

### Requirement 6: Linux High-Resolution Timer Loop

**User Story:** As a Flight Hub developer, I want a high-resolution 250Hz timer loop on Linux, so that axis processing achieves sub-millisecond jitter.

#### Acceptance Criteria

1. WHEN implementing the 250Hz loop THEN the System SHALL use clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME) with absolute target times
2. WHEN finishing each tick THEN the System SHALL busy-spin for the final 50μs using clock_gettime(CLOCK_MONOTONIC) to minimize jitter
3. WHEN the timer loop runs THEN the p99 jitter SHALL be ≤0.5ms measured over ≥10 minutes with warm-up excluded

### Requirement 7: Linux RT Metrics and Setup

**User Story:** As a Flight Hub developer, I want Linux RT metrics exposure and setup helpers, so that users can verify and configure RT operation.

#### Acceptance Criteria

1. WHEN running on Linux THEN the System SHALL expose metrics: runtime.linux.rt_enabled, runtime.linux.sched_policy, runtime.linux.priority, runtime.linux.mlockall_success
2. WHEN distributing THEN the System SHALL provide scripts/setup-linux-rt.sh that configures /etc/security/limits.conf for rtprio and memlock
3. WHEN documenting THEN the System SHALL provide instructions for group membership and logout/login requirements

### Requirement 8: Cross-Platform Jitter Validation

**User Story:** As a Flight Hub developer, I want hardware-backed jitter validation in CI, so that timing regressions are caught before release.

#### Acceptance Criteria

1. WHEN running jitter tests THEN the System SHALL implement a JitterMeasurement helper that records deviation vs ideal period and computes p50/p95/p99
2. WHEN running on hardware CI runners THEN the System SHALL run 10-minute synthetic loops at 250Hz and assert p99 ≤0.5ms
3. WHEN running on virtualized CI runners THEN the System SHALL run in report-only mode without failing builds
4. WHEN CI runs THEN the QG-RT-JITTER gate SHALL fail the build if p99 jitter >0.5ms on hardware runners

### Requirement 9: Windows MSI Installer

**User Story:** As a Flight Hub user, I want a signed Windows MSI installer, so that I can install Flight Hub safely and easily.

#### Acceptance Criteria

1. WHEN building the Windows installer THEN the System SHALL create an MSI package using WiX Toolset
2. WHEN installing THEN the MSI SHALL support features: core (required), MSFS integration (optional), X-Plane integration (optional), DCS integration (optional)
3. WHEN installing core components THEN they SHALL be installed per-user by default; sim integrations MAY require per-machine scope
4. WHEN installing THEN the installer SHALL display product posture summary and EULA excerpt
5. WHEN installing sim integrations THEN they SHALL be opt-in toggles, not installed by default
6. WHEN uninstalling THEN the uninstaller SHALL remove all installed binaries, restore backed-up Export.lua, and remove X-Plane plugins

### Requirement 10: Windows Code Signing

**User Story:** As a Flight Hub user, I want signed Windows binaries, so that I can trust the software I'm installing.

#### Acceptance Criteria

1. WHEN building releases THEN CI SHALL sign all EXE, DLL, and MSI artifacts using signtool with an OV or EV code signing certificate
2. WHEN building release jobs THEN they SHALL fail if any artifact is unsigned
3. WHEN displaying in UI THEN the System SHALL show signature status for installed binaries

### Requirement 11: Linux Package Formats

**User Story:** As a Linux user, I want native Linux packages, so that I can install Flight Hub using my distribution's package manager.

#### Acceptance Criteria

1. WHEN building Linux packages THEN the System SHALL create .deb packages with binaries in /usr/bin
2. WHEN building .deb packages THEN they SHALL include udev rules for /dev/hidraw* device access
3. WHEN installing .deb packages THEN postinst scripts SHALL add user to relevant groups and reload udev rules
4. WHEN documenting THEN the System SHALL provide installation instructions covering package installation, RT setup, and group membership

### Requirement 12: Third-Party Components Inventory

**User Story:** As a Flight Hub user, I want to know what third-party components are included, so that I can verify license compliance.

#### Acceptance Criteria

1. WHEN distributing THEN the System SHALL generate third-party-components.toml from cargo dependencies
2. WHEN distributing THEN the System SHALL collect and ship license texts for all redistributed components
3. WHEN installing THEN the installer SHALL link to the third-party components inventory

### Requirement 13: Soak Tests

**User Story:** As a Flight Hub developer, I want soak tests that validate long-running stability, so that users can trust Flight Hub for extended sessions.

#### Acceptance Criteria

1. WHEN running soak tests THEN the System SHALL run 24-48h synthetic telemetry + FFB loops on hardware
2. WHEN soak tests complete THEN they SHALL assert: no missed ticks beyond threshold, RSS stable (delta <10%), blackbox present on any faults
3. WHEN soak tests fail THEN they SHALL produce diagnostic output identifying the failure mode

### Requirement 14: Integration Test Suite

**User Story:** As a Flight Hub developer, I want comprehensive integration tests, so that end-to-end flows are validated before release.

#### Acceptance Criteria

1. WHEN running integration tests THEN the System SHALL test each sim adapter: connect → stream → disconnect → reconnect
2. WHEN running integration tests THEN the System SHALL test end-to-end: sim fixture → bus → FFB → safety → no faults under normal conditions
3. WHEN integration tests fail THEN they SHALL produce diagnostic output identifying the failure point

### Requirement 15: Product Posture Documentation

**User Story:** As a Flight Hub user, I want clear product positioning documentation, so that I understand Flight Hub's relationship to simulators.

#### Acceptance Criteria

1. WHEN documenting THEN the System SHALL create docs/product-posture.md stating: "Flight Hub is an accessory/input manager that requires MSFS/X-Plane/DCS; it does not emulate or replace any simulator"
2. WHEN documenting THEN the product posture SHALL include export-control and EULA reminders from sim vendors
3. WHEN distributing THEN the product posture SHALL be linked from README, website, and installer

### Requirement 16: What We Touch Documentation

**User Story:** As a Flight Hub user, I want to know exactly what Flight Hub modifies on my system, so that I can make informed decisions and revert changes if needed.

#### Acceptance Criteria

1. WHEN documenting MSFS integration THEN the System SHALL list: files modified, APIs used, SimVars accessed, ports used
2. WHEN documenting X-Plane integration THEN the System SHALL list: plugins installed, DataRefs accessed, UDP ports used
3. WHEN documenting DCS integration THEN the System SHALL list: Export.lua modifications, data exported, ports used
4. WHEN documenting THEN each "What We Touch" document SHALL include instructions for reverting all changes

### Requirement 17: User Documentation

**User Story:** As a Flight Hub user, I want comprehensive user documentation, so that I can install, configure, and troubleshoot Flight Hub.

#### Acceptance Criteria

1. WHEN documenting THEN the System SHALL provide install guides for Windows and Linux
2. WHEN documenting THEN the System SHALL provide per-sim setup guides for MSFS, X-Plane, and DCS
3. WHEN documenting THEN the System SHALL provide FFB device configuration and safety guidelines
4. WHEN documenting THEN the System SHALL provide troubleshooting guides for common issues (RT not enabled, no FFB, permissions)

### Requirement 18: CI Quality Gate Enforcement

**User Story:** As a Flight Hub developer, I want all quality gates enforced in CI, so that releases meet quality standards.

#### Acceptance Criteria

1. WHEN running CI THEN all QG-* checks SHALL have dedicated CI jobs: QG-SIM-MAPPING, QG-UNIT-CONV, QG-SANITY-GATE, QG-FFB-SAFETY, QG-RT-JITTER, QG-HID-LATENCY, QG-LEGAL-DOC
2. WHEN merging to main or release branches THEN QG-* jobs SHALL be required checks that block merge on failure
3. WHEN documenting THEN quality gates SHALL be documented in CONTRIBUTING.md

### Requirement 19: Release Preparation

**User Story:** As a Flight Hub developer, I want a clear release process, so that releases are consistent and reliable.

#### Acceptance Criteria

1. WHEN preparing release THEN the System SHALL run full test matrix: unit, integration, RT, HID, soak
2. WHEN preparing release THEN the System SHALL verify installers on clean Win10/11 and at least one Linux distro
3. WHEN preparing release THEN all CI quality gates SHALL be green
4. WHEN releasing THEN the System SHALL tag the release and archive artifacts: binaries, installers, docs
5. WHEN releasing THEN the System SHALL generate release notes from changelog
