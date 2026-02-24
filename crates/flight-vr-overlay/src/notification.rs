// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Overlay notifications — queued, time-limited messages shown in the VR panel.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Severity level of a notification, controls colour and icon in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Routine information (grey / white).
    Info,
    /// Non-critical warning (yellow).
    Warning,
    /// Action required (orange).
    Alert,
    /// Critical fault (red); persists until acknowledged.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => f.write_str("INFO"),
            Self::Warning => f.write_str("WARN"),
            Self::Alert => f.write_str("ALERT"),
            Self::Critical => f.write_str("CRIT"),
        }
    }
}

/// A single overlay notification.
#[derive(Debug, Clone)]
pub struct OverlayNotification {
    /// Human-readable message text (max 120 chars recommended).
    pub message: String,
    /// Severity level.
    pub severity: Severity,
    /// When this notification was created.
    created_at: Instant,
    /// Time-to-live. `None` = persists until explicitly dismissed.
    ttl: Option<Duration>,
    /// Whether the user has acknowledged this notification.
    acknowledged: bool,
}

impl OverlayNotification {
    /// Create a new notification with an explicit TTL.
    pub fn new(message: impl Into<String>, severity: Severity, ttl: Duration) -> Self {
        Self {
            message: message.into(),
            severity,
            created_at: Instant::now(),
            ttl: Some(ttl),
            acknowledged: false,
        }
    }

    /// Create a persistent notification that never auto-expires.
    pub fn persistent(message: impl Into<String>, severity: Severity) -> Self {
        Self {
            message: message.into(),
            severity,
            created_at: Instant::now(),
            ttl: None,
            acknowledged: false,
        }
    }

    /// Returns `true` if the notification has expired and should be removed.
    pub fn is_expired(&self) -> bool {
        if self.acknowledged {
            return true;
        }
        match self.ttl {
            Some(ttl) => self.created_at.elapsed() >= ttl,
            None => false,
        }
    }

    /// Age of this notification.
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Mark this notification as acknowledged (will be removed on next prune).
    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    /// Whether the notification has been acknowledged.
    pub fn is_acknowledged(&self) -> bool {
        self.acknowledged
    }
}

/// Thread-safe notification queue for the VR overlay.
///
/// # Example
///
/// ```
/// use flight_vr_overlay::notification::{NotificationQueue, OverlayNotification, Severity};
/// use std::time::Duration;
///
/// let mut q = NotificationQueue::new(5);
/// q.push(OverlayNotification::new("Profile loaded", Severity::Info, Duration::from_secs(4)));
/// assert_eq!(q.len(), 1);
/// ```
#[derive(Debug)]
pub struct NotificationQueue {
    items: Vec<OverlayNotification>,
    max_capacity: usize,
}

impl NotificationQueue {
    /// Create a new queue with the given maximum capacity.
    pub fn new(max_capacity: usize) -> Self {
        assert!(max_capacity > 0, "max_capacity must be >= 1");
        Self {
            items: Vec::with_capacity(max_capacity),
            max_capacity,
        }
    }

    /// Add a notification.
    ///
    /// If the queue is at capacity the oldest non-Critical item is evicted.
    /// If all items are Critical the new item is dropped.
    pub fn push(&mut self, notification: OverlayNotification) {
        self.prune_expired();
        if self.items.len() >= self.max_capacity {
            // Evict the oldest non-critical item
            if let Some(pos) = self
                .items
                .iter()
                .position(|n| n.severity != Severity::Critical)
            {
                self.items.remove(pos);
            } else {
                return; // All slots hold Critical messages — drop incoming
            }
        }
        self.items.push(notification);
    }

    /// Remove expired and acknowledged notifications.
    pub fn prune_expired(&mut self) {
        self.items.retain(|n| !n.is_expired());
    }

    /// Acknowledge the first notification matching `message` exactly.
    /// Returns `true` if a matching notification was found.
    pub fn acknowledge(&mut self, message: &str) -> bool {
        for n in &mut self.items {
            if n.message == message {
                n.acknowledge();
                return true;
            }
        }
        false
    }

    /// Returns all active (non-expired) notifications sorted by severity (highest first).
    pub fn active(&self) -> Vec<&OverlayNotification> {
        let mut active: Vec<&OverlayNotification> =
            self.items.iter().filter(|n| !n.is_expired()).collect();
        active.sort_by(|a, b| b.severity.cmp(&a.severity));
        active
    }

    /// Number of items in the queue (including expired ones not yet pruned).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the queue contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Highest severity among active notifications, or `None` if empty.
    pub fn max_severity(&self) -> Option<Severity> {
        self.active().iter().map(|n| n.severity).max()
    }

    /// Clear all notifications immediately.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn info(msg: &str) -> OverlayNotification {
        OverlayNotification::new(msg, Severity::Info, Duration::from_secs(60))
    }

    fn warn(msg: &str) -> OverlayNotification {
        OverlayNotification::new(msg, Severity::Warning, Duration::from_secs(60))
    }

    fn crit(msg: &str) -> OverlayNotification {
        OverlayNotification::new(msg, Severity::Critical, Duration::from_secs(60))
    }

    #[test]
    fn test_push_and_len() {
        let mut q = NotificationQueue::new(5);
        q.push(info("a"));
        q.push(info("b"));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn test_expired_notification_removed_on_prune() {
        let mut q = NotificationQueue::new(5);
        q.push(OverlayNotification::new(
            "short",
            Severity::Info,
            Duration::from_millis(1),
        ));
        std::thread::sleep(Duration::from_millis(5));
        q.prune_expired();
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn test_capacity_evicts_oldest_noncritical() {
        let mut q = NotificationQueue::new(2);
        q.push(info("first"));
        q.push(warn("second"));
        q.push(info("third")); // should evict "first"
        let active: Vec<&str> = q.active().iter().map(|n| n.message.as_str()).collect();
        assert!(!active.contains(&"first"));
        assert!(active.contains(&"third"));
    }

    #[test]
    fn test_capacity_drops_incoming_when_all_critical() {
        let mut q = NotificationQueue::new(2);
        q.push(crit("c1"));
        q.push(crit("c2"));
        q.push(info("dropped")); // should be dropped
        assert_eq!(q.len(), 2);
        let msgs: Vec<&str> = q.active().iter().map(|n| n.message.as_str()).collect();
        assert!(!msgs.contains(&"dropped"));
    }

    #[test]
    fn test_acknowledge_marks_notification() {
        let mut q = NotificationQueue::new(5);
        q.push(info("ack-me"));
        assert!(q.acknowledge("ack-me"));
        // After acknowledging, it becomes expired
        assert!(q.items[0].is_expired());
    }

    #[test]
    fn test_acknowledge_nonexistent_returns_false() {
        let mut q = NotificationQueue::new(5);
        assert!(!q.acknowledge("ghost"));
    }

    #[test]
    fn test_max_severity_empty() {
        let q = NotificationQueue::new(3);
        assert_eq!(q.max_severity(), None);
    }

    #[test]
    fn test_max_severity_with_items() {
        let mut q = NotificationQueue::new(5);
        q.push(info("a"));
        q.push(warn("b"));
        assert_eq!(q.max_severity(), Some(Severity::Warning));
    }

    #[test]
    fn test_active_sorted_by_severity_desc() {
        let mut q = NotificationQueue::new(5);
        q.push(info("low"));
        q.push(crit("high"));
        q.push(warn("mid"));
        let active = q.active();
        assert_eq!(active[0].severity, Severity::Critical);
        assert_eq!(active[1].severity, Severity::Warning);
        assert_eq!(active[2].severity, Severity::Info);
    }

    #[test]
    fn test_persistent_notification_does_not_expire() {
        let n = OverlayNotification::persistent("sticky", Severity::Alert);
        std::thread::sleep(Duration::from_millis(5));
        assert!(!n.is_expired());
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::Alert);
        assert!(Severity::Alert > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Info.to_string(), "INFO");
        assert_eq!(Severity::Warning.to_string(), "WARN");
        assert_eq!(Severity::Critical.to_string(), "CRIT");
    }

    #[test]
    fn test_clear_empties_queue() {
        let mut q = NotificationQueue::new(5);
        q.push(info("a"));
        q.push(warn("b"));
        q.clear();
        assert!(q.is_empty());
    }
}
