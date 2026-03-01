// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! A fake HID device and device backend for testing without hardware.

use std::collections::HashMap;

/// A single frame of synthetic device input.
#[derive(Debug, Clone)]
pub struct FakeInput {
    pub axes: Vec<f64>,
    pub buttons: Vec<bool>,
    pub delay_ms: u64,
}

/// A fake HID device for testing without hardware.
#[derive(Debug)]
pub struct FakeDevice {
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub axes: Vec<f64>,
    pub buttons: Vec<bool>,
    pub connected: bool,
    sequence: Vec<FakeInput>,
    position: usize,
}

impl FakeDevice {
    /// Create a new fake device with the given identity and axis/button counts.
    pub fn new(
        name: impl Into<String>,
        vid: u16,
        pid: u16,
        num_axes: usize,
        num_buttons: usize,
    ) -> Self {
        Self {
            name: name.into(),
            vid,
            pid,
            axes: vec![0.0; num_axes],
            buttons: vec![false; num_buttons],
            connected: false,
            sequence: Vec::new(),
            position: 0,
        }
    }

    /// Enqueue an input frame into the replay sequence.
    pub fn enqueue_input(&mut self, input: FakeInput) {
        self.sequence.push(input);
    }

    /// Consume the next input frame from the sequence.
    pub fn next_input(&mut self) -> Option<FakeInput> {
        if self.position < self.sequence.len() {
            let input = self.sequence[self.position].clone();
            self.position += 1;
            Some(input)
        } else {
            None
        }
    }

    /// Mark the device as connected.
    pub fn connect(&mut self) {
        self.connected = true;
    }

    /// Mark the device as disconnected.
    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Set the value of a single axis.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn set_axis(&mut self, index: usize, value: f64) {
        self.axes[index] = value;
    }

    /// Set the pressed state of a single button.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn set_button(&mut self, index: usize, pressed: bool) {
        self.buttons[index] = pressed;
    }

    /// Clear the input sequence and reset playback position.
    pub fn reset(&mut self) {
        self.sequence.clear();
        self.position = 0;
    }
}

// ---------------------------------------------------------------------------
// FakeDeviceBackend — manages multiple fake devices with enumeration
// ---------------------------------------------------------------------------

/// Event emitted by the device backend during polling.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceEvent {
    Connected {
        device_id: String,
    },
    Disconnected {
        device_id: String,
    },
    InputReceived {
        device_id: String,
        input: Vec<f64>,
    },
    ButtonChanged {
        device_id: String,
        index: usize,
        pressed: bool,
    },
}

/// Configuration for a fake device backend.
#[derive(Debug, Clone)]
pub struct FakeDeviceBackendConfig {
    /// Simulated polling rate in Hz.
    pub polling_rate_hz: u32,
    /// Simulated jitter in microseconds added to each poll interval.
    pub jitter_us: u64,
}

impl Default for FakeDeviceBackendConfig {
    fn default() -> Self {
        Self {
            polling_rate_hz: 250,
            jitter_us: 0,
        }
    }
}

/// A backend managing multiple fake devices for integration testing.
///
/// Supports device enumeration, hot-plug simulation, and deterministic
/// input injection across multiple devices.
#[derive(Debug)]
pub struct FakeDeviceBackend {
    config: FakeDeviceBackendConfig,
    devices: HashMap<String, FakeDevice>,
    events: Vec<DeviceEvent>,
    poll_count: u64,
}

impl FakeDeviceBackend {
    /// Create a new backend with the given configuration.
    pub fn new(config: FakeDeviceBackendConfig) -> Self {
        assert!(
            config.polling_rate_hz > 0,
            "polling_rate_hz must be > 0"
        );
        Self {
            config,
            devices: HashMap::new(),
            events: Vec::new(),
            poll_count: 0,
        }
    }

    /// Create a backend with default configuration (250 Hz, no jitter).
    pub fn with_defaults() -> Self {
        Self::new(FakeDeviceBackendConfig::default())
    }

    /// Register a device. Emits a `Connected` event.
    pub fn add_device(&mut self, id: impl Into<String>, device: FakeDevice) {
        let id = id.into();
        self.events.push(DeviceEvent::Connected {
            device_id: id.clone(),
        });
        self.devices.insert(id, device);
    }

    /// Remove a device by id. Emits a `Disconnected` event. Returns the
    /// removed device if it existed.
    pub fn remove_device(&mut self, id: &str) -> Option<FakeDevice> {
        if let Some(dev) = self.devices.remove(id) {
            self.events.push(DeviceEvent::Disconnected {
                device_id: id.to_owned(),
            });
            Some(dev)
        } else {
            None
        }
    }

    /// Simulate a disconnect followed by a reconnect.
    pub fn simulate_reconnect(&mut self, id: &str) {
        if let Some(dev) = self.devices.get_mut(id) {
            dev.disconnect();
            self.events.push(DeviceEvent::Disconnected {
                device_id: id.to_owned(),
            });
            dev.connect();
            self.events.push(DeviceEvent::Connected {
                device_id: id.to_owned(),
            });
        }
    }

    /// Enumerate all currently registered device ids.
    pub fn enumerate(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.devices.keys().map(String::as_str).collect();
        ids.sort();
        ids
    }

    /// Enumerate devices as `(id, vid, pid)` tuples.
    pub fn enumerate_with_ids(&self) -> Vec<(&str, u16, u16)> {
        let mut entries: Vec<(&str, u16, u16)> = self
            .devices
            .iter()
            .map(|(id, dev)| (id.as_str(), dev.vid, dev.pid))
            .collect();
        entries.sort_by_key(|(id, _, _)| *id);
        entries
    }

    /// Get a reference to a device by id.
    pub fn device(&self, id: &str) -> Option<&FakeDevice> {
        self.devices.get(id)
    }

    /// Get a mutable reference to a device by id.
    pub fn device_mut(&mut self, id: &str) -> Option<&mut FakeDevice> {
        self.devices.get_mut(id)
    }

    /// Inject a specific axis value into a device, emitting an `InputReceived` event.
    pub fn inject_axis(&mut self, device_id: &str, axis_index: usize, value: f64) {
        if let Some(dev) = self.devices.get_mut(device_id) {
            dev.set_axis(axis_index, value);
            self.events.push(DeviceEvent::InputReceived {
                device_id: device_id.to_owned(),
                input: dev.axes.clone(),
            });
        }
    }

    /// Inject a button state change into a device, emitting a `ButtonChanged` event.
    pub fn inject_button(&mut self, device_id: &str, button_index: usize, pressed: bool) {
        if let Some(dev) = self.devices.get_mut(device_id) {
            dev.set_button(button_index, pressed);
            self.events.push(DeviceEvent::ButtonChanged {
                device_id: device_id.to_owned(),
                index: button_index,
                pressed,
            });
        }
    }

    /// Poll all connected devices, consuming the next input frame from each.
    /// Returns the number of frames consumed.
    pub fn poll(&mut self) -> usize {
        self.poll_count += 1;
        let mut frames = 0;
        let mut device_ids: Vec<String> = self.devices.keys().cloned().collect();
        device_ids.sort();
        for id in device_ids {
            if let Some(dev) = self.devices.get_mut(&id) {
                if !dev.connected {
                    continue;
                }
                if let Some(input) = dev.next_input() {
                    self.events.push(DeviceEvent::InputReceived {
                        device_id: id,
                        input: input.axes,
                    });
                    frames += 1;
                }
            }
        }
        frames
    }

    /// Return the configured polling rate.
    pub fn polling_rate_hz(&self) -> u32 {
        self.config.polling_rate_hz
    }

    /// Return the configured jitter.
    pub fn jitter_us(&self) -> u64 {
        self.config.jitter_us
    }

    /// Return the computed poll interval in microseconds (including jitter).
    pub fn poll_interval_us(&self) -> u64 {
        let base = 1_000_000 / u64::from(self.config.polling_rate_hz);
        base + self.config.jitter_us
    }

    /// Return the total number of poll cycles executed.
    pub fn poll_count(&self) -> u64 {
        self.poll_count
    }

    /// Drain and return all accumulated events.
    pub fn drain_events(&mut self) -> Vec<DeviceEvent> {
        std::mem::take(&mut self.events)
    }

    /// Return a reference to the accumulated events without draining.
    pub fn events(&self) -> &[DeviceEvent] {
        &self.events
    }

    /// Return the number of registered devices.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_device_defaults() {
        let dev = FakeDevice::new("Test Stick", 0x06a3, 0x0762, 4, 12);
        assert_eq!(dev.name, "Test Stick");
        assert_eq!(dev.vid, 0x06a3);
        assert_eq!(dev.pid, 0x0762);
        assert_eq!(dev.axes.len(), 4);
        assert_eq!(dev.buttons.len(), 12);
        assert!(!dev.connected);
        assert!(dev.axes.iter().all(|&v| v == 0.0));
        assert!(dev.buttons.iter().all(|&v| !v));
    }

    #[test]
    fn connect_disconnect() {
        let mut dev = FakeDevice::new("Stick", 0x1234, 0x5678, 1, 1);
        assert!(!dev.connected);
        dev.connect();
        assert!(dev.connected);
        dev.disconnect();
        assert!(!dev.connected);
    }

    #[test]
    fn set_axis_and_button() {
        let mut dev = FakeDevice::new("Stick", 0x1234, 0x5678, 3, 4);
        dev.set_axis(1, 0.75);
        dev.set_button(2, true);
        assert!((dev.axes[1] - 0.75).abs() < f64::EPSILON);
        assert!(dev.buttons[2]);
    }

    #[test]
    fn enqueue_and_consume_inputs() {
        let mut dev = FakeDevice::new("Stick", 0x1234, 0x5678, 2, 2);
        dev.enqueue_input(FakeInput {
            axes: vec![0.5, -0.5],
            buttons: vec![true, false],
            delay_ms: 10,
        });
        dev.enqueue_input(FakeInput {
            axes: vec![1.0, 0.0],
            buttons: vec![false, true],
            delay_ms: 20,
        });

        let first = dev.next_input().unwrap();
        assert!((first.axes[0] - 0.5).abs() < f64::EPSILON);
        assert_eq!(first.delay_ms, 10);

        let second = dev.next_input().unwrap();
        assert!((second.axes[0] - 1.0).abs() < f64::EPSILON);
        assert_eq!(second.delay_ms, 20);

        assert!(dev.next_input().is_none());
    }

    #[test]
    fn reset_clears_sequence() {
        let mut dev = FakeDevice::new("Stick", 0x1234, 0x5678, 1, 1);
        dev.enqueue_input(FakeInput {
            axes: vec![0.1],
            buttons: vec![false],
            delay_ms: 5,
        });
        dev.next_input();
        dev.reset();
        assert!(dev.next_input().is_none());
    }

    #[test]
    #[should_panic]
    fn set_axis_out_of_bounds_panics() {
        let mut dev = FakeDevice::new("Stick", 0x1234, 0x5678, 2, 2);
        dev.set_axis(5, 1.0);
    }

    // --- FakeDeviceBackend tests ---

    #[test]
    fn backend_default_config() {
        let backend = FakeDeviceBackend::with_defaults();
        assert_eq!(backend.polling_rate_hz(), 250);
        assert_eq!(backend.jitter_us(), 0);
        assert_eq!(backend.poll_interval_us(), 4_000);
        assert_eq!(backend.device_count(), 0);
    }

    #[test]
    fn backend_custom_config() {
        let config = FakeDeviceBackendConfig {
            polling_rate_hz: 1000,
            jitter_us: 50,
        };
        let backend = FakeDeviceBackend::new(config);
        assert_eq!(backend.polling_rate_hz(), 1000);
        assert_eq!(backend.jitter_us(), 50);
        assert_eq!(backend.poll_interval_us(), 1_050);
    }

    #[test]
    fn backend_add_and_enumerate_devices() {
        let mut backend = FakeDeviceBackend::with_defaults();
        let dev1 = FakeDevice::new("Stick", 0x06a3, 0x0762, 4, 12);
        let dev2 = FakeDevice::new("Throttle", 0x06a3, 0x0763, 6, 8);
        backend.add_device("stick-1", dev1);
        backend.add_device("throttle-1", dev2);

        assert_eq!(backend.device_count(), 2);
        let mut ids = backend.enumerate();
        ids.sort();
        assert_eq!(ids, vec!["stick-1", "throttle-1"]);
    }

    #[test]
    fn backend_enumerate_with_vid_pid() {
        let mut backend = FakeDeviceBackend::with_defaults();
        backend.add_device("dev-a", FakeDevice::new("A", 0x1234, 0x0001, 2, 2));
        let entries = backend.enumerate_with_ids();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, 0x1234);
        assert_eq!(entries[0].2, 0x0001);
    }

    #[test]
    fn backend_remove_device() {
        let mut backend = FakeDeviceBackend::with_defaults();
        backend.add_device("dev-1", FakeDevice::new("X", 0x1, 0x2, 1, 1));
        assert_eq!(backend.device_count(), 1);

        let removed = backend.remove_device("dev-1");
        assert!(removed.is_some());
        assert_eq!(backend.device_count(), 0);
        assert!(backend.remove_device("dev-1").is_none());
    }

    #[test]
    fn backend_connected_events() {
        let mut backend = FakeDeviceBackend::with_defaults();
        backend.add_device("s1", FakeDevice::new("S", 0x1, 0x2, 1, 1));
        backend.remove_device("s1");

        let events = backend.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            DeviceEvent::Connected {
                device_id: "s1".to_owned()
            }
        );
        assert_eq!(
            events[1],
            DeviceEvent::Disconnected {
                device_id: "s1".to_owned()
            }
        );
    }

    #[test]
    fn backend_simulate_reconnect() {
        let mut backend = FakeDeviceBackend::with_defaults();
        let mut dev = FakeDevice::new("Stick", 0x1, 0x2, 1, 1);
        dev.connect();
        backend.add_device("stick", dev);
        backend.drain_events(); // clear add event

        backend.simulate_reconnect("stick");
        let events = backend.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            DeviceEvent::Disconnected {
                device_id: "stick".to_owned()
            }
        );
        assert_eq!(
            events[1],
            DeviceEvent::Connected {
                device_id: "stick".to_owned()
            }
        );
        // Device should be reconnected
        assert!(backend.device("stick").unwrap().connected);
    }

    #[test]
    fn backend_inject_axis() {
        let mut backend = FakeDeviceBackend::with_defaults();
        backend.add_device("dev", FakeDevice::new("D", 0x1, 0x2, 3, 1));
        backend.drain_events();

        backend.inject_axis("dev", 1, 0.75);
        let dev = backend.device("dev").unwrap();
        assert!((dev.axes[1] - 0.75).abs() < f64::EPSILON);

        let events = backend.drain_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            DeviceEvent::InputReceived { device_id, input } => {
                assert_eq!(device_id, "dev");
                assert!((input[1] - 0.75).abs() < f64::EPSILON);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn backend_inject_button() {
        let mut backend = FakeDeviceBackend::with_defaults();
        backend.add_device("dev", FakeDevice::new("D", 0x1, 0x2, 1, 4));
        backend.drain_events();

        backend.inject_button("dev", 2, true);
        let events = backend.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            DeviceEvent::ButtonChanged {
                device_id: "dev".to_owned(),
                index: 2,
                pressed: true,
            }
        );
    }

    #[test]
    fn backend_poll_connected_devices() {
        let mut backend = FakeDeviceBackend::with_defaults();
        let mut dev = FakeDevice::new("Stick", 0x1, 0x2, 2, 1);
        dev.connect();
        dev.enqueue_input(FakeInput {
            axes: vec![0.5, -0.5],
            buttons: vec![true],
            delay_ms: 0,
        });
        backend.add_device("s", dev);
        backend.drain_events();

        let frames = backend.poll();
        assert_eq!(frames, 1);
        assert_eq!(backend.poll_count(), 1);

        let events = backend.drain_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn backend_poll_skips_disconnected_devices() {
        let mut backend = FakeDeviceBackend::with_defaults();
        let mut dev = FakeDevice::new("Stick", 0x1, 0x2, 1, 1);
        dev.enqueue_input(FakeInput {
            axes: vec![0.5],
            buttons: vec![false],
            delay_ms: 0,
        });
        // not connected
        backend.add_device("s", dev);
        backend.drain_events();

        let frames = backend.poll();
        assert_eq!(frames, 0);
    }

    #[test]
    fn backend_device_mut_access() {
        let mut backend = FakeDeviceBackend::with_defaults();
        backend.add_device("dev", FakeDevice::new("D", 0x1, 0x2, 2, 2));
        backend.device_mut("dev").unwrap().set_axis(0, 1.0);
        assert!((backend.device("dev").unwrap().axes[0] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn backend_events_accumulate() {
        let mut backend = FakeDeviceBackend::with_defaults();
        backend.add_device("a", FakeDevice::new("A", 0x1, 0x1, 1, 1));
        backend.add_device("b", FakeDevice::new("B", 0x2, 0x2, 1, 1));
        assert_eq!(backend.events().len(), 2);
        let drained = backend.drain_events();
        assert_eq!(drained.len(), 2);
        assert!(backend.events().is_empty());
    }
}
