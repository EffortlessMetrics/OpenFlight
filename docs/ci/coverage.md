# Coverage

Codecov coverage is Rust execution-surface evidence.

It answers:

> Did tests execute this Rust surface?

It does not answer:

- whether real-time deadlines are met,
- whether physical HID devices behave correctly,
- whether simulator adapters are correct,
- whether force-feedback output is safe,
- whether hardware-in-the-loop testing passed,
- whether BDD feature coverage is complete,
- whether fuzzing is sufficient,
- whether release readiness is proven.

Those are separate proof lanes.

## Coverage Workflow

The Coverage workflow runs on:

- push to `main`,
- `workflow_dispatch`,
- PRs labeled `coverage`, `full-ci`, or `ci:full`.

### Scope

The initial Codecov flag is `rust-core`, scoped to portable core/control-plane crates:

- flight-core
- flight-axis
- flight-bus
- flight-scheduler
- flight-rules
- flight-profile
- flight-units
- flight-session
- flight-metrics
- flight-device-common
- flight-hid-types
- flight-adapter-common
- flight-test-helpers
- flight-workspace-meta

Hardware, simulator adapter, and force-feedback correctness are measured by separate testing lanes.

### Artifacts

Durable receipts from the Coverage workflow are:

- `coverage.json` — machine-readable coverage summary
- `coverage.txt` — human-readable coverage report
- `lcov.info` — LCOV format for Codecov upload
- GitHub Actions `coverage-report` artifact (14-day retention)
- Codecov dashboard (permanent link)

Codecov comments are disabled (quiet mode).

## Claim Boundary

Codecov coverage is **not**:

- proof of real-time deadline correctness,
- proof of hardware/device correctness,
- proof of simulator adapter correctness,
- proof of force-feedback safety,
- proof of HIL readiness,
- proof of BDD scenario completeness,
- proof of fuzz target robustness,
- proof of release readiness.

Each of those is a separate testing lane with dedicated receipts.
