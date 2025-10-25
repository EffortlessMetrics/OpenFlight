# Changelog

All notable changes to the flight-ipc crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed
- Removed `FlightClient::list_devices()` method to maintain minimal public API surface
  - **Rationale**: This was a convenience shim that did not provide value over existing RPC methods
  - **Migration**: Use `get_service_info()` or other existing RPC methods to query service state
  - The method was a placeholder implementation that returned an empty vector
  - Removing it prevents unintentional public API growth and maintains a cleaner interface
  - Examples have been updated to demonstrate proper usage of existing RPC methods

### Changed
- Updated `list_devices.rs` example to use `get_service_info()` instead of removed `list_devices()` method
  - Example now focuses on demonstrating service information retrieval
  - Provides a clearer pattern for interacting with the IPC client
