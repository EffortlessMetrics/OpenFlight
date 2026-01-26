use flight_core::rules::{Rule, RulesSchema};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_rule_validation_no_panic(when in "\\PC*", action in "\\PC*") {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when,
                do_action: action.clone(),
                action,
            }],
            defaults: None,
        };
        // The property is that this should not panic
        let _ = schema.validate();
    }

    #[test]
    fn test_rule_compilation_no_panic(when in "\\PC*", action in "\\PC*") {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when,
                do_action: action.clone(),
                action,
            }],
            defaults: None,
        };
        
        // Only attempt compile if validate passes, or just ensure compile also handles garbage gracefully
        // Compile likely calls parsing logic which might panic on invalid input if not careful
        let _ = schema.compile();
    }
}
