# Documentation Reorganization Plan (Diataxis)

## Current State Analysis
The current documentation is partially structured but mixes categories.

## Target Structure (Diataxis)

### 1. Tutorials (Learning-oriented)
*Goal: "Getting Started", "Your First X"*
*   [ ] Create `docs/tutorials/`
*   [ ] Create `docs/tutorials/getting-started.md` (New content)
*   [ ] Create `docs/tutorials/writing-your-first-profile.md` (New content)

### 2. How-To Guides (Problem-oriented)
*Goal: "How to X"*
*   Existing: `docs/how-to/*`
*   [ ] Move `docs/integration/xinput-integration-guide.md` -> `docs/how-to/integrate-xinput.md`
*   [ ] Move `docs/integration/ffb-emergency-stop.md` -> `docs/how-to/configure-emergency-stop.md`
*   [ ] Move `docs/troubleshooting.md` -> `docs/how-to/troubleshoot-common-issues.md`
*   [ ] Move `docs/regression-prevention-quick-reference.md` -> `docs/how-to/prevent-regressions.md`

### 3. Explanation (Understanding-oriented)
*Goal: "Concepts", "Why X"*
*   Existing: `docs/concepts/*`
*   [ ] Create `docs/explanation/`
*   [ ] Move `docs/concepts/*` -> `docs/explanation/`
*   [ ] Move `docs/adr/` -> `docs/explanation/adr/` (ADRs are explanatory context)
*   [ ] Move `docs/integration/xinput-limitations.md` -> `docs/explanation/xinput-limitations.md`
*   [ ] Move `docs/security/SUPPLY_CHAIN_SECURITY.md` -> `docs/explanation/supply-chain-security.md`
*   [ ] Move `docs/dev/quality-gates.md` -> `docs/explanation/quality-gates.md`

### 4. Reference (Information-oriented)
*Goal: Specs, API docs, Tables*
*   Existing: `docs/reference/*`
*   [ ] Move `docs/requirements/` -> `docs/reference/requirements/`
*   [ ] Move `docs/integration/msfs-simvar-mapping.md` -> `docs/reference/msfs-mapping.md`
*   [ ] Move `docs/integration/xplane-data-groups.md` -> `docs/reference/xplane-mapping.md`
*   [ ] Move `docs/integration/dcs-export-api.md` -> `docs/reference/dcs-api.md`
*   [ ] Move `docs/ci-required-checks.md` -> `docs/reference/ci-checks.md`
*   [ ] Move `docs/feature_status.md` -> `docs/reference/feature-status.md`
*   [ ] Move `docs/dev/clippy-core.md` -> `docs/reference/clippy-rules.md`

### 5. Archive / Meta
*   `docs/implementation_history/` (Keep as archive)
*   `docs/dev/` (Keep remaining dev-specific docs not fitting above, or move to `docs/contribution/`)

## Execution Steps
1.  Create directories: `tutorials`, `explanation`
2.  Move files to new locations.
3.  Create "Getting Started" tutorial content.
4.  Update `docs/README.md` (or run `xtask normalize-docs` if it handles the new structure, otherwise manual update).

## Question
Does the user want me to execute this move now?
The prompt said "Fully build out diataxis docs", which implies action. I will proceed with creating the structure and moving files, then creating the missing "Tutorials" content.
