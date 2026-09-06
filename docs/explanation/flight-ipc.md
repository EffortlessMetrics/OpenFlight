---
doc_id: DOC-IPC-OVERVIEW
kind: explanation
area: flight-ipc
status: active
links:
  requirements: [REQ-4]
  tasks: []
  adrs: []
---

# Flight IPC Concepts

`flight-ipc` is OpenFlight's typed boundary between the daemon and local clients such as `flightctl` and future external control-stream consumers.

The crate already contains protobuf/gRPC types, client/server helpers, feature negotiation, retry/timeout machinery, and platform transport abstractions. Those pieces are not all wired into the production daemon yet. This document distinguishes the current implementation from the production contract so a library capability is not mistaken for an installed product capability.

See [Product and Evidence Boundaries](product-boundaries.md) for the wider runtime and release model.

## Current implementation state

On current `main`:

- protobuf definitions live in `crates/flight-ipc/proto/flight.v1.proto`;
- tonic client/server wrappers can run over a loopback TCP endpoint;
- the legacy/default client connects to `127.0.0.1:50051`;
- named-pipe and Unix-socket transport types exist in the transport module but are not the transport used by the production `IpcServer` path;
- `ServerConfig` contains platform-style default endpoint strings, but `IpcServer::start` currently accepts a TCP `SocketAddr`;
- the standalone production-style IPC constructor creates a HID device manager and a mock profile manager rather than receiving the runtime owned by `flightd`;
- several service-context operations have successful empty/default implementations intended for testability;
- `flightd` does not currently start this IPC server as part of its lifecycle.

These are implementation seams to finish, not release guarantees.

## Production contract

The production boundary is one per-user `flightd` runtime exposed through current-user local IPC.

```text
flightd
  owns RuntimeHandle
       |
       +-- devices / profiles / health / metrics / adapters
       +-- optional control-stream hub
       +-- optional flight-control runtime
       |
       v
local IPC server
       |
       +-- flightctl
       +-- external versioned consumers
```

IPC handlers delegate to the same runtime state that `flightd` owns. They do not create a second hardware manager, profile manager, control hub, or safety state.

This work is tracked by #311–#314.

## Local transport

The default product boundary is local and user-scoped.

### Windows

The production endpoint should use a named pipe or equivalent local transport with an ACL that grants access to the current user by default.

### Linux

The production endpoint should use a Unix-domain socket under the current user's runtime directory with user-scoped permissions and explicit stale-socket cleanup.

### Test/development transport

Loopback TCP remains useful for deterministic tests and development harnesses. It should be an explicit transport selection rather than the default installed trust boundary.

## Protocol and feature negotiation

Protocol versioning protects structural compatibility. Capability negotiation protects runtime compatibility.

A feature is not advertised merely because its code compiled. Negotiation is derived from initialized backing subsystems and distinguishes states such as:

```text
enabled
available but disabled
degraded
unavailable
unsupported
```

A client therefore does not need to infer capability from package version alone.

`control_stream_v1` follows this generic negotiation model; it does not introduce a separate application-specific capability system.

## Mutation and read paths

Production mutations should use an explicit acknowledged runtime command boundary. A successful RPC means the backing runtime accepted the operation and can identify the resulting state or generation.

Read-heavy calls should use stable snapshots where possible. Long-lived streams use bounded channels and explicit continuity semantics rather than unbounded buffering.

Examples:

- profile apply returns the accepted effective generation/hash;
- metrics report runtime counters rather than a zero-filled schema placeholder;
- disabled adapter/output features return a typed disabled/unavailable result rather than simulated success;
- health subscription forwards actual runtime health events.

## External control streams

The generic external control stream is non-real-time and observe-only. Its IPC ordering is:

```text
descriptor
-> non-actionable baseline
-> ordered events
-> explicit gap/reset/disconnect when continuity changes
```

Tonic/protobuf/network work remains outside the protected 250 Hz flight-control path. The control stream consumes bounded observations from service-owned state; it is not an intermediate stage of axis processing.

See #300, #302, #305, and #306.

## Security properties

The default IPC design aims for:

- local-only exposure;
- current-user access by default;
- bounded connection/request/stream resources;
- explicit feature/version negotiation;
- no implicit privileged whole-product daemon requirement;
- narrow, separately designed privilege boundaries if a future hardware helper requires elevation.

Remote administration, multi-user network authentication, and internet-facing APIs are separate product decisions and are not implied by the local IPC crate.

## Testing boundaries

IPC evidence is layered:

1. protobuf round-trip and handler unit tests;
2. explicit mock-server client tests;
3. runtime-backed integration tests using simulated sources;
4. installed daemon/client smoke through the platform local endpoint;
5. package upgrade/restart/rollback receipts for released contracts.

A passing mock TCP test establishes client/server protocol behavior. It does not by itself establish the installed Windows named-pipe or Linux Unix-socket lifecycle.

## Related work

- Installed daemon substrate: #311
- Canonical config and paths: #312
- RuntimeHandle and daemon-owned IPC: #313
- Local transport and truthful capability negotiation: #314
- Control-stream IPC: #302
- Package/release authority: #318
- Evidence states: #319
