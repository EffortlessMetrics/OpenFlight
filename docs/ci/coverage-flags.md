# Codecov Coverage Flags

OpenFlight coverage is tracked using four separate Codecov flags, each representing a distinct subsystem. This allows independent tracking of execution-surface coverage across control-plane, hardware, simulator, and safety layers.

## Flags

### `rust-core` (Portable Control Plane)

**Scope:** Core control-plane and support crates

**Crates:**
- `flight-core` - Core types and profile management
- `flight-axis` - 250Hz axis processing engine
- `flight-bus` - Event bus for inter-component communication
- `flight-scheduler` - Platform real-time scheduling
- `flight-rules` - Rule engine for control logic
- `flight-profile` - Profile schema and validation
- `flight-units` - Unit conversion and types
- `flight-session` - Session management
- `flight-metrics` - Metrics and observability
- `flight-device-common` - Common device abstractions
- `flight-hid-types` - HID type definitions
- `flight-adapter-common` - Simulator adapter abstractions
- `flight-test-helpers` - Testing utilities
- `flight-workspace-meta` - Workspace metadata

**Claim Boundary:** Execution-surface evidence only
- ✅ Code path coverage
- ❌ Real-time deadline correctness
- ❌ Release readiness

**Status:** Primary flag, failing CI = blocking

---

### `rust-hardware` (HID & Device Layer)

**Scope:** Hardware abstraction and HID device management

**Crates:**
- `flight-hid` - HID device I/O
- `flight-hid-support` - Platform HID support
- `flight-device-common` - Device abstractions
- `flight-virtual` - Virtual device harness

**Claim Boundary:** Execution-surface evidence only
- ✅ Code path coverage for device abstraction
- ❌ Actual HID device correctness (requires hardware)
- ❌ Latency guarantees (requires hardware runners)
- ❌ Hot-swap robustness (requires hardware testing)

**Status:** Advisory flag, optional coverage
- Useful for detecting missing code paths
- Not sufficient for hardware release readiness
- Full correctness requires QG-HID-LATENCY hardware gate

---

### `rust-adapters` (Simulator Adapters)

**Scope:** Simulator integration and protocol adapters

**Crates:**
- `flight-simconnect` - MSFS SimConnect adapter
- `flight-xplane` - X-Plane UDP adapter
- `flight-dcs-export` - DCS Export.lua integration
- `flight-adapter-common` - Shared adapter infrastructure

**Claim Boundary:** Execution-surface evidence only
- ✅ Code path coverage for adapter logic
- ❌ Simulator protocol correctness (black-box testing required)
- ❌ Simulator integration boundaries (requires simulator)
- ❌ Round-trip latency (requires simulator)

**Status:** Advisory flag, optional coverage
- Detects missing code paths in adapter stubs
- Simulator correctness requires integration testing
- Protocol compliance tracked separately in BDD specs

---

### `rust-ffb` (Force Feedback & Safety)

**Scope:** Force feedback synthesis and safety systems

**Crates:**
- `flight-ffb` - Force feedback synthesis engine
- `flight-ffb-moza` - Moza wheel FFB
- `flight-ffb-vpforce` - VPforce Rhino FFB
- `flight-tactile` - Tactile feedback

**Claim Boundary:** Execution-surface evidence only
- ✅ Code path coverage for FFB logic
- ❌ Safety envelope correctness (requires QG-FFB-SAFETY gate)
- ❌ Device-specific FFB waveforms (requires hardware testing)
- ❌ Feedback fidelity (requires qualitative testing)

**Status:** Advisory flag, optional coverage
- Unit test coverage for safety checks
- Full FFB safety validation requires QG-FFB-SAFETY hardware gate
- Waveform correctness requires device integration testing

---

## Coverage Targets

| Flag | Required | Target | Enforcement |
|------|----------|--------|-------------|
| `rust-core` | ✅ Yes | 70%+ | Blocking on main |
| `rust-hardware` | ❌ No | 60%+ | Advisory |
| `rust-adapters` | ❌ No | 50%+ | Advisory |
| `rust-ffb` | ❌ No | 55%+ | Advisory |

Targets are advisory pending policy registration; see `docs/NOW_NEXT_LATER.md`.

---

## Interpretation Guide

### When Coverage Increases

**Good signals:**
- New unit tests added for edge cases
- Code paths previously untested are now exercised
- Refactored code maintains coverage

**Caution:**
- Mocked/stubbed code may show high coverage without real correctness
- Simulator adapters can have 90%+ coverage while simulator integration is incomplete
- Safety-critical FFB code may have full coverage but insufficient fuzzing

### When Coverage Decreases

**Acceptable reasons:**
- Dead code removed (coverage % may increase even though metrics decrease)
- Code paths deferred to specialized hardware gates (simulator, FFB safety)
- Temporary test failures on CI runners without required infrastructure

**Action items:**
- Only `rust-core` flag failures block merges
- `rust-hardware`, `rust-adapters`, `rust-ffb` failures are advisory
- Investigate coverage gaps unrelated to new changes

---

## Adding New Crates

When adding a new crate:

1. **Portable core logic?** → Add to `rust-core` flag
2. **Hardware abstraction?** → Add to `rust-hardware` flag  
3. **Simulator integration?** → Add to `rust-adapters` flag
4. **Force feedback?** → Add to `rust-ffb` flag
5. **Multiple layers?** → Add to all applicable flags

Update `.github/workflows/coverage.yml` and document in this file.

---

## See Also

- `docs/ci/coverage.md` - Coverage overview and claim boundaries
- `.github/workflows/coverage.yml` - Workflow definition
- `codecov.yml` - Codecov configuration (quiet mode, thresholds)
