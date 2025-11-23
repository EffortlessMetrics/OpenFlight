# BDD Specification Tests

This crate contains Behavior-Driven Development (BDD) tests for the Flight Hub project using Gherkin scenarios and Cucumber-rs.

## Overview

The specs crate executes Gherkin feature files located in `specs/features/` and validates them against the implementation through step definitions.

## Running Tests

To run all BDD tests:

```bash
cargo test -p specs
```

## Feature Coverage

### REQ-1: Real-Time Axis Processing
- **AC-1.1**: Processing latency under load (≤ 5ms p99)
- **AC-1.2**: Jitter measurement (≤ 0.5ms p99 at 250Hz)

### INF-REQ-1: Structured Documentation System
- **AC-1.1**: Documentation organization into bands
- **AC-1.2**: Front matter validation
- **AC-1.3**: Stable requirement ID references
- **AC-1.4**: Unique doc_id validation
- **AC-1.5**: Documentation index generation
- **AC-1.6**: Crate documentation coverage
- **AC-1.7**: Documentation status updates

## Structure

```
specs/
├── Cargo.toml              # Crate configuration
├── README.md               # This file
├── features/               # Gherkin feature files
│   ├── req_1_axis_processing.feature
│   └── req_inf_1_documentation.feature
└── tests/
    ├── cucumber.rs         # Test runner
    └── steps/              # Step definitions
        ├── mod.rs
        ├── axis_processing.rs
        └── documentation.rs
```

## Adding New Scenarios

1. Create or update a `.feature` file in `specs/features/`
2. Tag scenarios with `@REQ-*` or `@INF-REQ-*` and `@AC-*` tags
3. Implement step definitions in `specs/tests/steps/`
4. Run tests to verify

## CI Integration

BDD tests are automatically run in CI via the `bdd-tests` job in `.github/workflows/ci.yml`.

## Dependencies

- `cucumber` (0.21): BDD test framework
- `tokio`: Async runtime
- `serde_yaml`: YAML parsing for front matter
- `walkdir`: File system traversal
- `regex`: Pattern matching for requirement IDs

## Notes

- Tests are designed to be resilient to missing files (e.g., docs directory)
- Axis processing tests use simulated data for reproducibility
- Documentation tests validate against actual project structure when available
