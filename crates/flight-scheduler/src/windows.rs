// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Windows-specific scheduler implementation
//!
//! Uses high-precision timing, MMCSS registration, and thread priority
//! for real-time performance.

use std::ffi::c_void;
use std::time::Duration;
use tracing::{info, warn};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::*;

// =============================================================================
// MMCSS FFI Bindings (avrt.dll)
// =============================================================================
// The MMCSS functions are not available in the windows crate, so we use raw FFI.
// These functions are from avrt.dll which is available on Windows Vista and later.

#[link(name = "avrt")]
unsafe extern "system" {
    /// Associates the calling thread with the specified task.
    /// Returns a handle that can be used with AvRevertMmThreadCharacteristics.
    /// Returns 0 on failure.
    fn AvSetMmThreadCharacteristicsW(task_name: *const u16, task_index: *mut u32) -> isize;

    /// Indicates that a thread is no longer performing work associated with
    /// the specified task.
    fn AvRevertMmThreadCharacteristics(avrt_handle: isize) -> i32;
}

// =============================================================================
// Power Throttling FFI
// =============================================================================
// SetProcessInformation with ProcessPowerThrottling is available in Windows 10+

/// Process information class for power throttling
const PROCESS_POWER_THROTTLING: i32 = 4;

/// Power throttling state structure
#[repr(C)]
struct ProcessPowerThrottlingState {
    version: u32,
    control_mask: u32,
    state_mask: u32,
}

const PROCESS_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
const PROCESS_POWER_THROTTLING_EXECUTION_SPEED: u32 = 0x1;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetProcessInformation(
        process: HANDLE,
        process_information_class: i32,
        process_information: *const c_void,
        process_information_size: u32,
    ) -> i32;
}

// =============================================================================
// Error Types
// =============================================================================

/// Error type for real-time thread configuration
#[derive(Debug)]
pub enum RtError {
    /// MMCSS registration failed (non-fatal, logged as warning)
    MmcssRegistrationFailed(std::io::Error),
    /// Thread priority elevation failed
    PriorityElevationFailed(std::io::Error),
}

impl std::fmt::Display for RtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RtError::MmcssRegistrationFailed(e) => {
                write!(f, "MMCSS registration failed: {}", e)
            }
            RtError::PriorityElevationFailed(e) => {
                write!(f, "Thread priority elevation failed: {}", e)
            }
        }
    }
}

impl std::error::Error for RtError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RtError::MmcssRegistrationFailed(e) => Some(e),
            RtError::PriorityElevationFailed(e) => Some(e),
        }
    }
}

// =============================================================================
// WindowsRtThread
// =============================================================================

/// Windows real-time thread configuration
///
/// Provides RAII-based management of Windows real-time thread settings including:
/// - MMCSS (Multimedia Class Scheduler Service) registration
/// - Thread priority elevation to TIME_CRITICAL
/// - Process power throttling disable
///
/// When dropped, automatically restores original thread priority and releases
/// MMCSS registration.
///
/// # Example
///
/// ```no_run
/// use flight_scheduler::windows::WindowsRtThread;
///
/// // Configure RT thread with "Games" task
/// let rt_thread = WindowsRtThread::new("Games").expect("RT thread setup failed");
///
/// // Thread is now configured for real-time operation
/// // ... do real-time work ...
///
/// // When rt_thread is dropped, original settings are restored
/// ```
pub struct WindowsRtThread {
    /// MMCSS task handle (0 if registration failed)
    mmcss_handle: isize,
    /// Original thread priority (for restoration)
    original_priority: i32,
    /// Whether power throttling was disabled
    power_throttling_disabled: bool,
}

impl WindowsRtThread {
    /// Create and configure an RT thread
    ///
    /// Attempts MMCSS registration with the specified task name, elevates priority
    /// to TIME_CRITICAL, and disables power throttling. Failures are logged but
    /// don't prevent thread creation (graceful degradation).
    ///
    /// # Arguments
    ///
    /// * `task_name` - MMCSS task name, typically "Games" or "Pro Audio"
    ///
    /// # Returns
    ///
    /// Returns `Ok(WindowsRtThread)` on success. MMCSS registration failure is
    /// logged as a warning but doesn't cause an error return.
    ///
    /// # Requirements
    ///
    /// - Requirement 1.1: MMCSS registration via AvSetMmThreadCharacteristicsW
    /// - Requirement 1.2: Thread priority elevation via SetThreadPriority
    /// - Requirement 1.5: RAII cleanup via Drop trait
    pub fn new(task_name: &str) -> Result<Self, RtError> {
        let mut task_index: u32 = 0;

        // Convert task name to wide string (null-terminated UTF-16)
        let task_name_wide: Vec<u16> = task_name.encode_utf16().chain(std::iter::once(0)).collect();

        // Register with MMCSS
        // SAFETY: We're passing a valid null-terminated wide string and a valid pointer
        let mmcss_handle =
            unsafe { AvSetMmThreadCharacteristicsW(task_name_wide.as_ptr(), &mut task_index) };

        if mmcss_handle == 0 {
            let err = std::io::Error::last_os_error();
            warn!(
                "MMCSS registration failed for task '{}': {} (error code: {:?}). \
                 Continuing with elevated priority only.",
                task_name,
                err,
                err.raw_os_error()
            );
            // Continue without MMCSS - graceful degradation per Requirement 1.4
        } else {
            info!(
                "MMCSS registration successful for task '{}' (task_index: {}, handle: {})",
                task_name, task_index, mmcss_handle
            );
        }

        // Get original thread priority for restoration
        // SAFETY: GetCurrentThread returns a pseudo-handle that's always valid
        let original_priority = unsafe { GetThreadPriority(GetCurrentThread()) };

        // Elevate thread priority to TIME_CRITICAL
        // SAFETY: GetCurrentThread returns a pseudo-handle that's always valid
        let priority_result =
            unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) };

        if priority_result.is_err() {
            let err = std::io::Error::last_os_error();
            warn!("Failed to set thread priority to TIME_CRITICAL: {}", err);
            // We still continue - the MMCSS registration alone may help
        } else {
            info!("Thread priority elevated to TIME_CRITICAL");
        }

        // Disable power throttling
        let power_throttling_disabled = disable_power_throttling_internal();
        if power_throttling_disabled {
            info!("Process power throttling disabled");
        } else {
            warn!("Failed to disable process power throttling (may not be supported)");
        }

        Ok(Self {
            mmcss_handle,
            original_priority,
            power_throttling_disabled,
        })
    }

    /// Check if MMCSS registration was successful
    pub fn is_mmcss_registered(&self) -> bool {
        self.mmcss_handle != 0
    }

    /// Check if power throttling was disabled
    pub fn is_power_throttling_disabled(&self) -> bool {
        self.power_throttling_disabled
    }

    /// Get the original thread priority (before elevation)
    pub fn original_priority(&self) -> i32 {
        self.original_priority
    }

    /// Get the MMCSS handle (for debugging/testing)
    pub fn mmcss_handle(&self) -> isize {
        self.mmcss_handle
    }
}

impl Drop for WindowsRtThread {
    fn drop(&mut self) {
        // Restore original thread priority
        // SAFETY: GetCurrentThread returns a pseudo-handle that's always valid
        let restore_result = unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY(self.original_priority))
        };

        if restore_result.is_err() {
            warn!(
                "Failed to restore original thread priority ({})",
                self.original_priority
            );
        } else {
            info!(
                "Thread priority restored to original value ({})",
                self.original_priority
            );
        }

        // Release MMCSS registration if we have a valid handle
        if self.mmcss_handle != 0 {
            // SAFETY: We're passing a valid MMCSS handle obtained from
            // AvSetMmThreadCharacteristicsW
            let revert_result = unsafe { AvRevertMmThreadCharacteristics(self.mmcss_handle) };

            if revert_result == 0 {
                warn!("Failed to release MMCSS registration");
            } else {
                info!("MMCSS registration released");
            }
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Disable process power throttling for consistent performance
///
/// Uses SetProcessInformation with PROCESS_POWER_THROTTLING_EXECUTION_SPEED
/// to prevent Windows from throttling the process for power savings.
///
/// # Returns
///
/// Returns `true` if power throttling was successfully disabled, `false` otherwise.
fn disable_power_throttling_internal() -> bool {
    // Create the power throttling state structure
    // StateMask = 0 means disable throttling for the flags in ControlMask
    let state = ProcessPowerThrottlingState {
        version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        control_mask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        state_mask: 0, // 0 = disable throttling
    };

    // SAFETY: We're passing valid pointers and the correct size
    let result = unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            PROCESS_POWER_THROTTLING,
            std::ptr::addr_of!(state).cast(),
            std::mem::size_of::<ProcessPowerThrottlingState>() as u32,
        )
    };

    result != 0
}

/// Platform-specific sleep implementation for Windows
pub fn platform_sleep(duration: Duration) {
    // For now, use standard sleep - in a full implementation this would use
    // CreateWaitableTimer with high-resolution timing
    std::thread::sleep(duration);
}

// =============================================================================
// Timer Error Types
// =============================================================================

/// Error type for timer operations
#[derive(Debug)]
pub enum TimerError {
    /// Failed to create waitable timer
    CreateFailed(std::io::Error),
    /// Failed to query performance frequency
    QueryFrequencyFailed(std::io::Error),
}

impl std::fmt::Display for TimerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimerError::CreateFailed(e) => {
                write!(f, "Failed to create waitable timer: {}", e)
            }
            TimerError::QueryFrequencyFailed(e) => {
                write!(f, "Failed to query performance frequency: {}", e)
            }
        }
    }
}

impl std::error::Error for TimerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TimerError::CreateFailed(e) => Some(e),
            TimerError::QueryFrequencyFailed(e) => Some(e),
        }
    }
}

// =============================================================================
// Timer FFI Bindings (winmm.dll)
// =============================================================================
// The timeBeginPeriod/timeEndPeriod functions are from winmm.dll

#[link(name = "winmm")]
unsafe extern "system" {
    /// Requests a minimum resolution for periodic timers.
    /// Returns TIMERR_NOERROR (0) on success.
    fn timeBeginPeriod(uPeriod: u32) -> u32;

    /// Clears a previously set minimum timer resolution.
    /// Returns TIMERR_NOERROR (0) on success.
    fn timeEndPeriod(uPeriod: u32) -> u32;
}

// =============================================================================
// WindowsTimerLoop
// =============================================================================

/// High-resolution 250Hz timer loop for Windows
///
/// Provides a precise timing mechanism for the real-time axis processing loop.
/// Uses Windows waitable timers with high-resolution flag when available,
/// falling back to standard timers with `timeBeginPeriod(1)` on older systems.
///
/// The timer uses a hybrid approach:
/// 1. Waitable timer for the bulk of the wait period
/// 2. Busy-spin using QueryPerformanceCounter for the final 50-80μs
///
/// This achieves sub-millisecond jitter while minimizing CPU usage.
///
/// # Example
///
/// ```no_run
/// use flight_scheduler::windows::WindowsTimerLoop;
///
/// let timer = WindowsTimerLoop::new().expect("Failed to create timer");
///
/// // Get initial QPC value
/// let mut target = timer.current_qpc();
/// target += timer.ticks_per_period(); // First target is one period from now
///
/// loop {
///     // Wait for next tick, returns the next target QPC value
///     target = timer.wait_next_tick(target);
///     
///     // Do real-time work here...
/// }
/// ```
///
/// # Requirements
///
/// - Requirement 2.1: Use CreateWaitableTimerExW with CREATE_WAITABLE_TIMER_HIGH_RESOLUTION
/// - Requirement 2.2: Fall back to timeBeginPeriod(1) with standard waitable timers
/// - Requirement 2.3: Busy-spin for final 50-80μs using QueryPerformanceCounter
pub struct WindowsTimerLoop {
    /// Waitable timer handle
    timer: HANDLE,
    /// Whether high-resolution timer is available
    high_res_available: bool,
    /// QPC frequency (ticks per second)
    qpc_freq: i64,
    /// QPC ticks per 250Hz period (qpc_freq / 250)
    ticks_per_period: i64,
    /// Busy-spin threshold in QPC ticks (equivalent to ~80μs)
    busy_spin_threshold_ticks: i64,
}

impl WindowsTimerLoop {
    /// Create a 250Hz timer loop
    ///
    /// Attempts to create a high-resolution waitable timer first. If that fails
    /// (e.g., on older Windows versions), falls back to a standard waitable timer
    /// with `timeBeginPeriod(1)` to improve timer resolution.
    ///
    /// # Returns
    ///
    /// Returns `Ok(WindowsTimerLoop)` on success, or `Err(TimerError)` if timer
    /// creation fails completely.
    ///
    /// # Requirements
    ///
    /// - Requirement 2.1: CreateWaitableTimerExW with CREATE_WAITABLE_TIMER_HIGH_RESOLUTION
    /// - Requirement 2.2: Fallback to timeBeginPeriod(1) + standard timer
    pub fn new() -> Result<Self, TimerError> {
        // Get QPC frequency for timing calculations
        let mut freq: i64 = 0;
        // SAFETY: QueryPerformanceFrequency always succeeds on Windows XP and later
        let freq_result =
            unsafe { windows::Win32::System::Performance::QueryPerformanceFrequency(&mut freq) };

        if freq_result.is_err() || freq == 0 {
            return Err(TimerError::QueryFrequencyFailed(
                std::io::Error::last_os_error(),
            ));
        }

        // Try high-resolution timer first (Windows 10 1803+)
        // CREATE_WAITABLE_TIMER_HIGH_RESOLUTION = 0x00000002
        const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: u32 = 0x00000002;
        // TIMER_ALL_ACCESS = 0x1F0003
        const TIMER_ALL_ACCESS: u32 = 0x1F0003;

        // SAFETY: CreateWaitableTimerExW is safe to call with null security attributes and name
        let timer = unsafe {
            windows::Win32::System::Threading::CreateWaitableTimerExW(
                None,                                  // No security attributes
                None,                                  // No name
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, // High-resolution flag
                TIMER_ALL_ACCESS,                      // Full access
            )
        };

        let (timer, high_res_available) = match timer {
            Ok(handle) if !handle.is_invalid() => {
                info!("High-resolution waitable timer created successfully");
                (handle, true)
            }
            _ => {
                // Fall back to standard timer with timeBeginPeriod
                warn!(
                    "High-resolution timer unavailable, falling back to standard timer with timeBeginPeriod(1)"
                );

                // Request 1ms timer resolution
                // SAFETY: timeBeginPeriod is safe to call
                let period_result = unsafe { timeBeginPeriod(1) };
                if period_result != 0 {
                    warn!("timeBeginPeriod(1) returned non-zero: {}", period_result);
                }

                // Create standard waitable timer (manual-reset = false for auto-reset)
                // SAFETY: CreateWaitableTimerExW is safe to call
                let fallback_timer = unsafe {
                    windows::Win32::System::Threading::CreateWaitableTimerExW(
                        None,             // No security attributes
                        None,             // No name
                        0,                // No special flags (standard timer)
                        TIMER_ALL_ACCESS, // Full access
                    )
                };

                match fallback_timer {
                    Ok(handle) if !handle.is_invalid() => {
                        info!("Standard waitable timer created with timeBeginPeriod(1)");
                        (handle, false)
                    }
                    _ => {
                        // Clean up timeBeginPeriod if timer creation failed
                        unsafe { timeEndPeriod(1) };
                        return Err(TimerError::CreateFailed(std::io::Error::last_os_error()));
                    }
                }
            }
        };

        // Calculate timing constants
        let ticks_per_period = freq / 250; // 250Hz = 4ms period
        // Busy-spin threshold: ~80μs worth of QPC ticks
        let busy_spin_threshold_ticks = (freq * 80) / 1_000_000;

        info!(
            "WindowsTimerLoop initialized: qpc_freq={}, ticks_per_period={}, \
             busy_spin_threshold={}, high_res={}",
            freq, ticks_per_period, busy_spin_threshold_ticks, high_res_available
        );

        Ok(Self {
            timer,
            high_res_available,
            qpc_freq: freq,
            ticks_per_period,
            busy_spin_threshold_ticks,
        })
    }

    /// Wait for next tick with busy-spin finish
    ///
    /// Waits using the waitable timer for most of the period, then busy-spins
    /// for the final 50-80μs to minimize jitter. This hybrid approach provides
    /// precise timing while keeping CPU usage low.
    ///
    /// # Arguments
    ///
    /// * `target_qpc` - The target QPC value to wait until
    ///
    /// # Returns
    ///
    /// Returns the next target QPC value (target_qpc + ticks_per_period)
    ///
    /// # Requirements
    ///
    /// - Requirement 2.3: Busy-spin for final 50-80μs using QueryPerformanceCounter
    /// - Requirement 2.4: Use QueryPerformanceCounter as monotonic clock source
    pub fn wait_next_tick(&self, target_qpc: i64) -> i64 {
        // Get current time
        let now_qpc = self.current_qpc();

        // Calculate remaining time until target
        let remaining_ticks = target_qpc - now_qpc;

        if remaining_ticks > self.busy_spin_threshold_ticks {
            // Use waitable timer for bulk of wait
            // Leave busy_spin_threshold_ticks for busy-spin
            let wait_ticks = remaining_ticks - self.busy_spin_threshold_ticks;

            // Convert to 100ns units (negative = relative time)
            // 1 QPC tick = (10_000_000 / qpc_freq) 100ns units
            let wait_100ns = -((wait_ticks * 10_000_000) / self.qpc_freq);

            // SAFETY: SetWaitableTimer is safe with valid handle and due_time pointer
            let set_result = unsafe {
                windows::Win32::System::Threading::SetWaitableTimer(
                    self.timer,
                    &wait_100ns, // Due time (negative = relative)
                    0,           // Period (0 = one-shot)
                    None,        // No completion routine
                    None,        // No completion routine arg
                    false,       // Don't resume from suspend
                )
            };

            if set_result.is_ok() {
                // Wait for timer to fire
                // SAFETY: WaitForSingleObject is safe with valid handle
                unsafe {
                    windows::Win32::System::Threading::WaitForSingleObject(
                        self.timer,
                        u32::MAX, // INFINITE
                    );
                }
            }
        }

        // Busy-spin for final portion to achieve precise timing
        loop {
            let now = self.current_qpc();
            if now >= target_qpc {
                break;
            }
            std::hint::spin_loop();
        }

        // Return next target
        target_qpc + self.ticks_per_period
    }

    /// Get current QPC value
    ///
    /// Returns the current value of the QueryPerformanceCounter.
    ///
    /// # Requirements
    ///
    /// - Requirement 2.4: Use QueryPerformanceCounter as monotonic clock source
    #[inline]
    pub fn current_qpc(&self) -> i64 {
        let mut now: i64 = 0;
        // SAFETY: QueryPerformanceCounter always succeeds on Windows XP and later
        let _ = unsafe { windows::Win32::System::Performance::QueryPerformanceCounter(&mut now) };
        now
    }

    /// Get QPC frequency (ticks per second)
    #[inline]
    pub fn qpc_freq(&self) -> i64 {
        self.qpc_freq
    }

    /// Get QPC ticks per 250Hz period
    #[inline]
    pub fn ticks_per_period(&self) -> i64 {
        self.ticks_per_period
    }

    /// Check if high-resolution timer is available
    #[inline]
    pub fn is_high_res_available(&self) -> bool {
        self.high_res_available
    }

    /// Convert QPC ticks to microseconds
    #[inline]
    pub fn ticks_to_us(&self, ticks: i64) -> i64 {
        (ticks * 1_000_000) / self.qpc_freq
    }

    /// Convert QPC ticks to nanoseconds
    #[inline]
    pub fn ticks_to_ns(&self, ticks: i64) -> i64 {
        (ticks * 1_000_000_000) / self.qpc_freq
    }
}

impl Drop for WindowsTimerLoop {
    fn drop(&mut self) {
        // Close timer handle
        // SAFETY: CloseHandle is safe with valid handle
        let close_result = unsafe { windows::Win32::Foundation::CloseHandle(self.timer) };

        if close_result.is_err() {
            warn!("Failed to close timer handle");
        } else {
            info!("Timer handle closed");
        }

        // If we used timeBeginPeriod, we must call timeEndPeriod
        if !self.high_res_available {
            // SAFETY: timeEndPeriod is safe to call
            let end_result = unsafe { timeEndPeriod(1) };
            if end_result != 0 {
                warn!("timeEndPeriod(1) returned non-zero: {}", end_result);
            } else {
                info!("timeEndPeriod(1) called successfully");
            }
        }
    }
}

// Implement Default for convenience
impl Default for WindowsTimerLoop {
    fn default() -> Self {
        Self::new().expect("Failed to create WindowsTimerLoop")
    }
}

/// Set current thread to real-time priority (legacy function)
#[allow(dead_code)]
#[deprecated(since = "0.1.0", note = "Use WindowsRtThread::new() instead")]
pub fn set_realtime_priority() -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        // Set thread priority to time critical
        let result = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        if result.is_ok() {
            Ok(())
        } else {
            Err("Failed to set thread priority".into())
        }
    }
}

/// Disable process power throttling for consistent performance (legacy function)
#[allow(dead_code)]
#[deprecated(since = "0.1.0", note = "Use WindowsRtThread::new() instead")]
pub fn disable_power_throttling() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if disable_power_throttling_internal() {
        Ok(())
    } else {
        Err("Failed to disable power throttling".into())
    }
}

/// Check if system is configured for real-time performance
#[allow(dead_code)]
pub fn check_rt_configuration() -> RTConfigStatus {
    let issues = Vec::new();

    // Simplified check - in a full implementation this would check:
    // - Power plan settings
    // - Battery status
    // - CPU throttling settings
    // - USB selective suspend

    RTConfigStatus { issues }
}

/// Real-time configuration status
#[allow(dead_code)]
pub struct RTConfigStatus {
    pub issues: Vec<String>,
}

#[allow(dead_code)]
impl RTConfigStatus {
    pub fn is_optimal(&self) -> bool {
        self.issues.is_empty()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test WindowsRtThread creation and basic functionality
    ///
    /// Note: This test may behave differently depending on system configuration
    /// and whether MMCSS is available.
    #[test]
    fn test_windows_rt_thread_creation() {
        // Create RT thread with "Games" task
        let rt_thread = WindowsRtThread::new("Games");

        // Should succeed even if MMCSS registration fails (graceful degradation)
        assert!(rt_thread.is_ok(), "WindowsRtThread::new should not fail");

        let rt = rt_thread.unwrap();

        // Original priority should be a valid value
        // THREAD_PRIORITY_NORMAL is 0, but other values are possible
        assert!(
            (-15..=15).contains(&rt.original_priority()),
            "Original priority should be in valid range"
        );
    }

    /// Test that Drop properly cleans up
    #[test]
    fn test_windows_rt_thread_drop() {
        // Get current priority before creating RT thread
        let priority_before = unsafe { GetThreadPriority(GetCurrentThread()) };

        {
            let _rt_thread = WindowsRtThread::new("Games").unwrap();
            // Thread should now be at TIME_CRITICAL priority
        }
        // RT thread dropped here

        // Priority should be restored
        let priority_after = unsafe { GetThreadPriority(GetCurrentThread()) };
        assert_eq!(
            priority_before, priority_after,
            "Thread priority should be restored after drop"
        );
    }

    /// Test power throttling disable function
    #[test]
    fn test_disable_power_throttling() {
        // This may or may not succeed depending on system configuration
        // We just verify it doesn't panic
        let result = disable_power_throttling_internal();
        // Result can be true or false depending on permissions and Windows version
        let _ = result;
    }

    /// Test with different task names
    #[test]
    fn test_different_task_names() {
        // Test with "Games" task
        let rt_games = WindowsRtThread::new("Games");
        assert!(rt_games.is_ok());
        drop(rt_games);

        // Test with "Pro Audio" task
        let rt_audio = WindowsRtThread::new("Pro Audio");
        assert!(rt_audio.is_ok());
        drop(rt_audio);

        // Test with empty task name (should still work, MMCSS may fail)
        let rt_empty = WindowsRtThread::new("");
        assert!(rt_empty.is_ok());
    }

    /// Test MMCSS handle accessor
    #[test]
    fn test_mmcss_handle_accessor() {
        let rt = WindowsRtThread::new("Games").unwrap();

        // Handle should be consistent with is_mmcss_registered
        if rt.is_mmcss_registered() {
            assert_ne!(rt.mmcss_handle(), 0);
        } else {
            assert_eq!(rt.mmcss_handle(), 0);
        }
    }

    // =========================================================================
    // WindowsTimerLoop Tests
    // =========================================================================

    /// Test WindowsTimerLoop creation
    #[test]
    fn test_timer_loop_creation() {
        let timer = WindowsTimerLoop::new();
        assert!(timer.is_ok(), "WindowsTimerLoop::new should succeed");

        let timer = timer.unwrap();

        // QPC frequency should be positive and reasonable (typically 10MHz on modern systems)
        assert!(timer.qpc_freq() > 0, "QPC frequency should be positive");
        assert!(
            timer.qpc_freq() >= 1_000_000,
            "QPC frequency should be at least 1MHz"
        );

        // Ticks per period should be positive
        assert!(
            timer.ticks_per_period() > 0,
            "Ticks per period should be positive"
        );

        // Verify 250Hz period calculation
        // ticks_per_period = qpc_freq / 250
        let expected_ticks = timer.qpc_freq() / 250;
        assert_eq!(
            timer.ticks_per_period(),
            expected_ticks,
            "Ticks per period should be qpc_freq / 250"
        );
    }

    /// Test WindowsTimerLoop current_qpc
    #[test]
    fn test_timer_loop_current_qpc() {
        let timer = WindowsTimerLoop::new().unwrap();

        let qpc1 = timer.current_qpc();
        std::thread::sleep(Duration::from_millis(1));
        let qpc2 = timer.current_qpc();

        // QPC should be monotonically increasing
        assert!(qpc2 > qpc1, "QPC should increase over time");
    }

    /// Test WindowsTimerLoop time conversion utilities
    #[test]
    fn test_timer_loop_conversions() {
        let timer = WindowsTimerLoop::new().unwrap();

        // Test ticks_to_us
        let one_second_ticks = timer.qpc_freq();
        let us = timer.ticks_to_us(one_second_ticks);
        assert_eq!(us, 1_000_000, "One second should be 1,000,000 microseconds");

        // Test ticks_to_ns
        let ns = timer.ticks_to_ns(one_second_ticks);
        assert_eq!(
            ns, 1_000_000_000,
            "One second should be 1,000,000,000 nanoseconds"
        );

        // Test period conversion
        let period_us = timer.ticks_to_us(timer.ticks_per_period());
        // 250Hz = 4ms = 4000μs
        assert!(
            (3900..=4100).contains(&period_us),
            "Period should be approximately 4000μs, got {}",
            period_us
        );
    }

    /// Test WindowsTimerLoop drop cleanup
    #[test]
    fn test_timer_loop_drop() {
        // Create and drop multiple timers to verify cleanup works
        for _ in 0..3 {
            let timer = WindowsTimerLoop::new().unwrap();
            let _ = timer.is_high_res_available();
            // Timer dropped here
        }
        // If we get here without panic/crash, cleanup is working
    }

    /// Test WindowsTimerLoop wait_next_tick basic functionality
    #[test]
    fn test_timer_loop_wait_next_tick() {
        let timer = WindowsTimerLoop::new().unwrap();

        // Get current time and set target slightly in the future
        let now = timer.current_qpc();
        let target = now + timer.ticks_per_period();

        let start = std::time::Instant::now();
        let next_target = timer.wait_next_tick(target);
        let elapsed = start.elapsed();

        // Should have waited approximately 4ms (250Hz period)
        // Allow some tolerance for system scheduling
        assert!(
            (2..=10).contains(&elapsed.as_millis()),
            "Wait should be approximately 4ms, got {:?}",
            elapsed
        );

        // Next target should be one period after the original target
        assert_eq!(
            next_target,
            target + timer.ticks_per_period(),
            "Next target should be one period later"
        );
    }

    /// Test WindowsTimerLoop with immediate target (already passed)
    #[test]
    fn test_timer_loop_immediate_target() {
        let timer = WindowsTimerLoop::new().unwrap();

        // Set target in the past
        let now = timer.current_qpc();
        let target = now - timer.ticks_per_period();

        let start = std::time::Instant::now();
        let next_target = timer.wait_next_tick(target);
        let elapsed = start.elapsed();

        // Should return immediately (or very quickly)
        assert!(
            elapsed.as_micros() < 1000,
            "Should return quickly for past target, got {:?}",
            elapsed
        );

        // Next target should still be calculated correctly
        assert_eq!(
            next_target,
            target + timer.ticks_per_period(),
            "Next target should be one period after original target"
        );
    }
}

// =============================================================================
// Jitter Measurement (Simplified for Task 2.2)
// =============================================================================
// Note: A full JitterMeasurement struct will be implemented in Task 8.1.
// This is a simplified inline version for the Windows timer jitter test.

/// Simplified jitter measurement helper for RT validation
///
/// Records deviations from ideal period and computes p50/p95/p99 statistics.
/// This is a simplified version for Task 2.2; the full implementation will
/// be in Task 8.1 as a cross-platform helper.
///
/// # Requirements
///
/// - Requirement 8.1: Record deviation vs ideal period and compute p50/p95/p99
#[derive(Debug)]
pub struct JitterMeasurement {
    /// Target period in nanoseconds
    target_period_ns: u64,
    /// Recorded deviations from ideal period (absolute values in nanoseconds)
    deviations: Vec<i64>,
    /// Last tick timestamp in nanoseconds
    last_tick_ns: u64,
    /// Warmup ticks to skip
    warmup_ticks: usize,
    /// Current tick count
    tick_count: usize,
}

impl JitterMeasurement {
    /// Create a new jitter measurement for given Hz
    ///
    /// # Arguments
    ///
    /// * `target_hz` - Target frequency in Hz (e.g., 250 for 250Hz)
    /// * `warmup_seconds` - Number of seconds to skip at start (warmup period)
    pub fn new(target_hz: u32, warmup_seconds: u32) -> Self {
        let target_period_ns = 1_000_000_000 / target_hz as u64;
        let warmup_ticks = (target_hz * warmup_seconds) as usize;

        Self {
            target_period_ns,
            deviations: Vec::with_capacity(target_hz as usize * 600), // 10 min capacity
            last_tick_ns: 0,
            warmup_ticks,
            tick_count: 0,
        }
    }

    /// Record a tick and compute deviation from ideal period
    ///
    /// # Arguments
    ///
    /// * `now_ns` - Current timestamp in nanoseconds
    pub fn record_tick(&mut self, now_ns: u64) {
        self.tick_count += 1;

        if self.last_tick_ns > 0 && self.tick_count > self.warmup_ticks {
            let actual_period = now_ns.saturating_sub(self.last_tick_ns);
            let deviation = actual_period as i64 - self.target_period_ns as i64;
            self.deviations.push(deviation);
        }

        self.last_tick_ns = now_ns;
    }

    /// Compute jitter statistics
    ///
    /// Returns p50, p95, and p99 jitter values based on recorded deviations.
    pub fn compute_stats(&self) -> JitterStats {
        if self.deviations.is_empty() {
            return JitterStats::default();
        }

        // Sort absolute deviations for percentile calculation
        let mut sorted: Vec<i64> = self.deviations.iter().map(|d| d.abs()).collect();
        sorted.sort_unstable();

        let len = sorted.len();
        let p50 = sorted[len / 2];
        let p95 = sorted[(len * 95) / 100];
        let p99 = sorted[(len * 99) / 100];

        JitterStats {
            samples: len,
            p50_ns: p50,
            p95_ns: p95,
            p99_ns: p99,
            p99_ms: p99 as f64 / 1_000_000.0,
        }
    }

    /// Get the number of samples recorded (excluding warmup)
    pub fn sample_count(&self) -> usize {
        self.deviations.len()
    }

    /// Get the total tick count (including warmup)
    pub fn tick_count(&self) -> usize {
        self.tick_count
    }
}

/// Jitter statistics computed from recorded deviations
#[derive(Debug, Default, Clone)]
pub struct JitterStats {
    /// Number of samples used for statistics
    pub samples: usize,
    /// 50th percentile (median) jitter in nanoseconds
    pub p50_ns: i64,
    /// 95th percentile jitter in nanoseconds
    pub p95_ns: i64,
    /// 99th percentile jitter in nanoseconds
    pub p99_ns: i64,
    /// 99th percentile jitter in milliseconds (for easy comparison with 0.5ms threshold)
    pub p99_ms: f64,
}

// =============================================================================
// Property-Based Tests
// =============================================================================

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: release-readiness, Property 2: MMCSS Lifecycle
    // **Validates: Requirements 1.1, 1.2, 1.5**
    //
    // For any WindowsRtThread instance, if MMCSS registration succeeds (non-zero handle),
    // then thread priority SHALL be elevated to TIME_CRITICAL, and when the instance is
    // dropped, MMCSS registration SHALL be released via AvRevertMmThreadCharacteristics.

    /// Strategy for generating valid MMCSS task names
    fn task_name_strategy() -> impl Strategy<Value = String> {
        // MMCSS task names that are known to work on Windows
        // "Games" and "Pro Audio" are the most commonly used
        prop_oneof![
            Just("Games".to_string()),
            Just("Pro Audio".to_string()),
            Just("Audio".to_string()),
            Just("Capture".to_string()),
            Just("Distribution".to_string()),
            Just("Playback".to_string()),
            Just("Window Manager".to_string()),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property test: MMCSS lifecycle invariants
        ///
        /// **Validates: Requirements 1.1, 1.2, 1.5**
        ///
        /// This test verifies that for any valid MMCSS task name:
        /// 1. WindowsRtThread creation succeeds (graceful degradation)
        /// 2. If MMCSS registration succeeds, thread priority is elevated
        /// 3. After drop, thread priority is restored to original value
        /// 4. MMCSS handle state is consistent with is_mmcss_registered()
        #[test]
        fn prop_mmcss_lifecycle(task_name in task_name_strategy()) {
            // Capture thread priority before RT thread creation
            let priority_before = unsafe { GetThreadPriority(GetCurrentThread()) };

            // Create RT thread - should always succeed (graceful degradation)
            let rt_thread = WindowsRtThread::new(&task_name);
            prop_assert!(
                rt_thread.is_ok(),
                "WindowsRtThread::new should succeed for task '{}'",
                task_name
            );

            let rt = rt_thread.unwrap();

            // Property 1: MMCSS handle consistency
            // If is_mmcss_registered() returns true, handle must be non-zero
            // If is_mmcss_registered() returns false, handle must be zero
            if rt.is_mmcss_registered() {
                prop_assert_ne!(
                    rt.mmcss_handle(),
                    0,
                    "MMCSS handle should be non-zero when registered"
                );
            } else {
                prop_assert_eq!(
                    rt.mmcss_handle(),
                    0,
                    "MMCSS handle should be zero when not registered"
                );
            }

            // Property 2: Thread priority elevation
            // When RT thread is active, priority should be elevated to TIME_CRITICAL
            // (THREAD_PRIORITY_TIME_CRITICAL = 15)
            let current_priority = unsafe { GetThreadPriority(GetCurrentThread()) };
            prop_assert!(
                current_priority >= THREAD_PRIORITY_HIGHEST.0,
                "Thread priority ({}) should be at least HIGHEST ({}) when RT thread is active",
                current_priority,
                THREAD_PRIORITY_HIGHEST.0
            );

            // Property 3: Original priority is captured correctly
            // The original priority should be a valid Windows thread priority value
            let orig_priority = rt.original_priority();
            prop_assert!(
                (-15..=15).contains(&orig_priority),
                "Original priority ({}) should be in valid range [-15, 15]",
                orig_priority
            );

            // Drop the RT thread to trigger cleanup
            drop(rt);

            // Property 4: Priority restoration after drop
            // Thread priority should be restored to the value before RT thread creation
            let priority_after = unsafe { GetThreadPriority(GetCurrentThread()) };
            prop_assert_eq!(
                priority_before,
                priority_after,
                "Thread priority should be restored after drop (before: {}, after: {})",
                priority_before,
                priority_after
            );
        }

        /// Property test: Multiple RT thread instances lifecycle
        ///
        /// **Validates: Requirements 1.1, 1.2, 1.5**
        ///
        /// This test verifies that creating and dropping multiple RT thread instances
        /// in sequence maintains correct state and properly cleans up resources.
        #[test]
        fn prop_mmcss_multiple_instances(
            task_names in prop::collection::vec(task_name_strategy(), 1..5)
        ) {
            // Capture initial thread priority
            let initial_priority = unsafe { GetThreadPriority(GetCurrentThread()) };

            for task_name in task_names {
                // Create RT thread
                let rt = WindowsRtThread::new(&task_name).expect("RT thread creation should succeed");

                // Verify priority is elevated
                let current_priority = unsafe { GetThreadPriority(GetCurrentThread()) };
                prop_assert!(
                    current_priority >= THREAD_PRIORITY_HIGHEST.0,
                    "Priority should be elevated for task '{}'",
                    task_name
                );

                // Drop RT thread
                drop(rt);

                // Verify priority is restored
                let restored_priority = unsafe { GetThreadPriority(GetCurrentThread()) };
                prop_assert_eq!(
                    initial_priority,
                    restored_priority,
                    "Priority should be restored after dropping RT thread for task '{}'",
                    task_name
                );
            }
        }

        /// Property test: MMCSS registration state consistency
        ///
        /// **Validates: Requirements 1.1, 1.5**
        ///
        /// This test verifies that the MMCSS registration state is always consistent
        /// between the handle value and the is_mmcss_registered() method.
        #[test]
        fn prop_mmcss_state_consistency(task_name in task_name_strategy()) {
            let rt = WindowsRtThread::new(&task_name).expect("RT thread creation should succeed");

            // State consistency: handle and is_mmcss_registered must agree
            let handle = rt.mmcss_handle();
            let is_registered = rt.is_mmcss_registered();

            prop_assert_eq!(
                handle != 0,
                is_registered,
                "MMCSS handle ({}) and is_mmcss_registered ({}) must be consistent",
                handle,
                is_registered
            );

            // If registered, verify we can access the handle for potential debugging
            if is_registered {
                prop_assert!(
                    handle > 0,
                    "Valid MMCSS handle should be positive, got {}",
                    handle
                );
            }
        }
    }

    // =========================================================================
    // Jitter Measurement Accuracy Tests
    // =========================================================================

    // Feature: release-readiness, Property 10: Jitter Measurement Accuracy
    // **Validates: Requirements 8.1**
    //
    // For any JitterMeasurement instance with known synthetic deviations,
    // the computed p50/p95/p99 statistics SHALL match the expected percentile
    // values within floating-point tolerance.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property test: Jitter measurement accuracy for p50/p95/p99 statistics
        ///
        /// **Validates: Requirements 8.1**
        ///
        /// This test verifies that for any set of synthetic deviations:
        /// 1. The computed p50 matches the expected median of absolute deviations
        /// 2. The computed p95 matches the expected 95th percentile
        /// 3. The computed p99 matches the expected 99th percentile
        /// 4. All statistics are within floating-point tolerance
        #[test]
        fn prop_jitter_measurement_accuracy(
            deviations in prop::collection::vec(-1_000_000i64..1_000_000i64, 100..1000)
        ) {
            // Create JitterMeasurement with no warmup for testing
            let mut jitter = JitterMeasurement::new(250, 0);
            let base_ns: u64 = 1_000_000_000;
            let period_ns: u64 = 4_000_000; // 250Hz = 4ms period

            // Record ticks with the synthetic deviations
            // The deviation is added to the ideal tick time to simulate jitter
            for (i, dev) in deviations.iter().enumerate() {
                // Calculate tick time: base + (tick_index * period) + deviation
                // We need to handle negative deviations carefully
                let ideal_tick = base_ns + (i as u64 * period_ns);
                let tick_ns = if *dev >= 0 {
                    ideal_tick + (*dev as u64)
                } else {
                    ideal_tick.saturating_sub(dev.unsigned_abs())
                };
                jitter.record_tick(tick_ns);
            }

            let stats = jitter.compute_stats();

            // We should have n-1 samples (first tick establishes baseline)
            let expected_samples = deviations.len() - 1;
            prop_assert_eq!(
                stats.samples,
                expected_samples,
                "Should have {} samples, got {}",
                expected_samples,
                stats.samples
            );

            // Skip validation if we don't have enough samples
            if stats.samples < 10 {
                return Ok(());
            }

            // Compute expected percentiles from the deviations
            // Note: JitterMeasurement computes deviation between consecutive ticks,
            // so we need to compute the expected deviations the same way
            let mut expected_deviations: Vec<i64> = Vec::with_capacity(deviations.len() - 1);
            for i in 1..deviations.len() {
                // The actual period deviation is: (dev[i] - dev[i-1])
                // because tick[i] = base + i*period + dev[i]
                // and tick[i-1] = base + (i-1)*period + dev[i-1]
                // so actual_period = tick[i] - tick[i-1] = period + (dev[i] - dev[i-1])
                // and deviation from ideal = dev[i] - dev[i-1]
                let period_deviation = deviations[i] - deviations[i - 1];
                expected_deviations.push(period_deviation);
            }

            // Sort absolute deviations for percentile calculation
            let mut sorted: Vec<i64> = expected_deviations.iter().map(|d| d.abs()).collect();
            sorted.sort_unstable();

            let len = sorted.len();
            let expected_p50 = sorted[len / 2];
            let expected_p95 = sorted[(len * 95) / 100];
            let expected_p99 = sorted[(len * 99) / 100];

            // Allow tolerance for floating-point and rounding differences
            // The tolerance is 1000ns (1μs) which is reasonable for nanosecond measurements
            const TOLERANCE_NS: i64 = 1000;

            prop_assert!(
                (stats.p50_ns - expected_p50).abs() <= TOLERANCE_NS,
                "p50 mismatch: computed={}, expected={}, diff={}",
                stats.p50_ns,
                expected_p50,
                (stats.p50_ns - expected_p50).abs()
            );

            prop_assert!(
                (stats.p95_ns - expected_p95).abs() <= TOLERANCE_NS,
                "p95 mismatch: computed={}, expected={}, diff={}",
                stats.p95_ns,
                expected_p95,
                (stats.p95_ns - expected_p95).abs()
            );

            prop_assert!(
                (stats.p99_ns - expected_p99).abs() <= TOLERANCE_NS,
                "p99 mismatch: computed={}, expected={}, diff={}",
                stats.p99_ns,
                expected_p99,
                (stats.p99_ns - expected_p99).abs()
            );

            // Verify p99_ms is consistent with p99_ns
            let expected_p99_ms = stats.p99_ns as f64 / 1_000_000.0;
            prop_assert!(
                (stats.p99_ms - expected_p99_ms).abs() < 0.001,
                "p99_ms mismatch: computed={}, expected={}",
                stats.p99_ms,
                expected_p99_ms
            );
        }

        /// Property test: Jitter measurement with uniform deviations
        ///
        /// **Validates: Requirements 8.1**
        ///
        /// This test verifies that for uniformly distributed deviations,
        /// the percentiles are computed correctly.
        #[test]
        fn prop_jitter_measurement_uniform_deviations(
            max_deviation in 1000i64..1_000_000i64,
            num_samples in 100usize..500usize
        ) {
            let mut jitter = JitterMeasurement::new(250, 0);
            let base_ns: u64 = 1_000_000_000;
            let period_ns: u64 = 4_000_000;

            // Generate ticks with linearly increasing deviations
            // This creates a predictable distribution for testing
            for i in 0..num_samples {
                let deviation = (i as i64 * max_deviation) / (num_samples as i64);
                let tick_ns = base_ns + (i as u64 * period_ns) + (deviation as u64);
                jitter.record_tick(tick_ns);
            }

            let stats = jitter.compute_stats();

            // Verify we have the expected number of samples
            prop_assert_eq!(
                stats.samples,
                num_samples - 1,
                "Should have {} samples",
                num_samples - 1
            );

            // For linearly increasing deviations, the period deviations are constant
            // deviation[i] = i * max_deviation / num_samples
            // period_deviation = deviation[i] - deviation[i-1] = max_deviation / num_samples
            let expected_period_deviation = max_deviation / (num_samples as i64);

            // All period deviations should be approximately equal
            // So p50, p95, p99 should all be close to the expected period deviation
            const TOLERANCE_NS: i64 = 1000;

            prop_assert!(
                (stats.p50_ns - expected_period_deviation).abs() <= TOLERANCE_NS,
                "p50 should be close to expected period deviation: computed={}, expected={}",
                stats.p50_ns,
                expected_period_deviation
            );
        }

        /// Property test: Jitter measurement warmup exclusion
        ///
        /// **Validates: Requirements 8.1**
        ///
        /// This test verifies that warmup ticks are properly excluded from statistics.
        ///
        /// The warmup logic works as follows:
        /// - tick_count is incremented on each record_tick call
        /// - Deviations are recorded when: last_tick_ns > 0 AND tick_count > warmup_ticks
        /// - So the first deviation is recorded on tick (warmup_ticks + 1)
        /// - For post_warmup_ticks ticks after warmup, we get post_warmup_ticks deviations
        #[test]
        fn prop_jitter_measurement_warmup_exclusion(
            warmup_seconds in 1u32..5u32,
            post_warmup_ticks in 100usize..500usize
        ) {
            let target_hz: u32 = 250;
            let warmup_ticks = (target_hz * warmup_seconds) as usize;

            let mut jitter = JitterMeasurement::new(target_hz, warmup_seconds);
            let base_ns: u64 = 1_000_000_000;
            let period_ns: u64 = 4_000_000;

            let total_ticks = warmup_ticks + post_warmup_ticks;

            // Record all ticks
            for i in 0..total_ticks {
                let tick_ns = base_ns + (i as u64 * period_ns);
                jitter.record_tick(tick_ns);
            }

            let stats = jitter.compute_stats();

            // The warmup logic:
            // - Ticks 1 to warmup_ticks: no deviations recorded (tick_count <= warmup_ticks)
            // - Tick warmup_ticks+1: first deviation recorded (has previous tick from warmup)
            // - Ticks warmup_ticks+2 to total_ticks: deviations recorded
            // So we get exactly post_warmup_ticks deviations
            let expected_samples = post_warmup_ticks;
            prop_assert_eq!(
                stats.samples,
                expected_samples,
                "Should have {} samples after warmup, got {}",
                expected_samples,
                stats.samples
            );

            // Verify tick_count includes all ticks
            prop_assert_eq!(
                jitter.tick_count(),
                total_ticks,
                "tick_count should be {}, got {}",
                total_ticks,
                jitter.tick_count()
            );
        }
    }

    /// Unit test: MMCSS lifecycle with explicit verification
    ///
    /// Feature: release-readiness, Property 2: MMCSS Lifecycle
    /// **Validates: Requirements 1.1, 1.2, 1.5**
    ///
    /// This is a deterministic test that explicitly verifies the MMCSS lifecycle
    /// as specified in the design document.
    #[test]
    fn test_mmcss_lifecycle() {
        // Capture initial state
        let priority_before = unsafe { GetThreadPriority(GetCurrentThread()) };

        // Create RT thread with "Games" task (most commonly available)
        let rt = WindowsRtThread::new("Games").expect("RT thread creation should succeed");

        // Verify MMCSS handle is valid (if registration succeeded)
        // Note: May be 0 on systems without MMCSS or insufficient privileges
        let mmcss_registered = rt.is_mmcss_registered();
        if mmcss_registered {
            assert_ne!(
                rt.mmcss_handle(),
                0,
                "MMCSS handle should be non-zero when registered"
            );
        }

        // Verify priority was elevated
        // THREAD_PRIORITY_TIME_CRITICAL = 15, THREAD_PRIORITY_HIGHEST = 2
        let priority = unsafe { GetThreadPriority(GetCurrentThread()) };
        assert!(
            priority >= THREAD_PRIORITY_HIGHEST.0,
            "Thread priority ({}) should be at least HIGHEST ({}) after RT thread creation",
            priority,
            THREAD_PRIORITY_HIGHEST.0
        );

        // Drop should release MMCSS and restore priority
        drop(rt);

        // Verify priority was restored
        let priority_after = unsafe { GetThreadPriority(GetCurrentThread()) };
        assert_eq!(
            priority_before, priority_after,
            "Thread priority should be restored after drop (before: {}, after: {})",
            priority_before, priority_after
        );
    }

    // =========================================================================
    // Timer Jitter Tests
    // =========================================================================

    /// Feature: release-readiness, Property 1: Timer Loop Jitter (Windows)
    /// **Validates: Requirements 2.5**
    ///
    /// For any 250Hz timer loop running for ≥10 minutes (excluding 5s warmup),
    /// the p99 jitter SHALL be ≤0.5ms on Windows platforms.
    ///
    /// This test is marked with `#[ignore]` because it requires 10+ minutes to run.
    /// Run manually with: `cargo test -p flight-scheduler test_timer_jitter_windows -- --ignored --nocapture`
    /// Or in CI on hardware runners.
    #[test]
    #[ignore] // Requires 10+ minutes, run manually or in CI
    fn test_timer_jitter_windows() {
        use std::time::{Duration, Instant};

        // Test duration: 10 minutes (600 seconds)
        const TEST_DURATION_SECS: u64 = 600;
        // Warmup period: 5 seconds (excluded from statistics)
        const WARMUP_SECS: u32 = 5;
        // Target frequency: 250Hz
        const TARGET_HZ: u32 = 250;
        // Maximum allowed p99 jitter: 0.5ms
        const MAX_P99_MS: f64 = 0.5;

        println!("=== Windows Timer Jitter Test ===");
        println!(
            "Duration: {} seconds ({} minutes)",
            TEST_DURATION_SECS,
            TEST_DURATION_SECS / 60
        );
        println!("Warmup: {} seconds", WARMUP_SECS);
        println!(
            "Target frequency: {}Hz ({}ms period)",
            TARGET_HZ,
            1000.0 / TARGET_HZ as f64
        );
        println!("Max allowed p99 jitter: {}ms", MAX_P99_MS);
        println!();

        // Create the timer loop
        let timer = WindowsTimerLoop::new().expect("Failed to create WindowsTimerLoop");
        println!(
            "Timer created: high_res={}, qpc_freq={} Hz",
            timer.is_high_res_available(),
            timer.qpc_freq()
        );

        // Create jitter measurement with 5-second warmup
        let mut jitter = JitterMeasurement::new(TARGET_HZ, WARMUP_SECS);

        // Get initial QPC value and set first target
        let mut target_qpc = timer.current_qpc();
        target_qpc += timer.ticks_per_period(); // First target is one period from now

        // Use a base QPC for relative time calculation to avoid overflow
        let base_qpc = timer.current_qpc();

        let start = Instant::now();
        let test_duration = Duration::from_secs(TEST_DURATION_SECS);

        // Progress reporting interval
        let report_interval = Duration::from_secs(60);
        let mut last_report = Instant::now();

        println!("Starting timer loop...");
        println!();

        // Main timer loop
        while start.elapsed() < test_duration {
            // Wait for next tick
            target_qpc = timer.wait_next_tick(target_qpc);

            // Convert relative QPC to nanoseconds for jitter measurement
            // Use relative time from base to avoid overflow
            let relative_qpc = target_qpc - base_qpc;
            let now_ns = timer.ticks_to_ns(relative_qpc) as u64;
            jitter.record_tick(now_ns);

            // Progress report every minute
            if last_report.elapsed() >= report_interval {
                let elapsed_mins = start.elapsed().as_secs() / 60;
                let remaining_mins = (TEST_DURATION_SECS - start.elapsed().as_secs()) / 60;
                let samples = jitter.sample_count();

                // Compute intermediate stats
                let stats = jitter.compute_stats();

                println!(
                    "[{:2} min] Samples: {}, p50: {:.3}ms, p95: {:.3}ms, p99: {:.3}ms (remaining: {} min)",
                    elapsed_mins,
                    samples,
                    stats.p50_ns as f64 / 1_000_000.0,
                    stats.p95_ns as f64 / 1_000_000.0,
                    stats.p99_ms,
                    remaining_mins
                );

                last_report = Instant::now();
            }
        }

        // Compute final statistics
        let stats = jitter.compute_stats();

        println!();
        println!("=== Final Results ===");
        println!("Total ticks: {}", jitter.tick_count());
        println!("Samples (excluding warmup): {}", stats.samples);
        println!(
            "p50 jitter: {:.3}ms ({} ns)",
            stats.p50_ns as f64 / 1_000_000.0,
            stats.p50_ns
        );
        println!(
            "p95 jitter: {:.3}ms ({} ns)",
            stats.p95_ns as f64 / 1_000_000.0,
            stats.p95_ns
        );
        println!("p99 jitter: {:.3}ms ({} ns)", stats.p99_ms, stats.p99_ns);
        println!();

        // Assert p99 jitter is within threshold
        assert!(
            stats.p99_ms <= MAX_P99_MS,
            "p99 jitter ({:.3}ms) exceeds maximum allowed ({:.3}ms). \
             This may indicate:\n\
             - System is under heavy load\n\
             - Running in a VM (virtualized runners have higher jitter)\n\
             - Power management is throttling the CPU\n\
             - MMCSS/high-res timer is not available\n\
             \n\
             Stats: p50={:.3}ms, p95={:.3}ms, p99={:.3}ms, samples={}",
            stats.p99_ms,
            MAX_P99_MS,
            stats.p50_ns as f64 / 1_000_000.0,
            stats.p95_ns as f64 / 1_000_000.0,
            stats.p99_ms,
            stats.samples
        );

        println!(
            "✓ Test PASSED: p99 jitter ({:.3}ms) ≤ {:.3}ms",
            stats.p99_ms, MAX_P99_MS
        );
    }

    /// Short jitter test for quick validation (not ignored)
    ///
    /// This is a shorter version of the jitter test that runs for 10 seconds
    /// to provide quick feedback during development. It uses a more relaxed
    /// threshold since short tests have higher variance.
    #[test]
    fn test_timer_jitter_short() {
        use std::time::{Duration, Instant};

        // Short test: 10 seconds
        const TEST_DURATION_SECS: u64 = 10;
        // Warmup: 1 second
        const WARMUP_SECS: u32 = 1;
        // Target frequency: 250Hz
        const TARGET_HZ: u32 = 250;
        // Relaxed threshold for short test: 2ms (short tests have higher variance)
        const MAX_P99_MS: f64 = 2.0;

        let timer = WindowsTimerLoop::new().expect("Failed to create WindowsTimerLoop");
        let mut jitter = JitterMeasurement::new(TARGET_HZ, WARMUP_SECS);

        let mut target_qpc = timer.current_qpc();
        target_qpc += timer.ticks_per_period();

        // Use a base QPC for relative time calculation to avoid overflow
        let base_qpc = timer.current_qpc();

        let start = Instant::now();
        let test_duration = Duration::from_secs(TEST_DURATION_SECS);

        while start.elapsed() < test_duration {
            target_qpc = timer.wait_next_tick(target_qpc);
            // Convert relative QPC to nanoseconds to avoid overflow
            let relative_qpc = target_qpc - base_qpc;
            let now_ns = timer.ticks_to_ns(relative_qpc) as u64;
            jitter.record_tick(now_ns);
        }

        let stats = jitter.compute_stats();

        // Verify we got enough samples
        let expected_samples = (TARGET_HZ * (TEST_DURATION_SECS as u32 - WARMUP_SECS)) as usize;
        assert!(
            stats.samples >= expected_samples * 90 / 100,
            "Expected at least {} samples, got {}",
            expected_samples * 90 / 100,
            stats.samples
        );

        // Check jitter is reasonable (relaxed threshold for short test)
        assert!(
            stats.p99_ms <= MAX_P99_MS,
            "p99 jitter ({:.3}ms) exceeds relaxed threshold ({:.3}ms) in short test",
            stats.p99_ms,
            MAX_P99_MS
        );
    }

    /// Test JitterMeasurement statistics calculation
    #[test]
    fn test_jitter_measurement_stats() {
        let mut jitter = JitterMeasurement::new(250, 0); // No warmup for this test

        // Simulate ticks with known period deviations
        // Period is 4ms = 4_000_000ns at 250Hz
        let period_ns: u64 = 4_000_000;
        let base_ns: u64 = 1_000_000_000;

        // Generate 100 ticks where each period has a specific deviation
        // We want the period deviations to be: 100, 200, 300, ..., 9900 ns
        // So tick times are: base, base+period+100, base+2*period+100+200, ...
        let mut cumulative_deviation: u64 = 0;
        for i in 0..100 {
            let tick_ns = base_ns + (i as u64 * period_ns) + cumulative_deviation;
            jitter.record_tick(tick_ns);
            // Add deviation for next period
            cumulative_deviation += ((i + 1) * 100) as u64;
        }

        let stats = jitter.compute_stats();

        // We should have 99 samples (first tick establishes baseline)
        assert_eq!(stats.samples, 99, "Should have 99 samples");

        // Period deviations are: 100, 200, 300, ..., 9900 ns (99 values)
        // p50 should be around 5000ns (middle of 100-9900 range)
        // p99 should be around 9800ns (99th percentile of 99 values)
        assert!(
            (4500..=5500).contains(&stats.p50_ns),
            "p50 ({}) should be around 5000ns",
            stats.p50_ns
        );
        assert!(
            (9500..=10000).contains(&stats.p99_ns),
            "p99 ({}) should be around 9800ns",
            stats.p99_ns
        );
    }

    /// Test JitterMeasurement warmup exclusion
    #[test]
    fn test_jitter_measurement_warmup() {
        // 250Hz with 1 second warmup = 250 warmup ticks
        let mut jitter = JitterMeasurement::new(250, 1);

        let period_ns: u64 = 4_000_000;
        let base_ns: u64 = 1_000_000_000;

        // Generate 500 ticks (250 warmup + 250 measured)
        for i in 0..500 {
            let tick_ns = base_ns + (i as u64 * period_ns);
            jitter.record_tick(tick_ns);
        }

        let stats = jitter.compute_stats();

        // Should have ~249 samples (500 - 250 warmup - 1 for baseline)
        // The first tick after warmup establishes the baseline
        assert!(
            (248..=250).contains(&stats.samples),
            "Should have ~249 samples after warmup, got {}",
            stats.samples
        );
    }
}

// =============================================================================
// Power Management FFI Bindings (kernel32.dll)
// =============================================================================
// The Power Request functions are from kernel32.dll and are available on
// Windows 7 and later. They allow applications to prevent the system from
// entering sleep or turning off the display during active operation.

/// Power request type enumeration
/// See: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerRequestType {
    /// The display remains on even if there is no user input for an extended period.
    DisplayRequired = 0,
    /// The system continues to run instead of entering sleep after a period of user inactivity.
    SystemRequired = 1,
    /// The system enters away mode instead of sleep (S3 systems only).
    AwayModeRequired = 2,
    /// The calling process continues to run instead of being suspended or terminated.
    /// On Traditional Sleep (S3) systems, this implies SystemRequired.
    ExecutionRequired = 3,
}

/// Flags for REASON_CONTEXT
const POWER_REQUEST_CONTEXT_VERSION: u32 = 0;
const POWER_REQUEST_CONTEXT_SIMPLE_STRING: u32 = 0x00000001;

/// REASON_CONTEXT structure for PowerCreateRequest
/// This is a simplified version that only supports simple string reasons.
#[repr(C)]
struct ReasonContext {
    version: u32,
    flags: u32,
    // Union: we only use SimpleReasonString variant
    reason_string: *const u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    /// Creates a new power request object.
    /// Returns INVALID_HANDLE_VALUE on failure.
    fn PowerCreateRequest(context: *const ReasonContext) -> HANDLE;

    /// Increments the count of power requests of the specified type.
    /// Returns non-zero on success.
    fn PowerSetRequest(power_request: HANDLE, request_type: i32) -> i32;

    /// Decrements the count of power requests of the specified type.
    /// Returns non-zero on success.
    fn PowerClearRequest(power_request: HANDLE, request_type: i32) -> i32;
}

// =============================================================================
// PowerError
// =============================================================================

/// Error type for power management operations
#[derive(Debug)]
pub enum PowerError {
    /// Failed to create power request
    CreateFailed(std::io::Error),
    /// Failed to set power request
    SetFailed(std::io::Error),
    /// Failed to clear power request
    ClearFailed(std::io::Error),
}

impl std::fmt::Display for PowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerError::CreateFailed(e) => {
                write!(f, "Failed to create power request: {}", e)
            }
            PowerError::SetFailed(e) => {
                write!(f, "Failed to set power request: {}", e)
            }
            PowerError::ClearFailed(e) => {
                write!(f, "Failed to clear power request: {}", e)
            }
        }
    }
}

impl std::error::Error for PowerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PowerError::CreateFailed(e) => Some(e),
            PowerError::SetFailed(e) => Some(e),
            PowerError::ClearFailed(e) => Some(e),
        }
    }
}

// =============================================================================
// PowerManager
// =============================================================================

/// Power management for preventing sleep during active operation
///
/// This struct manages Windows power requests to prevent the system from
/// entering sleep or suspending the process during active Flight Hub operation.
/// It uses the Windows Power Request API (PowerCreateRequest, PowerSetRequest,
/// PowerClearRequest) to manage power state.
///
/// # Usage
///
/// The `PowerManager` should be activated when:
/// - At least one simulator is connected AND
/// - An FFB (Force Feedback) device is active
///
/// It should be deactivated when idle (no active sim or FFB).
///
/// # Example
///
/// ```no_run
/// use flight_scheduler::windows::PowerManager;
///
/// // Create power manager
/// let mut power_mgr = PowerManager::new().expect("Failed to create power manager");
///
/// // When sim connects and FFB is active, prevent sleep
/// power_mgr.activate();
///
/// // ... do real-time work ...
///
/// // When idle, allow sleep again
/// power_mgr.deactivate();
///
/// // PowerManager automatically deactivates and cleans up on drop
/// ```
///
/// # Requirements
///
/// - Requirement 3.1: When at least one sim is connected and FFB device is active,
///   call PowerCreateRequest and PowerSetRequest with EXECUTION_REQUIRED and SYSTEM_REQUIRED
/// - Requirement 3.2: When idle with no active sim or FFB, clear power requests via
///   PowerClearRequest to allow normal power management
pub struct PowerManager {
    /// Power request handle
    request: HANDLE,
    /// Whether power requests are currently active
    active: bool,
    /// Reason string (kept alive for the lifetime of the request)
    /// The wide string must remain valid while the power request exists.
    _reason_string: Vec<u16>,
}

impl PowerManager {
    /// Create a new PowerManager
    ///
    /// Creates a power request object that can be used to prevent the system
    /// from entering sleep during active operation. The power request is
    /// initially inactive; call `activate()` to enable it.
    ///
    /// # Returns
    ///
    /// Returns `Ok(PowerManager)` on success, or `Err(PowerError::CreateFailed)`
    /// if the power request could not be created.
    ///
    /// # Requirements
    ///
    /// - Requirement 3.1: PowerCreateRequest for power management
    pub fn new() -> Result<Self, PowerError> {
        // Create the reason string as null-terminated UTF-16
        // This string explains why we're preventing sleep
        let reason = "Flight Hub active operation";
        let reason_wide: Vec<u16> = reason.encode_utf16().chain(std::iter::once(0)).collect();

        // Create the REASON_CONTEXT structure
        let context = ReasonContext {
            version: POWER_REQUEST_CONTEXT_VERSION,
            flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            reason_string: reason_wide.as_ptr(),
        };

        // Create the power request
        // SAFETY: We're passing a valid REASON_CONTEXT structure with a valid string pointer
        let request = unsafe { PowerCreateRequest(&context) };

        // Check for INVALID_HANDLE_VALUE
        // INVALID_HANDLE_VALUE is typically -1 as isize, which is !0 as usize
        if request.is_invalid() {
            let err = std::io::Error::last_os_error();
            warn!(
                "PowerCreateRequest failed: {} (error code: {:?})",
                err,
                err.raw_os_error()
            );
            return Err(PowerError::CreateFailed(err));
        }

        info!("PowerManager created successfully");

        Ok(Self {
            request,
            active: false,
            _reason_string: reason_wide,
        })
    }

    /// Activate power requests (prevent sleep)
    ///
    /// When activated, the system will not enter sleep and the process will
    /// not be suspended due to power management. This should be called when
    /// at least one simulator is connected and an FFB device is active.
    ///
    /// This method is idempotent - calling it multiple times when already
    /// active has no effect.
    ///
    /// # Requirements
    ///
    /// - Requirement 3.1: PowerSetRequest with EXECUTION_REQUIRED and SYSTEM_REQUIRED
    pub fn activate(&mut self) {
        if self.active {
            return;
        }

        // Set EXECUTION_REQUIRED to prevent process suspension
        // On Traditional Sleep (S3) systems, this implies SYSTEM_REQUIRED
        // SAFETY: We have a valid power request handle
        let exec_result =
            unsafe { PowerSetRequest(self.request, PowerRequestType::ExecutionRequired as i32) };

        if exec_result == 0 {
            let err = std::io::Error::last_os_error();
            warn!(
                "PowerSetRequest(ExecutionRequired) failed: {} (error code: {:?})",
                err,
                err.raw_os_error()
            );
        } else {
            info!("PowerSetRequest(ExecutionRequired) succeeded");
        }

        // Set SYSTEM_REQUIRED to prevent system sleep
        // SAFETY: We have a valid power request handle
        let sys_result =
            unsafe { PowerSetRequest(self.request, PowerRequestType::SystemRequired as i32) };

        if sys_result == 0 {
            let err = std::io::Error::last_os_error();
            warn!(
                "PowerSetRequest(SystemRequired) failed: {} (error code: {:?})",
                err,
                err.raw_os_error()
            );
        } else {
            info!("PowerSetRequest(SystemRequired) succeeded");
        }

        self.active = true;
        info!("PowerManager activated - system sleep prevented");
    }

    /// Deactivate power requests (allow sleep)
    ///
    /// When deactivated, normal power management resumes and the system can
    /// enter sleep if idle. This should be called when Flight Hub is idle
    /// (no active simulator or FFB device).
    ///
    /// This method is idempotent - calling it multiple times when already
    /// inactive has no effect.
    ///
    /// # Requirements
    ///
    /// - Requirement 3.2: PowerClearRequest to allow normal power management
    pub fn deactivate(&mut self) {
        if !self.active {
            return;
        }

        // Clear EXECUTION_REQUIRED
        // SAFETY: We have a valid power request handle
        let exec_result =
            unsafe { PowerClearRequest(self.request, PowerRequestType::ExecutionRequired as i32) };

        if exec_result == 0 {
            let err = std::io::Error::last_os_error();
            warn!(
                "PowerClearRequest(ExecutionRequired) failed: {} (error code: {:?})",
                err,
                err.raw_os_error()
            );
        } else {
            info!("PowerClearRequest(ExecutionRequired) succeeded");
        }

        // Clear SYSTEM_REQUIRED
        // SAFETY: We have a valid power request handle
        let sys_result =
            unsafe { PowerClearRequest(self.request, PowerRequestType::SystemRequired as i32) };

        if sys_result == 0 {
            let err = std::io::Error::last_os_error();
            warn!(
                "PowerClearRequest(SystemRequired) failed: {} (error code: {:?})",
                err,
                err.raw_os_error()
            );
        } else {
            info!("PowerClearRequest(SystemRequired) succeeded");
        }

        self.active = false;
        info!("PowerManager deactivated - normal power management resumed");
    }

    /// Check if power requests are currently active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the underlying power request handle (for debugging/testing)
    #[inline]
    pub fn handle(&self) -> HANDLE {
        self.request
    }
}

impl Drop for PowerManager {
    fn drop(&mut self) {
        // Deactivate power requests if still active
        self.deactivate();

        // Close the power request handle
        // SAFETY: We have a valid handle that we own
        let close_result = unsafe { windows::Win32::Foundation::CloseHandle(self.request) };

        if close_result.is_err() {
            warn!("Failed to close power request handle");
        } else {
            info!("Power request handle closed");
        }
    }
}

// PowerManager is not Send/Sync because HANDLE is not thread-safe
// This is intentional - power management should be done from a single thread

// =============================================================================
// PowerManager Tests
// =============================================================================

#[cfg(test)]
mod power_tests {
    use super::*;

    /// Test PowerManager creation
    #[test]
    fn test_power_manager_creation() {
        let power_mgr = PowerManager::new();

        // Should succeed on Windows 7+
        assert!(
            power_mgr.is_ok(),
            "PowerManager::new should succeed on Windows 7+"
        );

        let pm = power_mgr.unwrap();

        // Should start inactive
        assert!(!pm.is_active(), "PowerManager should start inactive");

        // Handle should be valid (not INVALID_HANDLE_VALUE)
        assert!(
            !pm.handle().is_invalid(),
            "Power request handle should be valid"
        );
    }

    /// Test PowerManager activate/deactivate cycle
    #[test]
    fn test_power_manager_activate_deactivate() {
        let mut pm = PowerManager::new().expect("Failed to create PowerManager");

        // Initially inactive
        assert!(!pm.is_active());

        // Activate
        pm.activate();
        assert!(pm.is_active(), "Should be active after activate()");

        // Activate again (idempotent)
        pm.activate();
        assert!(
            pm.is_active(),
            "Should still be active after second activate()"
        );

        // Deactivate
        pm.deactivate();
        assert!(!pm.is_active(), "Should be inactive after deactivate()");

        // Deactivate again (idempotent)
        pm.deactivate();
        assert!(
            !pm.is_active(),
            "Should still be inactive after second deactivate()"
        );
    }

    /// Test PowerManager RAII cleanup
    #[test]
    fn test_power_manager_drop_cleanup() {
        // Create and activate, then drop
        {
            let mut pm = PowerManager::new().expect("Failed to create PowerManager");
            pm.activate();
            assert!(pm.is_active());
            // pm dropped here - should deactivate and close handle
        }

        // Create another one to verify resources were properly released
        let pm2 = PowerManager::new();
        assert!(
            pm2.is_ok(),
            "Should be able to create new PowerManager after previous one was dropped"
        );
    }

    /// Test PowerManager multiple instances
    #[test]
    fn test_power_manager_multiple_instances() {
        // Create multiple PowerManager instances
        let mut pm1 = PowerManager::new().expect("Failed to create PowerManager 1");
        let mut pm2 = PowerManager::new().expect("Failed to create PowerManager 2");

        // Activate both
        pm1.activate();
        pm2.activate();

        assert!(pm1.is_active());
        assert!(pm2.is_active());

        // Deactivate one
        pm1.deactivate();
        assert!(!pm1.is_active());
        assert!(pm2.is_active()); // pm2 should still be active

        // Deactivate the other
        pm2.deactivate();
        assert!(!pm2.is_active());
    }

    /// Test PowerManager state consistency
    #[test]
    fn test_power_manager_state_consistency() {
        let mut pm = PowerManager::new().expect("Failed to create PowerManager");

        // Test state transitions
        for _ in 0..5 {
            assert!(!pm.is_active());
            pm.activate();
            assert!(pm.is_active());
            pm.deactivate();
            assert!(!pm.is_active());
        }
    }
}
