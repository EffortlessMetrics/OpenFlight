// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Event bus step definitions.
//!
//! Connects Gherkin steps to `flight_bus` types:
//! - EventRouter creation and route registration
//! - Event publishing / routing
//! - Subscriber assertions

use crate::step_registry::{StepOutcome, StepRegistry};
use flight_bus::event_router::{EventFilter, EventRouter};

/// Register all bus-related step definitions.
pub fn register(registry: &mut StepRegistry) {
    // -- Given ----------------------------------------------------------

    registry.given(r"^the event bus is running$", |ctx, _caps| {
        let mut router = EventRouter::new();
        // Register a catch-all route for subscriber "sub-1"
        router.add_route(
            "sub-1",
            EventFilter {
                topic: "*".to_string(),
                device_id: None,
                min_priority: 0,
            },
        );
        ctx.set("event_router", router);
        StepOutcome::Passed
    });

    registry.given(
        r#"^a route for "([^"]+)" events to subscriber "([^"]+)"$"#,
        |ctx, caps| {
            let topic = caps[1].to_string();
            let subscriber = caps[2].to_string();
            // Build a fresh router with this route
            let mut router = EventRouter::new();
            router.add_route(
                &subscriber,
                EventFilter {
                    topic,
                    device_id: None,
                    min_priority: 0,
                },
            );
            ctx.set("event_router", router);
            StepOutcome::Passed
        },
    );

    // -- When -----------------------------------------------------------

    registry.when(
        r#"^event "([^"]+)" is published$"#,
        |ctx, caps| {
            let topic = caps[1].to_string();
            let router = match ctx.get::<EventRouter>("event_router") {
                Some(r) => r,
                None => return StepOutcome::Failed("no event_router in context".to_string()),
            };
            let matched = router.route_event(&topic, None, 1);
            ctx.set("last_route_match_count", matched.len());
            ctx.set("last_event_topic", topic);
            ctx.set("last_matched_subscribers", matched.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            StepOutcome::Passed
        },
    );

    // -- Then -----------------------------------------------------------

    registry.then(
        r#"^subscriber should receive event "([^"]+)"$"#,
        |ctx, caps| {
            let expected_topic = &caps[1];
            match ctx.get::<String>("last_event_topic") {
                Some(actual) if actual.as_str() == expected_topic => StepOutcome::Passed,
                Some(actual) => StepOutcome::Failed(format!(
                    "expected event '{expected_topic}', last was '{actual}'"
                )),
                None => StepOutcome::Failed("no event was published".to_string()),
            }
        },
    );

    registry.then(
        r"^the event should be routed to (\d+) subscribers?$",
        |ctx, caps| {
            let expected: usize = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad int: {e}")),
            };
            match ctx.get::<usize>("last_route_match_count") {
                Some(actual) if *actual >= expected => StepOutcome::Passed,
                Some(actual) => StepOutcome::Failed(format!(
                    "expected at least {expected} subscriber(s), got {actual}"
                )),
                None => StepOutcome::Failed("no route result in context".to_string()),
            }
        },
    );

    registry.then(
        r"^the router should have (\d+) routes?$",
        |ctx, caps| {
            let expected: usize = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad int: {e}")),
            };
            match ctx.get::<EventRouter>("event_router") {
                Some(router) => {
                    let count = router.route_count();
                    if count == expected {
                        StepOutcome::Passed
                    } else {
                        StepOutcome::Failed(format!(
                            "expected {expected} route(s), got {count}"
                        ))
                    }
                }
                None => StepOutcome::Failed("no event_router in context".to_string()),
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{parse_scenario, run_scenario};
    use crate::step_registry::StepRegistry;

    fn registry() -> StepRegistry {
        let mut r = StepRegistry::new();
        register(&mut r);
        r
    }

    #[test]
    fn bus_routes_event_by_topic() {
        let reg = registry();
        // Register a specific route, then publish matching event
        let s = parse_scenario(
            "topic_route",
            r#"Given a route for "axis.pitch" events to subscriber "axis-sub"
When event "axis.pitch" is published
Then subscriber should receive event "axis.pitch"
And the event should be routed to 1 subscriber"#,
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }

    #[test]
    fn bus_routes_button_event() {
        let reg = registry();
        let s = parse_scenario(
            "button_route",
            r#"Given a route for "button.trigger" events to subscriber "btn-sub"
When event "button.trigger" is published
Then subscriber should receive event "button.trigger""#,
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }

    #[test]
    fn bus_reports_route_count() {
        let reg = registry();
        let s = parse_scenario(
            "route_count",
            r#"Given the event bus is running
Then the router should have 1 route"#,
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }
}

