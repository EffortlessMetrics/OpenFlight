# Project Infrastructure Design Document

## Overview

The Project Infrastructure system provides a machine-readable framework for maintaining documentation, specifications, and infrastructure-as-code for the Flight Hub project. The design follows a "Kiro-native" philosophy: humans define contracts and schemas, while Kiro (the AI assistant) maintains the concrete artifacts through automated tasks.

The system consists of three primary layers:
1. **Core Primitives**: Structured documentation, spec ledger, Gherkin features, and IaC layouts
2. **Automation Surface**: xtask framework providing unified command entry points
3. **Quality Overlays**: Cross-reference checking, schema validation, and automated reporting

This design prioritizes mechanical checkability over human interpretation, enabling Kiro to validate and maintain artifacts without ambiguity.

## Architecture

### High-Level Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Developer / CI                          │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
         ┌───────────────────────┐
         │   cargo xtask CLI     │
         │  (Automation Layer)   │
         └───────────┬───────────┘
                     │
        ┌────────────┼────────────┐
        │            │            │
        ▼            ▼            ▼
   ┌────────┐  ┌─────────┐  ┌──────────┐
   │ check  │  │validate │  │ac-status │
   └────┬───┘  └────┬────┘  └────┬─────┘
        │           │            │
        ▼           ▼            ▼
┌──────────────────────────────────────────┐
│         Core Primitives Layer            │
├──────────────────────────────────────────┤
│  • specs/spec_ledger.yaml                │
│  • specs/features/*.feature              │
│  • docs/**/*.md (with front matter)      │
│  • infra/**/invariants.yaml              │
│  • schemas/*.json                        │
└──────────────────────────────────────────┘
```

### Data Flow

1. **Write Path**: Developer creates/updates artifacts → Kiro normalizes via xtask → Validation passes → Commit
2. **Read Path**: CI runs xtask validate → Reads all artifacts → Cross-references → Generates reports → Uploads artifacts
3. **Query Path**: Developer runs xtask ac-status → Parses ledger + Gherkin → Generates feature_status.md


## Components and Interfaces

### 1. Spec Ledger (specs/spec_ledger.yaml)

**Purpose**: Machine-readable mapping of requirements to acceptance criteria and tests.

**Schema** (schemas/spec_ledger.schema.json):
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["requirements"],
  "additionalProperties": false,
  "properties": {
    "requirements": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "name", "status", "ac"],
        "additionalProperties": false,
        "properties": {
          "id": {
            "type": "string",
            "pattern": "^(REQ|INF-REQ)-[0-9]+$"
          },
          "name": { "type": "string" },
          "status": {
            "type": "string",
            "enum": ["draft", "implemented", "tested", "deprecated"]
          },
          "ac": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["id", "description"],
              "additionalProperties": false,
              "properties": {
                "id": {
                  "type": "string",
                  "pattern": "^AC-[0-9]+\\.[0-9]+$"
                },
                "description": { "type": "string" },
                "tests": {
                  "type": "array",
                  "items": {
                    "oneOf": [
                      { "type": "string" },
                      {
                        "type": "object",
                        "minProperties": 1,
                        "additionalProperties": false,
                        "properties": {
                          "test": { "type": "string" },
                          "feature": { "type": "string" },
                          "command": { "type": "string" }
                        }
                      }
                    ]
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
```

**Test Reference Format**:

The spec ledger supports two primary test reference formats:

1. **Function-Based Test References** (for unit and integration tests):
   - Simple string format: `"<crate>::<module_path>::<test_fn_name>"`
   - Examples: `"flight_core::aircraft_switch::tests::test_phase_of_flight_determination"`
   - Use case: Product requirements (REQ-*), unit-level checks, specific code behaviors
   - Validation: Cross-reference checker verifies the test function exists in the codebase

2. **Command-Based Test References** (for infrastructure and system-level validation):
   - Format: `"cmd:<shell command>"`
   - Examples: 
     - `"cmd:cargo xtask validate"`
     - `"cmd:cargo test -p specs"`
     - `"cmd:cargo xtask ac-status"`
   - Semantics: Test is validated by running the command and checking exit code (0 = pass, non-zero = fail)
   - Use case: Infrastructure requirements (INF-REQ-*), integration-level checks, system-wide properties
   - Validation: Cross-reference checker recognizes `cmd:` prefix and treats as command-based test

3. **Object Format** (for complex test specifications):
   - Requires at least one of: `test`, `feature`, or `command`
   - Allows mixing multiple test types for comprehensive validation

**Choosing the Right Format**:
- Use **function-based tests** when validating specific code behaviors, algorithms, or data structures
- Use **command-based tests** when validating infrastructure, tooling, or system-wide properties that span multiple components
- Infrastructure requirements (INF-REQ-*) typically use command-based tests
- Product requirements (REQ-*) typically use function-based tests

**Schema Validation Scope**:
JSON Schema enforces structural validity. Status-dependent constraints (e.g., "status: tested ⇒ tests non-empty") are enforced in `cargo xtask validate` via Properties 1 and 8.

**Example**:
```yaml
requirements:
  - id: REQ-1
    name: Real-Time Axis Processing
    status: implemented
    ac:
      - id: AC-1.1
        description: "Processing latency SHALL be ≤ 5ms p99"
        tests:
          - flight-core::axis::tests::test_latency_p99
          - command: cargo bench --bench axis_latency
      - id: AC-1.2
        description: "Jitter SHALL be ≤ 0.5ms p99 at 250Hz"
        tests:
          - flight-scheduler::tests::test_jitter_p99
          - feature: specs/features/req_1_axis_processing.feature:Scenario: Jitter measurement
```


### 2. Documentation Front Matter (docs/**/*.md)

**Purpose**: Machine-readable metadata for documentation cross-referencing and indexing.

**Schema** (schemas/front_matter.schema.json):
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["doc_id", "kind", "area", "status", "links"],
  "additionalProperties": false,
  "properties": {
    "doc_id": {
      "type": "string",
      "pattern": "^DOC-[A-Z0-9-]+$"
    },
    "kind": {
      "type": "string",
      "enum": ["requirements", "design", "concept", "how-to", "reference", "adr"]
    },
    "area": {
      "type": "string",
      "enum": ["flight-core", "flight-virtual", "flight-hid", "flight-ipc", 
               "flight-scheduler", "flight-ffb", "flight-panels", "infra", "ci"]
    },
    "status": {
      "type": "string",
      "enum": ["draft", "active", "deprecated"]
    },
    "links": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "requirements": {
          "type": "array",
          "items": { "type": "string", "pattern": "^(REQ|INF-REQ)-[0-9]+$" }
        },
        "tasks": {
          "type": "array",
          "items": { "type": "string" }
        },
        "adrs": {
          "type": "array",
          "items": { "type": "string", "pattern": "^ADR-[0-9]+$" }
        }
      }
    }
  }
}
```

**Link Semantics**:
- Documentation links to requirements (REQ-* or INF-REQ-*) via `links.requirements`
- Acceptance criteria (AC-*) are linked via Gherkin tags, not front matter
- `links` field is required but may contain empty arrays

**Example**:
```markdown
---
doc_id: DOC-CORE-AXIS-PROCESSING
kind: concept
area: flight-core
status: active
links:
  requirements: [REQ-1, REQ-12]
  tasks: [TASK-3, TASK-7]
---

# Axis Processing Concepts

[Content follows...]
```


### 3. Gherkin Features (specs/features/*.feature)

**Purpose**: Executable specifications linking requirements to behavior scenarios.

**Naming Convention**: `req_<N>_<description>.feature`

**Tag Format**:
- Requirement tags: `@REQ-<N>` or `@INF-REQ-<N>`
- Acceptance criteria tags: `@AC-<N>.<M>`

**Example** (specs/features/req_1_axis_processing.feature):
```gherkin
@REQ-1
Feature: Real-Time Axis Processing

  @AC-1.1
  Scenario: Processing latency under load
    Given a flight-core axis pipeline with 4 axes
    And synthetic telemetry at 250Hz
    When processing 10 minutes of input
    Then p99 latency SHALL be ≤ 5ms

  @AC-1.2
  Scenario: Jitter measurement
    Given a flight-scheduler running at 250Hz
    When measuring tick intervals over 10 minutes
    And excluding the first 5 seconds warm-up
    Then p99 jitter SHALL be ≤ 0.5ms
```

**Cross-Reference Algorithm**:
1. Parse all .feature files in specs/features/
2. Extract @REQ-*, @INF-REQ-*, and @AC-* tags from scenarios
3. Build map: `{req_id: [scenario_locations], ac_id: [scenario_locations]}`
4. Cross-check against specs/spec_ledger.yaml
5. Report missing scenarios for implemented/tested requirements

**Gherkin Coverage Policy**:
- Product requirements (REQ-*) SHOULD have Gherkin scenarios when status is implemented or tested
- Infrastructure requirements (INF-REQ-*) MAY have Gherkin scenarios but are not required
- Both are supported via @REQ-* and @INF-REQ-* tags


### 4. Infrastructure Invariants (infra/**/invariants.yaml)

**Purpose**: Machine-readable contracts for infrastructure environments.

**Schema** (schemas/invariants.schema.json):
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["environment", "rust_version"],
  "properties": {
    "environment": { "type": "string" },
    "rust_version": { "type": "string" },
    "rust_edition": { "type": "string" },
    "ports": {
      "type": "object",
      "patternProperties": {
        "^[a-z-]+$": { "type": "integer" }
      }
    },
    "env_vars": {
      "type": "object",
      "patternProperties": {
        "^[A-Z_]+$": {
          "type": "object",
          "properties": {
            "required": { "type": "boolean" },
            "default": { "type": "string" },
            "description": { "type": "string" }
          }
        }
      }
    },
    "resources": {
      "type": "object",
      "properties": {
        "cpu_limit": { "type": "string" },
        "memory_limit": { "type": "string" }
      }
    }
  }
}
```

**Example** (infra/local/invariants.yaml):
```yaml
environment: local-development
rust_version: "1.92.0"
rust_edition: "2024"

ports:
  flight-service: 8080
  metrics: 9090

env_vars:
  RUST_LOG:
    required: false
    default: "info"
    description: "Logging level for tracing"
  FLIGHT_CONFIG_PATH:
    required: true
    description: "Path to flight configuration directory"

resources:
  cpu_limit: "2"
  memory_limit: "2Gi"
```

**Key Semantics**:
- `ports` keys are logical service names (flight-service, metrics), not raw port numbers
- `env_vars` keys match actual environment variable names (RUST_LOG, FLIGHT_CONFIG_PATH)
- `rust_edition` is required for local environments to ensure consistency


### 5. xtask Framework (xtask/ crate)

**Purpose**: Unified automation entry point for all project commands.

**Structure**:
```
xtask/
├── Cargo.toml
└── src/
    ├── main.rs           # CLI entry point
    ├── check.rs          # Fast smoke tests
    ├── validate.rs       # Full quality gate
    ├── ac_status.rs      # Feature status generation
    ├── normalize_docs.rs # Doc front matter normalization
    ├── validate_infra.rs # IaC validation
    ├── schema.rs         # Schema validation utilities
    └── cross_ref.rs      # Cross-reference checking
```

**Command Interface**:
```rust
// main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fast local smoke test (fmt, clippy, core tests)
    Check,
    /// Full quality gate (check + benches, API, cross-ref)
    Validate,
    /// Generate feature status report
    AcStatus,
    /// Normalize documentation front matter
    NormalizeDocs,
    /// Validate infrastructure configurations
    ValidateInfra,
}
```

**Dependencies**:
- `clap` - CLI parsing
- `serde`, `serde_yaml`, `serde_json` - Data parsing
- `jsonschema` - Schema validation
- `walkdir` - File traversal
- `regex` - Pattern matching for tags
- `anyhow` - Error handling


## Data Models

### SpecLedger
```rust
#[derive(Debug, Deserialize, Serialize)]
struct SpecLedger {
    requirements: Vec<Requirement>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Requirement {
    id: String,              // REQ-1, INF-REQ-1
    name: String,
    status: RequirementStatus,
    ac: Vec<AcceptanceCriteria>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum RequirementStatus {
    Draft,
    Implemented,
    Tested,
    Deprecated,
}

#[derive(Debug, Deserialize, Serialize)]
struct AcceptanceCriteria {
    id: String,              // AC-1.1
    description: String,
    #[serde(default)]
    tests: Vec<TestReference>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum TestReference {
    Simple(String),          // "flight-core::tests::test_name"
    Detailed {
        #[serde(skip_serializing_if = "Option::is_none")]
        test: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        feature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
    },
}
```

### FrontMatter
```rust
#[derive(Debug, Deserialize, Serialize)]
struct FrontMatter {
    doc_id: String,          // DOC-CORE-AXIS
    kind: DocKind,
    area: Area,
    status: DocStatus,
    #[serde(default)]
    links: Links,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DocKind {
    Requirements,
    Design,
    Concept,
    HowTo,
    Reference,
    Adr,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Area {
    FlightCore,
    FlightVirtual,
    FlightHid,
    FlightIpc,
    FlightScheduler,
    FlightFfb,
    FlightPanels,
    Infra,
    Ci,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum DocStatus {
    Draft,
    Active,
    Deprecated,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Links {
    #[serde(default)]
    requirements: Vec<String>,
    #[serde(default)]
    tasks: Vec<String>,
    #[serde(default)]
    adrs: Vec<String>,
}
```


### GherkinScenario
```rust
#[derive(Debug)]
struct GherkinScenario {
    file_path: PathBuf,
    line_number: usize,
    name: String,
    tags: Vec<String>,      // @REQ-1, @AC-1.1
}

impl GherkinScenario {
    fn req_tags(&self) -> Vec<String> {
        self.tags.iter()
            .filter(|t| t.starts_with("@REQ-") || t.starts_with("@INF-REQ-"))
            .map(|t| t.trim_start_matches('@').to_string())
            .collect()
    }

    fn ac_tags(&self) -> Vec<String> {
        self.tags.iter()
            .filter(|t| t.starts_with("@AC-"))
            .map(|t| t.trim_start_matches('@').to_string())
            .collect()
    }
}
```

### Invariants
```rust
#[derive(Debug, Deserialize, Serialize)]
struct Invariants {
    environment: String,
    rust_version: String,
    #[serde(default)]
    rust_edition: Option<String>,
    #[serde(default)]
    ports: HashMap<String, u16>,
    #[serde(default)]
    env_vars: HashMap<String, EnvVarSpec>,
    #[serde(default)]
    resources: Option<Resources>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EnvVarSpec {
    required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    description: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Resources {
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_limit: Option<String>,
}
```


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Spec Ledger Completeness
*For any* requirement in specs/spec_ledger.yaml with status "tested", all of its acceptance criteria SHALL have at least one linked test reference.

**Validates: Requirements INF-REQ-2.6**

**Test Strategy**: Parse spec_ledger.yaml, filter requirements by status, verify each AC has non-empty tests array.

### Property 2: Front Matter Uniqueness
*For any* two documentation files in docs/, their doc_id fields SHALL be distinct.

**Validates: Requirements INF-REQ-1.4**

**Test Strategy**: Walk docs/ tree, extract all doc_id values, check for duplicates using HashSet.

### Property 3: Requirement Link Validity
*For any* documentation file with front matter containing requirement links, all referenced requirement IDs SHALL exist in specs/spec_ledger.yaml.

**Validates: Requirements INF-REQ-6.1**

**Test Strategy**: Parse all front matter, collect requirement IDs, verify each exists in spec ledger.

### Property 4: Gherkin Tag Validity
*For any* Gherkin scenario with @REQ-* or @AC-* tags, the referenced IDs SHALL exist in specs/spec_ledger.yaml.

**Validates: Requirements INF-REQ-6.3**

**Test Strategy**: Parse all .feature files, extract tags, cross-reference with spec ledger.

### Property 5: Test Reference Existence
*For any* test reference in specs/spec_ledger.yaml (excluding commands and features), the referenced test path SHALL exist in the codebase.

**Validates: Requirements INF-REQ-2.4**

**Test Strategy**: Parse spec ledger, extract test paths, verify using ripgrep or file system checks.

### Property 6: Schema Conformance
*For any* file validated against a schema, it SHALL conform to the schema or validation SHALL fail with line numbers.

**Validates: Requirements INF-REQ-12.4**

**Test Strategy**: Use jsonschema crate to validate YAML/JSON against schemas/*.json, capture validation errors.

### Property 7: Crate Documentation Coverage
*For any* crate name appearing in Cargo.toml workspace members, or any area referenced in specs/spec_ledger.yaml, at least one concept document SHALL exist in docs/concepts/ for that area.

**Validates: Requirements INF-REQ-1.6**

**Test Strategy**: Parse Cargo.toml workspace.members and specs/spec_ledger.yaml requirement areas, map crate names to areas, verify docs/concepts/<area>.md exists for each unique area.

### Property 8: Gherkin Scenario Coverage
*For any* requirement with status "implemented" or "tested", at least one Gherkin scenario SHALL be tagged with its requirement ID.

**Validates: Requirements INF-REQ-3.6**

**Test Strategy**: Parse spec ledger for implemented/tested requirements, parse Gherkin files for @REQ-* tags, verify coverage.


## Error Handling

### Error Categories

1. **Schema Validation Errors**
   - Invalid YAML/JSON syntax
   - Schema constraint violations
   - Missing required fields
   - **Handling**: Report file path, line number, field name, and expected format

2. **Cross-Reference Errors**
   - Broken requirement links
   - Missing test references
   - Invalid Gherkin tags
   - Orphaned documentation
   - **Handling**: Report source location, target ID, and suggestion for fix

3. **Infrastructure Validation Errors**
   - Failed dry-run commands
   - Invalid configuration syntax
   - Missing invariants
   - **Handling**: Report command output, exit code, and suggested fix

4. **File System Errors**
   - Missing expected files
   - Permission issues
   - **Handling**: Report path, operation attempted, and recovery steps

### Error Format

All xtask commands SHALL emit errors in this format:
```
[ERROR] <error_code>: <message>
  File: <path>:<line>:<column>
  Expected: <expected_value>
  Found: <actual_value>
  Suggestion: <fix_suggestion>
```

**Error Code Pattern**: `INF-<DOMAIN>-<NNN>` where:
- `DOMAIN` is one of: SCHEMA, XREF, INFRA, TEST
- `NNN` is a zero-padded 3-digit number

Example:
```
[ERROR] INF-SCHEMA-001: Missing required field 'status'
  File: specs/spec_ledger.yaml:15:3
  Expected: status field with value 'draft', 'implemented', 'tested', or 'deprecated'
  Found: (field missing)
  Suggestion: Add 'status: draft' to requirement REQ-3
```

### Exit Codes

- `0`: Success, all checks passed
- `1`: Validation failures (schema, cross-ref, tests)
- `2`: Infrastructure errors (missing files, permissions)
- `3`: Command execution errors (cargo, docker, kubectl)


## Validation Pipeline

### Execution Order

Per INF-REQ-12, schema validation SHALL occur before other checks. The complete validation pipeline order:

1. **Schema Validation**
   - Validate specs/spec_ledger.yaml against schemas/spec_ledger.schema.json
   - Validate all docs/**/*.md front matter against schemas/front_matter.schema.json
   - Validate all infra/**/invariants.yaml against schemas/invariants.schema.json

2. **Cross-Reference Checks**
   - Documentation → spec ledger (requirement links)
   - Spec ledger → codebase (test references)
   - Gherkin → spec ledger (tags)
   - ADRs → ADRs (cross-references)

3. **Code Quality Checks**
   - Formatting: `cargo fmt --all -- --check`
   - Linting: `cargo clippy` on core crates
   - Unit tests: `cargo test` on core crates
   - Benchmarks: `cargo bench` (full validation only)

4. **API Stability**
   - Public API verification: `cargo public-api`

5. **Report Generation**
   - Update docs/validation_report.md
   - Include timestamp, commit hash, per-check status

### Generated File Headers

All generated markdown reports SHALL include this header:

```markdown
<!--
  AUTO-GENERATED FILE: DO NOT EDIT BY HAND.
  Generated by: cargo xtask <command>
  Generated at: <timestamp>
  Git commit: <hash>
  Source of truth: <list of source files>
-->
```

Manual edits to generated files MAY be overwritten by future xtask runs.

## Testing Strategy

### Unit Tests

Unit tests verify individual components and parsing logic:

1. **Schema Parsing Tests**
   - Valid spec ledger parsing
   - Valid front matter parsing
   - Invalid input rejection
   - Edge cases (empty arrays, optional fields)

2. **Cross-Reference Logic Tests**
   - Requirement ID extraction from docs
   - Gherkin tag parsing
   - Test path validation logic
   - Duplicate detection

3. **File System Operations Tests**
   - Directory traversal
   - YAML/JSON parsing
   - Front matter extraction from markdown

### Integration Tests

Integration tests verify end-to-end workflows:

1. **cargo xtask check**
   - Run on minimal test fixture
   - Verify fmt, clippy, test execution
   - Check exit codes

2. **cargo xtask validate**
   - Run on complete test fixture
   - Verify all validation steps execute
   - Check report generation

3. **cargo xtask ac-status**
   - Run on fixture with spec ledger + Gherkin
   - Verify feature_status.md generation
   - Check markdown formatting

### Property-Based Tests

Property-based tests verify correctness properties using `proptest`:

1. **Property 2: Front Matter Uniqueness**
   - Generate random sets of doc files with doc_ids
   - Verify uniqueness check catches duplicates
   - **Validates: INF-REQ-1.4**

2. **Property 3: Requirement Link Validity**
   - Generate random doc files with requirement links
   - Generate random spec ledger
   - Verify cross-reference catches broken links
   - **Validates: INF-REQ-6.1**

3. **Property 6: Schema Conformance**
   - Generate random valid/invalid YAML against schema
   - Verify validation correctly accepts/rejects
   - **Validates: INF-REQ-12.4**

### Test Fixtures

Create test fixtures under `xtask/tests/fixtures/`:
```
fixtures/
├── minimal/              # Minimal valid setup
│   ├── specs/
│   │   └── spec_ledger.yaml
│   ├── docs/
│   │   └── concepts/
│   └── schemas/
├── complete/             # Full featured setup
│   ├── specs/
│   │   ├── spec_ledger.yaml
│   │   └── features/
│   ├── docs/
│   ├── infra/
│   └── schemas/
└── invalid/              # Various error cases
    ├── broken_links/
    ├── schema_errors/
    └── missing_files/
```


## Implementation Details

### Phase 1: Minimal Infrastructure Spine

#### 1.1 xtask Crate Setup
- Create `xtask/` directory with Cargo.toml
- Add dependencies: clap, serde, serde_yaml, serde_json, jsonschema, walkdir, regex, anyhow
- Implement `check` subcommand:
  - Run `cargo fmt --all -- --check`
  - Run `cargo clippy -p flight-core -p flight-virtual -p flight-hid -p flight-ipc -- -D warnings`
  - Run `cargo test -p flight-core -p flight-virtual -p flight-hid -p flight-ipc`
- Implement `validate` subcommand:
  - Run schema validation first (per INF-REQ-12)
  - Run `check` subcommand
  - Add public API verification: `cargo public-api`
  - Add cross-reference checks
  - Generate docs/validation_report.md

**Core Crates List**: flight-core, flight-virtual, flight-hid, flight-ipc (centralized in xtask/src/config.rs)

#### 1.2 Schema Definitions
- Create `schemas/` directory
- Define `spec_ledger.schema.json`
- Define `front_matter.schema.json`
- Implement schema validation in xtask using jsonschema crate

#### 1.3 Spec Ledger Creation
- Create `specs/spec_ledger.yaml` with minimal seed content:
  - One product requirement (REQ-1) with 2 ACs
  - One infrastructure requirement (INF-REQ-1) with 2 ACs
  - Status: draft for initial entries
- Validate against schemas/spec_ledger.schema.json
- Expand to full Flight Hub requirements (REQ-1 through REQ-14) in subsequent iterations
- Add remaining infrastructure requirements (INF-REQ-2 through INF-REQ-12)
- Link existing tests where applicable

#### 1.4 Documentation Structure
- Create `docs/` directory structure:
  - `docs/requirements/`
  - `docs/design/`
  - `docs/concepts/`
  - `docs/how-to/`
  - `docs/reference/`
  - `docs/adr/`
- Create 2-3 minimal seed documents with valid front matter:
  - `docs/concepts/flight-core.md` (area: flight-core, status: draft)
  - `docs/how-to/run-tests.md` (area: ci, status: draft)
  - `docs/concepts/flight-virtual.md` (area: flight-virtual, status: draft)
- Implement front matter parsing in xtask
- Validate front matter against schemas/front_matter.schema.json

#### 1.5 Local Development Environment
- Choose primary mechanism (Docker Compose recommended)
- Create `infra/local/` directory
- Write `infra/local/README.md` with setup instructions
- Create `infra/local/invariants.yaml`
- Create `infra/local/docker-compose.yml` (or devcontainer.json)

#### 1.6 CI Integration
- Create `infra/ci/README.md` documenting CI jobs and guarantees
- Update `.github/workflows/ci.yml` to use `cargo xtask validate`
- Configure artifact uploads for validation reports

**CI Job Template**:
```yaml
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.92.0
      - name: Run validation
        run: cargo xtask validate
      - name: Upload validation report
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: validation-report
          path: docs/validation_report.md
      - name: Upload feature status
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: feature-status
          path: docs/feature_status.md
```

### Phase 2: Refinements and Quality Overlays

#### 2.1 Cross-Reference Checking
- Implement `cross_ref.rs` module in xtask
- Add requirement link validation (docs → spec ledger)
- Add test reference validation (spec ledger → codebase)
- Add Gherkin tag validation (features → spec ledger)
- Integrate into `validate` subcommand

#### 2.2 Reporting and Status Generation
- Implement `ac_status.rs` for feature status generation
- Generate `docs/feature_status.md` with coverage table
- Update `validate` to generate `docs/validation_report.md`
- Include timestamps, commit hashes, and check results

#### 2.3 Documentation Normalization
- Implement `normalize_docs.rs` subcommand
- Add front matter normalization
- Generate `docs/README.md` index
- Verify doc_id uniqueness

#### 2.4 Infrastructure Validation
- Implement `validate_infra.rs` subcommand
- Add Docker Compose validation (`docker compose config`)
- Add Kubernetes validation (`kubectl apply --dry-run=client`)
- Parse and validate invariants.yaml files

#### 2.5 BDD Step Runner (Optional)
- Create `specs/` crate
- Add cucumber-rs dependency
- Implement step definitions for common patterns
- Wire Gherkin features to step implementations
- Add to CI pipeline


## Algorithms

### Cross-Reference Validation Algorithm

```rust
fn validate_cross_references(
    spec_ledger: &SpecLedger,
    docs: &[DocumentWithFrontMatter],
    features: &[GherkinScenario],
) -> Vec<CrossRefError> {
    let mut errors = Vec::new();
    
    // Build requirement ID index
    let req_ids: HashSet<String> = spec_ledger.requirements
        .iter()
        .map(|r| r.id.clone())
        .collect();
    
    // Build AC ID index
    let ac_ids: HashSet<String> = spec_ledger.requirements
        .iter()
        .flat_map(|r| r.ac.iter().map(|ac| ac.id.clone()))
        .collect();
    
    // Check doc → spec ledger links
    for doc in docs {
        for req_id in &doc.front_matter.links.requirements {
            if !req_ids.contains(req_id) {
                errors.push(CrossRefError::BrokenRequirementLink {
                    doc_path: doc.path.clone(),
                    req_id: req_id.clone(),
                });
            }
        }
    }
    
    // Check Gherkin → spec ledger links
    for scenario in features {
        for req_tag in scenario.req_tags() {
            if !req_ids.contains(&req_tag) {
                errors.push(CrossRefError::InvalidGherkinTag {
                    feature_path: scenario.file_path.clone(),
                    line: scenario.line_number,
                    tag: req_tag,
                });
            }
        }
        for ac_tag in scenario.ac_tags() {
            if !ac_ids.contains(&ac_tag) {
                errors.push(CrossRefError::InvalidGherkinTag {
                    feature_path: scenario.file_path.clone(),
                    line: scenario.line_number,
                    tag: ac_tag,
                });
            }
        }
    }
    
    // Check spec ledger → codebase test references
    for req in &spec_ledger.requirements {
        for ac in &req.ac {
            for test_ref in &ac.tests {
                if let TestReference::Simple(test_path) = test_ref {
                    if !test_exists(test_path) {
                        errors.push(CrossRefError::MissingTest {
                            req_id: req.id.clone(),
                            ac_id: ac.id.clone(),
                            test_path: test_path.clone(),
                        });
                    }
                }
            }
        }
    }
    
    errors
}

fn test_exists(test_path: &str) -> bool {
    // Format: "crate::module::tests::test_name"
    // Strategy 1: Use `cargo test -p <crate> -- --list` and parse output
    // Strategy 2: Use ripgrep to search for "fn test_name" in crates/<crate>
    
    // If test references a crate not in Cargo.toml workspace members,
    // record a warning instead of an error (may be external/feature-gated)
    
    let parts: Vec<&str> = test_path.split("::").collect();
    if parts.is_empty() {
        return false;
    }
    
    let crate_name = parts[0];
    let test_fn = parts.last().unwrap();
    
    // Check if crate exists in workspace
    if !is_workspace_member(crate_name) {
        eprintln!("[WARN] Test references non-workspace crate: {}", crate_name);
        return true; // Don't fail on external crates
    }
    
    // Use ripgrep to find test function
    let output = std::process::Command::new("rg")
        .args(&["-l", &format!("fn {}", test_fn), &format!("crates/{}", crate_name)])
        .output()
        .ok()?;
    
    !output.stdout.is_empty()
}
```

### Feature Status Generation Algorithm

```rust
fn generate_feature_status(
    spec_ledger: &SpecLedger,
    features: &[GherkinScenario],
) -> String {
    let mut output = String::new();
    output.push_str("# Feature Status Report\n\n");
    output.push_str(&format!("Generated: {}\n", chrono::Utc::now()));
    output.push_str(&format!("Commit: {}\n\n", get_git_commit()));
    
    output.push_str("| REQ ID | AC ID | Description | Gherkin | Tests | Status |\n");
    output.push_str("|--------|-------|-------------|---------|-------|--------|\n");
    
    for req in &spec_ledger.requirements {
        for ac in &req.ac {
            let gherkin_scenarios = features.iter()
                .filter(|s| s.ac_tags().contains(&ac.id))
                .map(|s| format!("{}:{}", s.file_path.display(), s.line_number))
                .collect::<Vec<_>>()
                .join("<br>");
            
            let test_count = ac.tests.len();
            let status = match (req.status, test_count > 0, !gherkin_scenarios.is_empty()) {
                (RequirementStatus::Tested, true, true) => "✅ Complete",
                (RequirementStatus::Implemented, true, _) => "🟡 Needs Gherkin",
                (RequirementStatus::Implemented, false, _) => "🟡 Needs Tests",
                (RequirementStatus::Draft, _, _) => "⚪ Draft",
                _ => "❌ Incomplete",
            };
            
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                req.id,
                ac.id,
                ac.description,
                if gherkin_scenarios.is_empty() { "-" } else { &gherkin_scenarios },
                test_count,
                status
            ));
        }
    }
    
    output
}
```


## Security Considerations

### Schema Injection
- All YAML/JSON parsing uses safe deserializers
- Schema validation prevents arbitrary code execution
- File paths are validated before access

### File System Access
- xtask operates only within workspace root
- No network access required
- Read-only operations for validation
- Write operations limited to docs/ for reports

### CI Integration
- xtask runs in isolated CI environment
- No secrets required for validation
- Artifact uploads use CI-provided credentials
- Exit codes prevent silent failures

## Performance Considerations

### Validation Performance Targets
- `cargo xtask check`: < 30 seconds on typical hardware
- `cargo xtask validate`: < 5 minutes including benches
- `cargo xtask ac-status`: < 5 seconds
- Cross-reference checks: < 10 seconds for 1000+ files

### Optimization Strategies
1. **Parallel Processing**: Use rayon for parallel file parsing
2. **Caching**: Cache parsed schemas and spec ledger
3. **Incremental Validation**: Only validate changed files (Phase 2)
4. **Lazy Loading**: Parse files on-demand for specific checks

### Memory Usage
- Target: < 100MB RSS for validation
- Strategy: Stream large files, avoid loading entire codebase into memory
- Use iterators over collections where possible

## Deployment and Rollout

### Phase 1 Rollout (Weeks 1-2)
1. Create xtask crate with check/validate commands
2. Define schemas and create spec_ledger.yaml
3. Add front matter to 3-5 seed docs
4. Set up local dev environment (Docker Compose)
5. Update CI to use xtask validate

### Phase 2 Rollout (Weeks 3-4)
1. Implement cross-reference checking
2. Add ac-status and normalize-docs commands
3. Generate initial reports
4. Add infrastructure validation
5. Document workflow for team

### Success Criteria
- CI green using xtask validate
- All Phase 1 requirements have entries in spec_ledger.yaml
- At least 5 docs have valid front matter
- Local dev environment documented and working
- Team can run cargo xtask check locally

