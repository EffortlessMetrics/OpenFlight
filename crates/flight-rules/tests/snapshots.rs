// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Additional snapshot tests for `flight-rules` compiled bytecode and validation.
//!
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_rules::{Action, BytecodeOp, Rule, RulesSchema};
use serde::Serialize;
use std::collections::BTreeMap;

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

// ── Compiled rule output for additional condition types ──────────────────────

#[test]
fn snapshot_compiled_ge_comparison() {
    let snap = compile_snapshot(&schema(vec![rule(
        "altitude >= 18000",
        "led.panel('FL').on()",
    )]));
    insta::assert_json_snapshot!("compiled_ge_comparison", snap);
}

#[test]
fn snapshot_compiled_equality_comparison() {
    let snap = compile_snapshot(&schema(vec![rule(
        "gear_position == 1",
        "led.panel('GEAR').on()",
    )]));
    insta::assert_json_snapshot!("compiled_equality_comparison", snap);
}

#[test]
fn snapshot_compiled_and_with_comparisons() {
    let snap = compile_snapshot(&schema(vec![rule(
        "altitude > 5000 and speed < 250",
        "led.panel('CRUISE').on()",
    )]));
    insta::assert_json_snapshot!("compiled_and_with_comparisons", snap);
}
