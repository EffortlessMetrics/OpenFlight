---
doc_id: DOC-HOWTO-BENCHMARKS
kind: how-to
area: ci
status: active
links:
  requirements: []
  tasks: []
  adrs: []
---

# How to Run Benchmarks

This guide explains how to run performance benchmarks in the Flight Hub project.

## Overview

Flight Hub uses the Criterion benchmarking framework for performance testing. Benchmarks are located in the `benches/` directory of each crate.

## Quick Start

### Run All Benchmarks

```bash
cargo bench
```

This will run all benchmarks across all crates and generate HTML reports.

### Run Benchmarks for a Specific Crate

```bash
cargo bench -p flight-axis
```

### Run a Specific Benchmark

```bash
cargo bench --bench axis_performance
```

## Benchmark Categories

### Axis Processing Benchmarks

Located in `crates/flight-axis/benches/`:

```bash
# Axis processing performance
cargo bench -p flight-axis --bench axis_performance

# Detent processing performance
cargo bench -p flight-axis --bench detent_performance
```

These benchmarks measure:
- Axis transformation latency
- Detent calculation overhead
- Pipeline compilation time

### IPC Benchmarks

Located in `crates/flight-ipc/benches/`:

```bash
cargo bench -p flight-ipc --bench ipc_benchmarks
```

These benchmarks measure:
- RPC round-trip latency
- Streaming throughput
- Connection establishment time

### Replay Benchmarks

Located in `crates/flight-replay/benches/`:

```bash
cargo bench -p flight-replay --bench replay_performance
```

These benchmarks measure:
- Replay engine throughput
- Comparison algorithm performance
- Metric calculation overhead

### Updater Benchmarks

Located in `crates/flight-updater/benches/`:

```bash
cargo bench -p flight-updater --bench delta_apply
```

These benchmarks measure:
- Delta patch application speed
- Signature verification time
- Archive extraction performance

## Interpreting Results

### Console Output

Criterion displays results in the console:

```
axis_transform/simple    time:   [2.1234 µs 2.1456 µs 2.1678 µs]
                         change: [-2.34% -1.23% +0.12%] (p = 0.23 > 0.05)
                         No change in performance detected.
```

- **time**: The measured time with confidence interval
- **change**: Performance change compared to previous run
- **p-value**: Statistical significance (< 0.05 indicates significant change)

### HTML Reports

Detailed reports are generated in `target/criterion/`:

```bash
# Open the report in your browser
open target/criterion/report/index.html
```

The HTML reports include:
- Performance graphs over time
- Distribution plots
- Detailed statistics

## Performance Gates

### CI Performance Validation

The CI pipeline runs benchmarks and validates against performance gates:

```bash
cargo run --example ci_perf_gate
```

This ensures that performance doesn't regress below acceptable thresholds.

### Setting Performance Baselines

To establish a new baseline:

```bash
# Run benchmarks and save as baseline
cargo bench -- --save-baseline main

# Compare against baseline
cargo bench -- --baseline main
```

## Best Practices

### 1. Stable Environment

For accurate results:
- Close unnecessary applications
- Disable CPU frequency scaling (if possible)
- Run on a quiet system with minimal background activity

### 2. Multiple Iterations

Criterion automatically runs multiple iterations. For more stable results:

```bash
# Increase sample size
cargo bench -- --sample-size 1000
```

### 3. Warm-up Period

Criterion includes a warm-up period by default. For JIT-heavy code, increase it:

```bash
# Increase warm-up time
cargo bench -- --warm-up-time 5
```

## Writing New Benchmarks

### Basic Benchmark Structure

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_function(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| {
            // Code to benchmark
            my_function(black_box(42))
        });
    });
}

criterion_group!(benches, benchmark_function);
criterion_main!(benches);
```

### Parameterized Benchmarks

```rust
fn benchmark_with_params(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_group");
    
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter(|| my_function(black_box(size)));
            },
        );
    }
    
    group.finish();
}
```

## Troubleshooting

### Benchmarks Take Too Long

Reduce the sample size or measurement time:

```bash
cargo bench -- --sample-size 10 --measurement-time 1
```

### Inconsistent Results

- Ensure CPU frequency scaling is disabled
- Close background applications
- Run multiple times and compare

### Out of Memory

Some benchmarks may require significant memory. Increase available memory or reduce benchmark parameters.

## Continuous Integration

Benchmarks run in CI as part of the validation pipeline:

```bash
cargo xtask validate
```

This includes:
- Running all benchmarks
- Comparing against baselines
- Validating performance gates

## Related Documentation

- [How to Run Tests](./run-tests.md)
- [CI Configuration](../../infra/ci/README.md)
- [Performance Validation](../../docs/validation_report.md)

