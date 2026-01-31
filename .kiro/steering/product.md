# Product Overview

OpenFlight (Flight Hub) is a PC flight simulation input management system written in Rust.

## Purpose
Unified control plane for flight controls, panels, and force feedback devices across multiple flight simulators (MSFS, X-Plane, DCS).

## Key Features
- Real-time 250Hz axis processing with deterministic performance
- Multi-simulator support (MSFS, X-Plane, DCS)
- Force feedback safety systems with proper interlocks
- Auto-profile switching based on aircraft detection
- Panel and StreamDeck integration with rule-based LED control
- Blackbox recording for diagnostics

## Performance Requirements
- Axis processing latency ≤ 5ms p99
- Jitter ≤ 0.5ms p99 at 250Hz
- Zero allocations on real-time hot paths
- CPU usage < 3% of one core during normal operation

## Applications
- `flightd` - Main service daemon (flight-service crate)
- `flightctl` - Command-line interface (flight-cli crate)
