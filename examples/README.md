# flight-hub-examples

Feature-gated OpenFlight example binaries and integration demos.

## Responsibilities

- Provides runnable demos for core workflows such as profiles, IPC, and replay.
- Keeps optional integrations behind features to avoid heavy default builds.
- Serves as reference usage for workspace APIs during development.

## Run

```bash
cargo run -p flight-hub-examples --bin profile_demo
```

Enable feature-gated examples with `--features <feature>`.
