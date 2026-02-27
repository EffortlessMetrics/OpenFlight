// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for `flight-watchdog`.
//!
//! These tests use only the public API of `WatchdogSystem` and exercise
//! end-to-end scenarios: escalation to quarantine, recovery, independent
//! component isolation, and graceful shutdown/reset.

use flight_watchdog::{
    ComponentType, QuarantineStatus, SyntheticFault, WatchdogConfig, WatchdogEventType,
    WatchdogSystem,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Scenario 1: A service that always fails → escalates to quarantine
// ---------------------------------------------------------------------------

/// USB endpoint that keeps failing should be quarantined after the threshold.
#[test]
fn test_always_failing_usb_service_gets_quarantined() {
    let mut watchdog = WatchdogSystem::new();
    let endpoint_id = "always_failing_usb";
    let component = ComponentType::UsbEndpoint(endpoint_id.to_string());

    let config = WatchdogConfig {
        max_consecutive_failures: 3,
        ..WatchdogConfig::default()
    };
    watchdog.register_component(component.clone(), config);

    // Healthy initially.
    assert!(
        !watchdog.is_quarantined(&component),
        "component should start healthy"
    );
    assert_eq!(
        watchdog.get_health_summary().quarantined_components,
        0,
        "no quarantined components yet"
    );

    // Two failures: below threshold.
    watchdog.record_usb_error(endpoint_id, "connection refused");
    assert!(!watchdog.is_quarantined(&component));
    watchdog.record_usb_error(endpoint_id, "connection refused");
    assert!(!watchdog.is_quarantined(&component));

    // Third consecutive failure: meets threshold → quarantined.
    watchdog.record_usb_error(endpoint_id, "connection refused");
    assert!(
        watchdog.is_quarantined(&component),
        "should be quarantined after exceeding failure threshold"
    );

    let quarantined = watchdog.get_quarantined_components();
    assert!(quarantined.contains(&component));

    let summary = watchdog.get_health_summary();
    assert_eq!(summary.quarantined_components, 1);
}

/// Plugin that keeps overrunning its budget should be quarantined.
#[test]
fn test_always_overrunning_plugin_gets_quarantined() {
    let mut watchdog = WatchdogSystem::new();
    let plugin_id = "stuck_plugin";
    let component = ComponentType::NativePlugin(plugin_id.to_string());

    let config = WatchdogConfig {
        max_consecutive_failures: 3,
        max_execution_time: Duration::from_micros(100),
        ..WatchdogConfig::default()
    };
    watchdog.register_component(component.clone(), config);

    assert!(!watchdog.is_quarantined(&component));

    // Repeated overruns drive consecutive_overruns above the threshold.
    let over_budget = Duration::from_millis(5);
    watchdog.record_plugin_execution(plugin_id, over_budget, true);
    assert!(!watchdog.is_quarantined(&component));
    watchdog.record_plugin_execution(plugin_id, over_budget, true);
    assert!(!watchdog.is_quarantined(&component));
    watchdog.record_plugin_execution(plugin_id, over_budget, true);
    assert!(
        watchdog.is_quarantined(&component),
        "plugin should be quarantined after 3 consecutive overruns"
    );

    let stats = watchdog.get_plugin_overrun_stats(plugin_id).unwrap();
    assert!(stats.total_overruns >= 3);
    assert_eq!(stats.max_execution_time, Some(over_budget));
}

// ---------------------------------------------------------------------------
// Scenario 2: A service that fails then recovers
// ---------------------------------------------------------------------------

/// USB endpoint is quarantined then transitions to Recovering via `attempt_recovery`.
#[test]
fn test_usb_service_fails_then_begins_recovery() {
    let mut watchdog = WatchdogSystem::new();
    let endpoint_id = "transient_usb";
    let component = ComponentType::UsbEndpoint(endpoint_id.to_string());

    let config = WatchdogConfig {
        max_consecutive_failures: 2,
        ..WatchdogConfig::default()
    };
    watchdog.register_component(component.clone(), config);

    // Force quarantine via public API.
    watchdog.record_usb_error(endpoint_id, "timeout");
    watchdog.record_usb_error(endpoint_id, "timeout");
    assert!(watchdog.is_quarantined(&component), "should be quarantined");

    // Initiate recovery.
    let recovery_started = watchdog.attempt_recovery(&component);
    assert!(recovery_started, "recovery should start successfully");

    // Component should be in Recovering state, not quarantined.
    assert!(
        !watchdog.is_quarantined(&component),
        "recovering component must not appear quarantined"
    );

    match watchdog.get_quarantine_status(&component) {
        Some(QuarantineStatus::Recovering { attempt_count, .. }) => {
            assert_eq!(*attempt_count, 1, "first recovery attempt");
        }
        other => panic!("expected Recovering status, got {:?}", other),
    }

    // Health summary no longer shows a quarantined component.
    let summary = watchdog.get_health_summary();
    assert_eq!(summary.quarantined_components, 0);
}

/// After recovery the component can accept normal events without re-quarantining.
#[test]
fn test_plugin_recovery_returns_to_active() {
    let mut watchdog = WatchdogSystem::new();
    let plugin_id = "flaky_plugin";
    let component = ComponentType::NativePlugin(plugin_id.to_string());

    let config = WatchdogConfig {
        max_consecutive_failures: 1,
        ..WatchdogConfig::default()
    };
    watchdog.register_component(component.clone(), config);

    // One overrun → quarantine.
    watchdog.record_plugin_execution(plugin_id, Duration::from_millis(10), true);
    assert!(watchdog.is_quarantined(&component));

    // Begin recovery (moves to Recovering).
    watchdog.attempt_recovery(&component);
    assert!(!watchdog.is_quarantined(&component));

    // Verify Recovering state is set.
    assert!(
        matches!(
            watchdog.get_quarantine_status(&component),
            Some(QuarantineStatus::Recovering { .. })
        ),
        "should be in Recovering state"
    );

    // Watchdog continues to track the plugin: a normal execution produces no event.
    let normal_result =
        watchdog.record_plugin_execution(plugin_id, Duration::from_micros(50), true);
    assert!(
        normal_result.is_none(),
        "normal execution should not trigger events"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Multiple independent watches don't interfere
// ---------------------------------------------------------------------------

/// Quarantining one component must not affect the status of other components.
#[test]
fn test_multiple_independent_watches_no_interference() {
    let mut watchdog = WatchdogSystem::new();

    let usb_id = "usb_ind";
    let plugin_id = "plugin_ind";
    let axis_id = "axis_ind";

    let usb_comp = ComponentType::UsbEndpoint(usb_id.to_string());
    let plugin_comp = ComponentType::NativePlugin(plugin_id.to_string());
    let axis_comp = ComponentType::AxisNode(axis_id.to_string());

    let strict = WatchdogConfig {
        max_consecutive_failures: 1,
        ..WatchdogConfig::default()
    };
    watchdog.register_component(usb_comp.clone(), strict.clone());
    watchdog.register_component(plugin_comp.clone(), strict);
    watchdog.register_component(axis_comp.clone(), WatchdogConfig::default());

    // Quarantine USB endpoint.
    watchdog.record_usb_error(usb_id, "fatal");
    assert!(watchdog.is_quarantined(&usb_comp));
    assert!(
        !watchdog.is_quarantined(&plugin_comp),
        "plugin should still be active"
    );
    assert!(
        !watchdog.is_quarantined(&axis_comp),
        "axis node should still be active"
    );

    // Quarantine plugin (strict: 1 overrun).
    watchdog.record_plugin_execution(plugin_id, Duration::from_millis(10), true);
    assert!(watchdog.is_quarantined(&plugin_comp));
    assert!(
        !watchdog.is_quarantined(&axis_comp),
        "axis node should remain active after plugin quarantine"
    );

    // NaN on axis node — default config is non-critical, so LogOnly (no quarantine).
    let nan_event = watchdog.check_nan_guard(f32::NAN, "pitch_out", axis_comp.clone());
    assert!(nan_event.is_some(), "NaN guard should fire an event");
    assert!(
        !watchdog.is_quarantined(&axis_comp),
        "non-critical NaN should not quarantine the axis node"
    );

    let summary = watchdog.get_health_summary();
    assert_eq!(summary.total_components, 3);
    assert_eq!(summary.quarantined_components, 2);
    assert_eq!(summary.active_components, 1);
}

/// Many components registered and failed independently — quarantine list is accurate.
#[test]
fn test_many_components_independent_failure_tracking() {
    let mut watchdog = WatchdogSystem::new();
    let n = 5usize;

    let strict = WatchdogConfig {
        max_consecutive_failures: 2,
        ..WatchdogConfig::default()
    };

    for i in 0..n {
        let comp = ComponentType::UsbEndpoint(format!("ep{}", i));
        watchdog.register_component(comp, strict.clone());
    }

    // Fail only even-indexed endpoints enough to quarantine them.
    for i in (0..n).step_by(2) {
        let id = format!("ep{}", i);
        watchdog.record_usb_error(&id, "err");
        watchdog.record_usb_error(&id, "err");
    }

    let quarantined = watchdog.get_quarantined_components();
    let expected_quarantined = (n + 1) / 2; // indices 0, 2, 4
    assert_eq!(quarantined.len(), expected_quarantined);

    // Odd-indexed endpoints should still be active.
    for i in (1..n).step_by(2) {
        let comp = ComponentType::UsbEndpoint(format!("ep{}", i));
        assert!(!watchdog.is_quarantined(&comp));
    }
}

// ---------------------------------------------------------------------------
// Scenario 4: Graceful shutdown clears all watches
// ---------------------------------------------------------------------------

/// `clear_all_state` acts as a graceful shutdown: all registrations and event
/// history are discarded, and the health summary reflects an empty system.
#[test]
fn test_graceful_shutdown_clears_all_watches() {
    let mut watchdog = WatchdogSystem::new();

    // Register five endpoints, quarantine each one.
    for i in 0..5 {
        let id = format!("ep_{}", i);
        let comp = ComponentType::UsbEndpoint(id.clone());
        let config = WatchdogConfig {
            max_consecutive_failures: 1,
            ..WatchdogConfig::default()
        };
        watchdog.register_component(comp, config);
        watchdog.record_usb_error(&id, "pre-shutdown error");
    }

    watchdog.enable_fault_injection();

    let pre_summary = watchdog.get_health_summary();
    assert_eq!(pre_summary.total_components, 5);
    assert_eq!(pre_summary.quarantined_components, 5);
    assert!(pre_summary.fault_injection_enabled);
    assert!(!watchdog.get_all_events().is_empty());

    // Graceful shutdown.
    watchdog.clear_all_state();

    let post_summary = watchdog.get_health_summary();
    assert_eq!(post_summary.total_components, 0, "all components cleared");
    assert_eq!(post_summary.quarantined_components, 0);
    assert_eq!(post_summary.active_components, 0);
    assert!(
        watchdog.get_all_events().is_empty(),
        "event history cleared"
    );
    assert_eq!(watchdog.get_quarantined_components().len(), 0);
}

// ---------------------------------------------------------------------------
// Scenario 5: Fault injection isolation across components
// ---------------------------------------------------------------------------

/// Synthetic faults injected for one component do not affect another.
#[test]
fn test_synthetic_fault_isolation_across_components() {
    let mut watchdog = WatchdogSystem::new();

    let plugin_a = "plugin_a";
    let plugin_b = "plugin_b";
    let comp_a = ComponentType::NativePlugin(plugin_a.to_string());
    let comp_b = ComponentType::NativePlugin(plugin_b.to_string());

    watchdog.register_component(comp_a.clone(), WatchdogConfig::default());
    watchdog.register_component(comp_b.clone(), WatchdogConfig::default());
    watchdog.enable_fault_injection();

    // Inject one fault only for comp_a.
    watchdog.inject_synthetic_fault(SyntheticFault {
        component: comp_a.clone(),
        fault_type: WatchdogEventType::PluginOverrun,
        inject_at: std::time::Instant::now(),
        context: "fault for plugin_a".to_string(),
    });

    let events = watchdog.process_synthetic_faults();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].component, comp_a);

    // comp_b should have no events.
    let all = watchdog.get_recent_events(Duration::from_secs(1));
    let b_events: Vec<_> = all.iter().filter(|e| e.component == comp_b).collect();
    assert!(b_events.is_empty(), "comp_b should have no events");
}

/// Disabling fault injection prevents queued faults from firing.
#[test]
fn test_fault_injection_disabled_prevents_faults() {
    let mut watchdog = WatchdogSystem::new();
    let plugin_id = "guarded_plugin";
    let comp = ComponentType::NativePlugin(plugin_id.to_string());

    watchdog.register_component(comp.clone(), WatchdogConfig::default());
    watchdog.enable_fault_injection();
    assert!(watchdog.get_health_summary().fault_injection_enabled);

    watchdog.disable_fault_injection();
    assert!(!watchdog.get_health_summary().fault_injection_enabled);

    // Inject after disabling — silently discarded.
    watchdog.inject_synthetic_fault(SyntheticFault {
        component: comp,
        fault_type: WatchdogEventType::PluginOverrun,
        inject_at: std::time::Instant::now(),
        context: "should be discarded".to_string(),
    });
    let events = watchdog.process_synthetic_faults();
    assert!(events.is_empty(), "no events when injection is disabled");
    assert!(watchdog.get_all_events().is_empty());
}
