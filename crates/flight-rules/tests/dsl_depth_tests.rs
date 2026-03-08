// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the rules DSL parser/compiler pipeline.
//!
//! Organised into six areas:
//!   1. Condition parsing (8 tests)
//!   2. Action parsing (6 tests)
//!   3. Compilation (6 tests)
//!   4. Evaluation (5 tests)
//!   5. Schema validation (5 tests)
//!   6. Property tests (5 tests)

use flight_rules::{
    check_conflicts, Action, BytecodeOp, Rule, RuleDefaults, RulesSchema,
};
use std::collections::HashMap;

// ── Helpers ────────────────────────────────────────────────────────────────

fn rule(when: &str, action: &str) -> Rule {
    Rule {
        when: when.to_string(),
        do_action: action.to_string(),
        action: action.to_string(),
    }
}

fn schema(rules: Vec<Rule>) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules,
        defaults: None,
    }
}

fn schema_with_hysteresis(rules: Vec<Rule>, hyst: HashMap<String, f32>) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules,
        defaults: Some(RuleDefaults {
            hysteresis: Some(hyst),
        }),
    }
}

fn compile_ok(s: &RulesSchema) -> flight_rules::CompiledRules {
    s.compile().expect("schema should compile")
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Condition parsing (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn cond_simple_comparison_greater_than() {
    let compiled = compile_ok(&schema(vec![rule("ias > 250", "led.indexer.on()")]));
    let bc = &compiled.bytecode;
    // Must have LoadVar, LoadConst(250.0), Compare(Greater)
    assert!(bc.instructions.iter().any(
        |op| matches!(op, BytecodeOp::LoadConst(v) if (*v - 250.0).abs() < f32::EPSILON)
    ));
    assert!(bc
        .instructions
        .iter()
        .any(|op| matches!(op, BytecodeOp::LoadVar(_))));
}

#[test]
fn cond_and_two_clauses() {
    let compiled = compile_ok(&schema(vec![rule(
        "gear_down and ias < 200",
        "led.indexer.on()",
    )]));
    let bc = &compiled.bytecode;
    // AND condition should emit And bytecode op
    assert!(
        bc.instructions.iter().any(|op| matches!(op, BytecodeOp::And)),
        "AND condition must emit And op: {:?}",
        bc.instructions
    );
}

#[test]
fn cond_or_two_clauses() {
    let compiled = compile_ok(&schema(vec![rule(
        "stall_warn or aoa > 15",
        "led.indexer.on()",
    )]));
    let bc = &compiled.bytecode;
    assert!(
        bc.instructions.iter().any(|op| matches!(op, BytecodeOp::Or)),
        "OR condition must emit Or op: {:?}",
        bc.instructions
    );
}

#[test]
fn cond_nested_and_three_clauses() {
    // "a and b and c" should parse as And([a, b, c])
    let compiled = compile_ok(&schema(vec![rule(
        "gear_down and flaps_up and ias > 100",
        "led.indexer.on()",
    )]));
    let bc = &compiled.bytecode;
    // Three variables should be registered
    assert!(bc.variable_map.len() >= 3, "expected ≥3 variables, got {}", bc.variable_map.len());
    // Should have And ops for chaining
    let and_count = bc
        .instructions
        .iter()
        .filter(|op| matches!(op, BytecodeOp::And))
        .count();
    assert!(and_count >= 1, "chained AND should have ≥1 And op, got {and_count}");
}

#[test]
fn cond_negation_boolean() {
    let compiled = compile_ok(&schema(vec![rule("!autopilot_on", "led.indexer.off()")]));
    let bc = &compiled.bytecode;
    assert!(
        bc.instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Not)),
        "negated boolean must emit Not op: {:?}",
        bc.instructions
    );
}

#[test]
fn cond_string_matching_enum_state() {
    // "gear == DOWN" should map to a boolean variable "gear_DOWN"
    let compiled = compile_ok(&schema(vec![rule(
        "gear == DOWN",
        "led.panel('GEAR').on()",
    )]));
    let bc = &compiled.bytecode;
    assert!(
        bc.variable_map.contains_key("gear_DOWN"),
        "enum comparison should map to composite variable name: {:?}",
        bc.variable_map
    );
}

#[test]
fn cond_numeric_range_all_operators() {
    let operators = [
        ("alt > 1000", "Greater"),
        ("alt < 500", "Less"),
        ("alt >= 1000", "GreaterEqual"),
        ("alt <= 500", "LessEqual"),
        ("alt == 0", "Equal"),
        ("alt != 100", "NotEqual"),
    ];
    for (cond, _label) in &operators {
        let result = schema(vec![rule(cond, "led.indexer.on()")]).compile();
        assert!(result.is_ok(), "operator in {cond:?} should compile");
    }
}

#[test]
fn cond_malformed_condition_rejection() {
    let bad = [
        "ias > notanumber",
        "ias>=200",            // missing spaces
        "> 100",               // missing variable
        "altitude <",          // missing value
        "altitude < < 100",    // double operator
    ];
    for cond in &bad {
        let result = schema(vec![rule(cond, "led.indexer.on()")]).validate();
        assert!(
            result.is_err(),
            "malformed condition {cond:?} should be rejected"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Action parsing (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_led_on() {
    let compiled = compile_ok(&schema(vec![rule(
        "gear_down",
        "led.panel('GEAR').on()",
    )]));
    assert!(
        matches!(&compiled.bytecode.actions[0], Action::LedOn { target } if target == "GEAR")
    );
}

#[test]
fn action_led_off() {
    let compiled = compile_ok(&schema(vec![rule(
        "gear_down",
        "led.panel('GEAR').off()",
    )]));
    assert!(
        matches!(&compiled.bytecode.actions[0], Action::LedOff { target } if target == "GEAR")
    );
}

#[test]
fn action_led_blink_with_rate() {
    let compiled = compile_ok(&schema(vec![rule(
        "stall",
        "led.panel('STALL').blink(rate_hz=4.5)",
    )]));
    match &compiled.bytecode.actions[0] {
        Action::LedBlink { target, rate_hz } => {
            assert_eq!(target, "STALL");
            assert!((*rate_hz - 4.5).abs() < f32::EPSILON);
        }
        other => panic!("expected LedBlink, got {other:?}"),
    }
}

#[test]
fn action_led_brightness() {
    let compiled = compile_ok(&schema(vec![rule(
        "taxi_light",
        "led.panel('TAXI').brightness(0.6)",
    )]));
    match &compiled.bytecode.actions[0] {
        Action::LedBrightness {
            target,
            brightness,
        } => {
            assert_eq!(target, "TAXI");
            assert!((*brightness - 0.6).abs() < 1e-5);
        }
        other => panic!("expected LedBrightness, got {other:?}"),
    }
}

#[test]
fn action_indexer_shorthand() {
    // led.indexer.on/off/blink are shorthand forms with implicit target "indexer"
    let on = compile_ok(&schema(vec![rule("a", "led.indexer.on()")]));
    assert!(matches!(
        &on.bytecode.actions[0],
        Action::LedOn { target } if target == "indexer"
    ));

    let off = compile_ok(&schema(vec![rule("a", "led.indexer.off()")]));
    assert!(matches!(
        &off.bytecode.actions[0],
        Action::LedOff { target } if target == "indexer"
    ));

    let blink = compile_ok(&schema(vec![rule("a", "led.indexer.blink(rate_hz=2)")]));
    assert!(matches!(
        &blink.bytecode.actions[0],
        Action::LedBlink { target, .. } if target == "indexer"
    ));
}

#[test]
fn action_malformed_rejection() {
    let bad = [
        "not.a.real.action()",
        "led.panel('GEAR')",        // missing method call
        "led.panel('GEAR').fly()",   // unknown method
        "led.indexer.dance()",       // unknown method
        "",                          // empty
        "   ",                       // whitespace only
    ];
    for act in &bad {
        let result = schema(vec![rule("gear_down", act)]).validate();
        assert!(
            result.is_err(),
            "malformed action {act:?} should be rejected"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Compilation (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compile_condition_to_bytecode_structure() {
    // A simple comparison should emit: LoadVar, LoadConst, Compare, JumpFalse, Action
    let compiled = compile_ok(&schema(vec![rule("ias > 100", "led.indexer.on()")]));
    let bc = &compiled.bytecode;
    let ops: Vec<&str> = bc
        .instructions
        .iter()
        .map(|op| match op {
            BytecodeOp::LoadVar(_) => "LoadVar",
            BytecodeOp::LoadConst(_) => "LoadConst",
            BytecodeOp::Compare(_) => "Compare",
            BytecodeOp::JumpFalse(_) => "JumpFalse",
            BytecodeOp::Action(_) => "Action",
            BytecodeOp::And => "And",
            BytecodeOp::Or => "Or",
            BytecodeOp::Not => "Not",
            BytecodeOp::Hysteresis(_) => "Hysteresis",
            BytecodeOp::Jump(_) => "Jump",
            BytecodeOp::Nop => "Nop",
        })
        .collect();
    assert!(
        ops.len() >= 5,
        "expected at least 5 bytecode ops for simple comparison, got {}",
        ops.len(),
    );
    assert_eq!(
        &ops[..5],
        &["LoadVar", "LoadConst", "Compare", "JumpFalse", "Action"],
        "simple comparison bytecode structure (prefix)"
    );
    assert!(
        !ops.contains(&"Nop"),
        "did not expect Nop instructions for simple comparison, got ops = {:?}",
        ops,
    );
}

#[test]
fn compile_action_to_bytecode_action_index() {
    let compiled = compile_ok(&schema(vec![
        rule("a", "led.panel('A').on()"),
        rule("b", "led.panel('B').off()"),
    ]));
    let bc = &compiled.bytecode;
    // Two rules → two Action ops referencing indices 0 and 1
    let action_ops: Vec<u16> = bc
        .instructions
        .iter()
        .filter_map(|op| {
            if let BytecodeOp::Action(idx) = op {
                Some(*idx)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(action_ops, vec![0, 1], "action indices should be sequential");
    assert_eq!(bc.actions.len(), 2);
}

#[test]
fn compile_hysteresis_emitted_for_configured_variable() {
    let mut hyst = HashMap::new();
    hyst.insert("aoa".to_string(), 0.5);
    let compiled = compile_ok(&schema_with_hysteresis(
        vec![rule("aoa > 14", "led.indexer.blink(rate_hz=6)")],
        hyst,
    ));
    let bc = &compiled.bytecode;
    assert!(
        bc.instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Hysteresis(_))),
        "configured hysteresis variable should emit Hysteresis op: {:?}",
        bc.instructions
    );
}

#[test]
fn compile_no_hysteresis_when_not_configured() {
    // Without hysteresis defaults, compare should use Compare, not Hysteresis
    let compiled = compile_ok(&schema(vec![rule("aoa > 14", "led.indexer.on()")]));
    let bc = &compiled.bytecode;
    assert!(
        !bc.instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Hysteresis(_))),
        "no Hysteresis op expected without config: {:?}",
        bc.instructions
    );
    assert!(
        bc.instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Compare(_))),
        "Compare op expected: {:?}",
        bc.instructions
    );
}

#[test]
fn compile_nop_not_injected_for_simple_rules() {
    // The compiler should not add unnecessary Nop instructions
    let compiled = compile_ok(&schema(vec![rule("gear_down", "led.indexer.on()")]));
    let bc = &compiled.bytecode;
    assert!(
        !bc.instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Nop)),
        "Nop should not appear in simple rules: {:?}",
        bc.instructions
    );
}

#[test]
fn compile_determinism_complex_multi_rule() {
    let rules = vec![
        rule("gear_down and ias < 200", "led.panel('GEAR').on()"),
        rule("!flaps_up", "led.panel('FLAPS').on()"),
        rule("aoa > 14 or stall_warn", "led.indexer.blink(rate_hz=6)"),
    ];
    let s = schema(rules);
    let c1 = compile_ok(&s);
    let c2 = compile_ok(&s);
    assert_eq!(
        format!("{:?}", c1.bytecode.instructions),
        format!("{:?}", c2.bytecode.instructions),
        "compilation must be deterministic"
    );
    assert_eq!(c1.bytecode.actions.len(), c2.bytecode.actions.len());
    assert_eq!(c1.bytecode.stack_size, c2.bytecode.stack_size);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Evaluation / bytecode structure (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn eval_simple_rule_has_jump_false_guard() {
    // Every compiled rule should have a JumpFalse to skip the action when false
    let compiled = compile_ok(&schema(vec![rule("gear_down", "led.indexer.on()")]));
    let bc = &compiled.bytecode;
    assert!(
        bc.instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::JumpFalse(_))),
        "every rule needs a JumpFalse guard"
    );
}

#[test]
fn eval_complex_condition_chain_produces_correct_op_count() {
    // "a and b and c" → 3 boolean conditions + AND chaining + JumpFalse guards
    let compiled = compile_ok(&schema(vec![rule(
        "a and b and c",
        "led.indexer.on()",
    )]));
    let bc = &compiled.bytecode;
    let load_var_count = bc
        .instructions
        .iter()
        .filter(|op| matches!(op, BytecodeOp::LoadVar(_)))
        .count();
    // Each boolean condition is: LoadVar + LoadConst + Compare(NotEqual)
    // So 3 boolean conditions → 3 LoadVar ops
    assert_eq!(
        load_var_count, 3,
        "3 boolean vars in AND chain → 3 LoadVar ops"
    );
}

#[test]
fn eval_action_index_matches_action_table() {
    // Ensure Action(i) references the correct action in the table
    let compiled = compile_ok(&schema(vec![
        rule("gear_down", "led.panel('GEAR').on()"),
        rule("flaps > 0.5", "led.panel('FLAPS').blink(rate_hz=2)"),
    ]));
    let bc = &compiled.bytecode;
    assert!(matches!(
        &bc.actions[0],
        Action::LedOn { target } if target == "GEAR"
    ));
    assert!(matches!(
        &bc.actions[1],
        Action::LedBlink { target, .. } if target == "FLAPS"
    ));
}

#[test]
fn eval_multi_rule_state_independence() {
    // Each rule's compilation should not affect another's variable mapping
    let compiled = compile_ok(&schema(vec![
        rule("alpha", "led.panel('A').on()"),
        rule("beta", "led.panel('B').on()"),
    ]));
    let bc = &compiled.bytecode;
    // Both variables should be in the map with distinct indices
    let alpha_idx = bc.variable_map.get("alpha");
    let beta_idx = bc.variable_map.get("beta");
    assert!(alpha_idx.is_some(), "alpha should be in variable_map");
    assert!(beta_idx.is_some(), "beta should be in variable_map");
    assert_ne!(alpha_idx, beta_idx, "different variables should have different indices");
}

#[test]
fn eval_shared_variable_reuses_index() {
    // Two rules referencing the same variable should share the index
    let compiled = compile_ok(&schema(vec![
        rule("gear_down", "led.panel('GEAR').on()"),
        rule("gear_down", "led.indexer.on()"),
    ]));
    let bc = &compiled.bytecode;
    // Only one variable registered
    assert_eq!(
        bc.variable_map.len(),
        1,
        "same variable in two rules should be deduplicated: {:?}",
        bc.variable_map
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Schema validation (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn schema_validate_calls_parser_paths() {
    // Validation should exercise the full compile pipeline
    // A rule with valid condition but invalid action should fail validation
    let s = schema(vec![rule("gear_down", "invalid.action()")]);
    let err = s.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Unsupported action") || msg.contains("action"),
        "validation should surface action parse errors: {msg}"
    );
}

#[test]
fn schema_empty_condition_rejected() {
    let s = schema(vec![rule("", "led.indexer.on()")]);
    assert!(s.validate().is_err(), "empty condition must be rejected");
}

#[test]
fn schema_empty_action_rejected() {
    let s = schema(vec![rule("gear_down", "")]);
    assert!(s.validate().is_err(), "empty action must be rejected");
}

#[test]
fn schema_syntax_error_message_includes_rule_number() {
    let s = schema(vec![
        rule("gear_down", "led.indexer.on()"),       // valid
        rule("ias > abc", "led.indexer.on()"),        // invalid
        rule("flaps > 0.5", "led.indexer.on()"),      // valid
    ]);
    let err = s.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Rule 2"),
        "error should reference Rule 2 (1-indexed): {msg}"
    );
}

#[test]
fn schema_unknown_schema_version_detected() {
    let s = RulesSchema {
        schema: "flight.ledmap/99".to_string(),
        rules: vec![rule("gear_down", "led.indexer.on()")],
        defaults: None,
    };
    let err = s.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Unsupported schema version"),
        "should reject unknown schema version: {msg}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Property tests (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

use proptest::prelude::*;

proptest! {
    /// Parse → compile roundtrip: any well-formed condition+action that passes
    /// validation should also compile successfully.
    #[test]
    fn prop_validate_implies_compile(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        threshold in -5000.0f32..5000.0f32,
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let s = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: format!("{} > {}", var, threshold),
                do_action: format!("led.panel('{}').on()", target),
                action: format!("led.panel('{}').on()", target),
            }],
            defaults: None,
        };
        if s.validate().is_ok() {
            let compiled = s.compile();
            prop_assert!(compiled.is_ok(), "validate-ok should imply compile-ok: {:?}", compiled.err());
            prop_assert!(!compiled.unwrap().bytecode.instructions.is_empty());
        }
    }

    /// Compile determinism: same input always produces identical bytecode.
    #[test]
    fn prop_compile_determinism(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        threshold in -5000.0f32..5000.0f32,
    ) {
        let s = schema(vec![rule(
            &format!("{} <= {}", var, threshold),
            "led.indexer.on()",
        )]);
        let c1 = s.compile();
        let c2 = s.compile();
        prop_assert!(
            c1.is_ok(),
            "compile should succeed for generated schema (first run): {:?}",
            c1.err(),
        );
        prop_assert!(
            c2.is_ok(),
            "compile should succeed for generated schema (second run): {:?}",
            c2.err(),
        );
        let c1 = c1.unwrap();
        let c2 = c2.unwrap();
        prop_assert_eq!(
            format!("{:?}", c1.bytecode.instructions),
            format!("{:?}", c2.bytecode.instructions),
        );
        prop_assert_eq!(c1.bytecode.stack_size, c2.bytecode.stack_size);
        prop_assert_eq!(c1.bytecode.actions.len(), c2.bytecode.actions.len());
    }

    /// Validation is idempotent: calling validate() twice yields the same result.
    #[test]
    fn prop_validation_idempotent(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        threshold in -5000.0f32..5000.0f32,
    ) {
        let s = schema(vec![rule(
            &format!("{} >= {}", var, threshold),
            "led.indexer.on()",
        )]);
        let r1 = s.validate().is_ok();
        let r2 = s.validate().is_ok();
        prop_assert_eq!(r1, r2, "validate should be idempotent");
    }

    /// Variable map completeness: every variable referenced in conditions
    /// appears in the compiled bytecode's variable_map.
    #[test]
    fn prop_variable_map_completeness(
        var1 in "[a-zA-Z_][a-zA-Z0-9_]{0,8}",
        var2 in "[a-zA-Z_][a-zA-Z0-9_]{0,8}",
    ) {
        let cond = format!("{} and {}", var1, var2);
        let s = schema(vec![rule(&cond, "led.indexer.on()")]);
        if let Ok(compiled) = s.compile() {
            let vm = &compiled.bytecode.variable_map;
            prop_assert!(vm.contains_key(&var1), "var1 {:?} missing from variable_map", var1);
            prop_assert!(vm.contains_key(&var2), "var2 {:?} missing from variable_map", var2);
        }
    }

    /// Stack size is always at least the minimum (8) when instructions exist.
    #[test]
    fn prop_stack_size_minimum(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let s = schema(vec![rule(&var, &format!("led.panel('{}').on()", target))]);
        if let Ok(compiled) = s.compile()
            && !compiled.bytecode.instructions.is_empty()
        {
            prop_assert!(
                compiled.bytecode.stack_size >= 8,
                "stack_size should be ≥ 8, got {}",
                compiled.bytecode.stack_size
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Bonus: Cross-cutting integration tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn integration_conflict_detection_with_compilation() {
    // Rules that conflict should still compile individually
    let rules = vec![
        rule("gear_down", "led.panel('GEAR').on()"),
        rule("!gear_down", "led.panel('GEAR').off()"),
    ];
    let conflicts = check_conflicts(&rules);
    assert_eq!(conflicts.len(), 1, "should detect 1 conflict");
    assert_eq!(conflicts[0].output, "GEAR");

    // Both rules should still compile
    let s = schema(rules);
    assert!(s.compile().is_ok(), "conflicting rules should still compile");
}

#[test]
fn integration_multi_rule_variable_dedup() {
    // Variables used across rules should be deduplicated in the bytecode
    let compiled = compile_ok(&schema(vec![
        rule("ias > 200", "led.panel('FAST').on()"),
        rule("ias < 100", "led.panel('SLOW').on()"),
        rule("ias >= 150", "led.panel('MED').on()"),
    ]));
    let bc = &compiled.bytecode;
    assert!(
        bc.variable_map.contains_key("ias"),
        "ias should appear in variable_map"
    );
    // Only 1 variable (ias) should be registered
    assert_eq!(
        bc.variable_map.len(),
        1,
        "same variable across rules should be deduplicated"
    );
}

#[test]
fn integration_negated_enum_state() {
    // "gear != UP" should map to negated boolean "gear_UP"
    let compiled = compile_ok(&schema(vec![rule(
        "gear != UP",
        "led.panel('GEAR').on()",
    )]));
    let bc = &compiled.bytecode;
    assert!(
        bc.variable_map.contains_key("gear_UP"),
        "negated enum should map to composite variable: {:?}",
        bc.variable_map
    );
    assert!(
        bc.instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::Not)),
        "negated enum condition should emit Not op"
    );
}

#[test]
fn integration_empty_ruleset_compiles_to_empty_program() {
    let compiled = compile_ok(&schema(vec![]));
    let bc = &compiled.bytecode;
    assert!(bc.instructions.is_empty(), "empty ruleset → empty instructions");
    assert!(bc.actions.is_empty(), "empty ruleset → empty actions");
    assert!(bc.variable_map.is_empty(), "empty ruleset → empty variable_map");
}
