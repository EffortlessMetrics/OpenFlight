---
doc_id: DOC-HOWTO-TESTS
kind: how-to
area: ci
status: draft
links:
  requirements: ["INF-REQ-10"]
  tasks: []
  adrs: []
---

# How to Run Tests

This guide explains how to run tests in the Flight Hub project.

## Quick Start

### Run All Tests

```bash
cargo test
```

### Run Tests for a Specific Crate

```bash
cargo test -p flight-core
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

## Test Categories

### Unit Tests

Unit tests are located alongside the source code in each crate:

```bash
# Run unit tests for flight-core
cargo test -p flight-core
```

### Integration Tests

Integration tests are in the `tests/` directory of each crate:

```bash
# Run integration tests
cargo test --test integration_tests
```

### Benchmarks

Performance benchmarks use the criterion framework:

```bash
# Run benchmarks
cargo bench
```

## Using xtask

The project provides a unified testing interface through xtask:

```bash
# Fast smoke test (fmt, clippy, core tests)
cargo xtask check

# Full validation (all tests, benches, API checks)
cargo xtask validate
```

## Continuous Integration

The CI pipeline runs the full test suite on every pull request. Tests must pass before merging.

See `infra/ci/README.md` for details on CI configuration.
