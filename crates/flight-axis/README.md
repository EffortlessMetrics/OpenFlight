# flight-axis

Real-time axis processing engine for Flight Hub with zero-allocation guarantees and deterministic pipeline execution.

## Overview

The flight-axis crate implements Flight Hub's core 250Hz axis processing pipeline. It provides a compile-to-function-pointer system with atomic state swaps, ensuring deterministic behavior and strict real-time constraints. The engine processes flight control inputs through configurable pipeline nodes while maintaining sub-millisecond latency.

## Key Features

- **Zero-Allocation RT Path**: No memory allocation during real-time operation
- **Atomic State Swaps**: Configuration changes applied atomically at tick boundaries
- **Pipeline Nodes**: Modular processing with deadzone, curves, slew limiting, and mixing
- **Deterministic Execution**: Identical inputs produce identical outputs within FP tolerance
- **Detent Mapping**: Hysteretic detent zones with semantic role assignment
- **Cross-Axis Mixing**: Support for helicopter anti-torque and other multi-axis interactions

## Architecture

This crate implements several key architectural decisions:

- **[ADR-001: Real-Time Spine Architecture](../../docs/adr/001-rt-spine-architecture.md)** - Protected RT core with atomic state swaps
- **[ADR-004: Zero-Allocation Constraint](../../docs/adr/004-zero-allocation-constraint.md)** - Strict no-allocation policy for RT code

## Core Data Structures

```rust
#[repr(C)]
pub struct AxisFrame {
    pub in_raw: f32,        // Raw input value
    pub out: f32,           // Processed output value  
    pub d_in_dt: f32,       // Input derivative (per second)
    pub ts_mono_ns: u64,    // Monotonic timestamp
}

pub trait Node {
    #[inline(always)]
    fn step(&mut self, frame: &mut AxisFrame);
}
```

## Pipeline Nodes

### Core Processing Nodes

```rust
use flight_axis::{DeadzoneNode, CurveNode, SlewNode, DetentNode, MixerNode};

// Create pipeline nodes
let deadzone = DeadzoneNode::new(0.03); // 3% deadzone
let curve = CurveNode::exponential(0.2); // 20% exponential curve
let slew = SlewNode::new(1.2); // 1.2 units/second rate limit
let detent = DetentNode::new(detent_zones);
let mixer = MixerNode::new(mix_config);

// Compile to optimized pipeline
let pipeline = PipelineBuilder::new()
    .add_node(deadzone)
    .add_node(curve)
    .add_node(slew)
    .add_node(detent)
    .add_node(mixer)
    .compile()?;
```

### Node Types

1. **DeadzoneNode**: Symmetric/asymmetric dead zones with smooth transitions
2. **CurveNode**: Exponential, S-curve, and custom curve transformations (monotonic only)
3. **SlewNode**: Rate limiting in units/second with configurable attack/decay
4. **DetentNode**: Hysteretic detent zones with semantic roles (gear, flaps, etc.)
5. **MixerNode**: Cross-axis mixing for helicopter anti-torque and other interactions
6. **ClampNode**: Final output limiting with soft/hard clamp modes

## Real-Time Execution

```rust
use flight_axis::{AxisEngine, AxisFrame};

// Create engine with compiled pipeline
let mut engine = AxisEngine::new(pipeline);

// Real-time processing loop (250Hz)
loop {
    let mut frame = AxisFrame {
        in_raw: read_input(),
        out: 0.0,
        d_in_dt: 0.0,
        ts_mono_ns: get_monotonic_time(),
    };
    
    // Process frame (zero allocations!)
    engine.process(&mut frame);
    
    // Output processed value
    write_output(frame.out);
}
```

## Atomic Configuration Updates

Configuration changes are compiled off-thread and swapped atomically:

```rust
use flight_axis::{PipelineCompiler, ConfigUpdate};

// Compile new configuration off RT thread
let new_pipeline = PipelineCompiler::compile(new_config)?;

// Atomic swap at next tick boundary
engine.update_pipeline(new_pipeline).await?;
```

## Detent System

Detent zones provide hysteretic behavior for discrete positions:

```rust
use flight_axis::{DetentZone, DetentRole};

let gear_detent = DetentZone {
    center: 0.5,
    width: 0.1,
    hysteresis: 0.02,
    role: DetentRole::Gear,
};

let detent_node = DetentNode::new(vec![gear_detent]);
```

## Cross-Axis Mixing

Support for multi-axis interactions like helicopter anti-torque:

```rust
use flight_axis::{MixerConfig, MixerInput};

let mixer_config = MixerConfig {
    inputs: vec![
        MixerInput { axis: "collective", scale: -0.3 },
        MixerInput { axis: "pedals", scale: 1.0 },
    ],
    output_axis: "anti_torque",
};
```

## Performance Guarantees

- **Processing Latency**: ≤ 0.5ms p99 per frame
- **Jitter**: ≤ 0.5ms p99 at 250Hz (enforced by CI)
- **Allocations**: Zero on RT path (compile-time + runtime verification)
- **Determinism**: Identical inputs produce identical outputs within FP tolerance

## Data Layout Optimization

The engine uses Structure-of-Arrays (SoA) layout for cache efficiency:

```rust
// Optimized memory layout
struct PipelineState {
    node_states: Vec<u8>,     // Aligned to 64-byte boundaries
    function_ptrs: Vec<fn()>, // Static function pointers
    scratch_space: Vec<f32>,  // Pre-allocated scratch memory
}
```

## Quality Gates

This crate enforces strict quality gates in CI:

- **Zero-allocation verification** with runtime counters
- **Timing discipline validation** over 10+ minute runs  
- **Determinism testing** with property-based tests
- **Performance regression detection** with benchmarks

## Testing

```bash
# Run basic tests
cargo test --package flight-axis

# Run extended determinism tests
cargo test --package flight-axis test_determinism -- --ignored

# Run performance benchmarks
cargo bench --package flight-axis

# Verify zero-allocation constraint
cargo test --package flight-axis test_zero_alloc_constraint
```

## Unsafe Code

This crate uses `unsafe` code for:

- Lock-free atomic pointer swaps for pipeline updates
- SoA memory layout optimization with proper alignment
- Zero-allocation enforcement with compile-time checks

All unsafe code is thoroughly documented, tested, and isolated to specific modules.

## Requirements

This crate satisfies the following requirements:

- **AX-01**: Real-time axis processing with strict timing guarantees
- **NFR-01**: Performance and resource management constraints
- **QG-AX-Jitter**: CI quality gate for jitter measurement

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.