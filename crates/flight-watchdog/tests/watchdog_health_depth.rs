// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the watchdog and health monitoring subsystem.
//!
//! Covers: heartbeat monitoring, escalation chains, health aggregation,
//! recovery actions, and RT-safety properties across
//! `SystemMonitor`, `EscalationLadder`, `HealthAggregator`,
//! `HealthCheckManager`, `RecoveryPolicy`, `HardwareWatchdog`,
//! and `DeadManSwitch`.

use std::time::{Duration, Instant};

use flight_watchdog::escalation::{EscalationAction, EscalationConfig, EscalationLadder, EscalationLevel};
use flight_watchdog::health_aggregator::{
    HealthAggregator, SubsystemCheckConfig, SubsystemHealth,
};
use flight_watchdog::health_check::{HealthCheckManager, HealthStatus};
use flight_watchdog::monitor::{MonitorConfig, SystemMode, SystemMonitor};
use flight_watchdog::recovery::{RecoveryAction, RecoveryPolicy};
use flight_watchdog::supervisor::{
    DeadManStatus, DeadManSwitch, DeadManSwitchConfig, HardwareWatchdog, ProcessAlert,
    ProcessMonitor, ProcessMonitorConfig, ProcessSnapshot, WatchdogTimerConfig, WatchdogTimerStatus,
};

// ═══════════════════════════════════════════════════════════════════════════
// 1. Heartbeat Monitoring (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Sustained normal heartbeats keep the system in Normal mode with zero misses.
#[test]
fn heartbeat_sustained_normal_pattern() {
    let mut mon = SystemMonitor::new(MonitorConfig::default());
    for _ in 0..500 {
        mon.record_heartbeat();
    }
    assert_eq!(mon.mode(), SystemMode::Normal);
    assert_eq!(mon.consecutive_missed_ticks(), 0);
    assert_eq!(mon.total_received_ticks(), 500);
    assert_eq!(mon.total_missed_ticks(), 0);
}

/// A single missed tick is detected and the monitor transitions to Warning.
#[test]
fn heartbeat_missed_detection_transitions_to_warning() {
    let mut mon = SystemMonitor::new(MonitorConfig {
        warn_after_missed_ticks: 1,
        degrade_after_missed_ticks: 10,
        safe_mode_after_missed_ticks: 50,
        ..MonitorConfig::default()
    });
    mon.record_heartbeat();
    mon.record_missed_tick();
    assert_eq!(mon.mode(), SystemMode::Warning);
    assert_eq!(mon.consecutive_missed_ticks(), 1);
    assert_eq!(mon.total_missed_ticks(), 1);
}

/// Configurable thresholds control when each escalation level is reached.
#[test]
fn heartbeat_configurable_timeout_thresholds() {
    // Use non-default thresholds: warn=2, degrade=4, safe=8
    let mut mon = SystemMonitor::new(MonitorConfig {
        warn_after_missed_ticks: 2,
        degrade_after_missed_ticks: 4,
        safe_mode_after_missed_ticks: 8,
        ..MonitorConfig::default()
    });

    mon.record_missed_tick();
    assert_eq!(mon.mode(), SystemMode::Normal, "1 miss < warn(2)");

    mon.record_missed_tick();
    assert_eq!(mon.mode(), SystemMode::Warning, "2 misses = warn(2)");

    mon.record_missed_tick();
    mon.record_missed_tick();
    assert_eq!(mon.mode(), SystemMode::Degraded, "4 misses = degrade(4)");

    for _ in 0..4 {
        mon.record_missed_tick();
    }
    assert_eq!(mon.mode(), SystemMode::SafeMode, "8 misses = safe(8)");
}

/// Multiple independent SystemMonitor instances track heartbeats separately.
#[test]
fn heartbeat_multiple_monitors_independent() {
    let mut mon_a = SystemMonitor::new(MonitorConfig {
        warn_after_missed_ticks: 1,
        degrade_after_missed_ticks: 5,
        safe_mode_after_missed_ticks: 20,
        ..MonitorConfig::default()
    });
    let mut mon_b = SystemMonitor::new(MonitorConfig {
        warn_after_missed_ticks: 3,
        degrade_after_missed_ticks: 10,
        safe_mode_after_missed_ticks: 50,
        ..MonitorConfig::default()
    });

    // Two misses: A should be Warning, B should be Normal
    mon_a.record_missed_tick();
    mon_a.record_missed_tick();
    mon_b.record_missed_tick();
    mon_b.record_missed_tick();

    assert_eq!(mon_a.mode(), SystemMode::Warning);
    assert_eq!(mon_b.mode(), SystemMode::Normal, "B has warn threshold 3");
}

/// Flooding heartbeats does not cause issues; counters stay correct.
#[test]
fn heartbeat_flood_does_not_break_state() {
    let mut mon = SystemMonitor::new(MonitorConfig::default());
    // Record a very large number of heartbeats rapidly
    for _ in 0..100_000 {
        mon.record_heartbeat();
    }
    assert_eq!(mon.mode(), SystemMode::Normal);
    assert_eq!(mon.total_received_ticks(), 100_000);
    assert_eq!(mon.consecutive_missed_ticks(), 0);
}

/// After missed ticks, a heartbeat resumes normal operation and clears
/// consecutive miss count.
#[test]
fn heartbeat_resume_after_miss_clears_state() {
    let mut mon = SystemMonitor::new(MonitorConfig {
        warn_after_missed_ticks: 1,
        degrade_after_missed_ticks: 3,
        safe_mode_after_missed_ticks: 10,
        ..MonitorConfig::default()
    });

    // Escalate to Degraded
    for _ in 0..3 {
        mon.record_missed_tick();
    }
    assert_eq!(mon.mode(), SystemMode::Degraded);

    // Resume
    mon.record_heartbeat();
    assert_eq!(mon.mode(), SystemMode::Normal);
    assert_eq!(mon.consecutive_missed_ticks(), 0);

    // Verify we need fresh misses to re-escalate
    mon.record_missed_tick();
    assert_eq!(mon.mode(), SystemMode::Warning, "only 1 miss after reset");
}

/// `check_heartbeat_timeout` with no prior heartbeat does not trigger (no
/// baseline yet).
#[test]
fn heartbeat_grace_period_after_start() {
    let mut mon = SystemMonitor::new(MonitorConfig {
        expected_tick_interval: Duration::from_millis(4),
        tick_timeout_multiplier: 2.0,
        ..MonitorConfig::default()
    });
    // No heartbeat recorded yet — last_heartbeat is None
    let missed = mon.check_heartbeat_timeout();
    assert!(!missed, "should not trigger without a prior heartbeat");
    assert_eq!(mon.mode(), SystemMode::Normal);
}

/// Total tick counters are accurate after a mix of hits and misses.
#[test]
fn heartbeat_counter_accuracy_mixed() {
    let mut mon = SystemMonitor::new(MonitorConfig {
        warn_after_missed_ticks: 1,
        degrade_after_missed_ticks: 100,
        safe_mode_after_missed_ticks: 200,
        ..MonitorConfig::default()
    });

    for _ in 0..10 {
        mon.record_heartbeat();
    }
    for _ in 0..3 {
        mon.record_missed_tick();
    }
    for _ in 0..7 {
        mon.record_heartbeat();
    }

    assert_eq!(mon.total_received_ticks(), 17);
    assert_eq!(mon.total_missed_ticks(), 3);
    assert_eq!(mon.consecutive_missed_ticks(), 0, "heartbeat after misses resets");
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Escalation (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Full escalation chain: Normal → Warn → Degrade → Restart → SafeMode.
#[test]
fn escalation_full_chain_warn_degrade_restart_safemode() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 3,
        restart_threshold: 5,
        safe_mode_threshold: 10,
        recovery_threshold: 3,
        restart_cooldown: Duration::ZERO,
        max_restart_attempts: 10,
    });

    ladder.register("comp");
    assert_eq!(ladder.level("comp"), EscalationLevel::Normal);

    // 1 failure → Warn
    let action = ladder.record_failure("comp", "err");
    assert!(matches!(action, EscalationAction::Warn(_)));
    assert_eq!(ladder.level("comp"), EscalationLevel::Warn);

    // 2 more → Degrade at 3
    ladder.record_failure("comp", "err");
    let action = ladder.record_failure("comp", "err");
    assert!(matches!(action, EscalationAction::Degrade(_)));
    assert_eq!(ladder.level("comp"), EscalationLevel::Degrade);

    // 2 more → Restart at 5
    ladder.record_failure("comp", "err");
    let action = ladder.record_failure("comp", "err");
    assert!(matches!(action, EscalationAction::Restart(_)));
    assert_eq!(ladder.level("comp"), EscalationLevel::Restart);

    // 5 more → SafeMode at 10
    for _ in 0..5 {
        ladder.record_failure("comp", "err");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::SafeMode);
}

/// Restart cooldown prevents rapid restart attempts; escalation stays at
/// Degrade while cooldown is active.
#[test]
fn escalation_cooldown_between_restarts() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 3,
        restart_threshold: 5,
        safe_mode_threshold: 100,
        recovery_threshold: 3,
        restart_cooldown: Duration::from_secs(60), // long cooldown
        max_restart_attempts: 10,
    });

    // Drive to Restart
    for _ in 0..5 {
        ladder.record_failure("comp", "err");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::Restart);
    assert_eq!(ladder.restart_count("comp"), 1);

    // Recover briefly
    for _ in 0..3 {
        ladder.record_success("comp");
    }

    // Drive to Restart threshold again — cooldown should prevent actual Restart
    for _ in 0..5 {
        ladder.record_failure("comp", "err");
    }
    // Should be Degrade because cooldown is still active
    assert_eq!(ladder.level("comp"), EscalationLevel::Degrade);
}

/// After enough successes, escalation resets back step-by-step.
#[test]
fn escalation_reset_on_recovery_steps() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 3,
        restart_threshold: 5,
        safe_mode_threshold: 10,
        recovery_threshold: 2, // only 2 successes needed to de-escalate
        restart_cooldown: Duration::ZERO,
        max_restart_attempts: 10,
    });

    // Escalate to Degrade
    for _ in 0..3 {
        ladder.record_failure("comp", "err");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::Degrade);

    // 2 successes → Warn
    for _ in 0..2 {
        ladder.record_success("comp");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::Warn);

    // 2 more successes → Normal
    for _ in 0..2 {
        ladder.record_success("comp");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::Normal);
}

/// Transition history captures every escalation/de-escalation step.
#[test]
fn escalation_transition_history_recorded() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 2,
        restart_threshold: 100,
        safe_mode_threshold: 200,
        recovery_threshold: 1,
        restart_cooldown: Duration::ZERO,
        max_restart_attempts: 10,
    });

    ladder.record_failure("c", "err"); // Normal → Warn
    ladder.record_failure("c", "err"); // Warn → Degrade
    ladder.record_success("c"); // Degrade → Warn

    let transitions = ladder.transitions();
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[0].from, EscalationLevel::Normal);
    assert_eq!(transitions[0].to, EscalationLevel::Warn);
    assert_eq!(transitions[1].from, EscalationLevel::Warn);
    assert_eq!(transitions[1].to, EscalationLevel::Degrade);
    assert_eq!(transitions[2].from, EscalationLevel::Degrade);
    assert_eq!(transitions[2].to, EscalationLevel::Warn);
}

/// Max restart attempts exceeded triggers SafeMode even below safe_mode_threshold.
#[test]
fn escalation_max_restarts_triggers_safemode() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 2,
        restart_threshold: 3,
        safe_mode_threshold: 200, // very high — won't reach by count alone
        recovery_threshold: 1,
        restart_cooldown: Duration::ZERO,
        max_restart_attempts: 2,
    });

    // First restart
    for _ in 0..3 {
        ladder.record_failure("c", "err");
    }
    assert_eq!(ladder.level("c"), EscalationLevel::Restart);
    assert_eq!(ladder.restart_count("c"), 1);

    // Recover, then second restart
    ladder.record_success("c");
    for _ in 0..3 {
        ladder.record_failure("c", "err");
    }
    assert_eq!(ladder.restart_count("c"), 2);

    // Recover, then third cycle — failure at restart_threshold sets level=Restart
    // with restart_count=3 (≥ max=2), so the *next* failure triggers SafeMode.
    ladder.record_success("c");
    for _ in 0..4 {
        ladder.record_failure("c", "err");
    }
    assert_eq!(ladder.level("c"), EscalationLevel::SafeMode);
}

/// Manual reset returns component to Normal and clears failure/restart counts.
#[test]
fn escalation_manual_reset_clears_all() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 2,
        restart_threshold: 3,
        safe_mode_threshold: 5,
        recovery_threshold: 3,
        restart_cooldown: Duration::ZERO,
        max_restart_attempts: 10,
    });

    for _ in 0..4 {
        ladder.record_failure("comp", "err");
    }
    assert_ne!(ladder.level("comp"), EscalationLevel::Normal);
    assert!(ladder.failure_count("comp") > 0);

    ladder.reset("comp");
    assert_eq!(ladder.level("comp"), EscalationLevel::Normal);
    assert_eq!(ladder.failure_count("comp"), 0);
    assert_eq!(ladder.restart_count("comp"), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Health Aggregation (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

fn agg_config(name: &str) -> SubsystemCheckConfig {
    SubsystemCheckConfig::new(name)
        .with_interval(Duration::from_millis(100))
        .with_staleness_timeout(Duration::from_secs(60))
        .with_failure_threshold(3)
}

/// Multiple subsystems report health; aggregate correctly counts each state.
#[test]
fn aggregation_multiple_component_reports() {
    let mut agg = HealthAggregator::new();
    agg.register(agg_config("axis"));
    agg.register(agg_config("hid"));
    agg.register(agg_config("ffb"));

    agg.report_healthy("axis");
    agg.report_warning("hid", "latency");
    agg.report_failure("ffb", "disconnect");

    let report = agg.aggregate();
    assert_eq!(report.healthy_count, 1);
    assert_eq!(report.warning_count, 1);
    assert_eq!(report.degraded_count, 1); // single failure → Degraded
    assert_eq!(report.failed_count, 0); // need 3 consecutive for Failed
}

/// Aggregate overall status reflects the worst subsystem.
#[test]
fn aggregation_overall_is_worst_of_all() {
    let mut agg = HealthAggregator::new();
    agg.register(agg_config("a"));
    agg.register(agg_config("b"));
    agg.register(agg_config("c"));

    agg.report_healthy("a");
    agg.report_healthy("b");
    agg.report_healthy("c");
    assert_eq!(agg.aggregate().overall, SubsystemHealth::Healthy);

    agg.report_warning("b", "slow");
    assert_eq!(agg.aggregate().overall, SubsystemHealth::Warning);

    // Drive c to Failed
    for _ in 0..3 {
        agg.report_failure("c", "dead");
    }
    assert_eq!(agg.aggregate().overall, SubsystemHealth::Failed);
}

/// A degraded subsystem does not affect healthy subsystems in the aggregate.
#[test]
fn aggregation_degraded_component_isolation() {
    let mut agg = HealthAggregator::new();
    agg.register(agg_config("axis"));
    agg.register(agg_config("panel"));

    agg.report_healthy("axis");
    agg.report_failure("panel", "disconnected");

    assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Healthy);
    assert_eq!(agg.subsystem_health("panel"), SubsystemHealth::Degraded);

    let report = agg.aggregate();
    assert_eq!(report.healthy_count, 1);
    assert_eq!(report.degraded_count, 1);
}

/// Transition history records all state changes per subsystem.
#[test]
fn aggregation_health_history_window() {
    let mut agg = HealthAggregator::new();
    agg.register(agg_config("x"));

    agg.report_healthy("x"); // Unknown → Healthy
    agg.report_warning("x", "slow"); // Healthy → Warning
    agg.report_healthy("x"); // Warning → Healthy
    agg.report_failure("x", "err"); // Healthy → Degraded

    let history = agg.transitions_for("x");
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].from, SubsystemHealth::Unknown);
    assert_eq!(history[0].to, SubsystemHealth::Healthy);
    assert_eq!(history[3].from, SubsystemHealth::Healthy);
    assert_eq!(history[3].to, SubsystemHealth::Degraded);
}

/// Staleness detection marks healthy subsystems as Unknown when they stop
/// reporting.
#[test]
fn aggregation_staleness_marks_unknown() {
    let mut agg = HealthAggregator::new();
    let mut cfg = agg_config("stale_sub");
    cfg.staleness_timeout = Duration::from_millis(1);
    agg.register(cfg);

    agg.report_healthy("stale_sub");
    assert_eq!(agg.subsystem_health("stale_sub"), SubsystemHealth::Healthy);

    std::thread::sleep(Duration::from_millis(5));
    agg.check_staleness();
    assert_eq!(agg.subsystem_health("stale_sub"), SubsystemHealth::Unknown);
}

/// Consecutive warnings past the threshold escalate to Degraded.
#[test]
fn aggregation_warning_threshold_escalates() {
    let mut agg = HealthAggregator::new();
    let mut cfg = agg_config("w");
    cfg.warning_threshold = 3;
    agg.register(cfg);

    agg.report_healthy("w");
    agg.report_warning("w", "a");
    agg.report_warning("w", "b");
    assert_eq!(agg.subsystem_health("w"), SubsystemHealth::Warning);

    agg.report_warning("w", "c"); // 3rd consecutive → Degraded
    assert_eq!(agg.subsystem_health("w"), SubsystemHealth::Degraded);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Recovery Actions (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// A recovery policy triggers component restart at the configured threshold.
#[test]
fn recovery_component_restart_rule() {
    let mut policy = RecoveryPolicy::new();
    policy.add_rule("hid", 1, RecoveryAction::LogWarning("warn".into()));
    policy.add_rule("hid", 3, RecoveryAction::RestartComponent("hid".into()));

    assert_eq!(
        policy.evaluate("hid", 2),
        RecoveryAction::LogWarning("warn".into()),
    );
    assert_eq!(
        policy.evaluate("hid", 3),
        RecoveryAction::RestartComponent("hid".into()),
    );
    assert_eq!(
        policy.evaluate("hid", 10),
        RecoveryAction::RestartComponent("hid".into()),
    );
}

/// Recovery policy can trigger a full service safe-mode activation.
#[test]
fn recovery_safe_mode_activation() {
    let mut policy = RecoveryPolicy::new();
    policy.add_rule("ffb", 1, RecoveryAction::LogWarning("warn".into()));
    policy.add_rule("ffb", 5, RecoveryAction::RestartComponent("ffb".into()));
    policy.add_rule("ffb", 15, RecoveryAction::EnterSafeMode);

    assert_eq!(policy.evaluate("ffb", 15), RecoveryAction::EnterSafeMode);
    assert_eq!(policy.evaluate("ffb", 100), RecoveryAction::EnterSafeMode);
}

/// Recovery policy correctly generates user alerts.
#[test]
fn recovery_alert_user_on_escalation() {
    let mut policy = RecoveryPolicy::new();
    policy.add_rule("panel", 2, RecoveryAction::AlertUser("Panel offline".into()));

    assert_eq!(policy.evaluate("panel", 1), RecoveryAction::NoAction);
    assert_eq!(
        policy.evaluate("panel", 2),
        RecoveryAction::AlertUser("Panel offline".into()),
    );
}

/// Escalation ladder drives recovery actions based on failure count.
#[test]
fn recovery_escalation_driven_actions() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 3,
        restart_threshold: 5,
        safe_mode_threshold: 10,
        recovery_threshold: 3,
        restart_cooldown: Duration::ZERO,
        max_restart_attempts: 10,
    });

    let mut policy = RecoveryPolicy::new();
    policy.add_rule("c", 1, RecoveryAction::LogWarning("warn".into()));
    policy.add_rule("c", 3, RecoveryAction::RestartComponent("c".into()));
    policy.add_rule("c", 10, RecoveryAction::EnterSafeMode);

    // Drive failures and check policy in sync with ladder
    for i in 1..=10 {
        ladder.record_failure("c", "err");
        let action = policy.evaluate("c", ladder.failure_count("c"));

        match i {
            1..=2 => assert_eq!(action, RecoveryAction::LogWarning("warn".into())),
            3..=9 => assert_eq!(action, RecoveryAction::RestartComponent("c".into())),
            10 => assert_eq!(action, RecoveryAction::EnterSafeMode),
            _ => unreachable!(),
        }
    }
}

/// Per-component policy isolation: rules for one component don't affect another.
#[test]
fn recovery_per_component_policy_isolation() {
    let mut policy = RecoveryPolicy::new();
    policy.add_rule("axis", 1, RecoveryAction::LogWarning("axis warn".into()));
    policy.add_rule("ffb", 1, RecoveryAction::EnterSafeMode);

    assert_eq!(
        policy.evaluate("axis", 5),
        RecoveryAction::LogWarning("axis warn".into()),
    );
    assert_eq!(policy.evaluate("ffb", 1), RecoveryAction::EnterSafeMode);
    assert_eq!(policy.evaluate("unknown", 100), RecoveryAction::NoAction);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. RT Safety (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// HardwareWatchdog check() completes quickly and does not block.
#[test]
fn rt_safety_watchdog_timer_does_not_block() {
    let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
        timeout: Duration::from_secs(10),
        max_timeouts: 5,
    });
    wd.pet();

    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = wd.check();
    }
    let elapsed = start.elapsed();
    // 10k checks must complete in well under 100ms on any modern hardware
    assert!(
        elapsed < Duration::from_millis(100),
        "10k watchdog checks took {:?}, expected <100ms",
        elapsed,
    );
}

/// HealthCheckManager report operations are non-blocking and fast.
#[test]
fn rt_safety_lock_free_heartbeat_reporting() {
    let mut mgr = HealthCheckManager::new();
    mgr.register("rt_comp", Duration::from_secs(5), 3);

    let start = Instant::now();
    for _ in 0..100_000 {
        mgr.report_healthy("rt_comp");
    }
    let elapsed = start.elapsed();
    // 100k reports should be extremely fast (no locks/allocations on hot path)
    assert!(
        elapsed < Duration::from_millis(200),
        "100k health reports took {:?}, expected <200ms",
        elapsed,
    );
    assert_eq!(mgr.check_status("rt_comp"), Some(&HealthStatus::Healthy));
}

/// SystemMonitor state transitions are atomic — mode is always a valid enum
/// value after any sequence of operations.
#[test]
fn rt_safety_atomic_state_transitions() {
    let mut mon = SystemMonitor::new(MonitorConfig {
        warn_after_missed_ticks: 1,
        degrade_after_missed_ticks: 3,
        safe_mode_after_missed_ticks: 5,
        ..MonitorConfig::default()
    });

    let valid_modes = [
        SystemMode::Normal,
        SystemMode::Warning,
        SystemMode::Degraded,
        SystemMode::SafeMode,
    ];

    // Interleave misses and heartbeats; mode must always be valid
    for i in 0..50 {
        if i % 7 == 0 {
            mon.record_heartbeat();
        } else {
            mon.record_missed_tick();
        }
        assert!(
            valid_modes.contains(&mon.mode()),
            "mode {:?} is not a valid SystemMode",
            mon.mode(),
        );
    }
}

/// DeadManSwitch timer accuracy: it correctly detects missed ticks within
/// the expected interval tolerance.
#[test]
fn rt_safety_dead_man_switch_timer_accuracy() {
    // Use a high threshold so that a short sleep is Late, not Triggered
    let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
        expected_interval: Duration::from_millis(10),
        missed_intervals_threshold: 10,
    });

    // Immediately after creation: Alive
    assert_eq!(dms.check(), DeadManStatus::Alive);

    // Sleep for ~25ms (2-3 intervals) — should be Late but not Triggered
    std::thread::sleep(Duration::from_millis(25));
    let status = dms.check();
    assert!(
        matches!(status, DeadManStatus::Late { missed_ticks } if missed_ticks >= 1),
        "expected Late after ~2.5 intervals, got {:?}",
        status,
    );

    // Reset and use low threshold to test Triggered
    let mut dms2 = DeadManSwitch::new(DeadManSwitchConfig {
        expected_interval: Duration::from_millis(1),
        missed_intervals_threshold: 3,
    });
    std::thread::sleep(Duration::from_millis(15));
    let status = dms2.check();
    assert!(
        matches!(status, DeadManStatus::Triggered { .. }),
        "expected Triggered after many missed intervals, got {:?}",
        status,
    );
}

/// HardwareWatchdog pet/check cycle is non-blocking and correctly tracks
/// consecutive timeouts vs total timeouts.
#[test]
fn rt_safety_hardware_watchdog_nonblocking_check() {
    let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
        timeout: Duration::from_millis(1),
        max_timeouts: 5,
    });

    std::thread::sleep(Duration::from_millis(5));

    // First check: timeout detected
    let status = wd.check();
    assert!(matches!(status, WatchdogTimerStatus::Warning { .. }));
    assert_eq!(wd.consecutive_timeouts(), 1);
    assert_eq!(wd.total_timeouts(), 1);

    // Pet resets consecutive but not total
    wd.pet();
    assert_eq!(wd.consecutive_timeouts(), 0);
    assert_eq!(wd.total_timeouts(), 1); // total preserved

    // Immediate check after pet: Ok
    assert_eq!(wd.check(), WatchdogTimerStatus::Ok);

    // Disable → always Ok
    wd.set_enabled(false);
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(wd.check(), WatchdogTimerStatus::Ok);

    // Re-enable resets the timer
    wd.set_enabled(true);
    assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
}

// ═══════════════════════════════════════════════════════════════════════════
// Bonus: Cross-cutting integration depth tests
// ═══════════════════════════════════════════════════════════════════════════

/// End-to-end: escalation ladder + recovery policy produce correct actions
/// through a full failure-recovery-failure cycle.
#[test]
fn integration_escalation_recovery_cycle() {
    let mut ladder = EscalationLadder::new(EscalationConfig {
        warn_threshold: 1,
        degrade_threshold: 3,
        restart_threshold: 5,
        safe_mode_threshold: 50,
        recovery_threshold: 2,
        restart_cooldown: Duration::ZERO,
        max_restart_attempts: 10,
    });

    let mut policy = RecoveryPolicy::new();
    policy.add_rule("comp", 1, RecoveryAction::LogWarning("warn".into()));
    policy.add_rule("comp", 3, RecoveryAction::RestartComponent("comp".into()));
    policy.add_rule("comp", 5, RecoveryAction::EnterSafeMode);

    // Phase 1: escalate
    for _ in 0..5 {
        ladder.record_failure("comp", "err");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::Restart);

    // Phase 2: recover
    for _ in 0..2 {
        ladder.record_success("comp");
    }
    // De-escalate one level: Restart → Degrade
    assert_eq!(ladder.level("comp"), EscalationLevel::Degrade);

    for _ in 0..2 {
        ladder.record_success("comp");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::Warn);

    for _ in 0..2 {
        ladder.record_success("comp");
    }
    assert_eq!(ladder.level("comp"), EscalationLevel::Normal);

    // Phase 3: fresh failure cycle starts from scratch
    ladder.record_failure("comp", "err");
    assert_eq!(ladder.level("comp"), EscalationLevel::Warn);
    assert_eq!(
        policy.evaluate("comp", ladder.failure_count("comp")),
        RecoveryAction::LogWarning("warn".into()),
    );
}

/// ProcessMonitor evaluates combined memory + thread alerts correctly.
#[test]
fn integration_process_monitor_combined_alerts() {
    let monitor = ProcessMonitor::new(ProcessMonitorConfig {
        memory_warn_bytes: 100,
        memory_critical_bytes: 500,
        thread_warn_count: 10,
        thread_critical_count: 50,
    });

    // Normal
    let alert = monitor.evaluate(&ProcessSnapshot {
        memory_bytes: 50,
        thread_count: 5,
        uptime: Duration::from_secs(1),
    });
    assert_eq!(alert.severity, ProcessAlert::Normal);

    // Memory warning + thread normal → Warning
    let alert = monitor.evaluate(&ProcessSnapshot {
        memory_bytes: 200,
        thread_count: 5,
        uptime: Duration::from_secs(1),
    });
    assert_eq!(alert.severity, ProcessAlert::Warning);

    // Memory critical + thread critical → Critical
    let alert = monitor.evaluate(&ProcessSnapshot {
        memory_bytes: 600,
        thread_count: 60,
        uptime: Duration::from_secs(1),
    });
    assert_eq!(alert.severity, ProcessAlert::Critical);
    assert!(alert.messages.len() >= 2, "should have messages for both resources");
}

/// HealthCheckManager summary counts are accurate across mixed states.
#[test]
fn integration_health_check_summary_accuracy() {
    let mut mgr = HealthCheckManager::new();
    mgr.register("a", Duration::from_secs(5), 3);
    mgr.register("b", Duration::from_secs(5), 3);
    mgr.register("c", Duration::from_secs(5), 3);
    mgr.register("d", Duration::from_secs(5), 3);

    mgr.report_healthy("a");
    mgr.report_degraded("b", "slow");
    mgr.report_unhealthy("c", "dead");
    // d stays default (Healthy)

    let summary = mgr.summary();
    assert_eq!(summary.healthy, 2); // a, d
    assert_eq!(summary.degraded, 1); // b
    assert_eq!(summary.unhealthy, 1); // c

    assert!(!mgr.is_all_healthy());

    let unhealthy_names: Vec<_> = mgr
        .unhealthy_checks()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(unhealthy_names, vec!["c"]);
}
