// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Topic-based event subscription with wildcard pattern matching.
//!
//! Provides a string-based [`SubscriptionManager`] where subscribers register
//! topic patterns (optionally containing `*` wildcards) and receive byte-slice
//! events via [`broadcast`](SubscriptionManager::broadcast).
//!
//! # Wildcard rules
//!
//! | Pattern | Matches |
//! |---------|---------|
//! | `devices.connected` | Exactly `devices.connected` |
//! | `devices.*` | `devices.connected`, `devices.error`, … |
//! | `*` | Everything |
//! | `*.status` | `health.status`, `ffb.status`, … |

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Wildcard matching
// ---------------------------------------------------------------------------

/// Glob-style wildcard match where `*` matches zero or more characters.
fn matches_wildcard(pattern: &str, topic: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == topic;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match topic[pos..].find(part) {
            Some(idx) => {
                // First segment must anchor at the start of the topic
                if i == 0 && idx != 0 {
                    return false;
                }
                pos += idx + part.len();
            }
            None => return false,
        }
    }

    // If the pattern does not end with '*', the topic must be fully consumed
    pattern.ends_with('*') || pos == topic.len()
}

// ---------------------------------------------------------------------------
// SubscriptionHandle
// ---------------------------------------------------------------------------

/// Handle returned by [`SubscriptionManager::subscribe`].
#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    /// Unique subscription identifier.
    pub id: u64,
    /// Topic pattern this subscription matches against.
    pub pattern: String,
    active: Arc<AtomicBool>,
}

impl SubscriptionHandle {
    /// Returns `true` while the subscription is active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Cancel the subscription. Idempotent.
    pub fn cancel(&self) {
        self.active.store(false, Ordering::Release);
    }
}

// ---------------------------------------------------------------------------
// Internal record
// ---------------------------------------------------------------------------

struct SubscriptionRecord {
    id: u64,
    pattern: String,
    active: Arc<AtomicBool>,
}

// ---------------------------------------------------------------------------
// SubscriptionManager
// ---------------------------------------------------------------------------

/// Manages topic-based subscriptions with wildcard pattern matching.
///
/// Subscribers register topic patterns and are matched during
/// [`broadcast`](Self::broadcast). Cancelled or dropped handles are
/// garbage-collected automatically.
pub struct SubscriptionManager {
    subscriptions: HashMap<u64, SubscriptionRecord>,
    next_id: AtomicU64,
}

impl SubscriptionManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Subscribe to events matching `topic` (may contain `*` wildcards).
    pub fn subscribe(&mut self, topic: &str) -> SubscriptionHandle {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let active = Arc::new(AtomicBool::new(true));

        self.subscriptions.insert(
            id,
            SubscriptionRecord {
                id,
                pattern: topic.to_owned(),
                active: Arc::clone(&active),
            },
        );

        SubscriptionHandle {
            id,
            pattern: topic.to_owned(),
            active,
        }
    }

    /// Remove a subscription by handle. Returns `true` if found.
    pub fn unsubscribe(&mut self, handle: &SubscriptionHandle) -> bool {
        handle.cancel();
        self.subscriptions.remove(&handle.id).is_some()
    }

    /// Broadcast an event to all subscriptions whose pattern matches `topic`.
    ///
    /// Returns the subscription IDs that matched. The `_event` payload is
    /// accepted for API completeness; actual delivery would occur over IPC.
    pub fn broadcast(&mut self, topic: &str, _event: &[u8]) -> Vec<u64> {
        self.gc();
        self.subscriptions
            .values()
            .filter(|rec| matches_wildcard(&rec.pattern, topic))
            .map(|rec| rec.id)
            .collect()
    }

    /// Number of active subscriptions after garbage collection.
    pub fn active_count(&mut self) -> usize {
        self.gc();
        self.subscriptions.len()
    }

    /// Remove cancelled subscriptions.
    fn gc(&mut self) {
        self.subscriptions
            .retain(|_, rec| rec.active.load(Ordering::Acquire));
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

    // -- wildcard matching -------------------------------------------------

    #[test]
    fn wildcard_exact_match() {
        assert!(matches_wildcard("devices.connected", "devices.connected"));
        assert!(!matches_wildcard(
            "devices.connected",
            "devices.disconnected"
        ));
    }

    #[test]
    fn wildcard_star_matches_all() {
        assert!(matches_wildcard("*", "anything"));
        assert!(matches_wildcard("*", ""));
        assert!(matches_wildcard("*", "a.b.c"));
    }

    #[test]
    fn wildcard_trailing_star() {
        assert!(matches_wildcard("devices.*", "devices.connected"));
        assert!(matches_wildcard("devices.*", "devices.error"));
        assert!(!matches_wildcard("devices.*", "profiles.loaded"));
    }

    #[test]
    fn wildcard_leading_star() {
        assert!(matches_wildcard("*.status", "health.status"));
        assert!(matches_wildcard("*.status", "ffb.status"));
        assert!(!matches_wildcard("*.status", "health.check"));
    }

    #[test]
    fn wildcard_middle_star() {
        assert!(matches_wildcard("a.*.c", "a.b.c"));
        assert!(matches_wildcard("a.*.c", "a.xyz.c"));
        assert!(!matches_wildcard("a.*.c", "a.b.d"));
    }

    #[test]
    fn wildcard_empty_pattern_and_topic() {
        assert!(matches_wildcard("", ""));
        assert!(!matches_wildcard("", "nonempty"));
    }

    // -- subscribe / unsubscribe -------------------------------------------

    #[test]
    fn subscribe_returns_active_handle() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe("devices.*");
        assert!(h.is_active());
        assert_eq!(h.pattern, "devices.*");
        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn subscription_ids_are_unique() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe("a");
        let h2 = mgr.subscribe("b");
        assert_ne!(h1.id, h2.id);
    }

    #[test]
    fn unsubscribe_removes_subscription() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe("topic");
        assert!(mgr.unsubscribe(&h));
        assert!(!h.is_active());
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn unsubscribe_unknown_returns_false() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe("topic");
        mgr.unsubscribe(&h);
        assert!(!mgr.unsubscribe(&h));
    }

    #[test]
    fn cancel_handle_causes_gc() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe("topic");
        h.cancel();
        assert_eq!(mgr.active_count(), 0);
    }

    // -- broadcast ---------------------------------------------------------

    #[test]
    fn broadcast_exact_topic() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe("devices.connected");
        let ids = mgr.broadcast("devices.connected", b"payload");
        assert_eq!(ids, vec![h.id]);
    }

    #[test]
    fn broadcast_no_match() {
        let mut mgr = SubscriptionManager::new();
        let _h = mgr.subscribe("devices.connected");
        let ids = mgr.broadcast("profiles.loaded", b"payload");
        assert!(ids.is_empty());
    }

    #[test]
    fn broadcast_wildcard_match() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe("devices.*");
        let ids = mgr.broadcast("devices.connected", b"ev");
        assert_eq!(ids, vec![h.id]);
    }

    #[test]
    fn broadcast_multiple_subscribers() {
        let mut mgr = SubscriptionManager::new();
        let _h1 = mgr.subscribe("devices.*");
        let _h2 = mgr.subscribe("*");
        let ids = mgr.broadcast("devices.connected", b"ev");
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn broadcast_skips_cancelled() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe("topic");
        let _h2 = mgr.subscribe("topic");
        h1.cancel();
        let ids = mgr.broadcast("topic", b"ev");
        assert_eq!(ids.len(), 1);
        assert!(!ids.contains(&h1.id));
    }

    #[test]
    fn broadcast_empty_manager() {
        let mut mgr = SubscriptionManager::new();
        let ids = mgr.broadcast("topic", b"ev");
        assert!(ids.is_empty());
    }

    // -- concurrent safety -------------------------------------------------

    #[test]
    fn handle_clone_shares_state() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe("topic");
        let h2 = h1.clone();
        h1.cancel();
        assert!(!h2.is_active());
    }

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SubscriptionHandle>();
        assert_send_sync::<SubscriptionManager>();
    }

    // -- default -----------------------------------------------------------

    #[test]
    fn default_creates_empty_manager() {
        let mut mgr = SubscriptionManager::default();
        assert_eq!(mgr.active_count(), 0);
    }
}
