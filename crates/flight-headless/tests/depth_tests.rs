// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for `flight-headless`.
//!
//! Covers configuration parsing, service lifecycle, signal handling,
//! PID file management, log output routing, error handling, and
//! property-based validation of the headless runner.

use flight_headless::{HeadlessConfig, HeadlessResult, HeadlessRunner, OutputFormat};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ===========================================================================
// 1. HeadlessConfig — defaults and construction
// ===========================================================================

#[test]
fn config_default_enabled_is_false() {
    let cfg = HeadlessConfig::default();
    assert!(!cfg.enabled, "headless mode should be disabled by default");
}

#[test]
fn config_default_max_duration_is_none() {
    let cfg = HeadlessConfig::default();
    assert!(cfg.max_duration.is_none());
}

#[test]
fn config_default_tick_rate_is_250hz() {
    let cfg = HeadlessConfig::default();
    assert!((cfg.tick_rate_hz - 250.0).abs() < f64::EPSILON);
}

#[test]
fn config_default_fail_fast_is_false() {
    let cfg = HeadlessConfig::default();
    assert!(!cfg.fail_fast);
}

#[test]
fn config_default_output_format_is_text() {
    let cfg = HeadlessConfig::default();
    assert!(matches!(cfg.output_format, OutputFormat::Text));
}

#[test]
fn config_custom_max_duration() {
    let cfg = HeadlessConfig {
        max_duration: Some(Duration::from_secs(60)),
        ..Default::default()
    };
    assert_eq!(cfg.max_duration, Some(Duration::from_secs(60)));
}

#[test]
fn config_custom_tick_rate() {
    let cfg = HeadlessConfig {
        tick_rate_hz: 60.0,
        ..Default::default()
    };
    assert!((cfg.tick_rate_hz - 60.0).abs() < f64::EPSILON);
}

#[test]
fn config_enabled_flag() {
    let cfg = HeadlessConfig {
        enabled: true,
        ..Default::default()
    };
    assert!(cfg.enabled);
}

#[test]
fn config_fail_fast_flag() {
    let cfg = HeadlessConfig {
        fail_fast: true,
        ..Default::default()
    };
    assert!(cfg.fail_fast);
}

#[test]
fn config_clone_preserves_all_fields() {
    let cfg = HeadlessConfig {
        enabled: true,
        max_duration: Some(Duration::from_millis(500)),
        tick_rate_hz: 120.0,
        fail_fast: true,
        output_format: OutputFormat::Json,
    };
    let cloned = cfg.clone();
    assert_eq!(cloned.enabled, cfg.enabled);
    assert_eq!(cloned.max_duration, cfg.max_duration);
    assert!((cloned.tick_rate_hz - cfg.tick_rate_hz).abs() < f64::EPSILON);
    assert_eq!(cloned.fail_fast, cfg.fail_fast);
    assert_eq!(cloned.output_format, cfg.output_format);
}

#[test]
fn config_debug_impl_does_not_panic() {
    let cfg = HeadlessConfig::default();
    let debug = format!("{cfg:?}");
    assert!(!debug.is_empty());
}

// ===========================================================================
// 2. OutputFormat
// ===========================================================================

#[test]
fn output_format_default_is_text() {
    assert_eq!(OutputFormat::default(), OutputFormat::Text);
}

#[test]
fn output_format_eq_same_variant() {
    assert_eq!(OutputFormat::Json, OutputFormat::Json);
    assert_eq!(OutputFormat::Csv, OutputFormat::Csv);
    assert_eq!(OutputFormat::Text, OutputFormat::Text);
}

#[test]
fn output_format_ne_different_variant() {
    assert_ne!(OutputFormat::Json, OutputFormat::Csv);
    assert_ne!(OutputFormat::Text, OutputFormat::Json);
    assert_ne!(OutputFormat::Csv, OutputFormat::Text);
}

#[test]
fn output_format_copy_semantics() {
    let a = OutputFormat::Json;
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn output_format_debug_impl() {
    let s = format!("{:?}", OutputFormat::Json);
    assert!(s.contains("Json"));
}

// ===========================================================================
// 3. HeadlessResult — construction and formatting
// ===========================================================================

#[test]
fn result_success_has_zero_errors_and_exit_code() {
    let r = HeadlessResult::success(500, 2000);
    assert_eq!(r.ticks, 500);
    assert_eq!(r.errors, 0);
    assert_eq!(r.duration_ms, 2000);
    assert_eq!(r.exit_code, 0);
}

#[test]
fn result_success_is_success_true() {
    assert!(HeadlessResult::success(1, 1).is_success());
}

#[test]
fn result_with_errors_nonzero_exit_code() {
    let r = HeadlessResult::with_errors(100, 3, 400);
    assert_eq!(r.exit_code, 1);
    assert!(!r.is_success());
}

#[test]
fn result_with_errors_zero_errors_is_success() {
    let r = HeadlessResult::with_errors(100, 0, 400);
    assert_eq!(r.exit_code, 0);
    assert!(r.is_success());
}

#[test]
fn result_to_text_format() {
    let r = HeadlessResult::success(42, 100);
    let txt = r.to_text();
    assert!(txt.contains("42 ticks"));
    assert!(txt.contains("0 errors"));
    assert!(txt.contains("100ms"));
}

#[test]
fn result_to_json_valid_structure() {
    let r = HeadlessResult::success(10, 20);
    let j = r.to_json();
    assert!(j.starts_with('{'));
    assert!(j.ends_with('}'));
    assert!(j.contains("\"ticks\":10"));
    assert!(j.contains("\"errors\":0"));
    assert!(j.contains("\"duration_ms\":20"));
    assert!(j.contains("\"exit_code\":0"));
}

#[test]
fn result_to_json_with_errors() {
    let r = HeadlessResult::with_errors(5, 2, 50);
    let j = r.to_json();
    assert!(j.contains("\"errors\":2"));
    assert!(j.contains("\"exit_code\":1"));
}

#[test]
fn result_to_csv_format() {
    let r = HeadlessResult::success(7, 14);
    let csv = r.to_csv();
    assert_eq!(csv, "7,0,14,0");
}

#[test]
fn result_to_csv_with_errors() {
    let r = HeadlessResult::with_errors(7, 3, 14);
    let csv = r.to_csv();
    assert_eq!(csv, "7,3,14,1");
}

#[test]
fn result_clone_preserves_fields() {
    let r = HeadlessResult::with_errors(10, 2, 50);
    let c = r.clone();
    assert_eq!(c.ticks, r.ticks);
    assert_eq!(c.errors, r.errors);
    assert_eq!(c.duration_ms, r.duration_ms);
    assert_eq!(c.exit_code, r.exit_code);
}

#[test]
fn result_debug_impl() {
    let r = HeadlessResult::success(1, 1);
    let s = format!("{r:?}");
    assert!(s.contains("HeadlessResult"));
}

#[test]
fn result_zero_ticks_success() {
    let r = HeadlessResult::success(0, 0);
    assert!(r.is_success());
    assert_eq!(r.ticks, 0);
}

#[test]
fn result_large_tick_count() {
    let r = HeadlessResult::success(u64::MAX, u64::MAX);
    assert!(r.is_success());
    assert_eq!(r.ticks, u64::MAX);
}

// ===========================================================================
// 4. HeadlessRunner — lifecycle
// ===========================================================================

#[test]
fn runner_initial_state_not_running() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    assert!(!runner.is_running());
}

#[test]
fn runner_initial_tick_count_zero() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    assert_eq!(runner.tick_count(), 0);
}

#[test]
fn runner_initial_error_count_zero() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    assert_eq!(runner.error_count(), 0);
}

#[test]
fn runner_start_sets_running() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.start();
    assert!(runner.is_running());
}

#[test]
fn runner_stop_clears_running() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.start();
    runner.stop();
    assert!(!runner.is_running());
}

#[test]
fn runner_start_stop_start_cycle() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.start();
    assert!(runner.is_running());
    runner.stop();
    assert!(!runner.is_running());
    runner.start();
    assert!(runner.is_running());
}

#[test]
fn runner_record_tick_increments() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    for _ in 0..10 {
        runner.record_tick();
    }
    assert_eq!(runner.tick_count(), 10);
}

#[test]
fn runner_record_error_increments() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    for _ in 0..5 {
        runner.record_error();
    }
    assert_eq!(runner.error_count(), 5);
}

#[test]
fn runner_config_accessor() {
    let cfg = HeadlessConfig {
        tick_rate_hz: 120.0,
        fail_fast: true,
        ..Default::default()
    };
    let runner = HeadlessRunner::new(cfg);
    assert!((runner.config().tick_rate_hz - 120.0).abs() < f64::EPSILON);
    assert!(runner.config().fail_fast);
}

#[test]
fn runner_build_result_captures_counters() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.record_tick();
    runner.record_tick();
    runner.record_tick();
    runner.record_error();
    let result = runner.build_result(100);
    assert_eq!(result.ticks, 3);
    assert_eq!(result.errors, 1);
    assert_eq!(result.duration_ms, 100);
    assert!(!result.is_success());
}

#[test]
fn runner_build_result_no_errors() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.record_tick();
    let result = runner.build_result(50);
    assert!(result.is_success());
}

#[test]
fn runner_format_result_text() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        output_format: OutputFormat::Text,
        ..Default::default()
    });
    let r = HeadlessResult::success(1, 1);
    let out = runner.format_result(&r);
    assert!(out.contains("ticks"));
}

#[test]
fn runner_format_result_json() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        output_format: OutputFormat::Json,
        ..Default::default()
    });
    let r = HeadlessResult::success(1, 1);
    let out = runner.format_result(&r);
    assert!(out.starts_with('{'));
}

#[test]
fn runner_format_result_csv() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        output_format: OutputFormat::Csv,
        ..Default::default()
    });
    let r = HeadlessResult::success(1, 1);
    let out = runner.format_result(&r);
    assert!(out.contains(','));
    assert!(!out.contains('{'));
}

// ===========================================================================
// 5. Simulated service lifecycle (start → tick → stop → result)
// ===========================================================================

#[test]
fn full_lifecycle_no_errors() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        enabled: true,
        max_duration: Some(Duration::from_millis(100)),
        ..Default::default()
    });
    runner.start();
    assert!(runner.is_running());

    for _ in 0..25 {
        runner.record_tick();
    }

    runner.stop();
    assert!(!runner.is_running());

    let result = runner.build_result(100);
    assert_eq!(result.ticks, 25);
    assert!(result.is_success());
}

#[test]
fn full_lifecycle_with_errors_fail_fast() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        enabled: true,
        fail_fast: true,
        ..Default::default()
    });
    runner.start();

    runner.record_tick();
    runner.record_error();

    // Simulating fail-fast: check error_count and stop early
    if runner.config().fail_fast && runner.error_count() > 0 {
        runner.stop();
    }
    assert!(!runner.is_running());

    let result = runner.build_result(10);
    assert!(!result.is_success());
    assert_eq!(result.errors, 1);
}

#[test]
fn full_lifecycle_many_ticks() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.start();
    for _ in 0..1000 {
        runner.record_tick();
    }
    runner.stop();
    let result = runner.build_result(4000);
    assert_eq!(result.ticks, 1000);
    assert!(result.is_success());
}

// ===========================================================================
// 6. Signal handling simulation (stop flag from another thread)
// ===========================================================================

#[test]
fn signal_stop_from_another_thread() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.start();
    assert!(runner.is_running());

    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&stop_flag);

    let handle = std::thread::spawn(move || {
        flag_clone.store(true, Ordering::SeqCst);
    });

    handle.join().unwrap();
    if stop_flag.load(Ordering::SeqCst) {
        runner.stop();
    }
    assert!(!runner.is_running());
}

#[test]
fn concurrent_tick_recording() {
    let runner = Arc::new(HeadlessRunner::new(HeadlessConfig::default()));
    runner.start();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let r = Arc::clone(&runner);
            std::thread::spawn(move || {
                for _ in 0..100 {
                    r.record_tick();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(runner.tick_count(), 400);
}

#[test]
fn concurrent_error_recording() {
    let runner = Arc::new(HeadlessRunner::new(HeadlessConfig::default()));
    runner.start();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let r = Arc::clone(&runner);
            std::thread::spawn(move || {
                for _ in 0..50 {
                    r.record_error();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(runner.error_count(), 200);
}

// ===========================================================================
// 7. PID file management simulation
// ===========================================================================

#[test]
fn pid_file_write_and_cleanup() {
    let dir = std::env::temp_dir().join("flight_headless_test_pid");
    std::fs::create_dir_all(&dir).unwrap();
    let pid_path = dir.join("flightd.pid");

    let pid = std::process::id();
    std::fs::write(&pid_path, pid.to_string()).unwrap();
    assert!(pid_path.exists());

    let content = std::fs::read_to_string(&pid_path).unwrap();
    assert_eq!(content, pid.to_string());

    std::fs::remove_file(&pid_path).unwrap();
    assert!(!pid_path.exists());

    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn pid_file_overwrite_on_restart() {
    let dir = std::env::temp_dir().join("flight_headless_test_pid_restart");
    std::fs::create_dir_all(&dir).unwrap();
    let pid_path = dir.join("flightd.pid");

    std::fs::write(&pid_path, "12345").unwrap();
    assert_eq!(std::fs::read_to_string(&pid_path).unwrap(), "12345");

    std::fs::write(&pid_path, "67890").unwrap();
    assert_eq!(std::fs::read_to_string(&pid_path).unwrap(), "67890");

    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_dir(&dir);
}

// ===========================================================================
// 8. Log output routing
// ===========================================================================

#[test]
fn format_result_routes_to_text() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        output_format: OutputFormat::Text,
        ..Default::default()
    });
    let r = HeadlessResult::success(10, 100);
    let out = runner.format_result(&r);
    assert!(out.contains("Headless run:"));
}

#[test]
fn format_result_routes_to_json() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        output_format: OutputFormat::Json,
        ..Default::default()
    });
    let r = HeadlessResult::with_errors(10, 2, 100);
    let out = runner.format_result(&r);
    assert!(out.contains("\"ticks\""));
    assert!(out.contains("\"errors\""));
}

#[test]
fn format_result_routes_to_csv() {
    let runner = HeadlessRunner::new(HeadlessConfig {
        output_format: OutputFormat::Csv,
        ..Default::default()
    });
    let r = HeadlessResult::success(10, 100);
    let out = runner.format_result(&r);
    let fields: Vec<&str> = out.split(',').collect();
    assert_eq!(fields.len(), 4);
}

// ===========================================================================
// 9. Error handling edge cases
// ===========================================================================

#[test]
fn result_zero_duration() {
    let r = HeadlessResult::success(0, 0);
    assert!(r.is_success());
    assert_eq!(r.to_csv(), "0,0,0,0");
}

#[test]
fn result_max_u64_errors() {
    let r = HeadlessResult::with_errors(0, u64::MAX, 0);
    assert!(!r.is_success());
    assert_eq!(r.exit_code, 1);
}

#[test]
fn config_zero_tick_rate() {
    let cfg = HeadlessConfig {
        tick_rate_hz: 0.0,
        ..Default::default()
    };
    assert!((cfg.tick_rate_hz).abs() < f64::EPSILON);
}

#[test]
fn config_negative_tick_rate() {
    let cfg = HeadlessConfig {
        tick_rate_hz: -1.0,
        ..Default::default()
    };
    assert!(cfg.tick_rate_hz < 0.0);
}

#[test]
fn runner_stop_when_already_stopped() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.stop();
    assert!(!runner.is_running());
}

#[test]
fn runner_start_when_already_started() {
    let runner = HeadlessRunner::new(HeadlessConfig::default());
    runner.start();
    runner.start(); // double start
    assert!(runner.is_running());
}

// ===========================================================================
// 10. Property-based tests (proptest)
// ===========================================================================

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn result_success_always_exit_code_zero(ticks in 0u64..1_000_000, dur in 0u64..1_000_000) {
            let r = HeadlessResult::success(ticks, dur);
            prop_assert_eq!(r.exit_code, 0);
            prop_assert!(r.is_success());
            prop_assert_eq!(r.errors, 0);
        }

        #[test]
        fn result_with_nonzero_errors_fails(
            ticks in 0u64..1_000_000,
            errors in 1u64..1_000_000,
            dur in 0u64..1_000_000,
        ) {
            let r = HeadlessResult::with_errors(ticks, errors, dur);
            prop_assert!(!r.is_success());
            prop_assert_eq!(r.exit_code, 1);
        }

        #[test]
        fn result_with_zero_errors_succeeds(
            ticks in 0u64..1_000_000,
            dur in 0u64..1_000_000,
        ) {
            let r = HeadlessResult::with_errors(ticks, 0, dur);
            prop_assert!(r.is_success());
            prop_assert_eq!(r.exit_code, 0);
        }

        #[test]
        fn to_json_contains_all_fields(
            ticks in 0u64..100_000,
            errors in 0u64..100_000,
            dur in 0u64..100_000,
        ) {
            let r = HeadlessResult::with_errors(ticks, errors, dur);
            let j = r.to_json();
            let ticks_str = format!("\"ticks\":{}", ticks);
            let errors_str = format!("\"errors\":{}", errors);
            let dur_str = format!("\"duration_ms\":{}", dur);
            prop_assert!(j.contains(&ticks_str));
            prop_assert!(j.contains(&errors_str));
            prop_assert!(j.contains(&dur_str));
        }

        #[test]
        fn to_csv_has_four_fields(
            ticks in 0u64..100_000,
            errors in 0u64..100_000,
            dur in 0u64..100_000,
        ) {
            let r = HeadlessResult::with_errors(ticks, errors, dur);
            let csv = r.to_csv();
            let fields: Vec<&str> = csv.split(',').collect();
            prop_assert_eq!(fields.len(), 4);
        }

        #[test]
        fn to_text_contains_tick_count(ticks in 0u64..100_000) {
            let r = HeadlessResult::success(ticks, 0);
            let txt = r.to_text();
            let ticks_str = format!("{} ticks", ticks);
            prop_assert!(txt.contains(&ticks_str));
        }

        #[test]
        fn runner_tick_count_matches_records(n in 0u32..500) {
            let runner = HeadlessRunner::new(HeadlessConfig::default());
            for _ in 0..n {
                runner.record_tick();
            }
            prop_assert_eq!(runner.tick_count(), u64::from(n));
        }

        #[test]
        fn runner_error_count_matches_records(n in 0u32..500) {
            let runner = HeadlessRunner::new(HeadlessConfig::default());
            for _ in 0..n {
                runner.record_error();
            }
            prop_assert_eq!(runner.error_count(), u64::from(n));
        }

        #[test]
        fn runner_build_result_matches_state(
            ticks in 0u32..200,
            errors in 0u32..200,
            dur in 0u64..100_000,
        ) {
            let runner = HeadlessRunner::new(HeadlessConfig::default());
            for _ in 0..ticks {
                runner.record_tick();
            }
            for _ in 0..errors {
                runner.record_error();
            }
            let result = runner.build_result(dur);
            prop_assert_eq!(result.ticks, u64::from(ticks));
            prop_assert_eq!(result.errors, u64::from(errors));
            prop_assert_eq!(result.duration_ms, dur);
        }

        #[test]
        fn config_tick_rate_preserved(hz in 0.1f64..10000.0) {
            let cfg = HeadlessConfig {
                tick_rate_hz: hz,
                ..Default::default()
            };
            let runner = HeadlessRunner::new(cfg);
            prop_assert!((runner.config().tick_rate_hz - hz).abs() < f64::EPSILON);
        }
    }
}
