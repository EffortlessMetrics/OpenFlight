//! Unix-specific scheduler implementation

use std::time::Duration;

/// Platform-specific sleep implementation for Unix
pub fn platform_sleep(duration: Duration) {
    // TODO: Implement clock_nanosleep with CLOCK_MONOTONIC
    std::thread::sleep(duration);
}
