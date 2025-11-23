---
doc_id: DOC-SCHEDULER-OVERVIEW
kind: concept
area: flight-scheduler
status: active
links:
  requirements: [REQ-1]
  tasks: []
  adrs: [ADR-005]
---

# Flight Scheduler Concepts

The `flight-scheduler` crate provides real-time scheduling infrastructure for the Flight Hub system, ensuring deterministic timing for the axis processing pipeline.

## Overview

Flight Scheduler is responsible for:
- 250Hz real-time loop execution
- Phase-locked loop (PLL) timing discipline
- Jitter minimization and measurement
- Platform-specific high-resolution timing

## Key Components

### Real-Time Loop

The real-time loop provides:
- Fixed 250Hz tick rate (4ms period)
- Deterministic scheduling with minimal jitter
- Priority elevation on supported platforms
- CPU affinity control for isolation

### Phase-Locked Loop (PLL)

The PLL timing discipline ensures:
- Automatic drift correction
- Adaptive timing based on measured jitter
- Smooth frequency adjustments
- Long-term stability

### Platform Abstractions

Platform-specific implementations provide:
- **Windows**: Multimedia timer API with 1ms resolution
- **Linux**: CLOCK_MONOTONIC with nanosecond precision
- **macOS**: Mach absolute time for high-resolution timing

### Metrics and Monitoring

The scheduler exposes metrics for:
- Tick interval measurements
- Jitter distribution (p50, p99, p999)
- Missed deadlines
- CPU utilization

## Performance Characteristics

- Target tick rate: 250Hz (4ms period)
- Jitter: < 0.5ms p99
- Missed deadlines: < 0.01% under normal load
- CPU overhead: < 5% on modern hardware

## Timing Guarantees

The scheduler provides the following guarantees:
1. **Deterministic Execution**: Each tick executes at a predictable time
2. **Bounded Jitter**: Timing variation stays within specified limits
3. **No Blocking**: The RT loop never blocks on I/O or locks
4. **Priority Inversion Protection**: RT thread runs at elevated priority

## Design Principles

Following **ADR-005: PLL Timing Discipline**, the scheduler:
- Uses phase-locked loop for drift correction
- Measures and adapts to system timing characteristics
- Provides fallback modes for degraded performance
- Exposes timing metrics for monitoring

## Related Requirements

This component implements **REQ-1: Real-Time Axis Processing**, specifically the timing and jitter requirements for the 250Hz processing pipeline.

## Related Components

- `flight-core`: Uses the scheduler for axis processing
- `flight-ffb`: Uses the scheduler for force feedback updates
- `flight-tracing`: Monitors scheduler performance metrics

## Testing

Flight Scheduler includes:
- Unit tests for PLL algorithm
- Integration tests measuring actual jitter
- Performance validation tests
- Platform-specific timing tests

## Usage Example

```rust
use flight_scheduler::Scheduler;

let scheduler = Scheduler::new(250)?; // 250Hz

scheduler.run(|tick| {
    // This closure runs at 250Hz with minimal jitter
    process_axis_inputs(tick);
    update_force_feedback(tick);
});
```

