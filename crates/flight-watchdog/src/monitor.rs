// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! RT spine heartbeat monitoring, adapter health tracking, and escalation engine.
//!
//! Implements a multi-component health monitor with an escalation chain:
//! **Normal → Warning → Degraded → SafeMode**.
//!
//! The [`SystemMonitor`] tracks:
//! - RT spine heartbeat (250 Hz tick liveness)
//! - Simulator adapter connection health
//! - Memory / allocation budget for RT paths
//! - Device enumeration freshness

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

// ── System-wide operating mode ──────────────────────────────────────────────

/// The overall operating mode of the flight system, driven by health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemMode {
    /// All subsystems operating within tolerances.
    Normal,
    /// One or more subsystems have emitted warnings but remain functional.
    Warning,
    /// Sustained failures detected; non-essential subsystems disabled.
    Degraded,
    /// Critical failure; only safety-critical paths remain active.
    SafeMode,
}

impl std::fmt::Display for SystemMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemMode::Normal => write!(f, "Normal"),
            SystemMode::Warning => write!(f, "Warning"),
            SystemMode::Degraded => write!(f, "Degraded"),
            SystemMode::SafeMode => write!(f, "SafeMode"),
        }
    }
}

// ── Structured health events ────────────────────────────────────────────────

/// A structured event emitted by the monitor for consumption by the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthEvent {
    /// Which subsystem produced the event.
    pub source: String,
    /// Severity level.
    pub severity: Severity,
    /// Machine-readable event kind.
    pub kind: HealthEventKind,
    /// Human-readable description.
    pub message: String,
    /// System mode *after* this event was processed.
    pub resulting_mode: SystemMode,
}

/// Severity levels for health events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Machine-readable health event kinds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthEventKind {
    /// A single heartbeat tick was missed.
    HeartbeatMissed,
    /// Heartbeat recovered after misses.
    HeartbeatRecovered,
    /// System entered degraded mode.
    DegradedModeEntered,
    /// System entered safe mode.
    SafeModeEntered,
    /// System returned to normal operation.
    NormalModeRestored,
    /// Adapter connection lost.
    AdapterDisconnected,
    /// Adapter connection restored.
    AdapterReconnected,
    /// Memory budget exceeded on an RT path.
    MemoryBudgetExceeded,
    /// Device enumeration is stale.
    DeviceEnumerationStale,
    /// Device enumeration refreshed.
    DeviceEnumerationRefreshed,
    /// System mode changed.
    ModeTransition,
}

// ── Configuration ───────────────────────────────────────────────────────────

/// Configuration for the system monitor.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Consecutive missed ticks before entering Warning.
    pub warn_after_missed_ticks: u32,
    /// Consecutive missed ticks before entering Degraded.
    pub degrade_after_missed_ticks: u32,
    /// Consecutive missed ticks before entering SafeMode.
    pub safe_mode_after_missed_ticks: u32,
    /// Expected interval between heartbeat ticks (4 ms for 250 Hz).
    pub expected_tick_interval: Duration,
    /// Grace period: a tick is "missed" when this multiple of the interval elapses.
    pub tick_timeout_multiplier: f64,
    /// How long before device enumeration is considered stale.
    pub device_enum_staleness: Duration,
    /// Memory budget (bytes) for RT paths; 0 means unlimited.
    pub rt_memory_budget_bytes: u64,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            warn_after_missed_ticks: 1,
            degrade_after_missed_ticks: 5,
            safe_mode_after_missed_ticks: 20,
            expected_tick_interval: Duration::from_millis(4), // 250 Hz
            tick_timeout_multiplier: 2.0,
            device_enum_staleness: Duration::from_secs(30),
            rt_memory_budget_bytes: 0,
        }
    }
}

// ── Adapter state ───────────────────────────────────────────────────────────

/// Connection state for a simulator adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterState {
    Connected,
    Disconnected { since: Duration },
}

#[derive(Debug)]
struct AdapterEntry {
    connected: bool,
    last_seen: Instant,
    /// Number of consecutive disconnected observations.
    consecutive_disconnects: u32,
}

// ── Device enumeration state ────────────────────────────────────────────────

#[derive(Debug)]
struct DeviceEnumEntry {
    last_refresh: Instant,
}

// ── Main monitor ────────────────────────────────────────────────────────────

/// Central health monitor for the flight system.
///
/// Tracks multiple subsystems and maintains an escalation chain that drives
/// the overall [`SystemMode`].
#[derive(Debug)]
pub struct SystemMonitor {
    config: MonitorConfig,
    mode: SystemMode,

    // Heartbeat tracking
    last_heartbeat: Option<Instant>,
    consecutive_missed_ticks: u32,
    total_missed_ticks: u64,
    total_received_ticks: u64,

    // Adapter tracking
    adapters: HashMap<String, AdapterEntry>,

    // Memory tracking
    current_rt_memory_bytes: u64,
    peak_rt_memory_bytes: u64,

    // Device enumeration tracking
    device_enums: HashMap<String, DeviceEnumEntry>,

    // Event log
    events: Vec<HealthEvent>,
    max_events: usize,
}

impl SystemMonitor {
    /// Create a new monitor with the given configuration.
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            mode: SystemMode::Normal,
            last_heartbeat: None,
            consecutive_missed_ticks: 0,
            total_missed_ticks: 0,
            total_received_ticks: 0,
            adapters: HashMap::new(),
            current_rt_memory_bytes: 0,
            peak_rt_memory_bytes: 0,
            device_enums: HashMap::new(),
            events: Vec::new(),
            max_events: 10_000,
        }
    }

    // ── Heartbeat ───────────────────────────────────────────────────────

    /// Record a successful RT spine heartbeat tick.
    ///
    /// If the monitor was previously in an elevated mode solely due to missed
    /// heartbeats, it will attempt to de-escalate.
    pub fn record_heartbeat(&mut self) {
        let now = Instant::now();
        self.last_heartbeat = Some(now);
        self.total_received_ticks += 1;

        if self.consecutive_missed_ticks > 0 {
            let prev_misses = self.consecutive_missed_ticks;
            self.consecutive_missed_ticks = 0;

            info!(
                recovered_after = prev_misses,
                "RT heartbeat recovered after {} missed tick(s)",
                prev_misses
            );

            self.emit_event(HealthEvent {
                source: "rt_spine".into(),
                severity: Severity::Info,
                kind: HealthEventKind::HeartbeatRecovered,
                message: format!("Heartbeat recovered after {prev_misses} missed tick(s)"),
                resulting_mode: self.mode,
            });

            self.try_deescalate();
        }
    }

    /// Notify the monitor that a heartbeat tick was expected but not received.
    ///
    /// This drives the escalation chain: Warning → Degraded → SafeMode.
    pub fn record_missed_tick(&mut self) {
        self.consecutive_missed_ticks += 1;
        self.total_missed_ticks += 1;
        let n = self.consecutive_missed_ticks;

        if n >= self.config.safe_mode_after_missed_ticks {
            if self.mode != SystemMode::SafeMode {
                error!(
                    consecutive = n,
                    "RT spine: {n} consecutive missed ticks — entering SafeMode"
                );
                self.transition_to(SystemMode::SafeMode, "rt_spine", &format!(
                    "{n} consecutive missed ticks exceeded safe-mode threshold ({})",
                    self.config.safe_mode_after_missed_ticks
                ));
            }
        } else if n >= self.config.degrade_after_missed_ticks {
            if self.mode != SystemMode::Degraded && self.mode != SystemMode::SafeMode {
                warn!(
                    consecutive = n,
                    "RT spine: {n} consecutive missed ticks — entering Degraded mode"
                );
                self.transition_to(SystemMode::Degraded, "rt_spine", &format!(
                    "{n} consecutive missed ticks exceeded degraded threshold ({})",
                    self.config.degrade_after_missed_ticks
                ));
            }
        } else if n >= self.config.warn_after_missed_ticks {
            if self.mode == SystemMode::Normal {
                warn!(consecutive = n, "RT spine: missed tick #{n}");
                self.transition_to(SystemMode::Warning, "rt_spine", &format!(
                    "Missed tick #{n} (warn threshold: {})",
                    self.config.warn_after_missed_ticks
                ));
            }
            // Always emit the missed-tick event for observability.
            self.emit_event(HealthEvent {
                source: "rt_spine".into(),
                severity: Severity::Warning,
                kind: HealthEventKind::HeartbeatMissed,
                message: format!("Missed tick #{n}"),
                resulting_mode: self.mode,
            });
        }
    }

    /// Check whether the last heartbeat is overdue based on the configured
    /// interval and timeout multiplier.  Returns `true` if a tick was missed.
    pub fn check_heartbeat_timeout(&mut self) -> bool {
        let deadline = Duration::from_secs_f64(
            self.config.expected_tick_interval.as_secs_f64()
                * self.config.tick_timeout_multiplier,
        );

        if let Some(last) = self.last_heartbeat
            && last.elapsed() > deadline
        {
            self.record_missed_tick();
            return true;
        }
        false
    }

    // ── Adapter connection health ───────────────────────────────────────

    /// Register a simulator adapter for health tracking.
    pub fn register_adapter(&mut self, name: &str) {
        debug!(adapter = name, "Registering adapter for health monitoring");
        self.adapters.insert(
            name.to_string(),
            AdapterEntry {
                connected: true,
                last_seen: Instant::now(),
                consecutive_disconnects: 0,
            },
        );
    }

    /// Report that an adapter is connected and responsive.
    pub fn report_adapter_connected(&mut self, name: &str) {
        if let Some(entry) = self.adapters.get_mut(name) {
            let was_disconnected = !entry.connected;
            entry.connected = true;
            entry.last_seen = Instant::now();
            entry.consecutive_disconnects = 0;

            if was_disconnected {
                info!(adapter = name, "Adapter reconnected");
                self.emit_event(HealthEvent {
                    source: format!("adapter:{name}"),
                    severity: Severity::Info,
                    kind: HealthEventKind::AdapterReconnected,
                    message: format!("Adapter '{name}' reconnected"),
                    resulting_mode: self.mode,
                });
                self.try_deescalate();
            }
        }
    }

    /// Report that an adapter has disconnected or failed a health check.
    pub fn report_adapter_disconnected(&mut self, name: &str) {
        let consecutive = if let Some(entry) = self.adapters.get_mut(name) {
            entry.connected = false;
            entry.consecutive_disconnects += 1;
            entry.consecutive_disconnects
        } else {
            return;
        };

        warn!(
            adapter = name,
            consecutive = consecutive,
            "Adapter disconnected"
        );

        self.emit_event(HealthEvent {
            source: format!("adapter:{name}"),
            severity: Severity::Warning,
            kind: HealthEventKind::AdapterDisconnected,
            message: format!("Adapter '{name}' disconnected ({consecutive} consecutive)"),
            resulting_mode: self.mode,
        });

        if self.mode == SystemMode::Normal {
            self.transition_to(
                SystemMode::Warning,
                &format!("adapter:{name}"),
                "Adapter disconnected",
            );
        }
    }

    /// Query the connection state of a registered adapter.
    pub fn adapter_state(&self, name: &str) -> Option<AdapterState> {
        self.adapters.get(name).map(|e| {
            if e.connected {
                AdapterState::Connected
            } else {
                AdapterState::Disconnected {
                    since: e.last_seen.elapsed(),
                }
            }
        })
    }

    // ── Memory / allocation tracking ────────────────────────────────────

    /// Report the current RT-path memory usage in bytes.
    ///
    /// If the reported value exceeds the configured budget an event is emitted
    /// and the system escalates to at least Warning.
    pub fn report_rt_memory(&mut self, bytes: u64) {
        self.current_rt_memory_bytes = bytes;
        if bytes > self.peak_rt_memory_bytes {
            self.peak_rt_memory_bytes = bytes;
        }

        let budget = self.config.rt_memory_budget_bytes;
        if budget > 0 && bytes > budget {
            warn!(
                current = bytes,
                budget = budget,
                "RT memory budget exceeded"
            );
            self.emit_event(HealthEvent {
                source: "rt_memory".into(),
                severity: Severity::Warning,
                kind: HealthEventKind::MemoryBudgetExceeded,
                message: format!(
                    "RT memory {bytes} bytes exceeds budget of {budget} bytes"
                ),
                resulting_mode: self.mode,
            });

            if self.mode == SystemMode::Normal {
                self.transition_to(
                    SystemMode::Warning,
                    "rt_memory",
                    &format!("Memory {bytes}B exceeds budget {budget}B"),
                );
            }
        }
    }

    /// Current RT memory usage in bytes.
    pub fn current_rt_memory(&self) -> u64 {
        self.current_rt_memory_bytes
    }

    /// Peak RT memory usage observed since monitor creation.
    pub fn peak_rt_memory(&self) -> u64 {
        self.peak_rt_memory_bytes
    }

    // ── Device enumeration freshness ────────────────────────────────────

    /// Record that a device class was just enumerated.
    pub fn record_device_enumeration(&mut self, device_class: &str) {
        debug!(device_class, "Device enumeration refreshed");
        self.device_enums.insert(
            device_class.to_string(),
            DeviceEnumEntry {
                last_refresh: Instant::now(),
            },
        );
    }

    /// Check whether a device class enumeration is stale.
    pub fn is_device_enum_stale(&self, device_class: &str) -> bool {
        match self.device_enums.get(device_class) {
            Some(entry) => entry.last_refresh.elapsed() > self.config.device_enum_staleness,
            None => false, // not tracked → not stale
        }
    }

    /// Scan all tracked device classes and emit events for any that are stale.
    pub fn check_device_enumerations(&mut self) {
        let staleness = self.config.device_enum_staleness;
        let stale: Vec<String> = self
            .device_enums
            .iter()
            .filter(|(_, e)| e.last_refresh.elapsed() > staleness)
            .map(|(k, _)| k.clone())
            .collect();

        for name in stale {
            warn!(device_class = %name, "Device enumeration stale");
            self.emit_event(HealthEvent {
                source: format!("device_enum:{name}"),
                severity: Severity::Warning,
                kind: HealthEventKind::DeviceEnumerationStale,
                message: format!(
                    "Device class '{name}' enumeration older than {:?}",
                    staleness
                ),
                resulting_mode: self.mode,
            });
        }
    }

    // ── Mode management ─────────────────────────────────────────────────

    /// Current system operating mode.
    pub fn mode(&self) -> SystemMode {
        self.mode
    }

    /// Force the system into a specific mode (e.g. for external safe-mode triggers).
    pub fn force_mode(&mut self, mode: SystemMode) {
        if self.mode != mode {
            self.transition_to(mode, "external", "Forced mode transition");
        }
    }

    /// Number of consecutive heartbeat misses at this instant.
    pub fn consecutive_missed_ticks(&self) -> u32 {
        self.consecutive_missed_ticks
    }

    /// Lifetime total of missed ticks.
    pub fn total_missed_ticks(&self) -> u64 {
        self.total_missed_ticks
    }

    /// Lifetime total of received ticks.
    pub fn total_received_ticks(&self) -> u64 {
        self.total_received_ticks
    }

    // ── Events ──────────────────────────────────────────────────────────

    /// All health events emitted since creation (bounded by `max_events`).
    pub fn events(&self) -> &[HealthEvent] {
        &self.events
    }

    /// Drain and return all pending health events.
    pub fn drain_events(&mut self) -> Vec<HealthEvent> {
        std::mem::take(&mut self.events)
    }

    // ── Snapshot ─────────────────────────────────────────────────────────

    /// Build a point-in-time health snapshot.
    pub fn snapshot(&self) -> MonitorSnapshot {
        let connected_adapters = self.adapters.values().filter(|a| a.connected).count();
        let disconnected_adapters = self.adapters.values().filter(|a| !a.connected).count();

        MonitorSnapshot {
            mode: self.mode,
            consecutive_missed_ticks: self.consecutive_missed_ticks,
            total_missed_ticks: self.total_missed_ticks,
            total_received_ticks: self.total_received_ticks,
            connected_adapters,
            disconnected_adapters,
            current_rt_memory_bytes: self.current_rt_memory_bytes,
            peak_rt_memory_bytes: self.peak_rt_memory_bytes,
            event_count: self.events.len(),
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────

    fn transition_to(&mut self, new_mode: SystemMode, source: &str, reason: &str) {
        let old_mode = self.mode;
        self.mode = new_mode;

        let severity = match new_mode {
            SystemMode::Normal => Severity::Info,
            SystemMode::Warning => Severity::Warning,
            SystemMode::Degraded => Severity::Error,
            SystemMode::SafeMode => Severity::Critical,
        };

        let kind = match new_mode {
            SystemMode::Normal => HealthEventKind::NormalModeRestored,
            SystemMode::Degraded => HealthEventKind::DegradedModeEntered,
            SystemMode::SafeMode => HealthEventKind::SafeModeEntered,
            SystemMode::Warning => HealthEventKind::ModeTransition,
        };

        info!(
            from = %old_mode,
            to = %new_mode,
            source,
            reason,
            "System mode transition"
        );

        self.emit_event(HealthEvent {
            source: source.into(),
            severity,
            kind,
            message: format!("{old_mode} → {new_mode}: {reason}"),
            resulting_mode: new_mode,
        });
    }

    /// Attempt to lower the system mode when conditions improve.
    fn try_deescalate(&mut self) {
        if self.consecutive_missed_ticks > 0 {
            return; // still have heartbeat issues
        }

        let any_adapter_down = self.adapters.values().any(|a| !a.connected);
        let budget = self.config.rt_memory_budget_bytes;
        let over_budget = budget > 0 && self.current_rt_memory_bytes > budget;

        let target = if any_adapter_down || over_budget {
            SystemMode::Warning
        } else {
            SystemMode::Normal
        };

        if target < self.mode {
            self.transition_to(target, "auto_deescalate", "Conditions improved");
        }
    }

    fn emit_event(&mut self, event: HealthEvent) {
        if self.events.len() >= self.max_events {
            self.events.remove(0);
        }
        self.events.push(event);
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new(MonitorConfig::default())
    }
}

// ── Snapshot ─────────────────────────────────────────────────────────────────

/// Point-in-time snapshot of monitor state, safe to serialize / send over IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    pub mode: SystemMode,
    pub consecutive_missed_ticks: u32,
    pub total_missed_ticks: u64,
    pub total_received_ticks: u64,
    pub connected_adapters: usize,
    pub disconnected_adapters: usize,
    pub current_rt_memory_bytes: u64,
    pub peak_rt_memory_bytes: u64,
    pub event_count: usize,
}

// ── Implement Ord for SystemMode to allow comparison ────────────────────────

impl PartialOrd for SystemMode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SystemMode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn rank(m: &SystemMode) -> u8 {
            match m {
                SystemMode::Normal => 0,
                SystemMode::Warning => 1,
                SystemMode::Degraded => 2,
                SystemMode::SafeMode => 3,
            }
        }
        rank(self).cmp(&rank(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper ──────────────────────────────────────────────────────────

    fn default_monitor() -> SystemMonitor {
        SystemMonitor::new(MonitorConfig::default())
    }

    fn custom_monitor(warn: u32, degrade: u32, safe: u32) -> SystemMonitor {
        SystemMonitor::new(MonitorConfig {
            warn_after_missed_ticks: warn,
            degrade_after_missed_ticks: degrade,
            safe_mode_after_missed_ticks: safe,
            ..MonitorConfig::default()
        })
    }

    // ── Normal heartbeat pattern ────────────────────────────────────────

    #[test]
    fn normal_heartbeat_stays_in_normal_mode() {
        let mut mon = default_monitor();
        for _ in 0..100 {
            mon.record_heartbeat();
        }
        assert_eq!(mon.mode(), SystemMode::Normal);
        assert_eq!(mon.consecutive_missed_ticks(), 0);
        assert_eq!(mon.total_received_ticks(), 100);
        assert_eq!(mon.total_missed_ticks(), 0);
    }

    #[test]
    fn heartbeat_emits_no_events_when_healthy() {
        let mut mon = default_monitor();
        for _ in 0..10 {
            mon.record_heartbeat();
        }
        // No warning/error events should have been emitted.
        let non_info: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.severity > Severity::Info)
            .collect();
        assert!(non_info.is_empty(), "healthy heartbeats should not emit warnings");
    }

    // ── Missed tick detection ───────────────────────────────────────────

    #[test]
    fn single_missed_tick_transitions_to_warning() {
        let mut mon = custom_monitor(1, 5, 20);
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::Warning);
        assert_eq!(mon.consecutive_missed_ticks(), 1);
        assert_eq!(mon.total_missed_ticks(), 1);
    }

    #[test]
    fn missed_tick_event_is_emitted() {
        let mut mon = custom_monitor(1, 5, 20);
        mon.record_missed_tick();
        let missed_events: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.kind == HealthEventKind::HeartbeatMissed)
            .collect();
        assert!(!missed_events.is_empty(), "should emit HeartbeatMissed event");
    }

    // ── Escalation chain: Warning → Degraded → SafeMode ────────────────

    #[test]
    fn escalation_warning_to_degraded() {
        let mut mon = custom_monitor(1, 3, 10);
        // 1 miss → Warning
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::Warning);
        // 2 more misses → Degraded at 3
        mon.record_missed_tick();
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::Degraded);
    }

    #[test]
    fn escalation_degraded_to_safe_mode() {
        let mut mon = custom_monitor(1, 3, 6);
        for _ in 0..6 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::SafeMode);
    }

    #[test]
    fn full_escalation_chain() {
        let mut mon = custom_monitor(1, 3, 5);
        assert_eq!(mon.mode(), SystemMode::Normal);

        mon.record_missed_tick(); // 1 → Warning
        assert_eq!(mon.mode(), SystemMode::Warning);

        mon.record_missed_tick(); // 2 → still Warning
        assert_eq!(mon.mode(), SystemMode::Warning);

        mon.record_missed_tick(); // 3 → Degraded
        assert_eq!(mon.mode(), SystemMode::Degraded);

        mon.record_missed_tick(); // 4 → still Degraded
        assert_eq!(mon.mode(), SystemMode::Degraded);

        mon.record_missed_tick(); // 5 → SafeMode
        assert_eq!(mon.mode(), SystemMode::SafeMode);
    }

    #[test]
    fn safe_mode_is_sticky_until_recovery() {
        let mut mon = custom_monitor(1, 3, 5);
        for _ in 0..5 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::SafeMode);

        // More misses don't change mode.
        for _ in 0..10 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::SafeMode);
    }

    #[test]
    fn escalation_emits_structured_events() {
        let mut mon = custom_monitor(1, 3, 5);
        for _ in 0..5 {
            mon.record_missed_tick();
        }

        let kinds: Vec<_> = mon.events().iter().map(|e| e.kind.clone()).collect();
        assert!(
            kinds.contains(&HealthEventKind::ModeTransition),
            "should have Warning mode transition"
        );
        assert!(
            kinds.contains(&HealthEventKind::DegradedModeEntered),
            "should have Degraded event"
        );
        assert!(
            kinds.contains(&HealthEventKind::SafeModeEntered),
            "should have SafeMode event"
        );
    }

    // ── Recovery after temporary issues ─────────────────────────────────

    #[test]
    fn heartbeat_recovery_from_warning() {
        let mut mon = custom_monitor(1, 5, 20);
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::Warning);

        mon.record_heartbeat();
        assert_eq!(mon.mode(), SystemMode::Normal);
        assert_eq!(mon.consecutive_missed_ticks(), 0);
    }

    #[test]
    fn heartbeat_recovery_from_degraded() {
        let mut mon = custom_monitor(1, 3, 20);
        for _ in 0..3 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::Degraded);

        mon.record_heartbeat();
        assert_eq!(mon.mode(), SystemMode::Normal);
    }

    #[test]
    fn heartbeat_recovery_from_safe_mode() {
        let mut mon = custom_monitor(1, 3, 5);
        for _ in 0..5 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::SafeMode);

        mon.record_heartbeat();
        assert_eq!(mon.mode(), SystemMode::Normal);
        assert_eq!(mon.consecutive_missed_ticks(), 0);
    }

    #[test]
    fn recovery_emits_event() {
        let mut mon = custom_monitor(1, 5, 20);
        mon.record_missed_tick();
        mon.record_heartbeat();

        let recovered: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.kind == HealthEventKind::HeartbeatRecovered)
            .collect();
        assert_eq!(recovered.len(), 1);

        let restored: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.kind == HealthEventKind::NormalModeRestored)
            .collect();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn intermittent_misses_do_not_escalate() {
        let mut mon = custom_monitor(1, 3, 5);
        // miss - recover - miss - recover pattern
        for _ in 0..10 {
            mon.record_missed_tick();
            assert!(mon.mode() <= SystemMode::Warning);
            mon.record_heartbeat();
            assert_eq!(mon.mode(), SystemMode::Normal);
        }
    }

    // ── Adapter connection health ───────────────────────────────────────

    #[test]
    fn adapter_starts_connected() {
        let mut mon = default_monitor();
        mon.register_adapter("msfs");
        assert_eq!(mon.adapter_state("msfs"), Some(AdapterState::Connected));
    }

    #[test]
    fn adapter_disconnect_escalates_to_warning() {
        let mut mon = default_monitor();
        mon.register_adapter("xplane");
        mon.report_adapter_disconnected("xplane");
        assert_eq!(mon.mode(), SystemMode::Warning);
        assert!(matches!(
            mon.adapter_state("xplane"),
            Some(AdapterState::Disconnected { .. })
        ));
    }

    #[test]
    fn adapter_reconnect_deescalates() {
        let mut mon = default_monitor();
        mon.register_adapter("dcs");
        mon.report_adapter_disconnected("dcs");
        assert_eq!(mon.mode(), SystemMode::Warning);

        mon.report_adapter_connected("dcs");
        assert_eq!(mon.mode(), SystemMode::Normal);
        assert_eq!(mon.adapter_state("dcs"), Some(AdapterState::Connected));
    }

    #[test]
    fn unknown_adapter_is_none() {
        let mon = default_monitor();
        assert_eq!(mon.adapter_state("nonexistent"), None);
    }

    // ── Memory / allocation tracking ────────────────────────────────────

    #[test]
    fn memory_under_budget_stays_normal() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            rt_memory_budget_bytes: 1024,
            ..MonitorConfig::default()
        });
        mon.report_rt_memory(512);
        assert_eq!(mon.mode(), SystemMode::Normal);
        assert_eq!(mon.current_rt_memory(), 512);
        assert_eq!(mon.peak_rt_memory(), 512);
    }

    #[test]
    fn memory_over_budget_escalates_to_warning() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            rt_memory_budget_bytes: 1024,
            ..MonitorConfig::default()
        });
        mon.report_rt_memory(2048);
        assert_eq!(mon.mode(), SystemMode::Warning);

        let mem_events: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.kind == HealthEventKind::MemoryBudgetExceeded)
            .collect();
        assert_eq!(mem_events.len(), 1);
    }

    #[test]
    fn peak_memory_tracks_maximum() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            rt_memory_budget_bytes: 0, // unlimited
            ..MonitorConfig::default()
        });
        mon.report_rt_memory(100);
        mon.report_rt_memory(500);
        mon.report_rt_memory(200);
        assert_eq!(mon.peak_rt_memory(), 500);
        assert_eq!(mon.current_rt_memory(), 200);
    }

    #[test]
    fn zero_budget_means_unlimited() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            rt_memory_budget_bytes: 0,
            ..MonitorConfig::default()
        });
        mon.report_rt_memory(u64::MAX);
        assert_eq!(mon.mode(), SystemMode::Normal);
    }

    // ── Device enumeration freshness ────────────────────────────────────

    #[test]
    fn fresh_enumeration_is_not_stale() {
        let mut mon = default_monitor();
        mon.record_device_enumeration("hid_joystick");
        assert!(!mon.is_device_enum_stale("hid_joystick"));
    }

    #[test]
    fn untracked_device_is_not_stale() {
        let mon = default_monitor();
        assert!(!mon.is_device_enum_stale("unknown_class"));
    }

    #[test]
    fn stale_enumeration_detected_with_short_threshold() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            device_enum_staleness: Duration::from_millis(1),
            ..MonitorConfig::default()
        });
        mon.record_device_enumeration("hid_panels");
        std::thread::sleep(Duration::from_millis(5));
        assert!(mon.is_device_enum_stale("hid_panels"));
    }

    #[test]
    fn check_device_enumerations_emits_stale_events() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            device_enum_staleness: Duration::from_millis(1),
            ..MonitorConfig::default()
        });
        mon.record_device_enumeration("hid_panels");
        std::thread::sleep(Duration::from_millis(5));
        mon.check_device_enumerations();

        let stale_events: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.kind == HealthEventKind::DeviceEnumerationStale)
            .collect();
        assert_eq!(stale_events.len(), 1);
    }

    // ── Concurrent monitoring of multiple components ────────────────────

    #[test]
    fn multiple_subsystems_monitored_concurrently() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            warn_after_missed_ticks: 1,
            degrade_after_missed_ticks: 5,
            safe_mode_after_missed_ticks: 20,
            rt_memory_budget_bytes: 4096,
            device_enum_staleness: Duration::from_secs(30),
            ..MonitorConfig::default()
        });

        // Register everything.
        mon.register_adapter("msfs");
        mon.register_adapter("xplane");
        mon.record_device_enumeration("hid_joystick");
        mon.record_device_enumeration("hid_panels");

        // Normal operation.
        for _ in 0..50 {
            mon.record_heartbeat();
        }
        mon.report_rt_memory(1024);
        assert_eq!(mon.mode(), SystemMode::Normal);

        // Adapter drops.
        mon.report_adapter_disconnected("msfs");
        assert_eq!(mon.mode(), SystemMode::Warning);

        // Heartbeats still fine.
        mon.record_heartbeat();
        // Mode stays Warning because adapter is still down.
        assert_eq!(mon.mode(), SystemMode::Warning);

        // Adapter recovers.
        mon.report_adapter_connected("msfs");
        assert_eq!(mon.mode(), SystemMode::Normal);

        // Memory spike.
        mon.report_rt_memory(8192);
        assert_eq!(mon.mode(), SystemMode::Warning);

        // Memory drops.
        mon.report_rt_memory(2048);
        // Still warning until a deescalation trigger fires.
        // Record heartbeat to trigger deescalation check (heartbeat recovery path).
        // Since heartbeat was never missed, manually check via another path.
        // The memory being under budget will not auto-deescalate without a recovery trigger.
        // Report memory under budget and call record_heartbeat with a "missed then recovered" cycle.
        mon.report_rt_memory(1024);
        // Force deescalation by recording a missed tick then recovery.
        mon.record_missed_tick();
        mon.record_heartbeat();
        assert_eq!(mon.mode(), SystemMode::Normal);

        let snap = mon.snapshot();
        assert_eq!(snap.connected_adapters, 2);
        assert_eq!(snap.disconnected_adapters, 0);
    }

    #[test]
    fn adapter_down_prevents_full_deescalation() {
        let mut mon = custom_monitor(1, 5, 20);
        mon.register_adapter("msfs");

        // Miss a tick → Warning.
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::Warning);

        // Disconnect adapter.
        mon.report_adapter_disconnected("msfs");

        // Heartbeat recovers, but adapter is still down → stays Warning.
        mon.record_heartbeat();
        assert_eq!(mon.mode(), SystemMode::Warning);

        // Adapter reconnects → can now go Normal.
        mon.report_adapter_connected("msfs");
        assert_eq!(mon.mode(), SystemMode::Normal);
    }

    // ── Snapshot ────────────────────────────────────────────────────────

    #[test]
    fn snapshot_reflects_current_state() {
        let mut mon = custom_monitor(1, 3, 5);
        mon.register_adapter("a");
        mon.register_adapter("b");
        mon.report_adapter_disconnected("b");

        mon.record_missed_tick();
        mon.record_missed_tick();
        mon.record_heartbeat();
        mon.record_heartbeat();
        mon.record_heartbeat();

        let snap = mon.snapshot();
        assert_eq!(snap.mode, SystemMode::Warning); // adapter still down
        assert_eq!(snap.consecutive_missed_ticks, 0);
        assert_eq!(snap.total_missed_ticks, 2);
        assert_eq!(snap.total_received_ticks, 3);
        assert_eq!(snap.connected_adapters, 1);
        assert_eq!(snap.disconnected_adapters, 1);
    }

    // ── Drain events ───────────────────────────────────────────────────

    #[test]
    fn drain_events_clears_buffer() {
        let mut mon = custom_monitor(1, 5, 20);
        mon.record_missed_tick();
        assert!(!mon.events().is_empty());

        let drained = mon.drain_events();
        assert!(!drained.is_empty());
        assert!(mon.events().is_empty());
    }

    // ── Force mode ──────────────────────────────────────────────────────

    #[test]
    fn force_mode_overrides_current() {
        let mut mon = default_monitor();
        mon.force_mode(SystemMode::SafeMode);
        assert_eq!(mon.mode(), SystemMode::SafeMode);
    }

    #[test]
    fn force_same_mode_is_noop() {
        let mut mon = default_monitor();
        let events_before = mon.events().len();
        mon.force_mode(SystemMode::Normal);
        assert_eq!(mon.events().len(), events_before);
    }

    // ── SystemMode ordering ─────────────────────────────────────────────

    #[test]
    fn system_mode_ordering() {
        assert!(SystemMode::Normal < SystemMode::Warning);
        assert!(SystemMode::Warning < SystemMode::Degraded);
        assert!(SystemMode::Degraded < SystemMode::SafeMode);
    }
}
