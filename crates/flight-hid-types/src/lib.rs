// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared HID data types used across crates.

/// HID device information
#[derive(Debug, Clone)]
pub struct HidDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub device_path: String,
    pub usage_page: u16,
    pub usage: u16,
    /// Optional HID report descriptor for usage parsing and quirks.
    pub report_descriptor: Option<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hid_device_info_clone() {
        let info = HidDeviceInfo {
            vendor_id: 0x231D,
            product_id: 0x0136,
            serial_number: Some("SN001".to_string()),
            manufacturer: Some("VKB".to_string()),
            product_name: Some("STECS Mini Left".to_string()),
            device_path: "/dev/hidraw0".to_string(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: Some(vec![0x05, 0x01, 0x09, 0x04]),
        };
        let cloned = info.clone();
        assert_eq!(cloned.vendor_id, 0x231D);
        assert_eq!(cloned.product_id, 0x0136);
        assert_eq!(cloned.serial_number.as_deref(), Some("SN001"));
        assert_eq!(cloned.product_name.as_deref(), Some("STECS Mini Left"));
    }

    #[test]
    fn hid_device_info_optional_fields() {
        let info = HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB679,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: "\\\\?\\HID#VID_044F&PID_B679".to_string(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: None,
        };
        assert!(info.serial_number.is_none());
        assert!(info.report_descriptor.is_none());
        assert_eq!(info.usage_page, 0x01);
    }

    #[test]
    fn hid_device_info_with_descriptor() {
        let descriptor = vec![0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
        let info = HidDeviceInfo {
            vendor_id: 0x231D,
            product_id: 0x0138,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: String::new(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: Some(descriptor.clone()),
        };
        assert_eq!(info.report_descriptor.unwrap(), descriptor);
    }

    #[test]
    fn hid_device_info_debug_format() {
        let info = HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0xB679,
            serial_number: None,
            manufacturer: Some("Thrustmaster".to_string()),
            product_name: None,
            device_path: String::new(),
            usage_page: 0,
            usage: 0,
            report_descriptor: None,
        };
        let s = format!("{:?}", info);
        assert!(s.contains("0x044F") || s.contains("1103")); // vendor_id in some form
        assert!(s.contains("Thrustmaster"));
    }

    #[test]
    fn hid_device_info_max_vid_pid() {
        let info = HidDeviceInfo {
            vendor_id: 0xFFFF,
            product_id: 0xFFFF,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: String::new(),
            usage_page: 0xFFFF,
            usage: 0xFFFF,
            report_descriptor: None,
        };
        assert_eq!(info.vendor_id, 0xFFFF);
        assert_eq!(info.product_id, 0xFFFF);
        assert_eq!(info.usage_page, 0xFFFF);
    }
}
