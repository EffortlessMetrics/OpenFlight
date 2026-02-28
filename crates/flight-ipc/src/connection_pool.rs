// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Connection pool for managing multiple concurrent IPC client sessions.
//!
//! [`ConnectionPool`] tracks connected clients, enforces a maximum connection
//! limit, and prunes idle connections that have exceeded a configurable timeout.

use std::collections::HashMap;
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
