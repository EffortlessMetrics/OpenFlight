// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted mutation-killing tests for flight-rules bytecode compilation.

use flight_rules::{Action, BytecodeOp, CompareOp, Rule, RulesSchema};

/// Helper: build a `RulesSchema` from a list of (when, action) pairs.
fn make_schema(rules: Vec<(&str, &str)>) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: rules
            .into_iter()
            .map(|(when, action)| Rule {
                when: when.to_string(),
                do_action: action.to_string(),
                action: action.to_string(),
            })
            .collect(),
        defaults: None,
    }
}

/// 1. AND short-circuit must emit JumpFalse for the second operand.
///    A mutant that removes short-circuit evaluation would produce no JumpFalse
///    inside the AND compilation (only the rule-level one remains).
///    We also verify exactly 1 Action instruction is emitted.
#[test]
fn and_short_circuit_emits_jump_false() {
    let schema = make_schema(vec![("ias >= 200 and gear_down", "led.panel('gear').on()")]);
    let compiled = schema.compile().expect("compilation must succeed");
    let bc = &compiled.bytecode;

    let jump_false_count = bc
        .instructions
        .iter()
        .filter(|op| matches!(op, BytecodeOp::JumpFalse(_)))
        .count();

    // One JumpFalse for AND short-circuit + one for rule-level conditional = at least 2
    assert!(
        jump_false_count >= 2,
        "expected at least 2 JumpFalse instructions (short-circuit + rule-level), got {jump_false_count}"
    );

    let action_count = bc
        .instructions
        .iter()
        .filter(|op| matches!(op, BytecodeOp::Action(_)))
        .count();
    assert_eq!(action_count, 1, "expected exactly 1 Action instruction");
}

/// 2. Compiling `led.panel('NAV').on()` must produce exactly one action
///    of variant `Action::LedOn` whose target is `"NAV"`.
#[test]
fn action_execution_produces_correct_action() {
    let schema = make_schema(vec![("nav_active", "led.panel('NAV').on()")]);
    let compiled = schema.compile().expect("compilation must succeed");
    let actions = &compiled.bytecode.actions;

    assert_eq!(actions.len(), 1, "expected exactly 1 action in the actions vec");

    match &actions[0] {
        Action::LedOn { target } => {
            assert_eq!(target, "NAV", "action target must be \"NAV\"");
        }
        other => panic!("expected Action::LedOn, got {other:?}"),
    }
}

/// 3. Variables appearing in rule order must receive deterministic,
///    distinct indices in the variable_map.  The first-seen variable
///    gets index 0 and the second gets index 1.
#[test]
fn variable_binding_deterministic() {
    let schema = make_schema(vec![
        ("ias > 100", "led.panel('SPD').on()"),
        ("altitude > 5000", "led.panel('ALT').on()"),
    ]);
    let compiled = schema.compile().expect("compilation must succeed");
    let var_map = &compiled.bytecode.variable_map;

    let ias_idx = var_map.get("ias").expect("variable_map must contain 'ias'");
    let alt_idx = var_map
        .get("altitude")
        .expect("variable_map must contain 'altitude'");

    assert_ne!(ias_idx, alt_idx, "'ias' and 'altitude' must have different indices");
    assert!(
        ias_idx < alt_idx,
        "first-seen variable 'ias' (idx {ias_idx}) must have a lower index than 'altitude' (idx {alt_idx})"
    );
}

/// 4. `>=` must parse as `CompareOp::GreaterEqual`, not `CompareOp::Greater`.
///    `>`  must parse as `CompareOp::Greater`.
///    This kills mutants that confuse the two-char and single-char operator paths.
#[test]
fn operator_precedence_ge_not_gt() {
    // GreaterEqual case
    let schema_ge = make_schema(vec![("ias >= 200", "led.panel('SPD').on()")]);
    let compiled_ge = schema_ge.compile().expect("compilation of >= must succeed");
    let has_ge = compiled_ge
        .bytecode
        .instructions
        .iter()
        .any(|op| matches!(op, BytecodeOp::Compare(CompareOp::GreaterEqual)));
    assert!(has_ge, "condition 'ias >= 200' must produce a Compare(GreaterEqual) instruction");

    let has_gt_in_ge = compiled_ge
        .bytecode
        .instructions
        .iter()
        .any(|op| matches!(op, BytecodeOp::Compare(CompareOp::Greater)));
    assert!(
        !has_gt_in_ge,
        "'ias >= 200' must NOT produce a Compare(Greater) instruction"
    );

    // Greater case
    let schema_gt = make_schema(vec![("ias > 200", "led.panel('SPD').on()")]);
    let compiled_gt = schema_gt.compile().expect("compilation of > must succeed");
    let has_gt = compiled_gt
        .bytecode
        .instructions
        .iter()
        .any(|op| matches!(op, BytecodeOp::Compare(CompareOp::Greater)));
    assert!(has_gt, "condition 'ias > 200' must produce a Compare(Greater) instruction");
}

/// 5. `!gear_down` must emit a `BytecodeOp::Not`; plain `gear_down` must not.
///    This kills mutants that drop the negation branch.
#[test]
fn negation_inverts_boolean() {
    // Negated boolean
    let schema_neg = make_schema(vec![("!gear_down", "led.panel('gear').off()")]);
    let compiled_neg = schema_neg.compile().expect("compilation of !gear_down must succeed");
    let has_not = compiled_neg
        .bytecode
        .instructions
        .iter()
        .any(|op| matches!(op, BytecodeOp::Not));
    assert!(has_not, "'!gear_down' must produce a Not instruction");

    // Non-negated boolean
    let schema_pos = make_schema(vec![("gear_down", "led.panel('gear').on()")]);
    let compiled_pos = schema_pos.compile().expect("compilation of gear_down must succeed");
    let has_not_pos = compiled_pos
        .bytecode
        .instructions
        .iter()
        .any(|op| matches!(op, BytecodeOp::Not));
    assert!(
        !has_not_pos,
        "'gear_down' (non-negated) must NOT produce a Not instruction"
    );
}
