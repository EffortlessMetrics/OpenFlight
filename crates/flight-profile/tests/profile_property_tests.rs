use flight_profile::{
    AircraftId, AxisConfig, CurvePoint, DetentZone, PROFILE_SCHEMA_VERSION, Profile,
};
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
        AxisConfig { deadzone, expo, slew_rate, detents, curve, filter: None }
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

    /// serialize → deserialize → serialize produces structurally identical JSON.
    #[test]
    fn test_double_roundtrip_stable(profile in arb_profile()) {
        let json1 = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json1).unwrap();
        let json2 = serde_json::to_string(&restored).unwrap();
        // Compare via Value to ignore HashMap key ordering differences
        let val1: serde_json::Value = serde_json::from_str(&json1).unwrap();
        let val2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        prop_assert_eq!(val1, val2, "double round-trip produced different JSON structure");
    }

    /// merge_with always preserves the schema version from the base profile.
    #[test]
    fn test_merge_preserves_schema(base in arb_profile(), overlay in arb_profile()) {
        let merged = base.merge_with(&overlay).unwrap();
        prop_assert!(
            merged.schema == base.schema,
            "merge_with changed schema from '{}' to '{}'",
            base.schema, merged.schema
        );
    }

    /// effective_hash is identical for structurally equal profiles regardless
    /// of HashMap iteration order (verified by computing twice after a round-trip).
    #[test]
    fn test_effective_hash_stable_across_roundtrip(profile in arb_profile()) {
        let json = serde_json::to_string(&profile).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(
            profile.effective_hash(), restored.effective_hash(),
            "effective_hash differs after JSON round-trip"
        );
    }
}
