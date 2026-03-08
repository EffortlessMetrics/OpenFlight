// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Performance benchmarks for profile migration.

use criterion::{Criterion, criterion_group, criterion_main};
use flight_profile::profile_migration::MigrationRegistry;
use serde_json::{Value, json};

fn make_v1_profile(axis_count: usize) -> Value {
    let mut axes = serde_json::Map::new();
    for i in 0..axis_count {
        let mut axis = serde_json::Map::new();
        axis.insert("deadzone".to_string(), json!(0.01 + (i as f64) * 0.005));
        axis.insert("expo".to_string(), json!(0.1 + (i as f64) * 0.02));
        axis.insert("slew_rate".to_string(), json!(1.0 + (i as f64) * 0.1));
        axes.insert(format!("axis_{i}"), Value::Object(axis));
    }
    json!({
        "schema_version": "v1",
        "sim": "msfs",
        "aircraft": { "icao": "C172" },
        "axes": Value::Object(axes)
    })
}

fn bench_migrate_v1_to_v2_small(c: &mut Criterion) {
    let reg = MigrationRegistry::new();
    let profile = make_v1_profile(4);

    c.bench_function("migrate_v1_to_v2_4axes", |b| {
        b.iter(|| {
            let result = reg
                .migrate(std::hint::black_box(profile.clone()), "v1", "v2")
                .unwrap();
            std::hint::black_box(result)
        })
    });
}

fn bench_migrate_v1_to_v3_small(c: &mut Criterion) {
    let reg = MigrationRegistry::new();
    let profile = make_v1_profile(4);

    c.bench_function("migrate_v1_to_v3_4axes", |b| {
        b.iter(|| {
            let result = reg
                .migrate(std::hint::black_box(profile.clone()), "v1", "v3")
                .unwrap();
            std::hint::black_box(result)
        })
    });
}

fn bench_migrate_v1_to_v3_large(c: &mut Criterion) {
    let reg = MigrationRegistry::new();
    let profile = make_v1_profile(50);

    c.bench_function("migrate_v1_to_v3_50axes", |b| {
        b.iter(|| {
            let result = reg
                .migrate(std::hint::black_box(profile.clone()), "v1", "v3")
                .unwrap();
            std::hint::black_box(result)
        })
    });
}

fn bench_migrate_noop_same_version(c: &mut Criterion) {
    let reg = MigrationRegistry::new();
    let profile = make_v1_profile(4);

    c.bench_function("migrate_noop_same_version", |b| {
        b.iter(|| {
            let result = reg
                .migrate(std::hint::black_box(profile.clone()), "v1", "v1")
                .unwrap();
            std::hint::black_box(result)
        })
    });
}

criterion_group!(
    migration_benches,
    bench_migrate_v1_to_v2_small,
    bench_migrate_v1_to_v3_small,
    bench_migrate_v1_to_v3_large,
    bench_migrate_noop_same_version,
);

criterion_main!(migration_benches);
