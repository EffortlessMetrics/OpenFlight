# ADR-004: Zero-Allocation Real-Time Constraint

## Status
Accepted

## Context

Real-time systems require predictable timing, but memory allocation can cause unpredictable delays due to garbage collection, heap fragmentation, or system calls. Flight Hub's 250Hz axis processing must maintain sub-millisecond jitter, making allocation-induced delays unacceptable.

## Decision

We enforce a strict zero-allocation constraint on the real-time path:

1. **Compile-Time Prevention**: Use `#![forbid(heap_allocation)]` on RT modules
2. **Runtime Monitoring**: Allocation counters that must remain zero during operation
3. **Pre-Allocation Strategy**: All memory allocated at startup or during configuration
4. **Stack-Only Operations**: RT code uses only stack allocation and pre-allocated buffers

### Implementation Strategy

```rust
// RT module with allocation prevention
#![forbid(heap_allocation)]

pub struct AxisEngine {
    // Pre-allocated at startup
    pipeline_nodes: Vec<Box<dyn Node>>,  // Allocated once
    frame_buffer: [AxisFrame; 1024],     // Stack allocation
    state_arena: Vec<u8>,                // Pre-allocated, reused
}

impl AxisEngine {
    pub fn process_tick(&mut self, input: f32) -> f32 {
        // Zero allocations - only stack and pre-allocated memory
        let mut frame = AxisFrame { input, output: 0.0 };
        
        for node in &mut self.pipeline_nodes {
            node.process(&mut frame);  // No allocations allowed
        }
        
        frame.output
    }
}
```

### Allocation Counter

```rust
static RT_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

// Custom allocator for RT threads
struct RTAllocator;

impl GlobalAlloc for RTAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if is_rt_thread() {
            RT_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            panic!("Allocation in RT thread!");
        }
        System.alloc(layout)
    }
}
```

## Consequences

### Positive
- Predictable timing with bounded jitter
- No GC pauses or allocation delays
- Clear performance characteristics
- Easier reasoning about RT behavior

### Negative
- Increased complexity in data structure design
- Requires careful memory management
- More difficult to use standard library features
- Higher upfront memory usage

## Alternatives Considered

1. **Real-Time GC**: Rejected due to complexity and unpredictability
2. **Memory Pools**: Rejected due to allocation overhead
3. **Stack-Only**: Rejected due to size limitations
4. **Custom Allocator**: Considered but adds complexity

## Implementation Guidelines

### Allowed in RT Code
- Stack allocation (`let x = [0; 1024]`)
- Pre-allocated containers (`Vec` allocated at startup)
- Atomic operations
- Pointer arithmetic
- Static data

### Forbidden in RT Code
- `Box::new()`, `Vec::push()` (if capacity exceeded)
- `String` operations that allocate
- `HashMap::insert()` (if rehashing needed)
- `Arc::new()`, `Rc::new()`
- Any `std::collections` operations that allocate

### Testing Strategy
- Unit tests verify allocation counter remains zero
- Integration tests with allocation monitoring
- Stress tests under memory pressure
- CI gates fail on any RT allocations

## Migration Path

1. **Phase 1**: Add allocation monitoring to existing code
2. **Phase 2**: Refactor hot paths to eliminate allocations
3. **Phase 3**: Add compile-time prevention attributes
4. **Phase 4**: Comprehensive testing and validation

## References

- Flight Hub Requirements: NFR-01, QG-AX-Jitter
- [Real-Time Memory Management](https://example.com)
- [Rust Embedded Guidelines](https://example.com)