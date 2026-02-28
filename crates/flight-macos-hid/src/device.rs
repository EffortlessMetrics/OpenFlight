// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID device info and device handle.

use crate::HidError;
use crate::callback::{InputReport, InputReportQueue};
use crate::traits::MacInputReportReader;

/// Static metadata about a discovered HID device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_string: String,
    pub manufacturer_string: String,
    pub serial_number: String,
    pub usage_page: u16,
    pub usage: u16,
    pub location_id: u32,
}

// ═══════════════════════════════════════════════════════════════════════════
// MacHidDevice
// ═══════════════════════════════════════════════════════════════════════════

/// An open HID device handle.
///
/// On **macOS** this wraps an `IOHIDDeviceRef` with a registered
/// `IOHIDDeviceRegisterInputReportCallback` for async report delivery.
/// Reports are buffered in an [`InputReportQueue`] and consumed via
/// [`MacInputReportReader::next_report`].
///
/// On **other platforms** this is a mock that supports report injection
/// for cross-platform testing.
#[derive(Debug)]
pub struct MacHidDevice {
    info: HidDeviceInfo,
    report_queue: InputReportQueue,
    opened: bool,

    // -- macOS: IOKit device handle --
    #[cfg(target_os = "macos")]
    device_ref: crate::ffi::IOHIDDeviceRef,
    #[cfg(target_os = "macos")]
    report_buffer: Vec<u8>,
}

// ── macOS implementation ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
impl MacHidDevice {
    /// Maximum report buffer size (bytes).
    const MAX_REPORT_SIZE: usize = 1024;

    /// Open a device given its discovery info.
    pub fn open(info: &HidDeviceInfo) -> Result<Self, HidError> {
        // In a full implementation, we would look up the IOHIDDeviceRef
        // from the IOHIDManager's device set. For now, this is the
        // structural implementation that would be completed with a
        // device-ref lookup.
        Err(HidError::OpenFailed { code: -1 })
    }

    /// Open a device from a raw IOKit device reference.
    ///
    /// # Safety
    ///
    /// `device_ref` must be a valid `IOHIDDeviceRef` obtained from an
    /// `IOHIDManager` device set.
    pub unsafe fn open_from_ref(
        info: &HidDeviceInfo,
        device_ref: crate::ffi::IOHIDDeviceRef,
    ) -> Result<Self, HidError> {
        use crate::ffi;

        let ret = unsafe { ffi::IOHIDDeviceOpen(device_ref, ffi::K_IOHID_OPTIONS_TYPE_NONE) };
        if ret != ffi::K_IO_RETURN_SUCCESS {
            return Err(HidError::OpenFailed { code: ret });
        }

        let report_queue = InputReportQueue::new();
        let mut report_buffer = vec![0u8; Self::MAX_REPORT_SIZE];

        // Register input report callback.
        let ctx = Box::into_raw(Box::new(report_queue.clone())) as *mut std::os::raw::c_void;
        unsafe {
            ffi::IOHIDDeviceRegisterInputReportCallback(
                device_ref,
                report_buffer.as_mut_ptr(),
                report_buffer.len() as isize,
                input_report_callback,
                ctx,
            );
        }

        // Schedule on current run loop.
        let run_loop = core_foundation::runloop::CFRunLoop::get_current();
        let mode = unsafe { core_foundation_sys::runloop::kCFRunLoopDefaultMode };
        unsafe {
            ffi::IOHIDDeviceScheduleWithRunLoop(
                device_ref,
                core_foundation::base::TCFType::as_concrete_TypeRef(&run_loop),
                mode,
            );
        }

        Ok(Self {
            info: info.clone(),
            report_queue,
            opened: true,
            device_ref,
            report_buffer,
        })
    }

    /// Write an output report to the device.
    pub fn write_report(&self, data: &[u8]) -> Result<(), HidError> {
        use crate::ffi;
        if !self.opened {
            return Err(HidError::NotOpen);
        }
        let report_id = if data.is_empty() { 0 } else { data[0] as isize };
        let ret = unsafe {
            ffi::IOHIDDeviceSetReport(
                self.device_ref,
                ffi::K_IOHID_REPORT_TYPE_OUTPUT,
                report_id,
                data.as_ptr(),
                data.len() as isize,
            )
        };
        if ret != ffi::K_IO_RETURN_SUCCESS {
            return Err(HidError::WriteFailed { code: ret });
        }
        Ok(())
    }
}

/// IOKit input report callback.
#[cfg(target_os = "macos")]
unsafe extern "C" fn input_report_callback(
    context: *mut std::os::raw::c_void,
    _result: crate::ffi::IOReturn,
    _sender: crate::ffi::IOHIDDeviceRef,
    _report_type: u32,
    report_id: u32,
    report: *const u8,
    report_length: isize,
) {
    if context.is_null() || report.is_null() || report_length <= 0 {
        return;
    }
    let queue = unsafe { &*(context as *const InputReportQueue) };
    let data = unsafe { std::slice::from_raw_parts(report, report_length as usize) }.to_vec();
    queue.push(InputReport {
        report_id: report_id as u8,
        data,
        timestamp_ns: 0,
    });
}

// ── non-macOS mock implementation ────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
impl MacHidDevice {
    /// Open a mock device (non-macOS).
    pub fn open(info: &HidDeviceInfo) -> Result<Self, HidError> {
        Ok(Self {
            info: info.clone(),
            report_queue: InputReportQueue::new(),
            opened: true,
        })
    }

    /// Write an output report (mock — always succeeds).
    pub fn write_report(&self, _data: &[u8]) -> Result<(), HidError> {
        if !self.opened {
            return Err(HidError::NotOpen);
        }
        Ok(())
    }

    /// Inject a mock input report (test helper, non-macOS only).
    pub fn inject_report(&self, report_id: u8, data: Vec<u8>) {
        self.report_queue.push(InputReport {
            report_id,
            data,
            timestamp_ns: 0,
        });
    }

    /// Inject a mock input report with a timestamp.
    pub fn inject_report_with_timestamp(&self, report_id: u8, data: Vec<u8>, timestamp_ns: u64) {
        self.report_queue.push(InputReport {
            report_id,
            data,
            timestamp_ns,
        });
    }
}

// ── Shared (all platforms) ───────────────────────────────────────────────

impl MacHidDevice {
    /// Device metadata.
    pub fn info(&self) -> &HidDeviceInfo {
        &self.info
    }

    /// Whether the device handle is open.
    pub fn is_open(&self) -> bool {
        self.opened
    }

    /// Read an input report into `buf`, returning bytes read.
    ///
    /// Non-blocking. Returns 0 if no report is available.
    pub fn read_report(&self, buf: &mut [u8]) -> Result<usize, HidError> {
        if !self.opened {
            return Err(HidError::NotOpen);
        }
        match self.report_queue.pop() {
            Some(report) => {
                let n = report.data.len().min(buf.len());
                buf[..n].copy_from_slice(&report.data[..n]);
                Ok(n)
            }
            None => Ok(0),
        }
    }

    /// Access the underlying report queue.
    pub fn report_queue(&self) -> &InputReportQueue {
        &self.report_queue
    }
}

// ── Trait implementation ─────────────────────────────────────────────────

impl MacInputReportReader for MacHidDevice {
    fn next_report(&mut self) -> Option<(u8, Vec<u8>)> {
        self.report_queue.pop().map(|r| (r.report_id, r.data))
    }
}

/// Backward-compatible alias.
pub type HidDevice = MacHidDevice;

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_info() -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB67B,
            product_string: "T.Flight HOTAS 4".into(),
            manufacturer_string: "Thrustmaster".into(),
            serial_number: String::new(),
            usage_page: 0x01,
            usage: 0x04,
            location_id: 0,
        }
    }

    // ── HidDeviceInfo ────────────────────────────────────────────────

    #[test]
    fn test_device_info_clone() {
        let info = dummy_info();
        let info2 = info.clone();
        assert_eq!(info.vendor_id, info2.vendor_id);
        assert_eq!(info.product_string, info2.product_string);
    }

    #[test]
    fn test_device_info_eq() {
        let a = dummy_info();
        let b = dummy_info();
        assert_eq!(a, b);
    }

    // ── MacHidDevice open ────────────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_open_mock_device() {
        let dev = MacHidDevice::open(&dummy_info()).unwrap();
        assert!(dev.is_open());
        assert_eq!(dev.info().vendor_id, 0x044F);
    }

    // ── Mock report injection ────────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_inject_and_read_report() {
        let dev = MacHidDevice::open(&dummy_info()).unwrap();
        dev.inject_report(1, vec![0xAA, 0xBB, 0xCC]);

        let mut buf = [0u8; 64];
        let n = dev.read_report(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf[..3], &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_read_report_empty() {
        let dev = MacHidDevice::open(&dummy_info()).unwrap();
        let mut buf = [0u8; 64];
        let n = dev.read_report(&mut buf).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_multiple_reports_fifo() {
        let dev = MacHidDevice::open(&dummy_info()).unwrap();
        dev.inject_report(1, vec![0x01]);
        dev.inject_report(2, vec![0x02]);
        dev.inject_report(3, vec![0x03]);

        let mut buf = [0u8; 64];
        assert_eq!(dev.read_report(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], 0x01);
        assert_eq!(dev.read_report(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], 0x02);
        assert_eq!(dev.read_report(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], 0x03);
    }

    // ── Trait: MacInputReportReader ──────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_input_report_reader_trait() {
        let mut dev = MacHidDevice::open(&dummy_info()).unwrap();
        dev.inject_report(5, vec![0xDE, 0xAD]);

        let (id, data) = MacInputReportReader::next_report(&mut dev).unwrap();
        assert_eq!(id, 5);
        assert_eq!(data, vec![0xDE, 0xAD]);

        assert!(MacInputReportReader::next_report(&mut dev).is_none());
    }

    // ── Write report ─────────────────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_write_report_mock() {
        let dev = MacHidDevice::open(&dummy_info()).unwrap();
        dev.write_report(&[0x01, 0x02, 0x03]).unwrap();
    }

    // ── Report with timestamp ────────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_inject_report_with_timestamp() {
        let dev = MacHidDevice::open(&dummy_info()).unwrap();
        dev.inject_report_with_timestamp(1, vec![0xBE, 0xEF], 42_000);

        let report = dev.report_queue().pop().unwrap();
        assert_eq!(report.report_id, 1);
        assert_eq!(report.data, vec![0xBE, 0xEF]);
        assert_eq!(report.timestamp_ns, 42_000);
    }

    // ── Type alias ───────────────────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_hid_device_alias() {
        let _dev: HidDevice = MacHidDevice::open(&dummy_info()).unwrap();
    }

    // ── Buffer truncation ────────────────────────────────────────────

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_read_report_truncates_to_buffer() {
        let dev = MacHidDevice::open(&dummy_info()).unwrap();
        dev.inject_report(0, vec![1, 2, 3, 4, 5, 6, 7, 8]);

        let mut small_buf = [0u8; 4];
        let n = dev.read_report(&mut small_buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(small_buf, [1, 2, 3, 4]);
    }
}
