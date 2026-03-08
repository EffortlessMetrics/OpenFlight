// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane SDK (XPLM) bindings and safe wrappers.
//!
//! In non-test builds the `extern "C"` declarations bind to the real XPLM
//! functions provided by the X-Plane host process at plugin load time.
//! Under `#[cfg(test)]`, a thread-local mock registry is used instead so
//! that bridge logic can be unit-tested without the simulator.

#[cfg(any(test, feature = "test-support"))]
use std::ffi::c_void;
#[cfg(not(any(test, feature = "test-support")))]
use std::ffi::{CString, c_char, c_void};

// ── XPLM opaque handles ────────────────────────────────────────────────────

/// Opaque handle returned by `XPLMFindDataRef`.
pub type XPLMDataRef = *mut c_void;

/// Opaque handle returned by `XPLMFindCommand`.
pub type XPLMCommandRef = *mut c_void;

// ── DataRef type flags (from XPLMDataAccess.h) ─────────────────────────────

pub const XPLM_TYPE_INT: i32 = 1;
pub const XPLM_TYPE_FLOAT: i32 = 2;
pub const XPLM_TYPE_DOUBLE: i32 = 4;
pub const XPLM_TYPE_FLOAT_ARRAY: i32 = 8;
pub const XPLM_TYPE_INT_ARRAY: i32 = 16;
pub const XPLM_TYPE_DATA: i32 = 32;

// ── Real XPLM extern declarations (resolved by X-Plane at load time) ──────

#[cfg(not(any(test, feature = "test-support")))]
unsafe extern "C" {
    fn XPLMFindDataRef(name: *const c_char) -> XPLMDataRef;
    fn XPLMGetDataRefTypes(dataref: XPLMDataRef) -> i32;
    fn XPLMGetDatai(dataref: XPLMDataRef) -> i32;
    fn XPLMGetDataf(dataref: XPLMDataRef) -> f32;
    fn XPLMGetDatad(dataref: XPLMDataRef) -> f64;
    fn XPLMSetDatai(dataref: XPLMDataRef, value: i32);
    fn XPLMSetDataf(dataref: XPLMDataRef, value: f32);
    fn XPLMSetDatad(dataref: XPLMDataRef, value: f64);
    fn XPLMGetDatab(dataref: XPLMDataRef, out_buf: *mut c_void, offset: i32, max_len: i32) -> i32;
    fn XPLMFindCommand(name: *const c_char) -> XPLMCommandRef;
    fn XPLMCommandOnce(command: XPLMCommandRef);
}

// ── Mock backend (test builds) ─────────────────────────────────────────────

#[cfg(any(test, feature = "test-support"))]
pub mod mock {
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// A mock DataRef value stored in the thread-local registry.
    #[derive(Clone, Debug)]
    pub enum MockValue {
        Int(i32),
        Float(f32),
        Double(f64),
        Data(Vec<u8>),
    }

    thread_local! {
        static MOCK_DATAREFS: RefCell<HashMap<String, MockValue>> = RefCell::new(HashMap::new());
        static MOCK_COMMANDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    /// Register a mock DataRef value.
    pub fn set_mock_dataref(name: &str, value: MockValue) {
        MOCK_DATAREFS.with(|m| m.borrow_mut().insert(name.to_string(), value));
    }

    /// Clear all mock state (call at the start of each test).
    pub fn clear_mocks() {
        MOCK_DATAREFS.with(|m| m.borrow_mut().clear());
        MOCK_COMMANDS.with(|c| c.borrow_mut().clear());
    }

    /// Read a mock DataRef by name.
    pub fn get_mock_dataref(name: &str) -> Option<MockValue> {
        MOCK_DATAREFS.with(|m| m.borrow().get(name).cloned())
    }

    /// Record that a command was executed.
    pub fn record_command(name: &str) {
        MOCK_COMMANDS.with(|c| c.borrow_mut().push(name.to_string()));
    }

    /// Return all commands executed since the last `clear_mocks`.
    pub fn executed_commands() -> Vec<String> {
        MOCK_COMMANDS.with(|c| c.borrow().clone())
    }
}

#[cfg(any(test, feature = "test-support"))]
pub use mock::{MockValue, clear_mocks, executed_commands, set_mock_dataref};

// ── Safe wrappers ──────────────────────────────────────────────────────────

/// Read a DataRef and return its value as JSON.
///
/// Returns `None` if the DataRef does not exist.
pub fn read_dataref(name: &str) -> Option<serde_json::Value> {
    #[cfg(not(any(test, feature = "test-support")))]
    {
        let c_name = CString::new(name).ok()?;
        unsafe {
            let dr = XPLMFindDataRef(c_name.as_ptr());
            if dr.is_null() {
                return None;
            }
            let types = XPLMGetDataRefTypes(dr);
            // Prefer higher-precision types first.
            if types & XPLM_TYPE_DOUBLE != 0 {
                Some(serde_json::json!(XPLMGetDatad(dr)))
            } else if types & XPLM_TYPE_FLOAT != 0 {
                Some(serde_json::json!(XPLMGetDataf(dr) as f64))
            } else if types & XPLM_TYPE_INT != 0 {
                Some(serde_json::json!(XPLMGetDatai(dr)))
            } else {
                None
            }
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    {
        mock::get_mock_dataref(name).map(|v| match v {
            MockValue::Int(i) => serde_json::json!(i),
            MockValue::Float(f) => serde_json::json!(f as f64),
            MockValue::Double(d) => serde_json::json!(d),
            MockValue::Data(bytes) => {
                serde_json::json!(String::from_utf8_lossy(&bytes).to_string())
            }
        })
    }
}

/// Read a byte-array DataRef as a UTF-8 string (e.g. aircraft ICAO, description).
///
/// Returns `None` if the DataRef does not exist.
pub fn read_dataref_string(name: &str) -> Option<String> {
    #[cfg(not(any(test, feature = "test-support")))]
    {
        let c_name = CString::new(name).ok()?;
        unsafe {
            let dr = XPLMFindDataRef(c_name.as_ptr());
            if dr.is_null() {
                return None;
            }
            let mut buf = [0u8; 512];
            let len = XPLMGetDatab(dr, buf.as_mut_ptr() as *mut c_void, 0, buf.len() as i32);
            if len <= 0 {
                return None;
            }
            let slice = &buf[..len as usize];
            // Trim trailing NULs that X-Plane pads.
            let trimmed = match slice.iter().position(|&b| b == 0) {
                Some(pos) => &slice[..pos],
                None => slice,
            };
            Some(String::from_utf8_lossy(trimmed).into_owned())
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    {
        mock::get_mock_dataref(name).and_then(|v| match v {
            MockValue::Data(bytes) => {
                let trimmed = match bytes.iter().position(|&b| b == 0) {
                    Some(pos) => &bytes[..pos],
                    None => &bytes,
                };
                Some(String::from_utf8_lossy(trimmed).into_owned())
            }
            _ => None,
        })
    }
}

/// Write a JSON value to a DataRef.
///
/// Returns `true` on success, `false` if the DataRef is not found or the
/// value type is incompatible.
pub fn write_dataref(name: &str, value: &serde_json::Value) -> bool {
    #[cfg(not(any(test, feature = "test-support")))]
    {
        let Some(c_name) = CString::new(name).ok() else {
            return false;
        };
        unsafe {
            let dr = XPLMFindDataRef(c_name.as_ptr());
            if dr.is_null() {
                return false;
            }
            let types = XPLMGetDataRefTypes(dr);
            if let Some(num) = value.as_f64() {
                if types & XPLM_TYPE_DOUBLE != 0 {
                    XPLMSetDatad(dr, num);
                    return true;
                }
                if types & XPLM_TYPE_FLOAT != 0 {
                    XPLMSetDataf(dr, num as f32);
                    return true;
                }
                if types & XPLM_TYPE_INT != 0 {
                    XPLMSetDatai(dr, num as i32);
                    return true;
                }
            }
            if let Some(i) = value.as_i64()
                && types & XPLM_TYPE_INT != 0
            {
                XPLMSetDatai(dr, i as i32);
                return true;
            }
            false
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    {
        if mock::get_mock_dataref(name).is_none() {
            return false;
        }
        // Coerce the JSON value to a MockValue and store it.
        let mock_val = if let Some(f) = value.as_f64() {
            MockValue::Float(f as f32)
        } else if let Some(i) = value.as_i64() {
            MockValue::Int(i as i32)
        } else {
            return false;
        };
        mock::set_mock_dataref(name, mock_val);
        true
    }
}

/// Find and execute an X-Plane command by name.
///
/// Returns `true` if the command was found and executed, `false` otherwise.
pub fn execute_command(name: &str) -> bool {
    #[cfg(not(any(test, feature = "test-support")))]
    {
        let Some(c_name) = CString::new(name).ok() else {
            return false;
        };
        unsafe {
            let cmd = XPLMFindCommand(c_name.as_ptr());
            if cmd.is_null() {
                return false;
            }
            XPLMCommandOnce(cmd);
            true
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    {
        mock::record_command(name);
        true
    }
}

/// Current timestamp in milliseconds since UNIX epoch.
pub fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_dataref_float() {
        clear_mocks();
        set_mock_dataref("sim/test/float", MockValue::Float(42.5));
        let val = read_dataref("sim/test/float").unwrap();
        assert!((val.as_f64().unwrap() - 42.5).abs() < 0.01);
    }

    #[test]
    fn read_dataref_int() {
        clear_mocks();
        set_mock_dataref("sim/test/int", MockValue::Int(7));
        let val = read_dataref("sim/test/int").unwrap();
        assert_eq!(val.as_i64().unwrap(), 7);
    }

    #[test]
    fn read_dataref_double() {
        clear_mocks();
        set_mock_dataref("sim/test/double", MockValue::Double(4.56789));
        let val = read_dataref("sim/test/double").unwrap();
        assert!((val.as_f64().unwrap() - 4.56789).abs() < 1e-6);
    }

    #[test]
    fn read_dataref_not_found_returns_none() {
        clear_mocks();
        assert!(read_dataref("sim/nonexistent").is_none());
    }

    #[test]
    fn read_dataref_string_from_byte_data() {
        clear_mocks();
        set_mock_dataref(
            "sim/aircraft/view/acf_ICAO",
            MockValue::Data(b"C172\0".to_vec()),
        );
        let s = read_dataref_string("sim/aircraft/view/acf_ICAO").unwrap();
        assert_eq!(s, "C172");
    }

    #[test]
    fn read_dataref_string_not_found() {
        clear_mocks();
        assert!(read_dataref_string("sim/nonexistent").is_none());
    }

    #[test]
    fn write_dataref_updates_value() {
        clear_mocks();
        set_mock_dataref("sim/test/writable", MockValue::Float(0.0));
        assert!(write_dataref("sim/test/writable", &serde_json::json!(99.5)));
        let val = read_dataref("sim/test/writable").unwrap();
        assert!((val.as_f64().unwrap() - 99.5).abs() < 0.1);
    }

    #[test]
    fn write_dataref_not_found_returns_false() {
        clear_mocks();
        assert!(!write_dataref("sim/nonexistent", &serde_json::json!(1.0)));
    }

    #[test]
    fn execute_command_records_command() {
        clear_mocks();
        assert!(execute_command("sim/autopilot/heading_sync"));
        let cmds = executed_commands();
        assert_eq!(cmds, vec!["sim/autopilot/heading_sync"]);
    }

    #[test]
    fn timestamp_ms_returns_nonzero() {
        assert!(timestamp_ms() > 0);
    }
}
