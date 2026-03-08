# Priorities: Now, Next, Later

This document tracks the immediate focus, near-term goals, and long-term vision for the OpenFlight ecosystem.

**Last Updated:** 2026-03-01

## 🟢 Now (Current Sprint & Immediate Focus)
**Focus:** Hardening, Documentation, and Community Readiness.

*   **Quality & Coverage**
    *   [ ] **Test stabilization**: Reach 10,000 unit/integration tests across the workspace (currently 9,200+).
    *   [ ] **Fuzz hardening**: Expand fuzz targets beyond the current 92 to cover all parser and protocol crates.
    *   [ ] **COMPATIBILITY.md refresh**: Regenerate from the 2,301 device manifests (up from 2,201).

*   **Documentation**
    *   [x] **Tutorials**: Add "Creating your first Profile" tutorial.
    *   [x] **API Docs**: Complete `docs.rs` coverage for `flight-core` and `flight-ipc`.
    *   [ ] **Architecture guide**: Publish end-to-end data flow walkthrough for new contributors.

*   **CI & Automation**
    *   [ ] **Hardware CI gates**: Enable QG-RT-JITTER and QG-HID-LATENCY on dedicated runners.
    *   [ ] **Nightly fuzzing**: Automated nightly fuzz runs with corpus persistence.

## 🟡 Next (Upcoming ~4-8 Weeks)
**Focus:** Ecosystem Expansion and Plugin System.

*   **Plugin System (ADR-003)**
    *   [ ] **WASM sandbox**: Finalize plugin host with capability declarations and 20–120 Hz tick budget.
    *   [ ] **Native fast-path**: Shared-memory SPSC channel for latency-critical plugins.
    *   [ ] **Plugin marketplace**: Basic catalog and discovery mechanism.

*   **Vendor Driver Expansion**
    *   [ ] **Brunner CLS-E**: Complete force feedback integration via `flight-hotas-brunner`.
    *   [ ] **SimuCube / VPforce**: Production-ready FFB profiles for direct-drive wheels.
    *   [ ] **GoFlight panels**: Full MCP/EFIS panel coverage via `flight-panels-goflight`.

*   **Sim Integration Depth**
    *   [x] **DCS**: Finalize MP-safe enforcement, blocked feature UI warnings, and full telemetry with unit conversions.
    *   [x] **X-Plane**: Add more comprehensive data refs for complex aircraft.
    *   [x] **Elite: Dangerous**: Journal/Status file-watcher adapter (`flight-elite`) with gear, lights, fuel, star-system tracking.
    *   [ ] **IL-2 / Falcon BMS**: Shared-memory telemetry adapters.

## 🔴 Later (Long-Term Vision & Ideas)
**Focus:** Ecosystem Growth, Advanced Hardware, and Community.

*   **Hardware Ecosystem**
    *   [x] **Open Hardware**: Reference design for an OpenFlight-native FFB stick (firmware + PCB).
    *   [x] **Vendor Partnerships**: Official support for Moza, WinWing, VPforce devices.
    *   [x] **T.Flight HOTAS Gap Fill**: PC mode detection, throttle detent tracking, and HID receipt fixtures.

*   **Advanced Features**
    *   [x] **Cloud Profiles**: Community-shared profile repository with voting/rating.
    *   [x] **Motion Platform Support**: extend axis engine to 6DOF motion rigs.
    *   [x] **VR Overlay**: In-cockpit overlay for profile tuning and notifications.

*   **Platform Expansion**
    *   [x] **macOS Support**: Core loop porting (IOKit/HID) for X-Plane on Mac.
