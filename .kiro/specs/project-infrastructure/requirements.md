# Project Infrastructure Requirements Document

## Introduction

The Project Infrastructure system provides a structured, machine-readable framework for maintaining documentation, behavior-driven development (BDD) specifications, infrastructure-as-code (IaC), and automated validation for the Flight Hub project. This system enables Kiro (the AI coding assistant) to effectively maintain project artifacts by defining clear contracts, schemas, and validation commands. The goal is to establish a "Kiro-native" workflow where humans define shapes and contracts, while Kiro fills them and keeps them green through automated tasks.

All requirements in this document are prefixed **INF-REQ-** to distinguish them from product requirements (REQ-*) in the Flight Hub specification.

## System Overview (Normative)

The following canonical locations and entry points define the infrastructure system:

**File Locations:**
- Spec ledger: `specs/spec_ledger.yaml`
- Gherkin features: `specs/features/*.feature`
- Documentation: `docs/**/*.md` with YAML front matter
- Infrastructure configs: `infra/**`
- Schemas: `schemas/*.json`

**Automation Entry Points:**
- `cargo xtask check` - Fast local smoke test (fmt, clippy, core tests)
- `cargo xtask validate` - Full quality gate (check + benches, public API, cross-ref)
- `cargo xtask ac-status` - Generate feature_status.md from spec ledger and Gherkin
- `cargo xtask normalize-docs` - Normalize front matter and rebuild doc indexes
- `cargo xtask validate-infra` - Run all IaC dry-run checks

**Invariants:**
- All validation MUST be invocable via `cargo xtask` subcommands
- CI MUST use `cargo xtask` rather than custom `cargo` command sequences
- All machine-readable metadata MUST be parseable YAML or JSON

## Glossary

- **Kiro**: The AI coding assistant that executes tasks, runs commands, and applies edits
- **Spec Ledger**: A machine-readable YAML file mapping requirements to acceptance criteria and tests
- **Doc Band**: A category of documentation (requirements, design, concepts, how-to, reference, adr)
- **Front Matter**: YAML metadata at the top of markdown files for machine processing
- **xtask**: A Rust convention for project-specific automation commands (cargo xtask <command>)
- **AC**: Acceptance Criteria - testable conditions that validate a requirement
- **Gherkin**: A BDD syntax using Given/When/Then for executable specifications
- **IaC**: Infrastructure as Code - declarative configuration files for environments
- **INF-REQ**: Infrastructure Requirement - requirements specific to project infrastructure (this document)

## Requirements

### Requirement 1: Structured Documentation System (INF-REQ-1)

**User Story:** As a developer or maintainer, I want a well-organized documentation system with machine-readable metadata, so that documentation can be automatically validated, cross-referenced, and kept in sync with code.

**Enumerations:**
- The `kind` field SHALL be one of: `requirements`, `design`, `concept`, `how-to`, `reference`, `adr`
- The `area` field SHALL be one of: `flight-core`, `flight-virtual`, `flight-hid`, `flight-ipc`, `flight-scheduler`, `flight-ffb`, `flight-panels`, `infra`, `ci`
- The `status` field SHALL be one of: `draft`, `active`, `deprecated`

#### Acceptance Criteria

1. WHEN documentation is created THEN it SHALL be organized into bands: requirements, design, concepts, how-to, reference, and adr under `docs/`
2. WHEN a documentation file is created THEN it SHALL include YAML front matter with doc_id, kind, area, status, and links fields
3. WHEN documentation references requirements THEN it SHALL use stable requirement IDs (e.g., REQ-1, INF-REQ-1, AC-1.1)
4. WHEN validating documentation THEN the system SHALL verify all doc_id fields are unique and all requirement links are valid
5. WHEN generating documentation indexes THEN the system SHALL produce markdown tables grouped by area and kind
6. WHERE a crate or feature area is referenced in specs/spec_ledger.yaml or Cargo.toml THEN at least one concept document for that area SHALL exist in docs/concepts/
7. WHEN documentation status changes THEN the front matter status field SHALL be updated to reflect draft, active, or deprecated state

### Requirement 2: Spec Ledger and Traceability (INF-REQ-2)

**User Story:** As a quality engineer, I want a machine-readable ledger that maps requirements to acceptance criteria and tests, so that I can verify test coverage and trace features to their specifications.

**Requirement Status Values:**
- `draft` - Requirement defined but not yet implemented
- `implemented` - Code written but tests may be incomplete
- `tested` - All acceptance criteria have linked passing tests
- `deprecated` - No longer applicable

#### Acceptance Criteria

1. WHEN requirements are defined THEN they SHALL be recorded in specs/spec_ledger.yaml with unique IDs (REQ-* or INF-REQ-*)
2. WHEN acceptance criteria are defined THEN they SHALL be nested under requirements with unique AC IDs (e.g., AC-1.1, AC-1.2)
3. WHEN tests are written THEN they SHALL be linked to AC IDs in the spec ledger using test module paths or feature file references
4. WHEN validating the ledger THEN the system SHALL verify all referenced test paths exist in the codebase
5. WHEN generating coverage reports THEN the system SHALL produce a table showing REQ, AC, test status, and Gherkin feature links
6. WHEN a requirement in specs/spec_ledger.yaml has status: tested THEN at least one test SHALL be linked to each of its acceptance criteria
7. WHEN the spec ledger is updated THEN the system SHALL validate YAML syntax and schema compliance against schemas/spec_ledger.schema.json

### Requirement 3: Behavior-Driven Development Integration (INF-REQ-3)

**User Story:** As a product owner or tester, I want executable Gherkin specifications that link to requirements, so that acceptance criteria can be validated through automated tests.

#### Acceptance Criteria

1. WHEN Gherkin features are created THEN they SHALL be stored in specs/features/ with descriptive filenames matching pattern `req_<N>_<description>.feature`
2. WHEN Gherkin scenarios are written THEN they SHALL be tagged with @REQ-* and @AC-* tags matching the spec ledger
3. WHEN validating BDD coverage THEN the system SHALL cross-reference Gherkin tags with spec ledger entries
4. WHEN generating feature status reports THEN the system SHALL list which REQs have Gherkin scenarios and which are missing
5. WHEN BDD execution is enabled THEN Gherkin features SHALL map to step definitions in a specs crate
6. WHEN a requirement in the spec ledger has status: implemented or tested THEN at least one Gherkin scenario tagged with its REQ ID SHALL exist in specs/features/
7. WHEN Gherkin scenarios fail THEN the failure SHALL be traceable back to the specific AC ID via tags

### Requirement 4: Infrastructure as Code Management (INF-REQ-4)

**User Story:** As a DevOps engineer, I want declarative infrastructure definitions with clear contracts, so that environments can be reproduced consistently and Kiro can maintain configuration files.

#### Acceptance Criteria

1. WHEN defining local development environments THEN configuration SHALL be stored in infra/local/ with README.md and invariants.yaml defining contracts
2. WHEN defining CI environments THEN configuration SHALL be stored in infra/ci/ with README.md documenting job guarantees
3. WHEN defining deployment environments THEN configuration SHALL be stored in infra/k8s/ or infra/terraform/ with resource specifications
4. WHEN infrastructure contracts are defined THEN invariants.yaml SHALL specify required ports, environment variables, resource limits, and dependencies
5. WHEN validating infrastructure THEN the system SHALL run dry-run or validation commands (e.g., kubectl apply --dry-run=client, docker compose config)
6. WHEN infrastructure changes THEN validation SHALL check configuration against the invariants defined in infra/**/invariants.yaml
7. WHEN creating new infrastructure components THEN they SHALL include inline comments explaining resource choices and configuration decisions

### Requirement 5: Automated Validation Framework (INF-REQ-5)

**User Story:** As a CI maintainer, I want a unified validation command that checks all project health indicators, so that quality gates are consistent and easy to run.

**Validation Scope:**
Formatting checks, linting, unit tests, public API verification, cross-reference checks, and schema validation SHALL be part of validation.

#### Acceptance Criteria

1. WHEN running validation THEN it SHALL be invoked via cargo xtask validate
2. WHEN validation completes THEN it SHALL update docs/validation_report.md with timestamp, git commit hash, per-check status (pass/fail), and summary count of failures
3. WHEN validation fails THEN it SHALL provide actionable error messages with suggested fixes
4. WHEN adding new quality gates THEN they SHALL be integrated into the xtask validate command
5. WHEN validation is run in CI THEN it SHALL use the same xtask commands as local development
6. WHEN validation detects issues THEN it SHALL exit with non-zero status and log structured error information
7. WHEN validation runs THEN it SHALL complete without exceeding CI job timeout limits

### Requirement 6: Cross-Reference and Consistency Checking (INF-REQ-6)

**User Story:** As a technical writer, I want automated checks that verify documentation links and requirement references are valid, so that documentation stays accurate as the codebase evolves.

#### Acceptance Criteria

1. WHEN checking documentation THEN the system SHALL verify all requirement IDs referenced in docs exist in the spec ledger
2. WHEN checking the spec ledger THEN the system SHALL verify all test paths and names exist in the codebase
3. WHEN checking Gherkin features THEN the system SHALL verify all @REQ-* and @AC-* tags match spec ledger entries
4. WHEN checking ADRs THEN the system SHALL verify all cross-references to other ADRs are valid
5. WHEN generating cross-reference reports THEN the system SHALL list orphaned docs, missing tests, and broken links in docs/validation_report.md
6. WHEN a requirement is removed THEN the system SHALL identify all documentation and tests that reference it
7. WHEN running cross-reference checks THEN they SHALL be included in cargo xtask validate without exceeding CI job timeouts

### Requirement 7: Task-Driven Maintenance Workflow (INF-REQ-7)

**User Story:** As a project maintainer, I want well-defined tasks that Kiro can execute to maintain project artifacts, so that routine maintenance is automated and consistent.

#### Acceptance Criteria

1. WHEN defining maintenance tasks THEN they SHALL include title, motivation, step bullets, and acceptance commands
2. WHEN tasks reference files THEN they SHALL use relative paths from the workspace root
3. WHEN tasks include validation THEN they SHALL specify exact commands Kiro can run to verify completion
4. WHEN tasks are executed THEN Kiro SHALL follow the steps sequentially and report results
5. WHEN tasks fail THEN Kiro SHALL provide diagnostic information and suggest corrective actions
6. WHEN new maintenance needs arise THEN they SHALL be encoded as new tasks in the appropriate tasks.md file
7. WHEN tasks are completed THEN their acceptance commands SHALL be re-run and logged in the task summary so results are independently reproducible

### Requirement 8: Local Development Environment (INF-REQ-8)

**User Story:** As a new contributor, I want a reproducible local development environment, so that I can start contributing quickly without environment-specific issues.

#### Acceptance Criteria

1. WHEN setting up locally THEN the canonical local environment SHALL be defined using one of: Docker Compose, devcontainer, or Nix; additional options MAY be provided but only one SHALL be documented as primary in infra/local/README.md
2. WHEN the local environment starts THEN it SHALL build with Rust 1.89.0 and edition 2024
3. WHEN services are exposed THEN they SHALL use documented standard ports defined in infra/local/invariants.yaml
4. WHEN the local environment is started THEN application source code SHALL be bind-mounted into containers to allow code changes without rebuilding base images
5. WHEN validating the environment THEN health check endpoints SHALL return 200 status
6. WHEN environment variables are required THEN they SHALL be documented in infra/local/README.md and infra/local/invariants.yaml
7. WHEN the environment is updated THEN changes SHALL be reflected in both configuration files and documentation

### Requirement 9: Continuous Integration Configuration (INF-REQ-9)

**User Story:** As a CI engineer, I want clear documentation of CI job responsibilities and guarantees, so that the CI pipeline is maintainable and its behavior is predictable.

#### Acceptance Criteria

1. WHEN CI jobs are defined THEN they SHALL be documented in infra/ci/README.md with purpose and guarantees
2. WHEN CI runs validation THEN it SHALL use the same cargo xtask commands as local development
3. WHEN CI detects failures THEN xtask commands SHALL emit errors prefixed with [ERROR] and include a machine-parseable code, file path, and short description
4. WHEN CI configuration changes THEN the documentation SHALL be updated to reflect new behavior
5. WHEN adding new CI checks THEN they SHALL be added to the xtask framework first, then referenced in CI
6. WHEN CI runs THEN it SHALL use cargo xtask validate to enforce all quality gates
7. WHEN CI completes THEN it SHALL upload docs/validation_report.md and docs/feature_status.md (if changed) as artifacts for review

### Requirement 10: xtask Automation Framework (INF-REQ-10)

**User Story:** As a developer, I want a single entry point for all project automation commands, so that I don't need to remember multiple cargo commands with different flags.

**Subcommand Definitions:**
- `check`: Fast local smoke test - fmt, clippy for core crates, unit tests
- `validate`: Full quality gate - check + IPC benches, public API, cross-ref, schema validation
- `ac-status`: Generate docs/feature_status.md from spec ledger and Gherkin tags
- `normalize-docs`: Normalize front matter, verify doc_id uniqueness, rebuild docs indexes
- `validate-infra`: Run all IaC dry-run checks (docker compose config, kubectl --dry-run, etc.)

#### Acceptance Criteria

1. WHEN running cargo xtask check THEN it SHALL execute formatting checks, clippy for core crates, and unit tests
2. WHEN running cargo xtask validate THEN it SHALL execute check plus IPC benches, public API verification, cross-reference checks, and schema validation
3. WHEN running cargo xtask ac-status THEN it SHALL generate docs/feature_status.md from specs/spec_ledger.yaml and specs/features/*.feature
4. WHEN running cargo xtask normalize-docs THEN it SHALL update front matter in docs/**/*.md and verify doc_id uniqueness
5. WHEN running cargo xtask validate-infra THEN it SHALL run dry-run validation on all files in infra/
6. WHEN xtask commands fail THEN they SHALL provide clear error messages and exit with non-zero status
7. WHEN adding new automation THEN it SHALL be implemented as an xtask subcommand with --help documentation

### Requirement 11: Documentation Generation and Reporting (INF-REQ-11)

**User Story:** As a project manager, I want automatically generated reports showing test coverage, feature status, and documentation health, so that I can track project quality metrics.

**Report Outputs:**
- `docs/feature_status.md` - Generated by cargo xtask ac-status
- `docs/validation_report.md` - Updated by cargo xtask validate
- `docs/README.md` - Index generated by cargo xtask normalize-docs

#### Acceptance Criteria

1. WHEN cargo xtask ac-status runs THEN it SHALL generate docs/feature_status.md with columns: REQ ID, AC ID, Gherkin scenario, linked tests, status
2. WHEN cargo xtask validate runs THEN it SHALL update docs/validation_report.md with pass/fail status for all checks, including cross-reference results
3. WHEN cargo xtask normalize-docs runs THEN it SHALL generate docs/README.md with tables of all documentation grouped by band and area
4. WHEN reports are generated THEN they SHALL include timestamps and git commit hashes for traceability
5. WHEN reports are updated THEN they SHALL be formatted as valid markdown with proper tables and links
6. WHEN viewing reports THEN they SHALL be human-readable and suitable for inclusion in project reviews
7. WHEN cross-reference issues are found THEN they SHALL be listed in a dedicated section of docs/validation_report.md

### Requirement 12: Schema Validation and Enforcement (INF-REQ-12)

**User Story:** As a quality engineer, I want schemas that define the structure of spec ledgers, front matter, and configuration files, so that validation can catch errors early.

**Schema Locations:**
- Spec ledger schema: `schemas/spec_ledger.schema.json`
- Front matter schema: `schemas/front_matter.schema.json`
- Infrastructure invariants schema: `schemas/invariants.schema.json`

#### Acceptance Criteria

1. WHEN validating spec ledgers THEN the system SHALL enforce schemas/spec_ledger.schema.json with required fields (id, name, status, ac, tests)
2. WHEN validating front matter THEN the system SHALL enforce schemas/front_matter.schema.json with required fields (doc_id, kind, area, status, links)
3. WHEN validating Gherkin tags THEN the system SHALL verify they match patterns @REQ-[0-9]+ or @INF-REQ-[0-9]+ and @AC-[0-9]+\.[0-9]+
4. WHEN schema validation fails THEN the system SHALL report file paths, line numbers, and specific violations
5. WHEN schemas are updated THEN the system SHALL provide migration guidance in schemas/MIGRATIONS.md
6. WHEN adding new metadata fields THEN they SHALL be documented in the relevant schema file and README
7. WHEN running cargo xtask validate THEN schema checks SHALL be performed before any other validation steps


## Implementation Phases

To enable rapid adoption and iterative refinement, requirements are organized into two phases:

### Phase 1: Minimal Infrastructure Spine

Phase 1 establishes the foundational structure that enables Kiro to begin maintaining project artifacts:

- **INF-REQ-1**: Structured Documentation System (bands + front matter)
- **INF-REQ-2**: Spec Ledger and Traceability (specs/spec_ledger.yaml)
- **INF-REQ-3**: BDD Integration (Gherkin tags + files, without step runner)
- **INF-REQ-4**: IaC Management (layout + invariants.yaml)
- **INF-REQ-5**: Automated Validation Framework (cargo xtask validate)
- **INF-REQ-8**: Local Development Environment
- **INF-REQ-9**: CI Configuration (uses xtask)
- **INF-REQ-10**: xtask Automation Framework (check, validate subcommands)

**Phase 1 Deliverables:**
- Working `cargo xtask check` and `cargo xtask validate` commands
- Basic `specs/spec_ledger.yaml` with product requirements mapped
- Documentation structure with front matter on seed docs
- Local development environment (Docker Compose or devcontainer)
- CI pipeline using xtask commands

### Phase 2: Refinements and Quality Overlays

Phase 2 adds sophisticated cross-checking, reporting, and schema enforcement:

- **INF-REQ-6**: Cross-Reference and Consistency Checking
- **INF-REQ-7**: Task-Driven Maintenance Workflow (formalized encoding)
- **INF-REQ-10**: Full xtask suite (ac-status, normalize-docs, validate-infra)
- **INF-REQ-11**: Documentation Generation and Reporting
- **INF-REQ-12**: Schema Validation and Enforcement (formal JSON schemas)

**Phase 2 Deliverables:**
- Automated cross-reference validation
- Generated reports (feature_status.md, validation_report.md)
- JSON schemas for all metadata formats
- Full xtask command suite
- BDD step runner (specs crate with executable Gherkin)

**Rationale:** Phase 1 provides immediate value by establishing structure and automation. Phase 2 adds polish and comprehensive validation without blocking initial progress.
