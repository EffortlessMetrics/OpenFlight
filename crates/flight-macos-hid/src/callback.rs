// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Thread-safe event queues for IOKit callbacks.
//!
//! On macOS, IOKit delivers device attach/detach notifications and input
//! reports via C callbacks on the run-loop thread. These queues provide a
//! safe bridge between the callback context and the Rust consumer.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::traits::MacHotplugEvent;

// ── HotplugEventQueue ────────────────────────────────────────────────────

/// Thread-safe FIFO queue for device attach/detach events.
///
/// Shared between the IOKit callback context and the `MacHidManager` poll
/// path via `Arc<Mutex<…>>`.
#[derive(Debug, Clone)]
pub struct HotplugEventQueue {
    inner: Arc<Mutex<VecDeque<MacHotplugEvent>>>,
}

impl HotplugEventQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Push an event (called from IOKit callback context).
    pub fn push(&self, event: MacHotplugEvent) {
        if let Ok(mut q) = self.inner.lock() {
            q.push_back(event);
        }
    }

    /// Drain all pending events.
    pub fn drain(&self) -> Vec<MacHotplugEvent> {
        if let Ok(mut q) = self.inner.lock() {
            q.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|q| q.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for HotplugEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ── InputReport ──────────────────────────────────────────────────────────

/// A single HID input report received from a device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputReport {
    /// HID report ID (0 if the device doesn't use numbered reports).
    pub report_id: u8,
    /// Raw report data.
    pub data: Vec<u8>,
    /// Timestamp in nanoseconds (relative to device open).
    pub timestamp_ns: u64,
}

// ── InputReportQueue ─────────────────────────────────────────────────────

/// Thread-safe bounded FIFO queue for input reports.
///
/// Reports are pushed from the IOKit `IOHIDDeviceRegisterInputReportCallback`
/// and consumed by `MacHidDevice::next_report`. The queue is capped at
/// [`Self::MAX_PENDING`] entries; oldest reports are dropped on overflow
/// (drop-tail policy matching the RT spine design).
#[derive(Debug, Clone)]
pub struct InputReportQueue {
    inner: Arc<Mutex<VecDeque<InputReport>>>,
}

impl InputReportQueue {
    /// Maximum number of buffered reports before drop-tail kicks in.
    pub const MAX_PENDING: usize = 256;

    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(64))),
        }
    }

    /// Push a report, dropping the oldest if the queue is full.
    pub fn push(&self, report: InputReport) {
        if let Ok(mut q) = self.inner.lock() {
            if q.len() >= Self::MAX_PENDING {
                q.pop_front();
            }
            q.push_back(report);
        }
    }

    /// Pop the oldest report, or `None` if empty.
    pub fn pop(&self) -> Option<InputReport> {
        self.inner.lock().ok().and_then(|mut q| q.pop_front())
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|q| q.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Discard all pending reports.
    pub fn clear(&self) {
        if let Ok(mut q) = self.inner.lock() {
            q.clear();
        }
    }
}

impl Default for InputReportQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::HidDeviceInfo;

    fn sample_info() -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB67B,
            product_string: "HOTAS".into(),
            manufacturer_string: "Thrustmaster".into(),
            serial_number: String::new(),
            usage_page: 0x01,
            usage: 0x04,
            location_id: 0,
        }
    }

    // ── HotplugEventQueue ────────────────────────────────────────────

    #[test]
    fn test_hotplug_queue_push_drain() {
        let q = HotplugEventQueue::new();
        assert!(q.is_empty());

        q.push(MacHotplugEvent::Attached(sample_info()));
        assert_eq!(q.len(), 1);

        let events = q.drain();
        assert_eq!(events.len(), 1);
        assert!(q.is_empty());
    }

    #[test]
    fn test_hotplug_queue_multiple_events() {
        let q = HotplugEventQueue::new();
        q.push(MacHotplugEvent::Attached(sample_info()));
        q.push(MacHotplugEvent::Detached {
            vendor_id: 0x044F,
            product_id: 0xB67B,
            location_id: 0,
        });
        assert_eq!(q.len(), 2);

        let events = q.drain();
        assert!(events[0].is_attach());
        assert!(events[1].is_detach());
    }

    #[test]
    fn test_hotplug_queue_clone_shares_state() {
        let q1 = HotplugEventQueue::new();
        let q2 = q1.clone();
        q1.push(MacHotplugEvent::Attached(sample_info()));
        assert_eq!(q2.len(), 1);
    }

    // ── InputReportQueue ─────────────────────────────────────────────

    #[test]
    fn test_report_queue_push_pop() {
        let q = InputReportQueue::new();
        assert!(q.is_empty());
        assert!(q.pop().is_none());

        q.push(InputReport {
            report_id: 1,
            data: vec![0xAA, 0xBB],
            timestamp_ns: 1000,
        });
        assert_eq!(q.len(), 1);

        let r = q.pop().unwrap();
        assert_eq!(r.report_id, 1);
        assert_eq!(r.data, vec![0xAA, 0xBB]);
        assert!(q.is_empty());
    }

    #[test]
    fn test_report_queue_fifo_order() {
        let q = InputReportQueue::new();
        for i in 0..5u8 {
            q.push(InputReport {
                report_id: i,
                data: vec![i],
                timestamp_ns: u64::from(i) * 100,
            });
        }
        for i in 0..5u8 {
            assert_eq!(q.pop().unwrap().report_id, i);
        }
    }

    #[test]
    fn test_report_queue_drop_tail_on_overflow() {
        let q = InputReportQueue::new();
        for i in 0..InputReportQueue::MAX_PENDING + 10 {
            q.push(InputReport {
                report_id: (i & 0xFF) as u8,
                data: vec![],
                timestamp_ns: i as u64,
            });
        }
        // Should be capped at MAX_PENDING
        assert_eq!(q.len(), InputReportQueue::MAX_PENDING);
        // Oldest entries were dropped; first timestamp should be 10
        let r = q.pop().unwrap();
        assert_eq!(r.timestamp_ns, 10);
    }

    #[test]
    fn test_report_queue_clear() {
        let q = InputReportQueue::new();
        q.push(InputReport {
            report_id: 0,
            data: vec![1, 2, 3],
            timestamp_ns: 0,
        });
        assert!(!q.is_empty());
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn test_input_report_clone_eq() {
        let r1 = InputReport {
            report_id: 7,
            data: vec![0x01, 0x02],
            timestamp_ns: 42,
        };
        let r2 = r1.clone();
        assert_eq!(r1, r2);
    }
}
