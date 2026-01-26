# PR: Repo Cleanup, Diataxis Documentation Reorg, and Roadmap Definition

## Summary
This PR performs a major cleanup of the repository root and restructures the documentation to align with the **Diataxis framework**. It also establishes high-level project management documents including a Roadmap, Changelog, and Priority tracking to guide the project's journey toward production readiness.

##  Changes

### 1. Documentation Restructuring (Diataxis Framework)
The `docs/` directory has been reorganized into the four standard Diataxis quadrants to distinct user needs:

*   **Tutorials** (`docs/tutorials/`): Added `getting-started.md` as a step-by-step entry point for new developers.
*   **How-To Guides** (`docs/how-to/`): Grouped practical, goal-oriented guides (e.g., `run-benchmarks.md`, `integrate-xinput.md`).
*   **Explanation** (`docs/explanation/`): Renamed from `concepts/`; contains deep-dive architectural docs (e.g., `flight-core.md`, `quality-gates.md`).
*   **Reference** (`docs/reference/`): Consolidated technical specifications and third-party license info.

**Infrastructure Updates:**
*   Updated `xtask` source code (`front_matter.rs`, `normalize_docs.rs`) to support new document kinds (`tutorial`, `explanation`).
*   Regenerated `docs/README.md` to reflect the new structure.

### 2. Project Management & Strategy
Established clear documentation for project status and governance:

*   **`ROADMAP.md`**: Created a detailed roadmap covering Milestones 0-7, defining the path from "Foundation" to "Production Readiness."
*   **`CHANGELOG.md`**: Documented recent unreleased features (FFB Safety Envelope, Blackbox, Sim Adapters) and historical context.
*   **`docs/NOW_NEXT_LATER.md`**: Added a strategic prioritization document to focus current efforts on "Production Readiness" and "Runtime Reliability."
*   **`docs/GOVERNANCE.md`**: Defined the project's core values (Safety First, Real-Time Reliability), RFC process, and contributor roles.
*   **`CONTRIBUTING.md`**: Created a clear entry point for contributors, linking to the priority docs and defining the validation pipeline.

### 3. Repository Cleanup
*   Removed root-level clutter.
*   Moved development-specific documentation (e.g., `CROSS_REF_MODULE.md`) into `docs/dev/` or `docs/explanation/` as appropriate.

## Verification
*   **Documentation Index**: Ran `cargo xtask normalize-docs`.
    *   ✅ Generated `docs/README.md` with correct grouping (Tutorials, How-To, Explanation, Reference).
    *   ✅ Verified all doc links and metadata.
*   **Validation**:
    *   ✅ `cargo xtask check` passes (no broken links or lint errors).

## Checklist
- [x] Documentation updated (Diataxis structure implemented).
- [x] Roadmap and Changelog created.
- [x] `xtask` tooling updated to support new documentation schema.
- [x] `CONTRIBUTING.md` updated with new workflow instructions.
