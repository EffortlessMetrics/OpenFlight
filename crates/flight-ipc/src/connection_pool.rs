// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Connection pool for managing multiple concurrent IPC client sessions.
//!
//! [`ConnectionPool`] tracks connected clients, enforces a maximum connection
//! limit, and prunes idle connections that have exceeded a configurable timeout.
//!
//! [`ClientConnectionPool`] provides a client-side pool of gRPC connections
//! with health checking, round-robin selection, and connection lifecycle
//! management. [`KeepaliveConfig`] and [`PoolMetrics`] support observability.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
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
// KeepaliveConfig
// ---------------------------------------------------------------------------

/// Configures keepalive behaviour for pooled connections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepaliveConfig {
    /// Interval between keepalive pings.
    pub interval: Duration,
    /// How long to wait for a pong before declaring the connection dead.
    pub timeout: Duration,
    /// Maximum consecutive missed pings before eviction.
    pub max_missed_pings: u32,
}

impl Default for KeepaliveConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            max_missed_pings: 3,
        }
    }
}

impl KeepaliveConfig {
    /// Returns `true` when all fields have sensible positive values.
    pub fn is_valid(&self) -> bool {
        !self.interval.is_zero() && !self.timeout.is_zero() && self.max_missed_pings > 0
    }
}

// ---------------------------------------------------------------------------
// PoolMetrics
// ---------------------------------------------------------------------------

/// Observable metrics for a [`ClientConnectionPool`].
#[derive(Debug, Default)]
pub struct PoolMetrics {
    /// Total connections currently alive in the pool.
    pub active_connections: AtomicU64,
    /// Connections sitting idle (not checked-out).
    pub idle_connections: AtomicU64,
    /// Number of failed health checks since pool creation.
    pub failed_health_checks: AtomicU64,
    /// Total checkout requests served.
    pub total_requests: AtomicU64,
}

impl PoolMetrics {
    /// Snapshot the counters into a plain struct for serialisation or logging.
    pub fn snapshot(&self) -> PoolMetricsSnapshot {
        PoolMetricsSnapshot {
            active_connections: self.active_connections.load(Ordering::Relaxed),
            idle_connections: self.idle_connections.load(Ordering::Relaxed),
            failed_health_checks: self.failed_health_checks.load(Ordering::Relaxed),
            total_requests: self.total_requests.load(Ordering::Relaxed),
        }
    }
}

/// A plain-data snapshot of [`PoolMetrics`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolMetricsSnapshot {
    /// Total connections currently alive in the pool.
    pub active_connections: u64,
    /// Connections sitting idle (not checked-out).
    pub idle_connections: u64,
    /// Number of failed health checks since pool creation.
    pub failed_health_checks: u64,
    /// Total checkout requests served.
    pub total_requests: u64,
}

// ---------------------------------------------------------------------------
// PooledConnection — individual entry in the client pool
// ---------------------------------------------------------------------------

/// State of a connection inside the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    /// Available for checkout.
    Idle,
    /// Currently in use by a caller.
    InUse,
    /// Failed a health check and is awaiting eviction.
    Unhealthy,
}

/// A single connection tracked by [`ClientConnectionPool`].
#[derive(Debug)]
pub struct PooledConnection {
    /// Unique connection identifier.
    pub id: u64,
    /// Target endpoint address.
    pub endpoint: String,
    /// Current state.
    pub state: ConnState,
    /// Epoch-second timestamp of last successful health check.
    pub last_health_check: u64,
    /// Consecutive missed keepalive pings.
    pub missed_pings: u32,
    /// Total requests served by this connection.
    pub request_count: u64,
}

// ---------------------------------------------------------------------------
// SelectionStrategy
// ---------------------------------------------------------------------------

/// Strategy used to pick the next connection on checkout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// Cycle through connections in order.
    RoundRobin,
    /// Pick the connection with the fewest in-flight requests.
    LeastLoaded,
}

// ---------------------------------------------------------------------------
// ClientConnectionPool
// ---------------------------------------------------------------------------

/// Client-side pool of gRPC connections with health checking, configurable
/// size limits, and round-robin or least-loaded selection.
pub struct ClientConnectionPool {
    connections: Vec<PooledConnection>,
    min_connections: usize,
    max_connections: usize,
    keepalive: KeepaliveConfig,
    strategy: SelectionStrategy,
    metrics: PoolMetrics,
    next_id: u64,
    rr_index: usize,
}

impl ClientConnectionPool {
    /// Create a new pool targeting `endpoint`.
    ///
    /// `min` idle connections are pre-created; the pool will grow up to `max`.
    pub fn new(
        endpoint: &str,
        min: usize,
        max: usize,
        keepalive: KeepaliveConfig,
        strategy: SelectionStrategy,
    ) -> Self {
        assert!(min <= max, "min must be <= max");
        assert!(max > 0, "max must be > 0");

        let mut pool = Self {
            connections: Vec::with_capacity(max),
            min_connections: min,
            max_connections: max,
            keepalive,
            strategy,
            metrics: PoolMetrics::default(),
            next_id: 0,
            rr_index: 0,
        };

        // Pre-populate with `min` idle connections.
        for _ in 0..min {
            pool.create_connection(endpoint);
        }

        pool
    }

    // -- connection lifecycle -----------------------------------------------

    fn create_connection(&mut self, endpoint: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.connections.push(PooledConnection {
            id,
            endpoint: endpoint.to_owned(),
            state: ConnState::Idle,
            last_health_check: 0,
            missed_pings: 0,
            request_count: 0,
        });
        self.metrics
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .idle_connections
            .fetch_add(1, Ordering::Relaxed);
        id
    }

    /// Check out a connection using the configured [`SelectionStrategy`].
    ///
    /// Returns the connection ID, or `None` if the pool is exhausted.
    pub fn checkout(&mut self) -> Option<u64> {
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);

        let id = match self.strategy {
            SelectionStrategy::RoundRobin => self.checkout_round_robin(),
            SelectionStrategy::LeastLoaded => self.checkout_least_loaded(),
        };

        if let Some(conn_id) = id
            && let Some(conn) = self.connections.iter_mut().find(|c| c.id == conn_id)
        {
            conn.state = ConnState::InUse;
            conn.request_count += 1;
            self.metrics
                .idle_connections
                .fetch_sub(1, Ordering::Relaxed);
        }

        id
    }

    fn checkout_round_robin(&mut self) -> Option<u64> {
        let idle: Vec<usize> = self
            .connections
            .iter()
            .enumerate()
            .filter(|(_, c)| c.state == ConnState::Idle)
            .map(|(i, _)| i)
            .collect();
        if idle.is_empty() {
            return None;
        }
        // Find the next index >= rr_index, wrapping around.
        let pick = idle
            .iter()
            .find(|&&i| i >= self.rr_index)
            .or(idle.first())
            .copied()
            .unwrap();
        self.rr_index = pick + 1;
        Some(self.connections[pick].id)
    }

    fn checkout_least_loaded(&self) -> Option<u64> {
        self.connections
            .iter()
            .filter(|c| c.state == ConnState::Idle)
            .min_by_key(|c| c.request_count)
            .map(|c| c.id)
    }

    /// Return a connection to the pool after use.
    pub fn checkin(&mut self, id: u64) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.id == id)
            && conn.state == ConnState::InUse
        {
            conn.state = ConnState::Idle;
            self.metrics
                .idle_connections
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Simulate a health-check pass at time `now`.
    ///
    /// Connections with more missed pings than [`KeepaliveConfig::max_missed_pings`]
    /// are marked [`ConnState::Unhealthy`].
    pub fn health_check(&mut self, now: u64, healthy_ids: &[u64]) {
        for conn in &mut self.connections {
            if conn.state == ConnState::Unhealthy {
                continue;
            }
            if healthy_ids.contains(&conn.id) {
                conn.last_health_check = now;
                conn.missed_pings = 0;
            } else {
                conn.missed_pings += 1;
                self.metrics
                    .failed_health_checks
                    .fetch_add(1, Ordering::Relaxed);
                if conn.missed_pings >= self.keepalive.max_missed_pings {
                    let was_idle = conn.state == ConnState::Idle;
                    conn.state = ConnState::Unhealthy;
                    if was_idle {
                        self.metrics
                            .idle_connections
                            .fetch_sub(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    /// Evict all unhealthy connections.  Returns the IDs removed.
    pub fn evict_unhealthy(&mut self) -> Vec<u64> {
        let evicted: Vec<u64> = self
            .connections
            .iter()
            .filter(|c| c.state == ConnState::Unhealthy)
            .map(|c| c.id)
            .collect();
        let count = evicted.len() as u64;
        self.connections
            .retain(|c| c.state != ConnState::Unhealthy);
        self.metrics
            .active_connections
            .fetch_sub(count, Ordering::Relaxed);
        evicted
    }

    // -- accessors ----------------------------------------------------------

    /// Current number of connections (all states).
    pub fn size(&self) -> usize {
        self.connections.len()
    }

    /// Number of idle connections.
    pub fn idle_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|c| c.state == ConnState::Idle)
            .count()
    }

    /// Min pool size.
    pub fn min_connections(&self) -> usize {
        self.min_connections
    }

    /// Max pool size.
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }

    /// Reference to the keepalive config.
    pub fn keepalive(&self) -> &KeepaliveConfig {
        &self.keepalive
    }

    /// Reference to the live metrics.
    pub fn metrics(&self) -> &PoolMetrics {
        &self.metrics
    }

    /// Look up a connection by ID.
    pub fn get_connection(&self, id: u64) -> Option<&PooledConnection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// Returns `true` when the pool cannot grow further.
    pub fn is_full(&self) -> bool {
        self.connections.len() >= self.max_connections
    }

    /// Attempt to grow the pool by one connection. Returns the new ID, or
    /// `None` if already at capacity.
    pub fn grow(&mut self, endpoint: &str) -> Option<u64> {
        if self.is_full() {
            return None;
        }
        Some(self.create_connection(endpoint))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Existing ConnectionPool tests ===================================

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

    // ===== KeepaliveConfig tests ===========================================

    // 15. Default keepalive config is valid
    #[test]
    fn default_keepalive_is_valid() {
        let cfg = KeepaliveConfig::default();
        assert!(cfg.is_valid());
        assert_eq!(cfg.interval, Duration::from_secs(10));
        assert_eq!(cfg.timeout, Duration::from_secs(5));
        assert_eq!(cfg.max_missed_pings, 3);
    }

    // 16. Zero interval is invalid
    #[test]
    fn zero_interval_keepalive_invalid() {
        let cfg = KeepaliveConfig {
            interval: Duration::ZERO,
            ..KeepaliveConfig::default()
        };
        assert!(!cfg.is_valid());
    }

    // 17. Zero timeout is invalid
    #[test]
    fn zero_timeout_keepalive_invalid() {
        let cfg = KeepaliveConfig {
            timeout: Duration::ZERO,
            ..KeepaliveConfig::default()
        };
        assert!(!cfg.is_valid());
    }

    // 18. Zero max_missed_pings is invalid
    #[test]
    fn zero_max_missed_pings_invalid() {
        let cfg = KeepaliveConfig {
            max_missed_pings: 0,
            ..KeepaliveConfig::default()
        };
        assert!(!cfg.is_valid());
    }

    // ===== PoolMetrics tests ===============================================

    // 19. Default metrics are zeroed
    #[test]
    fn default_pool_metrics_zeroed() {
        let m = PoolMetrics::default();
        let snap = m.snapshot();
        assert_eq!(snap.active_connections, 0);
        assert_eq!(snap.idle_connections, 0);
        assert_eq!(snap.failed_health_checks, 0);
        assert_eq!(snap.total_requests, 0);
    }

    // 20. Metrics snapshot reflects atomic updates
    #[test]
    fn metrics_snapshot_reflects_updates() {
        let m = PoolMetrics::default();
        m.active_connections.store(5, Ordering::Relaxed);
        m.idle_connections.store(3, Ordering::Relaxed);
        m.failed_health_checks.store(1, Ordering::Relaxed);
        m.total_requests.store(42, Ordering::Relaxed);
        let snap = m.snapshot();
        assert_eq!(snap.active_connections, 5);
        assert_eq!(snap.idle_connections, 3);
        assert_eq!(snap.failed_health_checks, 1);
        assert_eq!(snap.total_requests, 42);
    }

    // ===== ClientConnectionPool tests ======================================

    // 21. Pool creation pre-populates min connections
    #[test]
    fn client_pool_creation_with_min_max() {
        let pool = ClientConnectionPool::new(
            "http://localhost:50051",
            2,
            5,
            KeepaliveConfig::default(),
            SelectionStrategy::RoundRobin,
        );
        assert_eq!(pool.size(), 2);
        assert_eq!(pool.idle_count(), 2);
        assert_eq!(pool.min_connections(), 2);
        assert_eq!(pool.max_connections(), 5);
        assert!(!pool.is_full());
    }

    // 22. Checkout and checkin cycle
    #[test]
    fn checkout_and_checkin() {
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            2,
            4,
            KeepaliveConfig::default(),
            SelectionStrategy::RoundRobin,
        );
        let id = pool.checkout().unwrap();
        assert_eq!(pool.idle_count(), 1);

        let conn = pool.get_connection(id).unwrap();
        assert_eq!(conn.state, ConnState::InUse);
        assert_eq!(conn.request_count, 1);

        pool.checkin(id);
        assert_eq!(pool.idle_count(), 2);
        let conn = pool.get_connection(id).unwrap();
        assert_eq!(conn.state, ConnState::Idle);
    }

    // 23. Checkout returns None when all busy
    #[test]
    fn checkout_returns_none_when_all_busy() {
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            1,
            1,
            KeepaliveConfig::default(),
            SelectionStrategy::RoundRobin,
        );
        let _id = pool.checkout().unwrap();
        assert!(pool.checkout().is_none());
    }

    // 24. Health check marks connections unhealthy after max_missed_pings
    #[test]
    fn health_check_evicts_stale() {
        let keepalive = KeepaliveConfig {
            max_missed_pings: 2,
            ..KeepaliveConfig::default()
        };
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            2,
            4,
            keepalive,
            SelectionStrategy::RoundRobin,
        );
        let id0 = pool.connections[0].id;
        let id1 = pool.connections[1].id;

        // First check: only id1 responds → id0 gets 1 miss
        pool.health_check(100, &[id1]);
        assert_eq!(pool.get_connection(id0).unwrap().missed_pings, 1);
        assert_eq!(pool.get_connection(id0).unwrap().state, ConnState::Idle);

        // Second check: still only id1 → id0 gets 2 misses → Unhealthy
        pool.health_check(200, &[id1]);
        assert_eq!(pool.get_connection(id0).unwrap().state, ConnState::Unhealthy);
        assert_eq!(pool.get_connection(id1).unwrap().state, ConnState::Idle);

        // Evict
        let evicted = pool.evict_unhealthy();
        assert_eq!(evicted, vec![id0]);
        assert_eq!(pool.size(), 1);
    }

    // 25. Round-robin selection distributes across connections
    #[test]
    fn round_robin_distributes() {
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            3,
            5,
            KeepaliveConfig::default(),
            SelectionStrategy::RoundRobin,
        );
        let a = pool.checkout().unwrap();
        pool.checkin(a);
        let b = pool.checkout().unwrap();
        pool.checkin(b);
        let c = pool.checkout().unwrap();
        pool.checkin(c);

        // All three should have been used (IDs 0, 1, 2)
        let mut used = vec![a, b, c];
        used.sort();
        used.dedup();
        assert_eq!(used.len(), 3);
    }

    // 26. Least-loaded selection picks connection with fewest requests
    #[test]
    fn least_loaded_selects_lowest() {
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            2,
            4,
            KeepaliveConfig::default(),
            SelectionStrategy::LeastLoaded,
        );

        // Checkout and checkin first connection 3 times
        for _ in 0..3 {
            let id = pool.checkout().unwrap();
            pool.checkin(id);
        }
        // Now conn 0 has 3 requests, conn 1 has 0 → least-loaded picks conn 1
        let next = pool.checkout().unwrap();
        assert_eq!(next, pool.connections[1].id);
    }

    // 27. Pool growth
    #[test]
    fn pool_grow() {
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            1,
            3,
            KeepaliveConfig::default(),
            SelectionStrategy::RoundRobin,
        );
        assert_eq!(pool.size(), 1);

        let id = pool.grow("http://localhost:50051").unwrap();
        assert_eq!(pool.size(), 2);
        assert_eq!(pool.get_connection(id).unwrap().state, ConnState::Idle);

        pool.grow("http://localhost:50051").unwrap();
        assert_eq!(pool.size(), 3);
        assert!(pool.is_full());
        assert!(pool.grow("http://localhost:50051").is_none());
    }

    // 28. Metrics track checkout and health-check failures
    #[test]
    fn pool_metrics_tracking() {
        let keepalive = KeepaliveConfig {
            max_missed_pings: 1,
            ..KeepaliveConfig::default()
        };
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            2,
            4,
            keepalive,
            SelectionStrategy::RoundRobin,
        );

        // 3 checkouts
        for _ in 0..3 {
            if let Some(id) = pool.checkout() {
                pool.checkin(id);
            }
        }

        // 1 failed health check
        pool.health_check(100, &[]);

        let snap = pool.metrics().snapshot();
        assert_eq!(snap.total_requests, 3);
        assert!(snap.failed_health_checks >= 2); // two connections failed
        assert_eq!(snap.active_connections, 2);
    }

    // 29. Evict unhealthy returns empty when all healthy
    #[test]
    fn evict_unhealthy_noop_when_healthy() {
        let mut pool = ClientConnectionPool::new(
            "http://localhost:50051",
            2,
            4,
            KeepaliveConfig::default(),
            SelectionStrategy::RoundRobin,
        );
        let evicted = pool.evict_unhealthy();
        assert!(evicted.is_empty());
        assert_eq!(pool.size(), 2);
    }
}
