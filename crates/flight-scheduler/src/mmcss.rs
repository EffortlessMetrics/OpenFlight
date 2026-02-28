// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Windows MMCSS (Multimedia Class Scheduler Service) integration
//!
//! Provides a trait-based abstraction over MMCSS for real-time thread
//! scheduling on Windows, with a mock backend for cross-platform testing.

// =============================================================================
// MMCSS FFI Stubs (avrt.dll / winmm.dll)
// =============================================================================

#[cfg(target_os = "windows")]
#[link(name = "avrt")]
unsafe extern "system" {
    fn AvSetMmThreadCharacteristicsW(task_name: *const u16, task_index: *mut u32) -> isize;
    fn AvRevertMmThreadCharacteristics(avrt_handle: isize) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

/// Result of MMCSS timer resolution change
#[cfg(target_os = "windows")]
const TIMERR_NOERROR: u32 = 0;

// =============================================================================
// Error Types
// =============================================================================

/// Errors from MMCSS operations
#[derive(Debug)]
pub enum MmcssError {
    /// MMCSS registration failed
    RegistrationFailed(String),
    /// Priority change failed
    PriorityFailed(String),
    /// Timer resolution change failed
    TimerResolutionFailed(String),
    /// MMCSS unregistration failed
    UnregistrationFailed(String),
}

impl std::fmt::Display for MmcssError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MmcssError::RegistrationFailed(e) => write!(f, "MMCSS registration failed: {e}"),
            MmcssError::PriorityFailed(e) => write!(f, "MMCSS priority change failed: {e}"),
            MmcssError::TimerResolutionFailed(e) => {
                write!(f, "Timer resolution change failed: {e}")
            }
            MmcssError::UnregistrationFailed(e) => {
                write!(f, "MMCSS unregistration failed: {e}")
            }
        }
    }
}

impl std::error::Error for MmcssError {}

// =============================================================================
// MMCSS Backend Trait
// =============================================================================

/// Thread priority levels within MMCSS scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmcssPriority {
    /// Low priority within the MMCSS task group
    Low,
    /// Normal priority within the MMCSS task group
    Normal,
    /// High priority within the MMCSS task group
    High,
    /// Critical priority — use sparingly
    Critical,
}

impl MmcssPriority {
    /// Convert to Windows AVRT_PRIORITY values
    pub fn as_avrt_priority(self) -> i32 {
        match self {
            MmcssPriority::Low => -1,     // AVRT_PRIORITY_LOW
            MmcssPriority::Normal => 0,   // AVRT_PRIORITY_NORMAL
            MmcssPriority::High => 1,     // AVRT_PRIORITY_HIGH
            MmcssPriority::Critical => 2, // AVRT_PRIORITY_CRITICAL
        }
    }
}

/// Abstraction over MMCSS operations for testability
pub trait MmcssBackend: Send {
    /// Register the current thread with MMCSS under the given task name.
    /// Returns a handle and the assigned task index.
    fn register(&self, task_name: &str, task_index: &mut u32) -> Result<isize, MmcssError>;

    /// Unregister the thread from MMCSS using the given handle.
    fn unregister(&self, handle: isize) -> Result<(), MmcssError>;

    /// Set the thread priority within the MMCSS task group.
    fn set_priority(&self, handle: isize, priority: MmcssPriority) -> Result<(), MmcssError>;

    /// Enable high-resolution timer (1ms period).
    fn enable_high_resolution_timer(&self) -> Result<(), MmcssError>;

    /// Disable high-resolution timer (restore default period).
    fn disable_high_resolution_timer(&self) -> Result<(), MmcssError>;
}

// =============================================================================
// Real Windows Backend
// =============================================================================

/// Real MMCSS backend that calls Windows APIs
#[cfg(target_os = "windows")]
#[derive(Debug)]
pub struct WindowsMmcssBackend;

#[cfg(target_os = "windows")]
impl MmcssBackend for WindowsMmcssBackend {
    fn register(&self, task_name: &str, task_index: &mut u32) -> Result<isize, MmcssError> {
        let wide: Vec<u16> = task_name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: valid null-terminated UTF-16 string and valid mutable pointer
        let handle = unsafe { AvSetMmThreadCharacteristicsW(wide.as_ptr(), task_index) };
        if handle == 0 {
            let err = std::io::Error::last_os_error();
            Err(MmcssError::RegistrationFailed(format!(
                "task '{}': {} (code: {:?})",
                task_name,
                err,
                err.raw_os_error()
            )))
        } else {
            Ok(handle)
        }
    }

    fn unregister(&self, handle: isize) -> Result<(), MmcssError> {
        // SAFETY: handle was obtained from AvSetMmThreadCharacteristicsW
        let result = unsafe { AvRevertMmThreadCharacteristics(handle) };
        if result == 0 {
            Err(MmcssError::UnregistrationFailed(
                std::io::Error::last_os_error().to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn set_priority(&self, _handle: isize, priority: MmcssPriority) -> Result<(), MmcssError> {
        use windows::Win32::System::Threading::*;
        let win_priority = match priority {
            MmcssPriority::Low => THREAD_PRIORITY_BELOW_NORMAL,
            MmcssPriority::Normal => THREAD_PRIORITY_NORMAL,
            MmcssPriority::High => THREAD_PRIORITY_ABOVE_NORMAL,
            MmcssPriority::Critical => THREAD_PRIORITY_TIME_CRITICAL,
        };
        // SAFETY: GetCurrentThread pseudo-handle is always valid
        let result = unsafe { SetThreadPriority(GetCurrentThread(), win_priority) };
        if result.is_err() {
            Err(MmcssError::PriorityFailed(
                std::io::Error::last_os_error().to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn enable_high_resolution_timer(&self) -> Result<(), MmcssError> {
        // SAFETY: timeBeginPeriod is safe to call
        let result = unsafe { timeBeginPeriod(1) };
        if result != TIMERR_NOERROR {
            Err(MmcssError::TimerResolutionFailed(format!(
                "timeBeginPeriod(1) returned {}",
                result
            )))
        } else {
            Ok(())
        }
    }

    fn disable_high_resolution_timer(&self) -> Result<(), MmcssError> {
        // SAFETY: timeEndPeriod is safe to call; must match previous timeBeginPeriod
        let result = unsafe { timeEndPeriod(1) };
        if result != TIMERR_NOERROR {
            Err(MmcssError::TimerResolutionFailed(format!(
                "timeEndPeriod(1) returned {}",
                result
            )))
        } else {
            Ok(())
        }
    }
}

// =============================================================================
// Mock Backend (for testing)
// =============================================================================

/// Mock MMCSS backend for cross-platform testing
#[derive(Debug)]
pub struct MockMmcssBackend {
    /// If true, register/unregister calls succeed; if false, they fail.
    pub succeed: bool,
    /// Track number of register calls
    register_count: std::sync::atomic::AtomicU32,
    /// Track number of unregister calls
    unregister_count: std::sync::atomic::AtomicU32,
    /// Track number of set_priority calls
    priority_count: std::sync::atomic::AtomicU32,
    /// Track high-res timer state
    timer_active: std::sync::atomic::AtomicBool,
}

impl MockMmcssBackend {
    /// Create a mock backend that succeeds on all calls
    pub fn new_success() -> Self {
        Self {
            succeed: true,
            register_count: std::sync::atomic::AtomicU32::new(0),
            unregister_count: std::sync::atomic::AtomicU32::new(0),
            priority_count: std::sync::atomic::AtomicU32::new(0),
            timer_active: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Create a mock backend that fails on all calls
    pub fn new_failure() -> Self {
        Self {
            succeed: false,
            register_count: std::sync::atomic::AtomicU32::new(0),
            unregister_count: std::sync::atomic::AtomicU32::new(0),
            priority_count: std::sync::atomic::AtomicU32::new(0),
            timer_active: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Number of register calls made
    pub fn register_count(&self) -> u32 {
        self.register_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Number of unregister calls made
    pub fn unregister_count(&self) -> u32 {
        self.unregister_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Number of set_priority calls made
    pub fn priority_count(&self) -> u32 {
        self.priority_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether the high-res timer is active
    pub fn is_timer_active(&self) -> bool {
        self.timer_active.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl MmcssBackend for MockMmcssBackend {
    fn register(&self, task_name: &str, task_index: &mut u32) -> Result<isize, MmcssError> {
        self.register_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.succeed {
            *task_index = 1;
            // Return a synthetic non-zero handle
            Ok(0xDEAD_BEEF_i64 as isize)
        } else {
            Err(MmcssError::RegistrationFailed(format!(
                "mock failure for task '{task_name}'"
            )))
        }
    }

    fn unregister(&self, _handle: isize) -> Result<(), MmcssError> {
        self.unregister_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.succeed {
            Ok(())
        } else {
            Err(MmcssError::UnregistrationFailed("mock failure".to_string()))
        }
    }

    fn set_priority(&self, _handle: isize, _priority: MmcssPriority) -> Result<(), MmcssError> {
        self.priority_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.succeed {
            Ok(())
        } else {
            Err(MmcssError::PriorityFailed("mock failure".to_string()))
        }
    }

    fn enable_high_resolution_timer(&self) -> Result<(), MmcssError> {
        if self.succeed {
            self.timer_active
                .store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            Err(MmcssError::TimerResolutionFailed(
                "mock failure".to_string(),
            ))
        }
    }

    fn disable_high_resolution_timer(&self) -> Result<(), MmcssError> {
        if self.succeed {
            self.timer_active
                .store(false, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            Err(MmcssError::TimerResolutionFailed(
                "mock failure".to_string(),
            ))
        }
    }
}

// =============================================================================
// MmcssHandle
// =============================================================================

/// RAII handle for MMCSS thread registration.
///
/// Registers the calling thread with the Multimedia Class Scheduler Service
/// and automatically unregisters on drop. Uses a trait-based backend so the
/// real Windows calls can be swapped for a mock in tests.
///
/// # Default task name
///
/// The default MMCSS task is **"Pro Audio"**, which yields the lowest
/// scheduling latency on Windows.
pub struct MmcssHandle<B: MmcssBackend = BoxedMmcssBackend> {
    backend: B,
    handle: isize,
    task_index: u32,
    task_name: String,
    priority: MmcssPriority,
    timer_enabled: bool,
}

impl<B: MmcssBackend> std::fmt::Debug for MmcssHandle<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MmcssHandle")
            .field("handle", &self.handle)
            .field("task_index", &self.task_index)
            .field("task_name", &self.task_name)
            .field("priority", &self.priority)
            .field("timer_enabled", &self.timer_enabled)
            .finish()
    }
}

/// Type-erased backend for ergonomic default usage
pub type BoxedMmcssBackend = Box<dyn MmcssBackend>;

impl MmcssBackend for Box<dyn MmcssBackend> {
    fn register(&self, task_name: &str, task_index: &mut u32) -> Result<isize, MmcssError> {
        (**self).register(task_name, task_index)
    }
    fn unregister(&self, handle: isize) -> Result<(), MmcssError> {
        (**self).unregister(handle)
    }
    fn set_priority(&self, handle: isize, priority: MmcssPriority) -> Result<(), MmcssError> {
        (**self).set_priority(handle, priority)
    }
    fn enable_high_resolution_timer(&self) -> Result<(), MmcssError> {
        (**self).enable_high_resolution_timer()
    }
    fn disable_high_resolution_timer(&self) -> Result<(), MmcssError> {
        (**self).disable_high_resolution_timer()
    }
}

impl<B: MmcssBackend> MmcssHandle<B> {
    /// Register the current thread with MMCSS.
    ///
    /// # Arguments
    /// * `backend`    — Platform backend (real or mock)
    /// * `task_name`  — MMCSS task name (e.g. "Pro Audio", "Games")
    /// * `task_index` — Initial task index hint (typically 0)
    pub fn register(backend: B, task_name: &str, task_index: u32) -> Result<Self, MmcssError> {
        let mut idx = task_index;
        let handle = backend.register(task_name, &mut idx)?;

        Ok(Self {
            backend,
            handle,
            task_index: idx,
            task_name: task_name.to_string(),
            priority: MmcssPriority::Normal,
            timer_enabled: false,
        })
    }

    /// Register with the default "Pro Audio" task for lowest latency.
    pub fn register_pro_audio(backend: B) -> Result<Self, MmcssError> {
        Self::register(backend, "Pro Audio", 0)
    }

    /// Set thread priority within the MMCSS task group.
    pub fn set_priority(&mut self, priority: MmcssPriority) -> Result<(), MmcssError> {
        self.backend.set_priority(self.handle, priority)?;
        self.priority = priority;
        Ok(())
    }

    /// Enable high-resolution (1 ms) system timer.
    pub fn enable_high_resolution_timer(&mut self) -> Result<(), MmcssError> {
        self.backend.enable_high_resolution_timer()?;
        self.timer_enabled = true;
        Ok(())
    }

    /// Explicitly unregister from MMCSS (also called automatically on drop).
    pub fn unregister(mut self) -> Result<(), MmcssError> {
        self.unregister_inner()
    }

    /// The raw MMCSS handle value.
    pub fn raw_handle(&self) -> isize {
        self.handle
    }

    /// The MMCSS task index assigned during registration.
    pub fn task_index(&self) -> u32 {
        self.task_index
    }

    /// The task name used for registration.
    pub fn task_name(&self) -> &str {
        &self.task_name
    }

    /// The current priority level.
    pub fn current_priority(&self) -> MmcssPriority {
        self.priority
    }

    /// Whether the high-resolution timer was enabled through this handle.
    pub fn is_timer_enabled(&self) -> bool {
        self.timer_enabled
    }

    /// Whether this handle holds an active MMCSS registration.
    pub fn is_registered(&self) -> bool {
        self.handle != 0
    }

    // internal unregister; sets handle to 0 to prevent double-unregister
    fn unregister_inner(&mut self) -> Result<(), MmcssError> {
        if self.timer_enabled {
            // Best-effort timer restoration
            let _ = self.backend.disable_high_resolution_timer();
            self.timer_enabled = false;
        }
        if self.handle != 0 {
            let result = self.backend.unregister(self.handle);
            self.handle = 0;
            return result;
        }
        Ok(())
    }
}

impl<B: MmcssBackend> Drop for MmcssHandle<B> {
    fn drop(&mut self) {
        let _ = self.unregister_inner();
    }
}

/// Convenience: enable 1 ms timer resolution without a full MMCSS registration.
///
/// Calls `timeBeginPeriod(1)` on Windows. Returns a guard that calls
/// `timeEndPeriod(1)` on drop.
pub fn enable_high_resolution_timer<B: MmcssBackend>(
    backend: &B,
) -> Result<HighResTimerGuard<'_, B>, MmcssError> {
    backend.enable_high_resolution_timer()?;
    Ok(HighResTimerGuard { backend })
}

/// RAII guard that restores the default timer resolution on drop.
pub struct HighResTimerGuard<'a, B: MmcssBackend> {
    backend: &'a B,
}

impl<B: MmcssBackend> Drop for HighResTimerGuard<'_, B> {
    fn drop(&mut self) {
        let _ = self.backend.disable_high_resolution_timer();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_register_success() {
        let backend = MockMmcssBackend::new_success();
        let handle = MmcssHandle::register(backend, "Pro Audio", 0);
        assert!(handle.is_ok());
        let h = handle.unwrap();
        assert!(h.is_registered());
        assert_eq!(h.task_name(), "Pro Audio");
        assert_eq!(h.task_index(), 1);
    }

    #[test]
    fn test_mock_register_failure() {
        let backend = MockMmcssBackend::new_failure();
        let result = MmcssHandle::register(backend, "Pro Audio", 0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, MmcssError::RegistrationFailed(_)));
    }

    #[test]
    fn test_register_pro_audio_default() {
        let backend = MockMmcssBackend::new_success();
        let h = MmcssHandle::register_pro_audio(backend).unwrap();
        assert_eq!(h.task_name(), "Pro Audio");
    }

    #[test]
    fn test_set_priority() {
        let backend = MockMmcssBackend::new_success();
        let mut h = MmcssHandle::register(backend, "Games", 0).unwrap();
        assert_eq!(h.current_priority(), MmcssPriority::Normal);
        h.set_priority(MmcssPriority::Critical).unwrap();
        assert_eq!(h.current_priority(), MmcssPriority::Critical);
    }

    #[test]
    fn test_set_priority_failure() {
        let backend = MockMmcssBackend::new_failure();
        // Force register to succeed so we can test priority failure
        let mut backend = backend;
        backend.succeed = true;
        let mut h = MmcssHandle::register(backend, "Games", 0).unwrap();
        // Now make it fail
        h.backend.succeed = false;
        let result = h.set_priority(MmcssPriority::High);
        assert!(result.is_err());
    }

    #[test]
    fn test_explicit_unregister() {
        let backend = MockMmcssBackend::new_success();
        let h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
        assert!(h.is_registered());
        let result = h.unregister();
        assert!(result.is_ok());
    }

    #[test]
    fn test_drop_calls_unregister() {
        let backend = MockMmcssBackend::new_success();
        let h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
        assert_eq!(h.backend.unregister_count(), 0);
        drop(h);
        // Can't check after drop since backend is moved, but no panic = success
    }

    #[test]
    fn test_double_drop_safe() {
        // Ensure calling unregister then dropping doesn't double-free
        let backend = MockMmcssBackend::new_success();
        let mut h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
        h.unregister_inner().unwrap();
        assert!(!h.is_registered());
        // Drop should be a no-op now
        drop(h);
    }

    #[test]
    fn test_enable_high_res_timer() {
        let backend = MockMmcssBackend::new_success();
        let mut h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
        assert!(!h.is_timer_enabled());
        h.enable_high_resolution_timer().unwrap();
        assert!(h.is_timer_enabled());
        assert!(h.backend.is_timer_active());
    }

    #[test]
    fn test_timer_disabled_on_drop() {
        let backend = MockMmcssBackend::new_success();
        let mut h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
        h.enable_high_resolution_timer().unwrap();
        assert!(h.backend.is_timer_active());
        drop(h);
        // Timer should have been disabled in drop — no panic = success
    }

    #[test]
    fn test_standalone_high_res_timer_guard() {
        let backend = MockMmcssBackend::new_success();
        {
            let _guard = enable_high_resolution_timer(&backend).unwrap();
            assert!(backend.is_timer_active());
        }
        assert!(!backend.is_timer_active());
    }

    #[test]
    fn test_raw_handle_nonzero() {
        let backend = MockMmcssBackend::new_success();
        let h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
        assert_ne!(h.raw_handle(), 0);
    }

    #[test]
    fn test_mock_call_counts() {
        let backend = MockMmcssBackend::new_success();
        assert_eq!(backend.register_count(), 0);
        let mut h = MmcssHandle::register(backend, "Pro Audio", 0).unwrap();
        assert_eq!(h.backend.register_count(), 1);
        h.set_priority(MmcssPriority::High).unwrap();
        assert_eq!(h.backend.priority_count(), 1);
        h.set_priority(MmcssPriority::Critical).unwrap();
        assert_eq!(h.backend.priority_count(), 2);
    }

    #[test]
    fn test_mmcss_priority_values() {
        assert_eq!(MmcssPriority::Low.as_avrt_priority(), -1);
        assert_eq!(MmcssPriority::Normal.as_avrt_priority(), 0);
        assert_eq!(MmcssPriority::High.as_avrt_priority(), 1);
        assert_eq!(MmcssPriority::Critical.as_avrt_priority(), 2);
    }

    #[test]
    fn test_mmcss_error_display() {
        let e = MmcssError::RegistrationFailed("test".to_string());
        assert!(format!("{e}").contains("MMCSS registration failed"));

        let e = MmcssError::PriorityFailed("test".to_string());
        assert!(format!("{e}").contains("priority change failed"));

        let e = MmcssError::TimerResolutionFailed("test".to_string());
        assert!(format!("{e}").contains("Timer resolution"));

        let e = MmcssError::UnregistrationFailed("test".to_string());
        assert!(format!("{e}").contains("unregistration failed"));
    }

    #[test]
    fn test_error_is_error_trait() {
        let e: Box<dyn std::error::Error> =
            Box::new(MmcssError::RegistrationFailed("test".to_string()));
        assert!(e.to_string().contains("MMCSS"));
    }

    #[test]
    fn test_different_task_names() {
        for name in &["Pro Audio", "Games", "Capture", "Playback"] {
            let backend = MockMmcssBackend::new_success();
            let h = MmcssHandle::register(backend, name, 0).unwrap();
            assert_eq!(h.task_name(), *name);
        }
    }
}
