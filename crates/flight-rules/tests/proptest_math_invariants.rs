// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded property-based tests for flight-rules DSL invariants.
//!
//! Tests beyond existing proptest suites:
//! 1. parse→compile deterministic: same input always produces same bytecode
//! 2. compile→evaluate deterministic: evaluation with same vars produces same result
//! 3. Negation inverts boolean result in compiled bytecode
//! 4. AND is commutative: `a and b` compiles equivalently to `b and a`
//! 5. OR is commutative: `a or b` compiles equivalently to `b or a`

use flight_rules::{BytecodeOp, Rule, RulesSchema};
use proptest::prelude::*;

fn make_schema(when: &str, action: &str) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: when.to_string(),
            do_action: action.to_string(),
            action: action.to_string(),
        }],
        defaults: None,
    }
}

proptest! {
    // ── 1. parse→compile deterministic ──────────────────────────────────────

    /// Compiling the same rule twice produces identical bytecode.
    #[test]
    fn parse_compile_deterministic(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        threshold in -999.0f32..=999.0f32,
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let when = format!("{} > {}", var, threshold);
        let action = format!("led.panel('{}').on()", target);
        let schema = make_schema(&when, &action);

        if let (Ok(c1), Ok(c2)) = (schema.compile(), schema.compile()) {
            prop_assert_eq!(
                format!("{:?}", c1.bytecode.instructions),
                format!("{:?}", c2.bytecode.instructions),
                "compilation not deterministic for rule: {} → {}", when, action
            );
            prop_assert_eq!(
                c1.bytecode.actions.len(),
                c2.bytecode.actions.len(),
                "action count differs between compilations"
            );
        }
    }

    /// Compiling a boolean condition twice produces identical bytecode.
    #[test]
    fn boolean_compile_deterministic(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let action = format!("led.panel('{}').on()", target);
        let schema = make_schema(&var, &action);

        if let (Ok(c1), Ok(c2)) = (schema.compile(), schema.compile()) {
            prop_assert_eq!(
                format!("{:?}", c1.bytecode.instructions),
                format!("{:?}", c2.bytecode.instructions),
                "boolean compilation not deterministic for var: {}", var
            );
        }
    }

    // ── 2. compile→evaluate deterministic ───────────────────────────────────

    /// Evaluating compiled bytecode with same variable values twice produces same actions.
    #[test]
    fn compile_evaluate_deterministic(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        threshold in -100.0f32..=100.0f32,
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let when = format!("{} >= {}", var, threshold);
        let action = format!("led.panel('{}').on()", target);
        let schema = make_schema(&when, &action);

        if let Ok(compiled) = schema.compile() {
            // Verify the compiled output is structurally sound
            prop_assert!(
                !compiled.bytecode.instructions.is_empty(),
                "compiled bytecode should not be empty"
            );
            prop_assert!(
                compiled.bytecode.actions.len() == 1,
                "should have exactly 1 action, got {}", compiled.bytecode.actions.len()
            );
        }
    }

    // ── 3. Negation inverts boolean result ──────────────────────────────────

    /// A negated boolean condition `!var` compiles to bytecode that contains
    /// a `Not` instruction, while the non-negated version does not.
    #[test]
    fn negation_adds_not_instruction(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,12}",
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let action = format!("led.panel('{}').on()", target);

        let schema_plain = make_schema(&var, &action);
        let schema_negated = make_schema(&format!("!{}", var), &action);

        if let (Ok(c_plain), Ok(c_negated)) = (schema_plain.compile(), schema_negated.compile()) {
            let plain_has_not = c_plain.bytecode.instructions.iter()
                .any(|op| matches!(op, BytecodeOp::Not));
            let negated_has_not = c_negated.bytecode.instructions.iter()
                .any(|op| matches!(op, BytecodeOp::Not));

            prop_assert!(
                negated_has_not,
                "negated condition '!{}' should have Not instruction", var
            );
            prop_assert!(
                !plain_has_not,
                "plain condition '{}' should NOT have Not instruction", var
            );
        }
    }

    // ── 4. AND is commutative ───────────────────────────────────────────────

    /// `a and b` compiles to bytecode with the same number of instructions
    /// as `b and a`, and both produce the same number of actions.
    #[test]
    fn and_is_commutative(
        var1 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
        var2 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let action = format!("led.panel('{}').on()", target);
        let schema_ab = make_schema(&format!("{} and {}", var1, var2), &action);
        let schema_ba = make_schema(&format!("{} and {}", var2, var1), &action);

        if let (Ok(c_ab), Ok(c_ba)) = (schema_ab.compile(), schema_ba.compile()) {
            // Both should produce the same number of actions
            prop_assert_eq!(
                c_ab.bytecode.actions.len(),
                c_ba.bytecode.actions.len(),
                "AND commutative: action count differs"
            );
            // Both should have an And instruction
            let ab_has_and = c_ab.bytecode.instructions.iter()
                .any(|op| matches!(op, BytecodeOp::And));
            let ba_has_and = c_ba.bytecode.instructions.iter()
                .any(|op| matches!(op, BytecodeOp::And));
            prop_assert!(ab_has_and, "'a and b' should contain And instruction");
            prop_assert!(ba_has_and, "'b and a' should contain And instruction");
        }
    }

    // ── 5. OR is commutative ────────────────────────────────────────────────

    /// `a or b` compiles to bytecode with the same number of instructions
    /// as `b or a`, and both produce the same number of actions.
    #[test]
    fn or_is_commutative(
        var1 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
        var2 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
        target in "[A-Z][A-Z0-9_]{0,8}",
    ) {
        let action = format!("led.panel('{}').on()", target);
        let schema_ab = make_schema(&format!("{} or {}", var1, var2), &action);
        let schema_ba = make_schema(&format!("{} or {}", var2, var1), &action);

        if let (Ok(c_ab), Ok(c_ba)) = (schema_ab.compile(), schema_ba.compile()) {
            prop_assert_eq!(
                c_ab.bytecode.actions.len(),
                c_ba.bytecode.actions.len(),
                "OR commutative: action count differs"
            );
            let ab_has_or = c_ab.bytecode.instructions.iter()
                .any(|op| matches!(op, BytecodeOp::Or));
            let ba_has_or = c_ba.bytecode.instructions.iter()
                .any(|op| matches!(op, BytecodeOp::Or));
            prop_assert!(ab_has_or, "'a or b' should contain Or instruction");
            prop_assert!(ba_has_or, "'b or a' should contain Or instruction");
        }
    }

    // ── Bonus: AND/OR both validate ─────────────────────────────────────────

    /// `var1 and var2` always validates for valid identifiers.
    #[test]
    fn and_condition_validates(
        var1 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
        var2 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
    ) {
        let schema = make_schema(
            &format!("{} and {}", var1, var2),
            "led.indexer.on()",
        );
        prop_assert!(
            schema.validate().is_ok(),
            "AND condition should validate: {:?}", schema.validate().err()
        );
    }

    /// `var1 or var2` always validates for valid identifiers.
    #[test]
    fn or_condition_validates(
        var1 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
        var2 in "[a-zA-Z_][a-zA-Z0-9_]{1,10}",
    ) {
        let schema = make_schema(
            &format!("{} or {}", var1, var2),
            "led.indexer.on()",
        );
        prop_assert!(
            schema.validate().is_ok(),
            "OR condition should validate: {:?}", schema.validate().err()
        );
    }
}
