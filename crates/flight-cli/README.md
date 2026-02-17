# flight-cli

Command-line interface (flightctl) for controlling OpenFlight.

## Responsibilities

- Implements the flightctl command surface for service operations.
- Supports human and JSON output formatting for automation.
- Maps IPC/service errors to stable error codes and exit statuses.

## Key Modules

- `src/client_manager.rs`
- `src/output.rs`
- `src/commands/`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
