//! Performance benchmarks for flight-profile merge and canonicalization.

use criterion::{Criterion, criterion_group, criterion_main};
use flight_profile::{AircraftId, AxisConfig, PofOverrides, Profile};
use std::collections::HashMap;

fn make_profile(axis_count: usize) -> Profile {
    let mut axes = HashMap::new();
    for i in 0..axis_count {
        let name = format!("axis_{i}");
        axes.insert(
            name,
            AxisConfig {
                deadzone: Some(0.03 + (i as f32) * 0.01),
                expo: Some(0.2),
                slew_rate: Some(1.2),
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
    }
    Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}

fn make_profile_with_pof(axis_count: usize) -> Profile {
    let mut p = make_profile(axis_count);
    let mut pof = HashMap::new();
    let mut takeoff_axes = HashMap::new();
    takeoff_axes.insert(
        "axis_0".to_string(),
        AxisConfig {
            deadzone: Some(0.01),
            expo: Some(0.3),
            slew_rate: Some(2.0),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    pof.insert(
        "takeoff".to_string(),
        PofOverrides {
            axes: Some(takeoff_axes),
            hysteresis: None,
        },
    );
    let mut cruise_axes = HashMap::new();
    cruise_axes.insert(
        "axis_1".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.1),
            slew_rate: Some(0.8),
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    pof.insert(
        "cruise".to_string(),
        PofOverrides {
            axes: Some(cruise_axes),
            hysteresis: None,
        },
    );
    p.pof_overrides = Some(pof);
    p
}

fn bench_profile_merge_with_simple(c: &mut Criterion) {
    let base = make_profile(2);
    let overlay = make_profile(2);

    c.bench_function("profile_merge_with_simple", |b| {
        b.iter(|| {
            let result = std::hint::black_box(&base)
                .merge_with(std::hint::black_box(&overlay))
                .unwrap();
            std::hint::black_box(result)
        })
    });
}

fn bench_profile_merge_with_complex(c: &mut Criterion) {
    let base = make_profile_with_pof(8);
    let overlay = make_profile_with_pof(8);

    c.bench_function("profile_merge_with_complex", |b| {
        b.iter(|| {
            let result = std::hint::black_box(&base)
                .merge_with(std::hint::black_box(&overlay))
                .unwrap();
            std::hint::black_box(result)
        })
    });
}

fn bench_profile_canonicalize(c: &mut Criterion) {
    let profile = make_profile(4);

    c.bench_function("profile_canonicalize", |b| {
        b.iter(|| {
            let canonical = std::hint::black_box(&profile).canonicalize();
            std::hint::black_box(canonical)
        })
    });
}

fn bench_profile_effective_hash(c: &mut Criterion) {
    let profile = make_profile(4);

    c.bench_function("profile_effective_hash", |b| {
        b.iter(|| {
            let hash = std::hint::black_box(&profile).effective_hash();
            std::hint::black_box(hash)
        })
    });
}

criterion_group!(
    benches,
    bench_profile_merge_with_simple,
    bench_profile_merge_with_complex,
    bench_profile_canonicalize,
    bench_profile_effective_hash,
);

criterion_main!(benches);
