//! Performance benchmarks for flight-axis crate
//!
//! Validates that the axis processing meets the strict timing requirements
//! specified in the requirements (≤0.5ms p99 processing time).

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use flight_axis::{
    AxisEngine, AxisFrame, PipelineBuilder, DeadzoneNode, CurveNode, Node
};
use std::time::Instant;

fn bench_axis_frame_creation(c: &mut Criterion) {
    c.bench_function("axis_frame_creation", |b| {
        b.iter(|| {
            let frame = AxisFrame::new(black_box(0.5), black_box(1000000));
            black_box(frame)
        })
    });
}

fn bench_deadzone_processing(c: &mut Criterion) {
    let mut node = DeadzoneNode::new(0.1);
    
    c.bench_function("deadzone_processing", |b| {
        b.iter(|| {
            let mut frame = AxisFrame::new(black_box(0.5), black_box(1000000));
            node.step(&mut frame);
            black_box(frame.out)
        })
    });
}

fn bench_curve_processing(c: &mut Criterion) {
    let mut node = CurveNode::new(0.2);
    
    c.bench_function("curve_processing", |b| {
        b.iter(|| {
            let mut frame = AxisFrame::new(black_box(0.5), black_box(1000000));
            node.step(&mut frame);
            black_box(frame.out)
        })
    });
}

fn bench_engine_processing(c: &mut Criterion) {
    let engine = AxisEngine::new();
    
    c.bench_function("engine_processing_no_pipeline", |b| {
        b.iter(|| {
            let mut frame = AxisFrame::new(black_box(0.5), black_box(1000000));
            let result = engine.process(&mut frame);
            black_box((result, frame.out))
        })
    });
}

fn bench_pipeline_compilation(c: &mut Criterion) {
    c.bench_function("pipeline_compilation", |b| {
        b.iter(|| {
            let pipeline = PipelineBuilder::new()
                .deadzone(black_box(0.05))
                .curve(black_box(0.2)).unwrap()
                .slew(black_box(1.5))
                .compile();
            black_box(pipeline)
        })
    });
}

fn bench_multi_node_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_node_pipeline");
    
    for node_count in [1, 3, 5].iter() {
        group.bench_with_input(
            BenchmarkId::new("nodes", node_count),
            node_count,
            |b, &node_count| {
                let mut deadzone_nodes = Vec::new();
                let mut curve_nodes = Vec::new();
                
                for i in 0..node_count {
                    if i % 2 == 0 {
                        deadzone_nodes.push(DeadzoneNode::new(0.05));
                    } else {
                        curve_nodes.push(CurveNode::new(0.2));
                    }
                }
                
                b.iter(|| {
                    let mut frame = AxisFrame::new(black_box(0.5), black_box(1000000));
                    
                    for node in &mut deadzone_nodes {
                        node.step(&mut frame);
                    }
                    
                    for node in &mut curve_nodes {
                        node.step(&mut frame);
                    }
                    
                    black_box(frame.out)
                })
            },
        );
    }
    group.finish();
}

fn bench_250hz_simulation(c: &mut Criterion) {
    let engine = AxisEngine::new();
    
    c.bench_function("250hz_simulation_1000_frames", |b| {
        b.iter(|| {
            let start_time = 1000000000u64; // 1 second in nanoseconds
            let frame_interval = 4000000u64; // 4ms = 250Hz
            
            for i in 0..1000 {
                let mut frame = AxisFrame::new(
                    black_box((i as f32) / 1000.0),
                    black_box(start_time + i * frame_interval)
                );
                
                let _result = engine.process(&mut frame);
                black_box(frame.out);
            }
        })
    });
}

fn bench_rt_timing_validation(c: &mut Criterion) {
    let engine = AxisEngine::new();
    
    c.bench_function("rt_timing_validation", |b| {
        b.iter_custom(|iters| {
            let mut total_time = std::time::Duration::ZERO;
            
            for i in 0..iters {
                let mut frame = AxisFrame::new(
                    black_box((i as f32) / (iters as f32)),
                    black_box(1000000 + i * 4000) // 250Hz intervals
                );
                
                let start = Instant::now();
                let _result = engine.process(&mut frame);
                let elapsed = start.elapsed();
                
                total_time += elapsed;
                black_box(frame.out);
                
                // Validate RT constraint: each frame should be < 500μs
                assert!(elapsed.as_micros() < 500, 
                       "Frame processing took {}μs, exceeds 500μs limit", 
                       elapsed.as_micros());
            }
            
            total_time
        })
    });
}

criterion_group!(
    benches,
    bench_axis_frame_creation,
    bench_deadzone_processing,
    bench_curve_processing,
    bench_engine_processing,
    bench_pipeline_compilation,
    bench_multi_node_pipeline,
    bench_250hz_simulation,
    bench_rt_timing_validation
);

criterion_main!(benches);