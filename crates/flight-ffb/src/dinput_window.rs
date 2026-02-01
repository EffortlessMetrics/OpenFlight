// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Message-only window for DirectInput cooperative level
//!
//! DirectInput requires a valid HWND for `SetCooperativeLevel` to enable exclusive
//! access mode (required for force feedback). For headless services like flightd,
//! we create a message-only window using `HWND_MESSAGE` as the parent.
//!
//! Message-only windows:
//! - Are not visible and cannot receive broadcast messages
//! - Don't appear in the window hierarchy
//! - Are perfect for background services that need an HWND
//!
//! # Safety
//!
//! Window handles must be used on the thread that created them. The `MessageOnlyWindow`
//! struct is `!Send` and `!Sync` to enforce this at compile time.

#![allow(dead_code)]
// Allow unsafe operations in unsafe fns for window procedure
#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
use std::cell::Cell;
#[cfg(windows)]
use std::ptr;
#[cfg(windows)]
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(windows)]
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_MESSAGE,
    RegisterClassExW, UnregisterClassW, WINDOW_EX_STYLE, WNDCLASSEXW, WS_OVERLAPPED,
};
#[cfg(windows)]
use windows::core::{Error, PCWSTR};

/// Error type for window operations
#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    #[error("Failed to get module handle: {0}")]
    ModuleHandleError(String),

    #[error("Failed to register window class: {0}")]
    ClassRegistrationError(String),

    #[error("Failed to create window: {0}")]
    WindowCreationError(String),

    #[error("Window has been destroyed")]
    WindowDestroyed,

    #[error("Platform not supported (Windows only)")]
    PlatformNotSupported,
}

pub type Result<T> = std::result::Result<T, WindowError>;

/// Global counter for unique window class names
#[cfg(windows)]
static CLASS_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Message-only window for DirectInput
///
/// This window is invisible and only used to satisfy DirectInput's HWND requirement
/// for `SetCooperativeLevel`. The window is automatically destroyed when this
/// struct is dropped.
///
/// # Thread Safety
///
/// Windows HWNDs must be used on the thread that created them. This struct is
/// marked `!Send` and `!Sync` via a `PhantomData<Cell<()>>` to prevent it from
/// being moved or shared across threads.
#[cfg(windows)]
pub struct MessageOnlyWindow {
    hwnd: HWND,
    class_atom: u16,
    class_name: Vec<u16>,
    hinstance: HINSTANCE,
    /// Marker to make this struct !Send and !Sync
    _not_send_sync: std::marker::PhantomData<Cell<()>>,
}

#[cfg(windows)]
impl MessageOnlyWindow {
    /// Create a new message-only window
    ///
    /// # Returns
    /// * `Result<Self>` - New window instance or error
    ///
    /// # Example
    /// ```ignore
    /// let window = MessageOnlyWindow::new()?;
    /// let hwnd = window.hwnd();
    /// // Use hwnd with SetCooperativeLevel
    /// ```
    pub fn new() -> Result<Self> {
        unsafe {
            // Get the module handle for the current process
            let hmodule = GetModuleHandleW(PCWSTR::null()).map_err(|e| {
                WindowError::ModuleHandleError(format!("GetModuleHandleW failed: {}", e))
            })?;
            // Convert HMODULE to HINSTANCE (they're the same underlying type)
            let hinstance = HINSTANCE(hmodule.0);

            // Generate a unique class name to avoid conflicts
            let class_id = CLASS_COUNTER.fetch_add(1, Ordering::Relaxed);
            let class_name_str = format!("FlightHubDInputWindow_{}\0", class_id);
            let class_name: Vec<u16> = class_name_str.encode_utf16().collect();

            // Register the window class
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: Default::default(),
                hCursor: Default::default(),
                hbrBackground: Default::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
                hIconSm: Default::default(),
            };

            let class_atom = RegisterClassExW(&wc);
            if class_atom == 0 {
                let error = Error::from_thread();
                return Err(WindowError::ClassRegistrationError(format!(
                    "RegisterClassExW failed: {}",
                    error
                )));
            }

            // Create the message-only window
            // Using HWND_MESSAGE as the parent creates a message-only window
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR::from_raw(class_name.as_ptr()),
                PCWSTR::null(),     // No window title
                WS_OVERLAPPED,      // Minimal style
                0,                  // x
                0,                  // y
                0,                  // width
                0,                  // height
                Some(HWND_MESSAGE), // Message-only window
                None,               // No menu
                Some(hinstance),
                Some(ptr::null()),
            )
            .map_err(|e| {
                // Clean up the class registration on failure
                let _ = UnregisterClassW(PCWSTR::from_raw(class_name.as_ptr()), Some(hinstance));
                WindowError::WindowCreationError(format!("CreateWindowExW failed: {}", e))
            })?;

            tracing::debug!(
                "Created message-only window: hwnd={:?}, class={}",
                hwnd,
                class_name_str.trim_end_matches('\0')
            );

            Ok(Self {
                hwnd,
                class_atom,
                class_name,
                hinstance,
                _not_send_sync: std::marker::PhantomData,
            })
        }
    }

    /// Get the window handle
    ///
    /// # Returns
    /// * `HWND` - The window handle for use with DirectInput
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Get the window handle as a raw pointer value
    ///
    /// This is useful for storing in contexts that don't have HWND available.
    pub fn hwnd_raw(&self) -> isize {
        self.hwnd.0 as isize
    }

    /// Check if the window is still valid
    pub fn is_valid(&self) -> bool {
        !self.hwnd.is_invalid()
    }
}

#[cfg(windows)]
impl Drop for MessageOnlyWindow {
    fn drop(&mut self) {
        unsafe {
            // Destroy the window
            if !self.hwnd.is_invalid() {
                if let Err(e) = DestroyWindow(self.hwnd) {
                    tracing::warn!("DestroyWindow failed: {}", e);
                }
            }

            // Unregister the window class
            if self.class_atom != 0 {
                if let Err(e) = UnregisterClassW(
                    PCWSTR::from_raw(self.class_name.as_ptr()),
                    Some(self.hinstance),
                ) {
                    tracing::warn!("UnregisterClassW failed: {}", e);
                }
            }

            tracing::debug!("Destroyed message-only window");
        }
    }
}

/// Window procedure for the message-only window
///
/// This is a minimal window proc that just passes messages to DefWindowProcW.
/// We don't need to handle any messages since this is just a dummy window.
#[cfg(windows)]
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ============================================================================
// Non-Windows Stub
// ============================================================================

/// Stub implementation for non-Windows platforms
#[cfg(not(windows))]
pub struct MessageOnlyWindow {
    _private: (),
}

#[cfg(not(windows))]
impl MessageOnlyWindow {
    /// Create a new message-only window (stub for non-Windows)
    pub fn new() -> Result<Self> {
        Err(WindowError::PlatformNotSupported)
    }

    /// Get the window handle (stub)
    pub fn hwnd(&self) -> usize {
        0
    }

    /// Get the window handle as a raw pointer value (stub)
    pub fn hwnd_raw(&self) -> isize {
        0
    }

    /// Check if the window is valid (stub)
    pub fn is_valid(&self) -> bool {
        false
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_window_creation() {
        let window = MessageOnlyWindow::new();
        assert!(window.is_ok(), "Window creation should succeed");

        let window = window.unwrap();
        assert!(window.is_valid(), "Window should be valid after creation");
        assert!(window.hwnd_raw() != 0, "Window handle should be non-zero");
    }

    #[test]
    #[cfg(windows)]
    fn test_multiple_windows() {
        // Should be able to create multiple windows
        let window1 = MessageOnlyWindow::new().expect("First window should succeed");
        let window2 = MessageOnlyWindow::new().expect("Second window should succeed");

        assert_ne!(
            window1.hwnd_raw(),
            window2.hwnd_raw(),
            "Windows should have different handles"
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_window_drop() {
        let hwnd_raw;
        {
            let window = MessageOnlyWindow::new().expect("Window creation should succeed");
            hwnd_raw = window.hwnd_raw();
            assert!(hwnd_raw != 0);
            // Window is dropped here
        }
        // After drop, the window should be destroyed
        // We can't easily test this without additional Windows API calls
    }

    #[test]
    #[cfg(not(windows))]
    fn test_non_windows_stub() {
        let result = MessageOnlyWindow::new();
        assert!(matches!(result, Err(WindowError::PlatformNotSupported)));
    }
}
