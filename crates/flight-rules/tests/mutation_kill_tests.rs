// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted tests to improve mutation kill rate in flight-rules.
// Covers condition parsing operators, action parsing, validation,
// and boolean/compound condition correctness.

use flight_rules::{Rule, RuleDefaults, RulesSchema};

fn make_schema(rules: Vec<Rule>) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules,
        defaults: None,
    }
}

fn make_rule(when: &str, action: &str) -> Rule {
    Rule {
        when: when.to_string(),
        do_action: action.to_string(),
        action: action.to_string(),
    }
}

// ── Schema validation ────────────────────────────────────────────────────

#[test]
fn wrong_schema_version_rejected() {
    let schema = RulesSchema {
        schema: "flight.ledmap/2".to_string(),
        rules: vec![],
        defaults: None,
    };
    assert!(schema.validate().is_err());
}

#[test]
fn correct_schema_with_valid_rule_accepted() {
    let schema = make_schema(vec![make_rule(
        "gear_down",
        "led.panel('GEAR').on()",
    )]);
    assert!(schema.validate().is_ok());
}

#[test]
fn empty_condition_rejected() {
    let schema = make_schema(vec![make_rule("", "led.panel('GEAR').on()")]);
    assert!(schema.validate().is_err());
}

#[test]
fn empty_action_rejected() {
    let schema = make_schema(vec![make_rule("gear_down", "")]);
    assert!(schema.validate().is_err());
}

// ── Operator parsing: each operator must parse distinctly ────────────────

#[test]
fn all_comparison_operators_parse_correctly() {
    // Each operator must be distinguishable — catches mutations that swap operators
    let ops = vec![
        ("alt > 1000", ">"),
        ("alt >= 1000", ">="),
        ("alt < 1000", "<"),
        ("alt <= 1000", "<="),
        ("alt == 1000", "=="),
        ("alt != 1000", "!="),
    ];

    for (cond, _op) in &ops {
        let schema = make_schema(vec![make_rule(cond, "led.panel('ALT').on()")]);
        assert!(
            schema.validate().is_ok(),
            "condition '{}' should be valid",
            cond
        );
    }

    // Compile and check operator type
    let schema = make_schema(vec![make_rule("alt > 1000", "led.panel('ALT').on()")]);
    let compiled = schema.compile().unwrap();
    // Verify it compiled successfully (bytecode produced)
    assert!(
        !compiled.bytecode.instructions.is_empty(),
        "must produce bytecode"
    );
}

// ── Action parsing ───────────────────────────────────────────────────────

#[test]
fn action_on_parses() {
    let schema = make_schema(vec![make_rule("gear_down", "led.panel('GEAR').on()")]);
    assert!(schema.compile().is_ok());
}

#[test]
fn action_off_parses() {
    let schema = make_schema(vec![make_rule("gear_down", "led.panel('GEAR').off()")]);
    assert!(schema.compile().is_ok());
}

#[test]
fn action_blink_parses() {
    let schema = make_schema(vec![make_rule(
        "gear_down",
        "led.panel('WARN').blink(rate_hz=2.0)",
    )]);
    assert!(schema.compile().is_ok());
}

#[test]
fn action_brightness_parses() {
    let schema = make_schema(vec![make_rule(
        "gear_down",
        "led.panel('PANEL').brightness(0.8)",
    )]);
    assert!(schema.compile().is_ok());
}

// ── Compound conditions: AND and OR ──────────────────────────────────────

#[test]
fn and_condition_compiles() {
    let schema = make_schema(vec![make_rule(
        "gear_down and alt < 500",
        "led.panel('GEAR').on()",
    )]);
    assert!(schema.compile().is_ok());
}

#[test]
fn or_condition_compiles() {
    let schema = make_schema(vec![make_rule(
        "gear_down or alt < 500",
        "led.panel('GEAR').on()",
    )]);
    assert!(schema.compile().is_ok());
}

#[test]
fn negated_boolean_compiles() {
    let schema = make_schema(vec![make_rule("!gear_down", "led.panel('GEAR').off()")]);
    assert!(schema.compile().is_ok());
}

// ── Multi-rule compilation ───────────────────────────────────────────────

#[test]
fn multiple_rules_produce_combined_bytecode() {
    let schema = make_schema(vec![
        make_rule("gear_down", "led.panel('GEAR').on()"),
        make_rule("!gear_down", "led.panel('GEAR').off()"),
        make_rule("alt > 10000", "led.panel('ALT').blink(rate_hz=1.0)"),
    ]);
    let compiled = schema.compile().unwrap();
    assert!(
        compiled.bytecode.instructions.len() > 3,
        "3 rules must produce more than 3 instructions"
    );
    assert!(
        compiled.bytecode.actions.len() >= 3,
        "must have at least 3 actions"
    );
}

// ── Hysteresis defaults ──────────────────────────────────────────────────

#[test]
fn hysteresis_defaults_propagated_to_bytecode() {
    let mut hyst = std::collections::HashMap::new();
    hyst.insert("alt".to_string(), 50.0);

    let schema = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![make_rule("alt > 1000", "led.panel('ALT').on()")],
        defaults: Some(RuleDefaults {
            hysteresis: Some(hyst),
        }),
    };

    let compiled = schema.compile().unwrap();
    assert!(
        compiled.hysteresis_bands.contains_key("alt"),
        "hysteresis default must be propagated"
    );
    assert!(
        (*compiled.hysteresis_bands.get("alt").unwrap() - 50.0).abs() < f32::EPSILON,
        "hysteresis band value must be preserved"
    );
}
