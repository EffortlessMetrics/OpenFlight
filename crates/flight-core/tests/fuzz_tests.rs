use flight_profile::{PROFILE_SCHEMA_VERSION, Profile};
use flight_rules::{Rule, RulesSchema};
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
            filter: None,
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

//
// Circuit Breaker State Machine — never panics regardless of event ordering
//

use flight_core::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
use std::time::Duration;

/// Encodes the three possible events that can be driven into a circuit breaker.
#[derive(Debug, Clone, Copy)]
enum CbEvent {
    CallAllowed,
    RecordSuccess,
    RecordFailure,
}

prop_compose! {
    fn arb_cb_event()(idx in 0u8..3) -> CbEvent {
        match idx {
            0 => CbEvent::CallAllowed,
            1 => CbEvent::RecordSuccess,
            _ => CbEvent::RecordFailure,
        }
    }
}

proptest! {
    /// CircuitBreaker never panics for arbitrary event sequences.
    #[test]
    fn test_circuit_breaker_never_panics(
        events in prop::collection::vec(arb_cb_event(), 0..50),
        failure_threshold in 1u32..10,
        success_threshold in 1u32..10,
    ) {
        let config = CircuitBreakerConfig {
            failure_threshold,
            success_threshold,
            timeout: Duration::from_millis(1),
        };
        let mut cb = CircuitBreaker::new(config);
        for event in &events {
            match event {
                CbEvent::CallAllowed => { let _ = cb.call_allowed(); }
                CbEvent::RecordSuccess => cb.record_success(),
                CbEvent::RecordFailure => cb.record_failure(),
            }
        }
        // State must always be one of the three valid states
        let state = cb.state();
        prop_assert!(
            state == CircuitState::Closed
                || state == CircuitState::Open
                || state == CircuitState::HalfOpen,
            "unexpected state: {:?}", state
        );
    }

    /// CircuitBreaker: rejection rate is always between 0.0 and 1.0.
    #[test]
    fn test_circuit_breaker_rejection_rate_bounded(
        events in prop::collection::vec(arb_cb_event(), 1..30),
    ) {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout: Duration::from_millis(1),
        };
        let mut cb = CircuitBreaker::new(config);
        for event in &events {
            match event {
                CbEvent::CallAllowed => { let _ = cb.call_allowed(); }
                CbEvent::RecordSuccess => cb.record_success(),
                CbEvent::RecordFailure => cb.record_failure(),
            }
        }
        let rate = cb.rejection_rate();
        prop_assert!(
            (0.0..=1.0).contains(&rate),
            "rejection rate {} out of [0, 1]", rate
        );
    }

    /// CircuitBreaker: total_calls >= total_rejections always.
    #[test]
    fn test_circuit_breaker_calls_ge_rejections(
        events in prop::collection::vec(arb_cb_event(), 0..40),
    ) {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_millis(1),
        };
        let mut cb = CircuitBreaker::new(config);
        for event in &events {
            match event {
                CbEvent::CallAllowed => { let _ = cb.call_allowed(); }
                CbEvent::RecordSuccess => cb.record_success(),
                CbEvent::RecordFailure => cb.record_failure(),
            }
        }
        prop_assert!(
            cb.total_calls() >= cb.total_rejections(),
            "total_calls({}) < total_rejections({})",
            cb.total_calls(), cb.total_rejections()
        );
    }
}
