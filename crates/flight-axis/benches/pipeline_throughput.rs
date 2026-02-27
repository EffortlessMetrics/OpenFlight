//! Throughput benchmark for the 8-axis RT tick
//!
//! Validates that processing all 8 axes in a single 250Hz tick fits within
//! the 4ms (4000μs) RT budget defined in ADR-001.

use criterion::{Criterion, criterion_group, criterion_main};
use flight_axis::{AxisEngine, AxisFrame, PipelineBuilder};
use std::time::Instant;

/// Build an axis engine with a realistic 3-node pipeline and prime the swap.
fn setup_engine(name: &str) -> AxisEngine {
    let engine = AxisEngine::new_for_axis(name.to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .expect("valid curve sensitivity")
        .slew(1.5)
        .compile()
        .expect("pipeline compiles");
    engine.update_pipeline(pipeline);
    // One dummy tick to activate the pending swap.
    let mut frame = AxisFrame::new(0.0, 0);
    let _ = engine.process(&mut frame);
    engine
}

/// Benchmark a single 250Hz tick across 8 axes.
fn bench_single_tick_8_axes(c: &mut Criterion) {
    let engines: Vec<AxisEngine> = (0..8).map(|i| setup_engine(&format!("axis_{i}"))).collect();

    c.bench_function("axis_engine_single_tick_8_axes", |b| {
        let mut ts: u64 = 1_000_000_000;
        b.iter(|| {
            ts += 4_000_000; // advance 4ms = 250Hz
            for (i, engine) in engines.iter().enumerate() {
                let input = std::hint::black_box((i as f32 + 1.0) * 0.1);
                let mut frame = AxisFrame::new(input, std::hint::black_box(ts));
                let _ = engine.process(&mut frame);
                std::hint::black_box(frame.out);
            }
        });
    });
}

/// Measure wall-clock cost of a full 8-axis tick to validate 4ms RT budget.
fn bench_full_250hz_budget(c: &mut Criterion) {
    let engines: Vec<AxisEngine> = (0..8)
        .map(|i| setup_engine(&format!("budget_{i}")))
        .collect();

    c.bench_function("full_250hz_tick_budget_8_axes", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for i in 0..iters {
                let ts = std::hint::black_box(1_000_000_000u64 + i * 4_000_000);
                let tick_start = Instant::now();
                for (j, engine) in engines.iter().enumerate() {
                    let input = std::hint::black_box((j as f32 + 1.0) * 0.1);
                    let mut frame = AxisFrame::new(input, ts);
                    let _ = engine.process(&mut frame);
                    std::hint::black_box(frame.out);
                }
                total += tick_start.elapsed();
            }
            total
        });
    });
}

criterion_group!(benches, bench_single_tick_8_axes, bench_full_250hz_budget);
criterion_main!(benches);
