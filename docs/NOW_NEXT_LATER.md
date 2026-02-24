# Priorities: Now, Next, Later

This document tracks the immediate focus, near-term goals, and long-term vision for the OpenFlight ecosystem.

**Last Updated:** 2026-02-24

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
    *   [ ] **Windows Installer**: Build signed MSI with WiX, handling per-user vs per-machine scope.
    *   [ ] **Linux Packaging**: Create `.deb` packages with correct udev rules and systemd units.
    *   [ ] **Auto-Update**: Implement update channels (Stable/Beta) and rollback mechanism.

*   **Telemetry & Observability (Phase 6)**
    *   [x] **Metrics System**: Implement system-wide counters (`sim.*`, `ffb.*`) and `flight-dashboard`.
    *   [x] **Remote Diagnostics**: Allow users to export sanitized blackbox logs for support.

*   **Sim Integration Polish**
    *   [ ] **DCS**: Finalize MP-safe enforcement and "blocked feature" UI warnings.
    *   [x] **X-Plane**: Add more comprehensive data refs for complex aircraft.

## 🔴 Later (Long-Term Vision & Ideas)
**Focus:** Ecosystem Growth, Advanced Hardware, and Community.

*   **Hardware Ecosystem**
    *   [ ] **Open Hardware**: Reference design for an OpenFlight-native FFB stick (firmware + PCB).
    *   [ ] **Vendor Partnerships**: Official support for Moza, WinWing, VPforce devices.

*   **Advanced Features**
    *   [ ] **Cloud Profiles**: Community-shared profile repository with voting/rating.
    *   [ ] **Motion Platform Support**: extend axis engine to 6DOF motion rigs.
    *   [ ] **VR Overlay**: In-cockpit overlay for profile tuning and notifications.

*   **Platform Expansion**
    *   [ ] **macOS Support**: Core loop porting (IOKit/HID) for X-Plane on Mac.
