# flight-scheduler

Real-time scheduler for Flight Hub with PLL-based timing discipline and zero-allocation guarantees.

## Overview

The flight-scheduler crate provides precise 250Hz timing for Flight Hub's real-time axis processing. It implements a Phase-Locked Loop (PLL) timing discipline to maintain sub-millisecond jitter while providing bounded SPSC ring buffers for non-blocking communication.

## Key Features

- **PLL Timing Discipline**: Automatic compensation for systematic timing drift
- **Zero-Allocation RT Path**: No memory allocation during real-time operation
- **Cross-Platform**: Windows (MMCSS) and Linux (SCHED_FIFO) support
- **Drop-Tail Backpressure**: Bounded queues that never block RT threads
- **Comprehensive Metrics**: Jitter measurement and performance monitoring

## Architecture

This crate implements the Real-Time Spine architecture as defined in [ADR-001](../../docs/adr/001-rt-spine-architecture.md). The scheduler maintains strict timing guarantees while isolating non-RT systems through bounded communication channels.

### Core Components

- **Scheduler**: Main 250Hz timing loop with PLL correction
- **SpscRing**: Lock-free single-producer single-consumer ring buffer
- **JitterMetrics**: Real-time jitter measurement and analysis
- **PLL**: Phase-locked loop for timing discipline

## Usage

```rust
use flight_scheduler::{Scheduler, SchedulerConfig, SpscRing};

// Create scheduler with default 250Hz timing
let config = SchedulerConfig::default();
let mut scheduler = Scheduler::new(config);

// Create communication ring
let ring: SpscRing<MyData> = SpscRing::new(1024);

// Real-time loop
loop {
    let tick = scheduler.wait_for_tick();
    
    // Process data (zero allocations!)
    process_rt_data();
    
    // Non-blocking communication
    if let Some(data) = ring.try_pop() {
        handle_data(data);
    }
}
```

## Performance Guarantees

- **Jitter p99**: ≤ 0.5ms (enforced by CI)
- **Miss Rate**: ≤ 0.1% (ticks >1.5× period late)
- **Allocations**: Zero on RT path (compile-time + runtime checks)
- **Latency**: ≤ 5ms p99 input to output

## Quality Gates

This crate enforces strict quality gates in CI:

- Timing discipline validation over 10+ minute runs
- Zero-allocation verification with runtime counters
- Cross-platform timing consistency
- Performance regression detection

## Architecture Decisions

This crate implements several key architectural decisions:

- **[ADR-001: Real-Time Spine Architecture](../../docs/adr/001-rt-spine-architecture.md)** - Protected RT core with atomic state swaps
- **[ADR-004: Zero-Allocation Constraint](../../docs/adr/004-zero-allocation-constraint.md)** - Strict no-allocation policy for RT code
- **[ADR-005: PLL Timing Discipline](../../docs/adr/005-pll-timing-discipline.md)** - Phase-locked loop for timing stability

## Platform Support

### Windows
- Uses `CreateWaitableTimer` for high-precision sleep
- MMCSS "Games" thread class for RT priority
- Process power throttling disabled

### Linux
- Uses `clock_nanosleep(CLOCK_MONOTONIC)` for precision
- SCHED_FIFO via rtkit for RT scheduling
- Memory locking with `mlockall`

## Testing

```bash
# Run basic tests
cargo test --package flight-scheduler

# Run extended timing validation (requires time)
cargo test --package flight-scheduler test_extended_timing_discipline -- --ignored

# Run performance benchmarks
cargo bench --package flight-scheduler
```

## Safety

This crate uses `unsafe` code for:
- Lock-free ring buffer operations (with careful memory ordering)
- Platform-specific RT scheduling APIs
- Zero-allocation enforcement in RT paths

All unsafe code is thoroughly documented and tested.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.