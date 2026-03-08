// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-headless.
//!
//! Covers HeadlessConfig, HeadlessResult, HeadlessRunner, and OutputFormat
//! with boundary values, property-based tests, and format round-trip checks.

#[cfg(test)]
mod depth_tests {
    use crate::*;
    use std::sync::Arc;
    use std::time::Duration;

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessConfig
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn config_default_values() {
        let c = HeadlessConfig::default();
        assert!(!c.enabled);
        assert!(c.max_duration.is_none());
        assert_eq!(c.tick_rate_hz, 250.0);
        assert!(!c.fail_fast);
        assert_eq!(c.output_format, OutputFormat::Text);
    }

    #[test]
    fn config_enabled_true() {
        let c = HeadlessConfig {
            enabled: true,
            ..Default::default()
        };
        assert!(c.enabled);
    }

    #[test]
    fn config_max_duration_some() {
        let c = HeadlessConfig {
            max_duration: Some(Duration::from_secs(60)),
            ..Default::default()
        };
        assert_eq!(c.max_duration.unwrap(), Duration::from_secs(60));
    }

    #[test]
    fn config_max_duration_none() {
        let c = HeadlessConfig::default();
        assert!(c.max_duration.is_none());
    }

    #[test]
    fn config_tick_rate_custom() {
        let c = HeadlessConfig {
            tick_rate_hz: 60.0,
            ..Default::default()
        };
        assert_eq!(c.tick_rate_hz, 60.0);
    }

    #[test]
    fn config_fail_fast_true() {
        let c = HeadlessConfig {
            fail_fast: true,
            ..Default::default()
        };
        assert!(c.fail_fast);
    }

    #[test]
    fn config_all_output_formats() {
        for fmt in [OutputFormat::Text, OutputFormat::Json, OutputFormat::Csv] {
            let c = HeadlessConfig {
                output_format: fmt,
                ..Default::default()
            };
            assert_eq!(c.output_format, fmt);
        }
    }

    #[test]
    fn config_clone() {
        let c = HeadlessConfig {
            enabled: true,
            max_duration: Some(Duration::from_secs(30)),
            tick_rate_hz: 100.0,
            fail_fast: true,
            output_format: OutputFormat::Json,
        };
        let c2 = c.clone();
        assert_eq!(c2.enabled, c.enabled);
        assert_eq!(c2.max_duration, c.max_duration);
        assert_eq!(c2.tick_rate_hz, c.tick_rate_hz);
        assert_eq!(c2.fail_fast, c.fail_fast);
        assert_eq!(c2.output_format, c.output_format);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // OutputFormat
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn output_format_default_is_text() {
        assert_eq!(OutputFormat::default(), OutputFormat::Text);
    }

    #[test]
    fn output_format_eq() {
        assert_eq!(OutputFormat::Text, OutputFormat::Text);
        assert_eq!(OutputFormat::Json, OutputFormat::Json);
        assert_eq!(OutputFormat::Csv, OutputFormat::Csv);
        assert_ne!(OutputFormat::Text, OutputFormat::Json);
        assert_ne!(OutputFormat::Json, OutputFormat::Csv);
    }

    #[test]
    fn output_format_copy() {
        let f = OutputFormat::Json;
        let f2 = f; // Copy
        assert_eq!(f, f2);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessResult — constructors
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn result_success_fields() {
        let r = HeadlessResult::success(1000, 4000);
        assert_eq!(r.ticks, 1000);
        assert_eq!(r.errors, 0);
        assert_eq!(r.duration_ms, 4000);
        assert_eq!(r.exit_code, 0);
        assert!(r.is_success());
    }

    #[test]
    fn result_success_zero_ticks() {
        let r = HeadlessResult::success(0, 0);
        assert_eq!(r.ticks, 0);
        assert!(r.is_success());
    }

    #[test]
    fn result_success_max_ticks() {
        let r = HeadlessResult::success(u64::MAX, u64::MAX);
        assert_eq!(r.ticks, u64::MAX);
        assert!(r.is_success());
    }

    #[test]
    fn result_with_errors_nonzero() {
        let r = HeadlessResult::with_errors(500, 3, 2000);
        assert_eq!(r.ticks, 500);
        assert_eq!(r.errors, 3);
        assert_eq!(r.duration_ms, 2000);
        assert_eq!(r.exit_code, 1);
        assert!(!r.is_success());
    }

    #[test]
    fn result_with_errors_zero_errors_is_success() {
        let r = HeadlessResult::with_errors(500, 0, 2000);
        assert_eq!(r.exit_code, 0);
        assert!(r.is_success());
    }

    #[test]
    fn result_with_errors_one_error() {
        let r = HeadlessResult::with_errors(100, 1, 400);
        assert_eq!(r.exit_code, 1);
        assert!(!r.is_success());
    }

    #[test]
    fn result_with_errors_max_errors() {
        let r = HeadlessResult::with_errors(0, u64::MAX, 0);
        assert_eq!(r.exit_code, 1);
        assert!(!r.is_success());
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessResult — output formats
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn result_to_text_format() {
        let r = HeadlessResult::success(100, 400);
        let t = r.to_text();
        assert!(t.contains("100 ticks"));
        assert!(t.contains("0 errors"));
        assert!(t.contains("400ms"));
    }

    #[test]
    fn result_to_text_with_errors() {
        let r = HeadlessResult::with_errors(50, 5, 200);
        let t = r.to_text();
        assert!(t.contains("50 ticks"));
        assert!(t.contains("5 errors"));
    }

    #[test]
    fn result_to_json_format() {
        let r = HeadlessResult::success(100, 400);
        let j = r.to_json();
        assert!(j.starts_with('{'));
        assert!(j.ends_with('}'));
        assert!(j.contains("\"ticks\":100"));
        assert!(j.contains("\"errors\":0"));
        assert!(j.contains("\"duration_ms\":400"));
        assert!(j.contains("\"exit_code\":0"));
    }

    #[test]
    fn result_to_json_with_errors() {
        let r = HeadlessResult::with_errors(50, 3, 200);
        let j = r.to_json();
        assert!(j.contains("\"errors\":3"));
        assert!(j.contains("\"exit_code\":1"));
    }

    #[test]
    fn result_to_json_is_parseable() {
        let r = HeadlessResult::success(100, 400);
        let j = r.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(parsed["ticks"], 100);
        assert_eq!(parsed["errors"], 0);
        assert_eq!(parsed["duration_ms"], 400);
        assert_eq!(parsed["exit_code"], 0);
    }

    #[test]
    fn result_to_csv_format() {
        let r = HeadlessResult::success(100, 400);
        let c = r.to_csv();
        assert_eq!(c, "100,0,400,0");
    }

    #[test]
    fn result_to_csv_with_errors() {
        let r = HeadlessResult::with_errors(50, 5, 200);
        let c = r.to_csv();
        assert_eq!(c, "50,5,200,1");
    }

    #[test]
    fn result_to_csv_zero_everything() {
        let r = HeadlessResult::success(0, 0);
        assert_eq!(r.to_csv(), "0,0,0,0");
    }

    #[test]
    fn result_to_csv_parseable_fields() {
        let r = HeadlessResult::with_errors(42, 7, 1234);
        let csv = r.to_csv();
        let fields: Vec<&str> = csv.split(',').collect();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0].parse::<u64>().unwrap(), 42);
        assert_eq!(fields[1].parse::<u64>().unwrap(), 7);
        assert_eq!(fields[2].parse::<u64>().unwrap(), 1234);
        assert_eq!(fields[3].parse::<i32>().unwrap(), 1);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessResult — clone
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn result_clone() {
        let r = HeadlessResult::with_errors(10, 2, 50);
        let r2 = r.clone();
        assert_eq!(r2.ticks, r.ticks);
        assert_eq!(r2.errors, r.errors);
        assert_eq!(r2.duration_ms, r.duration_ms);
        assert_eq!(r2.exit_code, r.exit_code);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessRunner — creation
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn runner_initial_state() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        assert!(!runner.is_running());
        assert_eq!(runner.tick_count(), 0);
        assert_eq!(runner.error_count(), 0);
    }

    #[test]
    fn runner_custom_config() {
        let config = HeadlessConfig {
            enabled: true,
            tick_rate_hz: 60.0,
            fail_fast: true,
            output_format: OutputFormat::Json,
            max_duration: Some(Duration::from_secs(30)),
        };
        let runner = HeadlessRunner::new(config);
        assert_eq!(runner.config().tick_rate_hz, 60.0);
        assert!(runner.config().fail_fast);
        assert_eq!(runner.config().output_format, OutputFormat::Json);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessRunner — start/stop
    // ═════════════════════════════════════════════════════════════════════════

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
    fn runner_double_start() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        runner.start();
        runner.start(); // idempotent
        assert!(runner.is_running());
    }

    #[test]
    fn runner_double_stop() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        runner.start();
        runner.stop();
        runner.stop(); // idempotent
        assert!(!runner.is_running());
    }

    #[test]
    fn runner_stop_without_start() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        runner.stop(); // should not panic
        assert!(!runner.is_running());
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessRunner — tick/error recording
    // ═════════════════════════════════════════════════════════════════════════

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
    fn runner_mixed_ticks_and_errors() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        runner.record_tick();
        runner.record_tick();
        runner.record_error();
        runner.record_tick();
        assert_eq!(runner.tick_count(), 3);
        assert_eq!(runner.error_count(), 1);
    }

    #[test]
    fn runner_no_records_zero_counts() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        assert_eq!(runner.tick_count(), 0);
        assert_eq!(runner.error_count(), 0);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessRunner — build_result
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn runner_build_result_no_activity() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        let r = runner.build_result(0);
        assert_eq!(r.ticks, 0);
        assert_eq!(r.errors, 0);
        assert_eq!(r.duration_ms, 0);
        assert!(r.is_success());
    }

    #[test]
    fn runner_build_result_with_activity() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        for _ in 0..100 {
            runner.record_tick();
        }
        runner.record_error();
        runner.record_error();
        let r = runner.build_result(5000);
        assert_eq!(r.ticks, 100);
        assert_eq!(r.errors, 2);
        assert_eq!(r.duration_ms, 5000);
        assert!(!r.is_success());
    }

    #[test]
    fn runner_build_result_only_ticks() {
        let runner = HeadlessRunner::new(HeadlessConfig::default());
        for _ in 0..50 {
            runner.record_tick();
        }
        let r = runner.build_result(200);
        assert_eq!(r.ticks, 50);
        assert_eq!(r.errors, 0);
        assert!(r.is_success());
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessRunner — format_result
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn runner_format_result_text() {
        let runner = HeadlessRunner::new(HeadlessConfig {
            output_format: OutputFormat::Text,
            ..Default::default()
        });
        let r = HeadlessResult::success(10, 40);
        let out = runner.format_result(&r);
        assert!(out.contains("10 ticks"));
    }

    #[test]
    fn runner_format_result_json() {
        let runner = HeadlessRunner::new(HeadlessConfig {
            output_format: OutputFormat::Json,
            ..Default::default()
        });
        let r = HeadlessResult::success(10, 40);
        let out = runner.format_result(&r);
        assert!(out.starts_with('{'));
        assert!(out.contains("\"ticks\":10"));
    }

    #[test]
    fn runner_format_result_csv() {
        let runner = HeadlessRunner::new(HeadlessConfig {
            output_format: OutputFormat::Csv,
            ..Default::default()
        });
        let r = HeadlessResult::success(10, 40);
        let out = runner.format_result(&r);
        assert_eq!(out, "10,0,40,0");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessRunner — thread safety (atomics)
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn runner_shared_across_threads() {
        let runner = Arc::new(HeadlessRunner::new(HeadlessConfig::default()));
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
    fn runner_concurrent_start_stop() {
        let runner = Arc::new(HeadlessRunner::new(HeadlessConfig::default()));
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let r = Arc::clone(&runner);
                std::thread::spawn(move || {
                    if i % 2 == 0 {
                        r.start();
                    } else {
                        r.stop();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        // Final state is deterministic based on last operation
        // but the key thing is no panic occurred
    }

    #[test]
    fn runner_concurrent_ticks_and_errors() {
        let runner = Arc::new(HeadlessRunner::new(HeadlessConfig::default()));
        let tick_handles: Vec<_> = (0..4)
            .map(|_| {
                let r = Arc::clone(&runner);
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        r.record_tick();
                    }
                })
            })
            .collect();
        let error_handles: Vec<_> = (0..2)
            .map(|_| {
                let r = Arc::clone(&runner);
                std::thread::spawn(move || {
                    for _ in 0..10 {
                        r.record_error();
                    }
                })
            })
            .collect();
        for h in tick_handles {
            h.join().unwrap();
        }
        for h in error_handles {
            h.join().unwrap();
        }
        assert_eq!(runner.tick_count(), 200);
        assert_eq!(runner.error_count(), 20);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // HeadlessRunner — config accessor
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn runner_config_accessor() {
        let config = HeadlessConfig {
            enabled: true,
            tick_rate_hz: 120.0,
            ..Default::default()
        };
        let runner = HeadlessRunner::new(config);
        assert!(runner.config().enabled);
        assert_eq!(runner.config().tick_rate_hz, 120.0);
    }
}
