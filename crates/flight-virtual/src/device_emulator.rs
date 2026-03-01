// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Emulated HID device for testing without physical hardware.
//!
//! [`EmulatedDevice`] mimics a real USB HID controller, accepting raw
//! input reports and producing output reports, with configurable identity
//! (VID/PID, product name) and optional force-feedback support.

use std::collections::VecDeque;

/// Configuration for an [`EmulatedDevice`].
#[derive(Debug, Clone)]
pub struct EmulatedDeviceConfig {
    /// USB Vendor ID.
    pub vid: u16,
    /// USB Product ID.
    pub pid: u16,
    /// Human-readable product name.
    pub product_name: String,
    /// Number of axes the device exposes.
    pub axis_count: usize,
    /// Whether the device advertises force-feedback capability.
    pub ffb_supported: bool,
}

impl Default for EmulatedDeviceConfig {
    fn default() -> Self {
        Self {
            vid: 0x044F,
            pid: 0xB10A,
            product_name: "Emulated Flight Stick".to_string(),
            axis_count: 4,
            ffb_supported: false,
        }
    }
}

/// An emulated HID device for testing.
pub struct EmulatedDevice {
    config: EmulatedDeviceConfig,
    /// Most recently injected raw input report.
    last_input: Option<Vec<u8>>,
    /// Pending output reports waiting to be consumed.
    output_queue: VecDeque<Vec<u8>>,
    /// Parsed axis state from the last injected input report.
    axes: Vec<f64>,
    /// Total number of input reports injected.
    input_count: u64,
    /// Total number of output reports enqueued.
    output_count: u64,
}

impl EmulatedDevice {
    /// Create a new emulated device with the given configuration.
    pub fn new(config: EmulatedDeviceConfig) -> Self {
        let axis_count = config.axis_count;
        Self {
            config,
            last_input: None,
            output_queue: VecDeque::new(),
            axes: vec![0.0; axis_count],
            input_count: 0,
            output_count: 0,
        }
    }

    /// Borrow the configuration.
    pub fn config(&self) -> &EmulatedDeviceConfig {
        &self.config
    }

    /// USB Vendor ID.
    pub fn vid(&self) -> u16 {
        self.config.vid
    }

    /// USB Product ID.
    pub fn pid(&self) -> u16 {
        self.config.pid
    }

    /// Product name string.
    pub fn product_name(&self) -> &str {
        &self.config.product_name
    }

    /// Whether this emulated device advertises FFB.
    pub fn supports_ffb(&self) -> bool {
        self.config.ffb_supported
    }

    /// Inject a raw HID input report.
    ///
    /// The report is parsed to update internal axis state.  Each axis is
    /// expected as a little-endian `u16` starting at byte offset 1 (byte 0
    /// is the report ID), normalised to `[-1.0, 1.0]`.
    pub fn inject_input(&mut self, report: &[u8]) {
        self.last_input = Some(report.to_vec());
        self.input_count += 1;

        // Parse axes from report (skip report ID byte).
        let data = if report.is_empty() { &[] } else { &report[1..] };
        for (i, slot) in self.axes.iter_mut().enumerate() {
            let offset = i * 2;
            if offset + 1 < data.len() {
                let raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
                // Map 0..65535 → -1.0..1.0
                *slot = (raw as f64 / 32767.5) - 1.0;
            }
        }
    }

    /// Enqueue a raw HID output report (e.g. LED or FFB command).
    pub fn enqueue_output(&mut self, report: Vec<u8>) {
        self.output_count += 1;
        self.output_queue.push_back(report);
    }

    /// Retrieve the next pending output report, if any.
    pub fn get_output(&mut self) -> Option<Vec<u8>> {
        self.output_queue.pop_front()
    }

    /// Return the most recently injected raw input report.
    pub fn last_input_report(&self) -> Option<&[u8]> {
        self.last_input.as_deref()
    }

    /// Return the current parsed axis value for the given index.
    pub fn get_axis(&self, index: usize) -> Option<f64> {
        self.axes.get(index).copied()
    }

    /// Total input reports injected so far.
    pub fn input_count(&self) -> u64 {
        self.input_count
    }

    /// Total output reports enqueued so far.
    pub fn output_count(&self) -> u64 {
        self.output_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_axis_report(values: &[u16]) -> Vec<u8> {
        let mut report = vec![0x01]; // report ID
        for &v in values {
            report.extend_from_slice(&v.to_le_bytes());
        }
        report
    }

    #[test]
    fn test_default_config() {
        let dev = EmulatedDevice::new(EmulatedDeviceConfig::default());
        assert_eq!(dev.vid(), 0x044F);
        assert_eq!(dev.pid(), 0xB10A);
        assert_eq!(dev.product_name(), "Emulated Flight Stick");
        assert!(!dev.supports_ffb());
    }

    #[test]
    fn test_device_identification() {
        let cfg = EmulatedDeviceConfig {
            vid: 0x1234,
            pid: 0x5678,
            product_name: "Test Stick".to_string(),
            ..Default::default()
        };
        let dev = EmulatedDevice::new(cfg);
        assert_eq!(dev.vid(), 0x1234);
        assert_eq!(dev.pid(), 0x5678);
        assert_eq!(dev.product_name(), "Test Stick");
    }

    #[test]
    fn test_ffb_support_flag() {
        let dev_no = EmulatedDevice::new(EmulatedDeviceConfig::default());
        assert!(!dev_no.supports_ffb());

        let dev_yes = EmulatedDevice::new(EmulatedDeviceConfig {
            ffb_supported: true,
            ..Default::default()
        });
        assert!(dev_yes.supports_ffb());
    }

    #[test]
    fn test_inject_input_stores_report() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig::default());
        assert!(dev.last_input_report().is_none());

        let report = vec![0x01, 0xFF, 0x7F];
        dev.inject_input(&report);
        assert_eq!(dev.last_input_report().unwrap(), &report[..]);
        assert_eq!(dev.input_count(), 1);
    }

    #[test]
    fn test_input_parses_axes() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig {
            axis_count: 2,
            ..Default::default()
        });

        // Center value: 32768 → ~0.0
        let report = make_axis_report(&[32768, 32768]);
        dev.inject_input(&report);

        let a0 = dev.get_axis(0).unwrap();
        assert!(a0.abs() < 0.01, "expected ~0, got {a0}");
    }

    #[test]
    fn test_input_parses_axis_extremes() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig {
            axis_count: 2,
            ..Default::default()
        });

        // 0 → -1.0, 65535 → +1.0
        let report = make_axis_report(&[0, 65535]);
        dev.inject_input(&report);

        let lo = dev.get_axis(0).unwrap();
        assert!((lo - (-1.0)).abs() < 0.01, "expected ~-1.0, got {lo}");

        let hi = dev.get_axis(1).unwrap();
        assert!((hi - 1.0).abs() < 0.01, "expected ~1.0, got {hi}");
    }

    #[test]
    fn test_output_enqueue_and_retrieve() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig::default());
        assert!(dev.get_output().is_none());

        dev.enqueue_output(vec![0x02, 0x01]);
        dev.enqueue_output(vec![0x02, 0x02]);

        assert_eq!(dev.output_count(), 2);

        let first = dev.get_output().unwrap();
        assert_eq!(first, vec![0x02, 0x01]);

        let second = dev.get_output().unwrap();
        assert_eq!(second, vec![0x02, 0x02]);

        assert!(dev.get_output().is_none());
    }

    #[test]
    fn test_oob_axis_returns_none() {
        let dev = EmulatedDevice::new(EmulatedDeviceConfig {
            axis_count: 2,
            ..Default::default()
        });
        assert!(dev.get_axis(99).is_none());
    }

    #[test]
    fn test_empty_report_injection() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig::default());
        dev.inject_input(&[]);
        assert_eq!(dev.last_input_report().unwrap(), &[] as &[u8]);
        assert_eq!(dev.input_count(), 1);
    }

    #[test]
    fn test_multiple_injections_overwrite() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig {
            axis_count: 1,
            ..Default::default()
        });

        dev.inject_input(&make_axis_report(&[0]));
        let v1 = dev.get_axis(0).unwrap();

        dev.inject_input(&make_axis_report(&[65535]));
        let v2 = dev.get_axis(0).unwrap();

        assert!(v2 > v1, "second injection should overwrite");
        assert_eq!(dev.input_count(), 2);
    }

    #[test]
    fn test_partial_report_does_not_panic() {
        let mut dev = EmulatedDevice::new(EmulatedDeviceConfig {
            axis_count: 4,
            ..Default::default()
        });
        // Only enough data for 1 axis — should not panic.
        dev.inject_input(&[0x01, 0x00, 0x80]);
        assert_eq!(dev.input_count(), 1);
    }
}
