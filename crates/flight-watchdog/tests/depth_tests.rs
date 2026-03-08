// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the watchdog system.
//!
//! Covers heartbeat monitoring, escalation policy, component lifecycle,
//! safe-mode triggering, resource monitoring, and integration scenarios.
//! All timing is deterministic (no wall-clock sleeps) unless explicitly
//! testing real-time watchdog timer behaviour.

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

    /// Components register and send heartbeats via HealthCheckManager.
    #[test]
    fn components_register_and_send_heartbeats() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("axis", Duration::from_secs(5), 3);
        mgr.register("ffb", Duration::from_secs(5), 3);
        mgr.register("hid", Duration::from_secs(5), 3);

        // All start healthy.
        assert!(mgr.is_all_healthy());
        assert_eq!(mgr.check_status("axis"), Some(&HealthStatus::Healthy));
        assert_eq!(mgr.check_status("ffb"), Some(&HealthStatus::Healthy));
        assert_eq!(mgr.check_status("hid"), Some(&HealthStatus::Healthy));

        // Heartbeats keep them healthy.
        mgr.report_healthy("axis");
        mgr.report_healthy("ffb");
        mgr.report_healthy("hid");
        assert!(mgr.is_all_healthy());
    }

    /// Missing heartbeat leads to Degraded and then Failed states.
    #[test]
    fn missing_heartbeat_leads_to_degraded_then_failed() {
        let mut agg = HealthAggregator::new();
        let cfg = SubsystemCheckConfig::new("axis")
            .with_failure_threshold(3);
        agg.register(cfg);
        agg.report_healthy("axis");

        // One failure → Degraded (below failure_threshold=3).
        agg.report_failure("axis", "heartbeat missed");
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Degraded);

        // Second failure → still Degraded.
        agg.report_failure("axis", "heartbeat missed");
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Degraded);

        // Third failure → Failed (meets threshold).
        agg.report_failure("axis", "heartbeat missed");
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Failed);
    }

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

    /// Property: heartbeats always reset the countdown in HealthCheckManager.
    #[test]
    fn heartbeat_always_resets_failure_countdown() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("x", Duration::from_secs(5), 5);

        for cycle in 0..10 {
            // Accumulate some failures.
            for _ in 0..4 {
                mgr.report_unhealthy("x", &format!("cycle {cycle}"));
            }
            // Heartbeat resets.
            mgr.report_healthy("x");
            assert_eq!(
                mgr.check_status("x"),
                Some(&HealthStatus::Healthy),
                "heartbeat must reset to Healthy on cycle {cycle}"
            );
        }
    }
}

// ============================================================================
// Module 2: Escalation levels (Warn → Degrade → Restart → SafeMode)
// ============================================================================

mod escalation_tests {
    use super::*;

    fn ladder_factory(warn: u32, degrade: u32, restart: u32, safe: u32) -> EscalationLadder {
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

    /// Warn → Degrade → Restart → SafeMode sequence with default thresholds.
    #[test]
    fn full_escalation_sequence() {
        let config = EscalationConfig {
            restart_cooldown: Duration::ZERO,
            ..EscalationConfig::default()
        };
        let mut ladder = EscalationLadder::new(config.clone());
        ladder.register("comp");

        // Accumulate failures and record levels at each threshold crossing.
        let mut levels = vec![];
        for i in 1..=config.safe_mode_threshold {
            ladder.record_failure("comp", "fail");
            let lvl = ladder.level("comp");
            if levels.last() != Some(&lvl) {
                levels.push(lvl);
            }
            // Early exit once we reach SafeMode.
            if lvl == EscalationLevel::SafeMode && i >= config.safe_mode_threshold {
                break;
            }
        }

        assert!(levels.contains(&EscalationLevel::Warn));
        assert!(levels.contains(&EscalationLevel::Degrade));
        assert!(levels.contains(&EscalationLevel::Restart));
        assert!(levels.contains(&EscalationLevel::SafeMode));
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
        let mut ld = ladder_factory(2, 5, 10, 20);
        ld.record_failure("c", "err");
        // 1 failure, threshold=2 → still Normal
        assert_eq!(ld.level("c"), EscalationLevel::Normal);
        let action = ld.record_failure("c", "err");
        assert!(matches!(action, EscalationAction::Warn(_)));
        assert_eq!(ld.level("c"), EscalationLevel::Warn);
    }

    #[test]
    fn degrade_threshold_triggers_degrade() {
        let mut ld = ladder_factory(1, 3, 10, 20);
        for _ in 0..3 {
            ld.record_failure("c", "err");
        }
        assert_eq!(ld.level("c"), EscalationLevel::Degrade);
    }

    #[test]
    fn restart_threshold_triggers_restart() {
        let mut ld = ladder_factory(1, 3, 5, 20);
        for _ in 0..5 {
            ld.record_failure("c", "err");
        }
        assert_eq!(ld.level("c"), EscalationLevel::Restart);
        assert_eq!(ld.restart_count("c"), 1);
    }

    #[test]
    fn safe_mode_threshold_triggers_safe_mode() {
        let mut ld = ladder_factory(1, 3, 5, 8);
        for _ in 0..8 {
            ld.record_failure("c", "err");
        }
        assert_eq!(ld.level("c"), EscalationLevel::SafeMode);
    }

    #[test]
    fn max_restarts_exceeded_triggers_safe_mode() {
        let mut ld = ladder_factory(1, 3, 5, 100);
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
        let mut ld = ladder_factory(1, 3, 5, 8);
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
        let mut ld = ladder_factory(1, 5, 10, 20);
        for _ in 0..5 {
            ld.record_failure("a", "err");
        }
        ld.record_failure("b", "err");
        assert_eq!(ld.level("a"), EscalationLevel::Degrade);
        assert_eq!(ld.level("b"), EscalationLevel::Warn);
    }

    #[test]
    fn reset_clears_component_state() {
        let mut ld = ladder_factory(1, 3, 5, 8);
        for _ in 0..5 {
            ld.record_failure("r", "err");
        }
        assert_eq!(ld.level("r"), EscalationLevel::Restart);
        ld.reset("r");
        assert_eq!(ld.level("r"), EscalationLevel::Normal);
        assert_eq!(ld.failure_count("r"), 0);
        assert_eq!(ld.restart_count("r"), 0);
    }

    #[test]
    fn multiple_components_independent_escalation() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 3,
            restart_threshold: 6,
            safe_mode_threshold: 12,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 5,
        });

        ladder.register("axis");
        ladder.register("ffb");
        ladder.register("hid");

        // axis → Warn (1 failure).
        ladder.record_failure("axis", "miss");
        // ffb → Degrade (3 failures).
        for _ in 0..3 {
            ladder.record_failure("ffb", "overrun");
        }
        // hid stays Normal.

        assert_eq!(ladder.level("axis"), EscalationLevel::Warn);
        assert_eq!(ladder.level("ffb"), EscalationLevel::Degrade);
        assert_eq!(ladder.level("hid"), EscalationLevel::Normal);
    }
}

// ============================================================================
// Module 3: Component health tracking and lifecycle
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

    // --- Component lifecycle tests ---

    #[test]
    fn register_monitor_unregister() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::UsbEndpoint("ep1".into());

        wd.register_component(comp.clone(), WatchdogConfig::default());
        assert_eq!(wd.get_quarantine_status(&comp), Some(&QuarantineStatus::Active));

        // Monitor via heartbeat.
        wd.record_usb_success("ep1");
        assert!(!wd.is_quarantined(&comp));

        // Unregister.
        wd.unregister_component(&comp);
        assert_eq!(wd.get_quarantine_status(&comp), None);
    }

    #[test]
    fn double_register_is_idempotent() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::UsbEndpoint("ep_double".into());

        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: 1,
                ..WatchdogConfig::default()
            },
        );

        // Quarantine it.
        wd.record_usb_error("ep_double", "err");
        assert!(wd.is_quarantined(&comp));

        // Re-register overwrites state.
        wd.register_component(comp.clone(), WatchdogConfig::default());
        assert_eq!(wd.get_quarantine_status(&comp), Some(&QuarantineStatus::Active));
        assert!(!wd.is_quarantined(&comp));
    }

    #[test]
    fn unregister_during_escalation_cleans_up() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::UsbEndpoint("ep_esc".into());

        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: 1,
                ..WatchdogConfig::default()
            },
        );
        wd.record_usb_error("ep_esc", "fatal");
        assert!(wd.is_quarantined(&comp));

        wd.unregister_component(&comp);
        assert!(!wd.is_quarantined(&comp));
        assert_eq!(wd.get_quarantine_status(&comp), None);

        // Health summary no longer counts it.
        let summary = wd.get_health_summary();
        assert_eq!(summary.total_components, 0);
        assert_eq!(summary.quarantined_components, 0);
    }
}

// ============================================================================
// Module 4: Recovery procedures and Safe Mode
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
        policy.add_rule("hid", 3, RecoveryAction::AlertUser("alert".into()));
        policy.add_rule("hid", 5, RecoveryAction::RestartComponent("hid".into()));
        policy.add_rule("hid", 10, RecoveryAction::EnterSafeMode);

        assert_eq!(
            policy.evaluate("hid", 1),
            RecoveryAction::LogWarning("warn".into())
        );
        assert_eq!(
            policy.evaluate("hid", 3),
            RecoveryAction::AlertUser("alert".into())
        );
        assert_eq!(
            policy.evaluate("hid", 5),
            RecoveryAction::RestartComponent("hid".into())
        );
        assert_eq!(policy.evaluate("hid", 10), RecoveryAction::EnterSafeMode);
        // Above highest threshold → still EnterSafeMode.
        assert_eq!(policy.evaluate("hid", 100), RecoveryAction::EnterSafeMode);
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
    fn safe_mode_on_critical_threshold() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 2,
            restart_threshold: 3,
            safe_mode_threshold: 5,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        });

        for i in 1..=5 {
            let action = ladder.record_failure("critical", "total failure");
            if i == 5 {
                assert!(
                    matches!(action, EscalationAction::EnterSafeMode(_)),
                    "must trigger safe mode at threshold"
                );
            }
        }
        assert_eq!(ladder.level("critical"), EscalationLevel::SafeMode);
    }

    #[test]
    fn exit_safe_mode_via_reset() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 2,
            restart_threshold: 3,
            safe_mode_threshold: 4,
            recovery_threshold: 2,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        });

        for _ in 0..4 {
            ladder.record_failure("sys", "fail");
        }
        assert_eq!(ladder.level("sys"), EscalationLevel::SafeMode);

        ladder.reset("sys");
        assert_eq!(ladder.level("sys"), EscalationLevel::Normal);
        assert_eq!(ladder.failure_count("sys"), 0);
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
// Module 5: Timeout behavior and Resource monitoring
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
    fn dead_man_switch_triggers_on_threshold() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 2,
        });
        std::thread::sleep(Duration::from_millis(10));
        let status = dms.check();
        assert!(matches!(status, DeadManStatus::Triggered { .. }), "should trigger after exceeding threshold");
        assert_eq!(dms.total_triggers(), 1);
    }

    #[test]
    fn memory_pressure_detection() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: 256 * 1024 * 1024,      // 256 MB
            memory_critical_bytes: 512 * 1024 * 1024, // 512 MB
            thread_warn_count: 1000,
            thread_critical_count: 2000,
        });

        // Normal.
        let normal = monitor.evaluate(&ProcessSnapshot {
            memory_bytes: 100 * 1024 * 1024,
            thread_count: 10,
            uptime: Duration::from_secs(60),
        });
        assert_eq!(normal.severity, ProcessAlert::Normal);

        // Warning.
        let warning = monitor.evaluate(&ProcessSnapshot {
            memory_bytes: 300 * 1024 * 1024,
            thread_count: 10,
            uptime: Duration::from_secs(60),
        });
        assert_eq!(warning.severity, ProcessAlert::Warning);

        // Critical.
        let critical = monitor.evaluate(&ProcessSnapshot {
            memory_bytes: 600 * 1024 * 1024,
            thread_count: 10,
            uptime: Duration::from_secs(60),
        });
        assert_eq!(critical.severity, ProcessAlert::Critical);
    }

    #[test]
    fn thread_count_limits() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: u64::MAX,
            memory_critical_bytes: u64::MAX,
            thread_warn_count: 50,
            thread_critical_count: 100,
        });

        let normal = monitor.evaluate(&ProcessSnapshot {
            memory_bytes: 0,
            thread_count: 20,
            uptime: Duration::from_secs(1),
        });
        assert_eq!(normal.severity, ProcessAlert::Normal);

        let warning = monitor.evaluate(&ProcessSnapshot {
            memory_bytes: 0,
            thread_count: 75,
            uptime: Duration::from_secs(1),
        });
        assert_eq!(warning.severity, ProcessAlert::Warning);

        let critical = monitor.evaluate(&ProcessSnapshot {
            memory_bytes: 0,
            thread_count: 150,
            uptime: Duration::from_secs(1),
        });
        assert_eq!(critical.severity, ProcessAlert::Critical);
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
    fn adapter_disconnect_triggers_warning() {
        let mut mon = SystemMonitor::default();
        mon.register_adapter("msfs");
        mon.report_adapter_disconnected("msfs");
        assert_eq!(mon.mode(), SystemMode::Warning);
    }

    #[test]
    fn memory_over_budget_escalates() {
        let mut mon = SystemMonitor::new(MonitorConfig {
            rt_memory_budget_bytes: 1024,
            ..MonitorConfig::default()
        });
        mon.report_rt_memory(2048);
        assert_eq!(mon.mode(), SystemMode::Warning);
    }
}

// ============================================================================
// Module 7: Integration scenarios
// ============================================================================

mod integration {
    use super::*;

    /// Full lifecycle: start watchdog → register 3 components → one fails →
    /// escalation → recovery.
    #[test]
    fn full_lifecycle_register_fail_recover() {
        let mut wd = WatchdogSystem::new();

        // Register 3 components.
        let axis = ComponentType::UsbEndpoint("axis_ep".into());
        let ffb = ComponentType::NativePlugin("ffb_plugin".into());
        let hid = ComponentType::UsbEndpoint("hid_ep".into());

        let config = WatchdogConfig {
            max_consecutive_failures: 2,
            ..WatchdogConfig::default()
        };
        wd.register_component(axis.clone(), config.clone());
        wd.register_component(ffb.clone(), WatchdogConfig {
            max_consecutive_failures: 2,
            max_execution_time: Duration::from_micros(100),
            ..WatchdogConfig::default()
        });
        wd.register_component(hid.clone(), config);

        let summary = wd.get_health_summary();
        assert_eq!(summary.total_components, 3);
        assert_eq!(summary.active_components, 3);
        assert_eq!(summary.quarantined_components, 0);

        // HID fails twice → quarantined.
        wd.record_usb_error("hid_ep", "disconnect");
        wd.record_usb_error("hid_ep", "disconnect");
        assert!(wd.is_quarantined(&hid));

        let summary = wd.get_health_summary();
        assert_eq!(summary.quarantined_components, 1);

        // Initiate recovery.
        assert!(wd.attempt_recovery(&hid));
        assert!(!wd.is_quarantined(&hid));
        assert!(matches!(
            wd.get_quarantine_status(&hid),
            Some(QuarantineStatus::Recovering { .. })
        ));

        // After recovery, system should show 0 quarantined.
        let summary = wd.get_health_summary();
        assert_eq!(summary.quarantined_components, 0);
    }
}

// ============================================================================
// Module 8: Error handling and edge cases
// ============================================================================

mod error_handling {
    use super::*;

    #[test]
    fn record_failure_auto_registers_component() {
        let mut ld = EscalationLadder::default();
        ld.record_failure("auto", "err");
        assert_eq!(ld.failure_count("auto"), 1);
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
    }

    #[test]
    fn component_type_display_name() {
        assert!(ComponentType::UsbEndpoint("ep1".into()).display_name().contains("USB"));
        assert!(ComponentType::NativePlugin("p1".into()).display_name().contains("Native"));
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

mod property_tests {
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
        fn resource_severity_monotonic(
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
    }
}
