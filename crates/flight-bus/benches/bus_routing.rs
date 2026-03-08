//! Performance benchmarks for flight-bus event routing.
//!
//! Validates that the allocation-free event router meets RT latency requirements
//! and sustains 250Hz+ throughput.

use criterion::{Criterion, criterion_group, criterion_main};
use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType, Topic,
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

fn bench_topic_filtered_routing(c: &mut Criterion) {
    let mut router = EventRouter::new();
    // 4 topic-specific routes + 4 wildcard routes.
    router.register_route(
        RoutePattern::for_topic(Topic::Commands),
        EventFilter::pass_all(),
        1,
    );
    router.register_route(
        RoutePattern::for_topic(Topic::Telemetry),
        EventFilter::pass_all(),
        2,
    );
    router.register_route(
        RoutePattern::for_topic(Topic::Lifecycle),
        EventFilter::pass_all(),
        3,
    );
    router.register_route(
        RoutePattern::for_topic(Topic::Diagnostics),
        EventFilter::pass_all(),
        4,
    );
    for i in 5..9 {
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), i);
    }

    c.bench_function("topic_filtered_routing_8_routes", |b| {
        let event = BusEvent::new(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Normal,
            1_000_000,
            EventPayload::Axis {
                axis_id: 0,
                value: 0.5,
            },
        );
        b.iter(|| {
            let matches = router.route_event(std::hint::black_box(&event));
            std::hint::black_box(matches.len())
        })
    });
}

/// Measures throughput: how many events/second can be routed through a typical
/// configuration (8 routes with mixed topics and filters).
fn bench_throughput_250hz(c: &mut Criterion) {
    let mut router = EventRouter::new();
    router.register_route(
        RoutePattern::for_topic(Topic::Commands),
        EventFilter::pass_all(),
        1,
    );
    router.register_route(
        RoutePattern::for_topic(Topic::Telemetry),
        EventFilter::pass_all(),
        2,
    );
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 3);
    router.register_route(
        RoutePattern::for_topic(Topic::Commands),
        EventFilter {
            min_value: Some(-0.5),
            max_value: Some(0.5),
            ..EventFilter::pass_all()
        },
        4,
    );

    let mut group = c.benchmark_group("throughput");
    group.throughput(criterion::Throughput::Elements(1));

    group.bench_function("route_event_typical_4_routes", |b| {
        let mut ts = 0u64;
        b.iter(|| {
            ts += 4_000; // 4ms = 250Hz
            let event = BusEvent::new(
                SourceType::Device,
                1,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                std::hint::black_box(ts),
                EventPayload::Axis {
                    axis_id: 0,
                    value: 0.5,
                },
            );
            let matches = router.route_event(&event);
            std::hint::black_box(matches.len())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_bus_event_creation,
    bench_event_router_register_route,
    bench_event_router_route_event_8_routes,
    bench_event_router_route_event_64_routes,
    bench_topic_filtered_routing,
    bench_throughput_250hz,
);

criterion_main!(benches);
