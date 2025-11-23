---
doc_id: DOC-CORE-OVERVIEW
kind: concept
area: flight-core
status: draft
links:
  requirements: [REQ-1]
  tasks: []
  adrs: []
---

# Flight Core Concepts

The `flight-core` crate provides the foundational components for the Flight Hub system, including real-time axis processing, profile management, and core data structures.

## Overview

Flight Core is responsible for:
- Real-time axis processing with low-latency guarantees
- Profile loading and validation
- Aircraft detection and switching
- Core error handling and type definitions

## Key Components

### Axis Processing

The axis processing system handles input from flight control devices with strict latency requirements (≤ 5ms p99). This ensures responsive control feedback for flight simulation.

### Profile Management

Profiles define the configuration for different aircraft and control setups. The profile system supports:
- YAML-based configuration
- Validation against schemas
- Hot-reloading during development

### Aircraft Detection

Automatic aircraft detection enables seamless switching between different aircraft configurations based on the active simulator and aircraft type.

## Related Requirements

This component implements **REQ-1: Real-Time Axis Processing**, which specifies the latency and jitter requirements for the axis processing pipeline.
