---
doc_id: DOC-IPC-OVERVIEW
kind: concept
area: flight-ipc
status: active
links:
  requirements: [REQ-4]
  tasks: []
  adrs: []
---

# Flight IPC Concepts

The `flight-ipc` crate provides inter-process communication infrastructure for Flight Hub, enabling the service, CLI, and UI to communicate efficiently and reliably.

## Overview

Flight IPC is responsible for:
- gRPC-based client-server communication
- Protocol buffer schema management
- Connection lifecycle and health monitoring
- API versioning and backward compatibility

## Key Components

### gRPC Service

The gRPC service layer provides:
- Type-safe RPC definitions via Protocol Buffers
- Bidirectional streaming for real-time updates
- Connection multiplexing over a single TCP connection
- Built-in retry and timeout handling

### Protocol Buffers

The IPC protocol is defined in `proto/flight.v1.proto` and includes:
- Device management RPCs (list, configure, status)
- Profile management RPCs (load, validate, switch)
- Health check and monitoring RPCs
- Streaming telemetry subscriptions

### Client Library

The client library provides:
- Automatic connection management
- Request/response helpers
- Streaming subscription utilities
- Error handling and retry logic

### Server Framework

The server framework handles:
- Service registration and routing
- Request authentication and authorization
- Concurrent request handling
- Graceful shutdown

## API Versioning

Flight IPC follows semantic versioning for the protocol:
- Major version changes indicate breaking changes
- Minor version changes add backward-compatible features
- Patch versions fix bugs without API changes

The system supports multiple protocol versions simultaneously during transitions.

## Performance Characteristics

- Connection establishment: < 50ms
- RPC latency: < 5ms p99 for local connections
- Streaming throughput: > 10k messages/sec
- Memory overhead: < 1MB per client connection

## Security

IPC communication includes:
- Local-only binding by default (127.0.0.1)
- Optional TLS for remote connections
- Request validation and sanitization
- Rate limiting per client

## Related Requirements

This component implements **REQ-4: Multi-Process Architecture**, which specifies the requirements for service isolation and inter-process communication.

## Related Components

- `flight-service`: Main service that hosts the gRPC server
- `flight-cli`: Command-line client using the IPC library
- `flight-ui`: GUI client using the IPC library

## Testing

Flight IPC includes:
- Unit tests for protocol encoding/decoding
- Integration tests with client/server pairs
- API compatibility tests
- Performance benchmarks
- Breaking change detection

