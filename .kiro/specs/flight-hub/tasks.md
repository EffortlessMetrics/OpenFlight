# Flight Hub Implementation Plan

## Overview

This implementation plan converts the Flight Hub requirements and design into a series of incremental, testable coding tasks. The plan follows a milestone-driven approach with a critical path through M0→M1→M2→M3, ensuring the real-time spine is established and protected before adding peripheral functionality.

Each task builds incrementally on previous work, with comprehensive testing and CI gates to prevent regressions. The focus is on creating a boring-reliable 250Hz axis processing core that can be extended safely.

## Definitions

**Latency:** Input sample timestamp → HID OUT write completion measured with monotonic clock; exclude device firmware time.

**Jitter p99:** p99 of tick interval error over ≥10 minutes, discarding first 5s warm-up.

**Missed Tick:** A tick whose start time slips >1.5× nominal period (≥6ms at 250Hz). Counted and logged.

**Fault:** Any of {USB OUT stall ≥3 frames, endpoint error, encoder invalid/NaN, over-temp/current, plugin overrun}.

**Session:** Process lifetime or until device power cycle; high-torque consent resets on power cycle.

## Implementation Tasks

### Milestone 0: Foundation (M0) - Contracts, CI, Virtual Device

- [x] 1. Workspace & CI bootstrap





  - Create Cargo workspace with proper crate structure and dependencies
  - Set up GitHub Actions with Windows+Linux matrices, caching, and lint enforcement
  - Configure rustfmt, clippy, cargo-deny for supply chain security
  - _Requirements: NFR-02, NFR-03_

- [x] 2. IPC schema & codegen (flight-ipc)





  - Define schemas/ipc/flight.v1.proto with core service definitions
  - Generate prost/tonic types with feature-gated transports (pipes/UDS)
  - Implement feature negotiation RPC and breaking-change CI checks
  - Create round-trip tests and examples for list-devices, health-subscribe
  - _Requirements: IFC-01, XPLAT-01_

- [x] 3. Profile & rules schemas (flight-core, flight-panels)





  - Implement flight.profile/1 JSON Schema with validation and line/column errors
  - Create monotonic curve checker and deterministic profile merging
  - Design rules DSL schema with minimal parser (stub implementation)
  - Add property tests for profile canonicalization and merge determinism
  - _Requirements: PRF-01, PNL-01_


- [x] 3.1 Profile canonicalization & hash (flight-core)

  - Implement canonical JSON (sorted keys, normalized float precision) + effective profile hash
  - Create merge property tests ensuring same inputs produce identical hash
  - Add rejection of non-monotonic curves with line/column error reporting
  - Build comprehensive determinism validation across machines
  - _Requirements: PRF-01, PRF-02_

- [x] 4. Virtual device & perf gate (flight-virtual, flight-scheduler)





  - Create loopback HID device for CI testing without hardware
  - Implement flight-scheduler with ABS timer + busy-spin tail for both platforms
  - Build jitter measurement harness with 250Hz loop targeting p99 ≤0.5ms
  - Set up CI perf gate that fails build on timing regressions
  - _Requirements: NFR-01, QG-AX-Jitter_

- [x] 4.1 Scheduler PLL & drop policy (flight-scheduler)


  - Implement ABS schedule + PLL that trims phase ≤0.1%/s with busy-spin tail (50-80μs)
  - Create bounded SPSC rings with drop-tail policy for non-RT streams with counters
  - Add overload testing to verify no RT blocking under consumer pressure
  - Build comprehensive timing discipline validation over 10+ minute runs
  - _Requirements: NFR-01, QG-AX-Jitter_



- [x] 4.2 ADRs & MSRV/semver guardrails





  - Create ADRs for RT spine, writers-as-data, plugin classes with clear rationale
  - Pin MSRV with CI job validation, configure cargo-deny and cargo-audit
  - Add CI gates that fail on advisory/deny violations
  - Reference ADRs in crate READMEs for contributor clarity
  - _Requirements: Documentation, Security_

### Milestone 1: Axis Engine Spine (M1) - Real-Time Core
- [x] 5. flight-axis: frame, node API, atomic swap & no-alloc guard



















- [ ] 5. flight-axis: frame, node API, atomic swap & no-alloc guard

  - Define AxisFrame struct with explicit units and Node trait
  - Implement core pipeline nodes: deadzone, curve, slew with zero-alloc guarantee
  - Create compile-to-function-pointer system with SoA state layout
  - Implement atomic pointer swap at tick boundary with proper ACK semantics
  - Add runtime counters for allocs/locks in hot path (must remain zero)
  - Ensure compile failure leaves RT state unchanged with comprehensive testing
  - _Requirements: AX-01_

- [x] 6. Detent mapper & events





  - Implement detent zones with hysteresis and semantic role enums
  - Create event channel for detent transitions and state changes
  - Add sweep tests to verify single transition per boundary crossing
  - Ensure deterministic behavior with property-based testing


  - _Requirements: AX-01_

- [x] 7. Mixers (helicopter torque cross-feed)



  - Implement mixer node for multi-axis interactions
  - Add per-axis scale/gain with proper unit handling
  - Create comprehensive unit tests for mixer mathematics
  - Verify zero-allocation constraint maintained
  - _Requirements: AX-01_

- [x] 8. Double-curve detector & guidance










  - Implement sim-curve detection hooks in axis pipeline
  - Create IPC surface for conflict detection and resolution prompts
  - Add blackbox annotation for curve conflicts

  - Build one-click disable mechanism via writer system
  - _Requirements: AX-01, UX-01_

- [x] 8.1 Double-curve one-click fix (writer hook)



  - Implement IPC action to disable sim curves or apply gain compensation via flight-writers
  - Create detection → button → verify workflow with comprehensive testing
  - Add blackbox markers for conflict detection and resolution events
  - Build user-friendly resolution interface with clear feedback
  - _Requirements: AX-01, UX-01_

### Milestone 2: Safety & Watchdogs (M2) - Critical Safety Systems

- [ ] 9. flight-ffb: safety state machine & interlock
  - Implement SafeTorque/HighTorque/Faulted state machine
  - Create physical button-combo challenge/ACK system with rolling tokens
  - Add UI consent verification and persistence until power-cycle
  - Build comprehensive negative tests for safety violations
  - _Requirements: FFB-01, SAFE-01_

- [ ] 9.1 Physical interlock token (device echo)
  - Implement challenge (blink/pattern) with rolling token returned by device
  - Create service validation before arming high torque with token verification
  - Add negative tests proving unlock impossible without device echo
  - Build comprehensive security validation for remote unlock prevention
  - _Requirements: FFB-01, SAFE-01_

- [ ] 10. Soft-stop ramp & audible cue
  - Implement torque ramp to zero within 50ms constraint
  - Add sound/LED cue system via panel integration
  - Create USB yank test infrastructure for HIL validation
  - Ensure 2s pre-fault capture in blackbox system
  - _Requirements: FFB-01, DIAG-01_

- [ ] 11. Kid/Demo caps (domain + engine)
  - Implement capability enforcement in profile validation and engine clamps
  - Create IPC toggles for demo/kid mode activation
  - Add tests to verify overrides are properly rejected
  - Ensure clamped outputs are logged for audit trail
  - _Requirements: SAFE-01_

- [ ] 12. Watchdogs & quarantine system
  - Implement USB stall timeout, NaN guards, and endpoint error detection
  - Create plugin overrun counters and PLUG-OVERRUN event system
  - Build quarantine mechanism that isolates failed components
  - Add synthetic fault injection tests for validation
  - _Requirements: SAFE-01, PLUG-01_

- [ ] 12.1 Fault matrix & pre-fault capture
  - Implement fault table (USB stall ≥3 frames, endpoint wedged, NaN, over-temp, plugin overrun)
  - Create action matrix (torque→0 ≤50ms) with 2s pre-fault .fbb capture
  - Add stable error codes for all fault conditions with KB article links
  - Build comprehensive fault injection suite with validation
  - _Requirements: SAFE-01, DIAG-01_

### Milestone 3: Sim Adapters, Bus, Writers, Auto-Profiles (M3) - Integration Layer

- [ ] 13. flight-bus: normalized model & publisher
  - Define BusSnapshot structure with comprehensive telemetry model
  - Implement pub/sub system with 30-60Hz rate limiting
  - Create fixtures for consistent snapshot publishing
  - Add subscriber validation for stable data delivery
  - _Requirements: GI-01_

- [ ] 13.1 Bus type/units schema
  - Implement typed fields (kt, deg, g, %) with unit validation
  - Create unit tests per adapter with converter helpers
  - Add validation preventing out-of-range/wrong units publication
  - Build comprehensive type safety for telemetry data
  - _Requirements: GI-01_

- [ ] 14. MSFS (SimConnect) adapter (flight-simconnect(-sys))
  - Create FFI wrapper or dynamic linking for SimConnect SDK
  - Implement variable/event mapping with aircraft detection
  - Build integration tests with recorded session fixtures
  - Document coverage and redistribution compliance
  - _Requirements: GI-01, LEG-01_

- [ ] 15. X-Plane adapter (flight-xplane(-sys))
  - Implement DataRefs via UDP and plugin interfaces
  - Add aircraft detection and web API integration paths
  - Create latency measurement and budget validation
  - Build comprehensive fixture-based testing
  - _Requirements: GI-01_

- [ ] 16. DCS export (flight-dcs-export)
  - Create minimal Export.lua with MP-safe/blocked feature flags
  - Implement socket bridge with version negotiation
  - Add SP verification and MP limitation documentation
  - Create user installation script and validation
  - _Requirements: GI-01, LEG-01_

- [ ] 16.1 DCS MP integrity contract
  - Implement user-installed Export.lua only with MP-safe vs blocked flags
  - Create adapter that refuses blocked features in MP with clear UI messaging
  - Add "what we touch + revert" documentation for DCS integration
  - Build MP session detection with "blocked" banner display
  - _Requirements: LEG-01_

- [ ] 17. Writers data-pipeline with golden tests & verify/repair
  - Implement versioned JSON diffs per sim/build with golden-file tests
  - Create Verify/Repair that runs scripted events (gear/flap/AP) and applies minimal diffs
  - Build coverage matrix generation with CI failure on golden mismatch
  - Add one-click rollback functionality with comprehensive validation
  - _Requirements: GI-01, GI-02, GI-05_

- [ ] 18. Aircraft auto-switch ≤500ms
  - Implement process+aircraft detection across all sim adapters
  - Create profile resolution with merge hierarchy enforcement
  - Build compile-and-swap system for profile changes
  - Add PoF hysteresis logic with comprehensive testing
  - _Requirements: GI-01, PRF-01_

### Milestone 4: Panels, StreamDeck, Rules DSL, Tactile (M4) - User Interface Layer

- [ ] 19. DSL compiler → bytecode + no-alloc evaluator
  - Compile AST → compact bytecode (ops + hysteresis keys) with pre-allocated stack
  - Create evaluator running 60-120Hz off RT thread with zero runtime allocations
  - Add LED latency ≤20ms with rate limiting (≥8ms min interval)
  - Build comprehensive validation of zero-allocation constraint
  - _Requirements: PNL-01_

- [ ] 20. Saitek/Logitech panel writer
  - Implement HID driver for common panel hardware
  - Create mapping from rules engine to LEDs/switches
  - Add verify test pattern with ≤20ms latency validation
  - Build Verify matrix integration for drift detection
  - _Requirements: PNL-01_

- [ ] 21. Cougar MFD writer
  - Implement MFD-specific mapping and verify patterns
  - Add fixture-based testing for hardware compatibility
  - Ensure latency budget compliance
  - Create comprehensive hardware validation suite
  - _Requirements: PNL-01_

- [ ] 22. StreamDeck plugin (flight-streamdeck)
  - Create local Web API for StreamDeck integration
  - Build plugin with sample profiles for GA/Airbus/Helo
  - Add verify event round-trip testing
  - Document installation process and port requirements
  - _Requirements: PNL-01_

- [ ] 22.1 StreamDeck version posture
  - Implement supported app version range with degrade-gracefully behavior
  - Add detection & warning for out-of-range versions without crashing
  - Create compatibility matrix and user guidance for version management
  - Build comprehensive version compatibility testing
  - _Requirements: PNL-01_

- [ ] 23. Tactile bridge (optional)
  - Implement basic channel routing for touchdown/rumble/stall effects
  - Create rate-limited thread with SimShaker-class app bridge
  - Add user toggle functionality for independent control
  - Verify no AX/FFB jitter regression with comprehensive testing
  - _Requirements: PNL-01_

### Milestone 5: Force Feedback Modes (M5) - Advanced FFB Features

- [ ] 24. FFB mode negotiation & trim limits
  - Implement device caps (supports_pid, supports_raw_torque, min_period_us, max_torque_nm, health_stream)
  - Create policy that selects DI/Raw/Synth with trim rate/jerk limits (no torque step)
  - Add selection matrix tests with HIL trim tests matching FP tolerance
  - Verify no AX jitter regression with comprehensive performance validation
  - _Requirements: FFB-01_

- [ ] 25. OFP-1 spec & handshake (optional)
  - Implement HID Feature 0x32 caps, OUT 0x30 torque, IN 0x31 health
  - Create capability negotiation in flight-hid
  - Add device opt-in negotiation and torque path stability
  - Build health stream visibility and monitoring
  - _Requirements: FFB-01, IFC-04_

- [ ] 25.1 OFP-1 reference emulator
  - Create virtual device that speaks OFP-1 feature/IN/OUT with health stream
  - Build engine negotiation and drive emulator with blackbox health capture
  - Add comprehensive testing without requiring physical hardware
  - Create development and validation environment for raw-torque mode
  - _Requirements: IFC-04_

- [ ] 26. Telemetry-synth effects
  - Implement stall buffet vs α, touchdown impulse, ground roll effects
  - Add gear warning and rotor Nr/Np synthesis
  - Create rate-limiting and off-RT scheduling
  - Build user tuning interface with blackbox markers
  - _Requirements: FFB-01_

- [ ] 27. Force-trim correctness validation
  - Implement non-FFB recentre illusion with trim-hold freeze
  - Add FFB setpoint change with rate/jerk limiting
  - Create HIL tests for trim behavior validation
  - Ensure replay reproducibility with comprehensive testing
  - _Requirements: FFB-01_

### Milestone 6: Diagnostics, Tracing, Replay (M6) - Observability Layer

- [ ] 28. Blackbox .fbb writer behavior
  - Implement chunked writes (4-8KB), index every 100ms, CRC32C footer
  - Create flush at stop or 1s cadence with no fsync per chunk
  - Add 10-minute capture: 0 drops; size <30MB/3min; corruption detection
  - Build comprehensive writer performance and reliability validation
  - _Requirements: DIAG-01, IFC-03_

- [ ] 29. Replay harness
  - Create offline axis/FFB engine feeding system
  - Implement FP-tolerant output comparison
  - Add acceptance test integration for recorded runs
  - Build comprehensive replay validation suite
  - _Requirements: DIAG-01_

- [ ] 30. Tracing & perf counters
  - Implement ETW provider (Win) / tracepoints (Linux) hooks
  - Add TickStart/End, HidWrite, DeadlineMiss instrumentation
  - Create counter reporting for CI perf gate consumption
  - Build regression detection and failure mechanisms
  - _Requirements: NFR-01, QG-AX-Jitter_

- [ ] 30.1 CI perf dashboard
  - Scrape counters (jitter p50/p99, HID p99, drops) from ETW/tracepoints
  - Publish trend graphs with CI failure on regression thresholds
  - Create visible performance monitoring with automated alerting
  - Build comprehensive performance regression detection system
  - _Requirements: NFR-01_

### Milestone 7: Service, CLI, Packaging, Updates (M7) - Production Readiness

- [ ] 31. Service wiring (flight-service)
  - Implement app/use-cases layer with port orchestration
  - Add profile apply, safety gate, auto-profiles functionality
  - Create health stream and error taxonomy with stable codes
  - Build end-to-end scenario acceptance testing
  - _Requirements: Multiple (DM/GI/PRF/SAFE/UX)_

- [ ] 31.1 Safe Mode + power hints
  - Implement --safe (axis-only; no panels/plugins/tactile) with basic profile
  - Add power checks (USB selective suspend, plan throttling on Win; rtkit/memlock on Linux)
  - Create remediation guidance with clear user instructions
  - Build safe boot validation and RT privilege detection
  - _Requirements: UX-01, XPLAT-01_

- [ ] 32. CLI parity (flight-cli)
  - Implement devices list|info, profile apply, sim configure commands
  - Add panels verify, torque unlock, diag record/replay functionality
  - Create --json output format with non-zero error codes
  - Build comprehensive CLI parity validation with IPC
  - _Requirements: UX-01_

- [ ] 33. Packaging & updates
  - Create MSI (signed) and systemd user units
  - Implement udev rules and delta update system
  - Add rollback functionality and signature verification
  - Ensure install without admin/root at runtime
  - _Requirements: XPLAT-01, SEC-01_

- [ ] 33.1 Channels & rollback
  - Implement stable/beta/canary channels with signed delta updates
  - Create auto-rollback on startup crash with prior 2 versions kept
  - Add update simulator tests (upgrade→crash→rollback) validation
  - Build comprehensive update reliability and recovery system
  - _Requirements: REL-01_

- [ ] 33.2 "What we touch" docs surfaced
  - Create per-sim page listing files/ports/vars and revert steps
  - Link documentation from installer & UI with validation
  - Add doc checks that pass with resolving links
  - Build comprehensive integration documentation for users
  - _Requirements: LEG-01_

## Cross-Cutting Tasks (Always-On)

- [ ] 34. ADRs & governance
  - Document architectural decisions for RT loop, driver-light approach
  - Create ADRs for pipeline ownership, FFB modes, interlock design
  - Document writers, schemas, and observability decisions
  - Maintain ADR repository with cross-references in documentation
  - _Requirements: Documentation_

- [ ] 35. Security & privacy posture
  - Implement local-only IPC with ACLs and no outbound telemetry default
  - Add opt-in metrics with redaction in support ZIP
  - Create plugin signing surface and capability validation
  - Build comprehensive security verification in CI/manual checks
  - _Requirements: SEC-01_

- [ ] 36. Documentation & examples
  - Create docs.rs documentation for each crate with examples
  - Build examples for profile parsing, pipeline compilation, SimConnect usage
  - Add StreamDeck panel and capture + replay examples
  - Ensure all examples compile and run successfully
  - _Requirements: Documentation_

- [ ] 37. Supply-chain & audits
  - Implement cargo-audit, cargo-deny with SPDX in each crate
  - Create third-party license list and audit trail
  - Add CI gates on advisories/deny with license scan validation
  - Maintain comprehensive dependency security monitoring
  - _Requirements: Security_

## Quality Gates & Success Criteria

Each milestone must pass these gates before proceeding:

**M0 Gates:**
- CI perf gate over 10 min; profile canonicalization/hash tests pass
- PLL/drop policy validated with overload testing
- Workspace compiles on Windows+Linux with all lint checks

**M1 Gates:**
- Atomic swap ACK with compile failure leaving RT state unchanged
- No alloc/locks counters remain zero in hot path during operation
- One-click sim-curve fix exercises writers system successfully

**M2 Gates:**
- Physical runner perf gates (jitter & HID p99) with timing validation
- FMEA faults recorded with 2s pre-fault capture in blackbox
- Tokened interlock verified with comprehensive negative testing
- **Physical Runner Required:** All perf gates run on one physical rig (Windows) in CI or nightly

**M3 Gates:**
- Writers golden tests + Verify/Repair green with coverage matrix published
- DCS MP safety enforced with blocked feature detection
- Aircraft auto-switch completes ≤500ms with PoF hysteresis working

**M4 Gates:**
- DSL evaluator allocs = 0 with runtime validation
- LED latency ≤20ms with rate limiter (≥8ms min interval)
- StreamDeck version compatibility validated

**M5 Gates:**
- Mode selection matrix tests with device capability negotiation
- Trim rate/jerk limits proven via HIL with no torque steps
- No axis jitter regression with comprehensive performance validation

**M6 Gates:**
- 10-min .fbb capture no drops with size <30MB/3min
- Replay FP epsilon validation with timing drift ≤0.1ms/s
- Perf dashboard wired to CI with regression detection

**M7 Gates:**
- Safe Mode boot with power hints and RT privilege detection
- Channels + rollback verified with update simulator tests
- Installer runs without admin/root at runtime with full validation

## Dependencies & Critical Path

**Critical Path:** M0 → M1 → M2 → M3 (must be sequential)
**Parallel After M3:** M4 (Panels), M5 (FFB), M6 (Diagnostics) can proceed in parallel
**Final Integration:** M7 requires completion of M4-M6

This implementation plan ensures incremental progress with comprehensive testing at each stage, maintaining the real-time performance guarantees while building a robust, production-ready flight simulation input management system.## R
isk Register & Mitigations

**USB Hub Timing Cliffs:** Mitigated by PLL & busy-spin tail; physical runner gate catches real-world timing issues.

**Sim Updates Breaking Writers:** Golden tests + Verify/Repair + coverage matrix provide automated detection and repair.

**Plugin Stalls:** Helper isolation + μs budget watchdog + quarantine prevents RT thread contamination.

**Missing RT Privileges (Linux/Win):** Safe Mode, power hints, graceful degradation handle privilege limitations.

**Anti-Cheat / MP Integrity (DCS):** User-installed Export.lua with MP-blocked flags prevents violations.

## Implementation Notes

**Rust-Specific Considerations:**
- Add panic = "abort" for RT threads (feature-gated)
- Use f64 on compile/merge side; cast to f32 for RT frames
- Avoid HidD_SetOutputReport on Windows hot path; use overlapped WriteFile
- Pin one RT thread per device; share the bus; isolate panels/telemetry off RT cores
- Adopt jemalloc/mimalloc globally (optional), but assert no allocations on RT threads at runtime

**Ownership & Velocity:**
- **RT Spine:** Systems developer focused on timing and performance
- **Adapters:** Integration developer for sim-specific implementations  
- **Panels/UI:** Application developer for user-facing features
- **Diagnostics:** QA/Tools engineer for observability and CI
- **Security/Packaging:** Infrastructure engineer for deployment and security

This enhanced implementation plan provides the timing discipline, governance, and operational guardrails necessary to ship a boring-reliable spine that maintains its performance guarantees when the real world pushes back.