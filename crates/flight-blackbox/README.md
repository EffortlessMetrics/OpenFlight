# flight-blackbox

Flight Black Box (.fbb) recording and replay data format support.

## Responsibilities

- Reads and writes the OpenFlight .fbb capture format.
- Maintains indexed stream data with CRC integrity checks.
- Provides bounded buffering and runtime stats for capture reliability.

## Key Modules

- `src/time.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
