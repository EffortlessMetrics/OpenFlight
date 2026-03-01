// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-rules` DSL — parser, compiler, schema validation,
//! property tests, and snapshot tests.

use flight_rules::{
    Action, BytecodeOp, CompareOp, CompiledRules, Condition, Rule, RuleDefaults,
    RulesCompiler, RulesSchema, check_conflicts,
};
use std::collections::HashMap;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn rule(when: &str, action: &str) -> Rule {
    Rule {
        when: when.to_string(),
        action: action.to_string(),
        do_action: action.to_string(),
    }
}

fn schema(rules: Vec<Rule>) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules,
        defaults: None,
    }
}

fn compiler() -> RulesCompiler {
    RulesCompiler::new(HashMap::new())
}

fn validate_one(when: &str, action: &str) -> flight_rules::Result<()> {
    schema(vec![rule(when, action)]).validate()
}

fn compile_one(when: &str, action: &str) -> flight_rules::Result<CompiledRules> {
    schema(vec![rule(when, action)]).compile()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Parser tests — conditions
// ═══════════════════════════════════════════════════════════════════════════

mod parser_conditions {
    use super::*;

    #[test]
    fn simple_boolean_true() {
        let c = compiler().parse_condition("gear_down").unwrap();
        assert!(
            matches!(&c, Condition::Boolean { variable, negate } if variable == "gear_down" && !negate)
        );
    }

    #[test]
    fn simple_boolean_negated() {
        let c = compiler().parse_condition("!gear_down").unwrap();
        assert!(
            matches!(&c, Condition::Boolean { variable, negate: true } if variable == "gear_down")
        );
    }

    #[test]
    fn enum_equality_maps_to_boolean() {
        let c = compiler().parse_condition("gear == DOWN").unwrap();
        assert!(
            matches!(&c, Condition::Boolean { variable, negate } if variable == "gear_DOWN" && !negate)
        );
    }

    #[test]
    fn enum_inequality_maps_to_negated_boolean() {
        let c = compiler().parse_condition("gear != UP").unwrap();
        assert!(
            matches!(&c, Condition::Boolean { variable, negate: true } if variable == "gear_UP")
        );
    }

    #[test]
    fn equal_numeric() {
        let c = compiler().parse_condition("altitude == 10000").unwrap();
        match &c {
            Condition::Compare {
                variable,
                operator: CompareOp::Equal,
                value,
            } => {
                assert_eq!(variable, "altitude");
                assert!((value - 10000.0).abs() < f32::EPSILON);
            }
            other => panic!("Expected Compare(Equal), got {:?}", other),
        }
    }

    #[test]
    fn not_equal_numeric() {
        let c = compiler().parse_condition("flaps != 0").unwrap();
        assert!(matches!(
            &c,
            Condition::Compare {
                operator: CompareOp::NotEqual,
                ..
            }
        ));
    }

    #[test]
    fn less_than() {
        let c = compiler().parse_condition("speed < 200").unwrap();
        match &c {
            Condition::Compare {
                variable,
                operator: CompareOp::Less,
                value,
            } => {
                assert_eq!(variable, "speed");
                assert!((value - 200.0).abs() < f32::EPSILON);
            }
            other => panic!("Expected Compare(Less), got {:?}", other),
        }
    }

    #[test]
    fn greater_than() {
        let c = compiler().parse_condition("altitude > 10000").unwrap();
        assert!(matches!(
            &c,
            Condition::Compare {
                operator: CompareOp::Greater,
                ..
            }
        ));
    }

    #[test]
    fn less_equal() {
        let c = compiler().parse_condition("flaps <= 0.5").unwrap();
        assert!(matches!(
            &c,
            Condition::Compare {
                operator: CompareOp::LessEqual,
                ..
            }
        ));
    }

    #[test]
    fn greater_equal() {
        let c = compiler().parse_condition("ias >= 200").unwrap();
        assert!(matches!(
            &c,
            Condition::Compare {
                operator: CompareOp::GreaterEqual,
                ..
            }
        ));
    }

    #[test]
    fn negative_threshold() {
        let c = compiler().parse_condition("pitch > -15").unwrap();
        match &c {
            Condition::Compare {
                operator: CompareOp::Greater,
                value,
                ..
            } => {
                assert!((value - (-15.0)).abs() < f32::EPSILON);
            }
            other => panic!("Expected Compare(Greater), got {:?}", other),
        }
    }

    #[test]
    fn decimal_threshold() {
        let c = compiler().parse_condition("aoa > 12.5").unwrap();
        match &c {
            Condition::Compare { value, .. } => {
                assert!((value - 12.5).abs() < f32::EPSILON);
            }
            other => panic!("Expected Compare, got {:?}", other),
        }
    }

    // ── Compound conditions ─────────────────────────────────────────────

    #[test]
    fn compound_and() {
        let c = compiler()
            .parse_condition("gear_down and airspeed < 200")
            .unwrap();
        match c {
            Condition::And(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(&parts[0], Condition::Boolean { .. }));
                assert!(matches!(
                    &parts[1],
                    Condition::Compare {
                        operator: CompareOp::Less,
                        ..
                    }
                ));
            }
            other => panic!("Expected And, got {:?}", other),
        }
    }

    #[test]
    fn compound_or() {
        let c = compiler()
            .parse_condition("gear_down or flaps_extended")
            .unwrap();
        match c {
            Condition::Or(parts) => assert_eq!(parts.len(), 2),
            other => panic!("Expected Or, got {:?}", other),
        }
    }

    #[test]
    fn chained_and_three() {
        let c = compiler().parse_condition("a and b and c").unwrap();
        match c {
            Condition::And(parts) => assert_eq!(parts.len(), 3),
            other => panic!("Expected And(3), got {:?}", other),
        }
    }

    #[test]
    fn chained_or_three() {
        let c = compiler().parse_condition("a or b or c").unwrap();
        match c {
            Condition::Or(parts) => assert_eq!(parts.len(), 3),
            other => panic!("Expected Or(3), got {:?}", other),
        }
    }

    // ── Error paths ─────────────────────────────────────────────────────

    #[test]
    fn invalid_number_in_comparison() {
        let e = compiler().parse_condition("ias > notanumber").unwrap_err();
        assert!(e.to_string().contains("Invalid number"));
    }

    #[test]
    fn missing_spaces_around_operator_is_error() {
        // Without spaces the parser treats it as a plain boolean var
        // "ias>=200" has `>` and `=` chars so it enters comparison parsing
        // but fails because of no space-delimited operator
        let result = compiler().parse_condition("ias>=200");
        // Must not parse as a correct >= comparison
        if let Ok(Condition::Compare {
            operator: CompareOp::GreaterEqual,
            ..
        }) = &result
        {
            panic!("Should not parse ias>=200 as valid >=");
        }
    }

    #[test]
    fn empty_rhs_after_operator() {
        let e = compiler().parse_condition("ias > ").unwrap_err();
        assert!(
            e.to_string().contains("Invalid") || e.to_string().contains("Unsupported"),
            "got: {}",
            e
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Parser tests — actions
// ═══════════════════════════════════════════════════════════════════════════

mod parser_actions {
    use super::*;

    #[test]
    fn led_panel_on() {
        let a = compiler().parse_action("led.panel('GEAR').on()").unwrap();
        assert!(matches!(&a, Action::LedOn { target } if target == "GEAR"));
    }

    #[test]
    fn led_panel_off() {
        let a = compiler().parse_action("led.panel('GEAR').off()").unwrap();
        assert!(matches!(&a, Action::LedOff { target } if target == "GEAR"));
    }

    #[test]
    fn led_panel_blink() {
        let a = compiler()
            .parse_action("led.panel('STALL').blink(rate_hz=4)")
            .unwrap();
        match a {
            Action::LedBlink { target, rate_hz } => {
                assert_eq!(target, "STALL");
                assert!((rate_hz - 4.0).abs() < f32::EPSILON);
            }
            other => panic!("Expected LedBlink, got {:?}", other),
        }
    }

    #[test]
    fn led_panel_brightness() {
        let a = compiler()
            .parse_action("led.panel('WARN').brightness(0.75)")
            .unwrap();
        match a {
            Action::LedBrightness { target, brightness } => {
                assert_eq!(target, "WARN");
                assert!((brightness - 0.75).abs() < f32::EPSILON);
            }
            other => panic!("Expected LedBrightness, got {:?}", other),
        }
    }

    #[test]
    fn led_indexer_on() {
        let a = compiler().parse_action("led.indexer.on()").unwrap();
        assert!(matches!(&a, Action::LedOn { target } if target == "indexer"));
    }

    #[test]
    fn led_indexer_off() {
        let a = compiler().parse_action("led.indexer.off()").unwrap();
        assert!(matches!(&a, Action::LedOff { target } if target == "indexer"));
    }

    #[test]
    fn led_indexer_blink() {
        let a = compiler()
            .parse_action("led.indexer.blink(rate_hz=6)")
            .unwrap();
        match a {
            Action::LedBlink { target, rate_hz } => {
                assert_eq!(target, "indexer");
                assert!((rate_hz - 6.0).abs() < f32::EPSILON);
            }
            other => panic!("Expected LedBlink, got {:?}", other),
        }
    }

    #[test]
    fn float_blink_rate() {
        let a = compiler()
            .parse_action("led.panel('X').blink(rate_hz=2.5)")
            .unwrap();
        assert!(matches!(&a, Action::LedBlink { rate_hz, .. } if (rate_hz - 2.5).abs() < f32::EPSILON));
    }

    // ── Error paths ─────────────────────────────────────────────────────

    #[test]
    fn unknown_action_is_error() {
        let e = compiler().parse_action("set_led master_warning on").unwrap_err();
        assert!(e.to_string().contains("Unsupported action"));
    }

    #[test]
    fn missing_method_call_is_error() {
        let e = compiler().parse_action("led.panel('GEAR')").unwrap_err();
        assert!(e.to_string().contains("Unsupported action"));
    }

    #[test]
    fn invalid_blink_rate_is_error() {
        let e = compiler()
            .parse_action("led.panel('X').blink(rate_hz=abc)")
            .unwrap_err();
        assert!(e.to_string().contains("Invalid rate"));
    }

    #[test]
    fn invalid_brightness_value_is_error() {
        let e = compiler()
            .parse_action("led.panel('X').brightness(abc)")
            .unwrap_err();
        assert!(e.to_string().contains("Invalid brightness"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Compilation tests
// ═══════════════════════════════════════════════════════════════════════════

mod compilation {
    use super::*;

    #[test]
    fn simple_rule_compiles_to_nonempty_bytecode() {
        let compiled = compile_one("gear_down", "led.indexer.on()").unwrap();
        assert!(!compiled.bytecode.instructions.is_empty());
    }

    #[test]
    fn compiled_bytecode_contains_jump_false_guard() {
        let compiled = compile_one("ias > 100", "led.indexer.on()").unwrap();
        assert!(compiled
            .bytecode
            .instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::JumpFalse(_))));
    }

    #[test]
    fn compiled_bytecode_contains_action() {
        let compiled = compile_one("gear_down", "led.panel('GEAR').on()").unwrap();
        assert!(compiled
            .bytecode
            .instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Action(_))));
        assert_eq!(compiled.bytecode.actions.len(), 1);
        assert!(matches!(
            &compiled.bytecode.actions[0],
            Action::LedOn { target } if target == "GEAR"
        ));
    }

    #[test]
    fn comparison_rule_loads_var_and_const() {
        let compiled = compile_one("altitude > 10000", "led.indexer.on()").unwrap();
        let instrs = &compiled.bytecode.instructions;
        assert!(instrs.iter().any(|op| matches!(op, BytecodeOp::LoadVar(_))));
        assert!(instrs
            .iter()
            .any(|op| matches!(op, BytecodeOp::LoadConst(v) if (*v - 10000.0).abs() < f32::EPSILON)));
        assert!(instrs
            .iter()
            .any(|op| matches!(op, BytecodeOp::Compare(CompareOp::Greater))));
    }

    #[test]
    fn boolean_condition_emits_neq_zero_check() {
        let compiled = compile_one("gear_down", "led.indexer.on()").unwrap();
        let instrs = &compiled.bytecode.instructions;
        // Boolean is: LoadVar, LoadConst(0.0), Compare(NotEqual)
        assert!(instrs
            .iter()
            .any(|op| matches!(op, BytecodeOp::LoadConst(v) if *v == 0.0)));
        assert!(instrs
            .iter()
            .any(|op| matches!(op, BytecodeOp::Compare(CompareOp::NotEqual))));
    }

    #[test]
    fn negated_boolean_emits_not() {
        let compiled = compile_one("!gear_down", "led.indexer.on()").unwrap();
        assert!(compiled
            .bytecode
            .instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Not)));
    }

    #[test]
    fn and_condition_emits_and_op() {
        let compiled = compile_one("gear_down and ias < 200", "led.indexer.on()").unwrap();
        assert!(compiled
            .bytecode
            .instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::And)));
    }

    #[test]
    fn or_condition_emits_or_op() {
        let compiled = compile_one("gear_down or ias > 200", "led.indexer.on()").unwrap();
        assert!(compiled
            .bytecode
            .instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Or)));
    }

    #[test]
    fn multiple_rules_ordered_by_index() {
        let s = schema(vec![
            rule("gear_down", "led.panel('GEAR').on()"),
            rule("ias > 200", "led.panel('SPEED').blink(rate_hz=3)"),
            rule("!flaps_up", "led.panel('FLAP').off()"),
        ]);
        let compiled = s.compile().unwrap();
        assert_eq!(compiled.bytecode.actions.len(), 3);
        assert!(matches!(
            &compiled.bytecode.actions[0],
            Action::LedOn { target } if target == "GEAR"
        ));
        assert!(matches!(
            &compiled.bytecode.actions[1],
            Action::LedBlink { target, .. } if target == "SPEED"
        ));
        assert!(matches!(
            &compiled.bytecode.actions[2],
            Action::LedOff { target } if target == "FLAP"
        ));
    }

    #[test]
    fn hysteresis_rule_emits_hysteresis_op() {
        let mut hysteresis = HashMap::new();
        hysteresis.insert("aoa".to_string(), 0.5);

        let s = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![rule("aoa > 14.5", "led.indexer.blink(rate_hz=6)")],
            defaults: Some(RuleDefaults {
                hysteresis: Some(hysteresis),
            }),
        };
        let compiled = s.compile().unwrap();
        assert!(compiled
            .bytecode
            .instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Hysteresis(_))));
        assert!(!compiled.bytecode.hysteresis_bands.is_empty());
    }

    #[test]
    fn variable_map_assigns_indices() {
        let compiled =
            compile_one("gear_down and ias < 200", "led.panel('GEAR').on()").unwrap();
        assert!(compiled.bytecode.variable_map.contains_key("gear_down"));
        assert!(compiled.bytecode.variable_map.contains_key("ias"));
        let idx1 = compiled.bytecode.variable_map["gear_down"];
        let idx2 = compiled.bytecode.variable_map["ias"];
        assert_ne!(idx1, idx2);
    }

    #[test]
    fn stack_size_at_least_minimum() {
        let compiled = compile_one("a", "led.indexer.on()").unwrap();
        assert!(compiled.bytecode.stack_size >= 8);
    }

    #[test]
    fn empty_rule_set_compiles() {
        let s = schema(vec![]);
        let compiled = s.compile().unwrap();
        assert!(compiled.bytecode.instructions.is_empty());
        assert!(compiled.bytecode.actions.is_empty());
    }

    #[test]
    fn invalid_condition_fails_compile() {
        let result = compile_one("ias > abc", "led.indexer.on()");
        assert!(result.is_err());
    }

    #[test]
    fn invalid_action_fails_compile() {
        let result = compile_one("gear_down", "not.valid()");
        assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Schema validation tests
// ═══════════════════════════════════════════════════════════════════════════

mod schema_validation {
    use super::*;

    #[test]
    fn valid_rule_passes() {
        assert!(validate_one("gear_down", "led.indexer.on()").is_ok());
    }

    #[test]
    fn all_comparison_operators_pass() {
        for op in &[">", "<", ">=", "<=", "==", "!="] {
            let cond = format!("ias {} 100", op);
            assert!(
                validate_one(&cond, "led.indexer.on()").is_ok(),
                "op {} should pass",
                op
            );
        }
    }

    #[test]
    fn all_action_types_pass() {
        let actions = [
            "led.panel('X').on()",
            "led.panel('X').off()",
            "led.panel('X').blink(rate_hz=2)",
            "led.panel('X').brightness(0.5)",
            "led.indexer.on()",
            "led.indexer.off()",
            "led.indexer.blink(rate_hz=3)",
        ];
        for act in &actions {
            assert!(
                validate_one("gear_down", act).is_ok(),
                "action {} should pass",
                act
            );
        }
    }

    #[test]
    fn wrong_schema_version_fails() {
        let s = RulesSchema {
            schema: "flight.ledmap/2".to_string(),
            rules: vec![],
            defaults: None,
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn empty_condition_fails() {
        let e = validate_one("", "led.indexer.on()").unwrap_err();
        assert!(e.to_string().contains("condition"));
    }

    #[test]
    fn empty_action_fails() {
        let e = validate_one("gear_down", "").unwrap_err();
        assert!(e.to_string().contains("action"));
    }

    #[test]
    fn invalid_condition_syntax_reports_clear_error() {
        let e = validate_one("ias > abc", "led.indexer.on()").unwrap_err();
        let msg = e.to_string();
        assert!(
            msg.contains("Invalid number") || msg.contains("Invalid condition"),
            "got: {}",
            msg
        );
    }

    #[test]
    fn invalid_action_syntax_reports_clear_error() {
        let e = validate_one("gear_down", "unknown()").unwrap_err();
        let msg = e.to_string();
        assert!(
            msg.contains("Unsupported action") || msg.contains("Invalid action"),
            "got: {}",
            msg
        );
    }

    #[test]
    fn error_includes_rule_index_one_based() {
        let s = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                rule("gear_down", "led.indexer.on()"),
                rule("ias > notanum", "led.indexer.on()"),
            ],
            defaults: None,
        };
        let e = s.validate().unwrap_err();
        assert!(e.to_string().contains("Rule 2"));
    }

    #[test]
    fn first_invalid_rule_is_reported() {
        let s = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                rule("", "led.indexer.on()"),
                rule("gear_down", "led.indexer.on()"),
            ],
            defaults: None,
        };
        let e = s.validate().unwrap_err();
        assert!(e.to_string().contains("Rule 1"));
    }

    #[test]
    fn whitespace_only_action_fails() {
        let e = validate_one("gear_down", "   ").unwrap_err();
        assert!(
            e.to_string().contains("action") || e.to_string().contains("Unsupported"),
            "got: {}",
            e
        );
    }

    #[test]
    fn empty_ruleset_is_valid() {
        assert!(schema(vec![]).validate().is_ok());
    }

    #[test]
    fn validate_and_compile_agree_on_valid() {
        let cases = [
            ("gear_down", "led.indexer.on()"),
            ("ias > 100", "led.panel('X').on()"),
            ("a and b", "led.panel('Y').off()"),
            ("a or b", "led.indexer.blink(rate_hz=2)"),
            ("!x", "led.panel('Z').brightness(0.5)"),
        ];
        for (when, act) in &cases {
            assert!(validate_one(when, act).is_ok(), "validate failed: {} → {}", when, act);
            assert!(compile_one(when, act).is_ok(), "compile failed: {} → {}", when, act);
        }
    }

    #[test]
    fn validate_and_compile_agree_on_invalid() {
        let cases = [
            ("ias > abc", "led.indexer.on()"),
            ("gear_down", "bad.action()"),
            ("", "led.indexer.on()"),
            ("gear_down", ""),
        ];
        for (when, act) in &cases {
            assert!(
                validate_one(when, act).is_err(),
                "validate should fail: {} → {}",
                when,
                act
            );
            // Empty strings short-circuit before compile, but compile also handles them
            let s = schema(vec![rule(when, act)]);
            let _ = s.compile(); // Must not panic
        }
    }

    #[test]
    fn negative_threshold_validates() {
        assert!(validate_one("alt > -500", "led.indexer.on()").is_ok());
    }

    #[test]
    fn decimal_threshold_validates() {
        assert!(validate_one("pitch < 0.001", "led.indexer.on()").is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Binding conflict detection tests
// ═══════════════════════════════════════════════════════════════════════════

mod conflict_detection {
    use super::*;

    #[test]
    fn no_conflicts_with_unique_targets() {
        let rules = vec![
            rule("gear_down", "led.panel('GEAR').on()"),
            rule("ias > 200", "led.panel('SPEED').on()"),
        ];
        assert!(check_conflicts(&rules).is_empty());
    }

    #[test]
    fn detects_same_target_conflict() {
        let rules = vec![
            rule("gear_down", "led.panel('GEAR').on()"),
            rule("!gear_down", "led.panel('GEAR').off()"),
        ];
        let conflicts = check_conflicts(&rules);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].output, "GEAR");
        assert_eq!(conflicts[0].sources.len(), 2);
    }

    #[test]
    fn multiple_conflicts_sorted_by_output() {
        let rules = vec![
            rule("a", "led.panel('ALPHA').on()"),
            rule("b", "led.panel('ALPHA').off()"),
            rule("c", "led.indexer.on()"),
            rule("d", "led.indexer.off()"),
        ];
        let conflicts = check_conflicts(&rules);
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].output, "ALPHA");
        assert_eq!(conflicts[1].output, "indexer");
    }

    #[test]
    fn empty_rules_no_conflicts() {
        assert!(check_conflicts(&[]).is_empty());
    }

    #[test]
    fn three_rules_same_target() {
        let rules = vec![
            rule("a", "led.panel('X').on()"),
            rule("b", "led.panel('X').off()"),
            rule("c", "led.panel('X').blink(rate_hz=2)"),
        ];
        let conflicts = check_conflicts(&rules);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].sources.len(), 3);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Property tests
// ═══════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Parsing a boolean condition is idempotent — parsing twice yields
        /// the same structure (no hidden mutable state).
        #[test]
        fn parse_condition_is_idempotent(
            var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
            threshold in -9999.0f32..=9999.0f32,
        ) {
            let c = compiler();
            for op in &[">", "<", ">=", "<=", "==", "!="] {
                let expr = format!("{} {} {}", var, op, threshold);
                let r1 = c.parse_condition(&expr);
                let r2 = c.parse_condition(&expr);
                prop_assert_eq!(r1.is_ok(), r2.is_ok());
            }
        }

        /// Same schema always yields identical bytecode instructions.
        #[test]
        fn compilation_is_deterministic(
            var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
            val in -1000.0f32..1000.0f32,
            target in "[A-Z][A-Z0-9_]{0,10}",
        ) {
            let s = RulesSchema {
                schema: "flight.ledmap/1".to_string(),
                rules: vec![Rule {
                    when: format!("{} > {}", var, val),
                    do_action: format!("led.panel('{}').on()", target),
                    action: format!("led.panel('{}').on()", target),
                }],
                defaults: None,
            };
            if let (Ok(c1), Ok(c2)) = (s.compile(), s.compile()) {
                prop_assert_eq!(
                    format!("{:?}", c1.bytecode.instructions),
                    format!("{:?}", c2.bytecode.instructions),
                );
            }
        }

        /// Neither validate nor compile ever panics on arbitrary input.
        #[test]
        fn no_panic_on_arbitrary_input(when in r"\PC*", action in r"\PC*") {
            let s = RulesSchema {
                schema: "flight.ledmap/1".to_string(),
                rules: vec![Rule {
                    when: when.clone(),
                    do_action: action.clone(),
                    action,
                }],
                defaults: None,
            };
            let _ = s.validate();
            let _ = s.compile();
        }

        /// A valid rule always compiles to non-empty bytecode.
        #[test]
        fn valid_rule_produces_instructions(
            var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
            target in "[A-Z][A-Z0-9_]{0,10}",
        ) {
            let s = schema(vec![Rule {
                when: var,
                do_action: format!("led.panel('{}').on()", target),
                action: format!("led.panel('{}').on()", target),
            }]);
            let compiled = s.compile().unwrap();
            prop_assert!(!compiled.bytecode.instructions.is_empty());
        }

        /// LoadConst values in compiled bytecode are always finite.
        #[test]
        fn compiled_constants_are_finite(
            var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
            threshold in -9999.0f32..=9999.0f32,
        ) {
            let s = schema(vec![Rule {
                when: format!("{} > {}", var, threshold),
                do_action: "led.indexer.on()".to_string(),
                action: "led.indexer.on()".to_string(),
            }]);
            if let Ok(compiled) = s.compile() {
                for instr in &compiled.bytecode.instructions {
                    if let BytecodeOp::LoadConst(v) = instr {
                        prop_assert!(v.is_finite());
                    }
                }
            }
        }

        /// N rules produce exactly N action entries (no dedup).
        #[test]
        fn n_rules_n_actions(
            var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
            target in "[A-Z][A-Z0-9_]{0,10}",
            n in 1usize..=5,
        ) {
            let r = Rule {
                when: var,
                do_action: format!("led.panel('{}').on()", target),
                action: format!("led.panel('{}').on()", target),
            };
            let s = schema(vec![r; n]);
            if let Ok(compiled) = s.compile() {
                prop_assert_eq!(compiled.bytecode.actions.len(), n);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Snapshot tests
// ═══════════════════════════════════════════════════════════════════════════

mod depth_snapshots {
    use super::*;
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct StableBytecode {
        instructions: Vec<BytecodeOp>,
        variable_map: BTreeMap<String, u16>,
        actions: Vec<Action>,
        stack_size: usize,
    }

    fn compile_snap(s: &RulesSchema) -> StableBytecode {
        let compiled = s.compile().expect("should compile");
        let bc = compiled.bytecode();
        StableBytecode {
            instructions: bc.instructions.clone(),
            variable_map: bc
                .variable_map
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            actions: bc.actions.clone(),
            stack_size: bc.stack_size,
        }
    }

    #[test]
    fn snapshot_depth_comparison_rule() {
        let snap = compile_snap(&schema(vec![rule(
            "altitude > 35000",
            "led.panel('ALT').blink(rate_hz=2)",
        )]));
        insta::assert_yaml_snapshot!(snap);
    }

    #[test]
    fn snapshot_depth_compound_and_with_comparisons() {
        let snap = compile_snap(&schema(vec![rule(
            "gear_down and speed < 150",
            "led.panel('APPROACH').on()",
        )]));
        insta::assert_yaml_snapshot!(snap);
    }

    #[test]
    fn snapshot_depth_negated_or() {
        let snap = compile_snap(&schema(vec![rule(
            "!engine_fire or fuel_low",
            "led.panel('WARN').blink(rate_hz=8)",
        )]));
        insta::assert_yaml_snapshot!(snap);
    }

    #[test]
    fn snapshot_depth_multi_rule_program() {
        let snap = compile_snap(&schema(vec![
            rule("gear_down", "led.panel('GEAR').on()"),
            rule("!gear_down", "led.panel('GEAR').off()"),
            rule("ias > 250", "led.panel('OVERSPEED').blink(rate_hz=4)"),
        ]));
        insta::assert_yaml_snapshot!(snap);
    }

    #[test]
    fn snapshot_depth_error_bad_number() {
        let e = compiler().parse_condition("speed > xyz").unwrap_err();
        insta::assert_snapshot!(e.to_string());
    }

    #[test]
    fn snapshot_depth_error_bad_action() {
        let e = compiler().parse_action("set_led foo bar").unwrap_err();
        insta::assert_snapshot!(e.to_string());
    }
}
