# xtask

Workspace automation and validation tasks for OpenFlight.

## Responsibilities

- Runs project automation commands such as `check` and `validate`.
- Validates docs/spec references and other repository-wide invariants.
- Provides a single entrypoint for repeatable CI-local checks.

## Key Paths

- `src/main.rs`
- `src/check.rs`
- `src/validate.rs`
- `tests/`
