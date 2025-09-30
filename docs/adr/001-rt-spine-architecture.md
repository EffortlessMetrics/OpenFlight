# ADR-001: Real-Time Spine Architecture

## Status
Accepted

## Context

Flight Hub needs to process flight control inputs at 250Hz with strict timing guarantees. The system must maintain consistent latency and jitter while supporting multiple simulators, devices, and user interfaces. Traditional event-driven architectures struggle with real-time constraints due to unpredictable scheduling and memory allocation.

## Decision

We adopt a "Real-Time Spine" architecture with the following principles:

1. **Protected RT Core**: A dedicated 250Hz loop that never blocks, allocates memory, or takes locks
2. **Atomic State Swaps**: Configuration changes compiled off-thread and swapped atomically at tick boundaries
3. **Drop-Tail Backpressure**: Non-RT systems use bounded queues with drop-tail policy to prevent RT blocking
4. **Isolation Boundaries**: Clear separation between RT and non-RT code with compile-time enforcement

### Architecture Components

```
┌─────────────────────────────────────────────────────────┐
│                    Non-RT Systems                       │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐   │
│  │ Sim Adapters│ │   Panels    │ │   Diagnostics   │   │
│  └─────────────┘ └─────────────┘ └─────────────────┘   │
└─────────────────────┬───────────────────────────────────┘
                      │ Drop-tail queues
┌─────────────────────┴───────────────────────────────────┐
│                  RT Spine (250Hz)                       │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐   │
│  │ Axis Engine │ │ FFB Engine  │ │   Scheduler     │   │
│  └─────────────┘ └─────────────┘ └─────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Consequences

### Positive
- Deterministic timing with bounded jitter
- Predictable behavior under load
- Clear separation of concerns
- Testable with synthetic loads

### Negative
- Increased complexity in state management
- Requires careful design of RT/non-RT boundaries
- More difficult debugging across boundaries

## Alternatives Considered

1. **Event-Driven Architecture**: Rejected due to unpredictable timing
2. **Actor Model**: Rejected due to message passing overhead
3. **Cooperative Multitasking**: Rejected due to blocking risk

## Implementation Notes

- RT threads use `SCHED_FIFO` (Linux) or `MMCSS "Games"` (Windows)
- All RT allocations happen at startup
- Atomic pointer swaps for configuration updates
- Compile-time checks prevent RT violations

## References

- [Real-Time Systems Design Patterns](https://example.com)
- [Lock-Free Programming](https://example.com)
- Flight Hub Requirements: NFR-01, QG-AX-Jitter