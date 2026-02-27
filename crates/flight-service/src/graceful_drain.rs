// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Graceful drain coordinator for service shutdown (REQ-677).
//!
//! Signals all registered components to stop and waits for them to
//! acknowledge completion within a configurable timeout.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of a drain operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainResult {
    /// All components drained before the deadline.
    Completed,
    /// The timeout elapsed before all components finished.
    TimedOut {
        /// Number of components that finished draining.
        completed: usize,
        /// Total number of registered components.
        total: usize,
    },
}

/// Shared drain state visible to all components.
#[derive(Debug)]
pub struct DrainToken {
    /// Set to `true` once a drain is initiated.
    draining: AtomicBool,
    /// Total number of registered components.
    total: AtomicUsize,
    /// Number of components that have acknowledged drain completion.
    completed: AtomicUsize,
}

impl DrainToken {
    fn new() -> Self {
        Self {
            draining: AtomicBool::new(false),
            total: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
        }
    }

    /// Returns `true` if a drain has been initiated.
    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::Acquire)
    }

    /// Called by a component to signal it has finished draining.
    pub fn mark_drained(&self) {
        self.completed.fetch_add(1, Ordering::Release);
    }
}

/// Handle given to each registered component to observe drain state.
#[derive(Debug, Clone)]
pub struct DrainHandle {
    token: Arc<DrainToken>,
}

impl DrainHandle {
    /// Returns `true` if the service is shutting down.
    pub fn is_draining(&self) -> bool {
        self.token.is_draining()
    }

    /// Signal that this component has completed its drain.
    pub fn mark_drained(&self) {
        self.token.mark_drained();
    }
}

/// Coordinates graceful shutdown of all registered components.
pub struct DrainCoordinator {
    token: Arc<DrainToken>,
    timeout: Duration,
}

impl DrainCoordinator {
    /// Create a new drain coordinator with the given timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            token: Arc::new(DrainToken::new()),
            timeout,
        }
    }

    /// Register a component and return a handle it can use to observe and
    /// acknowledge the drain.
    pub fn register(&self) -> DrainHandle {
        self.token.total.fetch_add(1, Ordering::Release);
        DrainHandle {
            token: Arc::clone(&self.token),
        }
    }

    /// Number of registered components.
    pub fn registered_count(&self) -> usize {
        self.token.total.load(Ordering::Acquire)
    }

    /// Number of components that have acknowledged drain so far.
    pub fn completed_count(&self) -> usize {
        self.token.completed.load(Ordering::Acquire)
    }

    /// Signal all components to begin draining.
    pub fn start_drain(&self) {
        self.token.draining.store(true, Ordering::Release);
    }

    /// Block until all components have drained or the timeout expires.
    pub fn wait_for_drain(&self) -> DrainResult {
        let deadline = Instant::now() + self.timeout;
        let total = self.token.total.load(Ordering::Acquire);
        loop {
            let completed = self.token.completed.load(Ordering::Acquire);
            if completed >= total {
                return DrainResult::Completed;
            }
            if Instant::now() >= deadline {
                return DrainResult::TimedOut { completed, total };
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_completes_before_timeout() {
        let coord = DrainCoordinator::new(Duration::from_secs(2));
        let h1 = coord.register();
        let h2 = coord.register();
        assert_eq!(coord.registered_count(), 2);

        coord.start_drain();
        assert!(h1.is_draining());
        assert!(h2.is_draining());

        h1.mark_drained();
        h2.mark_drained();

        assert_eq!(coord.wait_for_drain(), DrainResult::Completed);
    }

    #[test]
    fn drain_times_out() {
        let coord = DrainCoordinator::new(Duration::from_millis(50));
        let _h1 = coord.register();
        let h2 = coord.register();

        coord.start_drain();
        // Only one component drains.
        h2.mark_drained();

        let result = coord.wait_for_drain();
        assert_eq!(result, DrainResult::TimedOut { completed: 1, total: 2 });
    }

    #[test]
    fn drain_progress_tracking() {
        let coord = DrainCoordinator::new(Duration::from_secs(1));
        let h1 = coord.register();
        let h2 = coord.register();
        let h3 = coord.register();

        assert_eq!(coord.completed_count(), 0);

        coord.start_drain();
        h1.mark_drained();
        assert_eq!(coord.completed_count(), 1);

        h2.mark_drained();
        assert_eq!(coord.completed_count(), 2);

        h3.mark_drained();
        assert_eq!(coord.completed_count(), 3);

        assert_eq!(coord.wait_for_drain(), DrainResult::Completed);
    }

    #[test]
    fn not_draining_initially() {
        let coord = DrainCoordinator::new(Duration::from_secs(1));
        let h = coord.register();
        assert!(!h.is_draining());
    }
}
