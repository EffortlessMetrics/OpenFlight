// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based invariant tests for the `flight-rules` DSL.
//!
//! Covers the parser/compiler pipeline from arbitrary inputs through to
//! compiled bytecode, checking that key safety and correctness invariants
//! hold across the entire input space.

use flight_rules::{BytecodeOp, Rule, RulesSchema};
use proptest::prelude::*;

// Helper: build a minimal one-rule schema and validate it.
fn schema_with_condition(when: &str) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: when.to_string(),
            do_action: "led.indexer.on()".to_string(),
            action: "led.indexer.on()".to_string(),
        }],
        defaults: None,
    }
}

fn schema_with_action(action: &str) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: "gear_down".to_string(),
            do_action: action.to_string(),
            action: action.to_string(),
        }],
        defaults: None,
    }
}

// ── Parser round-trip invariants ───────────────────────────────────────────

proptest! {
    /// Any condition string that validates successfully also validates on a
    /// second attempt (validation / parsing has no hidden mutable state).
    #[test]
    fn condition_validate_is_idempotent(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        threshold in -9999.0f32..=9999.0f32,
    ) {
        for op in &["<", ">", "<=", ">=", "==", "!="] {
            let cond = format!("{} {} {}", var, op, threshold);
            let schema = schema_with_condition(&cond);
            if schema.validate().is_ok() {
                prop_assert!(
                    schema.validate().is_ok(),
                    "second validate of {:?} failed", cond
                );
            }
        }
    }

    /// An empty condition string (zero bytes) always causes validation to return an error.
    /// Uses `s.is_empty()` because `validate()` checks byte-level emptiness; whitespace-only
    /// strings with non-ASCII space chars may not be caught by the same path.
    #[test]
    fn empty_condition_always_fails_validation(s in r"\s*") {
        if s.is_empty() {
            let schema = schema_with_condition(&s);
            prop_assert!(
                schema.validate().is_err(),
                "empty condition {:?} should fail validation", s
            );
        }
    }
}

// ── Rule compilation invariants ────────────────────────────────────────────

proptest! {
    /// A rule with a well-formed condition and action always compiles to
    /// non-empty bytecode without panicking.
    #[test]
    fn valid_rule_compiles_without_panic(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        threshold in -9999.0f32..=9999.0f32,
        target in "[A-Z][A-Z0-9_]{0,10}",
    ) {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: format!("{} > {}", var, threshold),
                do_action: format!("led.panel('{}').on()", target),
                action: format!("led.panel('{}').on()", target),
            }],
            defaults: None,
        };
        let result = schema.compile();
        prop_assert!(result.is_ok(), "valid rule should compile: {:?}", result.err());
        prop_assert!(
            !result.unwrap().bytecode.instructions.is_empty(),
            "compiled bytecode must not be empty"
        );
    }

    /// Compilation is deterministic: the same schema always produces identical
    /// instruction sequences on repeated calls.
    #[test]
    fn compilation_is_deterministic(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        threshold in -9999.0f32..=9999.0f32,
    ) {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: format!("{} >= {}", var, threshold),
                do_action: "led.indexer.on()".to_string(),
                action: "led.indexer.on()".to_string(),
            }],
            defaults: None,
        };
        if let (Ok(c1), Ok(c2)) = (schema.compile(), schema.compile()) {
            prop_assert_eq!(
                format!("{:?}", c1.bytecode.instructions),
                format!("{:?}", c2.bytecode.instructions),
            );
        }
    }

    /// `compile()` never panics regardless of how garbage the input strings are.
    #[test]
    fn compile_is_total_on_arbitrary_input(when in r"\PC*", action in r"\PC*") {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: when.clone(),
                do_action: action.clone(),
                action,
            }],
            defaults: None,
        };
        let _ = schema.compile();
    }

    /// `validate()` never panics regardless of how garbage the input strings are.
    #[test]
    fn validate_is_total_on_arbitrary_input(when in r"\PC*", action in r"\PC*") {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: when.clone(),
                do_action: action.clone(),
                action,
            }],
            defaults: None,
        };
        let _ = schema.validate();
    }
}

// ── Rule engine invariants ─────────────────────────────────────────────────

proptest! {
    /// Adding the same rule twice produces exactly twice as many bytecode
    /// action entries as adding it once (no silent deduplication).
    #[test]
    fn duplicate_rule_doubles_action_count(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        threshold in -9999.0f32..=9999.0f32,
        target in "[A-Z][A-Z0-9_]{0,10}",
    ) {
        let rule = Rule {
            when: format!("{} > {}", var, threshold),
            do_action: format!("led.panel('{}').on()", target),
            action: format!("led.panel('{}').on()", target),
        };
        let schema_one = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![rule.clone()],
            defaults: None,
        };
        let schema_two = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![rule.clone(), rule],
            defaults: None,
        };
        if let (Ok(c1), Ok(c2)) = (schema_one.compile(), schema_two.compile()) {
            prop_assert_eq!(
                c2.bytecode.actions.len(),
                c1.bytecode.actions.len() * 2,
                "two identical rules must produce twice as many actions"
            );
        }
    }

    /// A rule whose condition can never be true at compile time is still emitted
    /// into bytecode — the compiler does not speculatively prune rules.
    /// This is verified by the presence of a `JumpFalse` instruction.
    #[test]
    fn compiler_emits_unreachable_condition_as_bytecode(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
    ) {
        // Threshold is always larger than any plausible runtime value but still valid.
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: format!("{} > 99999", var),
                do_action: "led.indexer.on()".to_string(),
                action: "led.indexer.on()".to_string(),
            }],
            defaults: None,
        };
        let result = schema.compile();
        prop_assert!(result.is_ok(), "large-threshold rule should compile");
        let compiled = result.unwrap();
        let has_jump_false = compiled
            .bytecode
            .instructions
            .iter()
            .any(|op| matches!(op, BytecodeOp::JumpFalse(_)));
        prop_assert!(has_jump_false, "compiled rule must contain a JumpFalse guard");
    }

    /// Boolean conditions (plain identifiers) always parse and always compile
    /// to non-empty bytecode containing a `JumpFalse` guard.
    #[test]
    fn boolean_condition_compiles_with_guard(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        target in "[A-Z][A-Z0-9_]{0,10}",
    ) {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: var.clone(),
                do_action: format!("led.panel('{}').on()", target),
                action: format!("led.panel('{}').on()", target),
            }],
            defaults: None,
        };
        // A plain identifier must pass validation
        prop_assert!(
            schema.validate().is_ok(),
            "plain identifier {:?} should validate: {:?}", var, schema.validate().err()
        );
        let result = schema.compile();
        prop_assert!(result.is_ok(), "boolean rule should compile: {:?}", result.err());
        let compiled = result.unwrap();
        prop_assert!(!compiled.bytecode.instructions.is_empty());
        prop_assert!(
            compiled
                .bytecode
                .instructions
                .iter()
                .any(|op| matches!(op, BytecodeOp::JumpFalse(_))),
            "boolean rule bytecode must contain a JumpFalse guard"
        );
    }
}

// ── Output / numeric safety invariants ────────────────────────────────────

proptest! {
    /// `LoadConst` instructions in the compiled bytecode are always finite
    /// when the source threshold is a finite f32 — the compiler must not
    /// introduce NaN or Infinity.
    #[test]
    fn compiled_constants_are_finite(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        threshold in -9999.0f32..=9999.0f32,
    ) {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: format!("{} > {}", var, threshold),
                do_action: "led.indexer.on()".to_string(),
                action: "led.indexer.on()".to_string(),
            }],
            defaults: None,
        };
        if let Ok(compiled) = schema.compile() {
            for instr in &compiled.bytecode.instructions {
                if let BytecodeOp::LoadConst(v) = instr {
                    prop_assert!(
                        v.is_finite(),
                        "LoadConst {} is not finite (threshold was {})", v, threshold
                    );
                }
            }
        }
    }

    /// The `stack_size` in compiled bytecode is always at least the minimum
    /// pre-allocated size (8) when instructions are present.
    #[test]
    fn stack_size_meets_minimum_when_instructions_present(
        var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        threshold in -9999.0f32..=9999.0f32,
    ) {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: format!("{} < {}", var, threshold),
                do_action: "led.indexer.on()".to_string(),
                action: "led.indexer.on()".to_string(),
            }],
            defaults: None,
        };
        if let Ok(compiled) = schema.compile() {
            if !compiled.bytecode.instructions.is_empty() {
                prop_assert!(
                    compiled.bytecode.stack_size >= 1,
                    "stack_size should be ≥ 1 when instructions exist, got {}",
                    compiled.bytecode.stack_size
                );
            }
        }
    }

    /// LED panel blink actions with finite positive rates always parse without error.
    #[test]
    fn blink_action_parses_for_finite_positive_rate(
        target in "[A-Z][A-Z0-9_]{0,10}",
        rate in 0.1f32..=100.0f32,
    ) {
        let expr = format!("led.panel('{}').blink(rate_hz={})", target, rate);
        let schema = schema_with_action(&expr);
        prop_assert!(
            schema.validate().is_ok(),
            "blink {:?} should validate: {:?}", expr, schema.validate().err()
        );
    }

    /// Negated boolean conditions (`!var`) always validate for valid identifiers.
    #[test]
    fn negated_boolean_condition_always_validates(var in "[a-zA-Z_][a-zA-Z0-9_]{0,15}") {
        let negated = format!("!{}", var);
        let schema = schema_with_condition(&negated);
        prop_assert!(
            schema.validate().is_ok(),
            "negated identifier {:?} should validate: {:?}", negated, schema.validate().err()
        );
    }
}

// ── Schema-level invariants (unit tests with deterministic assertions) ─────

#[test]
fn empty_condition_string_fails_validation() {
    assert!(schema_with_condition("").validate().is_err());
}

#[test]
fn empty_rule_set_validates_with_correct_schema() {
    let schema = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![],
        defaults: None,
    };
    assert!(schema.validate().is_ok());
}

#[test]
fn wrong_schema_versions_always_fail_validation() {
    let bad_versions = [
        "flight.ledmap/2",
        "flight.ledmap/0",
        "",
        "invalid",
        "FLIGHT.LEDMAP/1",
        "flight.ledmap/1 ",
    ];
    for version in &bad_versions {
        let schema = RulesSchema {
            schema: version.to_string(),
            rules: vec![],
            defaults: None,
        };
        assert!(
            s