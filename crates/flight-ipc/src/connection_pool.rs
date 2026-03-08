// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Connection pool for managing multiple concurrent IPC client sessions.
//!
//! [`ConnectionPool`] tracks connected clients, enforces a maximum connection
//! limit, and prunes idle connections that have exceeded a configurable timeout.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced by connection pool operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PoolError {
    /// The pool has reached its maximum number of connections.
    #[error("Connection pool full (max {max})")]
    PoolFull {
        /// Maximum allowed connections.
        max: usize,
    },

    /// A connection with the given ID already exists.
    #[error("Duplicate connection ID: {id}")]
    DuplicateId {
        /// The duplicated connection identifier.
        id: String,
    },

    /// No connection with the given ID was found.
    #[error("Connection not found: {id}")]
    NotFound {
        /// The requested connection identifier.
        id: String,
    },
}

// ---------------------------------------------------------------------------
// Connection info
// ---------------------------------------------------------------------------

/// Metadata for a single active IPC connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionInfo {
    /// Unique connection identifier.
    pub id: String,
    /// Human-readable client name.
    pub client_name: String,
    /// Timestamp (epoch seconds) when the connection was established.
    pub connected_at: u64,
    /// Timestamp (epoch seconds) of the most recent activity.
    pub last_activity: u64,
    /// Number of messages exchanged on this connection.
    pub message_count: u64,
}

// ---------------------------------------------------------------------------
// Connection pool
// ---------------------------------------------------------------------------

/// Manages a bounded set of IPC client connections with idle pruning.
pub struct ConnectionPool {
    connections: HashMap<String, ConnectionInfo>,
    max_connections: usize,
    idle_timeout_secs: u64,
}

impl ConnectionPool {
    /// Create a new pool with the given capacity and idle timeout.
    pub fn new(max_connections: usize, idle_timeout_secs: u64) -> Self {
        Self {
            connections: HashMap::new(),
            max_connections,
            idle_timeout_secs,
        }
    }

    /// Register a new connection.
    ///
    /// # Errors
    ///
    /// Returns [`PoolError::PoolFull`] if the pool is at capacity, or
    /// [`PoolError::DuplicateId`] if `id` is already registered.
    pub fn connect(&mut self, id: &str, client_name: &str, now: u64) -> Result<(), PoolError> {
        if self.connections.len() >= self.max_connections {
            return Err(PoolError::PoolFull {
                max: self.max_connections,
            });
        }
        if self.connections.contains_key(id) {
            return Err(PoolError::DuplicateId { id: id.to_owned() });
        }
        self.connections.insert(
            id.to_owned(),
            ConnectionInfo {
                id: id.to_owned(),
                client_name: client_name.to_owned(),
                connected_at: now,
                last_activity: now,
                message_count: 0,
            },
        );
        Ok(())
    }

    /// Remove a connection by ID.  Returns `true` if it was present.
    pub fn disconnect(&mut self, id: &str) -> bool {
        self.connections.remove(id).is_some()
    }

    /// Record activity on an existing connection and bump its message count.
    ///
    /// # Errors
    ///
    /// Returns [`PoolError::NotFound`] if `id` is not registered.
    pub fn activity(&mut self, id: &str, now: u64) -> Result<(), PoolError> {
        let conn = self
            .connections
            .get_mut(id)
            .ok_or_else(|| PoolError::NotFound { id: id.to_owned() })?;
        conn.last_activity = now;
        conn.message_count += 1;
        Ok(())
    }

    /// Remove all connections whose last activity is older than `now - idle_timeout_secs`.
    ///
    /// Returns the IDs of pruned connections.
    pub fn prune_idle(&mut self, now: u64) -> Vec<String> {
        let threshold = now.saturating_sub(self.idle_timeout_secs);
        let idle_ids: Vec<String> = self
            .connections
            .iter()
            .filter(|(_, info)| info.last_activity < threshold)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &idle_ids {
            self.connections.remove(id);
        }
        idle_ids
    }

    /// Number of currently active connections.
    pub fn active_count(&self) -> usize {
        self.connections.len()
    }

    /// Look up a connection by ID.
    pub fn get(&self, id: &str) -> Option<&ConnectionInfo> {
        self.connections.get(id)
    }

    /// Returns `true` if the pool has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.connections.len() >= self.max_connections
    }

    /// Returns all active connections (unordered).
    pub fn all_connections(&self) -> Vec<&ConnectionInfo> {
        self.connections.values().collect()
    }
}

// ---------------------------------------------------------------------------
// Client-side connection pool (acquire/release)
// ---------------------------------------------------------------------------

/// Configuration for [`ClientConnectionPool`].
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections the pool will manage.
    pub max_connections: usize,
    /// Duration after which an idle connection is eligible for eviction.
    pub idle_timeout: Duration,
    /// Interval between automatic health checks.
    pub health_check_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 8,
            idle_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// A connection checked out from [`ClientConnectionPool`].
#[derive(Debug)]
pub struct PooledConnection {
    /// Unique connection identifier within the pool.
    pub id: u64,
    created_at: Instant,
    last_used: Instant,
    healthy: bool,
}

impl PooledConnection {
    /// Returns `true` if this connection is considered healthy.
    pub fn is_healthy(&self) -> bool {
        self.healthy
    }

    /// Mark this connection as unhealthy. When released, it will be discarded.
    pub fn mark_unhealthy(&mut self) {
        self.healthy = false;
    }

    /// When this connection was created.
    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    /// When this connection was last used.
    pub fn last_used(&self) -> Instant {
        self.last_used
    }
}

/// Snapshot of pool statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolStats {
    /// Number of connections currently checked out.
    pub active: usize,
    /// Number of connections sitting idle in the pool.
    pub idle: usize,
    /// Total managed connections (active + idle).
    pub total: usize,
    /// Cumulative count of failed / discarded connections.
    pub failed: u64,
}

/// Client-side connection pool with acquire/release semantics, health
/// tracking, and idle eviction.
///
/// Uses a [`VecDeque`] for idle connections so that the oldest idle
/// connection is reused first (FIFO).
pub struct ClientConnectionPool {
    config: PoolConfig,
    idle_connections: VecDeque<PooledConnection>,
    active_count: usize,
    next_id: u64,
    failed_count: AtomicU64,
}

impl ClientConnectionPool {
    /// Create a new pool with the given configuration.
    pub fn new(config: PoolConfig) -> Self {
        Self {
            config,
            idle_connections: VecDeque::new(),
            active_count: 0,
            next_id: 1,
            failed_count: AtomicU64::new(0),
        }
    }

    /// Acquire a connection from the pool.
    ///
    /// Attempts to reuse an idle healthy connection. If none is available and
    /// the pool has capacity, a new connection is created.
    ///
    /// # Errors
    ///
    /// Returns [`PoolError::PoolFull`] when the pool has reached its maximum.
    pub fn acquire(&mut self) -> Result<PooledConnection, PoolError> {
        while let Some(mut conn) = self.idle_connections.pop_front() {
            if !conn.healthy {
                self.failed_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            if conn.last_used.elapsed() > self.config.idle_timeout {
                continue;
            }
            conn.last_used = Instant::now();
            self.active_count += 1;
            return Ok(conn);
        }

        let total = self.active_count + self.idle_connections.len();
        if total >= self.config.max_connections {
            return Err(PoolError::PoolFull {
                max: self.config.max_connections,
            });
        }

        let now = Instant::now();
        let id = self.next_id;
        self.next_id += 1;
        self.active_count += 1;
        Ok(PooledConnection {
            id,
            created_at: now,
            last_used: now,
            healthy: true,
        })
    }

    /// Return a connection to the pool.
    ///
    /// Healthy connections are placed back into the idle queue. Unhealthy
    /// connections are discarded and the failed counter is incremented.
    pub fn release(&mut self, mut conn: PooledConnection) {
        self.active_count = self.active_count.saturating_sub(1);
        if !conn.healthy {
            self.failed_count.fetch_add(1, Ordering::Relaxed);
            return;
        }
        conn.last_used = Instant::now();
        self.idle_connections.push_back(conn);
    }

    /// Mark an idle connection as unhealthy by ID.
    pub fn mark_unhealthy(&mut self, id: u64) {
        if let Some(conn) = self.idle_connections.iter_mut().find(|c| c.id == id) {
            conn.healthy = false;
        }
    }

    /// Evict idle connections that have exceeded the configured idle timeout,
    /// as well as any unhealthy connections. Returns the number evicted.
    pub fn evict_idle(&mut self) -> usize {
        let timeout = self.config.idle_timeout;
        let before = self.idle_connections.len();
        self.idle_connections
            .retain(|conn| conn.healthy && conn.last_used.elapsed() <= timeout);
        let evicted = before - self.idle_connections.len();
        self.failed_count
            .fetch_add(evicted as u64, Ordering::Relaxed);
        evicted
    }

    /// Evict all unhealthy idle connections. Returns the number evicted.
    pub fn evict_unhealthy(&mut self) -> usize {
        let before = self.idle_connections.len();
        self.idle_connections.retain(|conn| conn.healthy);
        let evicted = before - self.idle_connections.len();
        self.failed_count
            .fetch_add(evicted as u64, Ordering::Relaxed);
        evicted
    }

    /// Return a snapshot of the pool's current statistics.
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            active: self.active_count,
            idle: self.idle_connections.len(),
            total: self.active_count + self.idle_connections.len(),
            failed: self.failed_count.load(Ordering::Relaxed),
        }
    }

    /// Access the pool configuration.
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // 1. New pool is empty
    #[test]
    fn new_pool_is_empty() {
        let pool = ConnectionPool::new(10, 300);
        assert_eq!(pool.active_count(), 0);
        assert!(!pool.is_full());
    }

    // 2. Connect adds a client
    #[test]
    fn connect_adds_client() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "flightctl", 100).unwrap();
        assert_eq!(pool.active_count(), 1);
        let info = pool.get("c1").unwrap();
        assert_eq!(info.client_name, "flightctl");
        assert_eq!(info.connected_at, 100);
        assert_eq!(info.message_count, 0);
    }

    // 3. Duplicate ID rejected
    #[test]
    fn duplicate_id_rejected() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "a", 0).unwrap();
        let err = pool.connect("c1", "b", 1).unwrap_err();
        assert_eq!(
            err,
            PoolError::DuplicateId {
                id: "c1".to_owned()
            }
        );
    }

    // 4. Pool full rejected
    #[test]
    fn pool_full_rejected() {
        let mut pool = ConnectionPool::new(2, 300);
        pool.connect("c1", "a", 0).unwrap();
        pool.connect("c2", "b", 0).unwrap();
        assert!(pool.is_full());
        let err = pool.connect("c3", "c", 0).unwrap_err();
        assert_eq!(err, PoolError::PoolFull { max: 2 });
    }

    // 5. Disconnect removes client
    #[test]
    fn disconnect_removes_client() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "a", 0).unwrap();
        assert!(pool.disconnect("c1"));
        assert_eq!(pool.active_count(), 0);
        assert!(pool.get("c1").is_none());
    }

    // 6. Disconnect unknown returns false
    #[test]
    fn disconnect_unknown_returns_false() {
        let mut pool = ConnectionPool::new(10, 300);
        assert!(!pool.disconnect("nope"));
    }

    // 7. Activity updates timestamp and message count
    #[test]
    fn activity_updates_connection() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "a", 100).unwrap();
        pool.activity("c1", 200).unwrap();
        let info = pool.get("c1").unwrap();
        assert_eq!(info.last_activity, 200);
        assert_eq!(info.message_count, 1);
    }

    // 8. Activity on unknown ID returns error
    #[test]
    fn activity_unknown_id_errors() {
        let mut pool = ConnectionPool::new(10, 300);
        let err = pool.activity("ghost", 0).unwrap_err();
        assert_eq!(
            err,
            PoolError::NotFound {
                id: "ghost".to_owned()
            }
        );
    }

    // 9. Prune idle removes stale connections
    #[test]
    fn prune_idle_removes_stale() {
        let mut pool = ConnectionPool::new(10, 60);
        pool.connect("old", "a", 0).unwrap();
        pool.connect("new", "b", 100).unwrap();
        // At t=120, "old" last_activity=0 < 120-60=60 → pruned
        let pruned = pool.prune_idle(120);
        assert_eq!(pruned, vec!["old"]);
        assert_eq!(pool.active_count(), 1);
        assert!(pool.get("new").is_some());
    }

    // 10. Prune idle with no stale connections
    #[test]
    fn prune_idle_none_stale() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "a", 100).unwrap();
        let pruned = pool.prune_idle(100);
        assert!(pruned.is_empty());
        assert_eq!(pool.active_count(), 1);
    }

    // 11. All connections returns all
    #[test]
    fn all_connections_returns_all() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "a", 0).unwrap();
        pool.connect("c2", "b", 0).unwrap();
        let all = pool.all_connections();
        assert_eq!(all.len(), 2);
    }

    // 12. Connect after disconnect frees slot
    #[test]
    fn connect_after_disconnect_frees_slot() {
        let mut pool = ConnectionPool::new(1, 300);
        pool.connect("c1", "a", 0).unwrap();
        assert!(pool.is_full());
        pool.disconnect("c1");
        assert!(!pool.is_full());
        pool.connect("c2", "b", 1).unwrap();
        assert_eq!(pool.active_count(), 1);
    }

    // 13. Multiple activities accumulate message count
    #[test]
    fn multiple_activities_accumulate() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "a", 0).unwrap();
        for t in 1..=5 {
            pool.activity("c1", t).unwrap();
        }
        let info = pool.get("c1").unwrap();
        assert_eq!(info.message_count, 5);
        assert_eq!(info.last_activity, 5);
    }

    // 14. Prune with saturating subtraction at t=0
    #[test]
    fn prune_at_time_zero_does_not_underflow() {
        let mut pool = ConnectionPool::new(10, 300);
        pool.connect("c1", "a", 0).unwrap();
        // now=0, threshold = 0.saturating_sub(300) = 0
        // last_activity (0) is NOT less than threshold (0), so not pruned
        let pruned = pool.prune_idle(0);
        assert!(pruned.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Client-side pool tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod client_pool_tests {
    use super::*;
    use std::thread;

    fn test_config() -> PoolConfig {
        PoolConfig {
            max_connections: 4,
            idle_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(30),
        }
    }

    fn short_timeout_config() -> PoolConfig {
        PoolConfig {
            max_connections: 4,
            idle_timeout: Duration::from_millis(50),
            health_check_interval: Duration::from_millis(10),
        }
    }

    // 1. New pool is empty
    #[test]
    fn new_pool_is_empty() {
        let pool = ClientConnectionPool::new(test_config());
        let stats = pool.stats();
        assert_eq!(stats.active, 0);
        assert_eq!(stats.idle, 0);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.failed, 0);
    }

    // 2. Acquire creates a connection
    #[test]
    fn acquire_creates_connection() {
        let mut pool = ClientConnectionPool::new(test_config());
        let conn = pool.acquire().unwrap();
        assert!(conn.is_healthy());
        assert_eq!(pool.stats().active, 1);
    }

    // 3. Acquire/release cycle reuses connection
    #[test]
    fn acquire_release_cycle() {
        let mut pool = ClientConnectionPool::new(test_config());
        let conn = pool.acquire().unwrap();
        let id = conn.id;
        pool.release(conn);
        assert_eq!(pool.stats().active, 0);
        assert_eq!(pool.stats().idle, 1);

        let conn2 = pool.acquire().unwrap();
        assert_eq!(conn2.id, id);
    }

    // 4. Max connections enforced
    #[test]
    fn max_connections_enforced() {
        let mut pool = ClientConnectionPool::new(test_config());
        let mut conns = Vec::new();
        for _ in 0..4 {
            conns.push(pool.acquire().unwrap());
        }
        let err = pool.acquire().unwrap_err();
        assert_eq!(err, PoolError::PoolFull { max: 4 });
    }

    // 5. Release unhealthy connection is discarded
    #[test]
    fn release_unhealthy_discards() {
        let mut pool = ClientConnectionPool::new(test_config());
        let mut conn = pool.acquire().unwrap();
        conn.mark_unhealthy();
        pool.release(conn);
        assert_eq!(pool.stats().idle, 0);
        assert_eq!(pool.stats().failed, 1);
    }

    // 6. Idle eviction
    #[test]
    fn idle_eviction() {
        let mut pool = ClientConnectionPool::new(short_timeout_config());
        let conn = pool.acquire().unwrap();
        pool.release(conn);
        assert_eq!(pool.stats().idle, 1);

        thread::sleep(Duration::from_millis(100));
        let evicted = pool.evict_idle();
        assert_eq!(evicted, 1);
        assert_eq!(pool.stats().idle, 0);
    }

    // 7. Acquire skips expired idle connections
    #[test]
    fn acquire_skips_expired_idle() {
        let mut pool = ClientConnectionPool::new(short_timeout_config());
        let conn = pool.acquire().unwrap();
        pool.release(conn);

        thread::sleep(Duration::from_millis(100));
        let conn2 = pool.acquire().unwrap();
        assert_eq!(conn2.id, 2); // new connection, not the expired one
    }

    // 8. Mark unhealthy and evict
    #[test]
    fn mark_unhealthy_and_evict() {
        let mut pool = ClientConnectionPool::new(test_config());
        let conn = pool.acquire().unwrap();
        let id = conn.id;
        pool.release(conn);

        pool.mark_unhealthy(id);
        let evicted = pool.evict_unhealthy();
        assert_eq!(evicted, 1);
        assert_eq!(pool.stats().idle, 0);
    }

    // 9. Pool stats snapshot
    #[test]
    fn pool_stats_snapshot() {
        let mut pool = ClientConnectionPool::new(test_config());
        let c1 = pool.acquire().unwrap();
        let _c2 = pool.acquire().unwrap();
        pool.release(c1);

        let stats = pool.stats();
        assert_eq!(stats.active, 1);
        assert_eq!(stats.idle, 1);
        assert_eq!(stats.total, 2);
    }

    // 10. Config is accessible
    #[test]
    fn config_accessible() {
        let pool = ClientConnectionPool::new(test_config());
        assert_eq!(pool.config().max_connections, 4);
    }

    // 11. Default pool config values
    #[test]
    fn default_pool_config() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections, 8);
        assert_eq!(config.idle_timeout, Duration::from_secs(300));
        assert_eq!(config.health_check_interval, Duration::from_secs(30));
    }

    // 12. Send + Sync assertions
    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PoolConfig>();
        assert_send_sync::<PooledConnection>();
        assert_send_sync::<PoolStats>();
        assert_send_sync::<ClientConnectionPool>();
    }
}
