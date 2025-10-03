# ADR-007: Pipeline Ownership Model

## Status
Accepted

## Context

Flight Hub processes multiple input streams (axes, buttons, telemetry) through configurable pipelines. The system needs clear ownership semantics for pipeline configuration, state management, and resource allocation to prevent conflicts and ensure deterministic behavior.

## Decision

We implement a hierarchical ownership model with clear boundaries and atomic transitions:

### 1. Pipeline Ownership Hierarchy

```
Global Profile
    ↓ (inherits/overrides)
Simulator Profile  
    ↓ (inherits/overrides)
Aircraft Profile
    ↓ (inherits/overrides)
Phase of Flight Overrides
```

### 2. Ownership Rules

- **Single Writer**: Only one profile level can own a specific axis configuration
- **Last Writer Wins**: More specific profiles override general ones
- **Atomic Updates**: Profile changes compile off-thread and swap atomically
- **Immutable Runtime**: Active pipelines cannot be modified during execution

### 3. State Management

```rust
pub struct PipelineOwnership {
    // Compiled pipeline (immutable during RT execution)
    active_pipeline: Arc<CompiledPipeline>,
    
    // Pending compilation (off-thread)
    pending_compilation: Option<JoinHandle<CompiledPipeline>>,
    
    // Ownership tracking
    config_source: ProfileSource,
    effective_hash: u64,
}

pub enum ProfileSource {
    Global,
    Simulator(SimId),
    Aircraft(AircraftId),
    PhaseOfFlight(PoFId),
}
```

### 4. Compilation and Swap Process

1. **Profile Change Detected**: New configuration triggers compilation
2. **Off-Thread Compilation**: Pipeline compiled without blocking RT
3. **Validation**: Compiled pipeline validated for correctness
4. **Atomic Swap**: New pipeline swapped at tick boundary
5. **Acknowledgment**: Client receives confirmation of successful apply

### 5. Conflict Resolution

When multiple profiles affect the same axis:
- **Scalars**: Last writer wins (most specific profile)
- **Arrays**: Keyed merge by identity with documented tie-breaking
- **Curves**: Monotonicity validation, reject non-monotonic
- **Detents**: Merge zones, validate no overlaps

## Consequences

### Positive
- Clear ownership semantics prevent configuration conflicts
- Atomic updates ensure consistent state
- Hierarchical inheritance provides flexibility
- Deterministic behavior across profile changes

### Negative
- Increased complexity in profile management
- Memory overhead for multiple pipeline versions
- Potential delays during compilation phase

## Alternatives Considered

1. **Mutable Pipelines**: Rejected due to RT safety concerns
2. **Copy-on-Write**: Rejected due to allocation in RT path
3. **Lock-Based Updates**: Rejected due to RT blocking risk
4. **Event-Driven Updates**: Rejected due to timing unpredictability

## Implementation Details

### Profile Merging Algorithm

```rust
impl ProfileMerger {
    pub fn merge_hierarchy(&self, profiles: &[Profile]) -> MergedProfile {
        let mut result = MergedProfile::default();
        
        // Apply in hierarchy order: Global → Sim → Aircraft → PoF
        for profile in profiles {
            result.merge_scalars_last_writer_wins(profile);
            result.merge_arrays_by_key(profile);
            result.validate_monotonic_curves(profile)?;
        }
        
        result.compute_effective_hash()
    }
}
```

### Atomic Swap Mechanism

```rust
impl AxisEngine {
    pub fn apply_pipeline(&mut self, new_pipeline: CompiledPipeline) -> Result<()> {
        // Validate pipeline before swap
        new_pipeline.validate()?;
        
        // Atomic swap at next tick boundary
        self.pending_pipeline = Some(new_pipeline);
        
        // RT loop will swap at safe point
        Ok(())
    }
    
    fn rt_tick(&mut self) {
        // Check for pending pipeline swap
        if let Some(new_pipeline) = self.pending_pipeline.take() {
            self.active_pipeline = Arc::new(new_pipeline);
        }
        
        // Process with current pipeline
        self.active_pipeline.process_frame(&mut self.frame);
    }
}
```

### Ownership Tracking

- Each axis tracks its configuration source
- Profile changes update ownership metadata
- Conflicts logged with clear resolution path
- UI displays current ownership hierarchy

## Testing Strategy

### Unit Tests
- Profile merging with various hierarchies
- Conflict resolution scenarios
- Atomic swap correctness
- Hash determinism across platforms

### Integration Tests
- Multi-profile scenarios
- Concurrent profile updates
- Pipeline compilation failures
- RT timing during profile changes

### Property Tests
- Merge determinism (same inputs → same output)
- Hash stability across runs
- Monotonicity preservation
- Ownership consistency

## Error Handling

### Compilation Failures
- Invalid profiles rejected with line/column errors
- RT pipeline remains unchanged on failure
- Clear error messages with resolution guidance

### Runtime Errors
- Pipeline validation catches errors before swap
- Fallback to previous known-good configuration
- Error reporting with ownership context

## Performance Considerations

- Profile compilation happens off-thread
- Atomic swaps use Arc for zero-copy sharing
- Hash computation cached to avoid recomputation
- Memory usage bounded by profile complexity

## References

- Flight Hub Requirements: PRF-01, AX-01
- [Ownership and Borrowing in Rust](https://example.com)
- [Lock-Free Data Structures](https://example.com)