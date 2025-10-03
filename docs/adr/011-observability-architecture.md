# ADR-011: Observability Architecture

## Status
Accepted

## Context

Flight Hub is a real-time system with strict performance requirements where traditional logging and monitoring approaches can interfere with timing guarantees. The system needs comprehensive observability for debugging, performance monitoring, and user support while maintaining zero impact on the real-time path.

## Decision

We implement a multi-tier observability architecture with zero-RT-impact design:

### 1. Observability Tiers

**Tier 1: Real-Time Counters (Zero Impact)**
- Atomic counters only, no allocations or syscalls
- Updated in RT path, read from non-RT threads
- Performance metrics: jitter, latency, throughput

**Tier 2: Structured Events (Async)**
- Events queued from RT, processed off-thread
- State changes, faults, configuration updates
- Bounded queues with drop-tail policy

**Tier 3: Blackbox Recording (Continuous)**
- Binary format for high-frequency data
- Axis frames, telemetry, timing measurements
- Chunked writes with minimal RT impact

**Tier 4: Tracing Integration (Optional)**
- ETW (Windows) / tracepoints (Linux) hooks
- Detailed execution flow for debugging
- Can be enabled/disabled at runtime

### 2. Data Architecture

```rust
// Tier 1: RT Counters (lock-free, atomic)
pub struct RtCounters {
    pub ticks_processed: AtomicU64,
    pub missed_ticks: AtomicU64,
    pub hid_writes: AtomicU64,
    pub hid_write_errors: AtomicU64,
    pub allocation_violations: AtomicU64,
    pub jitter_p99_ns: AtomicU64,
}

// Tier 2: Structured Events
#[derive(Serialize)]
pub enum ObservabilityEvent {
    TickStart { timestamp_ns: u64, tick_id: u64 },
    TickEnd { timestamp_ns: u64, tick_id: u64, duration_ns: u32 },
    HidWrite { timestamp_ns: u64, device_id: u32, latency_ns: u32 },
    ProfileApplied { timestamp_ns: u64, profile_hash: u64, source: String },
    FaultDetected { timestamp_ns: u64, fault_type: FaultType, context: String },
    DeadlineMiss { timestamp_ns: u64, expected_ns: u64, actual_ns: u64 },
}

// Tier 3: Blackbox Format
#[repr(C)]
pub struct BlackboxHeader {
    pub magic: [u8; 4],           // "FBB1"
    pub endian: u8,               // 0x01 = little, 0x02 = big
    pub version: u8,              // Format version
    pub app_version: [u8; 16],    // Application version string
    pub timebase_ns: u64,         // Monotonic clock base
    pub sim_id: u32,              // Active simulator
    pub aircraft_id: u64,         // Aircraft identifier hash
    pub mode_flags: u32,          // Recording mode flags
}
```

### 3. Zero-Impact RT Design

```rust
impl AxisEngine {
    #[inline(always)]
    pub fn process_tick(&mut self) -> f32 {
        let start_time = self.clock.now_ns();
        
        // Increment atomic counter (zero syscalls)
        self.counters.ticks_processed.fetch_add(1, Ordering::Relaxed);
        
        // Process axis pipeline (zero allocations)
        let output = self.pipeline.process(&mut self.frame);
        
        // Update timing counter
        let duration = self.clock.now_ns() - start_time;
        self.update_jitter_p99(duration);
        
        // Queue event for async processing (bounded, non-blocking)
        if let Ok(_) = self.event_queue.try_send(ObservabilityEvent::TickEnd {
            timestamp_ns: start_time,
            tick_id: self.tick_counter,
            duration_ns: duration as u32,
        }) {
            // Event queued successfully
        }
        // If queue full, drop event (never block RT)
        
        output
    }
    
    #[inline(always)]
    fn update_jitter_p99(&mut self, duration_ns: u64) {
        // Lock-free p99 approximation using atomic operations
        let expected_ns = 4_000_000; // 4ms for 250Hz
        let error_ns = duration_ns.abs_diff(expected_ns);
        
        // Exponential moving average for p99 approximation
        let current_p99 = self.counters.jitter_p99_ns.load(Ordering::Relaxed);
        let alpha = 0.01; // Smoothing factor
        let new_p99 = ((1.0 - alpha) * current_p99 as f64 + alpha * error_ns as f64) as u64;
        
        self.counters.jitter_p99_ns.store(new_p99, Ordering::Relaxed);
    }
}
```

### 4. Event Processing Pipeline

```rust
pub struct ObservabilityProcessor {
    event_receiver: Receiver<ObservabilityEvent>,
    blackbox_writer: BlackboxWriter,
    metrics_collector: MetricsCollector,
    tracing_provider: Option<TracingProvider>,
}

impl ObservabilityProcessor {
    pub async fn process_events(&mut self) {
        while let Ok(event) = self.event_receiver.recv().await {
            // Write to blackbox (binary format)
            self.blackbox_writer.write_event(&event).await;
            
            // Update metrics
            self.metrics_collector.update(&event);
            
            // Send to tracing system if enabled
            if let Some(ref mut tracer) = self.tracing_provider {
                tracer.emit_event(&event);
            }
            
            // Handle critical events immediately
            if let ObservabilityEvent::FaultDetected { .. } = event {
                self.handle_fault_event(&event).await;
            }
        }
    }
}
```

### 5. Blackbox Recording

**File Format:**
```
[Header: 64 bytes]
[Stream A: 250Hz axis frames]
[Stream B: 60Hz telemetry snapshots]  
[Stream C: Variable-rate events]
[Index: 100ms intervals]
[Footer: CRC32C checksum]
```

**Writer Implementation:**
```rust
pub struct BlackboxWriter {
    file: File,
    buffer: Vec<u8>,
    last_flush: Instant,
    index_entries: Vec<IndexEntry>,
}

impl BlackboxWriter {
    pub async fn write_axis_frame(&mut self, frame: &AxisFrame) -> Result<()> {
        // Serialize to buffer (no allocations)
        frame.serialize_into(&mut self.buffer)?;
        
        // Flush every 1s or when buffer full
        if self.buffer.len() > 8192 || self.last_flush.elapsed() > Duration::from_secs(1) {
            self.flush_buffer().await?;
        }
        
        Ok(())
    }
    
    async fn flush_buffer(&mut self) -> Result<()> {
        // Write buffer to file (async I/O)
        self.file.write_all(&self.buffer).await?;
        self.buffer.clear();
        
        // Add index entry every 100ms
        if self.should_add_index_entry() {
            self.index_entries.push(IndexEntry {
                timestamp_ns: self.clock.now_ns(),
                file_offset: self.file.stream_position().await?,
            });
        }
        
        self.last_flush = Instant::now();
        Ok(())
    }
}
```

## Consequences

### Positive
- Zero impact on real-time performance
- Comprehensive debugging information
- Efficient binary format for high-frequency data
- Flexible tracing integration

### Negative
- Complex multi-tier architecture
- Potential data loss under extreme load
- Storage requirements for continuous recording
- Additional complexity in error handling

## Alternatives Considered

1. **Traditional Logging**: Rejected due to RT impact
2. **Sampling-Only**: Rejected due to missing critical events
3. **External Monitoring**: Rejected due to integration complexity
4. **In-Memory Only**: Rejected due to data loss on crashes

## Implementation Details

### Platform-Specific Tracing

**Windows (ETW):**
```rust
impl EtwProvider {
    pub fn emit_tick_start(&self, tick_id: u64, timestamp: u64) {
        unsafe {
            EventWrite(
                self.handle,
                &TICK_START_DESCRIPTOR,
                &[
                    EventDataDescriptor::from_u64(tick_id),
                    EventDataDescriptor::from_u64(timestamp),
                ]
            );
        }
    }
}
```

**Linux (tracepoints):**
```rust
impl TracepointProvider {
    pub fn emit_tick_start(&self, tick_id: u64, timestamp: u64) {
        // Use kernel tracepoints via /sys/kernel/debug/tracing
        tracepoint!(flight_hub, tick_start, tick_id, timestamp);
    }
}
```

### Metrics Collection

```rust
pub struct MetricsCollector {
    pub jitter_histogram: Histogram,
    pub hid_latency_histogram: Histogram,
    pub fault_counters: HashMap<FaultType, Counter>,
    pub throughput_gauge: Gauge,
}

impl MetricsCollector {
    pub fn update(&mut self, event: &ObservabilityEvent) {
        match event {
            ObservabilityEvent::TickEnd { duration_ns, .. } => {
                self.jitter_histogram.observe(*duration_ns as f64 / 1_000_000.0);
            },
            ObservabilityEvent::HidWrite { latency_ns, .. } => {
                self.hid_latency_histogram.observe(*latency_ns as f64 / 1_000.0);
            },
            ObservabilityEvent::FaultDetected { fault_type, .. } => {
                self.fault_counters.entry(*fault_type).or_default().inc();
            },
            _ => {}
        }
    }
}
```

## Testing Strategy

### Unit Tests
- Counter atomicity and correctness
- Event serialization/deserialization
- Blackbox format validation
- Metrics calculation accuracy

### Integration Tests
- End-to-end event flow
- Blackbox replay functionality
- Performance impact measurement
- Data loss scenarios

### Performance Tests
- RT impact validation (must be zero)
- Throughput under load
- Memory usage patterns
- Storage efficiency

## Quality Gates

**RT Impact Gate:**
- No measurable jitter increase with observability enabled
- Counter operations must be lock-free
- Event queuing must never block

**Data Integrity Gate:**
- Blackbox files must be readable after crashes
- CRC validation must pass
- Index entries must be consistent

**Performance Gate:**
- Event processing must keep up with generation
- Memory usage must remain bounded
- Storage growth must be predictable

## User Interface

### Real-Time Dashboard
- Live jitter and latency graphs
- Fault counters and status
- Throughput indicators
- System health overview

### Historical Analysis
- Blackbox replay with timeline
- Performance trend analysis
- Fault correlation tools
- Export capabilities for external analysis

## Privacy and Security

- No PII in observability data by default
- Opt-in for detailed telemetry collection
- Local-only storage (no automatic upload)
- Redaction tools for support bundles

## References

- Flight Hub Requirements: DIAG-01, NFR-01
- [High-Performance Logging](https://example.com)
- [Real-Time Monitoring Patterns](https://example.com)
- [ETW Documentation](https://example.com)