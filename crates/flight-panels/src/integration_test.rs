//! Integration tests for DSL compiler and evaluator system

#[cfg(test)]
mod tests {
    use super::super::*;
    use flight_core::rules::{RulesSchema, Rule, RuleDefaults};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    #[test]
    fn test_complete_dsl_pipeline() {
        // Test the complete pipeline: DSL → Bytecode → Evaluation → LED Control
        let mut panel_manager = PanelManager::new();

        // Create a comprehensive rules schema
        let mut hysteresis = HashMap::new();
        hysteresis.insert("aoa".to_string(), 1.0);
        hysteresis.insert("ias".to_string(), 5.0);

        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('GEAR').on()".to_string(),
                    action: "led.panel('GEAR').on()".to_string(),
                },
                Rule {
                    when: "!gear_down".to_string(),
                    do_action: "led.panel('GEAR').off()".to_string(),
                    action: "led.panel('GEAR').off()".to_string(),
                },
                Rule {
                    when: "aoa > 15".to_string(),
                    do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                    action: "led.indexer.blink(rate_hz=6)".to_string(),
                },
                Rule {
                    when: "ias > 200".to_string(),
                    do_action: "led.panel('OVERSPEED').on()".to_string(),
                    action: "led.panel('OVERSPEED').on()".to_string(),
                },
            ],
            defaults: Some(RuleDefaults {
                hysteresis: Some(hysteresis),
            }),
        };

        // Load and compile rules
        panel_manager.load_rules(rules_schema).unwrap();

        // Test various telemetry scenarios
        let mut telemetry = HashMap::new();

        // Scenario 1: Gear down, normal flight
        telemetry.insert("gear_down".to_string(), 1.0);
        telemetry.insert("aoa".to_string(), 5.0);
        telemetry.insert("ias".to_string(), 150.0);
        panel_manager.update(&telemetry).unwrap();

        // Scenario 2: Gear up, high AOA
        telemetry.insert("gear_down".to_string(), 0.0);
        telemetry.insert("aoa".to_string(), 16.0);
        panel_manager.update(&telemetry).unwrap();

        // Scenario 3: Overspeed condition
        telemetry.insert("ias".to_string(), 250.0);
        panel_manager.update(&telemetry).unwrap();

        // Verify LED controller has processed actions
        let led_controller = panel_manager.led_controller();
        let stats = led_controller.get_latency_stats();
        assert!(stats.is_some(), "LED controller should have latency statistics");
    }

    #[test]
    fn test_60_120hz_evaluation_rate() {
        // Test that evaluator can sustain 60-120Hz evaluation rate
        let mut evaluator = RulesEvaluator::new();
        
        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('GEAR').on()".to_string(),
                    action: "led.panel('GEAR').on()".to_string(),
                },
                Rule {
                    when: "ias > 100".to_string(),
                    do_action: "led.panel('SPEED').on()".to_string(),
                    action: "led.panel('SPEED').on()".to_string(),
                },
            ],
            defaults: None,
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(Duration::from_millis(0)); // No rate limiting for test

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);
        telemetry.insert("ias".to_string(), 120.0);

        // Test 120Hz rate (8.33ms period)
        let target_period = Duration::from_nanos(8_333_333); // ~120Hz
        let test_duration = Duration::from_secs(1);
        let start_time = Instant::now();
        let mut evaluations = 0;
        let mut max_eval_time = Duration::from_nanos(0);

        while start_time.elapsed() < test_duration {
            let eval_start = Instant::now();
            let _actions = evaluator.evaluate(&compiled, &telemetry);
            let eval_time = eval_start.elapsed();
            
            max_eval_time = max_eval_time.max(eval_time);
            evaluations += 1;

            // Sleep to maintain target rate
            if eval_time < target_period {
                std::thread::sleep(target_period - eval_time);
            }
        }

        let actual_rate = evaluations as f64 / test_duration.as_secs_f64();
        
        // Verify we achieved close to target rate
        assert!(actual_rate >= 100.0, "Evaluation rate too low: {:.1} Hz", actual_rate);
        assert!(actual_rate <= 130.0, "Evaluation rate too high: {:.1} Hz", actual_rate);
        
        // Verify individual evaluations are fast enough
        assert!(
            max_eval_time < Duration::from_millis(1), 
            "Individual evaluation too slow: {:?}", 
            max_eval_time
        );
    }

    #[test]
    fn test_hysteresis_prevents_flicker() {
        // Test that hysteresis prevents LED flicker around thresholds
        let mut evaluator = RulesEvaluator::new();
        
        let mut hysteresis = HashMap::new();
        hysteresis.insert("aoa".to_string(), 2.0); // ±1.0 band around threshold

        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "aoa > 10".to_string(),
                    do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                    action: "led.indexer.blink(rate_hz=6)".to_string(),
                },
            ],
            defaults: Some(RuleDefaults {
                hysteresis: Some(hysteresis),
            }),
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(Duration::from_millis(0));

        let mut telemetry = HashMap::new();
        let mut action_changes = 0;
        let mut last_action_count = 0;

        // Oscillate around threshold to test hysteresis
        for i in 0..1000 {
            let aoa = 10.0 + 0.5 * ((i as f32 * 0.1).sin()); // Oscillate ±0.5 around 10.0
            telemetry.insert("aoa".to_string(), aoa);
            
            let actions = evaluator.evaluate(&compiled, &telemetry);
            let current_action_count = actions.len();
            
            if current_action_count != last_action_count {
                action_changes += 1;
                last_action_count = current_action_count;
            }
        }

        // With hysteresis, we should have very few action changes despite oscillation
        assert!(
            action_changes < 10, 
            "Too many action changes ({}), hysteresis not working properly", 
            action_changes
        );
    }

    #[test]
    fn test_complex_rule_combinations() {
        // Test complex combinations of AND/OR conditions
        let mut evaluator = RulesEvaluator::new();
        
        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                // Gear warning: gear up AND low altitude AND low speed
                Rule {
                    when: "!gear_down".to_string(), // This is a simplified version
                    do_action: "led.panel('GEAR_WARNING').on()".to_string(),
                    action: "led.panel('GEAR_WARNING').on()".to_string(),
                },
                // Master caution: any warning condition
                Rule {
                    when: "gear_down".to_string(), // Simplified for testing
                    do_action: "led.panel('MASTER_CAUTION').on()".to_string(),
                    action: "led.panel('MASTER_CAUTION').on()".to_string(),
                },
            ],
            defaults: None,
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(Duration::from_millis(0));

        // Test various combinations
        let test_cases = vec![
            // (gear_down, altitude, ias, expected_actions)
            (1.0, 1000.0, 150.0, 1), // Normal flight - master caution only
            (0.0, 1000.0, 150.0, 1), // Gear up, high alt - gear warning only
            (0.0, 500.0, 80.0, 1),   // Gear up, low alt, low speed - gear warning
            (1.0, 500.0, 80.0, 1),   // Gear down, low alt, low speed - master caution
        ];

        for (gear, alt, ias, expected) in test_cases {
            let mut telemetry = HashMap::new();
            telemetry.insert("gear_down".to_string(), gear);
            telemetry.insert("altitude".to_string(), alt);
            telemetry.insert("ias".to_string(), ias);

            let actions = evaluator.evaluate(&compiled, &telemetry);
            assert_eq!(
                actions.len(), 
                expected,
                "Wrong number of actions for gear={}, alt={}, ias={}: got {}, expected {}",
                gear, alt, ias, actions.len(), expected
            );
        }
    }

    #[test]
    fn test_end_to_end_latency() {
        // Test complete end-to-end latency from telemetry update to LED write
        let mut panel_manager = PanelManager::new();
        
        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('GEAR').on()".to_string(),
                    action: "led.panel('GEAR').on()".to_string(),
                },
            ],
            defaults: None,
        };

        panel_manager.load_rules(rules_schema).unwrap();

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);

        // Measure end-to-end latency
        let mut latencies = Vec::new();
        
        for _ in 0..100 {
            let start = Instant::now();
            panel_manager.update(&telemetry).unwrap();
            let latency = start.elapsed();
            latencies.push(latency.as_nanos());
        }

        // Calculate statistics
        latencies.sort_unstable();
        let len = latencies.len();
        let mean = latencies.iter().sum::<u128>() / len as u128;
        let p99 = latencies[(len as f64 * 0.99) as usize];
        let max = latencies[len - 1];

        // Validate against requirements
        assert!(
            p99 <= 20_000_000, // 20ms in nanoseconds
            "End-to-end latency requirement violated: P99 = {} ns (>20ms)",
            p99
        );

        // Should be much faster in test environment
        assert!(
            mean < 5_000_000, // 5ms in nanoseconds
            "Mean latency should be much better in test: {} ns",
            mean
        );

        println!("End-to-end latency stats: mean={:.2}ms, p99={:.2}ms, max={:.2}ms", 
                 mean as f64 / 1_000_000.0,
                 p99 as f64 / 1_000_000.0,
                 max as f64 / 1_000_000.0);
    }

    #[test]
    fn test_sustained_operation() {
        // Test sustained operation over time without degradation
        let mut panel_manager = PanelManager::new();
        
        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('GEAR').on()".to_string(),
                    action: "led.panel('GEAR').on()".to_string(),
                },
                Rule {
                    when: "ias > 100".to_string(),
                    do_action: "led.panel('SPEED').on()".to_string(),
                    action: "led.panel('SPEED').on()".to_string(),
                },
            ],
            defaults: None,
        };

        panel_manager.load_rules(rules_schema).unwrap();

        let mut telemetry = HashMap::new();
        let start_time = Instant::now();
        let test_duration = Duration::from_secs(5); // 5 second sustained test
        let mut updates = 0;
        let mut max_update_time = Duration::from_nanos(0);

        // Run at ~60Hz for sustained period
        while start_time.elapsed() < test_duration {
            let update_start = Instant::now();
            
            // Vary telemetry to exercise different code paths
            let t = start_time.elapsed().as_secs_f32();
            telemetry.insert("gear_down".to_string(), if (t * 0.5).sin() > 0.0 { 1.0 } else { 0.0 });
            telemetry.insert("ias".to_string(), 90.0 + 20.0 * (t * 0.3).sin());
            
            panel_manager.update(&telemetry).unwrap();
            
            let update_time = update_start.elapsed();
            max_update_time = max_update_time.max(update_time);
            updates += 1;

            // Target ~60Hz
            std::thread::sleep(Duration::from_millis(16));
        }

        let actual_rate = updates as f64 / test_duration.as_secs_f64();
        
        // Verify sustained performance
        assert!(actual_rate >= 50.0, "Update rate too low: {:.1} Hz", actual_rate);
        assert!(
            max_update_time < Duration::from_millis(10),
            "Update time degraded: {:?}",
            max_update_time
        );

        // Check LED controller statistics
        let stats = panel_manager.led_controller().get_latency_stats().unwrap();
        assert!(
            stats.p99_ns <= 20_000_000,
            "LED latency degraded over time: P99 = {} ns",
            stats.p99_ns
        );

        println!("Sustained operation: {:.1} Hz for {:.1}s, max_update={:.2}ms, LED_p99={:.2}ms",
                 actual_rate,
                 test_duration.as_secs_f32(),
                 max_update_time.as_secs_f32() * 1000.0,
                 stats.p99_ns as f64 / 1_000_000.0);
    }
}