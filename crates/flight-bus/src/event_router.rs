// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Event routing and filtering for the telemetry bus.
//!
//! Routes events from publishers to subscribers based on topic and filter patterns.
//! Each subscriber registers one or more [`EventFilter`] entries, and the router
//! evaluates incoming events against all active filters to determine delivery targets.

/// Filter criteria for event routing.
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// Topic pattern to match against.
    pub topic: String,
    /// Optional device ID filter. When set, only events from this device match.
    pub device_id: Option<String>,
    /// Minimum priority threshold. Events below this priority are filtered out.
    pub min_priority: u8,
}

/// A routing table entry mapping a subscriber to its filter.
#[derive(Debug, Clone)]
pub struct RoutingEntry {
    /// Unique subscriber identifier.
    pub subscriber_id: String,
    /// Filter applied to incoming events.
    pub filter: EventFilter,
}

/// Routes events from publishers to subscribers based on topic/filter patterns.
pub struct EventRouter {
    routes: Vec<RoutingEntry>,
}

impl EventRouter {
    /// Creates a new empty event router.
    #[must_use]
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Adds a routing entry for the given subscriber.
    pub fn add_route(&mut self, subscriber_id: impl Into<String>, filter: EventFilter) {
        self.routes.push(RoutingEntry {
            subscriber_id: subscriber_id.into(),
            filter,
        });
    }

    /// Removes all routing entries for the given subscriber.
    pub fn remove_routes(&mut self, subscriber_id: &str) {
        self.routes.retain(|r| r.subscriber_id != subscriber_id);
    }

    /// Returns subscriber IDs whose filters match the given event parameters.
    #[must_use]
    pub fn route_event<'a>(
        &'a self,
        topic: &str,
        device_id: Option<&str>,
        priority: u8,
    ) -> Vec<&'a str> {
        self.routes
            .iter()
            .filter(|r| {
                let f = &r.filter;
                if f.topic != topic {
                    return false;
                }
                if priority < f.min_priority {
                    return false;
                }
                if let Some(ref filter_device) = f.device_id {
                    match device_id {
                        Some(event_device) if event_device == filter_device.as_str() => {}
                        _ => return false,
                    }
                }
                true
            })
            .map(|r| r.subscriber_id.as_str())
            .collect()
    }

    /// Returns the total number of routing entries.
    #[must_use]
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Returns subscriber IDs that have at least one route matching the given topic.
    #[must_use]
    pub fn subscribers_for_topic<'a>(&'a self, topic: &str) -> Vec<&'a str> {
        self.routes
            .iter()
            .filter(|r| r.filter.topic == topic)
            .map(|r| r.subscriber_id.as_str())
            .collect()
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_filter(topic: &str, device_id: Option<&str>, min_priority: u8) -> EventFilter {
        EventFilter {
            topic: topic.to_string(),
            device_id: device_id.map(String::from),
            min_priority,
        }
    }

    #[test]
    fn test_new_router_empty() {
        let router = EventRouter::new();
        assert_eq!(router.route_count(), 0);
    }

    #[test]
    fn test_add_route() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        assert_eq!(router.route_count(), 1);
    }

    #[test]
    fn test_remove_routes() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        router.add_route("sub1", make_filter("controls", None, 0));
        router.add_route("sub2", make_filter("telemetry", None, 0));
        router.remove_routes("sub1");
        assert_eq!(router.route_count(), 1);
    }

    #[test]
    fn test_route_event_matching_topic() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        let subs = router.route_event("telemetry", None, 5);
        assert_eq!(subs, vec!["sub1"]);
    }

    #[test]
    fn test_route_event_no_match() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        let subs = router.route_event("controls", None, 5);
        assert!(subs.is_empty());
    }

    #[test]
    fn test_route_event_with_device_filter_match() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", Some("dev-1"), 0));
        let subs = router.route_event("telemetry", Some("dev-1"), 5);
        assert_eq!(subs, vec!["sub1"]);
    }

    #[test]
    fn test_route_event_with_device_filter_no_match() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", Some("dev-1"), 0));
        let subs = router.route_event("telemetry", Some("dev-2"), 5);
        assert!(subs.is_empty());
    }

    #[test]
    fn test_route_event_device_filter_event_has_no_device() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", Some("dev-1"), 0));
        let subs = router.route_event("telemetry", None, 5);
        assert!(subs.is_empty());
    }

    #[test]
    fn test_route_event_no_device_filter_accepts_any() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        let subs = router.route_event("telemetry", Some("dev-99"), 5);
        assert_eq!(subs, vec!["sub1"]);
    }

    #[test]
    fn test_route_event_priority_below_threshold() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 5));
        assert!(router.route_event("telemetry", None, 3).is_empty());
    }

    #[test]
    fn test_route_event_priority_at_threshold() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 5));
        assert_eq!(router.route_event("telemetry", None, 5), vec!["sub1"]);
    }

    #[test]
    fn test_route_event_priority_above_threshold() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 5));
        assert_eq!(router.route_event("telemetry", None, 10), vec!["sub1"]);
    }

    #[test]
    fn test_subscribers_for_topic() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        router.add_route("sub2", make_filter("telemetry", None, 5));
        router.add_route("sub3", make_filter("controls", None, 0));
        let subs = router.subscribers_for_topic("telemetry");
        assert_eq!(subs.len(), 2);
        assert!(subs.contains(&"sub1"));
        assert!(subs.contains(&"sub2"));
    }

    #[test]
    fn test_multiple_routes_same_subscriber() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        router.add_route("sub1", make_filter("controls", None, 0));
        assert_eq!(router.route_count(), 2);
        assert_eq!(router.route_event("telemetry", None, 0), vec!["sub1"]);
        assert_eq!(router.route_event("controls", None, 0), vec!["sub1"]);
    }

    #[test]
    fn test_remove_nonexistent_subscriber() {
        let mut router = EventRouter::new();
        router.add_route("sub1", make_filter("telemetry", None, 0));
        router.remove_routes("sub99");
        assert_eq!(router.route_count(), 1);
    }

    #[test]
    fn test_default_is_empty() {
        let router = EventRouter::default();
        assert_eq!(router.route_count(), 0);
    }

    // ── Property-based tests ──────────────────────────────────────────────

    use proptest::prelude::*;

    proptest! {
        /// Publishing (routing) to no subscribers never panics.
        #[test]
        fn prop_route_event_no_subscribers_never_panics(
            topic in "[a-z]{1,8}",
            priority in 0u8..=255u8,
        ) {
            let router = EventRouter::new();
            let _ = router.route_event(&topic, None, priority);
        }

        /// Adding a route and routing to it always returns the subscriber.
        #[test]
        fn prop_add_route_then_find(
            topic in "[a-z]{1,8}",
            sub_id in "[a-z]{1,8}",
            priority in 0u8..=100u8,
        ) {
            let mut router = EventRouter::new();
            router.add_route(
                sub_id.clone(),
                EventFilter {
                    topic: topic.clone(),
                    device_id: None,
                    min_priority: 0,
                },
            );
            let matched = router.route_event(&topic, None, priority);
            prop_assert!(
                matched.contains(&sub_id.as_str()),
                "subscriber {} not found for topic {}",
                sub_id, topic
            );
        }

        /// Subscribing to same topic twice doesn't lose messages:
        /// both routes are found when routing.
        #[test]
        fn prop_duplicate_subscription_both_found(
            topic in "[a-z]{1,8}",
            priority in 0u8..=255u8,
        ) {
            let mut router = EventRouter::new();
            router.add_route(
                "sub1",
                EventFilter { topic: topic.clone(), device_id: None, min_priority: 0 },
            );
            router.add_route(
                "sub1",
                EventFilter { topic: topic.clone(), device_id: None, min_priority: 0 },
            );
            let matched = router.route_event(&topic, None, priority);
            // sub1 appears twice because it has two routes
            prop_assert!(
                matched.len() >= 2,
                "expected at least 2 matches for duplicate subscription, got {}",
                matched.len()
            );
        }

        /// Event ordering is preserved: subscribers added first appear first in results.
        #[test]
        fn prop_routing_preserves_insertion_order(
            topic in "[a-z]{1,8}",
            n in 1usize..=10usize,
        ) {
            let mut router = EventRouter::new();
            let sub_ids: Vec<String> = (0..n).map(|i| format!("sub{}", i)).collect();
            for id in &sub_ids {
                router.add_route(
                    id.as_str(),
                    EventFilter { topic: topic.clone(), device_id: None, min_priority: 0 },
                );
            }
            let matched = router.route_event(&topic, None, 5);
            for (i, id) in sub_ids.iter().enumerate() {
                prop_assert_eq!(
                    matched[i], id.as_str(),
                    "ordering violated at index {}", i
                );
            }
        }

        /// remove_routes + route_event never panics regardless of input.
        #[test]
        fn prop_remove_then_route_never_panics(
            topic in "[a-z]{1,8}",
            sub_id in "[a-z]{1,8}",
        ) {
            let mut router = EventRouter::new();
            router.add_route(
                sub_id.as_str(),
                EventFilter { topic: topic.clone(), device_id: None, min_priority: 0 },
            );
            router.remove_routes(&sub_id);
            let matched = router.route_event(&topic, None, 0);
            prop_assert!(matched.is_empty(), "should be empty after removing all routes");
        }
    }
}
