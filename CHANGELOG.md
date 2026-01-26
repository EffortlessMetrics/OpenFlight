# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Documentation**:
    - Comprehensive X-Plane data group mapping and integration tutorials.
    - Supply chain security audit and "What We Touch" documentation.
    - Cross-reference checking module documentation (`docs/dev/CROSS_REF_MODULE.md`).
- **Flight Hub Core**:
    - Complete FFB Safety Envelope with 50ms ramp-down guarantee.
    - Blackbox recorder for capturing high-frequency FFB and Axis events (pre/post-fault).
    - Emergency Stop (Software) functionality.
    - Double-curve detector and one-click fix via flight-writers.
- **Sim Integration**:
    - **MSFS**: Full SimConnect adapter with unit-safe telemetry mapping.
    - **X-Plane**: UDP and Plugin-based adapter.
    - **DCS**: Export.lua generation and secure integration (MP integrity checks).
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
    - Pinned MSRV to 1.89.0.
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
