// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane plugin entry points
//!
//! This crate compiles to a cdylib (`.xpl`) loaded by X-Plane. It spawns a
//! background thread that connects to the Flight Hub plugin interface TCP
//! server (localhost:52000) and relays DataRef values and aircraft info.
//!
//! # Building
//!
//! ```sh
//! # Windows (.xpl = .dll renamed)
//! cargo build --release --target x86_64-pc-windows-msvc
//! copy target\x86_64-pc-windows-msvc\release\flight_xplane_plugin.dll \
//!      "<X-Plane>/Resources/plugins/FlightHub/win.xpl"
//!
//! # Linux
//! cargo build --release --target x86_64-unknown-linux-gnu
//! cp target/x86_64-unknown-linux-gnu/release/libflight_xplane_plugin.so \
//!    "<X-Plane>/Resources/plugins/FlightHub/lin.xpl"
//!
//! # macOS (arm64 + x86_64 universal)
//! cargo build --release --target aarch64-apple-darwin
//! cargo build --release --target x86_64-apple-darwin
//! lipo -create -output mac.xpl ...
//! cp mac.xpl "<X-Plane>/Resources/plugins/FlightHub/mac.xpl"
//! ```
//!
//! # X-Plane SDK
//!
//! The XPLM type definitions below are manually transcribed from the
//! X-Plane Plugin SDK 4.0 headers. When the official `xplane-sdk` crate or
//! `xplane-sys` bindings become available in the workspace, replace these
//! stubs with the crate types.
//!
//! SDK reference: <https://developer.x-plane.com/sdk/>

mod bridge;
mod protocol;

use bridge::Bridge;
use std::sync::{Mutex, OnceLock};

static BRIDGE: OnceLock<Mutex<Option<Bridge>>> = OnceLock::new();

/// Maximum length of plugin name / description / signature strings (XPLM).
const XPLM_MSG_BUF: usize = 256;

/// Called by X-Plane when the plugin is first loaded.
///
/// # Safety
/// This function is called from C by X-Plane with raw pointer buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPluginStart(
    out_name: *mut std::ffi::c_char,
    out_sig: *mut std::ffi::c_char,
    out_desc: *mut std::ffi::c_char,
) -> i32 {
    unsafe {
        write_cstr(out_name, "FlightHub", XPLM_MSG_BUF);
        write_cstr(out_sig, "org.openflight.flighthub", XPLM_MSG_BUF);
        write_cstr(
            out_desc,
            "Bridges X-Plane to Flight Hub (OpenFlight)",
            XPLM_MSG_BUF,
        );
    }

    BRIDGE.get_or_init(|| Mutex::new(None));
    1 // success
}

/// Called by X-Plane when the plugin is unloaded.
///
/// # Safety
/// Called from C with no arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPluginStop() {
    if let Some(cell) = BRIDGE.get()
        && let Ok(mut guard) = cell.lock()
        && let Some(bridge) = guard.take()
    {
        bridge.shutdown();
    }
}

/// Called by X-Plane when the plugin is enabled (after XPluginStart).
///
/// # Safety
/// Called from C with no arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPluginEnable() -> i32 {
    if let Some(cell) = BRIDGE.get()
        && let Ok(mut guard) = cell.lock()
    {
        match Bridge::start() {
            Ok(b) => {
                *guard = Some(b);
                return 1;
            }
            Err(e) => {
                eprintln!("[FlightHub] Failed to start bridge: {}", e);
            }
        }
    }
    0 // failure
}

/// Called by X-Plane when the plugin is disabled.
///
/// # Safety
/// Called from C with no arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPluginDisable() {
    if let Some(cell) = BRIDGE.get()
        && let Ok(mut guard) = cell.lock()
        && let Some(bridge) = guard.take()
    {
        bridge.shutdown();
    }
}

/// Called by X-Plane to deliver inter-plugin messages.
///
/// # Safety
/// Called from C with raw pointer param.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPluginReceiveMessage(
    _from: i32,
    _message: i32,
    _param: *mut std::ffi::c_void,
) {
    // Reserved for future use (e.g. aircraft loaded message = 103).
}

// ── helpers ──────────────────────────────────────────────────────────────────

unsafe fn write_cstr(buf: *mut std::ffi::c_char, s: &str, max: usize) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(max - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const std::ffi::c_char, buf, len);
        *buf.add(len) = 0;
    }
}
