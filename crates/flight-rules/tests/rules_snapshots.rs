// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive snapshot tests for `flight-rules` structured outputs.
//!
//! Covers compiled rule output, validation error messages, and schema
//! serialization. Run `cargo insta review` to accept changes.

use flight_rules::{Action, BytecodeOp, Rule, RuleDefaults, RulesSchema};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

// ── helpers ──────────────────────────────────────────────────────────────────

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

/// Snapshot-friendly projection of compiled bytecode with deterministic key order.
#[derive(Serialize)]
struct BytecodeSnapshot {
    instructions: Vec<BytecodeOp>,
    variable_map: BTreeMap<String, u16>,
    actions: Vec<Action>,
    stack_size: usize,
}

fn compile_snapshot(schema: &RulesSchema) -> BytecodeSnapshot {
    let compiled = schema.compile().expect("schema should compile");
    let bc = compiled.bytecode();
    BytecodeSnapshot {
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

// ── Compiled rule output for known conditions ────────────────────────────────

#[test]
fn snapshot_compiled_boolean_condition() {
    let snap = compile_snapshot(&schema(vec![rule("gear_down", "led.panel('GEAR').on()")]));
    insta::assert_json_snapshot!("compiled_boolean_condition", snap);
}

#[test]
fn snapshot_compiled_comparison_gt() {
    let snap = compile_snapshot(&schema(vec![rule(
        "altitude > 10000",
        "led.panel('ALT').on()",
    )]));
    insta::assert_json_snapshot!("compiled_comparison_gt", snap);
}

#[test]
fn snapshot_compiled_comparison_le() {
    let snap = compile_snapshot(&schema(vec![rule(
        "speed <= 60",
        "led.panel('STALL').blink(rate_hz=2)",
    )]));
    insta::assert_json_snapshot!("compiled_comparison_le", snap);
}

#[test]
fn snapshot_compiled_or_condition() {
    let snap = compile_snapshot(&schema(vec![rule(
        "gear_down or flaps_extended",
        "led.panel('CONFIG').on()",
    )]));
    insta::assert_json_snapshot!("compiled_or_condition", snap);
}

#[test]
fn snapshot_compiled_negation() {
    let snap = compile_snapshot(&schema(vec![rule(
        "!engine_running",
        "led.panel('ENG').off()",
    )]));
    insta::assert_json_snapshot!("compiled_negation", snap);
}

#[test]
fn snapshot_compiled_multiple_rules() {
    let snap = compile_snapshot(&schema(vec![
        rule("gear_down", "led.panel('GEAR').on()"),
        rule("!gear_down", "led.panel('GEAR').off()"),
        rule("speed < 70", "led.panel('STALL').blink(rate_hz=3)"),
    ]));
    insta::assert_json_snapshot!("compiled_multiple_rules", snap);
}

// ── Validation error messages for invalid rules ──────────────────────────────

#[test]
fn snapshot_validation_error_bad_schema_version() {
    let s = RulesSchema {
        schema: "flight.ledmap/99".to_string(),
        rules: vec![rule("gear_down", "led.indexer.on()")],
        defaults: None,
    };
    let err = s.validate().unwrap_err();
    insta::assert_snapshot!("validation_error_bad_schema_version", err.to_string());
}

#[test]
fn snapshot_validation_error_empty_condition() {
    let s = schema(vec![rule("", "led.indexer.on()")]);
    let err = s.validate().unwrap_err();
    insta::assert_snapshot!("validation_error_empty_condition", err.to_string());
}

#[test]
fn snapshot_validation_error_empty_action() {
    let s = schema(vec![rule("gear_down", "")]);
    let err = s.validate().unwrap_err();
    insta::assert_snapshot!("validation_error_empty_action", err.to_string());
}

#[test]
fn snapshot_validation_error_invalid_action_syntax() {
    let s = schema(vec![rule("gear_down", "not_a_valid_action()")]);
    let err = s.validate().unwrap_err();
    insta::assert_snapshot!("validation_error_invalid_action_syntax", err.to_string());
}

#[test]
fn snapshot_validation_error_invalid_number_in_condition() {
    let s = schema(vec![rule("speed < abc", "led.indexer.on()")]);
    let err = s.validate().unwrap_err();
    insta::assert_snapshot!(
        "validation_error_invalid_number_in_condition",
        err.to_string()
    );
}

// ── Schema serialization ─────────────────────────────────────────────────────

#[test]
fn snapshot_schema_minimal_json() {
    let s = schema(vec![rule("gear_down", "led.panel('GEAR').on()")]);
    insta::assert_json_snapshot!("schema_minimal_json", s);
}

#[test]
fn snapshot_schema_with_defaults_json() {
    let mut hysteresis = HashMap::new();
    hysteresis.insert("speed".to_string(), 5.0f32);
    hysteresis.insert("altitude".to_string(), 100.0f32);

    let s = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![
            rule("speed < 100", "led.panel('WARN').on()"),
            rule("altitude > 10000", "led.panel('ALT').on()"),
        ],
        defaults: Some(RuleDefaults {
            hysteresis: Some(hysteresis),
        }),
    };
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("schema_with_defaults_json", s);
    });
}

#[test]
fn snapshot_schema_multi_rule_yaml() {
    let s = schema(vec![
        rule("gear_down", "led.panel('GEAR').on()"),
        rule("!gear_down", "led.panel('GEAR').off()"),
        rule("speed < 70", "led.panel('STALL').blink(rate_hz=3)"),
        rule("gear_down and speed < 200", "led.panel('APPROACH').on()"),
    ]);
    insta::assert_yaml_snapshot!("schema_multi_rule_yaml", s);
}
