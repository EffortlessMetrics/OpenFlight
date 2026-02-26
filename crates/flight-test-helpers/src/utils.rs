// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared test utility functions.

use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Initialize tracing subscriber for tests. Safe to call multiple times.
pub fn setup_test_logger() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    });
}

/// Create a temp directory with a deterministic prefix.
pub fn create_temp_dir(prefix: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .expect("failed to create temporary directory")
}

/// Poll `condition` until it returns true or timeout expires.
pub fn wait_for_condition<F>(timeout: Duration, poll_interval: Duration, mut condition: F) -> bool
where
    F: FnMut() -> bool,
{
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return true;
        }
        thread::sleep(poll_interval);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{create_temp_dir, wait_for_condition};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_create_temp_dir() {
        let dir = create_temp_dir("flight-test-helpers");
        assert!(dir.path().exists());
    }

    #[test]
    fn test_wait_for_condition() {
        let ready = Arc::new(AtomicBool::new(false));
        let ready_clone = Arc::clone(&ready);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            ready_clone.store(true, Ordering::Relaxed);
        });

        let success =
            wait_for_condition(Duration::from_millis(250), Duration::from_millis(5), || {
                ready.load(Ordering::Relaxed)
            });
        assert!(success);
    }

    #[test]
    fn test_wait_for_condition_timeout_when_never_ready() {
        let result =
            wait_for_condition(Duration::from_millis(50), Duration::from_millis(5), || {
                false
            });
        assert!(!result);
    }

    #[test]
    fn test_create_temp_dir_unique() {
        let d1 = create_temp_dir("flight-test");
        let d2 = create_temp_dir("flight-test");
        assert_ne!(d1.path(), d2.path());
    }
}
