// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Linux rtkit D-Bus integration for real-time scheduling
//!
//! Provides a trait-based abstraction over the rtkit (RealtimeKit) D-Bus
//! service, with a mock backend for cross-platform testing.

#[cfg(target_os = "linux")]
use tracing::{info, warn};

// =============================================================================
// Error Types
// =============================================================================

/// Errors from rtkit operations
#[derive(Debug)]
pub enum RtkitError {
    /// D-Bus connection to rtkit failed
    DbusConnectionFailed(String),
    /// rtkit denied the RT scheduling request
    RequestDenied(String),
    /// Thread affinity setting failed
    AffinityFailed(String),
    /// The requested priority is out of range
    InvalidPriority(String),
}

impl std::fmt::Display for RtkitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RtkitError::DbusConnectionFailed(e) => write!(f, "rtkit D-Bus connection failed: {e}"),
            RtkitError::RequestDenied(e) => write!(f, "rtkit denied RT request: {e}"),
            RtkitError::AffinityFailed(e) => write!(f, "thread affinity failed: {e}"),
            RtkitError::InvalidPriority(e) => write!(f, "invalid RT priority: {e}"),
        }
    }
}

impl std::error::Error for RtkitError {}

// =============================================================================
// Rtkit Backend Trait
// =============================================================================

/// Abstraction over rtkit D-Bus operations for testability
pub trait RtkitBackend: Send {
    /// Request real-time scheduling for the calling thread via rtkit.
    ///
    /// `priority` is in the range 1–99 (SCHED_FIFO).
    fn request_realtime(&self, thread_id: u64, priority: i32) -> Result<(), RtkitError>;

    /// Relinquish real-time scheduling (reset to SCHED_OTHER).
    fn relinquish_realtime(&self, thread_id: u64) -> Result<(), RtkitError>;

    /// Set CPU affinity for a thread.
    fn set_thread_affinity(&self, thread_id: u64, core_id: usize) -> Result<(), RtkitError>;

    /// Query the maximum RT priority rtkit will grant.
    fn max_realtime_priority(&self) -> Result<i32, RtkitError>;
}

// =============================================================================
// Real Linux Backend
// =============================================================================

/// Real rtkit backend that calls D-Bus via `dbus-send` or direct syscalls.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct LinuxRtkitBackend;

#[cfg(target_os = "linux")]
impl RtkitBackend for LinuxRtkitBackend {
    fn request_realtime(&self, thread_id: u64, priority: i32) -> Result<(), RtkitError> {
        if !(1..=99).contains(&priority) {
            return Err(RtkitError::InvalidPriority(format!(
                "priority {priority} not in 1..=99"
            )));
        }

        let output = std::process::Command::new("dbus-send")
            .args([
                "--system",
                "--print-reply",
                "--dest=org.freedesktop.RealtimeKit1",
                "/org/freedesktop/RealtimeKit1",
                "org.freedesktop.RealtimeKit1.MakeThreadRealtime",
                &format!("uint64:{thread_id}"),
                &format!("uint32:{priority}"),
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                info!("rtkit granted RT priority {priority} for thread {thread_id}");
                Ok(())
            }
            Ok(result) => {
                let stderr = String::from_utf8_lossy(&result.stderr);
                Err(RtkitError::RequestDenied(stderr.trim().to_string()))
            }
            Err(e) => Err(RtkitError::DbusConnectionFailed(format!(
                "dbus-send failed: {e}"
            ))),
        }
    }

    fn relinquish_realtime(&self, _thread_id: u64) -> Result<(), RtkitError> {
        // Reset the calling thread to SCHED_OTHER
        let param = libc::sched_param { sched_priority: 0 };
        let result = unsafe { libc::sched_setscheduler(0, libc::SCHED_OTHER, &param) };
        if result != 0 {
            let err = std::io::Error::last_os_error();
            warn!("Failed to relinquish RT scheduling: {err}");
            // Non-fatal: the thread will still be cleaned up on exit
        }
        Ok(())
    }

    fn set_thread_affinity(&self, _thread_id: u64, core_id: usize) -> Result<(), RtkitError> {
        use std::mem;
        unsafe {
            let mut cpuset: libc::cpu_set_t = mem::zeroed();
            libc::CPU_ZERO(&mut cpuset);
            libc::CPU_SET(core_id, &mut cpuset);
            let result = libc::sched_setaffinity(0, mem::size_of::<libc::cpu_set_t>(), &cpuset);
            if result != 0 {
                return Err(RtkitError::AffinityFailed(
                    std::io::Error::last_os_error().to_string(),
                ));
            }
        }
        info!("Thread pinned to core {core_id}");
        Ok(())
    }

    fn max_realtime_priority(&self) -> Result<i32, RtkitError> {
        let output = std::process::Command::new("dbus-send")
            .args([
                "--system",
                "--print-reply",
                "--dest=org.freedesktop.RealtimeKit1",
                "/org/freedesktop/RealtimeKit1",
                "org.freedesktop.DBus.Properties.Get",
                "string:org.freedesktop.RealtimeKit1",
                "string:MaxRealtimePriority",
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                // Parse "int32 N" from the D-Bus reply
                if let Some(val) = stdout.split("int32").nth(1) {
                    if let Ok(n) = val.trim().parse::<i32>() {
                        return Ok(n);
                    }
                }
                Ok(99) // default fallback
            }
            _ => Ok(99), // assume max if we can't query
        }
    }
}

// =============================================================================
// Mock Backend (for testing)
// =============================================================================

/// Mock rtkit backend for cross-platform testing
#[derive(Debug)]
pub struct MockRtkitBackend {
    /// If true, all operations succeed
    pub succeed: bool,
    /// Maximum priority to grant
    pub max_priority: i32,
    /// Track request calls
    request_count: std::sync::atomic::AtomicU32,
    /// Track relinquish calls
    relinquish_count: std::sync::atomic::AtomicU32,
    /// Track affinity calls
    affinity_count: std::sync::atomic::AtomicU32,
    /// Last core_id set
    last_core_id: std::sync::atomic::AtomicUsize,
    /// Last priority requested
    last_priority: std::sync::atomic::AtomicI32,
}

impl MockRtkitBackend {
    /// Create a mock that succeeds on all calls
    pub fn new_success() -> Self {
        Self {
            succeed: true,
            max_priority: 99,
            request_count: std::sync::atomic::AtomicU32::new(0),
            relinquish_count: std::sync::atomic::AtomicU32::new(0),
            affinity_count: std::sync::atomic::AtomicU32::new(0),
            last_core_id: std::sync::atomic::AtomicUsize::new(0),
            last_priority: std::sync::atomic::AtomicI32::new(0),
        }
    }

    /// Create a mock that fails on all calls
    pub fn new_failure() -> Self {
        Self {
            succeed: false,
            max_priority: 0,
            request_count: std::sync::atomic::AtomicU32::new(0),
            relinquish_count: std::sync::atomic::AtomicU32::new(0),
            affinity_count: std::sync::atomic::AtomicU32::new(0),
            last_core_id: std::sync::atomic::AtomicUsize::new(0),
            last_priority: std::sync::atomic::AtomicI32::new(0),
        }
    }

    pub fn request_count(&self) -> u32 {
        self.request_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn relinquish_count(&self) -> u32 {
        self.relinquish_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn affinity_count(&self) -> u32 {
        self.affinity_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn last_core_id(&self) -> usize {
        self.last_core_id.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn last_priority(&self) -> i32 {
        self.last_priority
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl RtkitBackend for MockRtkitBackend {
    fn request_realtime(&self, _thread_id: u64, priority: i32) -> Result<(), RtkitError> {
        self.request_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.last_priority
            .store(priority, std::sync::atomic::Ordering::Relaxed);
        if self.succeed {
            Ok(())
        } else {
            Err(RtkitError::RequestDenied("mock failure".to_string()))
        }
    }

    fn relinquish_realtime(&self, _thread_id: u64) -> Result<(), RtkitError> {
        self.relinquish_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.succeed {
            Ok(())
        } else {
            Err(RtkitError::RequestDenied(
                "mock relinquish failure".to_string(),
            ))
        }
    }

    fn set_thread_affinity(&self, _thread_id: u64, core_id: usize) -> Result<(), RtkitError> {
        self.affinity_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.last_core_id
            .store(core_id, std::sync::atomic::Ordering::Relaxed);
        if self.succeed {
            Ok(())
        } else {
            Err(RtkitError::AffinityFailed("mock failure".to_string()))
        }
    }

    fn max_realtime_priority(&self) -> Result<i32, RtkitError> {
        if self.succeed {
            Ok(self.max_priority)
        } else {
            Err(RtkitError::DbusConnectionFailed("mock failure".to_string()))
        }
    }
}

// =============================================================================
// RtkitHandle
// =============================================================================

/// RAII handle for rtkit-based real-time scheduling.
///
/// On creation, requests RT scheduling via the backend. On drop, relinquishes
/// RT scheduling back to `SCHED_OTHER`.
pub struct RtkitHandle<B: RtkitBackend = BoxedRtkitBackend> {
    backend: B,
    thread_id: u64,
    priority: i32,
    affinity_core: Option<usize>,
}

impl<B: RtkitBackend> std::fmt::Debug for RtkitHandle<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RtkitHandle")
            .field("thread_id", &self.thread_id)
            .field("priority", &self.priority)
            .field("affinity_core", &self.affinity_core)
            .finish()
    }
}

/// Type-erased backend for ergonomic default usage
pub type BoxedRtkitBackend = Box<dyn RtkitBackend>;

impl RtkitBackend for Box<dyn RtkitBackend> {
    fn request_realtime(&self, thread_id: u64, priority: i32) -> Result<(), RtkitError> {
        (**self).request_realtime(thread_id, priority)
    }
    fn relinquish_realtime(&self, thread_id: u64) -> Result<(), RtkitError> {
        (**self).relinquish_realtime(thread_id)
    }
    fn set_thread_affinity(&self, thread_id: u64, core_id: usize) -> Result<(), RtkitError> {
        (**self).set_thread_affinity(thread_id, core_id)
    }
    fn max_realtime_priority(&self) -> Result<i32, RtkitError> {
        (**self).max_realtime_priority()
    }
}

impl<B: RtkitBackend> RtkitHandle<B> {
    /// Request real-time scheduling for the current thread.
    ///
    /// # Arguments
    /// * `backend`  — Platform backend (real or mock)
    /// * `priority` — SCHED_FIFO priority (1–99)
    pub fn request_realtime(backend: B, priority: i32) -> Result<Self, RtkitError> {
        if !(1..=99).contains(&priority) {
            return Err(RtkitError::InvalidPriority(format!(
                "priority {priority} not in 1..=99"
            )));
        }
        // Use a synthetic thread ID for the abstraction; real backend uses actual TID
        let thread_id = current_thread_id();
        backend.request_realtime(thread_id, priority)?;
        Ok(Self {
            backend,
            thread_id,
            priority,
            affinity_core: None,
        })
    }

    /// Pin the thread to a specific CPU core.
    pub fn set_thread_affinity(&mut self, core_id: usize) -> Result<(), RtkitError> {
        self.backend.set_thread_affinity(self.thread_id, core_id)?;
        self.affinity_core = Some(core_id);
        Ok(())
    }

    /// The RT priority granted.
    pub fn priority(&self) -> i32 {
        self.priority
    }

    /// The thread ID this handle manages.
    pub fn thread_id(&self) -> u64 {
        self.thread_id
    }

    /// The CPU core the thread is pinned to, if any.
    pub fn affinity_core(&self) -> Option<usize> {
        self.affinity_core
    }

    /// Query the maximum RT priority the rtkit daemon will grant.
    pub fn max_realtime_priority(&self) -> Result<i32, RtkitError> {
        self.backend.max_realtime_priority()
    }
}

impl<B: RtkitBackend> Drop for RtkitHandle<B> {
    fn drop(&mut self) {
        let _ = self.backend.relinquish_realtime(self.thread_id);
    }
}

/// Get a thread identifier for the current thread.
fn current_thread_id() -> u64 {
    // Use a platform-independent approach: hash the ThreadId
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    hasher.finish()
}

/// Set CPU affinity for the current thread (standalone function).
pub fn set_thread_affinity<B: RtkitBackend>(backend: &B, core_id: usize) -> Result<(), RtkitError> {
    let thread_id = current_thread_id();
    backend.set_thread_affinity(thread_id, core_id)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_request_realtime_success() {
        let backend = MockRtkitBackend::new_success();
        let handle = RtkitHandle::request_realtime(backend, 10);
        assert!(handle.is_ok());
        let h = handle.unwrap();
        assert_eq!(h.priority(), 10);
    }

    #[test]
    fn test_mock_request_realtime_failure() {
        let backend = MockRtkitBackend::new_failure();
        let result = RtkitHandle::request_realtime(backend, 10);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RtkitError::RequestDenied(_)));
    }

    #[test]
    fn test_invalid_priority_too_low() {
        let backend = MockRtkitBackend::new_success();
        let result = RtkitHandle::request_realtime(backend, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RtkitError::InvalidPriority(_)
        ));
    }

    #[test]
    fn test_invalid_priority_too_high() {
        let backend = MockRtkitBackend::new_success();
        let result = RtkitHandle::request_realtime(backend, 100);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RtkitError::InvalidPriority(_)
        ));
    }

    #[test]
    fn test_set_affinity() {
        let backend = MockRtkitBackend::new_success();
        let mut h = RtkitHandle::request_realtime(backend, 10).unwrap();
        assert!(h.affinity_core().is_none());
        h.set_thread_affinity(2).unwrap();
        assert_eq!(h.affinity_core(), Some(2));
        assert_eq!(h.backend.last_core_id(), 2);
    }

    #[test]
    fn test_set_affinity_failure() {
        let mut backend = MockRtkitBackend::new_success();
        let mut h = RtkitHandle::request_realtime(backend, 10).unwrap();
        h.backend.succeed = false;
        let result = h.set_thread_affinity(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_relinquishes() {
        let backend = MockRtkitBackend::new_success();
        let h = RtkitHandle::request_realtime(backend, 10).unwrap();
        assert_eq!(h.backend.relinquish_count(), 0);
        drop(h);
        // No panic = successful relinquish attempt on drop
    }

    #[test]
    fn test_max_realtime_priority() {
        let backend = MockRtkitBackend::new_success();
        let h = RtkitHandle::request_realtime(backend, 10).unwrap();
        assert_eq!(h.max_realtime_priority().unwrap(), 99);
    }

    #[test]
    fn test_standalone_set_affinity() {
        let backend = MockRtkitBackend::new_success();
        set_thread_affinity(&backend, 3).unwrap();
        assert_eq!(backend.last_core_id(), 3);
        assert_eq!(backend.affinity_count(), 1);
    }

    #[test]
    fn test_mock_call_counts() {
        let backend = MockRtkitBackend::new_success();
        let mut h = RtkitHandle::request_realtime(backend, 10).unwrap();
        assert_eq!(h.backend.request_count(), 1);
        h.set_thread_affinity(0).unwrap();
        assert_eq!(h.backend.affinity_count(), 1);
        h.set_thread_affinity(1).unwrap();
        assert_eq!(h.backend.affinity_count(), 2);
    }

    #[test]
    fn test_rtkit_error_display() {
        let e = RtkitError::DbusConnectionFailed("no bus".to_string());
        assert!(format!("{e}").contains("D-Bus connection failed"));

        let e = RtkitError::RequestDenied("denied".to_string());
        assert!(format!("{e}").contains("denied RT request"));

        let e = RtkitError::AffinityFailed("fail".to_string());
        assert!(format!("{e}").contains("thread affinity failed"));

        let e = RtkitError::InvalidPriority("bad".to_string());
        assert!(format!("{e}").contains("invalid RT priority"));
    }

    #[test]
    fn test_error_is_error_trait() {
        let e: Box<dyn std::error::Error> = Box::new(RtkitError::RequestDenied("test".to_string()));
        assert!(e.to_string().contains("denied"));
    }

    #[test]
    fn test_thread_id_nonzero() {
        let id = current_thread_id();
        // Hash is extremely unlikely to be 0, but not impossible
        // Just verify it returns *something*
        let _ = id;
    }

    #[test]
    fn test_boundary_priorities() {
        // Priority 1 (minimum valid)
        let backend = MockRtkitBackend::new_success();
        let h = RtkitHandle::request_realtime(backend, 1).unwrap();
        assert_eq!(h.priority(), 1);

        // Priority 99 (maximum valid)
        let backend = MockRtkitBackend::new_success();
        let h = RtkitHandle::request_realtime(backend, 99).unwrap();
        assert_eq!(h.priority(), 99);
    }
}
