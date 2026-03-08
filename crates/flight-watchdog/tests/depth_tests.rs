// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for `flight-watchdog`: heartbeat timing, escalation levels,
//! component health tracking, recovery procedures, timeout behavior, error
//! handling, and property-based tests.

use std::time::Duration;

use flight_watchdog::escalation::{EscalationAction, EscalationConfig, EscalationLadder, EscalationLevel};
use flight_watchdog::health_aggregator::{
    HealthAggregator, SubsystemCheckConfig, SubsystemHealth,
};
use flight_watchdog::health_check::{HealthCheckManager, HealthStatus};
use flight_watchdog::monitor::{HealthEventKind, MonitorConfig, SystemMode, SystemMonitor};
use flight_watchdog::recovery::{RecoveryAction, RecoveryPolicy};
use flight_watchdog::supervisor::{
    DeadManStatus, DeadManSwitch, DeadManSwitchConfig, HardwareWatchdog, ProcessAlert,
    ProcessMonitor, ProcessMonitorConfig, ProcessSnapshot, WatchdogTimerConfig, WatchdogTimerStatus,
};
use flight_watchdog::{
    ComponentType, QuarantineStatus, SyntheticFault, WatchdogConfig, WatchdogEventType,
    WatchdogSystem,
};

// ============================================================================
// Module 1: Heartbeat timing and detection
// ============================================================================

mod heartbeat {
    use super::*;

    #[test]
    fn fresh_monitor_starts_normal() {
        let mon = SystemMonitor::default();
        assert_eq!(mon.mode(), SystemMode::Normal);
        assert_eq!(mon.consecutive_missed_ticks(), 0);
        assert_eq!(mon.total_missed_ticks(), 0);
        assert_eq!(mon.total_received_ticks(), 0);
    }

    #[test]
    fn heartbeat_increments_received_counter() {
        let mut mon = SystemMonitor::default();
        for i in 1..=25 {
            mon.record_heartbeat();
            assert_eq!(mon.total_received_ticks(), i);
        }
    }

    #[test]
    fn missed_tick_increments_counters() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            warn_after_missed_ticks: 100,
            degrade_after_missed_ticks: 200,
            safe_mode_after_missed_ticks: 300,
            ..MonitorConfig::default()
        });
        for i in 1..=10 {
            mon.record_missed_tick();
            assert_eq!(mon.consecutive_missed_ticks(), i);
            assert_eq!(mon.total_missed_ticks(), i as u64);
        }
    }

    #[test]
    fn heartbeat_resets_consecutive_misses() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            warn_after_missed_ticks: 100,
            ..MonitorConfig::default()
        });
        mon.record_missed_tick();
        mon.record_missed_tick();
        mon.record_missed_tick();
        assert_eq!(mon.consecutive_missed_ticks(), 3);
        mon.record_heartbeat();
        assert_eq!(mon.consecutive_missed_ticks(), 0);
        // total_missed_ticks is still preserved
        assert_eq!(mon.total_missed_ticks(), 3);
    }

    #[test]
    fn heartbeat_recovery_emits_event() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            warn_after_missed_ticks: 1,
            ..MonitorConfig::default()
        });
        mon.record_missed_tick();
        mon.record_heartbeat();

        let recovered: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.kind == HealthEventKind::HeartbeatRecovered)
            .collect();
        assert_eq!(recovered.len(), 1);
    }

    #[test]
    fn no_recovery_event_when_already_healthy() {
        let mut mon = SystemMonitor::default();
        mon.record_heartbeat();
        mon.record_heartbeat();
        let recovered: Vec<_> = mon
            .events()
            .iter()
            .filter(|e| e.kind == HealthEventKind::HeartbeatRecovered)
            .collect();
        assert!(recovered.is_empty());
    }

    #[test]
    fn check_heartbeat_timeout_returns_false_when_fresh() {
        let mut mon = SystemMonitor::default();
        mon.record_heartbeat();
        assert!(!mon.check_heartbeat_timeout());
    }

    #[test]
    fn check_heartbeat_timeout_returns_false_when_no_heartbeat_yet() {
        let mut mon = SystemMonitor::default();
        // No heartbeat recorded yet — last_heartbeat is None
        assert!(!mon.check_heartbeat_timeout());
    }

    #[test]
    fn snapshot_reflects_heartbeat_state() {
        let mut mon = SystemMonitor::default();
        for _ in 0..10 {
            mon.record_heartbeat();
        }
        mon.record_missed_tick();
        let snap = mon.snapshot();
        assert_eq!(snap.total_received_ticks, 10);
        assert_eq!(snap.total_missed_ticks, 1);
        assert_eq!(snap.consecutive_missed_ticks, 1);
    }

    #[test]
    fn drain_events_clears_buffer() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            warn_after_missed_ticks: 1,
            ..MonitorConfig::default()
        });
        mon.record_missed_tick();
        let events = mon.drain_events();
        assert!(!events.is_empty());
        assert!(mon.events().is_empty());
    }
}

// ============================================================================
// Module 2: Escalation levels (Warn → Degrade → Restart → SafeMode)
// ============================================================================

mod escalation_tests {
    use super::*;

    fn ladder(warn: u32, degrade: u32, restart: u32, safe: u32) -> EscalationLadder {
        EscalationLadder::new(EscalationConfig {
            warn_threshold: warn,
            degrade_threshold: degrade,
            restart_threshold: restart,
            safe_mode_threshold: safe,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 2,
        })
    }

    #[test]
    fn default_ladder_starts_normal() {
        let mut ld = EscalationLadder::default();
        ld.register("comp");
        assert_eq!(ld.level("comp"), EscalationLevel::Normal);
        assert_eq!(ld.failure_count("comp"), 0);
    }

    #[test]
    fn warn_threshold_triggers_warn_action() {
        let mut ld = ladder(2, 5, 10, 20);
        ld.record_failure("c", "err");
        // 1 failure, threshold=2 → still Normal
        assert_eq!(ld.level("c"), EscalationLevel::Normal);
        let action = ld.record_failure("c", "err");
        assert!(matches!(action, EscalationAction::Warn(_)));
        assert_eq!(ld.level("c"), EscalationLevel::Warn);
    }

    #[test]
    fn degrade_threshold_triggers_degrade() {
        let mut ld = ladder(1, 3, 10, 20);
        for _ in 0..3 {
            ld.record_failure("c", "err");
        }
        assert_eq!(ld.level("c"), EscalationLevel::Degrade);
    }

    #[test]
    fn restart_threshold_triggers_restart() {
        let mut ld = ladder(1, 3, 5, 20);
        for _ in 0..5 {
            ld.record_failure("c", "err");
        }
        assert_eq!(ld.level("c"), EscalationLevel::Restart);
        assert_eq!(ld.restart_count("c"), 1);
    }

    #[test]
    fn safe_mode_threshold_triggers_safe_mode() {
        let mut ld = ladder(1, 3, 5, 8);
        for _ in 0..8 {
            ld.record_failure("c", "err");
        }
        assert_eq!(ld.level("c"), EscalationLevel::SafeMode);
    }

    #[test]
    fn max_restarts_exceeded_triggers_safe_mode() {
        let mut ld = ladder(1, 3, 5, 100);
        // First restart cycle
        for _ in 0..5 {
            ld.record_failure("x", "err");
        }
        assert_eq!(ld.restart_count("x"), 1);
        // Recover
        for _ in 0..3 {
            ld.record_success("x");
        }
        // Second restart cycle
        for _ in 0..5 {
            ld.record_failure("x", "err");
        }
        assert_eq!(ld.restart_count("x"), 2);
        // Now max_restart_attempts=2, further restart-level failures → SafeMode
        for _ in 0..5 {
            ld.record_failure("x", "err");
        }
        assert_eq!(ld.level("x"), EscalationLevel::SafeMode);
    }

    #[test]
    fn escalation_level_ordering_is_correct() {
        assert!(EscalationLevel::Normal < EscalationLevel::Warn);
        assert!(EscalationLevel::Warn < EscalationLevel::Degrade);
        assert!(EscalationLevel::Degrade < EscalationLevel::Restart);
        assert!(EscalationLevel::Restart < EscalationLevel::SafeMode);
    }

    #[test]
    fn escalation_level_display() {
        assert_eq!(EscalationLevel::Normal.to_string(), "Normal");
        assert_eq!(EscalationLevel::SafeMode.to_string(), "SafeMode");
    }

    #[test]
    fn transitions_log_each_level_change() {
        let mut ld = ladder(1, 3, 5, 8);
        for _ in 0..8 {
            ld.record_failure("a", "err");
        }
        let ts = ld.transitions();
        // Normal→Warn, Warn→Degrade, Degrade→Restart, Restart→SafeMode
        assert!(ts.len() >= 4, "expected >=4 transitions, got {}", ts.len());
        assert_eq!(ts[0].from, EscalationLevel::Normal);
        assert_eq!(ts[0].to, EscalationLevel::Warn);
    }

    #[test]
    fn component_isolation_in_ladder() {
        let mut ld = ladder(1, 5, 10, 20);
        for _ in 0..5 {
            ld.record_failure("a", "err");
        }
        ld.record_failure("b", "err");
        assert_eq!(ld.level("a"), EscalationLevel::Degrade);
        assert_eq!(ld.level("b"), EscalationLevel::Warn);
    }

    #[test]
    fn reset_clears_component_state() {
        let mut ld = ladder(1, 3, 5, 8);
        for _ in 0..5 {
            ld.record_failure("r", "err");
        }
        assert_eq!(ld.level("r"), EscalationLevel::Restart);
        ld.reset("r");
        assert_eq!(ld.level("r"), EscalationLevel::Normal);
        assert_eq!(ld.failure_count("r"), 0);
        assert_eq!(ld.restart_count("r"), 0);
    }
}

// ============================================================================
// Module 3: Component health tracking (HealthCheckManager + HealthAggregator)
// ============================================================================

mod health_tracking {
    use super::*;

    #[test]
    fn health_check_manager_register_and_query() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("axis", Duration::from_secs(5), 3);
        assert_eq!(mgr.check_status("axis"), Some(&HealthStatus::Healthy));
    }

    #[test]
    fn degraded_report_tracks_consecutive_failures() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("ffb", Duration::from_secs(5), 3);
        mgr.report_degraded("ffb", "slow");
        mgr.report_degraded("ffb", "slower");
        let summary = mgr.summary();
        assert_eq!(summary.degraded, 1);
    }

    #[test]
    fn unhealthy_report_sets_status() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("hid", Duration::from_secs(5), 3);
        mgr.report_unhealthy("hid", "device lost");
        assert_eq!(
            mgr.check_status("hid"),
            Some(&HealthStatus::Unhealthy("device lost".into()))
        );
    }

    #[test]
    fn healthy_report_resets_failures() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("x", Duration::from_secs(5), 3);
        mgr.report_unhealthy("x", "err");
        mgr.report_unhealthy("x", "err");
        mgr.report_healthy("x");
        assert_eq!(mgr.check_status("x"), Some(&HealthStatus::Healthy));
    }

    #[test]
    fn all_healthy_check() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("a", Duration::from_secs(5), 3);
        mgr.register("b", Duration::from_secs(5), 3);
        assert!(mgr.is_all_healthy());
        mgr.report_degraded("a", "slow");
        assert!(!mgr.is_all_healthy());
    }

    #[test]
    fn summary_counts_all_states() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("a", Duration::from_secs(5), 3);
        mgr.register("b", Duration::from_secs(5), 3);
        mgr.register("c", Duration::from_secs(5), 3);
        mgr.register("d", Duration::from_secs(5), 3);
        mgr.report_degraded("b", "slow");
        mgr.report_unhealthy("c", "dead");
        let s = mgr.summary();
        assert_eq!(s.healthy, 2);
        assert_eq!(s.degraded, 1);
        assert_eq!(s.unhealthy, 1);
    }

    #[test]
    fn unknown_component_returns_none() {
        let mgr = HealthCheckManager::new();
        assert_eq!(mgr.check_status("ghost"), None);
    }

    // --- HealthAggregator tests ---

    #[test]
    fn aggregator_starts_unknown() {
        let mut agg = HealthAggregator::new();
        agg.register(SubsystemCheckConfig::new("axis"));
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Unknown);
    }

    #[test]
    fn aggregator_healthy_report() {
        let mut agg = HealthAggregator::new();
        agg.register(SubsystemCheckConfig::new("axis"));
        agg.report_healthy("axis");
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Healthy);
    }

    #[test]
    fn aggregator_failure_escalation() {
        let mut agg = HealthAggregator::new();
        agg.register(
            SubsystemCheckConfig::new("x").with_failure_threshold(2),
        );
        agg.report_healthy("x");
        agg.report_failure("x", "err");
        assert_eq!(agg.subsystem_health("x"), SubsystemHealth::Degraded);
        agg.report_failure("x", "err");
        assert_eq!(agg.subsystem_health("x"), SubsystemHealth::Failed);
    }

    #[test]
    fn aggregator_overall_worst() {
        let mut agg = HealthAggregator::new();
        agg.register(SubsystemCheckConfig::new("a"));
        agg.register(SubsystemCheckConfig::new("b"));
        agg.report_healthy("a");
        agg.report_healthy("b");
        agg.report_failure("b", "crash");
        let report = agg.aggregate();
        assert_eq!(report.overall, SubsystemHealth::Degraded);
        assert_eq!(report.healthy_count, 1);
        assert_eq!(report.degraded_count, 1);
    }

    #[test]
    fn aggregator_transitions_filtered_by_subsystem() {
        let mut agg = HealthAggregator::new();
        agg.register(SubsystemCheckConfig::new("a"));
        agg.register(SubsystemCheckConfig::new("b"));
        agg.report_healthy("a");
        agg.report_healthy("b");
        agg.report_failure("a", "err");
        let a_trans = agg.transitions_for("a");
        let b_trans = agg.transitions_for("b");
        assert_eq!(a_trans.len(), 2); // Unknown→Healthy, Healthy→Degraded
        assert_eq!(b_trans.len(), 1); // Unknown→Healthy
    }

    #[test]
    fn aggregator_empty_is_healthy() {
        let agg = HealthAggregator::new();
        let report = agg.aggregate();
        assert_eq!(report.overall, SubsystemHealth::Healthy);
        assert_eq!(report.healthy_count, 0);
    }
}

// ============================================================================
// Module 4: Recovery procedures
// ============================================================================

mod recovery_tests {
    use super::*;

    #[test]
    fn no_rule_returns_no_action() {
        let policy = RecoveryPolicy::new();
        assert_eq!(policy.evaluate("x", 10), RecoveryAction::NoAction);
    }

    #[test]
    fn escalating_rules_select_highest_matching() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("hid", 1, RecoveryAction::LogWarning("warn".into()));
        policy.add_rule("hid", 3, RecoveryAction::RestartComponent("hid".into()));
        policy.add_rule("hid", 10, RecoveryAction::EnterSafeMode);

        assert_eq!(
            policy.evaluate("hid", 1),
            RecoveryAction::LogWarning("warn".into())
        );
        assert_eq!(
            policy.evaluate("hid", 5),
            RecoveryAction::RestartComponent("hid".into())
        );
        assert_eq!(policy.evaluate("hid", 15), RecoveryAction::EnterSafeMode);
    }

    #[test]
    fn below_all_thresholds_returns_no_action() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("z", 5, RecoveryAction::EnterSafeMode);
        assert_eq!(policy.evaluate("z", 4), RecoveryAction::NoAction);
    }

    #[test]
    fn per_component_policy_isolation() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("a", 1, RecoveryAction::LogWarning("a-warn".into()));
        policy.add_rule("b", 1, RecoveryAction::AlertUser("b-alert".into()));
        assert_eq!(
            policy.evaluate("a", 1),
            RecoveryAction::LogWarning("a-warn".into())
        );
        assert_eq!(
            policy.evaluate("b", 1),
            RecoveryAction::AlertUser("b-alert".into())
        );
        assert_eq!(policy.evaluate("c", 1), RecoveryAction::NoAction);
    }

    #[test]
    fn rules_for_returns_ordered_subset() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("x", 1, RecoveryAction::LogWarning("w".into()));
        policy.add_rule("y", 2, RecoveryAction::EnterSafeMode);
        policy.add_rule("x", 5, RecoveryAction::RestartComponent("x".into()));
        let rules = policy.rules_for("x");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].on_failure_count, 1);
        assert_eq!(rules[1].on_failure_count, 5);
    }

    #[test]
    fn escalation_ladder_de_escalation_via_success() {
        let mut ld = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 3,
            restart_threshold: 10,
            safe_mode_threshold: 20,
            recovery_threshold: 2,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 5,
        });
        // Escalate to Degrade
        for _ in 0..3 {
            ld.record_failure("comp", "err");
        }
        assert_eq!(ld.level("comp"), EscalationLevel::Degrade);
        // De-escalate: 2 successes → Warn
        for _ in 0..2 {
            ld.record_success("comp");
        }
        assert_eq!(ld.level("comp"), EscalationLevel::Warn);
        // 2 more successes → Normal
        for _ in 0..2 {
            ld.record_success("comp");
        }
        assert_eq!(ld.level("comp"), EscalationLevel::Normal);
    }

    #[test]
    fn watchdog_system_quarantine_and_recovery_lifecycle() {
        let mut wd = WatchdogSystem::new();
        let id = "ep1";
        let comp = ComponentType::UsbEndpoint(id.to_string());
        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: 3,
                ..WatchdogConfig::default()
            },
        );

        // 3 errors → quarantine
        for _ in 0..3 {
            wd.record_usb_error(id, "fail");
        }
        assert!(wd.is_quarantined(&comp));
        assert!(matches!(
            wd.get_quarantine_status(&comp),
            Some(QuarantineStatus::Quarantined { .. })
        ));

        // Attempt recovery → enters Recovering state
        assert!(wd.attempt_recovery(&comp));
        assert!(matches!(
            wd.get_quarantine_status(&comp),
            Some(QuarantineStatus::Recovering { .. })
        ));
    }
}

// ============================================================================
// Module 5: Timeout behavior (supervisor watchdog + dead-man switch)
// ============================================================================

mod timeout_tests {
    use super::*;

    #[test]
    fn hardware_watchdog_ok_immediately_after_creation() {
        let mut wd = HardwareWatchdog::default();
        assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
    }

    #[test]
    fn hardware_watchdog_warns_on_single_timeout() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 5,
        });
        std::thread::sleep(Duration::from_millis(5));
        let status = wd.check();
        assert!(matches!(status, WatchdogTimerStatus::Warning { .. }));
    }

    #[test]
    fn hardware_watchdog_expires_after_max() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 2,
        });
        std::thread::sleep(Duration::from_millis(5));
        wd.check(); // 1st timeout
        let status = wd.check(); // 2nd timeout → expired
        assert!(matches!(status, WatchdogTimerStatus::Expired { .. }));
    }

    #[test]
    fn hardware_watchdog_pet_resets() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 5,
        });
        std::thread::sleep(Duration::from_millis(5));
        wd.check();
        assert!(wd.consecutive_timeouts() > 0);
        wd.pet();
        assert_eq!(wd.consecutive_timeouts(), 0);
        assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
    }

    #[test]
    fn hardware_watchdog_disabled_always_ok() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 1,
        });
        wd.set_enabled(false);
        assert!(!wd.is_enabled());
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
    }

    #[test]
    fn hardware_watchdog_re_enable_resets_pet() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_secs(10),
            max_timeouts: 3,
        });
        wd.set_enabled(false);
        wd.set_enabled(true);
        assert!(wd.is_enabled());
        // Should be Ok because re-enabling resets the pet timestamp
        assert_eq!(wd.check(), WatchdogTimerStatus::Ok);
    }

    #[test]
    fn hardware_watchdog_total_timeouts_accumulate() {
        let mut wd = HardwareWatchdog::new(WatchdogTimerConfig {
            timeout: Duration::from_millis(1),
            max_timeouts: 100,
        });
        std::thread::sleep(Duration::from_millis(5));
        wd.check();
        wd.check();
        wd.check();
        assert_eq!(wd.total_timeouts(), 3);
    }

    #[test]
    fn dead_man_switch_alive_when_recently_ticked() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_secs(10),
            missed_intervals_threshold: 5,
        });
        dms.tick();
        assert_eq!(dms.check(), DeadManStatus::Alive);
    }

    #[test]
    fn dead_man_switch_late_on_small_overdue() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 100,
        });
        std::thread::sleep(Duration::from_millis(5));
        let status = dms.check();
        assert!(matches!(status, DeadManStatus::Late { .. }));
    }

    #[test]
    fn dead_man_switch_triggers_on_threshold() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 2,
        });
        std::thread::sleep(Duration::from_millis(10));
        let status = dms.check();
        assert!(matches!(status, DeadManStatus::Triggered { .. }));
        assert_eq!(dms.total_triggers(), 1);
    }

    #[test]
    fn dead_man_switch_reset_recovers() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 2,
        });
        std::thread::sleep(Duration::from_millis(10));
        dms.check();
        assert!(dms.total_triggers() > 0);
        dms.reset();
        assert_eq!(dms.check(), DeadManStatus::Alive);
    }

    #[test]
    fn dead_man_switch_elapsed_since_tick() {
        let dms = DeadManSwitch::default();
        let elapsed = dms.elapsed_since_tick();
        // Should be very small right after creation
        assert!(elapsed < Duration::from_secs(1));
    }
}

// ============================================================================
// Module 6: SystemMonitor escalation chain
// ============================================================================

mod system_escalation {
    use super::*;

    fn monitor(warn: u32, degrade: u32, safe: u32) -> SystemMonitor {
        SystemMonitor::new(MonitorConfig {
            warn_after_missed_ticks: warn,
            degrade_after_missed_ticks: degrade,
            safe_mode_after_missed_ticks: safe,
            ..MonitorConfig::default()
        })
    }

    #[test]
    fn full_escalation_normal_to_safe_mode() {
        let mut mon = monitor(1, 3, 5);
        assert_eq!(mon.mode(), SystemMode::Normal);
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::Warning);
        mon.record_missed_tick();
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::Degraded);
        mon.record_missed_tick();
        mon.record_missed_tick();
        assert_eq!(mon.mode(), SystemMode::SafeMode);
    }

    #[test]
    fn safe_mode_sticky_under_continued_failures() {
        let mut mon = monitor(1, 3, 5);
        for _ in 0..5 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::SafeMode);
        for _ in 0..10 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::SafeMode);
    }

    #[test]
    fn heartbeat_recovery_from_safe_mode() {
        let mut mon = monitor(1, 3, 5);
        for _ in 0..5 {
            mon.record_missed_tick();
        }
        assert_eq!(mon.mode(), SystemMode::SafeMode);
        mon.record_heartbeat();
        assert_eq!(mon.mode(), SystemMode::Normal);
    }

    #[test]
    fn intermittent_miss_recover_does_not_escalate() {
        let mut mon = monitor(1, 3, 10);
        for _ in 0..20 {
            mon.record_missed_tick();
            mon.record_heartbeat();
        }
        // Should never have reached Degraded
        assert!(mon.mode() <= SystemMode::Normal);
    }

    #[test]
    fn adapter_disconnect_triggers_warning() {
        let mut mon = SystemMonitor::default();
        mon.register_adapter("msfs");
        mon.report_adapter_disconnected("msfs");
        assert_eq!(mon.mode(), SystemMode::Warning);
    }

    #[test]
    fn adapter_reconnect_recovers_to_normal() {
        let mut mon = SystemMonitor::default();
        mon.register_adapter("xplane");
        mon.report_adapter_disconnected("xplane");
        assert_eq!(mon.mode(), SystemMode::Warning);
        mon.report_adapter_connected("xplane");
        assert_eq!(mon.mode(), SystemMode::Normal);
    }

    #[test]
    fn memory_over_budget_escalates() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            rt_memory_budget_bytes: 1024,
            ..MonitorConfig::default()
        });
        mon.report_rt_memory(2048);
        assert_eq!(mon.mode(), SystemMode::Warning);
        assert_eq!(mon.current_rt_memory(), 2048);
    }

    #[test]
    fn peak_memory_tracks_maximum() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            rt_memory_budget_bytes: 0,
            ..MonitorConfig::default()
        });
        mon.report_rt_memory(100);
        mon.report_rt_memory(500);
        mon.report_rt_memory(200);
        assert_eq!(mon.peak_rt_memory(), 500);
        assert_eq!(mon.current_rt_memory(), 200);
    }

    #[test]
    fn force_mode_changes_system_mode() {
        let mut mon = SystemMonitor::default();
        mon.force_mode(SystemMode::SafeMode);
        assert_eq!(mon.mode(), SystemMode::SafeMode);
    }

    #[test]
    fn system_mode_ordering() {
        assert!(SystemMode::Normal < SystemMode::Warning);
        assert!(SystemMode::Warning < SystemMode::Degraded);
        assert!(SystemMode::Degraded < SystemMode::SafeMode);
    }
}

// ============================================================================
// Module 7: Error handling and edge cases
// ============================================================================

mod error_handling {
    use super::*;

    #[test]
    fn unregistered_component_level_is_normal() {
        let ld = EscalationLadder::default();
        assert_eq!(ld.level("nonexistent"), EscalationLevel::Normal);
        assert_eq!(ld.failure_count("nonexistent"), 0);
        assert_eq!(ld.restart_count("nonexistent"), 0);
    }

    #[test]
    fn record_failure_auto_registers_component() {
        let mut ld = EscalationLadder::default();
        ld.record_failure("auto", "err");
        assert_eq!(ld.failure_count("auto"), 1);
    }

    #[test]
    fn record_success_auto_registers_component() {
        let mut ld = EscalationLadder::default();
        let level = ld.record_success("auto");
        assert_eq!(level, EscalationLevel::Normal);
    }

    #[test]
    fn reset_nonexistent_component_is_noop() {
        let mut ld = EscalationLadder::default();
        ld.reset("nonexistent"); // should not panic
    }

    #[test]
    fn report_on_unregistered_subsystem_is_noop() {
        let mut agg = HealthAggregator::new();
        agg.report_healthy("ghost");
        agg.report_warning("ghost", "warn");
        agg.report_failure("ghost", "fail");
        // Nothing should panic; subsystem health for ghost is Unknown
        assert_eq!(agg.subsystem_health("ghost"), SubsystemHealth::Unknown);
    }

    #[test]
    fn watchdog_system_nan_guard_on_non_critical() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::AxisNode("axis0".to_string());
        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                enable_nan_guards: true,
                is_critical: false,
                ..WatchdogConfig::default()
            },
        );
        let event = wd.check_nan_guard(f32::NAN, "test_value", comp);
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event_type, WatchdogEventType::NanDetected);
    }

    #[test]
    fn watchdog_system_nan_guard_infinity() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::AxisNode("axis1".to_string());
        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                enable_nan_guards: true,
                is_critical: false,
                ..WatchdogConfig::default()
            },
        );
        let event = wd.check_nan_guard(f32::INFINITY, "inf_value", comp);
        assert!(event.is_some());
    }

    #[test]
    fn watchdog_system_nan_guard_normal_value() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::AxisNode("axis2".to_string());
        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                enable_nan_guards: true,
                ..WatchdogConfig::default()
            },
        );
        let event = wd.check_nan_guard(0.5, "normal_value", comp);
        assert!(event.is_none());
    }

    #[test]
    fn watchdog_system_clear_all_state() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::UsbEndpoint("ep".to_string());
        wd.register_component(comp.clone(), WatchdogConfig::default());
        wd.record_usb_error("ep", "err");
        wd.clear_all_state();
        assert_eq!(wd.get_all_events().len(), 0);
        assert!(wd.get_quarantine_status(&comp).is_none());
    }

    #[test]
    fn watchdog_health_summary_reflects_state() {
        let mut wd = WatchdogSystem::new();
        let comp_a = ComponentType::UsbEndpoint("a".to_string());
        let comp_b = ComponentType::UsbEndpoint("b".to_string());
        wd.register_component(comp_a.clone(), WatchdogConfig::default());
        wd.register_component(comp_b.clone(), WatchdogConfig::default());
        let summary = wd.get_health_summary();
        assert_eq!(summary.total_components, 2);
        assert_eq!(summary.active_components, 2);
        assert_eq!(summary.quarantined_components, 0);
    }

    #[test]
    fn component_type_display_name() {
        assert!(ComponentType::UsbEndpoint("ep1".into()).display_name().contains("USB"));
        assert!(ComponentType::NativePlugin("p1".into()).display_name().contains("Native"));
        assert!(ComponentType::WasmPlugin("w1".into()).display_name().contains("WASM"));
        assert!(ComponentType::SimAdapter("sa".into()).display_name().contains("Sim"));
        assert!(ComponentType::PanelDevice("pd".into()).display_name().contains("Panel"));
        assert!(ComponentType::AxisNode("an".into()).display_name().contains("Axis"));
    }

    #[test]
    fn component_type_id_extraction() {
        let comp = ComponentType::UsbEndpoint("my_ep".to_string());
        assert_eq!(comp.id(), "my_ep");
    }
}

// ============================================================================
// Module 8: Process monitor
// ============================================================================

mod process_monitor_tests {
    use super::*;

    #[test]
    fn default_check_is_normal() {
        let mon = ProcessMonitor::default();
        let alert = mon.check();
        assert_eq!(alert.severity, ProcessAlert::Normal);
        assert!(alert.messages.is_empty());
    }

    #[test]
    fn memory_warning_threshold() {
        let mon = ProcessMonitor::new(ProcessMonitorConfig {
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
        let alert = mon.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Warning);
    }

    #[test]
    fn memory_critical_threshold() {
        let mon = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: 100,
            memory_critical_bytes: 200,
            thread_warn_count: 1000,
            thread_critical_count: 2000,
        });
        let snap = ProcessSnapshot {
            memory_bytes: 250,
            thread_count: 4,
            uptime: Duration::from_secs(1),
        };
        let alert = mon.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Critical);
    }

    #[test]
    fn thread_count_warning() {
        let mon = ProcessMonitor::new(ProcessMonitorConfig {
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
        let alert = mon.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Warning);
    }

    #[test]
    fn thread_count_critical() {
        let mon = ProcessMonitor::new(ProcessMonitorConfig {
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
        let alert = mon.evaluate(&snap);
        assert_eq!(alert.severity, ProcessAlert::Critical);
    }

    #[test]
    fn process_alert_ordering() {
        assert!(ProcessAlert::Normal < ProcessAlert::Warning);
        assert!(ProcessAlert::Warning < ProcessAlert::Critical);
    }

    #[test]
    fn uptime_advances() {
        let mon = ProcessMonitor::default();
        std::thread::sleep(Duration::from_millis(10));
        assert!(mon.uptime() >= Duration::from_millis(10));
    }
}

// ============================================================================
// Module 9: Synthetic fault injection
// ============================================================================

mod fault_injection {
    use super::*;

    #[test]
    fn fault_injection_disabled_by_default() {
        let wd = WatchdogSystem::new();
        let summary = wd.get_health_summary();
        assert!(!summary.fault_injection_enabled);
    }

    #[test]
    fn enable_and_disable_fault_injection() {
        let mut wd = WatchdogSystem::new();
        wd.enable_fault_injection();
        assert!(wd.get_health_summary().fault_injection_enabled);
        wd.disable_fault_injection();
        assert!(!wd.get_health_summary().fault_injection_enabled);
    }

    #[test]
    fn inject_and_process_synthetic_fault() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::NativePlugin("test_plugin".to_string());
        wd.register_component(comp.clone(), WatchdogConfig::default());
        wd.enable_fault_injection();

        let fault = SyntheticFault {
            component: comp.clone(),
            fault_type: WatchdogEventType::PluginOverrun,
            inject_at: std::time::Instant::now(),
            context: "test overrun".to_string(),
        };
        wd.inject_synthetic_fault(fault);

        let events = wd.process_synthetic_faults();
        assert!(!events.is_empty());
        assert_eq!(events[0].event_type, WatchdogEventType::SyntheticFault);
    }
}

// ============================================================================
// Module 10: Property-based tests
// ============================================================================

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn escalation_level_never_exceeds_safe_mode(
            failures in 0u32..100,
        ) {
            let mut ld = EscalationLadder::default();
            for _ in 0..failures {
                ld.record_failure("c", "err");
            }
            assert!(ld.level("c") <= EscalationLevel::SafeMode);
        }

        #[test]
        fn failure_count_tracks_accurately(
            failures in 0u32..50,
        ) {
            let mut ld = EscalationLadder::new(EscalationConfig {
                warn_threshold: 100,
                degrade_threshold: 200,
                restart_threshold: 300,
                safe_mode_threshold: 400,
                recovery_threshold: 5,
                restart_cooldown: Duration::ZERO,
                max_restart_attempts: 10,
            });
            for _ in 0..failures {
                ld.record_failure("c", "err");
            }
            assert_eq!(ld.failure_count("c"), failures);
        }

        #[test]
        fn system_mode_is_always_valid_after_operations(
            misses in 0u32..30,
            recoveries in 0u32..10,
        ) {
            let mut mon = SystemMonitor::new(MonitorConfig {
                warn_after_missed_ticks: 1,
                degrade_after_missed_ticks: 5,
                safe_mode_after_missed_ticks: 20,
                ..MonitorConfig::default()
            });

            for _ in 0..misses {
                mon.record_missed_tick();
            }
            for _ in 0..recoveries {
                mon.record_heartbeat();
            }

            let mode = mon.mode();
            assert!(mode == SystemMode::Normal
                || mode == SystemMode::Warning
                || mode == SystemMode::Degraded
                || mode == SystemMode::SafeMode);
        }

        #[test]
        fn recovery_always_decreases_or_maintains_level(
            initial_failures in 1u32..10,
            successes in 0u32..20,
        ) {
            let mut ld = EscalationLadder::new(EscalationConfig {
                warn_threshold: 1,
                degrade_threshold: 3,
                restart_threshold: 5,
                safe_mode_threshold: 8,
                recovery_threshold: 2,
                restart_cooldown: Duration::ZERO,
                max_restart_attempts: 10,
            });

            for _ in 0..initial_failures {
                ld.record_failure("c", "err");
            }
            let level_after_failures = ld.level("c");

            for _ in 0..successes {
                ld.record_success("c");
            }
            let level_after_recovery = ld.level("c");

            assert!(level_after_recovery <= level_after_failures);
        }

        #[test]
        fn health_check_manager_summary_consistent(
            n_healthy in 0usize..5,
            n_degraded in 0usize..5,
            n_unhealthy in 0usize..5,
        ) {
            let mut mgr = HealthCheckManager::new();
            for i in 0..n_healthy {
                let name = format!("h{i}");
                mgr.register(&name, Duration::from_secs(5), 3);
                // already healthy by default
            }
            for i in 0..n_degraded {
                let name = format!("d{i}");
                mgr.register(&name, Duration::from_secs(5), 3);
                mgr.report_degraded(&name, "slow");
            }
            for i in 0..n_unhealthy {
                let name = format!("u{i}");
                mgr.register(&name, Duration::from_secs(5), 3);
                mgr.report_unhealthy(&name, "dead");
            }

            let s = mgr.summary();
            assert_eq!(s.healthy, n_healthy);
            assert_eq!(s.degraded, n_degraded);
            assert_eq!(s.unhealthy, n_unhealthy);
            assert_eq!(s.healthy + s.degraded + s.unhealthy, n_healthy + n_degraded + n_unhealthy);
        }

        #[test]
        fn process_monitor_severity_monotonic(
            memory in 0u64..1000,
        ) {
            let mon = ProcessMonitor::new(ProcessMonitorConfig {
                memory_warn_bytes: 300,
                memory_critical_bytes: 700,
                thread_warn_count: 1000,
                thread_critical_count: 2000,
            });

            let snap = ProcessSnapshot {
                memory_bytes: memory,
                thread_count: 1,
                uptime: Duration::from_secs(1),
            };

            let alert = mon.evaluate(&snap);
            if memory >= 700 {
                assert_eq!(alert.severity, ProcessAlert::Critical);
            } else if memory >= 300 {
                assert_eq!(alert.severity, ProcessAlert::Warning);
            } else {
                assert_eq!(alert.severity, ProcessAlert::Normal);
            }
        }

        #[test]
        fn aggregate_health_counts_sum_correctly(
            n in 1usize..8,
        ) {
            let mut agg = HealthAggregator::new();
            for i in 0..n {
                agg.register(SubsystemCheckConfig::new(&format!("s{i}")));
                agg.report_healthy(&format!("s{i}"));
            }
            let report = agg.aggregate();
            assert_eq!(report.healthy_count, n);
            assert_eq!(
                report.healthy_count + report.warning_count + report.degraded_count
                    + report.failed_count + report.unknown_count,
                n
            );
        }
    }
}
