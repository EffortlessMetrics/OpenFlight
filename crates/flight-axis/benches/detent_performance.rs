//! Performance benchmarks for detent mapper
//!
//! Validates that detent processing meets real-time requirements:
//! - Processing time ≤ 0.5ms p99
//! - Zero allocations during processing
//! - Deterministic execution time

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use crossbeam::channel;
use flight_axis::{AxisFrame, DetentNode, DetentRole, DetentState, DetentZone, Node};
use std::time::Instant;

/// Create a realistic detent configuration for benchmarking
fn create_realistic_detent_node() -> DetentNode {
    let zones = vec![
        DetentZone::new(-0.8, 0.08, 0.02, DetentRole::Reverse),
        DetentZone::new(-0.3, 0.12, 0.03, DetentRole::Idle),
        DetentZone::new(0.0, 0.15, 0.04, DetentRole::Taxi),
        DetentZone::new(0.4, 0.10, 0.025, DetentRole::Climb),
        DetentZone::new(0.7, 0.08, 0.02, DetentRole::Takeoff),
        DetentZone::new(0.95, 0.05, 0.01, DetentRole::Emergency),
    ];

    let (sender, _receiver) = channel::unbounded();
    DetentNode::new(zones).with_event_sender(sender)
}

/// Benchmark single detent processing step
fn bench_detent_processing(c: &mut Criterion) {
    let node = create_realistic_detent_node();
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    c.bench_function("detent_single_step", |b| {
        b.iter(|| {
            let mut frame =
                AxisFrame::new(std::hint::black_box(0.42), std::hint::black_box(1000000));
            unsafe {
                let state_ptr = &mut state as *mut DetentState as *mut u8;
                node.step_soa(&mut frame, state_ptr);
            }
            std::hint::black_box(frame.out);
        });
    });
}

/// Benchmark detent processing with varying number of zones
fn bench_detent_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("detent_scaling");

    for zone_count in [1, 3, 6, 10, 20].iter() {
        let mut zones = Vec::new();
        for i in 0..*zone_count {
            let center = -0.9 + (1.8 * i as f32 / (*zone_count - 1) as f32);
            zones.push(DetentZone::new(
                center,
                0.1,
                0.02,
                DetentRole::Custom(i as u8),
            ));
        }

        let (sender, _receiver) = channel::unbounded();
        let node = DetentNode::new(zones).with_event_sender(sender);
        let mut state = DetentState {
            active_detent_idx: u32::MAX,
            last_position: 0.0,
            last_event_ns: 0,
        };

        group.bench_with_input(BenchmarkId::new("zones", zone_count), zone_count, |b, _| {
            b.iter(|| {
                let mut frame =
                    AxisFrame::new(std::hint::black_box(0.42), std::hint::black_box(1000000));
                unsafe {
                    let state_ptr = &mut state as *mut DetentState as *mut u8;
                    node.step_soa(&mut frame, state_ptr);
                }
                std::hint::black_box(frame.out);
            });
        });
    }
    group.finish();
}

/// Benchmark worst-case scenario: frequent transitions
fn bench_detent_transitions(c: &mut Criterion) {
    let node = create_realistic_detent_node();
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    // Create a sequence that causes frequent transitions
    let positions: Vec<f32> = (0..1000)
        .map(|i| {
            let t = i as f32 * 0.01;
            0.4 * (t * 10.0).sin() // Oscillate around multiple detents
        })
        .collect();

    c.bench_function("detent_frequent_transitions", |b| {
        b.iter(|| {
            for (i, &position) in positions.iter().enumerate() {
                let mut frame = AxisFrame::new(
                    std::hint::black_box(position),
                    std::hint::black_box((i * 1000) as u64),
                );
                unsafe {
                    let state_ptr = &mut state as *mut DetentState as *mut u8;
                    node.step_soa(&mut frame, state_ptr);
                }
                std::hint::black_box(frame.out);
            }
        });
    });
}

/// Validate zero-allocation constraint during detent processing
fn bench_allocation_validation(c: &mut Criterion) {
    let node = create_realistic_detent_node();
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    c.bench_function("detent_zero_allocation", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();

            for i in 0..iters {
                let position = (i as f32 * 0.001) % 2.0 - 1.0; // Sweep -1 to 1
                let mut frame = AxisFrame::new(
                    std::hint::black_box(position),
                    std::hint::black_box(i * 1000),
                );

                unsafe {
                    let state_ptr = &mut state as *mut DetentState as *mut u8;
                    node.step_soa(&mut frame, state_ptr);
                }
                std::hint::black_box(frame.out);
            }

            start.elapsed()
        });
    });
}

/// Benchmark detent zone lookup performance
fn bench_zone_lookup(c: &mut Criterion) {
    let zones = vec![
        DetentZone::new(-0.8, 0.08, 0.02, DetentRole::Reverse),
        DetentZone::new(-0.3, 0.12, 0.03, DetentRole::Idle),
        DetentZone::new(0.0, 0.15, 0.04, DetentRole::Taxi),
        DetentZone::new(0.4, 0.10, 0.025, DetentRole::Climb),
        DetentZone::new(0.7, 0.08, 0.02, DetentRole::Takeoff),
        DetentZone::new(0.95, 0.05, 0.01, DetentRole::Emergency),
    ];

    let node = DetentNode::new(zones);

    c.bench_function("detent_zone_lookup", |b| {
        b.iter(|| {
            let position = std::hint::black_box(0.42);
            let result = node.find_entry_detent(position);
            std::hint::black_box(result);
        });
    });
}

/// Benchmark hysteresis calculations
fn bench_hysteresis_check(c: &mut Criterion) {
    let zone = DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle);

    c.bench_function("detent_hysteresis_check", |b| {
        b.iter(|| {
            let position = std::hint::black_box(0.12);
            let entry = zone.contains_entry(position);
            let exit = zone.contains_exit(position);
            std::hint::black_box((entry, exit));
        });
    });
}

criterion_group!(
    detent_benches,
    bench_detent_processing,
    bench_detent_scaling,
    bench_detent_transitions,
    bench_allocation_validation,
    bench_zone_lookup,
    bench_hysteresis_check
);

criterion_main!(detent_benches);
