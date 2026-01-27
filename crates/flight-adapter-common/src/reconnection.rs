// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Reconnection strategy helpers.

use std::time::Duration;

/// Exponential backoff reconnection strategy with caps.
#[derive(Debug, Clone)]
pub struct ReconnectionStrategy {
    max_attempts: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl ReconnectionStrategy {
    /// Create a new reconnection strategy.
    pub fn new(max_attempts: u32, initial_backoff: Duration, max_backoff: Duration) -> Self {
        Self {
            max_attempts,
            initial_backoff,
            max_backoff,
        }
    }

    /// Maximum number of attempts allowed.
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Initial backoff delay.
    pub fn initial_backoff(&self) -> Duration {
        self.initial_backoff
    }

    /// Maximum backoff delay.
    pub fn max_backoff(&self) -> Duration {
        self.max_backoff
    }

    /// Whether another attempt should be made.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt <= self.max_attempts
    }

    /// Compute the next backoff delay for a given attempt (1-based).
    pub fn next_backoff(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return self.initial_backoff.min(self.max_backoff);
        }

        let exp = 2u64.checked_pow(attempt.saturating_sub(1)).unwrap_or(u64::MAX);
        let base_ms = self.initial_backoff.as_millis() as u64;
        let backoff_ms = base_ms.saturating_mul(exp);
        let backoff = Duration::from_millis(backoff_ms);

        if backoff > self.max_backoff {
            self.max_backoff
        } else {
            backoff
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_progression() {
        let strategy = ReconnectionStrategy::new(
            5,
            Duration::from_secs(1),
            Duration::from_secs(30),
        );

        assert_eq!(strategy.next_backoff(1), Duration::from_secs(1));
        assert_eq!(strategy.next_backoff(2), Duration::from_secs(2));
        assert_eq!(strategy.next_backoff(3), Duration::from_secs(4));
        assert_eq!(strategy.next_backoff(4), Duration::from_secs(8));
        assert_eq!(strategy.next_backoff(5), Duration::from_secs(16));
        assert_eq!(strategy.next_backoff(6), Duration::from_secs(30));
    }

    #[test]
    fn test_should_retry() {
        let strategy = ReconnectionStrategy::new(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
        );

        assert!(strategy.should_retry(1));
        assert!(strategy.should_retry(3));
        assert!(!strategy.should_retry(4));
    }
}
