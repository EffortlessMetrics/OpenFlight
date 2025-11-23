# Project Infrastructure Implementation Plan

This implementation plan converts the infrastructure design into executable tasks for Kiro. Tasks build incrementally, with Phase 1 establishing the minimal spine and Phase 2 adding refinements.

## Error Code Families

All xtask commands use structured error codes:
- `INF-SCHEMA-xxx`: Schema validation errors
- `INF-XREF-xxx`: Cross-reference errors (docs ↔ ledger ↔ tests ↔ Gherkin)
- `INF-INFRA-xxx`: Infrastructure validation errors (Docker, Kubernetes, invariants)
- `INF-VALID-xxx`: General validation pipeline errors (tests failing, clippy warnings)

## Validation Pipeline Order

Per INF-REQ-12, `cargo xtask validate` MUST perform checks in this order:
1. Schema validation (spec_ledger, front matter, invariants)
2. Cross-reference validation (docs ↔ ledger ↔ tests ↔ Gherkin)
3. Code quality (formatting, clippy, tests, benches)
4. Public API checks
5. Report generation (validation_report.md)

## Phase 1: Minimal Infrastructure Spine

- [x] 1. Create xtask crate with basic structure





  - Create `xtask/` directory with Cargo.toml
  - Add dependencies: clap, serde, serde_yaml, serde_json, jsonschema, walkdir, regex, anyhow
  - Create `xtask/src/main.rs` with CLI entry point using clap
  - Create `xtask/src/config.rs` with CORE_CRATES constant: `["flight-core", "flight-virtual", "flight-hid", "flight-ipc"]`
  - Document in config.rs: "This is the single source of truth for which crates are included in fast checks. It does not need to include every workspace member."
  - Add xtask to workspace Cargo.toml
  - All xtask commands MUST run from workspace root (or chdir to workspace root on startup)
  - Acceptance: `cargo xtask --help` displays available commands
  - _Requirements: INF-REQ-10.1_

- [x] 2. Implement cargo xtask check command




  - Create `xtask/src/check.rs` module with `pub fn run_check() -> anyhow::Result<()>` function
  - Implement formatting check: `cargo fmt --all -- --check`
  - Implement clippy for core crates: loop over config::CORE_CRATES and run `cargo clippy -p <crate> -- -D warnings`
  - Implement unit tests for core crates: loop over config::CORE_CRATES and run `cargo test -p <crate>`
  - Wire check command into main.rs CLI (calls run_check())
  - Acceptance: `cargo xtask check` executes all three steps and exits 0 if all pass, 1 if any fail
  - _Requirements: INF-REQ-10.1, INF-REQ-5.1_

- [x] 3. Create JSON schemas for validation





  - Create `schemas/` directory
  - Create `schemas/spec_ledger.schema.json` with:
    - `additionalProperties: false` on all objects
    - Test reference object with `minProperties: 1` and `additionalProperties: false`
    - Pattern validation for REQ/INF-REQ/AC IDs
  - Create `schemas/front_matter.schema.json` with:
    - `links` as required field
    - `additionalProperties: false` on root and links objects
  - Create `schemas/invariants.schema.json` for infrastructure configs
  - Create test fixtures in `xtask/tests/fixtures/`:
    - `minimal/` - valid minimal setup
    - `invalid/schema_errors/` - various schema violations
  - Acceptance: All three schema files exist and are valid JSON Schema Draft 7
  - _Requirements: INF-REQ-12.1, INF-REQ-12.2_

- [x] 4. Implement schema validation module





  - Create `xtask/src/schema.rs` module
  - Implement `validate_yaml_against_schema(yaml_path: &Path, schema_path: &Path) -> Result<(), Vec<SchemaError>>`
  - Load YAML into serde_yaml::Value, convert to JSON for jsonschema validation
  - Keep original YAML text for line number reporting
  - Implement error formatting: `[ERROR] INF-SCHEMA-NNN: <message>\n  File: <path>:<line>:<column>\n  Expected: ...\n  Found: ...\n  Suggestion: ...`
  - Add unit tests using fixtures from Task 3 (valid and invalid inputs)
  - Acceptance: Tests pass for both valid schemas and intentional violations with correct error codes
  - _Requirements: INF-REQ-12.4, INF-REQ-12.7_

-

- [x] 5. Create minimal spec ledger


  - Create `specs/` directory
  - Create `specs/spec_ledger.yaml` with seed content:
    - One product requirement (REQ-1: Real-Time Axis Processing) with 2 ACs, status: draft
    - One infrastructure requirement (INF-REQ-1: Structured Documentation) with 2 ACs, status: draft
    - Use `status: draft` deliberately to avoid coverage-enforcement properties (Property 1/8) during bootstrap
  - Validate against schemas/spec_ledger.schema.json using schema validation module
  - Acceptance: `cargo run --manifest-path xtask/Cargo.toml -- validate-schema specs/spec_ledger.yaml schemas/spec_ledger.schema.json` exits 0
  - _Requirements: INF-REQ-2.1, INF-REQ-2.2_

- [x] 6. Create documentation structure with seed docs





  - Create `docs/` directory structure: requirements/, design/, concepts/, how-to/, reference/, adr/
  - Create `docs/requirements/overview.md` with front matter (doc_id: DOC-REQ-OVERVIEW, kind: requirements, area: infra, status: draft, links: {requirements: [], tasks: [], adrs: []})
  - Create `docs/concepts/flight-core.md` with front matter (doc_id: DOC-CORE-OVERVIEW, kind: concept, area: flight-core, status: draft, links: {requirements: [REQ-1], tasks: [], adrs: []})
  - Create `docs/how-to/run-tests.md` with front matter (doc_id: DOC-HOWTO-TESTS, kind: how-to, area: ci, status: draft, links: {requirements: [], tasks: [], adrs: []})
  - Create `docs/concepts/flight-virtual.md` with front matter (doc_id: DOC-VIRTUAL-OVERVIEW, kind: concept, area: flight-virtual, status: draft, links: {requirements: [], tasks: [], adrs: []})
  - Front matter delimiter: `---` at top of file, ending at next `---` line (no multi-document YAML)
  - Acceptance: All four docs exist with valid YAML front matter between `---` delimiters
  - _Requirements: INF-REQ-1.1, INF-REQ-1.2_

- [x] 7. Implement front matter parsing





  - Create `xtask/src/front_matter.rs` module
  - Implement `extract_front_matter(content: &str) -> Option<&str>` - extracts text between first `---` and second `---`
  - Implement `parse_front_matter(yaml: &str) -> Result<FrontMatter>` - parses YAML into FrontMatter struct
  - Implement `collect_all_front_matter(docs_dir: &Path) -> Result<Vec<(PathBuf, FrontMatter)>>` - walks docs/ recursively
  - Add unit tests with fixtures:
    - Valid front matter
    - Missing front matter (returns None)
    - Malformed YAML (returns Err)
    - Files with no front matter (skipped or flagged as warning)
  - Acceptance: Tests pass for all edge cases
  - _Requirements: INF-REQ-1.4_


- [x] 8. Implement cargo xtask validate command (Phase 1 version)


  - Create `xtask/src/validate.rs` module with `pub fn run_validate() -> anyhow::Result<()>`
  - Implement validation pipeline in order:
    1. Schema validation: validate specs/spec_ledger.yaml and all docs/**/*.md front matter
    2. Code quality: call check::run_check() (do NOT spawn `cargo xtask check` subprocess)
    3. Public API: run `cargo public-api` if installed; if not, log warning and skip (do not fail)
  - Generate `docs/validation_report.md` with:
    - Auto-generated header: `<!-- AUTO-GENERATED FILE: DO NOT EDIT BY HAND. Generated by: cargo xtask validate ... -->`
    - Timestamp and git commit hash
    - Table of checks (schema, fmt, clippy, tests, API) with pass/fail status
    - Summary count of failures
  - Wire validate command into main.rs CLI (calls run_validate())
  - Acceptance: `cargo xtask validate` generates docs/validation_report.md with all sections present
  - _Requirements: INF-REQ-5.1, INF-REQ-5.2, INF-REQ-10.2_

- [x] 9. Create local development environment





  - Create `infra/local/` directory
  - Create `infra/local/README.md` documenting:
    - Setup instructions
    - How to run: `docker compose up`
    - Health check: `curl -f http://localhost:8080/health` (should return 200)
  - Create `infra/local/invariants.yaml` with:
    - environment: "local-development"
    - rust_version: "1.89.0"
    - rust_edition: "2024" (required field)
    - ports: {flight-service: 8080} (logical service names, not raw numbers)
    - env_vars: {RUST_LOG: {required: false, default: "info", description: "..."}}
  - Create `infra/local/docker-compose.yml` with:
    - Service: flight-service using rust:1.89.0
    - Bind mount: `.:/workspace` for rapid iteration
    - Port mapping: 8080:8080
    - Health check endpoint (stub if needed)
  - Acceptance: `docker compose -f infra/local/docker-compose.yml config` exits 0
  - _Requirements: INF-REQ-8.1, INF-REQ-8.2, INF-REQ-8.3, INF-REQ-8.4, INF-REQ-8.5_

- [x] 10. Create CI configuration










  - Create `infra/ci/` directory
  - Create `infra/ci/README.md` documenting:
    - CI jobs and their purpose
    - Quality gates enforced (fmt, clippy, tests, API stability)
    - Guarantees (all checks must pass for merge)
  - Update `.github/workflows/ci.yml` with validate job:
    ```yaml
    jobs:
      validate:
        runs-on: ubuntu-latest
        steps:
          - uses: actions/checkout@v4
          - uses: dtolnay/rust-toolchain@1.89.0
          - run: cargo xtask validate
          - uses: actions/upload-artifact@v4
            if: always()
            with:
              name: validation-report
              path: docs/validation_report.md
    ```
  - Acceptance: Workflow file is valid YAML and references `cargo xtask validate`
  - _Requirements: INF-REQ-9.1, INF-REQ-9.2, INF-REQ-9.5, INF-REQ-9.6, INF-REQ-9.7_
- [x] 11. Checkpoint - Verify Phase 1 complete









- [ ] 11. Checkpoint - Verify Phase 1 complete

  - Run `cargo xtask check` and verify exit code 0
  - Run `cargo xtask validate` and verify exit code 0
  - Verify `docs/validation_report.md` exists and contains auto-generated header, timestamp, commit hash, and check results
  - Verify all seed docs (4 files) have valid front matter that validates against schema
  - Verify `specs/spec_ledger.yaml` validates against `schemas/spec_ledger.schema.json`
  - Run `docker compose -f infra/local/docker-compose.yml config` and verify exit code 0
  - Run `docker compose -f infra/local/docker-compose.yml up --build -d` and verify service starts
  - Run `curl -f http://localhost:8080/health` and verify 200 response (or document if stubbed)
  - Run `docker compose -f infra/local/docker-compose.yml down` to clean up


## Phase 2: Refinements and Quality Overlays


- [x] 12. Implement cross-reference checking module




  - Create `xtask/src/cross_ref.rs` module
  - Implement `build_req_index(ledger: &SpecLedger) -> (HashSet<String>, HashSet<String>)` - returns (req_ids, ac_ids)
  - Implement `validate_doc_links(docs: &[(PathBuf, FrontMatter)], req_ids: &HashSet<String>) -> Vec<CrossRefError>` - checks links.requirements
  - Implement `validate_test_references(ledger: &SpecLedger) -> Vec<CrossRefError>`:
    - Parse test reference format: `"<crate>::<module_path>::<test_fn_name>"`
    - Check if crate is in workspace members (parse Cargo.toml)
    - If not in workspace, emit warning (INF-XREF-1xx) instead of error
    - Use ripgrep to find `fn <test_fn>` in `crates/<crate>`
  - Add error formatting: `[ERROR] INF-XREF-NNN: <message>...`
  - Define severity levels: missing test = error (INF-XREF-0xx), external crate = warning (INF-XREF-1xx)
  - Add unit tests with fixtures for valid/invalid references
  - Acceptance: Tests pass for broken links, missing tests, and external crate warnings
  - _Requirements: INF-REQ-6.1, INF-REQ-6.2, INF-REQ-6.6_

-

- [x] 13. Implement Gherkin parsing and validation



  - Create `xtask/src/gherkin.rs` module
  - Implement `parse_feature_files(features_dir: &Path) -> Vec<GherkinScenario>`:
    - Parse .feature files in specs/features/
    - Extract scenario name, file path, line number
    - Extract tags from both Feature: and Scenario: lines (merge both)
  - Implement `extract_req_tags(tags: &[String]) -> Vec<String>` - filters @REQ-* and @INF-REQ-*
  - Implement `extract_ac_tags(tags: &[String]) -> Vec<String>` - filters @AC-*
  - Implement `validate_gherkin_tags(scenarios: &[GherkinScenario], req_ids: &HashSet<String>, ac_ids: &HashSet<String>) -> Vec<CrossRefError>`
  - Add error formatting: `[ERROR] INF-XREF-NNN: Invalid Gherkin tag...\n  File: <feature_path>:<line>...`
  - Cache parsed scenarios for reuse in ac-status command
  - Add unit tests with fixture .feature files
  - Acceptance: Tests pass for valid/invalid tags and both feature-level and scenario-level tag extraction
  - _Requirements: INF-REQ-3.2, INF-REQ-3.3, INF-REQ-6.3_

- [x] 14. Integrate cross-reference checks into validate





  - Update `xtask/src/validate.rs` to run cross-reference checks after schema validation, before code quality checks
  - Add cross-reference results to docs/validation_report.md with sections:
    - Broken requirement links (docs → ledger)
    - Missing test references (ledger → codebase)
    - Invalid Gherkin tags (features → ledger)
    - Orphaned documentation (docs with no requirement links)
  - Define exit code behavior: cross-ref errors cause validate to return 1
  - Acceptance: Create test fixture with broken link, run `cargo xtask validate`, verify error appears in report and exit code is 1
  - _Requirements: INF-REQ-6.5, INF-REQ-6.7_

-

- [x] 15. Implement cargo xtask ac-status command



  - Create `xtask/src/ac_status.rs` module
  - Implement `generate_feature_status(ledger: &SpecLedger, scenarios: &[GherkinScenario]) -> String`:
    - Reuse Property 8 logic for status computation per (REQ, AC):
      - ✅ Complete: status=tested, has tests, has Gherkin
      - 🟡 Needs Gherkin: status=implemented, has tests, no Gherkin
      - 🟡 Needs Tests: status=implemented, no tests
      - ⚪ Draft: status=draft
      - ❌ Incomplete: other cases
    - Generate markdown table with columns: REQ ID, AC ID, Description, Gherkin (file:line), Tests (count), Status
  - Generate `docs/feature_status.md` with auto-generated header: `<!-- AUTO-GENERATED FILE: DO NOT EDIT BY HAND. Generated by: cargo xtask ac-status ... -->`
  - Wire ac-status command into main.rs CLI
  - Acceptance: `cargo xtask ac-status` generates docs/feature_status.md with all columns and status icons
  - _Requirements: INF-REQ-10.3, INF-REQ-11.1, INF-REQ-11.4_

- [x] 16. Implement cargo xtask normalize-docs command






  - Create `xtask/src/normalize_docs.rs` module
  - Implement `verify_doc_id_uniqueness(docs: &[(PathBuf, FrontMatter)]) -> Result<()>` - checks for duplicate doc_ids
  - Implement `generate_docs_index(docs: &[(PathBuf, FrontMatter)]) -> String`:
    - Group docs by kind (band), then by area
    - Generate markdown tables with columns: Doc ID, Title (from file), Area, Status, Links
  - Generate `docs/README.md` with auto-generated header
  - normalize-docs does NOT mutate human-written content:
    - Never overwrites body content of docs
    - Only regenerates docs/README.md
    - Optionally normalizes front matter key ordering (but does not change values)
  - Wire normalize-docs command into main.rs CLI
  - Acceptance: `cargo xtask normalize-docs` generates docs/README.md with tables grouped by band and area
  - _Requirements: INF-REQ-10.4, INF-REQ-11.3_

- [x] 17. Implement cargo xtask validate-infra command





  - Create `xtask/src/validate_infra.rs` module
  - Implement Docker Compose validation:
    - Run `docker compose -f infra/local/docker-compose.yml config`
    - If docker/docker compose not installed, emit `[WARN] INF-INFRA-1xx: Docker not available` and skip (do not fail)
  - Implement Kubernetes validation (if infra/k8s/ exists):
    - Run `kubectl apply --dry-run=client -f infra/k8s/`
    - If kubectl not installed, emit warning and skip
  - Validate all `infra/**/invariants.yaml` files against `schemas/invariants.schema.json`
  - Cross-check invariants.yaml ports against compose service ports (if both exist)
  - Add error formatting: `[ERROR] INF-INFRA-NNN: <message>...`
  - Wire validate-infra command into main.rs CLI
  - Acceptance: `cargo xtask validate-infra` validates invariants and runs dry-run checks where tools are available
  - _Requirements: INF-REQ-10.5, INF-REQ-4.5, INF-REQ-4.6_

- [x] 18. Expand spec ledger with full requirements





  - Add all Flight Hub requirements (REQ-2 through REQ-14) to specs/spec_ledger.yaml
  - Add all infrastructure requirements (INF-REQ-2 through INF-REQ-12)
  - Consider adding optional `kind` field to requirements: `product` vs `infra` for easier filtering
  - Link existing tests where applicable (search codebase for test functions)
  - Update statuses based on implementation state (draft → implemented → tested)
  - Validate against schema: `cargo xtask validate` should pass
  - Acceptance: specs/spec_ledger.yaml contains all 26 requirements (14 product + 12 infra) and validates successfully
  - _Requirements: INF-REQ-2.1, INF-REQ-2.3_

- [x] 19. Create Gherkin features for key requirements




  - Create `specs/features/` directory
  - Standardize naming convention: use `req_<N>_<description>.feature` for both product and infra requirements
  - Create `specs/features/req_1_axis_processing.feature` with scenarios for REQ-1:
    - Tag Feature with @REQ-1
    - Tag each Scenario with appropriate @AC-1.1, @AC-1.2, etc.
  - Create `specs/features/req_inf_1_documentation.feature` with scenarios for INF-REQ-1:
    - Tag Feature with @INF-REQ-1
    - Tag each Scenario with appropriate @AC-1.1, @AC-1.2, etc.
  - Validate tags against spec ledger: `cargo xtask validate` should pass with no invalid tag errors
  - Acceptance: Both feature files exist, tags are valid, and appear in `cargo xtask ac-status` output
  - _Requirements: INF-REQ-3.1, INF-REQ-3.2_

- [ ] 20. Add comprehensive documentation with front matter
  - Add front matter to existing docs in docs/ directory (if any exist without it)
  - Create additional concept docs for major crates:
    - `docs/concepts/flight-hid.md` (area: flight-hid, links to REQ-3)
    - `docs/concepts/flight-ipc.md` (area: flight-ipc)
    - `docs/concepts/flight-scheduler.md` (area: flight-scheduler, links to REQ-1)
  - Create how-to guides:
    - `docs/how-to/setup-dev-env.md` (area: infra, links to INF-REQ-8)
    - `docs/how-to/run-benchmarks.md` (area: ci)
    - `docs/how-to/add-new-requirement.md` (area: infra, links to INF-REQ-2)
  - Enforce rule: any new concept doc for a crate SHOULD link to at least one REQ or INF-REQ in links.requirements
  - Run `cargo xtask normalize-docs` to generate docs/README.md index
  - Acceptance: All new docs have valid front matter, docs/README.md is generated with all docs listed
  - _Requirements: INF-REQ-1.2, INF-REQ-1.3, INF-REQ-1.6_

- [ ] 21. Final checkpoint - Verify Phase 2 complete
  - Run `cargo xtask validate` and verify exit code 0
  - Verify `docs/validation_report.md` includes:
    - Schema validation results
    - Cross-reference results (broken links, missing tests, invalid Gherkin tags)
    - Code quality results (fmt, clippy, tests)
    - Public API results (if cargo-public-api installed)
  - Run `cargo xtask ac-status` and verify `docs/feature_status.md` shows:
    - All 26 requirements (REQ-1 through REQ-14, INF-REQ-1 through INF-REQ-12)
    - Gherkin coverage for REQ-1 and INF-REQ-1
    - Status icons (✅🟡⚪❌) based on implementation state
  - Run `cargo xtask normalize-docs` and verify `docs/README.md` contains:
    - Tables grouped by band (requirements, design, concepts, how-to, reference, adr)
    - All documentation files listed with doc_id, area, status
  - Run `cargo xtask validate-infra` and verify:
    - invariants.yaml files validate against schema
    - Docker Compose config validates (or warning if docker not available)
  - Verify all xtask commands complete successfully

## Optional: BDD Step Runner

- [ ]* 22. Create specs crate for BDD execution
  - Create `specs/` crate in workspace with Cargo.toml
  - Add cucumber-rs dependency
  - Implement step definitions for common patterns:
    - Axis processing scenarios (REQ-1)
    - Documentation validation scenarios (INF-REQ-1)
  - Wire Gherkin features in specs/features/ to step implementations
  - Add to CI pipeline in .github/workflows/ci.yml: `cargo test -p specs`
  - Note: This task is optional; INF-REQ-3.5 only requires BDD execution "when enabled"
  - Acceptance: `cargo test -p specs` runs Gherkin scenarios and exits 0
  - _Requirements: INF-REQ-3.5_

## Notes

- Tasks marked with `*` are optional and can be skipped
- Each task should be completed before moving to the next
- Run acceptance commands after each task to verify completion
- Phase 1 provides immediate value; Phase 2 adds comprehensive validation
- Kiro should stop after each task for user review
