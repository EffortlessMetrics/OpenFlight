//! Performance benchmarks for flight-bus event routing.
//!
//! Validates that the allocation-free event router meets RT latency requirements.

use criterion::{Criterion, criterion_group, criterion_main};
use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType,
};

fn bench_bus_event_creation(c: &mut Criterion) {
    c.bench_function("bus_event_creation", |b| {
        b.iter(|| {
            let event = BusEvent::new(
                std::hint::black_box(SourceType::Device),
                std::hint::black_box(1),
                std::hint::black_box(EventKind::AxisUpdate),
                std::hint::black_box(EventPriority::Normal),
                std::hint::black_box(1_000_000),
                std::hint::black_box(EventPayload::Axis {
                    axis_id: 0,
                    value: 0.5,
                }),
            );
            std::hint::black_box(event)
        })
    });
}

fn bench_event_router_register_route(c: &mut Criterion) {
    c.bench_function("event_router_register_route", |b| {
        b.iter(|| {
            let mut router = EventRouter::new();
            let pattern = RoutePattern::any();
            let filter = EventFilter::pass_all();
            let id = router.register_route(
                std::hint::black_box(pattern),
                std::hint::black_box(filter),
                std::hint::black_box(1),
            );
            std::hint::black_box(id)
        })
    });
}

fn bench_event_router_route_event_8_routes(c: &mut Criterion) {
    let mut router = EventRouter::new();
    for i in 0..8 {
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), i);
    }

    c.bench_function("event_router_route_event_8_routes", |b| {
        b.iter(|| {
            let event = BusEvent::new(
                SourceType::Device,
                1,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                std::hint::black_box(1_000_000),
                EventPayload::Axis {
                    axis_id: 0,
                    value: 0.5,
                },
            );
            let matches = router.route_event(&event);
            std::hint::black_box(matches.len())
        })
    });
}

fn bench_event_router_route_event_64_routes(c: &mut Criterion) {
    let mut router = EventRouter::new();
    for i in 0..64 {
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), i);
    }

    c.bench_function("event_router_route_event_64_routes", |b| {
        b.iter(|| {
            let event = BusEvent::new(
                SourceType::Device,
                1,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                std::hint::black_box(1_000_000),
                EventPayload::Axis {
                    axis_id: 0,
                    value: 0.5,
                },
            );
            let matches = router.route_event(&event);
            std::hint::black_box(matches.len())
        })
    });
}

criterion_group!(
    benches,
    bench_bus_event_creation,
    bench_event_router_register_route,
    bench_event_router_route_event_8_routes,
    bench_event_router_route_event_64_routes,
);

criterion_main!(benches);
