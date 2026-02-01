# Contributing to OpenFlight

Thank you for your interest in contributing! We welcome contributions from everyone.

## Development Workflow

### 1. "Now/Next/Later" Priorities
We use [docs/NOW_NEXT_LATER.md](docs/NOW_NEXT_LATER.md) to track immediate focus. Please check this before picking up new tasks to ensure alignment with current goals.

### 2. Validation Pipeline
All changes must pass the validation pipeline:
```bash
cargo xtask validate
```

### 3. Documentation
We follow the **Diataxis** framework. When adding features:
- **Tutorials**: Learning-oriented (e.g., "Getting Started").
- **How-To**: Problem-oriented (e.g., "How to integrate XInput").
- **Explanation**: Understanding-oriented (e.g., "Concepts").
- **Reference**: Information-oriented (e.g., "API Specs").

See `docs/README.md` for the index.

### 4. Branching Strategy
- `main`: Stable development branch.
- `feat/`: Feature branches.
- `fix/`: Bug fix branches.
- `docs/`: Documentation updates.

## Quality Gates (QG-*)

Flight Hub uses quality gates to ensure releases meet performance and compliance standards. All QG-* checks must pass before merging to `main` or `release/*` branches.

### Quality Gate Overview

| Gate | Description | Runner | Threshold |
|------|-------------|--------|-----------|
| **QG-SANITY-GATE** | Basic compilation and formatting | Any | Must compile, pass fmt |
| **QG-UNIT-CONV** | Unit conversion accuracy | Any | All tests pass |
| **QG-SIM-MAPPING** | Simulator variable mappings | Any | All tests pass |
| **QG-FFB-SAFETY** | Force feedback safety systems | Any | All safety tests pass |
| **QG-LEGAL-DOC** | Required documentation exists | Any | All docs present |
| **QG-RT-JITTER** | Real-time timer jitter | Hardware | p99 ≤ 0.5ms |
| **QG-HID-LATENCY** | HID write latency | Hardware + HID device | p99 ≤ 300μs |

### Running Quality Gates Locally

#### Basic Gates (run on any machine)

```bash
# QG-SANITY-GATE: Basic sanity checks
cargo check --workspace
cargo fmt --all -- --check
cargo test -p flight-core

# QG-UNIT-CONV: Unit conversion tests
cargo test -p flight-units

# QG-FFB-SAFETY: FFB safety tests
cargo test -p flight-ffb safety
cargo test -p flight-ffb envelope

# QG-LEGAL-DOC: Check documentation exists
test -f docs/product-posture.md
test -f docs/explanation/integration/msfs.md
test -f docs/explanation/integration/xplane.md
test -f docs/explanation/integration/dcs.md
```

#### Hardware Gates (require specific hardware)

These tests require dedicated hardware runners and are only enforced on release branches:

```bash
# QG-RT-JITTER: Timer jitter test (10 minutes)
# Requires: Bare metal machine (not VM) for accurate timing
cargo test --release -p flight-scheduler test_timer_jitter -- --ignored --nocapture

# QG-HID-LATENCY: HID latency test (10 minutes)
# Requires: HID device connected (e.g., flight stick with FFB)
cargo test --release -p flight-hid test_hid_latency -- --ignored --nocapture
```

### Gate Enforcement

- **Pull Requests to `main`**: All non-hardware gates must pass
- **Pull Requests to `release/*`**: All gates must pass (including hardware gates)
- **Manual Trigger**: Hardware gates can be triggered manually via workflow dispatch

### Troubleshooting Gate Failures

#### QG-LEGAL-DOC Failures

If this gate fails, ensure all required documentation exists:
- `docs/product-posture.md` - Product positioning statement
- `docs/explanation/integration/msfs.md` - MSFS integration details
- `docs/explanation/integration/xplane.md` - X-Plane integration details
- `docs/explanation/integration/dcs.md` - DCS integration details

#### QG-RT-JITTER Failures

Jitter failures indicate timing issues. Common causes:
- Running on a VM (use bare metal for accurate results)
- Background processes consuming CPU
- Power management throttling the CPU

To debug locally:
```bash
# Run a shorter jitter test for debugging
cargo test --release -p flight-scheduler test_timer_jitter_short -- --ignored --nocapture
```

#### QG-FFB-SAFETY Failures

Safety test failures are critical and must be investigated. Check:
- Safety envelope calculations
- Fault detection timing
- Ramp-to-zero behavior

### Adding New Quality Gates

When adding a new quality gate:
1. Add the job to `.github/workflows/quality-gates.yml`
2. Add it to the `quality-gate-summary` job's `needs` list
3. Document it in this section of CONTRIBUTING.md
4. Ensure it can be run locally for developer testing
