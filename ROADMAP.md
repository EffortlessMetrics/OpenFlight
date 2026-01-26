# OpenFlight Roadmap

This roadmap outlines the development path for the OpenFlight ecosystem, organized by major initiatives.

**Legend:**
*   ✅ **Complete**: Implemented and verified.
*   🚧 **In Progress**: Active development or partially implemented.
*   📅 **Planned**: Specified but not yet started.

## 1. Flight Hub Core (Real-Time Axis Processing)
**Goal:** A boring-reliable 250Hz axis processing spine with sub-millisecond jitter.

*   ✅ **Milestone 0: Foundation**: Workspace, CI, IPC schema, Virtual Device.
*   ✅ **Milestone 1: Axis Engine**: Zero-alloc pipeline, Curves, Deadzones, Detents, Mixers.
*   ✅ **Milestone 2: Safety & Watchdogs**: Safety Envelope, Interlock, Fault detection.
*   ✅ **Milestone 3: Sim Adapters**: MSFS (SimConnect), X-Plane (UDP/Plugin), DCS (Export.lua).
*   ✅ **Milestone 4: Panels & UI**: StreamDeck integration, Rules DSL, Panel drivers.
*   ✅ **Milestone 5: FFB Modes**: Force Feedback synthesis, Trim logic, Stall effects.
*   ✅ **Milestone 6: Diagnostics**: Blackbox recorder, Replay harness, Performance tracing.
*   🚧 **Milestone 7: Production Readiness**: Service wiring, CLI parity.
    *   ✅ Safe Mode & Power Hints
    *   ✅ CLI Parity
    *   📅 Packaging (MSI / Deb) - *See Packaging Initiative*

## 2. Platform Runtime & Timing (The "Real-Time" Promise)
**Goal:** OS-level integration to guarantee timing constraints on Windows and Linux.

*   📅 **Windows RT Optimization**: MMCSS registration, High-Res timers, Power management.
*   📅 **Linux RT Optimization**: rtkit/SCHED_FIFO integration, udev rules.
*   📅 **Jitter & Latency Validation**: Hardware-backed CI gates for jitter (p99 < 0.5ms) and HID latency.

## 3. Packaging & Distribution
**Goal:** User-friendly installers and secure update channels.

*   📅 **Windows Installer**: Signed MSI with WiX, system/user scope handling.
*   📅 **Linux Packaging**: `.deb` packages with udev rules, potentially AppImage/RPM.
*   📅 **Update System**: Stable/Beta/Canary channels with rollback capabilities.
*   📅 **Legal Posture**: License inventory and "What We Touch" documentation per simulator.

## 4. Telemetry & Observability
**Goal:** Deep visibility into the system for debugging and user trust.

*   ✅ **Blackbox Recorder**: Flight data recorder for FFB and Axis events.
*   📅 **Metrics System**: System-wide counters/histograms (sim.*, ffb.*, runtime.*).
*   📅 **Dashboard**: Visualization of performance metrics.

## 5. Project Infrastructure
**Goal:** Developer experience and rigorous correctness validation.

*   ✅ **Validation Pipeline**: `cargo xtask validate` for schemas, cross-references, and code quality.
*   ✅ **Spec Ledger**: Traceability from Requirements (REQ/INF-REQ) to Tests and Docs.
*   ✅ **BDD Integration**: Gherkin feature files linked to implementation coverage.
*   ✅ **Local Dev Environment**: Docker-based infra setup.
*   ✅ **Repo Health**: Rust 2024 edition, MSRV 1.89.0 enforcement, CI hardening.
