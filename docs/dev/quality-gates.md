# Quality Gates

This document describes the CI quality gates implemented for Flight Hub, as defined in the sim-integration-implementation spec.

## Overview

Quality gates are automated checks that enforce critical requirements before code can be merged or released. They are implemented in the `xtask` automation tool and run as part of `cargo xtask validate`.

## Implemented Quality Gates

### QG-SIM-MAPPING

**Status:** ✅ Implemented

**Purpose:** Verify that all simulator adapters have complete mapping documentation.

**Requirements:**
- `docs/integration/msfs-simvar-mapping.md` - MSFS SimConnect adapter mapping
- `docs/integration/xplane-data-groups.md` - X-Plane adapter mapping  
- `docs/integration/dcs-export-api.md` - DCS Export.lua adapter mapping

**Failure Condition:** Build fails if any of the required mapping files are missing.

**Rationale:** Per sim-integration-implementation spec requirements MSFS-INT-01.Doc.*, XPLANE-INT-01.Doc.*, and DCS-INT-01.Doc.*, each adapter must maintain a complete mapping table documenting how simulator-native data maps to the canonical BusSnapshot structure. This ensures:
- Developers can understand the data flow
- Unit conversions are documented and verifiable
- Mapping completeness can be audited

### QG-UNIT-CONV

**Status:** ✅ Implemented

**Purpose:** Verify that unit conversion tests cover all BusSnapshot fields populated by each v1 adapter.

**Requirements:** Unit tests must exist for all conversions:
- Degrees ↔ Radians (for attitude angles, AoA, sideslip, wind direction)
- Knots ↔ m/s (for IAS, TAS, ground speed, wind speed)
- Feet ↔ Meters (for altitudes: MSL, AGL, pressure altitude)
- FPM ↔ m/s (for vertical speed)

**Failure Condition:** Build fails if any of the required unit conversion test functions are missing from `crates/flight-bus/src/snapshot.rs`.

**Rationale:** Per sim-integration-implementation spec requirements BUS-CORE-01.12 and SIM-TEST-01.2, all unit conversions must be tested to ensure correct data transformation from simulator-native units to the canonical SI units used in BusSnapshot. This prevents subtle bugs where incorrect conversion factors or missing conversions could lead to incorrect FFB output or profile behavior.

### QG-SANITY-GATE

**Status:** ✅ Implemented

**Purpose:** Verify that sanity gate tests inject NaN/Inf and implausible jumps, and verify proper handling.

**Requirements:** Tests must inject:
- NaN values in telemetry fields
- Inf values in telemetry fields
- Physically implausible jumps (attitude, velocity)
- Verification that `safe_for_ffb` goes false when violations occur

**Failure Condition:** Build fails if any of the required sanity gate test categories are missing from `crates/flight-simconnect/tests/sanity_gate_tests.rs`.

**Rationale:** Per sim-integration-implementation spec requirements MSFS-INT-01.15, MSFS-INT-01.16, and SIM-TEST-01.9, sanity gate tests must verify that the adapter correctly detects and handles invalid telemetry data. This ensures:
- NaN/Inf values are detected and marked invalid
- Physically implausible jumps are detected (e.g., 90° pitch change in 16ms)
- The `safe_for_ffb` flag is set to false when violations occur
- The system transitions to Faulted state when violation thresholds are exceeded

**Test Coverage:** The quality gate verifies that tests exist for:
- NaN detection (e.g., `test_nan_detection_in_angular_rates`)
- Inf detection (e.g., `test_inf_detection_in_angular_rates`)
- Implausible jump detection (e.g., `test_implausible_pitch_jump_detection`, `test_implausible_velocity_jump_detection`)
- safe_for_ffb behavior (e.g., `test_safe_for_ffb_false_in_faulted`, `test_safe_for_ffb_true_in_active_flight`)

### QG-FFB-SAFETY

**Status:** ✅ Implemented

**Purpose:** Verify that FFB safety tests validate torque ramp-down within 50ms on all fault types.

**Requirements:** Tests must verify:
- 50ms ramp-to-zero timing on fault detection
- Fault detection for all fault types (USB stall, NaN, over-temp, over-current, endpoint wedged, encoder invalid, device timeout, plugin overrun)
- Soft-stop controller integration with multiple ramp profiles
- Fault timestamp tracking and progress calculation
- Fault overrides safe_for_ffb flag

**Failure Condition:** Build fails if any of the required FFB safety tests are missing from:
- `crates/flight-ffb/src/safety_envelope_tests.rs` - Safety envelope and 50ms ramp tests
- `crates/flight-ffb/src/fault.rs` - Fault detection and recording tests
- `crates/flight-ffb/src/soft_stop.rs` - Soft-stop controller tests

**Rationale:** Per sim-integration-implementation spec requirements FFB-SAFETY-01.5, FFB-SAFETY-01.6, and SIM-TEST-01.10, FFB safety tests must verify that the system can safely ramp torque to zero within 50ms on any fault condition. This ensures:
- Faults trigger immediate safety response (≤50ms to zero torque)
- All fault types are properly detected and handled
- Soft-stop controller provides smooth, controlled ramp-down
- Fault state is properly tracked with timestamps
- Safety takes precedence over all other system states

**Test Coverage:** The quality gate verifies that tests exist for:
- 50ms ramp-to-zero timing (`test_fault_ramp_to_zero_timing`)
- Fault timestamp tracking (`test_fault_timestamp_tracking`)
- Fault overrides safe_for_ffb (`test_fault_overrides_safe_for_ffb`)
- Soft-stop ramp profiles (linear, exponential, S-curve)
- Soft-stop completion and timeout detection
- Fault type properties and error codes
- Fault recording and response completion
- All 9 fault types defined in FaultType enum

## Planned Quality Gates

The following quality gates are defined in the spec but not yet implemented:

### QG-RT-JITTER

**Status:** 📋 Not Started

**Purpose:** Verify that 250Hz p99 jitter ≤0.5ms on hardware-backed CI runners.

**Requirements:** Long-running jitter tests on real hardware; report-only mode on VMs.

### QG-HID-LATENCY

**Status:** 📋 Not Started

**Purpose:** Verify that HID write p99 ≤300μs on hardware-backed CI runners.

**Requirements:** Latency measurement harness on real hardware.

### QG-LEGAL-DOC

**Status:** 📋 Not Started

**Purpose:** Verify that product posture document exists and is referenced in required locations.

**Requirements:** Product posture document must exist and be linked from README, installer, etc.

## Running Quality Gates

### Locally

```bash
# Run all quality gates as part of full validation
cargo xtask validate

# Quality gates are in Step 4 of the validation pipeline
```

### In CI

Quality gates are automatically run as part of the `cargo xtask validate` command in CI pipelines. Failed quality gates will fail the build.

## Implementation

Quality gates are implemented in `xtask/src/quality_gates.rs` and integrated into the validation pipeline in `xtask/src/validate.rs`.

Each quality gate:
1. Returns a `QualityGateResult` with pass/fail status and optional details
2. Is called from the `run_quality_gates()` function
3. Has its result included in the validation report at `docs/validation_report.md`

## Adding New Quality Gates

To add a new quality gate:

1. Add a new function in `xtask/src/quality_gates.rs`:
   ```rust
   pub fn check_my_gate() -> Result<QualityGateResult> {
       // Implement check logic
       if check_passes {
           Ok(QualityGateResult::new("QG-MY-GATE", true))
       } else {
           Ok(QualityGateResult::with_details(
               "QG-MY-GATE",
               false,
               "Failure details here"
           ))
       }
   }
   ```

2. Call it from `run_quality_gates()` in `xtask/src/validate.rs`:
   ```rust
   let my_gate_result = crate::quality_gates::check_my_gate()
       .context("Failed to check my gate")?;
   results.push(my_gate_result);
   ```

3. Document it in this file

4. Update the spec task list to mark the gate as implemented
