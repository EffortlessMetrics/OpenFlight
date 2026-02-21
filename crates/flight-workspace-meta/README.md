# flight-workspace-meta

`flight-workspace-meta` provides small, focused utilities for OpenFlight workspace
microcrate discovery and crates.io metadata validation.

## Responsibilities

- Discover workspace members under `crates/*`.
- Resolve package fields that inherit via `*.workspace = true`.
- Validate crates.io-relevant metadata for each microcrate.

## Typical Usage

```rust
use flight_workspace_meta::{
    load_workspace_microcrate_names,
    validate_workspace_crates_io_metadata,
};

let names = load_workspace_microcrate_names(".")?;
let report = validate_workspace_crates_io_metadata(".")?;
```
