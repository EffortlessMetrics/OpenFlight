// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Subscription manager for IPC event topics.
//!
//! Clients subscribe to one or more named topics (e.g. `"device"`,
//! `"telemetry"`).  The [`SubscriptionManager`] tracks these subscriptions
//! and resolves which clients should receive a given event.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single client's subscription record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    /// Client that owns this subscription.
    pub client_id: String,
    /// Topics the client is subscribed to.
    pub topics: Vec<String>,
    /// Timestamp (epoch seconds) when the subscription was created.
    pub created_at: u64,
}

/// Manages per-client topic subscriptions for the IPC event bus.
pub struct SubscriptionManager {
    subscriptions: HashMap<String, Subscription>,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl SubscriptionManager {
    /// Create an empty subscription manager.
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
        }
    }

    /// Subscribe `client_id` to the given `topics`.
    ///
    /// If the client already has a subscription, it is replaced.
    pub fn subscribe(&mut self, client_id: &str, topics: Vec<String>, now: u64) {
        self.subscriptions.insert(
            client_id.to_owned(),
            Subscription {
                client_id: client_id.to_owned(),
                topics,
                created_at: now,
            },
        );
    }

    /// Remove all subscriptions for `client_id`.  Returns `true` if the
    /// client was subscribed.
    pub fn unsubscribe(&mut self, client_id: &str) -> bool {
        self.subscriptions.remove(client_id).is_some()
    }

    /// Return the client IDs of all clients subscribed to `topic`.
    pub fn subscribers_for_topic(&self, topic: &str) -> Vec<&str> {
        self.subscriptions
            .values()
            .filter(|sub| sub.topics.iter().any(|t| t == topic))
            .map(|sub| sub.client_id.as_str())
            .collect()
    }

    /// Return the topics a specific client is subscribed to, or `None` if the
    /// client has no subscription.
    pub fn topics_for_client(&self, client_id: &str) -> Option<&[String]> {
        self.subscriptions
            .get(client_id)
            .map(|sub| sub.topics.as_slice())
    }

    /// Total number of subscribed clients.
    pub fn subscriber_count(&self) -> usize {
        self.subscriptions.len()
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // 1. New manager is empty
    #[test]
    fn new_manager_is_empty() {
        let mgr = SubscriptionManager::new();
        assert_eq!(mgr.subscriber_count(), 0);
    }

    // 2. Subscribe adds a client
    #[test]
    fn subscribe_adds_client() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec!["device".into()], 100);
        assert_eq!(mgr.subscriber_count(), 1);
    }

    // 3. Topics for client returns correct topics
    #[test]
    fn topics_for_client_correct() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec!["device".into(), "telemetry".into()], 0);
        let topics = mgr.topics_for_client("c1").unwrap();
        assert_eq!(topics, &["device", "telemetry"]);
    }

    // 4. Topics for unknown client returns None
    #[test]
    fn topics_for_unknown_client_returns_none() {
        let mgr = SubscriptionManager::new();
        assert!(mgr.topics_for_client("ghost").is_none());
    }

    // 5. Subscribers for topic finds matching clients
    #[test]
    fn subscribers_for_topic_finds_matches() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec!["device".into()], 0);
        mgr.subscribe("c2", vec!["device".into(), "profile".into()], 0);
        mgr.subscribe("c3", vec!["profile".into()], 0);

        let mut subs = mgr.subscribers_for_topic("device");
        subs.sort();
        assert_eq!(subs, vec!["c1", "c2"]);
    }

    // 6. Subscribers for topic with no matches
    #[test]
    fn subscribers_for_topic_no_matches() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec!["device".into()], 0);
        let subs = mgr.subscribers_for_topic("telemetry");
        assert!(subs.is_empty());
    }

    // 7. Unsubscribe removes client
    #[test]
    fn unsubscribe_removes_client() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec!["device".into()], 0);
        assert!(mgr.unsubscribe("c1"));
        assert_eq!(mgr.subscriber_count(), 0);
        assert!(mgr.topics_for_client("c1").is_none());
    }

    // 8. Unsubscribe unknown returns false
    #[test]
    fn unsubscribe_unknown_returns_false() {
        let mut mgr = SubscriptionManager::new();
        assert!(!mgr.unsubscribe("ghost"));
    }

    // 9. Resubscribe replaces topics
    #[test]
    fn resubscribe_replaces_topics() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec!["device".into()], 0);
        mgr.subscribe("c1", vec!["profile".into(), "telemetry".into()], 10);

        assert_eq!(mgr.subscriber_count(), 1);
        let topics = mgr.topics_for_client("c1").unwrap();
        assert_eq!(topics, &["profile", "telemetry"]);
    }

    // 10. Multiple clients independent
    #[test]
    fn multiple_clients_independent() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec!["device".into()], 0);
        mgr.subscribe("c2", vec!["profile".into()], 0);

        mgr.unsubscribe("c1");
        assert_eq!(mgr.subscriber_count(), 1);
        assert!(mgr.topics_for_client("c2").is_some());
    }

    // 11. Default trait creates empty manager
    #[test]
    fn default_creates_empty_manager() {
        let mgr = SubscriptionManager::default();
        assert_eq!(mgr.subscriber_count(), 0);
    }

    // 12. Subscribe with empty topics list
    #[test]
    fn subscribe_with_empty_topics() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("c1", vec![], 0);
        assert_eq!(mgr.subscriber_count(), 1);
        let topics = mgr.topics_for_client("c1").unwrap();
        assert!(topics.is_empty());
        // Should not appear in any topic query
        let subs = mgr.subscribers_for_topic("device");
        assert!(subs.is_empty());
    }
}
