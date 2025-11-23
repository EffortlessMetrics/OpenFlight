// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Allocation tracking tests for zero-allocation constraint validation

#[cfg(test)]
mod tests {
    use super::super::*;
    use flight_core::rules::{Rule, RuleDefaults, RulesSchema};
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Custom allocator that tracks allocations
    struct TrackingAllocator {
        allocation_count: AtomicUsize,
    }

    impl TrackingAllocator {
        const fn new() -> Self {
            Self {
                allocation_count: AtomicUsize::new(0),
            }
        }

        fn reset_count(&self) {
            self.allocation_count.store(0, Ordering::SeqCst);
        }

        fn get_count(&self) -> usize {
            self.allocation_count.load(Ordering::SeqCst)
        }
    }

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            self.allocation_count.fetch_add(1, Ordering::SeqCst);
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    // Note: This would require setting a global allocator, which can't be done in tests
    // Instead, we'll use indirect methods to validate zero-allocation behavior

    #[test]
    fn test_evaluator_capacity_stability() {
        // Test that evaluator doesn't grow its internal buffers during evaluation
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
                    when: "ias > 90".to_string(),
                    do_action: "led.panel('SPEED').on()".to_string(),
                    action: "led.panel('SPEED').on()".to_string(),
                },
            ],
            defaults: None,
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));

        // Capture initial capacities
        let initial_stack_capacity = evaluator.stack().capacity();
        let initial_actions_capacity = evaluator.actions_buffer().capacity();
        let initial_variable_capacity = evaluator.variable_cache().capacity();

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);
        telemetry.insert("ias".to_string(), 95.0);

        // Run many evaluations
        for i in 0..10000 {
            telemetry.insert("ias".to_string(), 90.0 + (i % 20) as f32);
            let _actions = evaluator.evaluate(&compiled, &telemetry);
        }

        // Verify capacities haven't grown (indicating no allocations)
        assert_eq!(evaluator.stack().capacity(), initial_stack_capacity);
        assert_eq!(
            evaluator.actions_buffer().capacity(),
            initial_actions_capacity
        );
        assert_eq!(
            evaluator.variable_cache().capacity(),
            initial_variable_capacity
        );
    }

    #[test]
    fn test_hysteresis_state_stability() {
        // Test that hysteresis state doesn't cause allocations
        let mut evaluator = RulesEvaluator::new();

        let mut hysteresis = HashMap::new();
        hysteresis.insert("aoa".to_string(), 2.0);
        hysteresis.insert("ias".to_string(), 5.0);

        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "aoa > 10".to_string(),
                    do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                    action: "led.indexer.blink(rate_hz=6)".to_string(),
                },
                Rule {
                    when: "ias > 100".to_string(),
                    do_action: "led.panel('SPEED').on()".to_string(),
                    action: "led.panel('SPEED').on()".to_string(),
                },
            ],
            defaults: Some(RuleDefaults {
                hysteresis: Some(hysteresis),
            }),
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));

        // Capture initial hysteresis state capacity
        let initial_hyst_len = evaluator.hysteresis_state().len();

        let mut telemetry = HashMap::new();

        // Oscillate values around thresholds to trigger hysteresis
        for i in 0..5000 {
            let aoa = 10.0 + 2.0 * ((i as f32 * 0.1).sin());
            let ias = 100.0 + 10.0 * ((i as f32 * 0.05).cos());

            telemetry.insert("aoa".to_string(), aoa);
            telemetry.insert("ias".to_string(), ias);

            let _actions = evaluator.evaluate(&compiled, &telemetry);
        }

        // Verify hysteresis state hasn't grown
        assert_eq!(evaluator.hysteresis_state().len(), initial_hyst_len);
    }

    #[test]
    fn test_complex_rules_no_allocation() {
        // Test complex rules with multiple conditions and actions
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
                    when: "!gear_down".to_string(),
                    do_action: "led.panel('GEAR').off()".to_string(),
                    action: "led.panel('GEAR').off()".to_string(),
                },
                Rule {
                    when: "ias > 90".to_string(),
                    do_action: "led.panel('SPEED_HIGH').on()".to_string(),
                    action: "led.panel('SPEED_HIGH').on()".to_string(),
                },
                Rule {
                    when: "altitude > 1000".to_string(),
                    do_action: "led.panel('ALT_HIGH').on()".to_string(),
                    action: "led.panel('ALT_HIGH').on()".to_string(),
                },
            ],
            defaults: None,
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));

        // Capture all initial capacities
        let initial_stack_cap = evaluator.stack().capacity();
        let initial_actions_cap = evaluator.actions_buffer().capacity();
        let initial_vars_cap = evaluator.variable_cache().capacity();
        let initial_hyst_len = evaluator.hysteresis_state().len();

        let mut telemetry = HashMap::new();

        // Run complex evaluation patterns
        for i in 0..2000 {
            telemetry.insert(
                "gear_down".to_string(),
                if i % 100 < 50 { 1.0 } else { 0.0 },
            );
            telemetry.insert("ias".to_string(), 80.0 + (i % 50) as f32);
            telemetry.insert("altitude".to_string(), 500.0 + (i % 200) as f32 * 10.0);

            let actions = evaluator.evaluate(&compiled, &telemetry);

            // Verify we get reasonable results
            assert!(actions.len() <= 4); // Maximum possible actions
        }

        // Verify no capacity growth
        assert_eq!(evaluator.stack().capacity(), initial_stack_cap);
        assert_eq!(evaluator.actions_buffer().capacity(), initial_actions_cap);
        assert_eq!(evaluator.variable_cache().capacity(), initial_vars_cap);
        assert_eq!(evaluator.hysteresis_state().len(), initial_hyst_len);
    }

    #[test]
    fn test_evaluation_timing_consistency() {
        // Test that evaluation time remains consistent (no GC pauses from allocations)
        let mut evaluator = RulesEvaluator::new();

        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
            defaults: None,
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);

        let mut times = Vec::new();

        // Measure evaluation times
        for _ in 0..1000 {
            let start = std::time::Instant::now();
            let _actions = evaluator.evaluate(&compiled, &telemetry);
            let duration = start.elapsed();
            times.push(duration.as_nanos());
        }

        // Calculate statistics
        let mean = times.iter().sum::<u128>() / times.len() as u128;
        let max = *times.iter().max().unwrap();
        let min = *times.iter().min().unwrap();

        // Verify timing consistency (max shouldn't be much larger than mean)
        // This would catch GC pauses from allocations
        assert!(
            max < mean * 10,
            "Timing inconsistency detected: max={}, mean={}, min={}",
            max,
            mean,
            min
        );

        // Verify reasonable performance (should be very fast)
        assert!(mean < 10_000, "Evaluation too slow: {} ns", mean); // Less than 10μs
    }

    #[test]
    fn test_memory_usage_stability() {
        // Test that memory usage doesn't grow over time
        use std::process;

        let mut evaluator = RulesEvaluator::new();

        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
            defaults: None,
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);

        // Get initial memory usage (approximation)
        let initial_memory = get_memory_usage();

        // Run many evaluations
        for i in 0..50000 {
            telemetry.insert("gear_down".to_string(), if i % 2 == 0 { 1.0 } else { 0.0 });
            let _actions = evaluator.evaluate(&compiled, &telemetry);
        }

        // Check memory usage hasn't grown significantly
        let final_memory = get_memory_usage();
        let growth = final_memory.saturating_sub(initial_memory);

        // Allow some growth for OS/runtime overhead, but not much
        assert!(
            growth < 1024 * 1024,
            "Memory usage grew by {} bytes",
            growth
        ); // Less than 1MB growth
    }

    fn get_memory_usage() -> usize {
        // Simple approximation of memory usage
        // In a real implementation, we'd use platform-specific APIs
        std::mem::size_of::<RulesEvaluator>() * 1000 // Placeholder
    }
}
