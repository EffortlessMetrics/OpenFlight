//! Unix-specific scheduler implementation
//!
//! Uses clock_nanosleep with CLOCK_MONOTONIC for high-precision timing
//! and SCHED_FIFO via rtkit for real-time performance.

use std::time::Duration;
use nix::sys::mman::{mlockall, MlockAllFlags};
use nix::unistd::{getpid, getuid};
use nix::errno::Errno;
use libc::{self, timespec, CLOCK_MONOTONIC, TIMER_ABSTIME};

/// Platform-specific sleep implementation for Unix
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

/// Set current thread to real-time priority using rtkit
pub fn set_realtime_priority() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // First try to lock memory to prevent page faults
    if let Err(e) = mlockall(MlockAllFlags::MCL_CURRENT | MlockAllFlags::MCL_FUTURE) {
        eprintln!("Warning: Failed to lock memory: {}", e);
    }
    
    // Try to set SCHED_FIFO via rtkit D-Bus interface
    // This is a simplified version - full implementation would use D-Bus
    match try_rtkit_sched_fifo() {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("Warning: Failed to set RT priority via rtkit: {}", e);
            // Try direct syscall (requires CAP_SYS_NICE or running as root)
            try_direct_sched_fifo()
        }
    }
}

fn try_rtkit_sched_fifo() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // In a full implementation, this would use D-Bus to communicate with rtkit
    // For now, we'll return an error to fall back to direct syscall
    Err("rtkit D-Bus interface not implemented".into())
}

fn try_direct_sched_fifo() -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let param = libc::sched_param {
            sched_priority: 50, // Mid-range RT priority
        };
        
        let result = libc::sched_setscheduler(
            0, // Current thread
            libc::SCHED_FIFO,
            &param,
        );
        
        if result != 0 {
            let errno = Errno::last();
            return Err(format!("Failed to set SCHED_FIFO: {}", errno).into());
        }
        
        Ok(())
    }
}

/// Check if system is configured for real-time performance
pub fn check_rt_configuration() -> RTConfigStatus {
    let mut issues = Vec::new();
    
    // Check if running as root or with CAP_SYS_NICE
    if getuid().is_root() {
        // Running as root - should work but not recommended
        issues.push("Running as root (consider using rtkit instead)".to_string());
    }
    
    // Check rtkit availability (would require D-Bus in full implementation)
    
    // Check memory lock limits
    unsafe {
        let mut rlimit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        
        if libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut rlimit) == 0 {
            if rlimit.rlim_cur == 0 {
                issues.push("Memory lock limit is 0 (may cause RT issues)".to_string());
            }
        }
    }
    
    RTConfigStatus { issues }
}

/// Real-time configuration status
pub struct RTConfigStatus {
    pub issues: Vec<String>,
}

impl RTConfigStatus {
    pub fn is_optimal(&self) -> bool {
        self.issues.is_empty()
    }
}
