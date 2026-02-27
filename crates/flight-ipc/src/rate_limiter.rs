// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Token-bucket rate limiter for IPC endpoints.
//!
//! Each client is assigned an independent [`TokenBucket`] that refills at a
//! configurable rate.  The [`RateLimiter`] front-end manages per-client buckets
//! and falls back to a default configuration for unknown clients.

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Per-endpoint / per-client token-bucket rate limiter.
pub struct RateLimiter {
    buckets: HashMap<String, TokenBucket>,
    default_config: RateLimitConfig,
}

/// Configuration that governs a single token bucket.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum tokens the bucket can hold.
    pub max_tokens: u32,
    /// Tokens added per second.
    pub refill_rate: f64,
    /// Minimum interval between refill calculations.
    pub refill_interval: Duration,
}

/// Outcome of a rate-limit check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitResult {
    /// The request is allowed.
    Allowed,
    /// The request was rejected; retry after the given number of milliseconds.
    Limited {
        /// Suggested back-off in milliseconds.
        retry_after_ms: u64,
    },
}

// ---------------------------------------------------------------------------
// TokenBucket (internal)
// ---------------------------------------------------------------------------

struct TokenBucket {
    tokens: f64,
    max_tokens: u32,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        Self {
            tokens: f64::from(config.max_tokens),
            max_tokens: config.max_tokens,
            refill_rate: config.refill_rate,
            last_refill: Instant::now(),
        }
    }

    #[cfg(test)]
    fn new_at(config: &RateLimitConfig, now: Instant) -> Self {
        Self {
            tokens: f64::from(config.max_tokens),
            max_tokens: config.max_tokens,
            refill_rate: config.refill_rate,
            last_refill: now,
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let added = elapsed.as_secs_f64() * self.refill_rate;
        if added > 0.0 {
            self.tokens = (self.tokens + added).min(f64::from(self.max_tokens));
            self.last_refill = now;
        }
    }

    #[cfg(test)]
    fn refill_at(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.last_refill);
        let added = elapsed.as_secs_f64() * self.refill_rate;
        if added > 0.0 {
            self.tokens = (self.tokens + added).min(f64::from(self.max_tokens));
            self.last_refill = now;
        }
    }

    fn try_consume(&mut self, cost: u32) -> RateLimitResult {
        self.refill();
        let cost_f = f64::from(cost);
        if self.tokens >= cost_f {
            self.tokens -= cost_f;
            RateLimitResult::Allowed
        } else {
            let deficit = cost_f - self.tokens;
            let wait_secs = if self.refill_rate > 0.0 {
                deficit / self.refill_rate
            } else {
                f64::from(u32::MAX)
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let retry_after_ms = (wait_secs * 1000.0).ceil() as u64;
            RateLimitResult::Limited { retry_after_ms }
        }
    }

    fn remaining(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let r = self.tokens.floor() as u32;
        r
    }
}

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

impl RateLimiter {
    /// Create a new limiter with the given default bucket configuration.
    pub fn new(default_config: RateLimitConfig) -> Self {
        Self {
            buckets: HashMap::new(),
            default_config,
        }
    }

    /// Check (and consume one token for) `client_id`.
    ///
    /// A bucket is lazily created with the default config if the client has not
    /// been seen before.
    pub fn check(&mut self, client_id: &str) -> RateLimitResult {
        self.check_with_cost(client_id, 1)
    }

    /// Check and consume `cost` tokens for `client_id`.
    pub fn check_with_cost(&mut self, client_id: &str, cost: u32) -> RateLimitResult {
        let config = &self.default_config;
        let bucket = self
            .buckets
            .entry(client_id.to_owned())
            .or_insert_with(|| TokenBucket::new(config));
        bucket.try_consume(cost)
    }

    /// Assign a custom configuration to a specific client, replacing any
    /// existing bucket.
    pub fn configure_client(&mut self, client_id: &str, config: RateLimitConfig) {
        self.buckets
            .insert(client_id.to_owned(), TokenBucket::new(&config));
    }

    /// Remove the bucket for `client_id`.
    pub fn remove_client(&mut self, client_id: &str) {
        self.buckets.remove(client_id);
    }

    /// Return the number of remaining whole tokens for `client_id`, if known.
    pub fn remaining_tokens(&self, client_id: &str) -> Option<u32> {
        self.buckets.get(client_id).map(TokenBucket::remaining)
    }

    /// Number of tracked clients.
    pub fn client_count(&self) -> usize {
        self.buckets.len()
    }

    /// Drop all client buckets.
    pub fn reset(&mut self) {
        self.buckets.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn default_config() -> RateLimitConfig {
        RateLimitConfig {
            max_tokens: 10,
            refill_rate: 10.0, // 10 tokens/s
            refill_interval: Duration::from_millis(100),
        }
    }

    // 1. Single request allowed
    #[test]
    fn single_request_allowed() {
        let mut rl = RateLimiter::new(default_config());
        assert_eq!(rl.check("client-a"), RateLimitResult::Allowed);
    }

    // 2. Burst up to max_tokens allowed
    #[test]
    fn burst_up_to_max_tokens_allowed() {
        let mut rl = RateLimiter::new(default_config());
        for _ in 0..10 {
            assert_eq!(rl.check("client-b"), RateLimitResult::Allowed);
        }
    }

    // 3. Exceeding max_tokens gets limited
    #[test]
    fn exceeding_max_tokens_limited() {
        let mut rl = RateLimiter::new(default_config());
        for _ in 0..10 {
            rl.check("client-c");
        }
        match rl.check("client-c") {
            RateLimitResult::Limited { retry_after_ms } => {
                assert!(retry_after_ms > 0);
            }
            RateLimitResult::Allowed => panic!("should have been limited"),
        }
    }

    // 4. Tokens refill after time passes
    #[test]
    fn tokens_refill_after_time() {
        let config = RateLimitConfig {
            max_tokens: 2,
            refill_rate: 100.0, // fast refill for test
            refill_interval: Duration::from_millis(1),
        };
        let mut rl = RateLimiter::new(config);

        // Drain
        assert_eq!(rl.check("client-d"), RateLimitResult::Allowed);
        assert_eq!(rl.check("client-d"), RateLimitResult::Allowed);
        assert!(matches!(
            rl.check("client-d"),
            RateLimitResult::Limited { .. }
        ));

        // Wait long enough for at least 1 token to refill
        thread::sleep(Duration::from_millis(50));

        assert_eq!(rl.check("client-d"), RateLimitResult::Allowed);
    }

    // 5. Custom cost per request
    #[test]
    fn custom_cost_per_request() {
        let mut rl = RateLimiter::new(default_config());
        // Consume 8 tokens at once
        assert_eq!(rl.check_with_cost("client-e", 8), RateLimitResult::Allowed);
        // 2 remaining, cost 3 should fail
        assert!(matches!(
            rl.check_with_cost("client-e", 3),
            RateLimitResult::Limited { .. }
        ));
    }

    // 6. Per-client configuration
    #[test]
    fn per_client_configuration() {
        let mut rl = RateLimiter::new(default_config());
        let custom = RateLimitConfig {
            max_tokens: 2,
            refill_rate: 1.0,
            refill_interval: Duration::from_secs(1),
        };
        rl.configure_client("vip", custom);

        // vip bucket has only 2 tokens
        assert_eq!(rl.check("vip"), RateLimitResult::Allowed);
        assert_eq!(rl.check("vip"), RateLimitResult::Allowed);
        assert!(matches!(
            rl.check("vip"),
            RateLimitResult::Limited { .. }
        ));

        // default client still has 10
        for _ in 0..10 {
            assert_eq!(rl.check("default"), RateLimitResult::Allowed);
        }
    }

    // 7. Remove client clears bucket
    #[test]
    fn remove_client_clears_bucket() {
        let mut rl = RateLimiter::new(default_config());
        rl.check("gone");
        assert_eq!(rl.client_count(), 1);

        rl.remove_client("gone");
        assert_eq!(rl.client_count(), 0);
        assert_eq!(rl.remaining_tokens("gone"), None);
    }

    // 8. Reset clears all
    #[test]
    fn reset_clears_all() {
        let mut rl = RateLimiter::new(default_config());
        rl.check("a");
        rl.check("b");
        rl.check("c");
        assert_eq!(rl.client_count(), 3);

        rl.reset();
        assert_eq!(rl.client_count(), 0);
    }

    // 9. New client gets default config
    #[test]
    fn new_client_gets_default_config() {
        let mut rl = RateLimiter::new(default_config());
        // First access creates bucket with max_tokens = 10
        assert_eq!(rl.remaining_tokens("fresh"), None);
        rl.check("fresh");
        // After consuming 1, should have 9 left
        assert_eq!(rl.remaining_tokens("fresh"), Some(9));
    }

    // 10. Remaining tokens accurate
    #[test]
    fn remaining_tokens_accurate() {
        let mut rl = RateLimiter::new(default_config());
        rl.check("counter");
        assert_eq!(rl.remaining_tokens("counter"), Some(9));

        rl.check_with_cost("counter", 4);
        assert_eq!(rl.remaining_tokens("counter"), Some(5));

        rl.check_with_cost("counter", 5);
        assert_eq!(rl.remaining_tokens("counter"), Some(0));
    }

    // 11. Retry-after value is reasonable
    #[test]
    fn retry_after_is_reasonable() {
        let config = RateLimitConfig {
            max_tokens: 1,
            refill_rate: 2.0, // 2 tokens/s → 500ms per token
            refill_interval: Duration::from_millis(100),
        };
        let mut rl = RateLimiter::new(config);
        rl.check("timing");
        match rl.check("timing") {
            RateLimitResult::Limited { retry_after_ms } => {
                // Should be ~500ms (1 token deficit / 2 tokens-per-sec)
                assert!(
                    retry_after_ms >= 400 && retry_after_ms <= 600,
                    "unexpected retry_after_ms: {retry_after_ms}"
                );
            }
            RateLimitResult::Allowed => panic!("should have been limited"),
        }
    }

    // 12. Token bucket refill via manual instant (unit-level)
    #[test]
    fn token_bucket_refill_manual_instant() {
        let config = default_config();
        let start = Instant::now();
        let mut bucket = TokenBucket::new_at(&config, start);

        // Drain all tokens directly to avoid real-time refill in try_consume
        bucket.tokens = 0.0;
        assert_eq!(bucket.remaining(), 0);

        // Advance by 500ms → should add 5 tokens (10 tokens/s × 0.5s)
        let later = start + Duration::from_millis(500);
        bucket.refill_at(later);
        assert_eq!(bucket.remaining(), 5);
    }
}
