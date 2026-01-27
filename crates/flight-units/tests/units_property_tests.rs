use flight_units::{angles, conversions};
use proptest::prelude::*;

proptest! {
    //
    // Angle Conversions
    //

    #[test]
    fn test_degrees_radians_roundtrip(degrees in -720.0f32..720.0f32) {
        let radians = conversions::degrees_to_radians(degrees);
        let back_to_degrees = conversions::radians_to_degrees(radians);

        // Floating point comparison with epsilon
        prop_assert!((degrees - back_to_degrees).abs() < 1e-4);
    }

    #[test]
    fn test_radians_degrees_roundtrip(radians in -10.0f32..10.0f32) {
        let degrees = conversions::radians_to_degrees(radians);
        let back_to_radians = conversions::degrees_to_radians(degrees);

        prop_assert!((radians - back_to_radians).abs() < 1e-4);
    }

    //
    // Angle Normalization
    //

    #[test]
    fn test_normalize_degrees_signed_bounds(degrees in -10000.0f32..10000.0f32) {
        let normalized = angles::normalize_degrees_signed(degrees);
        prop_assert!(normalized >= -180.0);
        // It can be slightly larger than 180.0 due to float precision, but logically <= 180
        prop_assert!(normalized <= 180.0 + 1e-5);
    }

    #[test]
    fn test_normalize_degrees_unsigned_bounds(degrees in -10000.0f32..10000.0f32) {
        let normalized = angles::normalize_degrees_unsigned(degrees);
        prop_assert!(normalized >= 0.0);
        prop_assert!(normalized < 360.0 + 1e-5);
    }

    #[test]
    fn test_normalize_degrees_signed_periodicity(degrees in -10000.0f32..10000.0f32) {
        let n1 = angles::normalize_degrees_signed(degrees);
        let n2 = angles::normalize_degrees_signed(degrees + 360.0);

        // They should be effectively the same
        let diff = (n1 - n2).abs();
        prop_assert!(diff < 1e-3);
    }

    //
    // Speed Conversions
    //

    #[test]
    fn test_knots_mps_roundtrip(knots in 0.0f32..1000.0f32) {
        let mps = conversions::knots_to_mps(knots);
        let back = conversions::mps_to_knots(mps);
        prop_assert!((knots - back).abs() < 1e-3);
    }

    #[test]
    fn test_kph_mps_roundtrip(kph in 0.0f32..1000.0f32) {
        let mps = conversions::kph_to_mps(kph);
        let back = conversions::mps_to_kph(mps);
        prop_assert!((kph - back).abs() < 1e-3);
    }

    #[test]
    fn test_knots_kph_consistency(knots in 0.0f32..1000.0f32) {
        // Direct conversion
        let kph_direct = conversions::knots_to_kph(knots);

        // Indirect conversion via MPS
        let mps = conversions::knots_to_mps(knots);
        let kph_indirect = conversions::mps_to_kph(mps);

        prop_assert!((kph_direct - kph_indirect).abs() < 1e-4);
    }

    #[test]
    fn test_kph_knots_consistency(kph in 0.0f32..1000.0f32) {
        let knots_direct = conversions::kph_to_knots(kph);

        let mps = conversions::kph_to_mps(kph);
        let knots_indirect = conversions::mps_to_knots(mps);

        prop_assert!((knots_direct - knots_indirect).abs() < 1e-4);
    }

    //
    // Distance/Altitude Conversions
    //

    #[test]
    fn test_feet_meters_roundtrip(feet in -10000.0f32..50000.0f32) {
        let meters = conversions::feet_to_meters(feet);
        let back = conversions::meters_to_feet(meters);
        prop_assert!((feet - back).abs() < 1e-2);
    }

    //
    // Vertical Speed Conversions
    //

    #[test]
    fn test_fpm_mps_roundtrip(fpm in -10000.0f32..10000.0f32) {
        let mps = conversions::fpm_to_mps(fpm);
        let back = conversions::mps_to_fpm(mps);
        prop_assert!((fpm - back).abs() < 0.1); // Slightly looser tolerance might be needed due to coefficients
    }
}
