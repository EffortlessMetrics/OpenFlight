//! Windows-specific scheduler implementation

use std::time::Duration;

/// Platform-specific sleep implementation for Windows
pub fn platform_sleep(duration: Duration) {
    // TODO: Implement Windows waitable timer with SetWaitableTimerEx
    std::thread::sleep(duration);
}
