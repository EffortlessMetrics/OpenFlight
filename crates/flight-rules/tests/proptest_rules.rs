// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Additional proptest coverage for the `flight-rules` DSL.
//!
//! Covers OR compound conditions, multi-rule action counts, and brightness
//! actions — paths not exercised by the existing proptest suites.

use flight_rules::{Rule, RulesSchema};
use proptest::prelude::*;

fn make_schema(rules: Vec<Rule>) -> RulesSchema {
    RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules,
        defaults: None,
    }
}

proptest! {
    /// OR compound conditions always compile to non-empty bytecode without panicking.
    #[test]
    fn or_compound_condition_compiles(
        var1 in "[a-zA-Z_][a-zA-Z0-9_]{1,15}",
        var2 in "[a-zA-Z_][a-zA-Z0-9_]{1,15}",
        target in "[A-Z][A-Z0-9_]{0,10}",
    ) {
        let schema = make_schema(vec![Rule {
            when: format!("{} or {}", var1, var2),
            do_action: format!("led.panel('{}').on()", target),
            action: format!("led.panel('{}').on()", target),
        }]);
        let result = schema.compile();
        prop_assert!(result.is_ok(), "OR compound rule should compile: {:?}", result.err());
        prop_assert!(
            !result.unwrap().bytecode.instructions.is_empty(),
            "compiled OR rule must produce instructions"
        );
    }

    /// A schema with N valid rules always contains exactly N action entries in
    /// the compiled bytecode (no silent deduplication or elision).
    #[test]
    fn n_rules_produce_exactly_n_actions(
        var in "[a-zA-Z_][a-zA-Z0-9_]{1,15}",
        target in "[A-Z][A-Z0-9_]{0,10}",
        n in 1usize..=5,
    ) {
        let rule = Rule {
            when: var,
            do_action: format!("led.panel('{}').on()", target),
            action: format!("led.panel('{}').on()", target),
        };
        let schema = make_schema(vec![rule; n]);
        if let Ok(compiled) = schema.compile() {
            prop_assert_eq!(
                compiled.bytecode.actions.len(),
                n,
                "{} rules must produce exactly {} action entries", n, n
            );
        }
    }

    /// `led.panel(...).brightness(v)` actions always validate when `v` is in
    /// the finite range [0.0, 1.0].
    #[test]
    fn brightness_action_validates_for_unit_range(
        target in "[A-Z][A-Z0-9_]{0,10}",
        brightness in 0.0f32..=1.0f32,
    ) {
        let action = format!("led.panel('{}').brightness({})", target, brightness);
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: action.clone(),
                action: action.clone(),
            }],
            defaults: None,
        };
        prop_assert!(
            schema.validate().is_ok(),
            "brightness action {:?} should validate: {:?}", action, schema.validate().err()
        );
    }
}
