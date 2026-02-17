# flight-streamdeck

StreamDeck integration with local Web API and profiles.

## Responsibilities

- Serves the local API surface used by the StreamDeck plugin.
- Implements compatibility checks for plugin/app version ranges.
- Bridges StreamDeck actions to core profile and telemetry flows.

## Key Modules

- `src/api.rs`
- `src/compatibility.rs`
- `src/plugin.rs`
- `src/profiles.rs`
- `src/server.rs`
- `src/verify.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
