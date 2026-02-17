# specs

BDD specification tests for OpenFlight requirements.

## Responsibilities

- Executes Gherkin/Cucumber scenarios under `specs/features/`.
- Verifies requirement-level behavior against workspace crates.
- Guards documentation and cross-reference integrity used by the docs system.

## Run

```bash
cargo test -p specs
```

## Key Paths

- `features/`
- `tests/cucumber.rs`
- `tests/steps/`
