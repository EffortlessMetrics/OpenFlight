# flight-core

Core data structures, schemas, and utilities for Flight Hub's profile management and validation system.

## Overview

The flight-core crate provides the foundational components for Flight Hub's deterministic profile system, including JSON schema validation, canonical profile representation, and merge semantics. This crate implements the core data models that ensure consistent behavior across all Flight Hub components.

## Key Features

- **JSON Schema Validation**: flight.profile/1 schema with line/column error reporting
- **Canonical Profiles**: Deterministic profile representation with stable hashing
- **Merge Semantics**: Hierarchical profile merging with conflict resolution
- **Monotonic Curves**: Validation and enforcement of monotonic axis curves
- **Zero-Allocation Parsing**: Efficient profile parsing for real-time constraints

## Architecture

This crate implements key architectural decisions:

- **[ADR-002: Writers as Data Pattern](../../docs/adr/002-writers-as-data.md)** - Table-driven configuration management
- **[ADR-004: Zero-Allocation Constraint](../../docs/adr/004-zero-allocation-constraint.md)** - Memory-efficient profile operations

## Core Components

### Profile Management

```rust
use flight_core::{Profile, ProfileMerger, ValidationError};

// Load and validate profile
let profile = Profile::from_json(json_str)?;

// Merge profiles with hierarchy: Global → Sim → Aircraft → PoF
let merger = ProfileMerger::new();
let merged = merger
    .with_global(global_profile)
    .with_sim(sim_profile)
    .with_aircraft(aircraft_profile)
    .merge()?;

// Get canonical representation
let canonical_hash = merged.canonical_hash();
```

### Schema Validation

```rust
use flight_core::{ProfileValidator, ValidationResult};

let validator = ProfileValidator::new();
match validator.validate(&profile_json) {
    Ok(profile) => println!("Profile valid"),
    Err(ValidationError::Schema { line, column, message }) => {
        eprintln!("Validation error at {}:{}: {}", line, column, message);
    }
}
```

### Curve Validation

```rust
use flight_core::{Curve, CurveValidator};

let curve = Curve::new(points);
let validator = CurveValidator::new();

// Ensure monotonic curves
if !validator.is_monotonic(&curve) {
    return Err("Non-monotonic curve detected");
}
```

## Profile Schema

Flight Hub uses the `flight.profile/1` JSON schema:

```json
{
  "schema": "flight.profile/1",
  "sim": "msfs",
  "aircraft": {"icao": "C172"},
  "axes": {
    "pitch": {
      "deadzone": 0.03,
      "expo": 0.2,
      "curve": [[0.0, 0.0], [1.0, 1.0]],
      "slew_rate": 1.2
    }
  },
  "pof_overrides": {
    "approach": {
      "axes": {"pitch": {"expo": 0.25}},
      "hysteresis": {"enter": {"ias": 90}, "exit": {"ias": 100}}
    }
  }
}
```

## Merge Semantics

Profile merging follows deterministic rules:

1. **Scalars**: Last-writer-wins (PoF > Aircraft > Sim > Global)
2. **Arrays**: Keyed merge by identity fields with documented tie-breaks
3. **Curves**: Complete replacement (no interpolation)
4. **Hysteresis**: Additive with conflict detection

## Canonical Representation

Profiles are canonicalized for deterministic hashing:

- Keys sorted alphabetically
- Float precision normalized to 1e-6
- Whitespace normalized
- Comments stripped

This ensures identical profiles produce identical hashes across machines.

## Performance

- **Validation**: Sub-millisecond for typical profiles
- **Merging**: Zero-allocation for hot path operations
- **Hashing**: Consistent timing regardless of profile size
- **Memory**: Minimal allocation with string interning

## Testing

```bash
# Run core tests
cargo test --package flight-core

# Run property-based tests for merge determinism
cargo test --package flight-core test_merge_determinism -- --ignored

# Validate schema compliance
cargo test --package flight-core test_schema_validation
```

## Requirements

This crate satisfies the following requirements:

- **PRF-01**: Profile management and aircraft auto-switching
- **PRF-02**: Deterministic profile merging and validation
- **NFR-01**: Performance constraints for real-time operation

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.