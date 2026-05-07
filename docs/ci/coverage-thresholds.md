# Coverage Thresholds & Ratcheting

OpenFlight uses Codecov thresholds to prevent coverage regressions while accommodating the distinct maturity levels of different crate categories. Thresholds are **execution-surface metrics only**—they measure code path coverage, not correctness.

## Threshold Strategy

### Blocking vs Advisory

**Blocking (rust-core):**
- `flight-core`, `flight-axis`, `flight-bus`, `flight-scheduler`, `flight-rules`, `flight-profile`, and supporting crates
- Project target: **70%**
- Patch target: **80%**
- Regression threshold: **2%**
- PR check: **Fails CI** if thresholds not met
- Rationale: Core control-plane is stable, changes should maintain coverage

**Advisory (rust-hardware, rust-adapters, rust-ffb):**
- Hardware layer, simulator adapters, FFB systems
- Project targets: 60% (hardware), 50% (adapters), 55% (FFB)
- Patch targets: 70%, 65%, 70% respectively
- Regression threshold: **None** (informational only)
- PR check: **Reports but doesn't block** CI
- Rationale: These layers require hardware/integration testing; unit coverage alone is insufficient for correctness claims

### Ratcheting Mechanics

Coverage ratcheting prevents regression via:

1. **Carryforward**: Each flag maintains history of coverage across commits
2. **Threshold checks**: Codecov compares current coverage against target + regression threshold
3. **Per-patch validation**: New code must meet patch targets (tighter than project targets)

**Example:**
```
rust-core flag: last commit had 72% coverage
New commit adds code covering 3 additional paths but removes coverage from 2 paths
Result: 71% coverage

Evaluation:
- Project target: 70% ✅ (71 >= 70)
- Regression check: 72% - 2% = 70% threshold, new is 71% ✅
- Patch target: 80% ❌ (patch coverage is only 65%)
→ PR passes with patch warning
```

## Per-Flag Targets

| Flag | Category | Project | Patch | Regression | Blocking |
|------|----------|---------|-------|------------|----------|
| rust-core | Control Plane | 70% | 80% | 2% | ✅ Yes |
| rust-hardware | Hardware | 60% | 70% | — | ❌ Advisory |
| rust-adapters | Simulators | 50% | 65% | — | ❌ Advisory |
| rust-ffb | Force Feedback | 55% | 70% | — | ❌ Advisory |

## What These Thresholds Prove

✅ **Execution-surface coverage**: Code paths exercised by unit tests
✅ **Regression detection**: Catches unmocked code changes
✅ **Test completeness**: Most code has corresponding test cases

## What These Thresholds Don't Prove

❌ **Functional correctness**: Test coverage ≠ test correctness
❌ **Real-time deadline compliance**: Timing tested via QG-RT-JITTER gate
❌ **Hardware device correctness**: Requires actual hardware (QG-HID-LATENCY)
❌ **Simulator integration**: Requires black-box simulator testing
❌ **Force feedback safety**: Requires QG-FFB-SAFETY gate + fuzzing
❌ **Release readiness**: Requires BDD specs + fuzz targets + hardware validation

## Interpreting Coverage Reports

### Healthy Signal

- rust-core trending 70-75% and stable
- rust-hardware 60%+, covering device abstractions
- rust-adapters 50%+, covering protocol parsing
- rust-ffb 55%+, covering safety envelope logic

### Warning Signs

- rust-core dropping below 70% → Changes removing safety checks
- Hardware/adapter coverage flat → New crates added without tests
- FFB coverage declining → Safety logic untested

### Not a Problem

- rust-hardware at 40% on a PR → Optional/advisory, hardware testing handles correctness
- Simulator adapter at 45% → Requires simulator; unit tests insufficient
- FFB at 50% on large change → Requires QG-FFB-SAFETY; coverage is supporting metric

## Raising Thresholds

Thresholds increase when:

1. **Maturity milestone reached**: Once hardware tests stabilize (QG-HID-LATENCY), hardware flag target can increase
2. **Test infrastructure added**: When new integration tests run in CI, targets reflect new coverage capacity
3. **Debt paid down**: After major safety/correctness improvements, tighter targets enforce continued diligence

Current thresholds assume:
- Unit tests only (no hardware runners in default CI)
- Black-box simulator testing out-of-band (not in CI)
- FFB safety validated via dedicated gate (not unit tests)

## Threshold Governance

Thresholds are **policy**, not implementation detail:

- Changes require **ADR** (Architecture Decision Record) or team consensus
- Raises require evidence (e.g., "hardware gate now running, justify 65%→75%")
- Lowering requires **justification with sunset date** (e.g., "temporary 65%→60% while refactoring, revert by 2026-07-01")
- Governance via `codecov.yml` in main branch (no local overrides)

## See Also

- `docs/ci/coverage.md` - Coverage overview and claim boundaries
- `docs/ci/coverage-flags.md` - Flag scopes and semantics
- `.github/workflows/coverage.yml` - Coverage workflow definition
- `codecov.yml` - Threshold configuration
