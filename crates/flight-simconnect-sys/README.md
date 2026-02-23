# flight-simconnect-sys

Low-level FFI bindings for the Microsoft SimConnect SDK.

## Responsibilities

- Provides raw SimConnect ABI bindings for Rust consumers.
- Supports dynamic and static linking modes for SimConnect.
- Exposes foundational error/result handling for wrapper crates.

## MSFS 2024 Notes

- Dynamic loader attempts `SimConnect.dll` first, then `SimConnect_internal.dll`.
- This keeps local development resilient across MSFS 2020/2024 installations and SDK layouts.

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
