# Project Structure

## Crate Organization

### Real-Time Core (250Hz spine - no allocations, no locks)
- `flight-axis` - Axis processing (curves, deadzones, detents, mixers)
- `flight-scheduler` - Platform RT scheduling (MMCSS/rtkit)
- `flight-bus` - Event bus for inter-component communication
- `flight-blackbox` - Flight data recording

### Hardware Integration
- `flight-hid` - HID device management
- `flight-hid-support` - HID descriptor parsing
- `flight-hid-types` - Shared HID types
- `flight-ffb` - Force feedback synthesis
- `flight-panels` - Generic panel driver
- `flight-panels-core/saitek/cougar` - Panel implementations
- `flight-streamdeck` - StreamDeck integration
- `flight-tactile` - Haptic feedback

### Simulator Adapters
- `flight-simconnect` / `flight-simconnect-sys` - MSFS SimConnect
- `flight-xplane` - X-Plane UDP/plugin
- `flight-dcs-export` - DCS Export.lua integration
- `flight-adapter-common` - Shared adapter utilities

### Infrastructure
- `flight-core` - Core types, profile management, aircraft detection
- `flight-ipc` - gRPC-based IPC (protobuf in `proto/`)
- `flight-profile` - Profile schema and validation
- `flight-rules` - Rule engine for panel/LED control
- `flight-writers` - Table-driven sim variable writers
- `flight-replay` - Session replay
- `flight-tracing` - Observability
- `flight-metrics` - Performance metrics
- `flight-security` - Security utilities
- `flight-updater` - Auto-update system
- `flight-watchdog` - Process monitoring
- `flight-session` - Session management
- `flight-units` - Unit conversions
- `flight-virtual` - Virtual device support
- `flight-process-detection` - Sim process detection

### Applications
- `flight-service` - Main daemon (`flightd`)
- `flight-cli` - CLI (`flightctl`)
- `flight-ui` - GUI components

## Other Directories
- `docs/` - Documentation (Diataxis: explanation, how-to, reference, tutorials)
- `docs/explanation/adr/` - Architecture Decision Records
- `examples/` - Example code
- `specs/` - Gherkin feature specs and cucumber tests
- `xtask/` - Build automation tasks
- `scripts/` - CI and validation scripts
- `schemas/` - JSON schemas
- `infra/` - Infrastructure configs

## Architecture (ADR-001: RT Spine)
```
Non-RT Systems (Sim Adapters, Panels, Diagnostics)
                    │ Drop-tail queues
RT Spine (250Hz): Axis Engine │ FFB Engine │ Scheduler
```
Configuration changes compiled off-thread, swapped atomically at tick boundaries.
