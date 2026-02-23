---
doc_id: DOC-CI-REQUIRED-CHECKS
kind: reference
area: ci
status: active
links:
  requirements: [INF-REQ-9]
  tasks: []
  adrs: []
---

# CI Required Checks Configuration

This document describes the required status checks that should be configured in GitHub repository settings for PR approval.

## Required Checks for PR Approval

The following checks must pass before a PR can be merged:

### Clippy Checks
- `clippy-core / Clippy - flight-core (ubuntu-latest)`
- `clippy-core / Clippy - flight-core (windows-latest)`
- `clippy-ipc-benches / Clippy - IPC Benches (strict) (ubuntu-latest)`
- `clippy-ipc-benches / Clippy - IPC Benches (strict) (windows-latest)`

### Public API Checks
- `public-api-check / Public API Guard`

### Test Checks
- `test / Test Suite (ubuntu-latest, stable)`
-- `test / Test Suite (ubuntu-latest, 1.92.0)`
- `test / Test Suite (windows-latest, stable)`
-- `test / Test Suite (windows-latest, 1.92.0)`

## Configuration Steps

To configure these required checks in GitHub:

1. Go to repository Settings → Branches
2. Add or edit branch protection rule for `main` branch
3. Enable "Require status checks to pass before merging"
4. Search for and select the checks listed above
5. Enable "Require branches to be up to date before merging" (optional but recommended)

## Optional Checks

The following checks are informational and not required for merge:

- `clippy-ipc-benches / Clippy - IPC Benches (unblock)` - Only runs when `clippy-unblock` label is applied
- `gated-ipc-smoke` - Runs on schedule or with `run-gated` label
- `gated-hid-smoke` - Runs on schedule or with `run-gated` label
- `cross-platform` - Runs on schedule only
- `feature-powerset` - Runs on all PRs but not required for merge

## Notes

- The `clippy-core` job includes formatting checks (`cargo fmt --all -- --check`)
- The `clippy-ipc-benches` job has two modes:
  - **strict**: Runs with workspace dependencies (default, required)
  - **unblock**: Runs with `--no-deps` flag (label-gated, optional)
-- All clippy jobs use pinned toolchain `1.92.0` for deterministic builds
- The `public-api-check` job automatically retries with nightly toolchain if rustdoc-json fails
