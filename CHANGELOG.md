# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Vendor Partnerships** (REQ-49):
    - `flight-ffb-vpforce`: VPforce Rhino V2/V3 FFB base driver — 20-byte HID report parsing, Spring/Damper/Sine/ConstantForce/StopAll effect serialisation, health monitor, Hall-effect presets.
    - `flight-hotas-winwing`: WinWing HOTAS driver — Orion2 Throttle (24-byte), Orion2 Stick (12-byte), TFRP Rudder (8-byte) parsers, health monitor, recommended presets.
    - `flight-ffb-moza`: Moza AB9/R3 FFB base driver — 16-byte HID report parsing, TorqueCommand serialisation with safety clamping, health monitor (torque-fault support), Hall-effect presets.
    - BDD spec `specs/features/req_49_vendor_partnerships.feature` with 16 scenarios; REQ-49 with 9 ACs in `specs/spec_ledger.yaml`.
- **Documentation**:
    - Comprehensive X-Plane data group mapping and integration tutorials.
    - Supply chain security audit and "What We Touch" documentation.
    - Cross-reference checking module documentation (`docs/dev/CROSS_REF_MODULE.md`).
- **Flight Hub Core**:
    - Complete FFB Safety Envelope with 50ms ramp-down guarantee.
    - Blackbox recorder for capturing high-frequency FFB and Axis events (pre/post-fault).
    - Emergency Stop (Software) functionality.
    - Double-curve detector and one-click fix via flight-writers.
- **Packaging & Distribution**:
    - **Windows installer**: WiX 3.x MSI with per-machine scope, auto-start Windows service (FlightHub), service recovery policy, Start Menu + Desktop shortcuts, optional PATH registration (`installer/wix/`).
    - **Linux packaging**: Debian `.deb` package with udev rules, systemd user service unit, postinst/postrm scripts, and automated `installer/debian/build.sh` build script.
    - **Auto-update CLI**: `flightctl update check|channel|channels` subcommand exposing Stable/Beta/Canary channels with persisted per-user channel preference; backed by `flight-updater` crate (channels, rollback, delta patching, Ed25519 signature verification).
- **Hardware Support — T.Flight HOTAS**:
    - **PC Mode Detector** (`flight-hotas-thrustmaster::pc_mode`): `PcModeDetector` classifies incoming HID reports by length to distinguish PC mode (≥8 bytes, Green LED) from Console mode (<8 bytes, Red LED); configurable confirmation threshold prevents false transitions; `console_mode_guidance()` returns step-by-step handshake instructions when console mode is detected.
    - **Throttle Detent Tracker** (`flight-hotas-thrustmaster::detents`): `ThrottleDetentTracker` emits `DetentEvent::Entered`/`Exited` as throttle crosses configured zone boundaries; hysteresis prevents chatter; `ThrottleDetentConfig::hotas4_idle()` preset targets the HOTAS 4 factory notch at 5 % (±2 %).
    - **HID Receipt Fixtures**: 5 synthetic binary fixtures added to `receipts/hid/thrustmaster/tflight-hotas4/` (merged centred, separate centred, separate aux-dominant, button+HAT, console-mode); `meta.json` updated with fixture inventory.
- **Community Cloud Profiles** (`flight-cloud-profiles`):
    - New crate providing an async HTTP client for the Flight Hub community profile repository.
    - `CloudProfileClient` with `list_page`/`list`/`get`/`publish`/`vote`/`remove_vote` methods backed by `reqwest`.
    - `ProfileCache` — TTL-based local disk cache under `~/.cache/flight-hub/cloud-profiles/`; `store`/`get`/`evict`/`clear`/`list_cached` API; entries expire automatically per configurable TTL.
    - `sanitize_for_upload` — normalises profile schema version and lowercases sim slug before publishing.
    - `validate_for_publish` — guards title length (3–80 chars) and axis bounds.
    - `ListFilter` — filter by sim, aircraft ICAO, free-text query, sort order (top-rated / newest / most-downloaded), and pagination.
    - `flightctl cloud-profiles` CLI: `list`, `get`, `publish`, `vote`, `unvote`, `clear-cache` subcommands.
    - REQ-47 BDD spec (`specs/features/req_47_cloud_profiles.feature`) with 16 scenarios; 9 ACs added to spec_ledger.
- **VR Overlay** (`flight-vr-overlay`):
    - New crate providing an in-cockpit VR overlay for notifications, profile status, and axis monitoring.
    - `OverlayConfig` — configurable opacity, scale, anchor point, depth, notification TTL, and panel sections (`show_axis_status`, `show_profile_name`, `show_ffb_status`); `validate()` enforces safe ranges.
    - `NotificationQueue` — capacity-bounded queue with TTL expiry, severity-ordered `active()`, `acknowledge()`, and Critical-item protection (non-Critical items evicted first at capacity).
    - `Severity` — `Info / Warning / Alert / Critical` with `PartialOrd` for comparison and Display for rendering.
    - `OverlayState` — cloneable frame snapshot: profile name, sim name + `SimConnectionStatus`, live `AxisStatus` list, `FfbStatus`, visibility flag.
    - `OverlayRenderer` trait — `render_frame`, `show`, `hide`, `set_opacity`, `backend_name`; `NullRenderer` for tests.
    - `RendererBackend` enum — `Null`, `OpenXr`, `SteamVr` (future feature flags).
    - `OverlayService::spawn()` — tokio-driven ~60 Hz render loop; command channel (`Show/Hide/Toggle/Notify/SetProfile/UpdateState/Shutdown`); `OverlayHandle` for state watch + command send.
    - `flightctl overlay` CLI: `status`, `show`, `hide`, `toggle`, `notify`, `backends` subcommands.
    - REQ-48 BDD spec (`specs/features/req_48_vr_overlay.feature`) with 16 scenarios; 9 ACs added to spec_ledger.
- **Sim Integration**:
    - **MSFS**: Full SimConnect adapter with unit-safe telemetry mapping.
    - **X-Plane**: UDP and Plugin-based adapter.
    - **DCS**: Export.lua generation and secure integration (MP integrity checks).
    - **DCS (enhancements)**: Full unit conversions (IAS/TAS m/s→knots, altitude m→ft, VS m/s→fpm, AoA rad→deg, waypoint distance m→NM); gear/flaps config telemetry via `telemetry_config` feature flag; AoA, angular rates, and navigation (ground track, distance-to-dest) adapter mappings; `dcs mp-policy` CLI subcommand for blocked-feature visibility.
    - **Elite: Dangerous**: Journal file reader with file discovery and byte-offset tailing; `Location`, `FsdJump` (star position), `RefuelAll` protocol support; navigation context in BusSnapshot; README and integration reference doc.
    - **Motion Platform (6DOF)**: New `flight-motion` crate providing washout-filtered 6DOF motion platform output. `WashoutFilter` bank of per-channel HP/LP first-order filters; `MotionMapper` translates `BusSnapshot` (g-forces, bank/pitch angles, yaw rate) into normalized `MotionFrame` (-1..1); `SimToolsUdpOutput` broadcasts SimTools-compatible UDP datagrams (`A{i}B{i}C{i}D{i}E{i}F{i}\n`). Per-channel gain, invert, and enable/disable. Intensity and max-G/max-angle-deg scaling.
- **Infrastructure**:
    - New `cargo xtask` based validation pipeline.
    - Cross-reference checking (Requirements ↔ Code ↔ Tests).
    - Gherkin (BDD) feature file parsing and status reporting.

### Changed
- **Data Serialization**:
    - Migrated Blackbox recorder framing to `postcard` for compact, zero-copy serialization.
    - Unified time bases and unit conversions across telemetry systems.
- **Repository Health**:
    - Migrated to Rust 2024 Edition.
    - Pinned MSRV to 1.92.0.
    - Hardened CI workflows with concurrency control and strict timeouts.
    - Standardized error code families (`INF-SCHEMA`, `INF-XREF`, etc.).
- **Flight Core**:
    - Improved PhaseOfFlight classification logic (prioritizing high-energy phases).
    - Refactored profile switching logic with metric counters.

### Fixed
- Fixed `flight-virtual` stability issues (abnormal thread exits).
- Resolved `flight-hid` private interface leakage.
- Corrected unit test assertions to be meaningful for unsigned types.
- Fixed meaningless `assert!(value >= 0)` checks in tests.

## [0.1.0] - Previous Baseline

### Added
- Initial Axis Processing Engine (flight-axis).
- Basic Flight Service architecture.
- Flight CLI foundation.
- Initial support for StreamDeck panels.
