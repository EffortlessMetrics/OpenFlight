// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for the `flight-rules` DSL bytecode compiler output.
//!
//! Run `cargo test -p flight-rules snapshot -- --force-update-snapshots` to
//! regenerate `.snap` files after intentional compiler changes.

use flight_rules::{Action, BytecodeOp, Rule, RulesSchema};
use insta::assert_yaml_snapshot;
use serde::Serialize;
use std::collections::BTreeMap;

/// Snapshot-friendly projection of a compiled program.
///
/// Uses `BTreeMap` instead of `HashMap` so YAML key order is deterministic
/// across runs regardless of hash-map seed randomisation.
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

fn schema(rules: Vec<Rule>) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules,
        defaults: None,
    }
}

fn rule(when: &str, action: &str) -> Rule {
    Rule {
        when: when.to_string(),
        action: action.to_string(),
        do_action: action.to_string(),
    }
}

/// 1. Simple boolean rule: `gear_down` → `led.indexer.on()`
#[test]
fn snapshot_simple_boolean_rule() {
    let snap = compile_snapshot(&schema(vec![rule("gear_down", "led.indexer.on()")]));
    assert_yaml_snapshot!(snap);
}

/// 2. Comparison (less-than) rule: `speed < 100` → `led.panel('WARN').on()`
#[test]
fn snapshot_comparison_lt_rule() {
    let snap = compile_snapshot(&schema(vec![rule("speed < 100", "led.panel('WARN').on()")]));
    assert_yaml_snapshot!(snap);
}

/// 3. Negation rule: `!flap_deployed` → `led.indexer.off()`
#[test]
fn snapshot_negation_rule() {
    let snap = compile_snapshot(&schema(vec![rule("!flap_deployed", "led.indexer.off()")]));
    assert_yaml_snapshot!(snap);
}

/// 4. AND compound rule: `gear_down and speed < 200` → `led.panel('GEAR').on()`
#[test]
fn snapshot_and_compound_rule() {
    let snap = compile_snapshot(&schema(vec![rule(
        "gear_down and speed < 200",
        "led.panel('GEAR').on()",
    )]));
    assert_yaml_snapshot!(snap);
}

/// 5. Empty ruleset: no rules → empty instruction list
#[test]
fn snapshot_empty_ruleset() {
    let snap = compile_snapshot(&schema(vec![]));
    assert_yaml_snapshot!(snap);
}

/// 6. Chained actions: two rules each contributing one action to the program
#[test]
fn snapshot_chained_actions() {
    let snap = compile_snapshot(&schema(vec![
        rule("gear_down", "led.panel('GEAR').on()"),
        rule("!gear_down", "led.panel('GEAR').off()"),
    ]));
    assert_yaml_snapshot!(snap);
}
