# Priorities: Now, Next, Later

This document tracks the immediate focus, near-term goals, and long-term vision for the OpenFlight ecosystem.

**Last Updated:** 2026-02-25

## 🟢 Now (Current Sprint & Immediate Focus)
**Focus:** Production Readiness, Runtime Reliability, and Packaging Prep.

*   **Production Readiness (M7)**
    *   [x] **Service Wiring**: Finalize `flight-service` orchestration for app/use-cases layer.
    *   [x] **Safe Mode**: Complete `--safe` implementation with power checks and privilege detection.
    *   [x] **CLI Parity**: Ensure all service functionality is accessible via `flight-cli --json`.

*   **Runtime & Timing (Phase 4)**
    *   [x] **Windows RT**: Implement `MMCSS` registration and high-res timer loop.
    *   [x] **Linux RT**: Integrate `rtkit` for unprivileged real-time scheduling.
    *   [x] **Jitter Validation**: Harden CI gates for p99 jitter < 0.5ms on hardware runners.

*   **Documentation Refinement**
    *   [x] **Tutorials**: Add "Creating your first Profile" tutorial.
    *   [x] **API Docs**: Complete `docs.rs` coverage for `flight-core` and `flight-ipc`.

## 🟡 Next (Upcoming ~4-8 Weeks)
**Focus:** Distribution, User Experience, and Telemetry.

*   **Packaging & Distribution (Phase 5)**
    *   [x] **Windows Installer**: Build signed MSI with WiX (per-machine scope with auto-start Windows service; `installer/wix/`).
    *   [x] **Linux Packaging**: `.deb` package with udev rules, systemd user unit, and automated `installer/debian/build.sh`.
    *   [x] **Auto-Update**: `flightctl update check|channel|channels` with Stable/Beta/Canary channels and persisted channel preference (`flight-updater` crate).

*   **Telemetry & Observability (Phase 6)**
    *   [x] **Metrics System**: Implement system-wide counters (`sim.*`, `ffb.*`) and `flight-dashboard`.
    *   [x] **Remote Diagnostics**: Allow users to export sanitized blackbox logs for support.

*   **Sim Integration Polish**
    *   [x] **DCS**: Finalize MP-safe enforcement, blocked feature UI warnings (`dcs mp-policy`), and full telemetry (gear/flaps/AoA/angular rates/navigation) with correct unit conversions.
    *   [x] **X-Plane**: Add more comprehensive data refs for complex aircraft.
    *   [x] **Elite: Dangerous**: Journal/Status file-watcher adapter (`flight-elite`) with gear, lights, fuel, star-system tracking.

## 🔴 Later (Long-Term Vision & Ideas)
**Focus:** Ecosystem Growth, Advanced Hardware, and Community.

*   **Hardware Ecosystem**
    *   [ ] **Open Hardware**: Reference design for an OpenFlight-native FFB stick (firmware + PCB).
    *   [x] **Vendor Partnerships**: Official support for Moza, WinWing, VPforce devices.
    *   [x] **T.Flight HOTAS Gap Fill**: PC mode detection (`PcModeDetector`), throttle detent tracking (`ThrottleDetentTracker`), and HID receipt fixtures for the Thrustmaster T.Flight HOTAS 4/One.

*   **Advanced Features**
    *   [x] **Cloud Profiles**: Community-shared profile repository with voting/rating.
    *   [x] **Motion Platform Support**: extend axis engine to 6DOF motion rigs.
    *   [x] **VR Overlay**: In-cockpit overlay for profile tuning and notifications.

*   **Platform Expansion**
    *   [ ] **macOS Support**: Core loop porting (IOKit/HID) for X-Plane on Mac.
