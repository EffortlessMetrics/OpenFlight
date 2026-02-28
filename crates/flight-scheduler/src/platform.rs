// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Platform-agnostic real-time scheduling interface
//!
//! Dispatches to MMCSS on Windows, rtkit on Linux, and provides a no-op
//! fallback on other platforms. Uses trait-based backends so that all
//! platforms can be tested with mocks.

use crate::mmcss::{self, MmcssBackend, MmcssError, MmcssPriority};
use crate::rtkit::{self, RtkitBackend, RtkitError};

// =============================================================================
// RtPriority
// =============================================================================

/// Platform-independent real-time priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtPriority {
    /// Normal OS scheduling — no real-time elevation
    Normal,
    /// Elevated priority — above normal threads but not hard RT
    Elevated,
    /// Full real-time — lowest latency, highest scheduling priority
    Realtime,
}

impl RtPriority {
    /// Convert to an MMCSS priority (Windows path)
    fn to_mmcss(self) -> MmcssPriority {
        match self {
            RtPriority::Normal => MmcssPriority::Normal,
            RtPriority::Elevated => MmcssPriority::High,
            RtPriority::Realtime => MmcssPriority::Critical,
        }
    }

    /// Convert to a SCHED_FIFO priority value (Linux path, 1–99)
    fn to_fifo_priority(self) -> i32 {
        match self {
            RtPriority::Normal => 1,
            RtPriority::Elevated => 20,
            RtPriority::Realtime => 50,
        }
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// Unified error for platform RT operations
#[derive(Debug)]
pub enum PlatformRtError {
    /// Error from the Windows MMCSS subsystem
    Mmcss(MmcssError),
    /// Error from the Linux rtkit subsystem
    Rtkit(RtkitError),
    /// RT scheduling is not available on this platform
    Unsupported(String),
}

impl std::fmt::Display for PlatformRtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformRtError::Mmcss(e) => write!(f, "MMCSS: {e}"),
            PlatformRtError::Rtkit(e) => write!(f, "rtkit: {e}"),
            PlatformRtError::Unsupported(msg) => write!(f, "RT unsupported: {msg}"),
        }
    }
}

impl std::error::Error for PlatformRtError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlatformRtError::Mmcss(e) => Some(e),
            PlatformRtError::Rtkit(e) => Some(e),
            PlatformRtError::Unsupported(_) => None,
        }
    }
}

impl From<MmcssError> for PlatformRtError {
    fn from(e: MmcssError) -> Self {
        PlatformRtError::Mmcss(e)
    }
}

impl From<RtkitError> for PlatformRtError {
    fn from(e: RtkitError) -> Self {
        PlatformRtError::Rtkit(e)
    }
}

// =============================================================================
// Platform Detection
// =============================================================================

/// Detected platform for RT scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Windows with MMCSS
    Windows,
    /// Linux with rtkit / SCHED_FIFO
    Linux,
    /// macOS (limited RT support)
    MacOs,
    /// Unknown / unsupported
    Other,
}

/// Detect the current platform at compile time.
pub const fn detect_platform() -> Platform {
    if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Other
    }
}

/// Check whether real-time scheduling is available on this platform.
///
/// Returns `true` on Windows (MMCSS) and Linux (rtkit / SCHED_FIFO).
pub const fn is_rt_available() -> bool {
    matches!(detect_platform(), Platform::Windows | Platform::Linux)
}

// =============================================================================
// RtHandle — platform-agnostic handle
// =============================================================================

/// Platform handle variant stored inside [`RtHandle`].
enum RtHandleInner<M: MmcssBackend, R: RtkitBackend> {
    Mmcss(mmcss::MmcssHandle<M>),
    Rtkit(rtkit::RtkitHandle<R>),
    Noop,
}

/// RAII handle for platform RT scheduling.
///
/// Holds either an MMCSS handle (Windows), an rtkit handle (Linux), or a
/// no-op placeholder. Automatically releases RT scheduling on drop.
pub struct RtHandle<
    M: MmcssBackend = mmcss::BoxedMmcssBackend,
    R: RtkitBackend = rtkit::BoxedRtkitBackend,
> {
    inner: RtHandleInner<M, R>,
    level: RtPriority,
    platform: Platform,
}

impl<M: MmcssBackend, R: RtkitBackend> std::fmt::Debug for RtHandle<M, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RtHandle")
            .field("level", &self.level)
            .field("platform", &self.platform)
            .field("is_active", &self.is_active())
            .finish()
    }
}

impl<M: MmcssBackend, R: RtkitBackend> RtHandle<M, R> {
    /// The priority level this handle was created with.
    pub fn level(&self) -> RtPriority {
        self.level
    }

    /// The platform this handle targets.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Whether this handle holds an active RT registration.
    pub fn is_active(&self) -> bool {
        !matches!(self.inner, RtHandleInner::Noop)
    }

    /// Release the RT scheduling explicitly (also happens on drop).
    pub fn release(self) -> Result<(), PlatformRtError> {
        match self.inner {
            RtHandleInner::Mmcss(h) => h.unregister().map_err(PlatformRtError::Mmcss),
            RtHandleInner::Rtkit(_) => {
                // Drop takes care of relinquishing
                Ok(())
            }
            RtHandleInner::Noop => Ok(()),
        }
    }
}

// =============================================================================
// Request Functions
// =============================================================================

/// Request RT priority using an MMCSS backend (Windows path).
pub fn request_rt_priority_mmcss<M: MmcssBackend>(
    backend: M,
    level: RtPriority,
) -> Result<RtHandle<M, rtkit::BoxedRtkitBackend>, PlatformRtError> {
    let mut handle = mmcss::MmcssHandle::register_pro_audio(backend)?;
    if level != RtPriority::Normal {
        handle.set_priority(level.to_mmcss())?;
    }
    Ok(RtHandle {
        inner: RtHandleInner::Mmcss(handle),
        level,
        platform: Platform::Windows,
    })
}

/// Request RT priority using an rtkit backend (Linux path).
pub fn request_rt_priority_rtkit<R: RtkitBackend>(
    backend: R,
    level: RtPriority,
) -> Result<RtHandle<mmcss::BoxedMmcssBackend, R>, PlatformRtError> {
    let priority = level.to_fifo_priority();
    let handle = rtkit::RtkitHandle::request_realtime(backend, priority)?;
    Ok(RtHandle {
        inner: RtHandleInner::Rtkit(handle),
        level,
        platform: Platform::Linux,
    })
}

/// Create a no-op RT handle for unsupported platforms.
pub fn request_rt_priority_noop<M: MmcssBackend, R: RtkitBackend>(
    level: RtPriority,
) -> RtHandle<M, R> {
    RtHandle {
        inner: RtHandleInner::Noop,
        level,
        platform: detect_platform(),
    }
}

/// Request RT priority using the native platform backend.
///
/// - **Windows**: registers with MMCSS "Pro Audio" task
/// - **Linux**: requests SCHED_FIFO via rtkit
/// - **Other**: returns a no-op handle
///
/// This is the main entry point for production code.
#[cfg(target_os = "windows")]
pub fn request_rt_priority(
    level: RtPriority,
) -> Result<RtHandle<mmcss::BoxedMmcssBackend, rtkit::BoxedRtkitBackend>, PlatformRtError> {
    let backend: mmcss::BoxedMmcssBackend = Box::new(mmcss::WindowsMmcssBackend);
    request_rt_priority_mmcss(backend, level)
}

#[cfg(target_os = "linux")]
pub fn request_rt_priority(
    level: RtPriority,
) -> Result<RtHandle<mmcss::BoxedMmcssBackend, rtkit::BoxedRtkitBackend>, PlatformRtError> {
    let backend: rtkit::BoxedRtkitBackend = Box::new(rtkit::LinuxRtkitBackend);
    request_rt_priority_rtkit(backend, level)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn request_rt_priority(
    level: RtPriority,
) -> Result<RtHandle<mmcss::BoxedMmcssBackend, rtkit::BoxedRtkitBackend>, PlatformRtError> {
    Ok(request_rt_priority_noop(level))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmcss::MockMmcssBackend;
    use crate::rtkit::MockRtkitBackend;
    use std::error::Error;

    // --- Platform detection ---

    #[test]
    fn test_detect_platform() {
        let p = detect_platform();
        // On Windows CI this must be Windows
        if cfg!(target_os = "windows") {
            assert_eq!(p, Platform::Windows);
        } else if cfg!(target_os = "linux") {
            assert_eq!(p, Platform::Linux);
        }
    }

    #[test]
    fn test_is_rt_available() {
        let available = is_rt_available();
        if cfg!(any(target_os = "windows", target_os = "linux")) {
            assert!(available);
        }
    }

    // --- RtPriority conversions ---

    #[test]
    fn test_rt_priority_to_mmcss() {
        assert_eq!(RtPriority::Normal.to_mmcss(), MmcssPriority::Normal);
        assert_eq!(RtPriority::Elevated.to_mmcss(), MmcssPriority::High);
        assert_eq!(RtPriority::Realtime.to_mmcss(), MmcssPriority::Critical);
    }

    #[test]
    fn test_rt_priority_to_fifo() {
        assert_eq!(RtPriority::Normal.to_fifo_priority(), 1);
        assert_eq!(RtPriority::Elevated.to_fifo_priority(), 20);
        assert_eq!(RtPriority::Realtime.to_fifo_priority(), 50);
    }

    // --- MMCSS path ---

    #[test]
    fn test_request_mmcss_normal() {
        let backend = MockMmcssBackend::new_success();
        let h = request_rt_priority_mmcss(backend, RtPriority::Normal).unwrap();
        assert_eq!(h.level(), RtPriority::Normal);
        assert_eq!(h.platform(), Platform::Windows);
        assert!(h.is_active());
    }

    #[test]
    fn test_request_mmcss_realtime() {
        let backend = MockMmcssBackend::new_success();
        let h = request_rt_priority_mmcss(backend, RtPriority::Realtime).unwrap();
        assert_eq!(h.level(), RtPriority::Realtime);
        assert!(h.is_active());
    }

    #[test]
    fn test_request_mmcss_failure() {
        let backend = MockMmcssBackend::new_failure();
        let result = request_rt_priority_mmcss(backend, RtPriority::Normal);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PlatformRtError::Mmcss(_)));
    }

    #[test]
    fn test_release_mmcss_handle() {
        let backend = MockMmcssBackend::new_success();
        let h = request_rt_priority_mmcss(backend, RtPriority::Elevated).unwrap();
        assert!(h.release().is_ok());
    }

    // --- rtkit path ---

    #[test]
    fn test_request_rtkit_normal() {
        let backend = MockRtkitBackend::new_success();
        let h = request_rt_priority_rtkit(backend, RtPriority::Normal).unwrap();
        assert_eq!(h.level(), RtPriority::Normal);
        assert_eq!(h.platform(), Platform::Linux);
        assert!(h.is_active());
    }

    #[test]
    fn test_request_rtkit_realtime() {
        let backend = MockRtkitBackend::new_success();
        let h = request_rt_priority_rtkit(backend, RtPriority::Realtime).unwrap();
        assert_eq!(h.level(), RtPriority::Realtime);
    }

    #[test]
    fn test_request_rtkit_failure() {
        let backend = MockRtkitBackend::new_failure();
        let result = request_rt_priority_rtkit(backend, RtPriority::Realtime);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PlatformRtError::Rtkit(_)));
    }

    #[test]
    fn test_release_rtkit_handle() {
        let backend = MockRtkitBackend::new_success();
        let h = request_rt_priority_rtkit(backend, RtPriority::Elevated).unwrap();
        assert!(h.release().is_ok());
    }

    // --- Noop path ---

    #[test]
    fn test_noop_handle() {
        let h: RtHandle<MockMmcssBackend, MockRtkitBackend> =
            request_rt_priority_noop(RtPriority::Realtime);
        assert!(!h.is_active());
        assert_eq!(h.level(), RtPriority::Realtime);
    }

    #[test]
    fn test_noop_release() {
        let h: RtHandle<MockMmcssBackend, MockRtkitBackend> =
            request_rt_priority_noop(RtPriority::Normal);
        assert!(h.release().is_ok());
    }

    // --- Drop safety ---

    #[test]
    fn test_drop_mmcss_handle_no_panic() {
        let backend = MockMmcssBackend::new_success();
        let h = request_rt_priority_mmcss(backend, RtPriority::Realtime).unwrap();
        drop(h); // must not panic
    }

    #[test]
    fn test_drop_rtkit_handle_no_panic() {
        let backend = MockRtkitBackend::new_success();
        let h = request_rt_priority_rtkit(backend, RtPriority::Realtime).unwrap();
        drop(h); // must not panic
    }

    #[test]
    fn test_drop_noop_handle_no_panic() {
        let h: RtHandle<MockMmcssBackend, MockRtkitBackend> =
            request_rt_priority_noop(RtPriority::Elevated);
        drop(h); // must not panic
    }

    // --- Error types ---

    #[test]
    fn test_platform_error_from_mmcss() {
        let e = PlatformRtError::from(MmcssError::RegistrationFailed("test".into()));
        assert!(format!("{e}").contains("MMCSS"));
    }

    #[test]
    fn test_platform_error_from_rtkit() {
        let e = PlatformRtError::from(RtkitError::RequestDenied("test".into()));
        assert!(format!("{e}").contains("rtkit"));
    }

    #[test]
    fn test_platform_error_unsupported() {
        let e = PlatformRtError::Unsupported("no RT".into());
        assert!(format!("{e}").contains("unsupported"));
    }

    #[test]
    fn test_platform_error_is_error_trait() {
        let e: Box<dyn std::error::Error> = Box::new(PlatformRtError::Unsupported("test".into()));
        assert!(e.to_string().contains("unsupported"));
    }

    #[test]
    fn test_platform_error_source_mmcss() {
        let e = PlatformRtError::Mmcss(MmcssError::PriorityFailed("inner".into()));
        assert!(e.source().is_some());
    }

    #[test]
    fn test_platform_error_source_unsupported() {
        let e = PlatformRtError::Unsupported("none".into());
        assert!(e.source().is_none());
    }

    // --- RtPriority traits ---

    #[test]
    fn test_rt_priority_clone_eq() {
        let a = RtPriority::Elevated;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(RtPriority::Normal, RtPriority::Realtime);
    }

    #[test]
    fn test_rt_priority_debug() {
        let s = format!("{:?}", RtPriority::Realtime);
        assert_eq!(s, "Realtime");
    }

    // --- Platform enum ---

    #[test]
    fn test_platform_enum_values() {
        assert_ne!(Platform::Windows, Platform::Linux);
        assert_ne!(Platform::Linux, Platform::MacOs);
        assert_ne!(Platform::MacOs, Platform::Other);
    }

    // --- Full lifecycle with mock ---

    #[test]
    fn test_full_mmcss_lifecycle() {
        let backend = MockMmcssBackend::new_success();
        let h = request_rt_priority_mmcss(backend, RtPriority::Realtime).unwrap();
        assert!(h.is_active());
        assert_eq!(h.level(), RtPriority::Realtime);
        assert_eq!(h.platform(), Platform::Windows);
        h.release().unwrap();
    }

    #[test]
    fn test_full_rtkit_lifecycle() {
        let backend = MockRtkitBackend::new_success();
        let h = request_rt_priority_rtkit(backend, RtPriority::Elevated).unwrap();
        assert!(h.is_active());
        assert_eq!(h.level(), RtPriority::Elevated);
        assert_eq!(h.platform(), Platform::Linux);
        h.release().unwrap();
    }
}
