// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Unix-specific scheduler implementation
//!
//! Uses clock_nanosleep with CLOCK_MONOTONIC for high-precision timing
//! and SCHED_FIFO via rtkit for real-time performance.
//!
//! # Real-Time Thread Configuration
//!
//! The [`LinuxRtThread`] struct provides RAII-based management of Linux real-time
//! thread settings including:
//! - rtkit D-Bus integration for unprivileged RT scheduling
//! - Fallback to direct `sched_setscheduler` with SCHED_FIFO
//! - Memory locking via `mlockall` to prevent page faults
//! - RLIMIT validation for rtprio and memlock
//!
//! # Requirements Coverage
//!
//! - Requirement 5.1: rtkit D-Bus integration via MakeThreadRealtime
//! - Requirement 5.2: Fallback to sched_setscheduler with SCHED_FIFO
//! - Requirement 5.3: Fallback to normal priority with warnings and metrics
//! - Requirement 5.4: mlockall(MCL_CURRENT | MCL_FUTURE) when RT enabled
//! - Requirement 5.5: Validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK limits

use libc::{self, CLOCK_MONOTONIC, timespec};
use std::time::Duration;
use tracing::{info, warn};

// =============================================================================
// Error Types
// =============================================================================

/// Error type for real-time thread configuration
#[derive(Debug)]
pub enum RtError {
    /// rtkit D-Bus connection failed
    DbusConnection(String),
    /// rtkit MakeThreadRealtime call failed
    RtkitFailed(String),
    /// sched_setscheduler failed
    SchedSetschedulerFailed(std::io::Error),
    /// mlockall failed
    MlockallFailed(std::io::Error),
    /// RLIMIT check failed
    RlimitCheckFailed(String),
}

impl std::fmt::Display for RtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RtError::DbusConnection(e) => write!(f, "D-Bus connection failed: {}", e),
            RtError::RtkitFailed(e) => write!(f, "rtkit MakeThreadRealtime failed: {}", e),
            RtError::SchedSetschedulerFailed(e) => write!(f, "sched_setscheduler failed: {}", e),
            RtError::MlockallFailed(e) => write!(f, "mlockall failed: {}", e),
            RtError::RlimitCheckFailed(e) => write!(f, "RLIMIT check failed: {}", e),
        }
    }
}

impl std::error::Error for RtError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RtError::SchedSetschedulerFailed(e) | RtError::MlockallFailed(e) => Some(e),
            _ => None,
        }
    }
}

// =============================================================================
// RLIMIT Validation
// =============================================================================

/// RLIMIT validation results
#[derive(Debug, Clone)]
pub struct RlimitStatus {
    /// Current RLIMIT_RTPRIO soft limit
    pub rtprio_soft: u64,
    /// Current RLIMIT_RTPRIO hard limit
    pub rtprio_hard: u64,
    /// Current RLIMIT_MEMLOCK soft limit (bytes)
    pub memlock_soft: u64,
    /// Current RLIMIT_MEMLOCK hard limit (bytes)
    pub memlock_hard: u64,
    /// Whether rtprio limit is sufficient for RT scheduling
    pub rtprio_sufficient: bool,
    /// Whether memlock limit is sufficient for mlockall
    pub memlock_sufficient: bool,
    /// Warning messages for insufficient limits
    pub warnings: Vec<String>,
}

impl RlimitStatus {
    /// Check if all limits are sufficient for RT operation
    pub fn is_sufficient(&self) -> bool {
        self.rtprio_sufficient && self.memlock_sufficient
    }
}

/// Validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK limits
///
/// Checks if the current process has sufficient resource limits for
/// real-time scheduling and memory locking.
///
/// # Requirements
///
/// - Requirement 5.5: Validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK limits
pub fn validate_rlimits(requested_priority: i32) -> RlimitStatus {
    let mut status = RlimitStatus {
        rtprio_soft: 0,
        rtprio_hard: 0,
        memlock_soft: 0,
        memlock_hard: 0,
        rtprio_sufficient: false,
        memlock_sufficient: false,
        warnings: Vec::new(),
    };

    // Check RLIMIT_RTPRIO
    unsafe {
        let mut rlimit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };

        if libc::getrlimit(libc::RLIMIT_RTPRIO, &mut rlimit) == 0 {
            status.rtprio_soft = rlimit.rlim_cur;
            status.rtprio_hard = rlimit.rlim_max;

            // Check if we can set the requested priority
            // RLIMIT_RTPRIO of 0 means no RT scheduling allowed (without CAP_SYS_NICE)
            // A value >= requested_priority means we can use that priority
            if rlimit.rlim_cur == libc::RLIM_INFINITY
                || rlimit.rlim_cur >= requested_priority as u64
            {
                status.rtprio_sufficient = true;
            } else if rlimit.rlim_cur == 0 {
                status.warnings.push(format!(
                    "RLIMIT_RTPRIO is 0. RT scheduling requires CAP_SYS_NICE or rtkit. \
                     Consider adding to /etc/security/limits.conf: \
                     @audio - rtprio {}",
                    requested_priority
                ));
            } else {
                status.warnings.push(format!(
                    "RLIMIT_RTPRIO ({}) is less than requested priority ({}). \
                     Consider increasing in /etc/security/limits.conf",
                    rlimit.rlim_cur, requested_priority
                ));
            }
        } else {
            status
                .warnings
                .push("Failed to query RLIMIT_RTPRIO".to_string());
        }
    }

    // Check RLIMIT_MEMLOCK
    unsafe {
        let mut rlimit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };

        if libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut rlimit) == 0 {
            status.memlock_soft = rlimit.rlim_cur;
            status.memlock_hard = rlimit.rlim_max;

            // For mlockall, we need unlimited or at least enough for our working set
            // A reasonable minimum is 64MB for Flight Hub
            const MIN_MEMLOCK_BYTES: u64 = 64 * 1024 * 1024; // 64MB

            if rlimit.rlim_cur == libc::RLIM_INFINITY || rlimit.rlim_cur >= MIN_MEMLOCK_BYTES {
                status.memlock_sufficient = true;
            } else {
                status.warnings.push(format!(
                    "RLIMIT_MEMLOCK ({} bytes) may be insufficient for mlockall. \
                     Consider adding to /etc/security/limits.conf: \
                     @audio - memlock unlimited",
                    rlimit.rlim_cur
                ));
            }
        } else {
            status
                .warnings
                .push("Failed to query RLIMIT_MEMLOCK".to_string());
        }
    }

    status
}

// =============================================================================
// rtkit D-Bus Integration
// =============================================================================

/// Try to acquire RT scheduling via rtkit D-Bus interface
///
/// rtkit (RealtimeKit) is a D-Bus service that allows unprivileged processes
/// to acquire real-time scheduling. It's the preferred method on modern Linux
/// systems as it doesn't require root or special capabilities.
///
/// # Arguments
///
/// * `priority` - RT priority to request (1-99 for SCHED_FIFO)
///
/// # Requirements
///
/// - Requirement 5.1: rtkit D-Bus integration via MakeThreadRealtime
fn try_rtkit(priority: i32) -> Result<(), RtError> {
    // Get the current thread ID using gettid syscall
    let thread_id = unsafe { libc::syscall(libc::SYS_gettid) } as u64;

    // Try to connect to rtkit via D-Bus
    // We use a simple approach: spawn dbus-send command
    // This avoids adding a heavy D-Bus library dependency
    //
    // The rtkit D-Bus interface:
    // - Service: org.freedesktop.RealtimeKit1
    // - Object: /org/freedesktop/RealtimeKit1
    // - Interface: org.freedesktop.RealtimeKit1
    // - Method: MakeThreadRealtime(uint64 thread_id, uint32 priority)

    let output = std::process::Command::new("dbus-send")
        .args([
            "--system",
            "--print-reply",
            "--dest=org.freedesktop.RealtimeKit1",
            "/org/freedesktop/RealtimeKit1",
            "org.freedesktop.RealtimeKit1.MakeThreadRealtime",
            &format!("uint64:{}", thread_id),
            &format!("uint32:{}", priority),
        ])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                Err(RtError::RtkitFailed(format!(
                    "rtkit denied request: {}",
                    stderr.trim()
                )))
            }
        }
        Err(e) => {
            // dbus-send not available or failed to execute
            Err(RtError::DbusConnection(format!(
                "Failed to execute dbus-send: {}. Is dbus-send installed?",
                e
            )))
        }
    }
}

// =============================================================================
// LinuxRtThread
// =============================================================================

/// Linux real-time thread configuration
///
/// Provides RAII-based management of Linux real-time thread settings including:
/// - rtkit D-Bus integration for unprivileged RT scheduling
/// - Fallback to direct `sched_setscheduler` with SCHED_FIFO
/// - Memory locking via `mlockall` to prevent page faults
/// - RLIMIT validation for rtprio and memlock
///
/// When dropped, the thread remains at its current scheduling policy
/// (no automatic restoration, as this is typically desired for RT threads).
///
/// # Example
///
/// ```no_run
/// use flight_scheduler::unix::LinuxRtThread;
///
/// // Configure RT thread with priority 10
/// let rt_thread = LinuxRtThread::new(10).expect("RT thread setup failed");
///
/// // Check if RT was acquired
/// let metrics = rt_thread.metrics();
/// if metrics.rt_enabled {
///     println!("Running with RT priority {}", metrics.priority);
/// } else {
///     println!("Running at normal priority (RT unavailable)");
/// }
/// ```
///
/// # Requirements
///
/// - Requirement 5.1: rtkit D-Bus integration via MakeThreadRealtime
/// - Requirement 5.2: Fallback to sched_setscheduler with SCHED_FIFO
/// - Requirement 5.3: Fallback to normal priority with warnings and metrics
/// - Requirement 5.4: mlockall(MCL_CURRENT | MCL_FUTURE) when RT enabled
/// - Requirement 5.5: Validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK limits
pub struct LinuxRtThread {
    /// Whether RT scheduling was acquired
    rt_enabled: bool,
    /// Scheduling policy (SCHED_FIFO, SCHED_OTHER, etc.)
    sched_policy: i32,
    /// RT priority (1-99 for SCHED_FIFO, 0 for SCHED_OTHER)
    priority: i32,
    /// Whether mlockall succeeded
    mlockall_success: bool,
    /// RLIMIT validation status
    rlimit_status: RlimitStatus,
}

impl LinuxRtThread {
    /// Create and configure an RT thread
    ///
    /// Attempts to acquire real-time scheduling in the following order:
    /// 1. rtkit D-Bus interface (works without root)
    /// 2. Direct sched_setscheduler (requires CAP_SYS_NICE or root)
    /// 3. Falls back to normal priority with warnings
    ///
    /// If RT scheduling is acquired, also calls mlockall to prevent page faults.
    ///
    /// # Arguments
    ///
    /// * `priority` - Requested RT priority (1-99 for SCHED_FIFO)
    ///
    /// # Returns
    ///
    /// Always returns `Ok(LinuxRtThread)` - failures are logged as warnings
    /// and the thread continues at normal priority (graceful degradation).
    ///
    /// # Requirements
    ///
    /// - Requirement 5.1: rtkit D-Bus integration
    /// - Requirement 5.2: Fallback to sched_setscheduler
    /// - Requirement 5.3: Fallback to normal priority with warnings
    /// - Requirement 5.4: mlockall on RT success
    /// - Requirement 5.5: Validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK
    pub fn new(priority: i32) -> Result<Self, RtError> {
        // Clamp priority to valid SCHED_FIFO range (1-99)
        let priority = priority.clamp(1, 99);

        // Validate RLIMIT_RTPRIO and RLIMIT_MEMLOCK first
        let rlimit_status = validate_rlimits(priority);

        // Log any RLIMIT warnings
        for warning in &rlimit_status.warnings {
            warn!("{}", warning);
        }

        let mut rt_enabled = false;
        let mut sched_policy = libc::SCHED_OTHER;
        let mut actual_priority = 0;

        // Try rtkit first (works without root)
        // Requirement 5.1: rtkit D-Bus integration
        match try_rtkit(priority) {
            Ok(()) => {
                rt_enabled = true;
                sched_policy = libc::SCHED_FIFO;
                actual_priority = priority;
                info!("RT scheduling acquired via rtkit (priority {})", priority);
            }
            Err(e) => {
                warn!("rtkit failed: {}, trying direct sched_setscheduler", e);

                // Requirement 5.2: Fallback to sched_setscheduler
                // Try direct sched_setscheduler (requires CAP_SYS_NICE or root)
                let param = libc::sched_param {
                    sched_priority: priority,
                };

                let result = unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) };

                if result == 0 {
                    rt_enabled = true;
                    sched_policy = libc::SCHED_FIFO;
                    actual_priority = priority;
                    info!(
                        "RT scheduling acquired via sched_setscheduler (priority {})",
                        priority
                    );
                } else {
                    // Requirement 5.3: Fallback to normal priority with warnings
                    let err = std::io::Error::last_os_error();
                    warn!(
                        "RT scheduling unavailable: {}. Running at normal priority. \
                         Consider installing rtkit or configuring /etc/security/limits.conf",
                        err
                    );
                }
            }
        }

        // Requirement 5.4: mlockall on RT success
        let mlockall_success = if rt_enabled {
            let result = unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) };

            if result != 0 {
                let err = std::io::Error::last_os_error();
                warn!(
                    "mlockall failed: {}. Page faults may occur in RT threads. \
                     Consider increasing RLIMIT_MEMLOCK.",
                    err
                );
                false
            } else {
                info!("mlockall(MCL_CURRENT | MCL_FUTURE) succeeded");
                true
            }
        } else {
            // Don't try mlockall if we're not running RT
            false
        };

        Ok(Self {
            rt_enabled,
            sched_policy,
            priority: actual_priority,
            mlockall_success,
            rlimit_status,
        })
    }

    /// Check if RT scheduling was acquired
    #[inline]
    pub fn is_rt_enabled(&self) -> bool {
        self.rt_enabled
    }

    /// Get the current scheduling policy
    #[inline]
    pub fn sched_policy(&self) -> i32 {
        self.sched_policy
    }

    /// Get the current RT priority (0 if not RT)
    #[inline]
    pub fn priority(&self) -> i32 {
        self.priority
    }

    /// Check if mlockall succeeded
    #[inline]
    pub fn is_mlockall_success(&self) -> bool {
        self.mlockall_success
    }

    /// Get RLIMIT validation status
    #[inline]
    pub fn rlimit_status(&self) -> &RlimitStatus {
        &self.rlimit_status
    }

    /// Get metrics for observability
    ///
    /// Returns a snapshot of the RT thread configuration for monitoring
    /// and debugging purposes.
    ///
    /// # Requirements
    ///
    /// - Requirement 7.1: Expose runtime.linux.rt_enabled, sched_policy, priority, mlockall_success
    pub fn metrics(&self) -> LinuxRtMetrics {
        LinuxRtMetrics {
            rt_enabled: self.rt_enabled,
            sched_policy: self.sched_policy,
            priority: self.priority,
            mlockall_success: self.mlockall_success,
        }
    }

    /// Get a human-readable description of the scheduling policy
    pub fn sched_policy_name(&self) -> &'static str {
        match self.sched_policy {
            libc::SCHED_FIFO => "SCHED_FIFO",
            libc::SCHED_RR => "SCHED_RR",
            libc::SCHED_OTHER => "SCHED_OTHER",
            libc::SCHED_BATCH => "SCHED_BATCH",
            libc::SCHED_IDLE => "SCHED_IDLE",
            _ => "UNKNOWN",
        }
    }
}

// Note: We don't implement Drop to restore scheduling policy because:
// 1. RT threads typically want to stay RT until they exit
// 2. Restoring to SCHED_OTHER could cause timing issues
// 3. mlockall cannot be easily undone (munlockall would unlock all memory)

/// Metrics for Linux RT status
///
/// Provides observability into the RT thread configuration for monitoring,
/// debugging, and validation purposes.
///
/// # Requirements
///
/// - Requirement 7.1: Expose runtime.linux.rt_enabled, sched_policy, priority, mlockall_success
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxRtMetrics {
    /// Whether RT scheduling was acquired
    pub rt_enabled: bool,
    /// Scheduling policy (SCHED_FIFO, SCHED_OTHER, etc.)
    pub sched_policy: i32,
    /// RT priority (1-99 for SCHED_FIFO, 0 for SCHED_OTHER)
    pub priority: i32,
    /// Whether mlockall succeeded
    pub mlockall_success: bool,
}

impl Default for LinuxRtMetrics {
    fn default() -> Self {
        Self {
            rt_enabled: false,
            sched_policy: libc::SCHED_OTHER,
            priority: 0,
            mlockall_success: false,
        }
    }
}

impl LinuxRtMetrics {
    /// Get a human-readable description of the scheduling policy
    pub fn sched_policy_name(&self) -> &'static str {
        match self.sched_policy {
            libc::SCHED_FIFO => "SCHED_FIFO",
            libc::SCHED_RR => "SCHED_RR",
            libc::SCHED_OTHER => "SCHED_OTHER",
            libc::SCHED_BATCH => "SCHED_BATCH",
            libc::SCHED_IDLE => "SCHED_IDLE",
            _ => "UNKNOWN",
        }
    }
}

// =============================================================================
// LinuxTimerLoop - High-Resolution 250Hz Timer
// =============================================================================

/// High-resolution 250Hz timer loop for Linux
///
/// Implements a precise timing loop using `clock_nanosleep` with absolute target
/// times and a busy-spin finish for the final 50μs to minimize jitter.
///
/// # Design
///
/// The timer loop uses a two-phase approach:
/// 1. **Sleep phase**: Uses `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)` to
///    sleep until 50μs before the target time. This is efficient and doesn't
///    consume CPU while waiting.
/// 2. **Busy-spin phase**: Spins using `clock_gettime(CLOCK_MONOTONIC)` for the
///    final 50μs to achieve sub-microsecond precision.
///
/// # Example
///
/// ```no_run
/// use flight_scheduler::unix::LinuxTimerLoop;
///
/// let mut timer = LinuxTimerLoop::new();
///
/// loop {
///     // Wait for next 250Hz tick
///     let actual_time = timer.wait_next_tick();
///     
///     // Process axis data, FFB, etc.
///     // ...
/// }
/// ```
///
/// # Requirements
///
/// - Requirement 6.1: Use clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME) with absolute target times
/// - Requirement 6.2: Busy-spin for final 50μs using clock_gettime(CLOCK_MONOTONIC)
pub struct LinuxTimerLoop {
    /// Target period in nanoseconds (4ms = 250Hz)
    period_ns: i64,
    /// Next absolute target time
    next_target: libc::timespec,
    /// Busy-spin threshold in nanoseconds (50μs before target)
    spin_threshold_ns: i64,
}

impl LinuxTimerLoop {
    /// Default period for 250Hz (4ms in nanoseconds)
    pub const DEFAULT_PERIOD_NS: i64 = 4_000_000;

    /// Default busy-spin threshold (50μs in nanoseconds)
    pub const DEFAULT_SPIN_THRESHOLD_NS: i64 = 50_000;

    /// One second in nanoseconds
    const NANOS_PER_SEC: i64 = 1_000_000_000;

    /// Create a 250Hz timer loop
    ///
    /// Initializes the timer with the current monotonic time as the starting
    /// point. The first call to `wait_next_tick()` will wait for one full
    /// period (4ms) from this starting point.
    ///
    /// # Requirements
    ///
    /// - Requirement 6.1: Use clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)
    pub fn new() -> Self {
        let mut now = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        // Get current monotonic time as starting point
        // SAFETY: clock_gettime is safe to call with valid pointers
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now);
        }

        Self {
            period_ns: Self::DEFAULT_PERIOD_NS,
            next_target: now,
            spin_threshold_ns: Self::DEFAULT_SPIN_THRESHOLD_NS,
        }
    }

    /// Create a timer loop with custom period
    ///
    /// # Arguments
    ///
    /// * `period_ns` - Target period in nanoseconds (e.g., 4_000_000 for 250Hz)
    ///
    /// # Panics
    ///
    /// Panics if `period_ns` is less than or equal to the spin threshold (50μs).
    pub fn with_period(period_ns: i64) -> Self {
        assert!(
            period_ns > Self::DEFAULT_SPIN_THRESHOLD_NS,
            "Period ({} ns) must be greater than spin threshold ({} ns)",
            period_ns,
            Self::DEFAULT_SPIN_THRESHOLD_NS
        );

        let mut timer = Self::new();
        timer.period_ns = period_ns;
        timer
    }

    /// Create a timer loop with custom period and spin threshold
    ///
    /// # Arguments
    ///
    /// * `period_ns` - Target period in nanoseconds
    /// * `spin_threshold_ns` - Busy-spin threshold in nanoseconds (time before target to start spinning)
    ///
    /// # Panics
    ///
    /// Panics if `period_ns` is less than or equal to `spin_threshold_ns`.
    pub fn with_period_and_threshold(period_ns: i64, spin_threshold_ns: i64) -> Self {
        assert!(
            period_ns > spin_threshold_ns,
            "Period ({} ns) must be greater than spin threshold ({} ns)",
            period_ns,
            spin_threshold_ns
        );
        assert!(
            spin_threshold_ns >= 0,
            "Spin threshold must be non-negative"
        );

        let mut timer = Self::new();
        timer.period_ns = period_ns;
        timer.spin_threshold_ns = spin_threshold_ns;
        timer
    }

    /// Get the target period in nanoseconds
    #[inline]
    pub fn period_ns(&self) -> i64 {
        self.period_ns
    }

    /// Get the target frequency in Hz
    #[inline]
    pub fn frequency_hz(&self) -> f64 {
        Self::NANOS_PER_SEC as f64 / self.period_ns as f64
    }

    /// Get the busy-spin threshold in nanoseconds
    #[inline]
    pub fn spin_threshold_ns(&self) -> i64 {
        self.spin_threshold_ns
    }

    /// Get the next target time
    #[inline]
    pub fn next_target(&self) -> libc::timespec {
        self.next_target
    }

    /// Wait for next tick with busy-spin finish
    ///
    /// This method:
    /// 1. Advances the target time by one period
    /// 2. Sleeps using `clock_nanosleep` until 50μs before the target
    /// 3. Busy-spins for the final 50μs using `clock_gettime`
    ///
    /// # Returns
    ///
    /// The actual time when the tick completed (from `clock_gettime`).
    ///
    /// # Requirements
    ///
    /// - Requirement 6.1: Use clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME) with absolute target times
    /// - Requirement 6.2: Busy-spin for final 50μs using clock_gettime(CLOCK_MONOTONIC)
    pub fn wait_next_tick(&mut self) -> libc::timespec {
        // Advance target time by one period
        self.advance_target();

        // Calculate busy-spin threshold (spin_threshold_ns before target)
        let spin_target = self.calculate_spin_target();

        // Sleep until spin threshold using clock_nanosleep with absolute time
        // Requirement 6.1: Use clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)
        // SAFETY: clock_nanosleep is safe to call with valid pointers
        unsafe {
            libc::clock_nanosleep(
                libc::CLOCK_MONOTONIC,
                libc::TIMER_ABSTIME,
                &spin_target,
                std::ptr::null_mut(),
            );
        }

        // Busy-spin for final portion
        // Requirement 6.2: Busy-spin for final 50μs using clock_gettime(CLOCK_MONOTONIC)
        self.busy_spin_until_target()
    }

    /// Advance the target time by one period
    ///
    /// Handles nanosecond overflow by incrementing seconds when necessary.
    #[inline]
    fn advance_target(&mut self) {
        self.next_target.tv_nsec += self.period_ns;

        // Handle nanosecond overflow
        while self.next_target.tv_nsec >= Self::NANOS_PER_SEC {
            self.next_target.tv_nsec -= Self::NANOS_PER_SEC;
            self.next_target.tv_sec += 1;
        }
    }

    /// Calculate the time to start busy-spinning (spin_threshold_ns before target)
    #[inline]
    fn calculate_spin_target(&self) -> libc::timespec {
        let mut spin_target = self.next_target;
        spin_target.tv_nsec -= self.spin_threshold_ns;

        // Handle nanosecond underflow
        if spin_target.tv_nsec < 0 {
            spin_target.tv_nsec += Self::NANOS_PER_SEC;
            spin_target.tv_sec -= 1;
        }

        spin_target
    }

    /// Busy-spin until the target time is reached
    ///
    /// Uses `clock_gettime(CLOCK_MONOTONIC)` in a tight loop with `spin_loop`
    /// hint for optimal CPU behavior.
    ///
    /// # Returns
    ///
    /// The actual time when the target was reached.
    #[inline]
    fn busy_spin_until_target(&self) -> libc::timespec {
        let mut now = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        loop {
            // SAFETY: clock_gettime is safe to call with valid pointers
            unsafe {
                libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now);
            }

            // Check if we've reached or passed the target time
            if self.time_reached(&now) {
                break;
            }

            // Hint to the CPU that we're in a spin loop
            std::hint::spin_loop();
        }

        now
    }

    /// Check if the current time has reached or passed the target time
    #[inline]
    fn time_reached(&self, now: &libc::timespec) -> bool {
        now.tv_sec > self.next_target.tv_sec
            || (now.tv_sec == self.next_target.tv_sec && now.tv_nsec >= self.next_target.tv_nsec)
    }

    /// Reset the timer to the current time
    ///
    /// Useful for resynchronizing after a pause or when the timer has drifted
    /// significantly.
    pub fn reset(&mut self) {
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut self.next_target);
        }
    }

    /// Get the current monotonic time
    ///
    /// Utility method for measuring elapsed time or jitter.
    pub fn current_time() -> libc::timespec {
        let mut now = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now);
        }
        now
    }

    /// Convert a timespec to nanoseconds since epoch
    ///
    /// Utility method for time calculations.
    #[inline]
    pub fn timespec_to_nanos(ts: &libc::timespec) -> i64 {
        ts.tv_sec * Self::NANOS_PER_SEC + ts.tv_nsec
    }

    /// Calculate the difference between two timespecs in nanoseconds
    ///
    /// Returns `a - b` in nanoseconds.
    #[inline]
    pub fn timespec_diff_nanos(a: &libc::timespec, b: &libc::timespec) -> i64 {
        Self::timespec_to_nanos(a) - Self::timespec_to_nanos(b)
    }
}

impl Default for LinuxTimerLoop {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Platform Sleep
// =============================================================================

/// Platform-specific sleep implementation for Unix
///
/// Uses clock_nanosleep with CLOCK_MONOTONIC for high-precision timing.
pub fn platform_sleep(duration: Duration) {
    let sleep_time = timespec {
        tv_sec: duration.as_secs() as libc::time_t,
        tv_nsec: duration.subsec_nanos() as libc::c_long,
    };

    unsafe {
        // Use clock_nanosleep for high precision
        let result = libc::clock_nanosleep(
            CLOCK_MONOTONIC,
            0, // Relative time
            &sleep_time,
            std::ptr::null_mut(),
        );

        if result != 0 {
            // Fallback to standard sleep on error
            std::thread::sleep(duration);
        }
    }
}

// =============================================================================
// Legacy Functions (for backward compatibility)
// =============================================================================

/// Set current thread to real-time priority using rtkit
///
/// This is a legacy function. Prefer using [`LinuxRtThread::new()`] instead.
#[deprecated(since = "0.1.0", note = "Use LinuxRtThread::new() instead")]
pub fn set_realtime_priority() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let rt = LinuxRtThread::new(50)?;
    if rt.is_rt_enabled() {
        Ok(())
    } else {
        Err("Failed to acquire RT scheduling".into())
    }
}

/// Check if system is configured for real-time performance
pub fn check_rt_configuration() -> RTConfigStatus {
    let rlimit_status = validate_rlimits(50); // Check with mid-range priority

    let mut issues = rlimit_status.warnings.clone();

    // Check if running as root (not recommended)
    if unsafe { libc::getuid() } == 0 {
        issues.push("Running as root (consider using rtkit instead)".to_string());
    }

    // Check if rtkit is available by trying to query it
    let rtkit_available = std::process::Command::new("dbus-send")
        .args([
            "--system",
            "--print-reply",
            "--dest=org.freedesktop.RealtimeKit1",
            "/org/freedesktop/RealtimeKit1",
            "org.freedesktop.DBus.Properties.Get",
            "string:org.freedesktop.RealtimeKit1",
            "string:MaxRealtimePriority",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !rtkit_available {
        issues.push(
            "rtkit service not available. Install rtkit package for unprivileged RT scheduling."
                .to_string(),
        );
    }

    RTConfigStatus {
        issues,
        rlimit_status,
        rtkit_available,
    }
}

/// Real-time configuration status
pub struct RTConfigStatus {
    /// Issues found during configuration check
    pub issues: Vec<String>,
    /// RLIMIT validation status
    pub rlimit_status: RlimitStatus,
    /// Whether rtkit service is available
    pub rtkit_available: bool,
}

impl RTConfigStatus {
    /// Check if the system is optimally configured for RT operation
    pub fn is_optimal(&self) -> bool {
        self.issues.is_empty() && self.rlimit_status.is_sufficient() && self.rtkit_available
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test LinuxRtThread creation
    ///
    /// Note: This test may behave differently depending on system configuration
    /// and whether rtkit is available.
    #[test]
    fn test_linux_rt_thread_creation() {
        // Create RT thread with priority 10
        let rt_thread = LinuxRtThread::new(10);

        // Should succeed even if RT scheduling fails (graceful degradation)
        assert!(rt_thread.is_ok(), "LinuxRtThread::new should not fail");

        let rt = rt_thread.unwrap();

        // Priority should be in valid range
        assert!(
            rt.priority() >= 0 && rt.priority() <= 99,
            "Priority should be in valid range"
        );

        // Scheduling policy should be valid
        assert!(
            rt.sched_policy() == libc::SCHED_FIFO || rt.sched_policy() == libc::SCHED_OTHER,
            "Scheduling policy should be SCHED_FIFO or SCHED_OTHER"
        );
    }

    /// Test metrics exposure
    #[test]
    fn test_metrics_exposure() {
        let rt = LinuxRtThread::new(10).unwrap();
        let metrics = rt.metrics();

        // Metrics should match struct fields
        assert_eq!(metrics.rt_enabled, rt.is_rt_enabled());
        assert_eq!(metrics.sched_policy, rt.sched_policy());
        assert_eq!(metrics.priority, rt.priority());
        assert_eq!(metrics.mlockall_success, rt.is_mlockall_success());
    }

    /// Test RLIMIT validation
    #[test]
    fn test_rlimit_validation() {
        let status = validate_rlimits(10);

        // Should have queried both limits
        // (values depend on system configuration)
        // Just verify the function doesn't panic
        let _ = status.rtprio_soft;
        let _ = status.memlock_soft;
    }

    /// Test priority clamping
    #[test]
    fn test_priority_clamping() {
        // Test with priority below minimum
        let rt_low = LinuxRtThread::new(0).unwrap();
        // Priority should be clamped to 1 (if RT enabled) or 0 (if not)
        assert!(rt_low.priority() >= 0);

        // Test with priority above maximum
        let rt_high = LinuxRtThread::new(100).unwrap();
        // Priority should be clamped to 99 (if RT enabled) or 0 (if not)
        assert!(rt_high.priority() <= 99);
    }

    /// Test sched_policy_name
    #[test]
    fn test_sched_policy_name() {
        let rt = LinuxRtThread::new(10).unwrap();
        let name = rt.sched_policy_name();

        // Should return a valid policy name
        assert!(
            name == "SCHED_FIFO" || name == "SCHED_OTHER",
            "Policy name should be SCHED_FIFO or SCHED_OTHER, got: {}",
            name
        );
    }

    /// Test LinuxRtMetrics default
    #[test]
    fn test_metrics_default() {
        let metrics = LinuxRtMetrics::default();

        assert!(!metrics.rt_enabled);
        assert_eq!(metrics.sched_policy, libc::SCHED_OTHER);
        assert_eq!(metrics.priority, 0);
        assert!(!metrics.mlockall_success);
    }

    /// Test check_rt_configuration
    #[test]
    fn test_check_rt_configuration() {
        let status = check_rt_configuration();

        // Should return a valid status (may have issues depending on system)
        let _ = status.is_optimal();
        let _ = status.rtkit_available;
    }

    // =========================================================================
    // LinuxTimerLoop Tests
    // =========================================================================

    /// Test LinuxTimerLoop creation
    #[test]
    fn test_linux_timer_loop_creation() {
        let timer = LinuxTimerLoop::new();

        // Should have default 250Hz period (4ms)
        assert_eq!(timer.period_ns(), LinuxTimerLoop::DEFAULT_PERIOD_NS);
        assert_eq!(timer.period_ns(), 4_000_000);

        // Should have default spin threshold (50μs)
        assert_eq!(
            timer.spin_threshold_ns(),
            LinuxTimerLoop::DEFAULT_SPIN_THRESHOLD_NS
        );
        assert_eq!(timer.spin_threshold_ns(), 50_000);

        // Frequency should be 250Hz
        let freq = timer.frequency_hz();
        assert!((freq - 250.0).abs() < 0.001, "Expected 250Hz, got {}", freq);
    }

    /// Test LinuxTimerLoop with custom period
    #[test]
    fn test_linux_timer_loop_custom_period() {
        // 1000Hz = 1ms period
        let timer = LinuxTimerLoop::with_period(1_000_000);

        assert_eq!(timer.period_ns(), 1_000_000);

        let freq = timer.frequency_hz();
        assert!(
            (freq - 1000.0).abs() < 0.001,
            "Expected 1000Hz, got {}",
            freq
        );
    }

    /// Test LinuxTimerLoop with custom period and threshold
    #[test]
    fn test_linux_timer_loop_custom_threshold() {
        // 500Hz with 100μs spin threshold
        let timer = LinuxTimerLoop::with_period_and_threshold(2_000_000, 100_000);

        assert_eq!(timer.period_ns(), 2_000_000);
        assert_eq!(timer.spin_threshold_ns(), 100_000);
    }

    /// Test LinuxTimerLoop panics on invalid period
    #[test]
    #[should_panic(expected = "Period")]
    fn test_linux_timer_loop_invalid_period() {
        // Period less than spin threshold should panic
        let _ = LinuxTimerLoop::with_period(10_000); // 10μs < 50μs threshold
    }

    /// Test LinuxTimerLoop panics on invalid threshold
    #[test]
    #[should_panic(expected = "Period")]
    fn test_linux_timer_loop_invalid_threshold() {
        // Period less than threshold should panic
        let _ = LinuxTimerLoop::with_period_and_threshold(100_000, 200_000);
    }

    /// Test LinuxTimerLoop default implementation
    #[test]
    fn test_linux_timer_loop_default() {
        let timer = LinuxTimerLoop::default();

        assert_eq!(timer.period_ns(), LinuxTimerLoop::DEFAULT_PERIOD_NS);
        assert_eq!(
            timer.spin_threshold_ns(),
            LinuxTimerLoop::DEFAULT_SPIN_THRESHOLD_NS
        );
    }

    /// Test LinuxTimerLoop reset
    #[test]
    fn test_linux_timer_loop_reset() {
        let mut timer = LinuxTimerLoop::new();

        // Get initial target
        let initial_target = timer.next_target();

        // Sleep a bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Reset should update target to current time
        timer.reset();
        let reset_target = timer.next_target();

        // Reset target should be later than initial
        let initial_nanos = LinuxTimerLoop::timespec_to_nanos(&initial_target);
        let reset_nanos = LinuxTimerLoop::timespec_to_nanos(&reset_target);

        assert!(
            reset_nanos > initial_nanos,
            "Reset target should be later than initial"
        );
    }

    /// Test LinuxTimerLoop current_time
    #[test]
    fn test_linux_timer_loop_current_time() {
        let time1 = LinuxTimerLoop::current_time();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let time2 = LinuxTimerLoop::current_time();

        let nanos1 = LinuxTimerLoop::timespec_to_nanos(&time1);
        let nanos2 = LinuxTimerLoop::timespec_to_nanos(&time2);

        // time2 should be at least 1ms later
        assert!(
            nanos2 - nanos1 >= 1_000_000,
            "Expected at least 1ms difference, got {} ns",
            nanos2 - nanos1
        );
    }

    /// Test LinuxTimerLoop timespec utilities
    #[test]
    fn test_linux_timer_loop_timespec_utils() {
        let ts1 = libc::timespec {
            tv_sec: 1,
            tv_nsec: 500_000_000,
        };
        let ts2 = libc::timespec {
            tv_sec: 2,
            tv_nsec: 0,
        };

        // Test timespec_to_nanos
        let nanos1 = LinuxTimerLoop::timespec_to_nanos(&ts1);
        assert_eq!(nanos1, 1_500_000_000);

        let nanos2 = LinuxTimerLoop::timespec_to_nanos(&ts2);
        assert_eq!(nanos2, 2_000_000_000);

        // Test timespec_diff_nanos
        let diff = LinuxTimerLoop::timespec_diff_nanos(&ts2, &ts1);
        assert_eq!(diff, 500_000_000); // 0.5 seconds
    }

    /// Test LinuxTimerLoop wait_next_tick basic functionality
    ///
    /// This test verifies that wait_next_tick advances the target time
    /// and returns a time close to the target.
    #[test]
    fn test_linux_timer_loop_wait_next_tick() {
        // Use a shorter period for faster testing (1ms)
        let mut timer = LinuxTimerLoop::with_period_and_threshold(1_000_000, 10_000);

        let initial_target = timer.next_target();
        let initial_nanos = LinuxTimerLoop::timespec_to_nanos(&initial_target);

        // Wait for one tick
        let actual = timer.wait_next_tick();
        let actual_nanos = LinuxTimerLoop::timespec_to_nanos(&actual);

        // Target should have advanced by one period
        let new_target = timer.next_target();
        let new_target_nanos = LinuxTimerLoop::timespec_to_nanos(&new_target);

        assert_eq!(
            new_target_nanos - initial_nanos,
            1_000_000,
            "Target should advance by one period"
        );

        // Actual time should be close to target (within 100μs tolerance)
        let deviation = (actual_nanos - new_target_nanos).abs();
        assert!(
            deviation < 100_000,
            "Actual time should be within 100μs of target, deviation: {} ns",
            deviation
        );
    }

    /// Test LinuxTimerLoop multiple ticks
    #[test]
    fn test_linux_timer_loop_multiple_ticks() {
        // Use a shorter period for faster testing (500μs)
        let mut timer = LinuxTimerLoop::with_period_and_threshold(500_000, 10_000);

        let start = LinuxTimerLoop::current_time();
        let start_nanos = LinuxTimerLoop::timespec_to_nanos(&start);

        // Run 10 ticks
        for _ in 0..10 {
            timer.wait_next_tick();
        }

        let end = LinuxTimerLoop::current_time();
        let end_nanos = LinuxTimerLoop::timespec_to_nanos(&end);

        // Total time should be approximately 10 * 500μs = 5ms
        let elapsed = end_nanos - start_nanos;
        let expected = 10 * 500_000;

        // Allow 20% tolerance for system scheduling variations
        let tolerance = expected / 5;
        assert!(
            (elapsed - expected).abs() < tolerance,
            "Expected ~{} ns elapsed, got {} ns (deviation: {} ns)",
            expected,
            elapsed,
            (elapsed - expected).abs()
        );
    }
}

// =============================================================================
// Property-Based Tests
// =============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: release-readiness, Property 3: RT Metrics Exposure
    //
    // *For any* LinuxRtThread instance, the metrics() method SHALL return valid values
    // for rt_enabled, sched_policy, priority, and mlockall_success that accurately
    // reflect the actual thread state.
    //
    // **Validates: Requirements 5.3, 7.1**

    /// Valid Linux scheduling policies that LinuxRtThread may use
    const VALID_SCHED_POLICIES: [i32; 5] = [
        libc::SCHED_OTHER,
        libc::SCHED_FIFO,
        libc::SCHED_RR,
        libc::SCHED_BATCH,
        libc::SCHED_IDLE,
    ];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property test: metrics() returns consistent values with struct fields
        ///
        /// For any requested priority in the valid range (1-99), the metrics()
        /// method SHALL return values that exactly match the internal state
        /// of the LinuxRtThread instance.
        ///
        /// **Validates: Requirements 5.3, 7.1**
        #[test]
        fn prop_metrics_consistency(requested_priority in 1i32..=99i32) {
            let rt = LinuxRtThread::new(requested_priority)
                .expect("LinuxRtThread::new should not fail (graceful degradation)");

            let metrics = rt.metrics();

            // Property: metrics() values MUST match accessor methods
            prop_assert_eq!(
                metrics.rt_enabled,
                rt.is_rt_enabled(),
                "rt_enabled mismatch: metrics={}, accessor={}",
                metrics.rt_enabled,
                rt.is_rt_enabled()
            );

            prop_assert_eq!(
                metrics.sched_policy,
                rt.sched_policy(),
                "sched_policy mismatch: metrics={}, accessor={}",
                metrics.sched_policy,
                rt.sched_policy()
            );

            prop_assert_eq!(
                metrics.priority,
                rt.priority(),
                "priority mismatch: metrics={}, accessor={}",
                metrics.priority,
                rt.priority()
            );

            prop_assert_eq!(
                metrics.mlockall_success,
                rt.is_mlockall_success(),
                "mlockall_success mismatch: metrics={}, accessor={}",
                metrics.mlockall_success,
                rt.is_mlockall_success()
            );
        }

        /// Property test: sched_policy is always a valid Linux scheduling policy
        ///
        /// For any LinuxRtThread instance, the sched_policy field SHALL be
        /// one of the valid Linux scheduling policies (SCHED_OTHER, SCHED_FIFO,
        /// SCHED_RR, SCHED_BATCH, or SCHED_IDLE).
        ///
        /// **Validates: Requirements 5.3, 7.1**
        #[test]
        fn prop_sched_policy_valid(requested_priority in 1i32..=99i32) {
            let rt = LinuxRtThread::new(requested_priority)
                .expect("LinuxRtThread::new should not fail");

            let metrics = rt.metrics();

            // Property: sched_policy MUST be a valid Linux scheduling policy
            prop_assert!(
                VALID_SCHED_POLICIES.contains(&metrics.sched_policy),
                "Invalid sched_policy: {}. Expected one of: {:?}",
                metrics.sched_policy,
                VALID_SCHED_POLICIES
            );

            // Additional check: sched_policy_name should return a known name
            let policy_name = metrics.sched_policy_name();
            prop_assert!(
                policy_name != "UNKNOWN",
                "sched_policy {} returned UNKNOWN name",
                metrics.sched_policy
            );
        }

        /// Property test: priority is in valid range (0-99)
        ///
        /// For any LinuxRtThread instance, the priority field SHALL be
        /// in the valid range: 0 for non-RT threads, or 1-99 for RT threads.
        ///
        /// **Validates: Requirements 5.3, 7.1**
        #[test]
        fn prop_priority_valid_range(requested_priority in 1i32..=99i32) {
            let rt = LinuxRtThread::new(requested_priority)
                .expect("LinuxRtThread::new should not fail");

            let metrics = rt.metrics();

            // Property: priority MUST be in valid range [0, 99]
            prop_assert!(
                metrics.priority >= 0 && metrics.priority <= 99,
                "Priority {} out of valid range [0, 99]",
                metrics.priority
            );

            // Property: if RT is enabled, priority should be in RT range [1, 99]
            if metrics.rt_enabled {
                prop_assert!(
                    metrics.priority >= 1 && metrics.priority <= 99,
                    "RT enabled but priority {} not in RT range [1, 99]",
                    metrics.priority
                );
            }

            // Property: if RT is not enabled, priority should be 0
            if !metrics.rt_enabled {
                prop_assert_eq!(
                    metrics.priority,
                    0,
                    "RT not enabled but priority is {} (expected 0)",
                    metrics.priority
                );
            }
        }

        /// Property test: mlockall_success consistency with rt_enabled
        ///
        /// For any LinuxRtThread instance, mlockall_success can only be true
        /// if rt_enabled is also true. mlockall is only attempted when RT
        /// scheduling is acquired.
        ///
        /// **Validates: Requirements 5.3, 5.4, 7.1**
        #[test]
        fn prop_mlockall_consistency(requested_priority in 1i32..=99i32) {
            let rt = LinuxRtThread::new(requested_priority)
                .expect("LinuxRtThread::new should not fail");

            let metrics = rt.metrics();

            // Property: mlockall_success implies rt_enabled
            // (mlockall is only attempted when RT is enabled)
            if metrics.mlockall_success {
                prop_assert!(
                    metrics.rt_enabled,
                    "mlockall_success is true but rt_enabled is false. \
                     mlockall should only succeed when RT scheduling is acquired."
                );
            }

            // Note: The converse is NOT required - rt_enabled can be true
            // while mlockall_success is false (e.g., insufficient RLIMIT_MEMLOCK)
        }

        /// Property test: sched_policy consistency with rt_enabled
        ///
        /// For any LinuxRtThread instance, if rt_enabled is true, the
        /// sched_policy should be SCHED_FIFO (or SCHED_RR). If rt_enabled
        /// is false, sched_policy should be SCHED_OTHER.
        ///
        /// **Validates: Requirements 5.2, 5.3, 7.1**
        #[test]
        fn prop_sched_policy_consistency(requested_priority in 1i32..=99i32) {
            let rt = LinuxRtThread::new(requested_priority)
                .expect("LinuxRtThread::new should not fail");

            let metrics = rt.metrics();

            if metrics.rt_enabled {
                // Property: RT enabled implies RT scheduling policy
                prop_assert!(
                    metrics.sched_policy == libc::SCHED_FIFO
                        || metrics.sched_policy == libc::SCHED_RR,
                    "RT enabled but sched_policy is {} (expected SCHED_FIFO={} or SCHED_RR={})",
                    metrics.sched_policy,
                    libc::SCHED_FIFO,
                    libc::SCHED_RR
                );
            } else {
                // Property: RT not enabled implies non-RT scheduling policy
                prop_assert_eq!(
                    metrics.sched_policy,
                    libc::SCHED_OTHER,
                    "RT not enabled but sched_policy is {} (expected SCHED_OTHER={})",
                    metrics.sched_policy,
                    libc::SCHED_OTHER
                );
            }
        }
    }

    /// Property test: priority clamping behavior
    ///
    /// For any requested priority (including out-of-range values), the
    /// LinuxRtThread should clamp the priority to the valid range [1, 99]
    /// before attempting RT scheduling.
    ///
    /// **Validates: Requirements 5.3, 7.1**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_priority_clamping(requested_priority in -100i32..=200i32) {
            let rt = LinuxRtThread::new(requested_priority)
                .expect("LinuxRtThread::new should not fail");

            let metrics = rt.metrics();

            // Property: priority is always in valid range regardless of input
            prop_assert!(
                metrics.priority >= 0 && metrics.priority <= 99,
                "Priority {} out of valid range [0, 99] for requested priority {}",
                metrics.priority,
                requested_priority
            );

            // Property: if RT enabled, priority should be clamped to [1, 99]
            if metrics.rt_enabled {
                let expected_clamped = requested_priority.clamp(1, 99);
                prop_assert_eq!(
                    metrics.priority,
                    expected_clamped,
                    "RT enabled: expected clamped priority {} but got {} for requested {}",
                    expected_clamped,
                    metrics.priority,
                    requested_priority
                );
            }
        }
    }

    /// Property test: LinuxRtMetrics default values
    ///
    /// The default LinuxRtMetrics should represent a non-RT thread state.
    #[test]
    fn prop_metrics_default_is_non_rt() {
        let default_metrics = LinuxRtMetrics::default();

        // Default should represent non-RT state
        assert!(
            !default_metrics.rt_enabled,
            "Default rt_enabled should be false"
        );
        assert_eq!(
            default_metrics.sched_policy,
            libc::SCHED_OTHER,
            "Default sched_policy should be SCHED_OTHER"
        );
        assert_eq!(default_metrics.priority, 0, "Default priority should be 0");
        assert!(
            !default_metrics.mlockall_success,
            "Default mlockall_success should be false"
        );
    }

    /// Property test: LinuxRtMetrics sched_policy_name consistency
    ///
    /// The sched_policy_name() method should return consistent names for
    /// all valid scheduling policies.
    #[test]
    fn prop_sched_policy_name_consistency() {
        // Test all valid policies
        let test_cases = [
            (libc::SCHED_OTHER, "SCHED_OTHER"),
            (libc::SCHED_FIFO, "SCHED_FIFO"),
            (libc::SCHED_RR, "SCHED_RR"),
            (libc::SCHED_BATCH, "SCHED_BATCH"),
            (libc::SCHED_IDLE, "SCHED_IDLE"),
        ];

        for (policy, expected_name) in test_cases {
            let metrics = LinuxRtMetrics {
                rt_enabled: false,
                sched_policy: policy,
                priority: 0,
                mlockall_success: false,
            };

            assert_eq!(
                metrics.sched_policy_name(),
                expected_name,
                "sched_policy {} should return name '{}'",
                policy,
                expected_name
            );
        }
    }

    /// Property test: LinuxRtMetrics equality
    ///
    /// Two LinuxRtMetrics with the same values should be equal.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        fn prop_metrics_equality(
            rt_enabled in proptest::bool::ANY,
            sched_policy in prop::sample::select(VALID_SCHED_POLICIES.to_vec()),
            priority in 0i32..=99i32,
            mlockall_success in proptest::bool::ANY
        ) {
            let metrics1 = LinuxRtMetrics {
                rt_enabled,
                sched_policy,
                priority,
                mlockall_success,
            };

            let metrics2 = LinuxRtMetrics {
                rt_enabled,
                sched_policy,
                priority,
                mlockall_success,
            };

            // Property: identical metrics should be equal
            prop_assert_eq!(metrics1, metrics2);

            // Property: Clone should produce equal value
            let metrics_clone = metrics1;
            prop_assert_eq!(metrics1, metrics_clone);
        }
    }
}

// =============================================================================
// Jitter Measurement (for Task 6.2)
// =============================================================================
// Note: A full cross-platform JitterMeasurement struct will be implemented in Task 8.1.
// This is a simplified inline version for the Linux timer jitter test.

/// Simplified jitter measurement helper for RT validation
///
/// Records deviations from ideal period and computes p50/p95/p99 statistics.
/// This is a simplified version for Task 6.2; the full implementation will
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
// Linux Timer Jitter Test (Task 6.2)
// =============================================================================

#[cfg(test)]
mod jitter_tests {
    use super::*;

    // Feature: release-readiness, Property 1: Timer Loop Jitter (Linux)
    //
    // *For any* 250Hz timer loop running for ≥10 minutes (excluding 5s warmup),
    // the p99 jitter SHALL be ≤0.5ms on Linux platforms.
    //
    // **Validates: Requirements 6.3**

    /// Convert a libc::timespec to nanoseconds
    #[inline]
    fn timespec_to_ns(ts: &libc::timespec) -> u64 {
        (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
    }

    /// Test timer jitter for Linux high-resolution timer loop
    ///
    /// This test validates that the LinuxTimerLoop achieves p99 jitter ≤0.5ms
    /// over a 10-minute test period with a 5-second warmup excluded.
    ///
    /// # Requirements
    ///
    /// - Requirement 6.3: p99 jitter SHALL be ≤0.5ms measured over ≥10 minutes with warm-up excluded
    ///
    /// # Test Configuration
    ///
    /// - Target frequency: 250Hz (4ms period)
    /// - Test duration: 10 minutes (600 seconds)
    /// - Warmup period: 5 seconds (excluded from statistics)
    /// - Expected samples: ~148,750 (250Hz × 595 seconds)
    /// - Pass criteria: p99 jitter ≤ 0.5ms (500,000 ns)
    ///
    /// # Notes
    ///
    /// This test is marked with `#[ignore]` because it takes 10+ minutes to run.
    /// Run manually with: `cargo test -p flight-scheduler test_timer_jitter_linux -- --ignored --nocapture`
    /// Or in CI on hardware runners.
    #[test]
    #[ignore] // Requires 10+ minutes, run manually or in CI
    fn test_timer_jitter_linux() {
        const TARGET_HZ: u32 = 250;
        const TEST_DURATION_SECS: u64 = 600; // 10 minutes
        const WARMUP_SECS: u32 = 5;
        const MAX_P99_MS: f64 = 0.5;

        println!("=== Linux Timer Jitter Test ===");
        println!("Target frequency: {} Hz", TARGET_HZ);
        println!(
            "Test duration: {} seconds ({} minutes)",
            TEST_DURATION_SECS,
            TEST_DURATION_SECS / 60
        );
        println!("Warmup period: {} seconds", WARMUP_SECS);
        println!("Pass criteria: p99 jitter ≤ {} ms", MAX_P99_MS);
        println!();

        // Create the timer loop
        let mut timer = LinuxTimerLoop::new();

        // Create jitter measurement with 5-second warmup
        let mut jitter = JitterMeasurement::new(TARGET_HZ, WARMUP_SECS);

        // Track test start time
        let start = std::time::Instant::now();

        // Progress reporting interval (every 60 seconds)
        let mut last_progress_report = std::time::Instant::now();
        let progress_interval = std::time::Duration::from_secs(60);

        println!("Starting timer loop...");

        // Run the timer loop for the test duration
        while start.elapsed() < std::time::Duration::from_secs(TEST_DURATION_SECS) {
            // Wait for next tick and get actual time
            let actual_time = timer.wait_next_tick();

            // Convert timespec to nanoseconds and record
            let now_ns = timespec_to_ns(&actual_time);
            jitter.record_tick(now_ns);

            // Progress reporting
            if last_progress_report.elapsed() >= progress_interval {
                let elapsed_secs = start.elapsed().as_secs();
                let samples = jitter.sample_count();
                let ticks = jitter.tick_count();
                println!(
                    "Progress: {}/{} seconds, {} ticks, {} samples recorded",
                    elapsed_secs, TEST_DURATION_SECS, ticks, samples
                );
                last_progress_report = std::time::Instant::now();
            }
        }

        // Compute final statistics
        let stats = jitter.compute_stats();

        println!();
        println!("=== Test Results ===");
        println!("Total ticks: {}", jitter.tick_count());
        println!("Samples (excluding warmup): {}", stats.samples);
        println!(
            "p50 jitter: {} ns ({:.3} ms)",
            stats.p50_ns,
            stats.p50_ns as f64 / 1_000_000.0
        );
        println!(
            "p95 jitter: {} ns ({:.3} ms)",
            stats.p95_ns,
            stats.p95_ns as f64 / 1_000_000.0
        );
        println!("p99 jitter: {} ns ({:.3} ms)", stats.p99_ns, stats.p99_ms);
        println!();

        // Validate p99 jitter is within threshold
        assert!(
            stats.p99_ms <= MAX_P99_MS,
            "FAIL: p99 jitter {:.3} ms exceeds threshold of {} ms\n\
             This indicates the timer loop is not meeting real-time requirements.\n\
             Possible causes:\n\
             - System not configured for RT (check rtkit, RLIMIT_RTPRIO)\n\
             - High system load during test\n\
             - Running in a VM without RT support\n\
             - Kernel not configured for low-latency operation",
            stats.p99_ms,
            MAX_P99_MS
        );

        println!(
            "PASS: p99 jitter {:.3} ms ≤ {} ms threshold",
            stats.p99_ms, MAX_P99_MS
        );
    }

    /// Short jitter test for basic validation (not the full 10-minute test)
    ///
    /// This test runs for only 10 seconds to verify basic timer functionality
    /// without the full 10-minute duration. It's useful for quick validation
    /// during development.
    #[test]
    fn test_timer_jitter_linux_short() {
        const TARGET_HZ: u32 = 250;
        const TEST_DURATION_SECS: u64 = 10; // 10 seconds
        const WARMUP_SECS: u32 = 1;

        // Create the timer loop
        let mut timer = LinuxTimerLoop::new();

        // Create jitter measurement with 1-second warmup
        let mut jitter = JitterMeasurement::new(TARGET_HZ, WARMUP_SECS);

        // Track test start time
        let start = std::time::Instant::now();

        // Run the timer loop for the test duration
        while start.elapsed() < std::time::Duration::from_secs(TEST_DURATION_SECS) {
            let actual_time = timer.wait_next_tick();
            let now_ns = timespec_to_ns(&actual_time);
            jitter.record_tick(now_ns);
        }

        // Compute statistics
        let stats = jitter.compute_stats();

        // Basic sanity checks
        assert!(stats.samples > 0, "Should have recorded samples");
        assert!(
            stats.samples >= (TARGET_HZ * (TEST_DURATION_SECS as u32 - WARMUP_SECS) - 100) as usize,
            "Should have approximately {} samples, got {}",
            TARGET_HZ * (TEST_DURATION_SECS as u32 - WARMUP_SECS),
            stats.samples
        );

        // Log results (don't fail on jitter for short test - system may not be RT configured)
        println!("Short jitter test results:");
        println!("  Samples: {}", stats.samples);
        println!("  p50: {:.3} ms", stats.p50_ns as f64 / 1_000_000.0);
        println!("  p95: {:.3} ms", stats.p95_ns as f64 / 1_000_000.0);
        println!("  p99: {:.3} ms", stats.p99_ms);
    }

    /// Test JitterMeasurement statistics calculation
    #[test]
    fn test_jitter_measurement_stats() {
        let mut jitter = JitterMeasurement::new(250, 0); // No warmup for this test

        // Simulate ticks with known period deviations
        // Target period at 250Hz = 4,000,000 ns
        let base_ns: u64 = 1_000_000_000;
        let period_ns: u64 = 4_000_000;

        // First tick establishes baseline
        jitter.record_tick(base_ns);

        // Subsequent ticks with controlled deviations
        // Deviations: 0, +100μs, -50μs, +200μs, +500μs (in ns: 0, 100000, -50000, 200000, 500000)
        let deviations = [0i64, 100_000, -50_000, 200_000, 500_000];

        for (i, dev) in deviations.iter().enumerate() {
            let tick_ns = base_ns + ((i as u64 + 1) * period_ns) + (*dev as u64);
            jitter.record_tick(tick_ns);
        }

        let stats = jitter.compute_stats();

        // Should have 5 samples (first tick is baseline)
        assert_eq!(stats.samples, 5, "Expected 5 samples");

        // p99 should be close to max deviation (500μs = 500,000 ns)
        // With 5 samples, p99 index = (5 * 99) / 100 = 4, which is the max
        assert!(
            stats.p99_ns >= 400_000 && stats.p99_ns <= 600_000,
            "p99 should be around 500μs, got {} ns",
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

        // Record 300 ticks (250 warmup + 50 actual)
        for i in 0..300 {
            let tick_ns = base_ns + (i as u64 * period_ns);
            jitter.record_tick(tick_ns);
        }

        let stats = jitter.compute_stats();

        // Should have ~49 samples (300 - 250 warmup - 1 baseline)
        // The first tick after warmup establishes the new baseline
        assert!(
            stats.samples >= 48 && stats.samples <= 50,
            "Expected ~49 samples after warmup, got {}",
            stats.samples
        );
    }
}
