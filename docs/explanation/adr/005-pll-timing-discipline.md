# ADR-005: PLL-Based Timing Discipline

## Status
Accepted

## Context

Maintaining precise 250Hz timing is challenging due to system jitter, USB frame timing variations, and scheduler inconsistencies. Simple fixed-interval scheduling accumulates timing drift, while naive correction can cause instability. Flight Hub requires sub-millisecond jitter for professional-grade performance.

## Decision

We implement a Phase-Locked Loop (PLL) timing discipline system:

1. **Absolute Scheduling**: Target times calculated from absolute start time, not intervals
2. **Phase Error Tracking**: Measure timing error and accumulate phase drift
3. **Proportional Correction**: Apply gradual correction to period length (≤0.1%/s)
4. **Bounded Adjustment**: Limit corrections to ±1% to prevent instability
5. **Busy-Spin Tail**: Final 50-80μs uses CPU spinning for precision

### PLL Algorithm

```rust
pub struct Pll {
    gain: f64,                    // 0.001 = 0.1%/s correction rate
    nominal_period_ns: f64,       // 4,000,000ns for 250Hz
    corrected_period_ns: f64,     // Current adjusted period
    phase_error: f64,             // Accumulated timing error
}

impl Pll {
    pub fn update(&mut self, error_ns: f64) -> f64 {
        // Accumulate phase error
        self.phase_error += error_ns;
        
        // Apply proportional correction
        let correction = -self.gain * self.phase_error;
        self.corrected_period_ns = self.nominal_period_ns + correction;
        
        // Bound correction to ±1%
        let max_correction = self.nominal_period_ns * 0.01;
        self.corrected_period_ns = self.corrected_period_ns
            .clamp(self.nominal_period_ns - max_correction,
                   self.nominal_period_ns + max_correction);
        
        self.corrected_period_ns
    }
}
```

### Timing Sequence

```
1. Calculate next absolute target time
2. Sleep until ~50-80μs before target
3. Busy-spin until exact target time
4. Measure actual timing error
5. Update PLL with error
6. Use corrected period for next cycle
```

## Consequences

### Positive
- Excellent long-term timing stability
- Automatic compensation for systematic drift
- Bounded corrections prevent instability
- Works across different hardware/OS combinations

### Negative
- More complex than simple interval timing
- Requires tuning of PLL parameters
- CPU usage from busy-spin tail
- Potential for oscillation if poorly tuned

## Alternatives Considered

1. **Fixed Intervals**: Rejected due to drift accumulation
2. **Simple Error Correction**: Rejected due to instability risk
3. **Hardware Timers**: Rejected due to portability issues
4. **Adaptive Intervals**: Rejected due to complexity

## Implementation Details

### Platform-Specific Timing
- **Windows**: `CreateWaitableTimer` with high-resolution flag
- **Linux**: `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)`
- **Fallback**: `std::thread::sleep` with longer busy-spin

### PLL Parameters
- **Gain**: 0.001 (0.1%/s correction rate)
- **Max Correction**: ±1% of nominal period
- **Busy-Spin**: 50-80μs before target time
- **Measurement**: Monotonic clock with nanosecond precision

### Quality Gates
- Jitter p99 ≤ 0.5ms over 10+ minute runs
- Miss rate ≤ 0.1% (ticks >1.5× period late)
- Phase drift ≤ 1ms over 1 hour runs

## Tuning Guidelines

### Conservative (Default)
- Gain: 0.001 (0.1%/s)
- Busy-spin: 65μs
- Good for most systems

### Aggressive (High-Performance)
- Gain: 0.002 (0.2%/s)
- Busy-spin: 80μs
- For dedicated RT systems

### Gentle (Shared Systems)
- Gain: 0.0005 (0.05%/s)
- Busy-spin: 50μs
- For development machines

## Testing Strategy

- Unit tests verify PLL mathematics
- Integration tests measure actual timing
- Stress tests under system load
- Long-running stability tests (24+ hours)
- Cross-platform validation

## References

- Flight Hub Requirements: NFR-01, QG-AX-Jitter
- [Phase-Locked Loop Theory](https://example.com)
- [Real-Time Scheduling](https://example.com)