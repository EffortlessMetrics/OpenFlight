---
doc_id: DOC-PRODUCT-BOUNDARIES
kind: explanation
area: architecture
status: active
links:
  requirements: []
  tasks: []
  adrs: []
---

# Product and Evidence Boundaries

OpenFlight contains several substantial subsystems. A scheduler, parser, adapter, installer definition, or test can be real without the installed product boundary around it being complete. We keep those claims separate so implementation progress does not silently become release or hardware-support evidence.

This document defines the boundaries we use for runtime architecture, the current control-stream program, the 250 Hz flight-control product, packaging, and support claims.

## Three boundaries, not one completion percentage

OpenFlight currently has three related but distinct jobs.

### Installed control plane

The shared product substrate is the per-user `flightd` process, its configuration and lifecycle, secure local IPC, `flightctl`, diagnostics, and package integration.

Its target shape is:

```text
versioned configuration
  -> flightd owns the production runtime
  -> initialized subsystem state
  -> secure local IPC
  -> truthful negotiated capabilities
  -> truthful CLI and diagnostics
```

The production IPC server must expose the same runtime that `flightd` owns. It must not create a second hardware or profile owner, and unsupported operations must not manufacture successful responses.

This boundary is tracked by #311–#314.

### External control stream

`control_stream_v1` is a non-real-time, observe-only projection of normalized physical-controller state for external applications.

```text
device runtime
  -> stable descriptor
  -> non-actionable baseline
  -> ordered control events
  -> explicit gap/reset/disconnect
  -> bounded service-owned hub
  -> versioned IPC stream
```

OpenFlight owns device identity, parsing, calibration, normalization, and publication facts. Consumers own bindings and application semantics. A held control at subscription or reconnect is baseline state, not a newly generated action edge.

The control stream is deliberately not the data path for the 250 Hz flight-control loop. It may become the first installed alpha because it exercises the shared daemon, IPC, CLI, replay, package, and consumer seams without requiring device output or high-torque hardware claims.

This boundary is tracked by #300, #302–#310.

### 250 Hz flight-control runtime

The original flight-control product is a separate real-time path:

```text
latest bounded input state
  -> scheduler-owned tick
  -> complete multi-axis pipeline generation
  -> safety and freshness policy
  -> owned output backend
```

The protected tick must not depend on tonic, serialization, application consumers, unbounded queues, logging, string/map work, or descriptive conflict analysis. Control-plane code compiles configuration off-thread and publishes complete immutable generations; the scheduler owns active mutable processing state.

This boundary is tracked by #315–#317.

## Shared substrate does not collapse the products

The control stream and flight-control runtime share:

- product identity and paths;
- configuration versioning;
- daemon lifecycle;
- local IPC and capability negotiation;
- health and diagnostics;
- package and release machinery;
- evidence conventions.

They do not prove each other.

A working `control_stream_v1` does not establish 250 Hz timing or virtual/FFB output. A passing scheduler benchmark does not establish installed IPC, upgrade behavior, or an external consumer contract. A device parser fixture does not establish physical-device support.

## One production runtime owner

`flightd` is the production owner. Startup should converge on this sequence:

```text
load and migrate config
-> construct runtime
-> initialize requested subsystems
-> derive capability state
-> start current-user local IPC
-> publish ready state
```

Shutdown reverses ownership rather than dropping unrelated tasks opportunistically:

```text
stop accepting mutations
-> close or reset subscriptions
-> stop workers
-> neutralize owned outputs when present
-> persist durable state
-> stop IPC and remove endpoint
```

IPC handlers delegate to a service-owned runtime handle. Mutations use an explicit acknowledged command boundary; read-heavy operations use stable snapshots where possible. Mocks remain explicit test fixtures rather than production defaults.

## Capability state is observed state

A compiled feature is not automatically an available feature. Negotiation and operator surfaces distinguish at least:

```text
enabled
available but disabled
degraded
unavailable
unsupported
```

A production command reports success only after the backing subsystem accepted the operation and the resulting state was observed or acknowledged. A placeholder schema, zero-filled metric object, or note that an RPC is not implemented is not a successful operation.

## Per-user local security boundary

The default product is per-user on both supported desktop platforms.

- Windows uses a current-user local IPC boundary such as a named pipe with a user-scoped ACL.
- Linux uses a Unix-domain socket in the user runtime directory with user-scoped permissions.
- Loopback TCP may remain an explicit test/development transport, but it is not the default production trust boundary.
- A future privileged helper, if required for a narrow hardware operation, is a separate component with a narrow API rather than the whole OpenFlight daemon running privileged.

## Real-time ownership rules

When the flight-control runtime is enabled, the scheduler thread owns fixed processing state. The profile publication unit is a complete multi-axis generation, not a sequence of independent axis writes.

The protected path does not fail open because a control-plane lock is busy. In particular:

- processing is not skipped on lock contention;
- capability or safety clamps are not skipped on lock contention;
- profile generations are all-or-nothing;
- stale or disconnected input has an explicit output policy;
- shutdown and fault behavior neutralize owned output deterministically.

Descriptive conflict analysis and other allocation-heavy diagnostics operate on bounded observations outside the tick.

## Package state is product state

We treat the built and installed artifact as the authority for release claims. Source-tree package descriptions are inputs, not receipts.

The package contract records the coherent versions of:

- package and daemon;
- configuration/profile schemas;
- IPC contract;
- feature-specific external contracts;
- replay fixtures used by release smoke tests;
- required documentation, licenses, SBOM, and provenance assets.

Windows and Linux package renderers consume the same product truth. Installed tests inspect the actual package and system lifecycle.

A first alpha may use manual GitHub release downloads. Automatic self-update is a separate capability and is not considered production-ready merely because updater library code and placeholder keys exist.

This boundary is tracked by #318.

## Evidence states

We use evidence states instead of a single support/completion flag:

```text
specified
implemented
unit-tested
replay-tested
daemon-integrated
package-tested
hardware-validated
released
```

These states answer different questions.

- A linked acceptance criterion is not an executed test.
- A mock or shadow-state BDD step is not daemon integration.
- A source-tree binary is not package proof.
- A parser/replay fixture is not physical-device validation.
- Shared-host timing data is useful regression evidence but is not controlled HIL proof for a sub-millisecond guarantee.

Hardware and live-simulator promotion should reference receipts that identify the tested combination, package/commit, platform, relevant firmware/revision, fixture or capture hashes, and review/expiry state.

This boundary is tracked by #319.

## Release claims

A release claim is the narrowest claim supported by the installed receipt.

An installed control-stream alpha can claim a versioned external control stream if package tests prove descriptor/baseline/event/reset semantics through the installed daemon. It cannot therefore claim broad real-HOTAS support without device receipts.

A flight-control release can claim a particular input-to-output path once the installed package proves that path and controlled timing evidence establishes any performance guarantee. It cannot therefore promote every catalogued device, simulator, or FFB backend.

FFB high-torque output receives its own hardware safety evidence before release enablement.

## Current execution order

The dependency order is intentionally narrow:

```text
truthful current main and CI
-> installed daemon substrate (#311–#314)
-> control-stream contract/projector/hub (#300/#305/#306)
-> control-stream IPC/replay (#302/#303)
-> package authority (#318)
-> installed control-stream proof (#307/#308)
```

The real-time lane (#315–#317) can proceed in parallel after baseline correctness is restored. It does not block the first observe-only alpha.

The deciding criterion is evidence surface: we ship the smallest installed capability whose runtime, package, and consumer behavior can be proven end to end without borrowing claims from adjacent unfinished systems.
