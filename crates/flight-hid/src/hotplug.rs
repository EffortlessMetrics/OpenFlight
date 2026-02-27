// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device hot-plug detection and reconnect policy.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

/// A hot-plug event from a device monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotplugEvent {
    /// A new device was connected.
    Connected { vid: u16, pid: u16, path: String },
    /// A device was disconnected.
    Disconnected { vid: u16, pid: u16, path: String },
}

impl HotplugEvent {
    pub fn vid(&self) -> u16 {
        match self {
            HotplugEvent::Connected { vid, .. } | HotplugEvent::Disconnected { vid, .. } => *vid,
        }
    }

    pub fn pid(&self) -> u16 {
        match self {
            HotplugEvent::Connected { pid, .. } | HotplugEvent::Disconnected { pid, .. } => *pid,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            HotplugEvent::Connected { path, .. } | HotplugEvent::Disconnected { path, .. } => {
                path.as_str()
            }
        }
    }

    pub fn is_connect(&self) -> bool {
        matches!(self, HotplugEvent::Connected { .. })
    }

    pub fn is_disconnect(&self) -> bool {
        !self.is_connect()
    }
}

/// Trait for receiving hot-plug events (platform-specific implementations).
pub trait HotplugMonitor: Send + Sync {
    /// Poll for pending events (non-blocking). Returns events since last call.
    fn poll_events(&mut self) -> Vec<HotplugEvent>;
}

/// Mock hot-plug monitor for testing.
pub struct MockHotplugMonitor {
    pending: VecDeque<HotplugEvent>,
}

impl MockHotplugMonitor {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
        }
    }

    pub fn push_event(&mut self, event: HotplugEvent) {
        self.pending.push_back(event);
    }
}

impl Default for MockHotplugMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl HotplugMonitor for MockHotplugMonitor {
    fn poll_events(&mut self) -> Vec<HotplugEvent> {
        self.pending.drain(..).collect()
    }
}

/// Reconnect policy state for a single device.
#[derive(Debug, Clone)]
pub struct ReconnectState {
    pub vid: u16,
    pub pid: u16,
    pub path: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub last_seen_at: Instant,
}

impl ReconnectState {
    pub fn new(vid: u16, pid: u16, path: String, max_attempts: u32) -> Self {
        Self {
            vid,
            pid,
            path,
            attempts: 0,
            max_attempts,
            last_seen_at: Instant::now(),
        }
    }

    pub fn should_retry(&self) -> bool {
        self.attempts < self.max_attempts
    }

    pub fn increment_attempt(&mut self) {
        self.attempts += 1;
    }

    pub fn is_exhausted(&self) -> bool {
        self.attempts >= self.max_attempts
    }

    pub fn reset(&mut self) {
        self.attempts = 0;
    }
}

/// Manager for reconnect policies across multiple devices.
pub struct ReconnectManager {
    policies: HashMap<(u16, u16), ReconnectState>,
    default_max_attempts: u32,
}

impl ReconnectManager {
    pub fn new(default_max_attempts: u32) -> Self {
        Self {
            policies: HashMap::new(),
            default_max_attempts,
        }
    }

    /// Record a disconnect event, inserting a new policy entry if needed.
    pub fn on_disconnect(&mut self, vid: u16, pid: u16, path: &str) {
        self.policies
            .entry((vid, pid))
            .and_modify(|s| {
                s.path = path.to_owned();
                s.last_seen_at = Instant::now();
            })
            .or_insert_with(|| {
                ReconnectState::new(vid, pid, path.to_owned(), self.default_max_attempts)
            });
    }

    /// Record a successful connection, resetting the retry counter.
    pub fn on_connect(&mut self, vid: u16, pid: u16) {
        if let Some(state) = self.policies.get_mut(&(vid, pid)) {
            state.reset();
        }
    }

    pub fn get_state(&self, vid: u16, pid: u16) -> Option<&ReconnectState> {
        self.policies.get(&(vid, pid))
    }

    pub fn should_retry(&self, vid: u16, pid: u16) -> bool {
        self.policies
            .get(&(vid, pid))
            .is_some_and(|s| s.should_retry())
    }

    /// Returns VID/PID pairs whose retry attempts are exhausted.
    pub fn exhausted_devices(&self) -> Vec<(u16, u16)> {
        self.policies
            .iter()
            .filter(|(_, s)| s.is_exhausted())
            .map(|(k, _)| *k)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected(vid: u16, pid: u16, path: &str) -> HotplugEvent {
        HotplugEvent::Connected {
            vid,
            pid,
            path: path.to_owned(),
        }
    }

    fn disconnected(vid: u16, pid: u16, path: &str) -> HotplugEvent {
        HotplugEvent::Disconnected {
            vid,
            pid,
            path: path.to_owned(),
        }
    }

    #[test]
    fn test_hotplug_event_connected_fields() {
        let ev = connected(0x1234, 0x5678, "/dev/input0");
        assert_eq!(ev.vid(), 0x1234);
        assert_eq!(ev.pid(), 0x5678);
        assert_eq!(ev.path(), "/dev/input0");
        assert!(ev.is_connect());
        assert!(!ev.is_disconnect());
    }

    #[test]
    fn test_hotplug_event_disconnected_is_disconnect() {
        let ev = disconnected(0xABCD, 0x0001, "/dev/input1");
        assert!(ev.is_disconnect());
        assert!(!ev.is_connect());
        assert_eq!(ev.vid(), 0xABCD);
        assert_eq!(ev.pid(), 0x0001);
        assert_eq!(ev.path(), "/dev/input1");
    }

    #[test]
    fn test_mock_monitor_empty_initially() {
        let mut monitor = MockHotplugMonitor::new();
        assert!(monitor.poll_events().is_empty());
    }

    #[test]
    fn test_mock_monitor_returns_pushed_events() {
        let mut monitor = MockHotplugMonitor::new();
        let e1 = connected(0x045E, 0x02FF, "/dev/hid0");
        let e2 = disconnected(0x045E, 0x02FF, "/dev/hid0");
        monitor.push_event(e1.clone());
        monitor.push_event(e2.clone());
        let events = monitor.poll_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], e1);
        assert_eq!(events[1], e2);
    }

    #[test]
    fn test_mock_monitor_drains_on_poll() {
        let mut monitor = MockHotplugMonitor::new();
        monitor.push_event(connected(0x0001, 0x0002, "/dev/hid1"));
        let first = monitor.poll_events();
        assert_eq!(first.len(), 1);
        let second = monitor.poll_events();
        assert!(second.is_empty());
    }

    #[test]
    fn test_reconnect_state_should_retry_initial() {
        let state = ReconnectState::new(0x1000, 0x2000, "/dev/hid2".to_owned(), 5);
        assert!(state.should_retry());
        assert!(!state.is_exhausted());
        assert_eq!(state.attempts, 0);
    }

    #[test]
    fn test_reconnect_state_exhausted_after_max_attempts() {
        let mut state = ReconnectState::new(0x1000, 0x2000, "/dev/hid3".to_owned(), 3);
        for _ in 0..3 {
            state.increment_attempt();
        }
        assert!(state.is_exhausted());
        assert!(!state.should_retry());
    }

    #[test]
    fn test_reconnect_state_reset_clears_attempts() {
        let mut state = ReconnectState::new(0x1000, 0x2000, "/dev/hid4".to_owned(), 2);
        state.increment_attempt();
        state.increment_attempt();
        assert!(state.is_exhausted());
        state.reset();
        assert_eq!(state.attempts, 0);
        assert!(state.should_retry());
    }

    #[test]
    fn test_reconnect_manager_on_connect_resets() {
        let mut mgr = ReconnectManager::new(5);
        mgr.on_disconnect(0xAAAA, 0xBBBB, "/dev/hid5");
        {
            let state = mgr.policies.get_mut(&(0xAAAA, 0xBBBB)).unwrap();
            state.increment_attempt();
            state.increment_attempt();
        }
        assert_eq!(mgr.get_state(0xAAAA, 0xBBBB).unwrap().attempts, 2);
        mgr.on_connect(0xAAAA, 0xBBBB);
        assert_eq!(mgr.get_state(0xAAAA, 0xBBBB).unwrap().attempts, 0);
    }

    #[test]
    fn test_reconnect_manager_exhausted_devices() {
        let mut mgr = ReconnectManager::new(2);
        mgr.on_disconnect(0x0010, 0x0020, "/dev/hid6");
        {
            let state = mgr.policies.get_mut(&(0x0010, 0x0020)).unwrap();
            state.increment_attempt();
            state.increment_attempt();
        }
        let exhausted = mgr.exhausted_devices();
        assert_eq!(exhausted.len(), 1);
        assert_eq!(exhausted[0], (0x0010, 0x0020));
    }

    #[test]
    fn test_reconnect_manager_should_retry_unknown_device() {
        let mgr = ReconnectManager::new(3);
        assert!(!mgr.should_retry(0xDEAD, 0xBEEF));
    }
}
