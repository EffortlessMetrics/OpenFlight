// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Hardware watchdog supervisor, process self-monitoring, and dead-man's switch.
//!
//! Provides:
//! - [`HardwareWatchdog`]: supervisor-pattern watchdog timer that must be
//!   periodically petted or it triggers recovery actions.
//! - [`ProcessMonitor`]: self-monitoring of memory, CPU, and thread count.
//! - [`DeadManSwitch`]: axis-engine tick liveness detector.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

// ── Hardware watchdog timer (supervisor pattern) ────────────────────────────

/// Configuration for the hardware watchdog timer.
#[derive(Debug, Clone)]
pub struct WatchdogTimerConfig {
    /// How often the watchdog must be petted to avoid triggering.
    pub timeout: Duration,
    /// Number of consecutive timeouts before taking recovery action.
    pub max_timeouts: u32,
}

impl Default for WatchdogTimerConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            max_timeouts: 3,
        }
    }
}

/// Result of a watchdog timer check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchdogTimerStatus {
    /// Timer is healthy, last pet was within timeout.
    Ok,
    /// Timer has expired once — warning.
    Warning { missed_count: u32 },
    /// Timer has expired beyond max_timeouts — recovery needed.
    Expired { missed_count: u32 },
}

/// A supervisor-pattern hardware watchdog timer.
///
/// The supervised component must periodically call [`pet`] to prevent
/// the timer from expiring. The supervisor checks the timer with [`check`].
pub struct HardwareWatchdog {
    config: WatchdogTimerConfig,
    last_pet: Instant,
    consecutive_timeouts: u32,
    total_timeouts: u64,
    enabled: bool,
}

impl HardwareWatchdog {
    /// Create a new watchdog timer with the given configuration.
    pub fn new(config: WatchdogTimerConfig) -> Self {
        Self {
            config,
            last_pet: Instant::now(),
            consecutive_timeouts: 0,
            total_timeouts: 0,
            enabled: true,
        }
    }

    /// Pet (kick) the watchdog to indicate the supervised component is alive.
    pub fn pet(&mut self) {
        self.last_pet = Instant::now();
        self.consecutive_timeouts = 0;
    }

    /// Check the watchdog timer and return its current status.
    pub fn check(&mut self) -> WatchdogTimerStatus {
        if !self.enabled {
            return WatchdogTimerStatus::Ok;
        }

        if self.last_pet.elapsed() > self.config.timeout {
            self.consecutive_timeouts += 1;
            self.total_timeouts += 1;

            if self.consecutive_timeouts >= self.config.max_timeouts {
                WatchdogTimerStatus::Expired {
                    missed_count: self.consecutive_timeouts,
                }
            } else {
                WatchdogTimerStatus::Warning {
                    missed_count: self.consecutive_timeouts,
                }
            }
        } else {
            WatchdogTimerStatus::Ok
        }
    }

    /// Enable or disable the watchdog timer.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.last_pet = Instant::now();
        }
    }

    /// Whether the watchdog is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Total number of timeouts since creation.
    pub fn total_timeouts(&self) -> u64 {
        self.total_timeouts
    }

    /// Current consecutive timeout count.
    pub fn consecutive_timeouts(&self) -> u32 {
        self.consecutive_timeouts
    }

    /// Duration since the last pet.
    pub fn elapsed_since_pet(&self) -> Duration {
        self.last_pet.elapsed()
    }
}

impl Default for HardwareWatchdog {
    fn default() -> Self {
        Self::new(WatchdogTimerConfig::default())
    }
}

// ── Process self-monitoring ────────────────────────────────────────────────

/// Thresholds for process self-monitoring.
#[derive(Debug, Clone)]
pub struct ProcessMonitorConfig {
    /// Maximum memory usage in bytes before warning.
    pub memory_warn_bytes: u64,
    /// Maximum memory usage in bytes before critical alert.
    pub memory_critical_bytes: u64,
    /// Maximum thread count before warning.
    pub thread_warn_count: u32,
    /// Maximum thread count before critical alert.
    pub thread_critical_count: u32,
}

impl Default for ProcessMonitorConfig {
    fn default() -> Self {
        Self {
            memory_warn_bytes: 512 * 1024 * 1024,      // 512 MB
            memory_critical_bytes: 1024 * 1024 * 1024, // 1 GB
            thread_warn_count: 100,
            thread_critical_count: 500,
        }
    }
}

/// Current process resource snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    /// Estimated memory usage in bytes.
    pub memory_bytes: u64,
    /// Number of active threads.
    pub thread_count: u32,
    /// Process uptime.
    pub uptime: Duration,
}

/// Severity of a process monitor finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessAlert {
    /// All resources within normal bounds.
    Normal,
    /// One or more resources approaching limits.
    Warning,
    /// One or more resources at critical levels.
    Critical,
}

/// Alert detail from process monitoring.
#[derive(Debug, Clone)]
pub struct ProcessAlertDetail {
    pub severity: ProcessAlert,
    pub messages: Vec<String>,
}

/// Monitors the current process for resource consumption.
pub struct ProcessMonitor {
    config: ProcessMonitorConfig,
    start_time: Instant,
}

impl ProcessMonitor {
    /// Create a new process monitor with the given configuration.
    pub fn new(config: ProcessMonitorConfig) -> Self {
        Self {
            config,
            start_time: Instant::now(),
        }
    }

    /// Take a snapshot of current process resources.
    ///
    /// Uses platform-agnostic estimates; real values depend on OS APIs.
    pub fn snapshot(&self) -> ProcessSnapshot {
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);

        ProcessSnapshot {
            memory_bytes: 0, // Must be populated by platform-specific code
            thread_count,
            uptime: self.start_time.elapsed(),
        }
    }

    /// Evaluate a process snapshot against configured thresholds.
    pub fn evaluate(&self, snapshot: &ProcessSnapshot) -> ProcessAlertDetail {
        let mut messages = Vec::new();
        let mut severity = ProcessAlert::Normal;

        // Memory checks
        if snapshot.memory_bytes >= self.config.memory_critical_bytes {
            messages.push(format!(
                "Memory critical: {} MB (limit: {} MB)",
                snapshot.memory_bytes / (1024 * 1024),
                self.config.memory_critical_bytes / (1024 * 1024),
            ));
            severity = ProcessAlert::Critical;
        } else if snapshot.memory_bytes >= self.config.memory_warn_bytes {
            messages.push(format!(
                "Memory warning: {} MB (limit: {} MB)",
                snapshot.memory_bytes / (1024 * 1024),
                self.config.memory_warn_bytes / (1024 * 1024),
            ));
            if severity < ProcessAlert::Warning {
                severity = ProcessAlert::Warning;
            }
        }

        // Thread count checks
        if snapshot.thread_count >= self.config.thread_critical_count {
            messages.push(format!(
                "Thread count critical: {} (limit: {})",
                snapshot.thread_count, self.config.thread_critical_count,
            ));
            severity = ProcessAlert::Critical;
        } else if snapshot.thread_count >= self.config.thread_warn_count {
            messages.push(format!(
                "Thread count warning: {} (limit: {})",
                snapshot.thread_count, self.config.thread_warn_count,
            ));
            if severity < ProcessAlert::Warning {
                severity = ProcessAlert::Warning;
            }
        }

        ProcessAlertDetail { severity, messages }
    }

    /// Convenience: take a snapshot and evaluate it in one call.
    pub fn check(&self) -> ProcessAlertDetail {
        let snap = self.snapshot();
        self.evaluate(&snap)
    }

    /// Process uptime since monitor creation.
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new(ProcessMonitorConfig::default())
    }
}

impl PartialOrd for ProcessAlert {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProcessAlert {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn rank(a: &ProcessAlert) -> u8 {
            match a {
                ProcessAlert::Normal => 0,
                ProcessAlert::Warning => 1,
                ProcessAlert::Critical => 2,
            }
        }
        rank(self).cmp(&rank(other))
    }
}

// ── Dead-man's switch for the axis engine ──────────────────────────────────

/// Configuration for the dead-man's switch.
#[derive(Debug, Clone)]
pub struct DeadManSwitchConfig {
    /// Expected tick interval (4ms for 250 Hz).
    pub expected_interval: Duration,
    /// Number of missed intervals before the switch triggers.
    pub missed_intervals_threshold: u32,
}

impl Default for DeadManSwitchConfig {
    fn default() -> Self {
        Self {
            expected_interval: Duration::from_millis(4),
            missed_intervals_threshold: 5,
        }
    }
}

/// Status of the dead-man's switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadManStatus {
    /// Axis engine is ticking within expected interval.
    Alive,
    /// Axis engine has missed some ticks but is within tolerance.
    Late { missed_ticks: u32 },
    /// Axis engine has not ticked beyond the threshold — it is considered dead.
    Triggered { missed_ticks: u32 },
}

/// Dead-man's switch for the axis engine tick.
///
/// The axis engine must call [`tick`] on every processing cycle.
/// The supervisor calls [`check`] to determine if the engine is alive.
pub struct DeadManSwitch {
    config: DeadManSwitchConfig,
    last_tick: Instant,
    total_triggers: u64,
}

impl DeadManSwitch {
    /// Create a new dead-man's switch.
    pub fn new(config: DeadManSwitchConfig) -> Self {
        Self {
            config,
            last_tick: Instant::now(),
            total_triggers: 0,
        }
    }

    /// Record a tick from the axis engine.
    pub fn tick(&mut self) {
        self.last_tick = Instant::now();
    }

    /// Check whether the axis engine is still alive.
    pub fn check(&mut self) -> DeadManStatus {
        let elapsed = self.last_tick.elapsed();
        let interval_ns = self.config.expected_interval.as_nanos().max(1);
        let missed = (elapsed.as_nanos() / interval_ns) as u32;

        if missed >= self.config.missed_intervals_threshold {
            self.total_triggers += 1;
            DeadManStatus::Triggered {
                missed_ticks: missed,
            }
        } else if missed > 0 {
            DeadManStatus::Late {
                missed_ticks: missed,
            }
        } else {
            DeadManStatus::Alive
        }
    }

    /// Total number of times the switch has triggered since creation.
    pub fn total_triggers(&self) -> u64 {
        self.total_triggers
    }

    /// Duration since last tick.
    pub fn elapsed_since_tick(&self) -> Duration {
        self.last_tick.elapsed()
    }

    /// Reset the switch (e.g. after recovery).
    pub fn reset(&mut self) {
        self.last_tick = Instant::now();
    }
}

impl Default for DeadManSwitch {
    fn default() -> Self {
        Self::new(DeadManSwitchConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // ── HardwareWatchdog tests ──────────────────────────────────────────

    #[test]
    fn watchdog_ok_when_petted() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_secs(10),
            max_timeouts: 3,
        });
        wd.pet();
        assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
    }

    #[test]
    fn watchdog_warns_on_timeout() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 3,
        });
        thread::sleep(Duration::from_millis(5));
        let status = wd.check();
        assert!(matches!(status, WatchdogTimerStatus::Warning { .. }));
    }

    #[test]
    fn watchdog_expires_after_max_timeouts() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 2,
        });
        thread::sleep(Duration::from_millis(5));
        wd.check(); // first timeout
        // The timer is still expired, so consecutive check increments again
        let status = wd.check();
        assert!(matches!(status, WatchdogTimerStatus::Expired { .. }));
    }

    #[test]
    fn watchdog_pet_resets_timeouts() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 3,
        });
        thread::sleep(Duration::from_millis(5));
        wd.check();
        assert!(wd.consecutive_timeouts() > 0);
        wd.pet();
        assert_eq!(wd.consecutive_timeouts(), 0);
        assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
    }

    #[test]
    fn watchdog_disabled_always_ok() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 1,
        });
        wd.set_enabled(false);
        thread::sleep(Duration::from_millis(5));
        assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
        assert!(!wd.is_enabled());
    }

    #[test]
    fn watchdog_total_timeouts_tracked() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 100,
        });
        thread::sleep(Duration::from_millis(5));
        wd.check();
        wd.check();
        assert_eq!(wd.total_timeouts(), 2);
    }

    // ── ProcessMonitor tests ────────────────────────────────────────────

    #[test]
    fn process_monitor_default_is_normal() {
        let monitor = ProcessMonitor::default();
        let alert = monitor.check();
        assert_eq!(alert.severity, ProcessAlert::Normal);
        assert!(alert.messages.is_empty());
    }

    #[test]
    fn process_monitor_warns_on_high_memory() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: 100,
            memory_critical_bytes: 200,
            thread_warn_count: 1000,
            thread_critical_count: 2000,
        });
        let snap = ProcessSnapshot {
            memory_bytes: 150,
            thread_count: 4,
            uptime: Duration::from_secs(1),
        };
        let alert = monitor.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Warning);
        assert!(!alert.messages.is_empty());
    }

    #[test]
    fn process_monitor_critical_on_extreme_memory() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: 100,
            memory_critical_bytes: 200,
            thread_warn_count: 1000,
            thread_critical_count: 2000,
        });
        let snap = ProcessSnapshot {
            memory_bytes: 300,
            thread_count: 4,
            uptime: Duration::from_secs(1),
        };
        let alert = monitor.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Critical);
    }

    #[test]
    fn process_monitor_thread_warning() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: u64::MAX,
            memory_critical_bytes: u64::MAX,
            thread_warn_count: 5,
            thread_critical_count: 10,
        });
        let snap = ProcessSnapshot {
            memory_bytes: 0,
            thread_count: 7,
            uptime: Duration::from_secs(1),
        };
        let alert = monitor.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Warning);
    }

    #[test]
    fn process_monitor_thread_critical() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: u64::MAX,
            memory_critical_bytes: u64::MAX,
            thread_warn_count: 5,
            thread_critical_count: 10,
        });
        let snap = ProcessSnapshot {
            memory_bytes: 0,
            thread_count: 15,
            uptime: Duration::from_secs(1),
        };
        let alert = monitor.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Critical);
    }

    #[test]
    fn process_monitor_uptime_advances() {
        let monitor = ProcessMonitor::default();
        thread::sleep(Duration::from_millis(10));
        assert!(monitor.uptime() >= Duration::from_millis(10));
    }

    // ── DeadManSwitch tests ─────────────────────────────────────────────

    #[test]
    fn dead_man_switch_alive_when_ticked() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_secs(10),
            missed_intervals_threshold: 5,
        });
        dms.tick();
        assert_eq!(dms.check(), DeadManStatus::Alive);
    }

    #[test]
    fn dead_man_switch_late_when_overdue() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 100,
        });
        thread::sleep(Duration::from_millis(5));
        let status = dms.check();
        assert!(matches!(status, DeadManStatus::Late { .. }));
    }

    #[test]
    fn dead_man_switch_triggers_on_threshold() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 2,
        });
        thread::sleep(Duration::from_millis(10));
        let status = dms.check();
        assert!(matches!(status, DeadManStatus::Triggered { .. }));
        assert_eq!(dms.total_triggers(), 1);
    }

    #[test]
    fn dead_man_switch_reset_clears_state() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 2,
        });
        thread::sleep(Duration::from_millis(10));
        dms.check();
        assert!(dms.total_triggers() > 0);
        dms.reset();
        assert_eq!(dms.check(), DeadManStatus::Alive);
    }

    #[test]
    fn dead_man_switch_multiple_triggers() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 2,
        });
        thread::sleep(Duration::from_millis(10));
        dms.check();
        dms.check();
        assert_eq!(dms.total_triggers(), 2);
    }

    // ── ProcessAlert ordering ───────────────────────────────────────────

    #[test]
    fn process_alert_ordering() {
        assert!(ProcessAlert::Normal < ProcessAlert::Warning);
        assert!(ProcessAlert::Warning < ProcessAlert::Critical);
    }
}
