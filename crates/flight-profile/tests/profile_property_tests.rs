use flight_profile::{Profile, AxisConfig, AircraftId, DetentZone, CurvePoint, PROFILE_SCHEMA_VERSION};
use proptest::prelude::*;

prop_compose! {
    fn arb_aircraft_id()(icao in "[A-Z0-9]{4}") -> AircraftId {
        AircraftId { icao }
    }
}

prop_compose! {
    fn arb_detent_zone()(
        position in -1.0f32..1.0f32,
        width in 0.01f32..0.5f32,
        role in "[a-z]+"
    ) -> DetentZone {
        DetentZone { position, width, role }
    }
}

prop_compose! {
    fn arb_curve_point()(input in 0.0f32..1.0f32, output in 0.0f32..1.0f32) -> CurvePoint {
        CurvePoint { input, output }
    }
}

prop_compose! {
    fn arb_axis_config()(
        deadzone in prop::option::of(0.0f32..0.5f32),
        expo in prop::option::of(0.0f32..1.0f32),
        slew_rate in prop::option::of(0.0f32..100.0f32),
        detents in prop::collection::vec(arb_detent_zone(), 0..3),
        curve in prop::option::of(prop::collection::vec(arb_curve_point(), 2..5))
    ) -> AxisConfig {
        AxisConfig { deadzone, expo, slew_rate, detents, curve }
    }
}

prop_compose! {
    fn arb_profile()(
        sim in prop::option::of("[a-z]+"),
        aircraft in prop::option::of(arb_aircraft_id()),
        axes in prop::collection::hash_map("[a-z]+", arb_axis_config(), 0..3)
    ) -> Profile {
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim,
            aircraft,
            axes,
            pof_overrides: None // Simplifying for now
        }
    }
}

proptest! {
    #[test]
    fn test_canonicalize_deterministic(profile in arb_profile()) {
        let c1 = profile.canonicalize();
        let c2 = profile.canonicalize();
        prop_assert_eq!(c1, c2);
    }
    
    #[test]
    fn test_merge_idempotent(base in arb_profile(), override_profile in arb_profile()) {
        // (Base.merge(Override)).merge(Override) == Base.merge(Override)
        // Applying the same override twice should be the same as applying it once
        
        let merged1 = base.merge_with(&override_profile).unwrap();
        let merged2 = merged1.merge_with(&override_profile).unwrap();
        
        prop_assert_eq!(merged1, merged2);
    }

    #[test]
    fn test_serialization_roundtrip(profile in arb_profile()) {
        let serialized = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(profile, deserialized);
    }
}
