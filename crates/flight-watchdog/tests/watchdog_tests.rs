// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! External test suite for `flight-watchdog`.
//!
//! Exercises the public API with scenario-level tests that are complementary
//! to the inline unit tests in `src/tests.rs` and the integration scenarios in
//! `tests/watchdog_integration.rs`.  Covers:
//!
//! 1. USB success resets the consecutive-failure counter
//! 2. Each USB failure increments the miss counter
//! 3. Configurable failure threshold controls when quarantine is triggered
//! 4. Unregistering and re-registering a component gives it a fresh state
//! 5. Components of different types are truly independent
//! 6. `clear_all_state` leaves the system safe for new events
//! 7. Custom execution budget per plugin type
//! 8. Quarantine-status lifecycle: Active → Quarantined → Recovering
//! 9. External state query reflects current component health
//! 10. proptest: arbitrary tick/failure sequences stay in a valid state

use flight_watchdog::{ComponentType, QuarantineStatus, WatchdogConfig, WatchdogSystem};
use std::time::Duration;

// ── 1. Healthy tick resets the watchdog timer ────────────────────────────────

/// `record_usb_success` clears the consecutive-failure count so that a
/// subsequent failure sequence must restart from 0 before quarantining.
#[test]
fn usb_success_resets_consecutive_failure_counter() {
    let mut wd = WatchdogSystem::new();
    let id = "ep_reset";
    let comp = ComponentType::UsbEndpoint(id.to_string());

    // threshold = 3; after 2 errors a success should reset the window.
    wd.register_component(
        comp.clone(),
        WatchdogConfig {
            max_consecutive_failures: 3,
            ..WatchdogConfig::default()
        },
    );

    // Two failures — below the quarantine threshold.
    wd.record_usb_error(id, "err");
    wd.record_usb_error(id, "err");
    assert!(!wd.is_quarantined(&comp), "should not be quarantined yet");

    // Success: resets consecutive-failure window.
    wd.record_usb_success(id);

    // Two more failures — still below the (now-reset) threshold.
    wd.record_usb_error(id, "err");
    wd.record_usb_error(id, "err");
    assert!(
        !wd.is_quarantined(&comp),
        "component must not be quarantined after success reset the counter"
    );
}

/// After `record_usb_success` the USB endpoint stops being stalled.
#[test]
fn usb_success_clears_stall_state() {
    let mut wd = WatchdogSystem::new();
    let id = "ep_stall";
    let comp = ComponentType::UsbEndpoint(id.to_string());
    wd.register_component(comp.clone(), WatchdogConfig::default());

    // Two stalls (below the 3-stall trigger).
    wd.record_usb_stall(id);
    wd.record_usb_stall(id);

    // Success resets the per-endpoint stall counter.
    wd.reset_usb_stall_counter();

    // After reset a single stall must NOT fire an event (counter is back to 1).
    let event = wd.record_usb_stall(id);
    assert!(
        event.is_none(),
        "stall after reset should not fire immediately (need 3 stalls)"
    );
}

// ── 2. Missed tick increments the miss counter ───────────────────────────────

/// Each USB error must increment the plugin-overrun / failure counters
/// that feed quarantine logic, observable through `get_plugin_overrun_stats`.
#[test]
fn plugin_failure_increments_overrun_counter() {
    let mut wd = WatchdogSystem::new();
    let plugin_id = "counter_plugin";
    let comp = ComponentType::NativePlugin(plugin_id.to_string());

    wd.register_component(
        comp.clone(),
        WatchdogConfig {
            max_consecutive_failures: 100, // won't quarantine during test
            ..WatchdogConfig::default()
        },
    );

    for i in 1u32..=5 {
        wd.record_plugin_execution(plugin_id, Duration::from_millis(10), true);
        let stats = wd
            .get_plugin_overrun_stats(plugin_id)
            .expect("stats must exist");
        assert_eq!(
            stats.total_overruns, i,
            "total_overruns must equal the number of overruns so far"
        );
    }
}

/// Each USB error is reflected in `get_health_summary().recent_usb_errors`.
#[test]
fn usb_errors_accumulate_in_health_summary() {
    let mut wd = WatchdogSystem::new();
    let id = "ep_acc";
    wd.register_component(
        ComponentType::UsbEndpoint(id.to_string()),
        WatchdogConfig {
            max_consecutive_failures: 100,
            ..WatchdogConfig::default()
        },
    );

    wd.record_usb_error(id, "e1");
    wd.record_usb_error(id, "e2");
    wd.record_usb_error(id, "e3");

    let summary = wd.get_health_summary();
    assert_eq!(
        summary.recent_usb_errors, 3,
        "all three errors must show up in the summary"
    );
}

// ── 3. N consecutive misses triggers quarantine ──────────────────────────────

/// Threshold = N means the Nth consecutive failure (not the N+1th) quarantines.
#[test]
fn configurable_threshold_triggers_quarantine_at_exact_n() {
    for threshold in [1u32, 2, 4] {
        let mut wd = WatchdogSystem::new();
        let id = format!("ep_th{threshold}");
        let comp = ComponentType::UsbEndpoint(id.to_string());

        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: threshold,
                ..WatchdogConfig::default()
            },
        );

        // threshold - 1 failures: still healthy.
        for _ in 0..threshold.saturating_sub(1) {
            wd.record_usb_error(&id, "err");
        }
        assert!(
            !wd.is_quarantined(&comp),
            "should not yet be quarantined after {}/{}  failures",
            threshold.saturating_sub(1),
            threshold
        );

        // The Nth failure crosses the threshold.
        wd.record_usb_error(&id, "err");
        assert!(
            wd.is_quarantined(&comp),
            "should be quarantined after exactly {threshold} consecutive failures"
        );
    }
}

// ── 4. Watchdog can be disabled and re-enabled ───────────────────────────────

/// After unregistering and re-registering a component the failure counter
/// is reset to zero so quarantine does not trigger prematurely.
#[test]
fn unregister_and_reregister_resets_failure_state() {
    let mut wd = WatchdogSystem::new();
    let id = "ep_rereg";
    let comp = ComponentType::UsbEndpoint(id.to_string());

    wd.register_component(
        comp.clone(),
        WatchdogConfig {
            max_consecutive_failures: 2,
            ..WatchdogConfig::default()
        },
    );

    // One failure — halfway to quarantine.
    wd.record_usb_error(id, "err");
    assert!(!wd.is_quarantined(&comp));

    // Unregister: all state wiped.
    wd.unregister_component(&comp);
    assert!(
        !wd.is_quarantined(&comp),
        "unregistered component must not appear quarantined"
    );

    // Re-register with the same config — fresh state.
    wd.register_component(
        comp.clone(),
        WatchdogConfig {
            max_consecutive_failures: 2,
            ..WatchdogConfig::default()
        },
    );

    // One failure after re-register: below threshold again.
    wd.record_usb_error(id, "err");
    assert!(
        !wd.is_quarantined(&comp),
        "should not be quarantined: failure counter was reset on re-register"
    );
}

/// Fault injection can be enabled and disabled independently of component state.
#[test]
fn fault_injection_can_be_toggled_without_affecting_component_state() {
    let mut wd = WatchdogSystem::new();
    let plugin_id = "plugin_toggle";
    let comp = ComponentType::NativePlugin(plugin_id.to_string());
    wd.register_component(comp.clone(), WatchdogConfig::default());

    assert!(!wd.get_health_summary().fault_injection_enabled);

    wd.enable_fault_injection();
    assert!(wd.get_health_summary().fault_injection_enabled);
    assert!(
        !wd.is_quarantined(&comp),
        "enabling fault injection must not quarantine healthy components"
    );

    wd.disable_fault_injection();
    assert!(!wd.get_health_summary().fault_injection_enabled);
    assert!(
        !wd.is_quarantined(&comp),
        "disabling fault injection must not affect component state"
    );
}

// ── 5. Multiple watchdog channels monitored independently ────────────────────

/// Quarantining a USB endpoint must not change the status of a SimAdapter
/// or a PanelDevice registered at the same time.
#[test]
fn quarantining_one_component_type_leaves_others_unaffected() {
    let mut wd = WatchdogSystem::new();

    let usb_id = "iso_usb";
    let sim_id = "iso_sim";
    let panel_id = "iso_panel";

    let usb = ComponentType::UsbEndpoint(usb_id.to_string());
    let sim = ComponentType::SimAdapter(sim_id.to_string());
    let panel = ComponentType::PanelDevice(panel_id.to_string());

    let strict = WatchdogConfig {
        max_consecutive_failures: 1,
        ..WatchdogConfig::default()
    };
    wd.register_component(usb.clone(), strict);
    wd.register_component(sim.clone(), WatchdogConfig::default());
    wd.register_component(panel.clone(), WatchdogConfig::default());

    // Quarantine the USB endpoint.
    wd.record_usb_error(usb_id, "fatal");
    assert!(wd.is_quarantined(&usb));

    // Other components must remain active.
    assert!(!wd.is_quarantined(&sim), "SimAdapter must be unaffected");
    assert!(!wd.is_quarantined(&panel), "PanelDevice must be unaffected");

    let summary = wd.get_health_summary();
    assert_eq!(summary.total_components, 3);
    assert_eq!(summary.quarantined_components, 1);
    assert_eq!(summary.active_components, 2);
}

// ── 6. Graceful shutdown doesn't fire false alerts ───────────────────────────

/// `clear_all_state` must leave the system in a fully empty state so that
/// new registrations afterwards do not inherit any old quarantine history.
#[test]
fn clear_all_state_followed_by_fresh_registration_has_no_false_quarantine() {
    let mut wd = WatchdogSystem::new();

    // Set up and quarantine some components.
    for i in 0..3u32 {
        let id = format!("ep_shutdown_{i}");
        let comp = ComponentType::UsbEndpoint(id.clone());
        wd.register_component(
            comp,
            WatchdogConfig {
                max_consecutive_failures: 1,
                ..WatchdogConfig::default()
            },
        );
        wd.record_usb_error(&id, "pre-shutdown");
    }
    assert_eq!(wd.get_health_summary().quarantined_components, 3);

    // Graceful shutdown.
    wd.clear_all_state();

    // Post-shutdown state is clean.
    assert_eq!(
        wd.get_health_summary().total_components,
        0,
        "all components must be gone after clear_all_state"
    );
    assert!(
        wd.get_all_events().is_empty(),
        "event history must be empty"
    );
    assert!(wd.get_quarantined_components().is_empty());

    // A fresh registration after clear must start healthy.
    let fresh_id = "ep_fresh";
    let fresh_comp = ComponentType::UsbEndpoint(fresh_id.to_string());
    wd.register_component(fresh_comp.clone(), WatchdogConfig::default());
    assert!(
        !wd.is_quarantined(&fresh_comp),
        "freshly registered component after clear must not be quarantined"
    );
}

// ── 7. Timeout duration is configurable ─────────────────────────────────────

/// A plugin with a tighter execution budget is quarantined by executions
/// that would be fine under a looser budget.
#[test]
fn custom_execution_budget_controls_overrun_detection() {
    let mut wd = WatchdogSystem::new();

    let tight_id = "plugin_tight";
    let loose_id = "plugin_loose";

    wd.register_component(
        ComponentType::NativePlugin(tight_id.to_string()),
        WatchdogConfig {
            max_execution_time: Duration::from_micros(50), // tight: 50 µs
            max_consecutive_failures: 100,
            ..WatchdogConfig::default()
        },
    );
    wd.register_component(
        ComponentType::NativePlugin(loose_id.to_string()),
        WatchdogConfig {
            max_execution_time: Duration::from_millis(5), // loose: 5 ms
            max_consecutive_failures: 100,
            ..WatchdogConfig::default()
        },
    );

    // 200 µs: overruns tight budget but not the loose one.
    let exec_time = Duration::from_micros(200);
    let tight_event = wd.record_plugin_execution(tight_id, exec_time, true);
    let loose_event = wd.record_plugin_execution(loose_id, exec_time, true);

    assert!(
        tight_event.is_some(),
        "tight-budget plugin must report an overrun at 200 µs"
    );
    assert!(
        loose_event.is_none(),
        "loose-budget plugin must not report an overrun at 200 µs"
    );
}

// ── 8. Alert fires exactly once per incident ─────────────────────────────────

/// When a component crosses the quarantine threshold, the quarantined-components
/// list must contain exactly one entry for it, even if subsequent errors
/// continue to arrive.
#[test]
fn quarantined_component_count_stays_at_one_after_further_errors() {
    let mut wd = WatchdogSystem::new();
    let id = "ep_once";
    let comp = ComponentType::UsbEndpoint(id.to_string());

    wd.register_component(
        comp.clone(),
        WatchdogConfig {
            max_consecutive_failures: 2,
            ..WatchdogConfig::default()
        },
    );

    // Drive to quarantine.
    wd.record_usb_error(id, "e1");
    wd.record_usb_error(id, "e2");
    assert!(wd.is_quarantined(&comp));

    let quarantined_after_trigger = wd.get_quarantined_components().len();

    // Additional errors on an already-quarantined component.
    wd.record_usb_error(id, "e3");
    wd.record_usb_error(id, "e4");

    let quarantined_after_extra = wd.get_quarantined_components().len();
    assert_eq!(
        quarantined_after_extra, quarantined_after_trigger,
        "quarantined-components count must not grow beyond 1 for the same component"
    );
}

/// After recovery is initiated, the component no longer appears in the
/// quarantined list, and a second attempt on an already-Recovering component
/// returns `false`.
#[test]
fn recovery_attempt_on_recovering_component_returns_false() {
    let mut wd = WatchdogSystem::new();
    let id = "ep_recover";
    let comp = ComponentType::UsbEndpoint(id.to_string());

    wd.register_component(
        comp.clone(),
        WatchdogConfig {
            max_consecutive_failures: 1,
            ..WatchdogConfig::default()
        },
    );

    // Quarantine.
    wd.record_usb_error(id, "err");
    assert!(wd.is_quarantined(&comp));

    // First recovery attempt: transitions to Recovering.
    let started = wd.attempt_recovery(&comp);
    assert!(started, "first recovery attempt must succeed");
    assert!(
        !wd.is_quarantined(&comp),
        "recovering component must not appear quarantined"
    );

    // Second attempt while already Recovering: must return false (still in window).
    let second = wd.attempt_recovery(&comp);
    assert!(
        !second,
        "second recovery attempt during recovery window must return false"
    );
}

// ── 9. Watch state can be queried externally ─────────────────────────────────

/// `get_quarantine_status` reflects the exact enum variant for each lifecycle phase.
#[test]
fn external_quarantine_status_query_reflects_lifecycle() {
    let mut wd = WatchdogSystem::new();
    let id = "ep_lifecycle";
    let comp = ComponentType::UsbEndpoint(id.to_string());

    wd.register_component(
        comp.clone(),
        WatchdogConfig {
            max_consecutive_failures: 2,
            ..WatchdogConfig::default()
        },
    );

    // Initially Active.
    assert_eq!(
        wd.get_quarantine_status(&comp),
        Some(&QuarantineStatus::Active)
    );

    // After threshold: Quarantined.
    wd.record_usb_error(id, "err");
    wd.record_usb_error(id, "err");
    assert!(
        matches!(
            wd.get_quarantine_status(&comp),
            Some(QuarantineStatus::Quarantined { .. })
        ),
        "status must be Quarantined after threshold exceeded"
    );

    // After recovery attempt: Recovering.
    wd.attempt_recovery(&comp);
    assert!(
        matches!(
            wd.get_quarantine_status(&comp),
            Some(QuarantineStatus::Recovering { .. })
        ),
        "status must be Recovering after attempt_recovery"
    );
}

/// `get_health_summary` correctly counts components across all three states.
#[test]
fn health_summary_counts_match_actual_state() {
    let mut wd = WatchdogSystem::new();

    // Register 5 components.
    let n = 5usize;
    for i in 0..n {
        wd.register_component(
            ComponentType::UsbEndpoint(format!("ep_{i}")),
            WatchdogConfig {
                max_consecutive_failures: 1,
                ..WatchdogConfig::default()
            },
        );
    }

    // Quarantine 2 of them.
    wd.record_usb_error("ep_0", "err");
    wd.record_usb_error("ep_1", "err");

    // Put 1 in recovery.
    let recovering = ComponentType::UsbEndpoint("ep_0".to_string());
    wd.attempt_recovery(&recovering);

    let summary = wd.get_health_summary();
    assert_eq!(
        summary.total_components, n,
        "total must equal registration count"
    );
    // ep_0 is Recovering (not Quarantined), ep_1 is Quarantined, ep_2..4 Active.
    assert_eq!(
        summary.quarantined_components, 1,
        "only ep_1 should be fully quarantined"
    );
    assert_eq!(
        summary.active_components,
        n - 1,
        "active = total - quarantined"
    );
}

/// Component type metadata helpers are consistent.
#[test]
fn component_type_id_and_display_name_are_consistent() {
    let cases: &[(&str, ComponentType)] = &[
        ("my_usb", ComponentType::UsbEndpoint("my_usb".to_string())),
        (
            "my_plugin",
            ComponentType::NativePlugin("my_plugin".to_string()),
        ),
        ("my_wasm", ComponentType::WasmPlugin("my_wasm".to_string())),
        ("my_sim", ComponentType::SimAdapter("my_sim".to_string())),
        (
            "my_panel",
            ComponentType::PanelDevice("my_panel".to_string()),
        ),
        ("my_axis", ComponentType::AxisNode("my_axis".to_string())),
    ];

    for (expected_id, comp) in cases {
        assert_eq!(comp.id(), *expected_id, "id() must return the inner string");
        assert!(
            comp.display_name().contains(expected_id),
            "display_name() must contain the id: '{}'",
            comp.display_name()
        );
    }
}

// ── 10. proptest: any tick/miss sequence stays in a valid state ──────────────

use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Tick, // record_usb_success
    Miss, // record_usb_error
}

fn ops_strategy() -> impl Strategy<Value = Vec<Op>> {
    proptest::collection::vec(prop_oneof![Just(Op::Tick), Just(Op::Miss)], 1..40)
}

proptest! {
    /// For any sequence of successes and errors the system must never report
    /// more quarantined components than registered components.
    #[test]
    fn quarantined_never_exceeds_registered(ops in ops_strategy()) {
        let mut wd = WatchdogSystem::new();
        let id = "prop_ep";
        let comp = ComponentType::UsbEndpoint(id.to_string());

        wd.register_component(
            comp,
            WatchdogConfig {
                max_consecutive_failures: 3,
                ..WatchdogConfig::default()
            },
        );

        for op in &ops {
            match op {
                Op::Tick => wd.record_usb_success(id),
                Op::Miss => { wd.record_usb_error(id, "miss"); }
            }
        }

        let summary = wd.get_health_summary();
        prop_assert!(
            summary.quarantined_components <= summary.total_components,
            "quarantined ({}) must never exceed total ({})",
            summary.quarantined_components,
            summary.total_components
        );
        prop_assert_eq!(
            summary.active_components + summary.quarantined_components,
            summary.total_components,
            "active + quarantined must equal total"
        );
    }

    /// After a successful recovery attempt a component is no longer
    /// in the quarantined list, regardless of how it got there.
    #[test]
    fn recovery_clears_quarantine(n_errors in 1u32..10u32) {
        let mut wd = WatchdogSystem::new();
        let id = "prop_rec";
        let comp = ComponentType::UsbEndpoint(id.to_string());

        wd.register_component(
            comp.clone(),
            WatchdogConfig {
                max_consecutive_failures: 1,
                ..WatchdogConfig::default()
            },
        );

        for _ in 0..n_errors {
            wd.record_usb_error(id, "err");
        }

        prop_assert!(wd.is_quarantined(&comp));

        let started = wd.attempt_recovery(&comp);
        prop_assert!(started, "recovery from Quarantined must always succeed");
        prop_assert!(!wd.is_quarantined(&comp), "component must not be quarantined while Recovering");
    }
}
