// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device discovery with polling-based scan and hot-plug event streaming.
//!
//! [`DeviceDiscovery`] periodically enumerates connected HID devices, compares
//! the result with a [`DeviceRegistry`](super::stable_id::DeviceRegistry), and
//! emits [`DeviceEvent`]s for newly connected or disconnected devices.

use std::collections::HashSet;
use std::sync::mpsc;
use std::time::Duration;

use crate::stable_id::{DeviceFingerprint, DeviceRegistry, StableDeviceId};

// ── DiscoveredDevice ─────────────────────────────────────────────────────

/// A device found during a discovery scan.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    /// Persistent identifier for this device.
    pub stable_id: StableDeviceId,
    /// Full device characteristics.
    pub fingerprint: DeviceFingerprint,
    /// `true` if this device was not previously in the registry.
    pub is_new: bool,
}

// ── DeviceEvent ──────────────────────────────────────────────────────────

/// An event emitted by the device watcher.
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// A device was connected (or discovered for the first time).
    Connected(DiscoveredDevice),
    /// A previously-seen device is no longer present.
    Disconnected(StableDeviceId),
}

// ── DeviceScanner trait ──────────────────────────────────────────────────

/// Abstraction over the platform HID enumeration so tests can inject a mock.
pub trait DeviceScanner: Send {
    /// Enumerate all currently-connected HID devices.
    fn enumerate(&mut self) -> Vec<DeviceFingerprint>;
}

// ── MockScanner ──────────────────────────────────────────────────────────

/// A test-only scanner whose device list is set programmatically.
pub struct MockScanner {
    devices: Vec<DeviceFingerprint>,
}

impl MockScanner {
    pub fn new(devices: Vec<DeviceFingerprint>) -> Self {
        Self { devices }
    }

    /// Replace the current device list (simulates plug/unplug).
    pub fn set_devices(&mut self, devices: Vec<DeviceFingerprint>) {
        self.devices = devices;
    }
}

impl DeviceScanner for MockScanner {
    fn enumerate(&mut self) -> Vec<DeviceFingerprint> {
        self.devices.clone()
    }
}

// ── DeviceDiscovery ──────────────────────────────────────────────────────

/// Polling-based device discovery engine.
pub struct DeviceDiscovery<S: DeviceScanner> {
    scanner: S,
    registry: DeviceRegistry,
    /// IDs currently seen as connected.
    connected: HashSet<StableDeviceId>,
    /// Polling interval for the watcher.
    poll_interval: Duration,
}

impl<S: DeviceScanner> DeviceDiscovery<S> {
    /// Create a new discovery engine with the given scanner and poll interval.
    pub fn new(scanner: S, registry: DeviceRegistry, poll_interval: Duration) -> Self {
        Self {
            scanner,
            registry,
            connected: HashSet::new(),
            poll_interval,
        }
    }

    /// Create with the default 1-second poll interval.
    pub fn with_defaults(scanner: S, registry: DeviceRegistry) -> Self {
        Self::new(scanner, registry, Duration::from_secs(1))
    }

    /// Perform a single scan, returning all discovered devices.
    pub fn scan(&mut self) -> Vec<DiscoveredDevice> {
        let fingerprints = self.scanner.enumerate();
        fingerprints
            .into_iter()
            .map(|fp| {
                let id = fp.stable_id();
                let is_new = self.registry.lookup(id).is_none();
                self.registry.register(fp.clone());
                self.connected.insert(id);
                DiscoveredDevice {
                    stable_id: id,
                    fingerprint: fp,
                    is_new,
                }
            })
            .collect()
    }

    /// Perform a single scan tick and return events for changes since the last
    /// tick.
    pub fn poll_events(&mut self) -> Vec<DeviceEvent> {
        let fingerprints = self.scanner.enumerate();
        let mut events = Vec::new();
        let mut current_ids = HashSet::new();

        for fp in fingerprints {
            let id = fp.stable_id();
            current_ids.insert(id);

            if !self.connected.contains(&id) {
                let is_new = self.registry.lookup(id).is_none();
                self.registry.register(fp.clone());
                events.push(DeviceEvent::Connected(DiscoveredDevice {
                    stable_id: id,
                    fingerprint: fp,
                    is_new,
                }));
            }
        }

        // Detect disconnects
        for &id in &self.connected {
            if !current_ids.contains(&id) {
                events.push(DeviceEvent::Disconnected(id));
            }
        }

        self.connected = current_ids;
        events
    }

    /// Start a watcher that sends [`DeviceEvent`]s to a channel.
    ///
    /// Returns `(sender_handle, receiver)`. The caller should run the sender
    /// in a loop or thread; here we provide a synchronous helper that performs
    /// `max_ticks` polling iterations.
    pub fn watch(&mut self, max_ticks: usize) -> Vec<DeviceEvent> {
        let mut all_events = Vec::new();
        for _ in 0..max_ticks {
            all_events.extend(self.poll_events());
        }
        all_events
    }

    /// Create a channel-based watcher. The caller is responsible for calling
    /// `poll_events()` periodically and forwarding events.
    pub fn create_channel(&self) -> (mpsc::Sender<DeviceEvent>, mpsc::Receiver<DeviceEvent>) {
        mpsc::channel()
    }

    /// Access the underlying registry.
    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    /// Mutable access to the underlying registry.
    pub fn registry_mut(&mut self) -> &mut DeviceRegistry {
        &mut self.registry
    }

    /// The configured poll interval.
    pub fn poll_interval(&self) -> Duration {
        self.poll_interval
    }

    /// Current set of connected device IDs.
    pub fn connected_ids(&self) -> &HashSet<StableDeviceId> {
        &self.connected
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn warthog_fp() -> DeviceFingerprint {
        DeviceFingerprint {
            vid: 0x044F,
            pid: 0x0402,
            serial: Some("WH001".into()),
            manufacturer: Some("Thrustmaster".into()),
            product: Some("HOTAS Warthog Joystick".into()),
            interface_number: Some(0),
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-2.3".into()),
        }
    }

    fn vkb_fp() -> DeviceFingerprint {
        DeviceFingerprint {
            vid: 0x231D,
            pid: 0x0136,
            serial: Some("VKB001".into()),
            manufacturer: Some("VKB".into()),
            product: Some("Gladiator NXT EVO".into()),
            interface_number: None,
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-4.1".into()),
        }
    }

    fn t16000m_fp() -> DeviceFingerprint {
        DeviceFingerprint {
            vid: 0x044F,
            pid: 0xB679,
            serial: None,
            manufacturer: Some("Thrustmaster".into()),
            product: Some("T16000M".into()),
            interface_number: None,
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-5.2".into()),
        }
    }

    #[test]
    fn scan_empty() {
        let scanner = MockScanner::new(vec![]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        assert!(found.is_empty());
    }

    #[test]
    fn scan_finds_devices() {
        let scanner = MockScanner::new(vec![warthog_fp(), vkb_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|d| d.is_new));
    }

    #[test]
    fn scan_marks_known_device() {
        let mut registry = DeviceRegistry::new();
        registry.register(warthog_fp());
        let scanner = MockScanner::new(vec![warthog_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, registry);
        let found = disc.scan();
        assert_eq!(found.len(), 1);
        assert!(!found[0].is_new, "already registered device is not new");
    }

    #[test]
    fn poll_detects_connect() {
        let scanner = MockScanner::new(vec![]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        // Initial empty poll
        let events = disc.poll_events();
        assert!(events.is_empty());

        // Plug in a device
        disc.scanner.set_devices(vec![warthog_fp()]);
        let events = disc.poll_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], DeviceEvent::Connected(d) if d.is_new));
    }

    #[test]
    fn poll_detects_disconnect() {
        let scanner = MockScanner::new(vec![warthog_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        // First poll sees the device
        disc.poll_events();

        // Unplug
        disc.scanner.set_devices(vec![]);
        let events = disc.poll_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], DeviceEvent::Disconnected(_)));
    }

    #[test]
    fn poll_no_events_when_stable() {
        let scanner = MockScanner::new(vec![warthog_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        disc.poll_events(); // initial connect
        let events = disc.poll_events(); // same state
        assert!(events.is_empty());
    }

    #[test]
    fn poll_connect_disconnect_connect() {
        let scanner = MockScanner::new(vec![]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());

        disc.poll_events();

        // Connect
        disc.scanner.set_devices(vec![warthog_fp()]);
        let ev = disc.poll_events();
        assert_eq!(ev.len(), 1);
        assert!(matches!(&ev[0], DeviceEvent::Connected(d) if d.is_new));

        // Disconnect
        disc.scanner.set_devices(vec![]);
        let ev = disc.poll_events();
        assert_eq!(ev.len(), 1);
        assert!(matches!(&ev[0], DeviceEvent::Disconnected(_)));

        // Reconnect — same device, not new anymore
        disc.scanner.set_devices(vec![warthog_fp()]);
        let ev = disc.poll_events();
        assert_eq!(ev.len(), 1);
        assert!(matches!(&ev[0], DeviceEvent::Connected(d) if !d.is_new));
    }

    #[test]
    fn poll_multiple_connect_disconnect() {
        let scanner = MockScanner::new(vec![]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        disc.poll_events();

        // Two devices connect
        disc.scanner.set_devices(vec![warthog_fp(), vkb_fp()]);
        let ev = disc.poll_events();
        assert_eq!(ev.len(), 2);
        assert!(ev.iter().all(|e| matches!(e, DeviceEvent::Connected(_))));

        // One disconnects
        disc.scanner.set_devices(vec![vkb_fp()]);
        let ev = disc.poll_events();
        assert_eq!(ev.len(), 1);
        assert!(matches!(&ev[0], DeviceEvent::Disconnected(_)));
    }

    #[test]
    fn watch_collects_events() {
        let scanner = MockScanner::new(vec![warthog_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let events = disc.watch(2);
        // First tick: connect; second tick: no change
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn discovery_updates_registry() {
        let scanner = MockScanner::new(vec![warthog_fp(), vkb_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        disc.scan();
        assert_eq!(disc.registry().len(), 2);
    }

    #[test]
    fn connected_ids_tracks_state() {
        let scanner = MockScanner::new(vec![warthog_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        disc.scan();
        assert_eq!(disc.connected_ids().len(), 1);
    }

    #[test]
    fn poll_interval_default() {
        let scanner = MockScanner::new(vec![]);
        let disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        assert_eq!(disc.poll_interval(), Duration::from_secs(1));
    }

    #[test]
    fn create_channel_works() {
        let scanner = MockScanner::new(vec![]);
        let disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let (tx, rx) = disc.create_channel();
        let warthog_id = warthog_fp().stable_id();
        tx.send(DeviceEvent::Disconnected(warthog_id)).unwrap();
        let received = rx.recv().unwrap();
        assert!(matches!(received, DeviceEvent::Disconnected(_)));
    }

    #[test]
    fn scan_three_devices_all_new() {
        let scanner = MockScanner::new(vec![warthog_fp(), vkb_fp(), t16000m_fp()]);
        let mut disc = DeviceDiscovery::with_defaults(scanner, DeviceRegistry::new());
        let found = disc.scan();
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|d| d.is_new));
    }
}
