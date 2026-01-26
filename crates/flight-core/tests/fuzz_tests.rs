use flight_profile::{Profile, PROFILE_SCHEMA_VERSION};
use flight_rules::{RulesSchema, Rule};
use proptest::prelude::*;

//
// JSON Deserialization Fuzzing
//

proptest! {
    #[test]
    fn test_profile_json_deserialization_no_panic(s in "\\PC*") {
        // Attempt to deserialize random strings
        // Should return Result::Err but never panic
        let _ = serde_json::from_str::<Profile>(&s);
    }

    #[test]
    fn test_rules_json_deserialization_no_panic(s in "\\PC*") {
        let _ = serde_json::from_str::<RulesSchema>(&s);
    }
}

//
// Robustness against Weird Values (NaN, Inf, etc.)
//

// Helper to generate problematic f32s
prop_compose! {
    fn arb_weird_f32()(val in prop::sample::select(&[
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        f32::MIN,
        0.0,
        -0.0,
        1.0,
        -1.0
    ])) -> f32 {
        val
    }
}

// We need manual construction because standard arbitrary might not include NaNs easily
// or we want to specifically target them.

proptest! {
    #[test]
    fn test_profile_validation_robustness(
        deadzone in arb_weird_f32(),
        expo in arb_weird_f32(),
        slew in arb_weird_f32()
    ) {
        // Construct a partial profile with weird values
        // We use string interpolation or manual JSON construction to bypass type checks if we were using structs directly?
        // No, we can use the struct.
        
        use flight_profile::{AxisConfig, AircraftId};
        use std::collections::HashMap;

        let mut axes = HashMap::new();
        axes.insert("test_axis".to_string(), AxisConfig {
            deadzone: Some(deadzone),
            expo: Some(expo),
            slew_rate: Some(slew),
            detents: vec![],
            curve: None,
        });

        let profile = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("test".to_string()),
            aircraft: Some(AircraftId { icao: "TEST".to_string() }),
            axes,
            pof_overrides: None,
        };

        // Validate should handle these gracefully (return Err), specifically not panic on NaN comparisons
        let _ = profile.validate();
    }
}

//
// Rules Fuzzing (Repeated from property tests but specialized)
//

proptest! {
    #[test]
    fn test_rules_parsing_robustness(
        when in "\\PC*",
        action in "\\PC*"
    ) {
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when,
                do_action: action.clone(),
                action,
            }],
            defaults: None,
        };

        // compile calls parsing logic
        let _ = schema.compile();
    }
}

//
// Units Fuzzing
//

proptest! {
    #[test]
    fn test_units_normalization_robustness(val in arb_weird_f32()) {
        use flight_units::angles;
        
        let _ = angles::normalize_degrees_signed(val);
        let _ = angles::normalize_degrees_unsigned(val);
    }

    #[test]
    fn test_units_conversion_robustness(val in arb_weird_f32()) {
        use flight_units::conversions;
        
        // These are pure math, should not panic even with NaN/Inf
        let _ = conversions::degrees_to_radians(val);
        let _ = conversions::knots_to_mps(val);
        let _ = conversions::feet_to_meters(val);
    }
}
