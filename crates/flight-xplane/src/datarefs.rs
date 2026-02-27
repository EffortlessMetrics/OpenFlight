// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane dataref subscription management (REQ-669)
//!
//! Provides a high-level manager for subscribing to X-Plane datarefs at
//! configurable update rates, and retrieving their latest cached values.

use std::collections::HashMap;

/// A single dataref subscription descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct DatarefSubscription {
    /// Full dataref path, e.g. `"sim/flightmodel/position/indicated_airspeed"`.
    pub dataref_path: String,
    /// Desired update rate in Hz.
    pub update_rate_hz: f32,
}

/// Manages active dataref subscriptions and their cached values.
#[derive(Debug)]
pub struct DatarefManager {
    subscriptions: HashMap<String, DatarefSubscription>,
    values: HashMap<String, f32>,
}

impl DatarefManager {
    /// Create a new, empty dataref manager.
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            values: HashMap::new(),
        }
    }

    /// Subscribe to a dataref at the given update rate.
    ///
    /// If the dataref is already subscribed, the rate is updated.
    pub fn subscribe(&mut self, path: &str, rate: f32) {
        let sub = DatarefSubscription {
            dataref_path: path.to_string(),
            update_rate_hz: rate,
        };
        self.subscriptions.insert(path.to_string(), sub);
    }

    /// Unsubscribe from a dataref, removing it and its cached value.
    pub fn unsubscribe(&mut self, path: &str) {
        self.subscriptions.remove(path);
        self.values.remove(path);
    }

    /// Get the latest cached value for a dataref, or `None` if unknown.
    pub fn get_value(&self, path: &str) -> Option<f32> {
        self.values.get(path).copied()
    }

    /// Update the cached value for a dataref.
    pub fn set_value(&mut self, path: &str, value: f32) {
        self.values.insert(path.to_string(), value);
    }

    /// Returns the number of active subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Returns `true` if the given dataref path is actively subscribed.
    pub fn is_subscribed(&self, path: &str) -> bool {
        self.subscriptions.contains_key(path)
    }

    /// Returns the subscription descriptor for a dataref, if subscribed.
    pub fn get_subscription(&self, path: &str) -> Option<&DatarefSubscription> {
        self.subscriptions.get(path)
    }
}

impl Default for DatarefManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_adds_to_active_list() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/flightmodel/position/indicated_airspeed", 30.0);
        assert!(mgr.is_subscribed("sim/flightmodel/position/indicated_airspeed"));
        assert_eq!(mgr.subscription_count(), 1);
    }

    #[test]
    fn test_unsubscribe_removes_from_active_list() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/flightmodel/position/indicated_airspeed", 30.0);
        mgr.unsubscribe("sim/flightmodel/position/indicated_airspeed");
        assert!(!mgr.is_subscribed("sim/flightmodel/position/indicated_airspeed"));
        assert_eq!(mgr.subscription_count(), 0);
    }

    #[test]
    fn test_get_value_returns_none_for_unknown() {
        let mgr = DatarefManager::new();
        assert_eq!(mgr.get_value("sim/nonexistent"), None);
    }

    #[test]
    fn test_get_value_returns_cached_value() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 10.0);
        mgr.set_value("sim/airspeed", 150.0);
        assert_eq!(mgr.get_value("sim/airspeed"), Some(150.0));
    }

    #[test]
    fn test_unsubscribe_clears_cached_value() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 10.0);
        mgr.set_value("sim/airspeed", 150.0);
        mgr.unsubscribe("sim/airspeed");
        assert_eq!(mgr.get_value("sim/airspeed"), None);
    }

    #[test]
    fn test_subscribe_updates_rate() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 10.0);
        mgr.subscribe("sim/airspeed", 60.0);
        let sub = mgr.get_subscription("sim/airspeed").unwrap();
        assert_eq!(sub.update_rate_hz, 60.0);
        assert_eq!(mgr.subscription_count(), 1);
    }

    #[test]
    fn test_multiple_subscriptions() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 30.0);
        mgr.subscribe("sim/altitude", 10.0);
        mgr.subscribe("sim/heading", 5.0);
        assert_eq!(mgr.subscription_count(), 3);
    }

    #[test]
    fn test_default_manager_is_empty() {
        let mgr = DatarefManager::default();
        assert_eq!(mgr.subscription_count(), 0);
    }
}
