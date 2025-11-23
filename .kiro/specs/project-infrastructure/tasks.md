# Project Infrastructure Implementation Plan

This implementation plan converts the infrastructure design into executable tasks for Kiro. Tasks build incrementally, with Phase 1 establishing the minimal spine and Phase 2 adding refinements.

## Phase 1: Minimal Infrastructure Spine

- [ ] 1. Create xtask crate with basic structure
  - Create `xtask/` directory with Cargo.toml
  - Add dependencies: clap, serde, serde_yaml, serde_json, jsonschema, walkdir, regex, anyhow
  - Create `xtask/src/main.rs` with CLI entry point using clap
  - Create `xtask/src/config.rs` with core crates list: flight-core, flight-virtual, flight-hid, flight-ipc
  - Add xtask to workspace Cargo.toml
  - _Requirements: INF-REQ-10.1_

- [ ] 2. Implement cargo xtask check command
  - Create `xtask/src/check.rs` module
  - Implement formatting check: `cargo fmt --all -- --check`
  - Implement clippy for core crates: `cargo clippy -p flight-core -p flight-virtual -p flight-hid -p flight-ipc -- -D warnings`
  - Implement unit tests for core crates: `cargo test -p flight-core -p flight-virtual -p flight-hid -p flight-ipc`
  - Wire check command into main.rs CLI
  - Test: Run `cargo xtask check` and verify it executes all three steps
  - _Requirements: INF-REQ-10.1, INF-REQ-5.1_

- [ ] 3. Create JSON schemas for validation
  - Create `schemas/` directory
  - Create `schemas/spec_ledger.schema.json` with complete schema including additionalProperties: false
  - Create `schemas/front_matter.schema.json` with links as required field
  - Create `schemas/invariants.schema.json` for infrastructure configs
  - Add schema validation test fixtures in `xtask/tests/fixtures/schemas/`
  - _Requirements: INF-REQ-12.1, INF-REQ-12.2_

- [ ] 4. Implement schema validation module
  - Create `xtask/src/schema.rs` module
  - Implement function to validate YAML against JSON schema using jsonschema crate
  - Implement error formatting with INF-SCHEMA-NNN codes, file paths, and line numbers
  - Add unit tests for valid and invalid inputs
  - _Requirements: INF-REQ-12.4, INF-REQ-12.7_

- [ ] 5. Create minimal spec ledger
  - Create `specs/` directory
  - Create `specs/spec_ledger.yaml` with seed content:
    - One product requirement (REQ-1: Real-Time Axis Processing) with 2 ACs
    - One infrastructure requirement (INF-REQ-1: Structured Documentation) with 2 ACs
    - Both with status: draft
  - Validate against schemas/spec_ledger.schema.json
  - _Requirements: INF-REQ-2.1, INF-REQ-2.2_

- [ ] 6. Create documentation structure with seed docs
  - Create `docs/` directory structure: requirements/, design/, concepts/, how-to/, reference/, adr/
  - Create `docs/concepts/flight-core.md` with front matter (doc_id: DOC-CORE-OVERVIEW, kind: concept, area: flight-core, status: draft, links: {requirements: [REQ-1]})
  - Create `docs/how-to/run-tests.md` with front matter (doc_id: DOC-HOWTO-TESTS, kind: how-to, area: ci, status: draft, links: {requirements: []})
  - Create `docs/concepts/flight-virtual.md` with front matter (doc_id: DOC-VIRTUAL-OVERVIEW, kind: concept, area: flight-virtual, status: draft, links: {requirements: []})
  - _Requirements: INF-REQ-1.1, INF-REQ-1.2_

- [ ] 7. Implement front matter parsing
  - Create `xtask/src/front_matter.rs` module
  - Implement function to extract YAML front matter from markdown files
  - Implement function to parse front matter into FrontMatter struct
  - Implement function to walk docs/ and collect all front matter
  - Add unit tests with test fixtures
  - _Requirements: INF-REQ-1.4_

- [ ] 8. Implement cargo xtask validate command (Phase 1 version)
  - Create `xtask/src/validate.rs` module
  - Implement validation pipeline in order:
    1. Schema validation (spec_ledger.yaml, all docs front matter)
    2. Run cargo xtask check
    3. Run cargo public-api (if available)
  - Generate `docs/validation_report.md` with auto-generated header, timestamp, commit hash, and per-check results
  - Wire validate command into main.rs CLI
  - Test: Run `cargo xtask validate` and verify report generation
  - _Requirements: INF-REQ-5.1, INF-REQ-5.2, INF-REQ-10.2_

- [ ] 9. Create local development environment
  - Create `infra/local/` directory
  - Create `infra/local/README.md` documenting setup instructions
  - Create `infra/local/invariants.yaml` with rust_version: "1.89.0", rust_edition: "2024", ports, env_vars
  - Choose Docker Compose as primary mechanism
  - Create `infra/local/docker-compose.yml` with:
    - Rust 1.89.0 build environment
    - Source code bind mount
    - Port 8080 exposed for flight-service
  - Test: Run `docker compose -f infra/local/docker-compose.yml config` to validate
  - _Requirements: INF-REQ-8.1, INF-REQ-8.2, INF-REQ-8.3, INF-REQ-8.4_

- [ ] 10. Create CI configuration
  - Create `infra/ci/` directory
  - Create `infra/ci/README.md` documenting CI jobs, guarantees, and quality gates
  - Update `.github/workflows/ci.yml` to use `cargo xtask validate`
  - Add artifact uploads for docs/validation_report.md
  - Test: Verify CI workflow syntax with `gh workflow view` or manual inspection
  - _Requirements: INF-REQ-9.1, INF-REQ-9.2, INF-REQ-9.5, INF-REQ-9.7_

- [ ] 11. Checkpoint - Verify Phase 1 complete
  - Run `cargo xtask check` locally and verify it passes
  - Run `cargo xtask validate` locally and verify docs/validation_report.md is generated
  - Verify all seed docs have valid front matter
  - Verify specs/spec_ledger.yaml validates against schema
  - Verify local dev environment starts: `docker compose -f infra/local/docker-compose.yml up --build`
  - Ensure all tests pass, ask the user if questions arise


## Phase 2: Refinements and Quality Overlays

- [ ] 12. Implement cross-reference checking module
  - Create `xtask/src/cross_ref.rs` module
  - Implement function to build requirement ID index from spec_ledger.yaml
  - Implement function to validate doc front matter links against spec ledger
  - Implement function to validate test references exist in codebase (using ripgrep)
  - Implement workspace member checking to warn on external crate references
  - Add error formatting with INF-XREF-NNN codes
  - Add unit tests with test fixtures
  - _Requirements: INF-REQ-6.1, INF-REQ-6.2_

- [ ] 13. Implement Gherkin parsing and validation
  - Create `xtask/src/gherkin.rs` module
  - Implement function to parse .feature files and extract scenarios with tags
  - Implement function to extract @REQ-*, @INF-REQ-*, and @AC-* tags
  - Implement function to validate Gherkin tags against spec ledger
  - Add error formatting with INF-XREF-NNN codes
  - Add unit tests with test fixtures
  - _Requirements: INF-REQ-3.2, INF-REQ-3.3, INF-REQ-6.3_

- [ ] 14. Integrate cross-reference checks into validate
  - Update `xtask/src/validate.rs` to run cross-reference checks after schema validation
  - Add cross-reference results to docs/validation_report.md
  - Include orphaned docs, missing tests, and broken links sections
  - Test: Create intentionally broken links and verify they're caught
  - _Requirements: INF-REQ-6.5, INF-REQ-6.7_

- [ ] 15. Implement cargo xtask ac-status command
  - Create `xtask/src/ac_status.rs` module
  - Implement function to generate feature status table from spec ledger and Gherkin
  - Generate `docs/feature_status.md` with columns: REQ ID, AC ID, Description, Gherkin, Tests, Status
  - Add auto-generated header with timestamp and commit hash
  - Wire ac-status command into main.rs CLI
  - Test: Run `cargo xtask ac-status` and verify docs/feature_status.md generation
  - _Requirements: INF-REQ-10.3, INF-REQ-11.1_

- [ ] 16. Implement cargo xtask normalize-docs command
  - Create `xtask/src/normalize_docs.rs` module
  - Implement function to verify doc_id uniqueness across all docs
  - Implement function to generate docs/README.md index with tables grouped by band and area
  - Add auto-generated header to docs/README.md
  - Wire normalize-docs command into main.rs CLI
  - Test: Run `cargo xtask normalize-docs` and verify docs/README.md generation
  - _Requirements: INF-REQ-10.4, INF-REQ-11.3_

- [ ] 17. Implement cargo xtask validate-infra command
  - Create `xtask/src/validate_infra.rs` module
  - Implement validation for Docker Compose: `docker compose -f infra/local/docker-compose.yml config`
  - Implement validation for Kubernetes (if present): `kubectl apply --dry-run=client -f infra/k8s/`
  - Validate all infra/**/invariants.yaml files against schema
  - Add error formatting with INF-INFRA-NNN codes
  - Wire validate-infra command into main.rs CLI
  - Test: Run `cargo xtask validate-infra` and verify all checks pass
  - _Requirements: INF-REQ-10.5, INF-REQ-4.5, INF-REQ-4.6_

- [ ] 18. Expand spec ledger with full requirements
  - Add all Flight Hub requirements (REQ-2 through REQ-14) to specs/spec_ledger.yaml
  - Add all infrastructure requirements (INF-REQ-2 through INF-REQ-12)
  - Link existing tests where applicable
  - Update statuses based on implementation state
  - Validate against schema
  - _Requirements: INF-REQ-2.1, INF-REQ-2.3_

- [ ] 19. Create Gherkin features for key requirements
  - Create `specs/features/` directory
  - Create `specs/features/req_1_axis_processing.feature` with scenarios for REQ-1
  - Create `specs/features/inf_req_1_documentation.feature` with scenarios for INF-REQ-1
  - Tag scenarios with @REQ-1, @INF-REQ-1, and appropriate @AC-* tags
  - Validate tags against spec ledger using cargo xtask validate
  - _Requirements: INF-REQ-3.1, INF-REQ-3.2_

- [ ] 20. Add comprehensive documentation with front matter
  - Add front matter to existing docs in docs/ directory
  - Create additional concept docs for major crates (flight-hid, flight-ipc, flight-scheduler)
  - Create how-to guides (setup-dev-env, run-benchmarks, add-new-requirement)
  - Ensure all docs link to appropriate requirements
  - Run `cargo xtask normalize-docs` to generate index
  - _Requirements: INF-REQ-1.2, INF-REQ-1.3, INF-REQ-1.6_

- [ ] 21. Final checkpoint - Verify Phase 2 complete
  - Run `cargo xtask validate` and verify all checks pass
  - Run `cargo xtask ac-status` and verify feature coverage report
  - Run `cargo xtask normalize-docs` and verify docs index
  - Run `cargo xtask validate-infra` and verify infrastructure validation
  - Verify docs/validation_report.md includes cross-reference results
  - Verify docs/feature_status.md shows Gherkin coverage
  - Ensure all tests pass, ask the user if questions arise

## Optional: BDD Step Runner

- [ ]* 22. Create specs crate for BDD execution
  - Create `specs/` crate in workspace
  - Add cucumber-rs dependency
  - Implement step definitions for common patterns (axis processing, documentation checks)
  - Wire Gherkin features to step implementations
  - Add to CI pipeline: `cargo test -p specs`
  - _Requirements: INF-REQ-3.5_

## Notes

- Tasks marked with `*` are optional and can be skipped
- Each task should be completed before moving to the next
- Run acceptance commands after each task to verify completion
- Phase 1 provides immediate value; Phase 2 adds comprehensive validation
- Kiro should stop after each task for user review
