// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health-probe monitoring, deadlock detection, and recovery policies.
//!
//! Provides:
//! - [`HealthProbe`]: deadline-based heartbeat monitoring for a subsystem.
//! - [`DeadlockDetector`]: fixed-size, zero-heap progress-token tracker.
//! - [`RecoveryAction`] / [`WatchdogPolicy`]: failure → action mapping.
//! - [`WatchdogReport`]: aggregated snapshot of all probe states.

use std::time::{Duration, Instant};

// ── Constants ───────────────────────────────────────────────────────────────

/// Maximum number of probe slots in [`DeadlockDetector`] (no heap).
pub const MAX_PROBE_SLOTS: usize = 32;

/// Maximum length of a probe name stored inline.
const MAX_NAME_LEN: usize = 64;

// ── HealthProbe ─────────────────────────────────────────────────────────────

/// A monitored subsystem with deadline-based heartbeat checking.
#[derive(Debug, Clone)]
pub struct HealthProbe {
    name: String,
    last_heartbeat: Option<Instant>,
    deadline: Duration,
    expected_interval: Duration,
}

impl HealthProbe {
    /// Create a new probe.
    ///
    /// * `deadline` — max time since last heartbeat before the probe is unhealthy.
    /// * `expected_interval` — nominal heartbeat period (informational).
    pub fn new(name: &str, deadline: Duration, expected_interval: Duration) -> Self {
        Self {
            name: name.to_string(),
            last_heartbeat: None,
            deadline,
            expected_interval,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn deadline(&self) -> Duration {
        self.deadline
    }

    pub fn expected_interval(&self) -> Duration {
        self.expected_interval
    }

    pub fn last_heartbeat(&self) -> Option<Instant> {
        self.last_heartbeat
    }

    /// Record a heartbeat at the current time.
    pub fn record_heartbeat(&mut self) {
        self.last_heartbeat = Some(Instant::now());
    }

    /// Record a heartbeat at a specific instant (useful for testing).
    pub fn record_heartbeat_at(&mut self, now: Instant) {
        self.last_heartbeat = Some(now);
    }

    /// Returns `true` if the last heartbeat is within the deadline.
    pub fn is_healthy(&self) -> bool {
        self.is_healthy_at(Instant::now())
    }

    /// Returns `true` if the last heartbeat is within the deadline relative to `now`.
    pub fn is_healthy_at(&self, now: Instant) -> bool {
        match self.last_heartbeat {
            Some(last) => now.duration_since(last) <= self.deadline,
            None => false,
        }
    }
}

// ── DeadlockDetector ────────────────────────────────────────────────────────

/// Inline name buffer — avoids heap allocation for slot names.
#[derive(Clone, Copy)]
struct InlineName {
    buf: [u8; MAX_NAME_LEN],
    len: usize,
}

impl InlineName {
    const EMPTY: Self = Self {
        buf: [0; MAX_NAME_LEN],
        len: 0,
    };

    fn from_str(s: &str) -> Self {
        let mut buf = [0u8; MAX_NAME_LEN];
        let len = s.len().min(MAX_NAME_LEN);
        buf[..len].copy_from_slice(&s.as_bytes()[..len]);
        Self { buf, len }
    }

    fn as_str(&self) -> &str {
        // SAFETY: we only ever copy valid UTF-8 bytes in `from_str`.
        unsafe { std::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl std::fmt::Debug for InlineName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

/// A single slot inside [`DeadlockDetector`].
#[derive(Debug, Clone, Copy)]
struct ProbeSlot {
    active: bool,
    name: InlineName,
    progress_token: u64,
    last_progress: Option<Instant>,
    deadline: Duration,
}

impl ProbeSlot {
    const EMPTY: Self = Self {
        active: false,
        name: InlineName::EMPTY,
        progress_token: 0,
        last_progress: None,
        deadline: Duration::from_secs(5),
    };
}

/// Monitors subsystems for deadlock / stall by tracking progress tokens.
///
/// Fixed-size array — **no heap allocations**.
pub struct DeadlockDetector {
    slots: [ProbeSlot; MAX_PROBE_SLOTS],
    count: usize,
}

impl DeadlockDetector {
    /// Create an empty detector.
    pub fn new() -> Self {
        Self {
            slots: [ProbeSlot::EMPTY; MAX_PROBE_SLOTS],
            count: 0,
        }
    }

    /// Register a subsystem. Returns `false` if the table is full.
    pub fn register(&mut self, name: &str, deadline: Duration) -> bool {
        if self.count >= MAX_PROBE_SLOTS {
            return false;
        }
        let slot = &mut self.slots[self.count];
        slot.active = true;
        slot.name = InlineName::from_str(name);
        slot.deadline = deadline;
        slot.progress_token = 0;
        slot.last_progress = None;
        self.count += 1;
        true
    }

    /// Record forward progress for a subsystem.
    pub fn record_progress(&mut self, name: &str, token: u64) {
        self.record_progress_at(name, token, Instant::now());
    }

    /// Record forward progress with an explicit timestamp.
    pub fn record_progress_at(&mut self, name: &str, token: u64, now: Instant) {
        if let Some(slot) = self.find_mut(name)
            && token != slot.progress_token
        {
            slot.progress_token = token;
            slot.last_progress = Some(now);
        }
    }

    /// Return the current progress token for a subsystem.
    pub fn progress_token(&self, name: &str) -> Option<u64> {
        self.find(name).map(|s| s.progress_token)
    }

    /// Return names of subsystems that appear stuck (no progress within deadline).
    pub fn detect_stuck(&self) -> Vec<String> {
        self.detect_stuck_at(Instant::now())
    }

    /// Return names of stuck subsystems relative to `now`.
    pub fn detect_stuck_at(&self, now: Instant) -> Vec<String> {
        let mut stuck = Vec::new();
        for slot in &self.slots[..self.count] {
            if !slot.active {
                continue;
            }
            let is_stuck = match slot.last_progress {
                Some(t) => now.duration_since(t) > slot.deadline,
                None => false, // never reported — not stuck yet
            };
            if is_stuck {
                stuck.push(slot.name.as_str().to_string());
            }
        }
        stuck
    }

    /// Number of registered probes.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether the detector has no registered probes.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    // ── helpers ──

    fn find(&self, name: &str) -> Option<&ProbeSlot> {
        self.slots[..self.count]
            .iter()
            .find(|s| s.active && s.name.as_str() == name)
    }

    fn find_mut(&mut self, name: &str) -> Option<&mut ProbeSlot> {
        self.slots[..self.count]
            .iter_mut()
            .find(|s| s.active && s.name.as_str() == name)
    }
}

impl Default for DeadlockDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ── RecoveryAction ──────────────────────────────────────────────────────────

/// Recovery action taken when a health probe or deadlock detector flags a problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeRecoveryAction {
    /// Restart the failed subsystem.
    RestartSubsystem(String),
    /// Enter system-wide safe mode.
    EnterSafeMode,
    /// Log the failure and continue operating.
    LogAndContinue,
    /// Shut down the system.
    Shutdown,
}

// ── SubsystemCriticality ────────────────────────────────────────────────────

/// Criticality classification for a subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemCriticality {
    /// Safety-critical (e.g. axis engine, FFB).
    Critical,
    /// Non-critical (e.g. adapter, panel).
    NonCritical,
}

// ── WatchdogPolicy ──────────────────────────────────────────────────────────

/// A rule that maps a subsystem failure to a recovery action.
#[derive(Debug, Clone)]
struct PolicyRule {
    subsystem: String,
    action: ProbeRecoveryAction,
}

/// Policy engine that decides recovery actions based on probe failures.
///
/// Rules:
/// - Critical subsystem failure → [`ProbeRecoveryAction::EnterSafeMode`]
/// - Non-critical failure → [`ProbeRecoveryAction::RestartSubsystem`]
/// - Multiple simultaneous failures beyond a cascade threshold → [`ProbeRecoveryAction::Shutdown`]
pub struct WatchdogPolicy {
    rules: Vec<PolicyRule>,
    cascade_threshold: usize,
}

impl WatchdogPolicy {
    /// Create a policy with the given cascade threshold.
    ///
    /// When the number of simultaneous failures reaches `cascade_threshold`,
    /// the policy returns [`ProbeRecoveryAction::Shutdown`].
    pub fn new(cascade_threshold: usize) -> Self {
        Self {
            rules: Vec::new(),
            cascade_threshold,
        }
    }

    /// Register a subsystem with its criticality.
    pub fn register(&mut self, subsystem: &str, criticality: SubsystemCriticality) {
        let action = match criticality {
            SubsystemCriticality::Critical => ProbeRecoveryAction::EnterSafeMode,
            SubsystemCriticality::NonCritical => {
                ProbeRecoveryAction::RestartSubsystem(subsystem.to_string())
            }
        };
        self.rules.push(PolicyRule {
            subsystem: subsystem.to_string(),
            action,
        });
    }

    /// Evaluate the policy for a single failed subsystem.
    pub fn evaluate_single(&self, subsystem: &str) -> ProbeRecoveryAction {
        self.rules
            .iter()
            .find(|r| r.subsystem == subsystem)
            .map(|r| r.action.clone())
            .unwrap_or(ProbeRecoveryAction::LogAndContinue)
    }

    /// Evaluate the policy given a set of simultaneously failed subsystems.
    ///
    /// If the number of failures meets or exceeds the cascade threshold,
    /// returns [`ProbeRecoveryAction::Shutdown`]. Otherwise returns the
    /// highest-severity single action.
    pub fn evaluate(&self, failed: &[&str]) -> ProbeRecoveryAction {
        if failed.is_empty() {
            return ProbeRecoveryAction::LogAndContinue;
        }
        if failed.len() >= self.cascade_threshold {
            return ProbeRecoveryAction::Shutdown;
        }

        // Return the most severe individual action.
        let mut worst = ProbeRecoveryAction::LogAndContinue;
        for name in failed {
            let action = self.evaluate_single(name);
            worst = more_severe(worst, action);
        }
        worst
    }

    /// The cascade failure threshold.
    pub fn cascade_threshold(&self) -> usize {
        self.cascade_threshold
    }
}

impl Default for WatchdogPolicy {
    fn default() -> Self {
        Self::new(3)
    }
}

/// Return the more severe of two actions.
fn more_severe(a: ProbeRecoveryAction, b: ProbeRecoveryAction) -> ProbeRecoveryAction {
    fn rank(a: &ProbeRecoveryAction) -> u8 {
        match a {
            ProbeRecoveryAction::LogAndContinue => 0,
            ProbeRecoveryAction::RestartSubsystem(_) => 1,
            ProbeRecoveryAction::EnterSafeMode => 2,
            ProbeRecoveryAction::Shutdown => 3,
        }
    }
    if rank(&b) > rank(&a) { b } else { a }
}

// ── WatchdogReport ──────────────────────────────────────────────────────────

/// Status of a single probe in a [`WatchdogReport`].
#[derive(Debug, Clone)]
pub struct ProbeStatus {
    /// Probe name.
    pub name: String,
    /// Whether the probe's heartbeat is within its deadline.
    pub healthy: bool,
    /// Time since last heartbeat, if any.
    pub time_since_heartbeat: Option<Duration>,
}

/// Aggregated snapshot of all probe states.
#[derive(Debug, Clone)]
pub struct WatchdogReport {
    /// Per-probe status entries.
    pub probes: Vec<ProbeStatus>,
    /// Names of subsystems flagged as stuck by deadlock detection.
    pub stuck_subsystems: Vec<String>,
    /// Number of healthy probes.
    pub healthy_count: usize,
    /// Number of unhealthy probes.
    pub unhealthy_count: usize,
    /// Overall system healthy (all probes healthy and no stuck subsystems).
    pub all_healthy: bool,
}

impl WatchdogReport {
    /// Build a report from a slice of probes and a deadlock detector.
    pub fn build(probes: &[HealthProbe], detector: &DeadlockDetector) -> Self {
        Self::build_at(probes, detector, Instant::now())
    }

    /// Build a report relative to a specific instant.
    pub fn build_at(
        probes: &[HealthProbe],
        detector: &DeadlockDetector,
        now: Instant,
    ) -> Self {
        let mut statuses = Vec::with_capacity(probes.len());
        let mut healthy_count = 0usize;
        let mut unhealthy_count = 0usize;

        for p in probes {
            let healthy = p.is_healthy_at(now);
            if healthy {
                healthy_count += 1;
            } else {
                unhealthy_count += 1;
            }
            statuses.push(ProbeStatus {
                name: p.name().to_string(),
                healthy,
                time_since_heartbeat: p.last_heartbeat().map(|t| now.duration_since(t)),
            });
        }

        let stuck = detector.detect_stuck_at(now);
        let all_healthy = unhealthy_count == 0 && stuck.is_empty();

        Self {
            probes: statuses,
            stuck_subsystems: stuck,
            healthy_count,
            unhealthy_count,
            all_healthy,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    // ── HealthProbe ─────────────────────────────────────────────────────

    #[test]
    fn probe_unhealthy_before_first_heartbeat() {
        let probe = HealthProbe::new("axis", Duration::from_secs(1), Duration::from_millis(4));
        assert!(!probe.is_healthy());
    }

    #[test]
    fn probe_healthy_within_deadline() {
        let mut probe = HealthProbe::new("axis", Duration::from_secs(5), Duration::from_millis(4));
        probe.record_heartbeat();
        assert!(probe.is_healthy());
    }

    #[test]
    fn probe_unhealthy_past_deadline() {
        let mut probe =
            HealthProbe::new("axis", Duration::from_millis(1), Duration::from_millis(1));
        probe.record_heartbeat();
        thread::sleep(Duration::from_millis(5));
        assert!(!probe.is_healthy());
    }

    #[test]
    fn heartbeat_updates_timestamp() {
        let mut probe = HealthProbe::new("ffb", Duration::from_secs(5), Duration::from_millis(4));
        assert!(probe.last_heartbeat().is_none());
        probe.record_heartbeat();
        assert!(probe.last_heartbeat().is_some());
    }

    #[test]
    fn probe_accessors() {
        let probe =
            HealthProbe::new("scheduler", Duration::from_secs(2), Duration::from_millis(4));
        assert_eq!(probe.name(), "scheduler");
        assert_eq!(probe.deadline(), Duration::from_secs(2));
        assert_eq!(probe.expected_interval(), Duration::from_millis(4));
    }

    #[test]
    fn probe_healthy_at_explicit_time() {
        let mut probe = HealthProbe::new("bus", Duration::from_millis(100), Duration::from_millis(4));
        let t0 = Instant::now();
        probe.record_heartbeat_at(t0);
        // 50 ms later → still healthy
        assert!(probe.is_healthy_at(t0 + Duration::from_millis(50)));
        // 200 ms later → unhealthy
        assert!(!probe.is_healthy_at(t0 + Duration::from_millis(200)));
    }

    // ── DeadlockDetector ────────────────────────────────────────────────

    #[test]
    fn detector_register_and_count() {
        let mut det = DeadlockDetector::new();
        assert!(det.is_empty());
        det.register("axis", Duration::from_secs(1));
        assert_eq!(det.len(), 1);
        assert!(!det.is_empty());
    }

    #[test]
    fn detector_register_full_returns_false() {
        let mut det = DeadlockDetector::new();
        for i in 0..MAX_PROBE_SLOTS {
            assert!(det.register(&format!("s{i}"), Duration::from_secs(1)));
        }
        assert!(!det.register("overflow", Duration::from_secs(1)));
    }

    #[test]
    fn detector_no_stuck_when_never_reported() {
        let mut det = DeadlockDetector::new();
        det.register("axis", Duration::from_secs(1));
        assert!(det.detect_stuck().is_empty());
    }

    #[test]
    fn detector_not_stuck_when_progressing() {
        let mut det = DeadlockDetector::new();
        det.register("axis", Duration::from_secs(5));
        det.record_progress("axis", 1);
        assert!(det.detect_stuck().is_empty());
    }

    #[test]
    fn detector_stuck_after_deadline() {
        let mut det = DeadlockDetector::new();
        det.register("axis", Duration::from_millis(1));
        det.record_progress("axis", 1);
        thread::sleep(Duration::from_millis(5));
        let stuck = det.detect_stuck();
        assert_eq!(stuck, vec!["axis"]);
    }

    #[test]
    fn detector_progress_token_tracking() {
        let mut det = DeadlockDetector::new();
        det.register("ffb", Duration::from_secs(5));
        assert_eq!(det.progress_token("ffb"), Some(0));
        det.record_progress("ffb", 42);
        assert_eq!(det.progress_token("ffb"), Some(42));
    }

    #[test]
    fn detector_same_token_does_not_update_timestamp() {
        let mut det = DeadlockDetector::new();
        det.register("axis", Duration::from_millis(1));
        let t0 = Instant::now();
        det.record_progress_at("axis", 1, t0);
        // Record the same token at a much later time — should NOT update.
        det.record_progress_at("axis", 1, t0 + Duration::from_secs(10));
        // The timestamp should still be t0, so check relative to t0 + 5ms.
        let stuck = det.detect_stuck_at(t0 + Duration::from_millis(5));
        assert_eq!(stuck, vec!["axis"]);
    }

    #[test]
    fn detector_multiple_subsystems_independent() {
        let mut det = DeadlockDetector::new();
        det.register("axis", Duration::from_millis(1));
        det.register("ffb", Duration::from_secs(60));
        let t0 = Instant::now();
        det.record_progress_at("axis", 1, t0);
        det.record_progress_at("ffb", 1, t0);
        // Only axis should be stuck at t0 + 5ms.
        let stuck = det.detect_stuck_at(t0 + Duration::from_millis(5));
        assert_eq!(stuck, vec!["axis"]);
    }

    // ── WatchdogPolicy ──────────────────────────────────────────────────

    #[test]
    fn policy_single_noncritical_failure_restarts() {
        let mut policy = WatchdogPolicy::new(3);
        policy.register("adapter", SubsystemCriticality::NonCritical);
        let action = policy.evaluate(&["adapter"]);
        assert_eq!(
            action,
            ProbeRecoveryAction::RestartSubsystem("adapter".to_string())
        );
    }

    #[test]
    fn policy_critical_failure_enters_safe_mode() {
        let mut policy = WatchdogPolicy::new(3);
        policy.register("axis_engine", SubsystemCriticality::Critical);
        let action = policy.evaluate(&["axis_engine"]);
        assert_eq!(action, ProbeRecoveryAction::EnterSafeMode);
    }

    #[test]
    fn policy_cascade_triggers_shutdown() {
        let mut policy = WatchdogPolicy::new(3);
        policy.register("a", SubsystemCriticality::NonCritical);
        policy.register("b", SubsystemCriticality::NonCritical);
        policy.register("c", SubsystemCriticality::NonCritical);
        let action = policy.evaluate(&["a", "b", "c"]);
        assert_eq!(action, ProbeRecoveryAction::Shutdown);
    }

    #[test]
    fn policy_unknown_subsystem_logs_and_continues() {
        let policy = WatchdogPolicy::new(3);
        let action = policy.evaluate_single("unknown");
        assert_eq!(action, ProbeRecoveryAction::LogAndContinue);
    }

    #[test]
    fn policy_no_failures_logs_and_continues() {
        let policy = WatchdogPolicy::new(3);
        let action = policy.evaluate(&[]);
        assert_eq!(action, ProbeRecoveryAction::LogAndContinue);
    }

    #[test]
    fn policy_mixed_picks_most_severe() {
        let mut policy = WatchdogPolicy::new(5);
        policy.register("adapter", SubsystemCriticality::NonCritical);
        policy.register("axis_engine", SubsystemCriticality::Critical);
        // Two failures (below cascade threshold of 5)
        let action = policy.evaluate(&["adapter", "axis_engine"]);
        assert_eq!(action, ProbeRecoveryAction::EnterSafeMode);
    }

    #[test]
    fn policy_cascade_threshold_accessor() {
        let policy = WatchdogPolicy::new(7);
        assert_eq!(policy.cascade_threshold(), 7);
    }

    // ── WatchdogReport ──────────────────────────────────────────────────

    #[test]
    fn report_all_healthy() {
        let mut probes = vec![
            HealthProbe::new("axis", Duration::from_secs(5), Duration::from_millis(4)),
            HealthProbe::new("ffb", Duration::from_secs(5), Duration::from_millis(4)),
        ];
        for p in &mut probes {
            p.record_heartbeat();
        }
        let det = DeadlockDetector::new();
        let report = WatchdogReport::build(&probes, &det);
        assert!(report.all_healthy);
        assert_eq!(report.healthy_count, 2);
        assert_eq!(report.unhealthy_count, 0);
        assert!(report.stuck_subsystems.is_empty());
    }

    #[test]
    fn report_with_unhealthy_probe() {
        let mut probes = vec![
            HealthProbe::new("axis", Duration::from_secs(5), Duration::from_millis(4)),
            HealthProbe::new("ffb", Duration::from_secs(5), Duration::from_millis(4)),
        ];
        // Only heartbeat the first probe.
        probes[0].record_heartbeat();
        let det = DeadlockDetector::new();
        let report = WatchdogReport::build(&probes, &det);
        assert!(!report.all_healthy);
        assert_eq!(report.healthy_count, 1);
        assert_eq!(report.unhealthy_count, 1);
    }

    #[test]
    fn report_with_stuck_subsystem() {
        let mut probes = vec![
            HealthProbe::new("axis", Duration::from_secs(5), Duration::from_millis(4)),
        ];
        probes[0].record_heartbeat();

        let mut det = DeadlockDetector::new();
        det.register("axis", Duration::from_millis(1));
        det.record_progress("axis", 1);
        thread::sleep(Duration::from_millis(5));

        let report = WatchdogReport::build(&probes, &det);
        assert!(!report.all_healthy);
        assert_eq!(report.stuck_subsystems, vec!["axis"]);
    }

    #[test]
    fn report_probe_statuses_contain_names() {
        let mut probes = vec![
            HealthProbe::new("a", Duration::from_secs(5), Duration::from_millis(4)),
            HealthProbe::new("b", Duration::from_secs(5), Duration::from_millis(4)),
        ];
        probes[0].record_heartbeat();
        let det = DeadlockDetector::new();
        let report = WatchdogReport::build(&probes, &det);
        let names: Vec<&str> = report.probes.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }
}
