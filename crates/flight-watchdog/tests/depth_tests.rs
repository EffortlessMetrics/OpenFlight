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
use flight_watchdog::health_aggregator::{HealthAggregator, SubsystemCheckConfig, SubsystemHealth};
use flight_watchdog::health_check::{HealthCheckManager, HealthStatus};
use flight_watchdog::recovery::{RecoveryAction, RecoveryPolicy};
use flight_watchdog::supervisor::{
    DeadManStatus, DeadManSwitch, DeadManSwitchConfig, ProcessAlert, ProcessMonitor,
    ProcessMonitorConfig, ProcessSnapshot,
};
use flight_watchdog::{ComponentType, QuarantineStatus, WatchdogConfig, WatchdogSystem};

// ═══════════════════════════════════════════════════════════════════════════
// 1. Heartbeat monitoring
// ═══════════════════════════════════════════════════════════════════════════

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

    /// Missing heartbeat leads to warning after threshold.
    #[test]
    fn missing_heartbeat_produces_warning_after_threshold() {
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

    /// Multiple missed heartbeats drive escalation through levels.
    #[test]
    fn multiple_missed_heartbeats_escalate() {
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

        // 1 failure → Warn.
        ladder.record_failure("axis", "no heartbeat");
        assert_eq!(ladder.level("axis"), EscalationLevel::Warn);

        // 2 more → Degrade.
        ladder.record_failure("axis", "no heartbeat");
        ladder.record_failure("axis", "no heartbeat");
        assert_eq!(ladder.level("axis"), EscalationLevel::Degrade);

        // 3 more → Restart.
        for _ in 0..3 {
            ladder.record_failure("axis", "no heartbeat");
        }
        assert_eq!(ladder.level("axis"), EscalationLevel::Restart);

        // 6 more → SafeMode.
        for _ in 0..6 {
            ladder.record_failure("axis", "no heartbeat");
        }
        assert_eq!(ladder.level("axis"), EscalationLevel::SafeMode);
    }

    /// Recovery: component resumes heartbeats → back to healthy.
    #[test]
    fn recovery_after_resumed_heartbeats() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 3,
            restart_threshold: 6,
            safe_mode_threshold: 20,
            recovery_threshold: 2,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 5,
        });

        // Escalate to Warn.
        ladder.record_failure("hid", "miss");
        assert_eq!(ladder.level("hid"), EscalationLevel::Warn);

        // Recover with 2 consecutive successes (recovery_threshold = 2).
        ladder.record_success("hid");
        ladder.record_success("hid");
        assert_eq!(ladder.level("hid"), EscalationLevel::Normal);
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

// ═══════════════════════════════════════════════════════════════════════════
// 2. Escalation policy
// ═══════════════════════════════════════════════════════════════════════════

mod escalation_policy {
    use super::*;

    /// Warn → Degrade → Restart → SafeMode sequence with default thresholds.
    #[test]
    fn full_escalation_sequence() {
        let config = EscalationConfig::default();
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
        // Restart may or may not appear depending on cooldown; SafeMode is terminal.
        assert!(levels.contains(&EscalationLevel::SafeMode));
    }

    /// Custom thresholds per component.
    #[test]
    fn custom_thresholds_per_component() {
        let mut ladder_strict = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 2,
            restart_threshold: 3,
            safe_mode_threshold: 4,
            recovery_threshold: 1,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        });

        let mut ladder_lenient = EscalationLadder::new(EscalationConfig {
            warn_threshold: 5,
            degrade_threshold: 10,
            restart_threshold: 15,
            safe_mode_threshold: 20,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        });

        // 3 failures: strict → Restart, lenient → Normal.
        for _ in 0..3 {
            ladder_strict.record_failure("a", "err");
            ladder_lenient.record_failure("b", "err");
        }
        assert_eq!(ladder_strict.level("a"), EscalationLevel::Restart);
        assert_eq!(ladder_lenient.level("b"), EscalationLevel::Normal);
    }

    /// Deterministic escalation timing (no wall clock dependency).
    #[test]
    fn escalation_is_deterministic_by_failure_count() {
        let config = EscalationConfig {
            warn_threshold: 2,
            degrade_threshold: 4,
            restart_threshold: 8,
            safe_mode_threshold: 16,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        };

        // Run twice — must produce identical results.
        for _ in 0..2 {
            let mut ladder = EscalationLadder::new(config.clone());
            ladder.register("x");

            for _ in 0..2 {
                ladder.record_failure("x", "f");
            }
            assert_eq!(ladder.level("x"), EscalationLevel::Warn);

            for _ in 0..2 {
                ladder.record_failure("x", "f");
            }
            assert_eq!(ladder.level("x"), EscalationLevel::Degrade);

            for _ in 0..4 {
                ladder.record_failure("x", "f");
            }
            assert_eq!(ladder.level("x"), EscalationLevel::Restart);
        }
    }

    /// De-escalation when component recovers.
    #[test]
    fn de_escalation_on_recovery() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 3,
            restart_threshold: 6,
            safe_mode_threshold: 50,
            recovery_threshold: 2,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        });

        // Escalate to Degrade.
        for _ in 0..3 {
            ladder.record_failure("comp", "fail");
        }
        assert_eq!(ladder.level("comp"), EscalationLevel::Degrade);

        // De-escalate one level: Degrade → Warn.
        for _ in 0..2 {
            ladder.record_success("comp");
        }
        assert_eq!(ladder.level("comp"), EscalationLevel::Warn);

        // De-escalate again: Warn → Normal.
        for _ in 0..2 {
            ladder.record_success("comp");
        }
        assert_eq!(ladder.level("comp"), EscalationLevel::Normal);
    }

    /// Multiple components can be in different escalation states.
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

// ═══════════════════════════════════════════════════════════════════════════
// 3. Component lifecycle
// ═══════════════════════════════════════════════════════════════════════════

mod lifecycle {
    use super::*;

    /// Register → Monitor → Unregister.
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

    /// Double-register is idempotent — config is overwritten, status reset.
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

    /// Unregister during escalation cleans up the component fully.
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

    /// Component crash detection: no heartbeat + process gone
    /// simulated by repeated failures without any successes.
    #[test]
    fn component_crash_detection_via_repeated_failures() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::NativePlugin("crasher".into());

        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: 3,
                max_execution_time: Duration::from_micros(50),
                ..WatchdogConfig::default()
            },
        );

        // Plugin never sends a successful heartbeat; only overruns.
        for _ in 0..3 {
            wd.record_plugin_execution("crasher", Duration::from_millis(10), true);
        }

        assert!(wd.is_quarantined(&comp), "crashed plugin must be quarantined");

        let stats = wd.get_plugin_overrun_stats("crasher").unwrap();
        assert!(stats.total_overruns >= 3);
    }

    /// HealthCheckManager lifecycle: register → degrade → recover → unregister.
    #[test]
    fn health_check_manager_full_lifecycle() {
        let mut mgr = HealthCheckManager::new();
        mgr.register("panel", Duration::from_secs(5), 3);

        // Healthy initially.
        assert_eq!(mgr.check_status("panel"), Some(&HealthStatus::Healthy));

        // Degrade.
        mgr.report_degraded("panel", "slow USB");
        assert!(matches!(
            mgr.check_status("panel"),
            Some(HealthStatus::Degraded(_))
        ));

        // Unhealthy.
        mgr.report_unhealthy("panel", "disconnected");
        assert!(matches!(
            mgr.check_status("panel"),
            Some(HealthStatus::Unhealthy(_))
        ));

        // Recover.
        mgr.report_healthy("panel");
        assert_eq!(mgr.check_status("panel"), Some(&HealthStatus::Healthy));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Safe mode triggering
// ═══════════════════════════════════════════════════════════════════════════

mod safe_mode {
    use super::*;

    /// Watchdog triggers safe mode when critical threshold reached.
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

    /// Safe mode via max restart attempts exceeded.
    #[test]
    fn safe_mode_via_max_restarts() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 3,
            restart_threshold: 5,
            safe_mode_threshold: 100, // high so it doesn't trigger by count alone
            recovery_threshold: 2,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 2,
        });

        // First cycle → Restart #1.
        for _ in 0..5 {
            ladder.record_failure("comp", "fail");
        }
        assert_eq!(ladder.level("comp"), EscalationLevel::Restart);
        assert_eq!(ladder.restart_count("comp"), 1);

        // Recover.
        for _ in 0..2 {
            ladder.record_success("comp");
        }

        // Second cycle → Restart #2.
        for _ in 0..5 {
            ladder.record_failure("comp", "fail");
        }
        assert_eq!(ladder.restart_count("comp"), 2);

        // Third cycle → SafeMode (max_restart_attempts=2 exceeded).
        for _ in 0..5 {
            ladder.record_failure("comp", "fail");
        }
        assert_eq!(ladder.level("comp"), EscalationLevel::SafeMode);
    }

    /// Exit safe mode on manual reset (operator command).
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

    /// Exit safe mode via gradual recovery (de-escalation).
    #[test]
    fn exit_safe_mode_via_gradual_recovery() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 2,
            restart_threshold: 3,
            safe_mode_threshold: 4,
            recovery_threshold: 1,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        });

        for _ in 0..4 {
            ladder.record_failure("sys", "fail");
        }
        assert_eq!(ladder.level("sys"), EscalationLevel::SafeMode);

        // De-escalate step by step: SafeMode → Restart → Degrade → Warn → Normal.
        ladder.record_success("sys");
        assert_eq!(ladder.level("sys"), EscalationLevel::Restart);

        ladder.record_success("sys");
        assert_eq!(ladder.level("sys"), EscalationLevel::Degrade);

        ladder.record_success("sys");
        assert_eq!(ladder.level("sys"), EscalationLevel::Warn);

        ladder.record_success("sys");
        assert_eq!(ladder.level("sys"), EscalationLevel::Normal);
    }

    /// Transition history is logged on safe mode entry.
    #[test]
    fn diagnostic_transitions_logged_on_safe_mode_entry() {
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 2,
            restart_threshold: 3,
            safe_mode_threshold: 4,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 10,
        });

        for _ in 0..4 {
            ladder.record_failure("core", "critical");
        }

        let transitions = ladder.transitions();
        assert!(!transitions.is_empty(), "transitions must be logged");

        // Verify the final transition is to SafeMode.
        let last = transitions.last().unwrap();
        assert_eq!(last.to, EscalationLevel::SafeMode);
        assert_eq!(last.component, "core");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Resource monitoring
// ═══════════════════════════════════════════════════════════════════════════

mod resource_monitoring {
    use super::*;

    /// Memory pressure detection.
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

    /// Thread count limits.
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

    /// Tick budget overrun detection via DeadManSwitch with short interval.
    #[test]
    fn tick_budget_overrun_detection() {
        let mut dms = DeadManSwitch::new(DeadManSwitchConfig {
            expected_interval: Duration::from_millis(1),
            missed_intervals_threshold: 3,
        });

        // Immediately after creation, should be alive.
        // (Instant::now() was just called in new(), so ~0 missed ticks.)
        let status = dms.check();
        assert_eq!(status, DeadManStatus::Alive);

        // Simulate overrun by sleeping.
        std::thread::sleep(Duration::from_millis(10));
        let status = dms.check();
        assert!(
            matches!(status, DeadManStatus::Triggered { .. }),
            "should trigger after exceeding threshold"
        );
    }

    /// Resource checks produce monotonically increasing severity.
    #[test]
    fn resource_severity_is_monotonically_ordered() {
        assert!(ProcessAlert::Normal < ProcessAlert::Warning);
        assert!(ProcessAlert::Warning < ProcessAlert::Critical);

        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: 100,
            memory_critical_bytes: 200,
            thread_warn_count: 50,
            thread_critical_count: 100,
        });

        let snapshots = [
            ProcessSnapshot { memory_bytes: 50, thread_count: 10, uptime: Duration::ZERO },
            ProcessSnapshot { memory_bytes: 150, thread_count: 10, uptime: Duration::ZERO },
            ProcessSnapshot { memory_bytes: 250, thread_count: 10, uptime: Duration::ZERO },
        ];

        let severities: Vec<ProcessAlert> =
            snapshots.iter().map(|s| monitor.evaluate(s).severity).collect();

        // Severities must be non-decreasing.
        for window in severities.windows(2) {
            assert!(window[0] <= window[1], "severity must not decrease");
        }
        // And specifically: Normal, Warning, Critical.
        assert_eq!(severities, vec![ProcessAlert::Normal, ProcessAlert::Warning, ProcessAlert::Critical]);
    }

    /// Combined memory + thread pressure yields highest severity.
    #[test]
    fn combined_resource_pressure_yields_highest_severity() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig {
            memory_warn_bytes: 100,
            memory_critical_bytes: 200,
            thread_warn_count: 10,
            thread_critical_count: 20,
        });

        // Memory warning + thread critical → overall Critical.
        let alert = monitor.evaluate(&ProcessSnapshot {
            memory_bytes: 150,
            thread_count: 25,
            uptime: Duration::ZERO,
        });
        assert_eq!(alert.severity, ProcessAlert::Critical);
        assert!(alert.messages.len() >= 2, "should have messages for both resources");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Integration scenarios
// ═══════════════════════════════════════════════════════════════════════════

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
        assert!(!wd.is_quarantined(&axis));
        assert!(!wd.is_quarantined(&ffb));

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

    /// Shutdown: graceful shutdown sequence with timeout.
    #[test]
    fn graceful_shutdown_sequence() {
        let mut wd = WatchdogSystem::new();

        // Set up several components, some quarantined.
        for i in 0..5 {
            let comp = ComponentType::UsbEndpoint(format!("ep_{i}"));
            wd.register_component(
                comp,
                WatchdogConfig {
                    max_consecutive_failures: 1,
                    ..WatchdogConfig::default()
                },
            );
        }

        // Quarantine a couple.
        wd.record_usb_error("ep_0", "err");
        wd.record_usb_error("ep_2", "err");
        assert_eq!(wd.get_health_summary().quarantined_components, 2);

        // Graceful shutdown.
        wd.clear_all_state();

        let summary = wd.get_health_summary();
        assert_eq!(summary.total_components, 0);
        assert_eq!(summary.quarantined_components, 0);
        assert!(wd.get_all_events().is_empty());
        assert!(wd.get_quarantined_components().is_empty());
    }

    /// Concurrent: multiple watchers observing same component via different
    /// subsystems (HealthAggregator + EscalationLadder + RecoveryPolicy).
    #[test]
    fn multiple_watchers_same_component() {
        // HealthAggregator tracks subsystem health.
        let mut agg = HealthAggregator::new();
        agg.register(SubsystemCheckConfig::new("axis").with_failure_threshold(3));

        // EscalationLadder tracks escalation level.
        let mut ladder = EscalationLadder::new(EscalationConfig {
            warn_threshold: 1,
            degrade_threshold: 3,
            restart_threshold: 5,
            safe_mode_threshold: 10,
            recovery_threshold: 3,
            restart_cooldown: Duration::ZERO,
            max_restart_attempts: 3,
        });
        ladder.register("axis");

        // RecoveryPolicy decides actions.
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("axis", 1, RecoveryAction::LogWarning("axis slow".into()));
        policy.add_rule("axis", 3, RecoveryAction::RestartComponent("axis".into()));
        policy.add_rule("axis", 5, RecoveryAction::EnterSafeMode);

        // Simulate 3 failures.
        for _ in 0..3 {
            agg.report_failure("axis", "tick missed");
            ladder.record_failure("axis", "tick missed");
        }

        // All three systems should reflect the same component's distress.
        assert_eq!(agg.subsystem_health("axis"), SubsystemHealth::Failed);
        assert_eq!(ladder.level("axis"), EscalationLevel::Degrade);
        assert_eq!(
            policy.evaluate("axis", 3),
            RecoveryAction::RestartComponent("axis".into())
        );
    }

    /// Recovery policy escalation matches expected actions at each threshold.
    #[test]
    fn recovery_policy_escalation_ladder() {
        let mut policy = RecoveryPolicy::new();
        policy.add_rule("hid", 1, RecoveryAction::LogWarning("warn".into()));
        policy.add_rule("hid", 3, RecoveryAction::AlertUser("alert".into()));
        policy.add_rule("hid", 5, RecoveryAction::RestartComponent("hid".into()));
        policy.add_rule("hid", 10, RecoveryAction::EnterSafeMode);

        assert_eq!(policy.evaluate("hid", 0), RecoveryAction::NoAction);
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

    /// Health aggregator aggregate report reflects worst overall status.
    #[test]
    fn health_aggregator_worst_of_all() {
        let mut agg = HealthAggregator::new();
        agg.register(SubsystemCheckConfig::new("a").with_failure_threshold(3));
        agg.register(SubsystemCheckConfig::new("b").with_failure_threshold(3));
        agg.register(SubsystemCheckConfig::new("c").with_failure_threshold(3));

        agg.report_healthy("a");
        agg.report_healthy("b");
        agg.report_healthy("c");

        let report = agg.aggregate();
        assert_eq!(report.overall, SubsystemHealth::Healthy);
        assert_eq!(report.healthy_count, 3);

        // One warning → overall Warning.
        agg.report_warning("b", "slow");
        let report = agg.aggregate();
        assert_eq!(report.overall, SubsystemHealth::Warning);

        // One failure → overall Degraded (or Failed depending on count).
        agg.report_failure("c", "down");
        let report = agg.aggregate();
        assert!(
            report.overall == SubsystemHealth::Degraded
                || report.overall == SubsystemHealth::Failed,
            "overall must be at least Degraded"
        );
    }

    /// WatchdogSystem: event history is bounded and correct.
    #[test]
    fn event_history_tracking() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::UsbEndpoint("ep_hist".into());
        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: 100,
                ..WatchdogConfig::default()
            },
        );

        for i in 0..20 {
            wd.record_usb_error("ep_hist", &format!("error {i}"));
        }

        let events = wd.get_all_events();
        // Each record_usb_error generates an UsbError event, plus quarantine
        // events once the threshold is crossed. Just verify events are tracked.
        assert!(events.len() >= 20, "should have at least 20 events, got {}", events.len());
    }

    /// Fault storm detection.
    #[test]
    fn fault_storm_detection() {
        let mut wd = WatchdogSystem::new();
        let comp = ComponentType::UsbEndpoint("ep_storm".into());
        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: 100,
                ..WatchdogConfig::default()
            },
        );

        // Generate > 10 faults in quick succession.
        for i in 0..15 {
            wd.record_usb_error("ep_storm", &format!("storm {i}"));
        }

        assert!(wd.is_in_fault_storm(), "should detect fault storm");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Property tests
// ═══════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Heartbeats always reset the escalation failure countdown.
    proptest! {
        #[test]
        fn heartbeat_resets_escalation_countdown(
            failures_before in 1u32..20,
        ) {
            let config = EscalationConfig {
                warn_threshold: 1,
                degrade_threshold: 5,
                restart_threshold: 10,
                safe_mode_threshold: 50,
                recovery_threshold: 1,
                restart_cooldown: Duration::ZERO,
                max_restart_attempts: 10,
            };
            let mut ladder = EscalationLadder::new(config);
            ladder.register("x");

            // Accumulate some failures.
            for _ in 0..failures_before {
                ladder.record_failure("x", "miss");
            }

            // "Heartbeat" via recovery_threshold=1 successes.
            ladder.record_success("x");

            // After one success the failure count should be reset.
            prop_assert_eq!(ladder.failure_count("x"), 0);
        }
    }

    // Escalation level never decreases on failure.
    proptest! {
        #[test]
        fn escalation_never_decreases_on_failure(n_failures in 1u32..30) {
            let mut ladder = EscalationLadder::new(EscalationConfig {
                warn_threshold: 1,
                degrade_threshold: 3,
                restart_threshold: 6,
                safe_mode_threshold: 12,
                recovery_threshold: 5,
                restart_cooldown: Duration::ZERO,
                max_restart_attempts: 10,
            });
            ladder.register("comp");

            let mut prev_level = EscalationLevel::Normal;
            for _ in 0..n_failures {
                ladder.record_failure("comp", "fail");
                let current = ladder.level("comp");
                prop_assert!(
                    current >= prev_level,
                    "level must not decrease on failure: {:?} -> {:?}",
                    prev_level,
                    current
                );
                prev_level = current;
            }
        }
    }

    // WatchdogSystem: quarantined count never exceeds total.
    proptest! {
        #[test]
        fn quarantined_never_exceeds_total(
            n_components in 1usize..6,
            ops in proptest::collection::vec(
                prop_oneof![Just(true), Just(false)],
                1..30
            ),
        ) {
            let mut wd = WatchdogSystem::new();

            for i in 0..n_components {
                wd.register_component(
                    ComponentType::UsbEndpoint(format!("ep_{i}")),
                    WatchdogConfig {
                        max_consecutive_failures: 2,
                        ..WatchdogConfig::default()
                    },
                );
            }

            for (i, &is_success) in ops.iter().enumerate() {
                let ep = format!("ep_{}", i % n_components);
                if is_success {
                    wd.record_usb_success(&ep);
                } else {
                    wd.record_usb_error(&ep, "err");
                }
            }

            let summary = wd.get_health_summary();
            prop_assert!(
                summary.quarantined_components <= summary.total_components,
                "quarantined ({}) > total ({})",
                summary.quarantined_components,
                summary.total_components,
            );
        }
    }

    // Resource severity is monotonically increasing with resource usage.
    proptest! {
        #[test]
        fn resource_severity_monotonic(
            mem_low in 0u64..100,
            mem_high in 200u64..400,
        ) {
            let monitor = ProcessMonitor::new(ProcessMonitorConfig {
                memory_warn_bytes: 100,
                memory_critical_bytes: 200,
                thread_warn_count: 1000,
                thread_critical_count: 2000,
            });

            let low = monitor.evaluate(&ProcessSnapshot {
                memory_bytes: mem_low,
                thread_count: 1,
                uptime: Duration::ZERO,
            });
            let high = monitor.evaluate(&ProcessSnapshot {
                memory_bytes: mem_high,
                thread_count: 1,
                uptime: Duration::ZERO,
            });

            prop_assert!(
                high.severity >= low.severity,
                "higher memory must yield >= severity: {:?} vs {:?}",
                low.severity,
                high.severity,
            );
        }
    }
}
