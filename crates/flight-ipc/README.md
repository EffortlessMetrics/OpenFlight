# flight-ipc

Cross-platform gRPC IPC layer for OpenFlight components.

## Responsibilities

- Defines protobuf-backed IPC contracts and generated model types.
- Implements client/server layers and version negotiation.
- Provides transport abstractions for named pipes and Unix sockets.

## Key Modules

- `src/client.rs`
- `src/negotiation.rs`
- `src/server.rs`
- `src/transport.rs`

## Status

Internal workspace crate; APIs can evolve with Flight Hub service requirements.
