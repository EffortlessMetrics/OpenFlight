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

*   📅 **Windows RT Optimization**:
    *   MMCSS registration ("Games" / "Pro Audio" profiles).
    *   High-Res Waitable Timers (1μs resolution).
    *   Power Management (prevent core parking/throttling).
    *   HID Overlapped I/O for non-blocking writes.
*   📅 **Linux RT Optimization**:
    *   `rtkit` integration for unprivileged RT scheduling.
    *   `CLOCK_MONOTONIC` absolute timers.
    *   `udev` rules for low-latency HID access.
*   📅 **Jitter & Latency Validation**:
    *   Hardware-backed CI gates for jitter (p99 < 0.5ms).
    *   End-to-end HID latency measurement harness.

## 3. Packaging & Distribution
**Goal:** User-friendly installers and secure update channels.

*   📅 **Windows Installer**:
    *   Signed MSI using WiX Toolset.
    *   Per-user vs Per-machine scope handling.
    *   Firewall rule automation.
*   📅 **Linux Packaging**:
    *   `.deb` packages with systemd user units.
    *   AppImage for portable distribution.
    *   Post-install scripts for group membership (`input`/`dialout`).
*   📅 **Update System**:
    *   Stable/Beta/Canary channels.
    *   Delta updates to reduce bandwidth.
    *   Automatic rollback on startup crash.

## 4. Telemetry & Observability
**Goal:** Deep visibility into the system for debugging and user trust.

*   ✅ **Blackbox Recorder**: Flight data recorder for FFB and Axis events.
*   📅 **Metrics System**:
    *   In-process counters/histograms (sim.*, ffb.*, runtime.*).
    *   Prometheus exporter (optional).
*   📅 **Dashboard**: Visualization of performance metrics.

## 5. Project Infrastructure
**Goal:** Developer experience and rigorous correctness validation.

*   ✅ **Validation Pipeline**: `cargo xtask validate` for schemas, cross-references, and code quality.
*   ✅ **Spec Ledger**: Traceability from Requirements (REQ/INF-REQ) to Tests and Docs.
*   ✅ **BDD Integration**: Gherkin feature files linked to implementation coverage.
*   ✅ **Local Dev Environment**: Docker-based infra setup.
*   ✅ **Repo Health**: Rust 2024 edition, MSRV 1.92.0 enforcement, CI hardening.
*   ✅ **Cross-Reference Checking**: Automated link validation between Specs, Docs, and Code.
