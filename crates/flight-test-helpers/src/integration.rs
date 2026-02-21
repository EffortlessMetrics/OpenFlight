// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Lightweight integration test harness primitives.

use crate::fixtures::TestConfig;
use std::time::{Duration, Instant};

/// Basic harness with timing boundaries used by integration tests.
#[derive(Debug, Clone)]
pub struct TestHarness {
    config: TestConfig,
    started: Instant,
}

impl TestHarness {
    pub fn new(config: TestConfig) -> Self {
        Self {
            config,
            started: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn timed_out(&self) -> bool {
        self.elapsed() >= self.config.timeout
    }

    pub fn poll_interval(&self) -> Duration {
        self.config.poll_interval
    }
}

#[cfg(test)]
mod tests {
    use super::TestHarness;
    use crate::fixtures::TestConfigBuilder;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_harness_timeout() {
        let config = TestConfigBuilder::default()
            .with_timeout(Duration::from_millis(20))
            .build();
        let harness = TestHarness::new(config);
        thread::sleep(Duration::from_millis(25));
        assert!(harness.timed_out());
    }
}
