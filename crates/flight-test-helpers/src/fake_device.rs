// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! A fake HID device for testing without hardware.

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

#[cfg(test)]
mod tests {
    use super::{FakeDevice, FakeInput};

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
}
