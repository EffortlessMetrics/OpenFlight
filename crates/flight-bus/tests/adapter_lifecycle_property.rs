// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for adapter lifecycle validation.
//!
//! **Property 7: Adapter Lifecycle**
//! *For any* simulator adapter (MSFS, X-Plane, DCS), the lifecycle sequence
//! connect → stream → disconnect → reconnect SHALL complete successfully
//! with no NaN/Inf values in streamed snapshots under normal conditions.
//!
//! **Validates: Requirements 14.1, 14.2**

use flight_bus::fixtures::ScenarioType;
use flight_bus::integration_test::{AdapterIntegrationTest, AdapterType};
use flight_bus::types::AircraftId;
use proptest::prelude::*;

/// Strategy for generating adapter types
fn adapter_type_strategy() -> impl Strategy<Value = AdapterType> {
    prop_oneof![
        Just(AdapterType::Msfs),
        Just(AdapterType::XPlane),
        Just(AdapterType::Dcs),
    ]
}

/// Strategy for generating scenario types
fn scenario_type_strategy() -> impl Strategy<Value = ScenarioType> {
    prop_oneof![
        Just(ScenarioType::ColdAndDark),
        Just(ScenarioType::GroundIdle),
        Just(ScenarioType::Takeoff),
        Just(ScenarioType::Cruise),
        Just(ScenarioType::Approach),
        Just(ScenarioType::Emergency),
    ]
}

/// Strategy for generating frame counts (reasonable range for testing)
fn frame_count_strategy() -> impl Strategy<Value = usize> {
    10usize..100
}

/// Strategy for generating aircraft IDs
fn aircraft_strategy() -> impl Strategy<Value = AircraftId> {
    prop_oneof![
        Just(AircraftId::new("C172")),
        Just(AircraftId::new("A320")),
        Just(AircraftId::new("B738")),
        Just(AircraftId::new("F16C")),
        Just(AircraftId::new("A10C")),
        Just(AircraftId::new("Ka50")),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Property 7: Adapter Lifecycle**
    ///
    /// *For any* simulator adapter (MSFS, X-Plane, DCS), the lifecycle sequence
    /// connect → stream → disconnect → reconnect SHALL complete successfully
    /// with no NaN/Inf values in streamed snapshots under normal conditions.
    ///
    /// **Validates: Requirements 14.1, 14.2**
    #[test]
    fn prop_adapter_lifecycle_completes_successfully(
        adapter_type in adapter_type_strategy(),
        scenario in scenario_type_strategy(),
        frame_count in frame_count_strategy(),
    ) {
        // Get a valid aircraft for this adapter type
        let aircraft = adapter_type.default_aircraft();

        let test = AdapterIntegrationTest::new(adapter_type)
            .with_aircraft(aircraft)
            .with_scenario(scenario)
            .with_frame_count(frame_count);

        let result = test.run().expect("Test should not error");

        // Property: Connect phase must succeed
        prop_assert!(
            result.connect_success,
            "Connect phase failed for {:?} adapter with {:?} scenario",
            adapter_type,
            scenario
        );

        // Property: No NaN/Inf values in any snapshot
        prop_assert!(
            !result.nan_inf_detected,
            "NaN/Inf detected in {:?} adapter with {:?} scenario. Fields: {:?}",
            adapter_type,
            scenario,
            result.nan_inf_fields
        );

        // Property: Disconnect phase must succeed
        prop_assert!(
            result.disconnect_success,
            "Disconnect phase failed for {:?} adapter with {:?} scenario",
            adapter_type,
            scenario
        );

        // Property: Reconnect phase must succeed
        prop_assert!(
            result.reconnect_success,
            "Reconnect phase failed for {:?} adapter with {:?} scenario",
            adapter_type,
            scenario
        );

        // Property: Overall test must pass
        prop_assert!(
            result.passed,
            "Overall lifecycle test failed for {:?} adapter with {:?} scenario",
            adapter_type,
            scenario
        );

        // Property: Frames must be processed
        prop_assert!(
            result.frames_processed > 0,
            "No frames processed for {:?} adapter with {:?} scenario",
            adapter_type,
            scenario
        );

        // Property: All requested frames should be processed
        prop_assert_eq!(
            result.frames_processed,
            frame_count,
            "Not all frames processed for {:?} adapter: expected {}, got {}",
            adapter_type,
            frame_count,
            result.frames_processed
        );
    }

    /// Property test for adapter lifecycle with various aircraft
    ///
    /// Tests that the lifecycle works correctly for various aircraft types.
    ///
    /// **Validates: Requirements 14.1**
    #[test]
    fn prop_adapter_lifecycle_with_aircraft(
        adapter_type in adapter_type_strategy(),
        aircraft in aircraft_strategy(),
        scenario in scenario_type_strategy(),
    ) {
        let test = AdapterIntegrationTest::new(adapter_type)
            .with_aircraft(aircraft.clone())
            .with_scenario(scenario)
            .with_frame_count(30);

        let result = test.run().expect("Test should not error");

        prop_assert!(
            result.passed,
            "Lifecycle failed for {:?} adapter with aircraft {} and {:?} scenario",
            adapter_type,
            aircraft.icao,
            scenario
        );

        prop_assert!(
            !result.nan_inf_detected,
            "NaN/Inf detected for {:?} adapter with aircraft {}",
            adapter_type,
            aircraft.icao
        );
    }

    /// Property test for helicopter scenarios (DCS specific)
    ///
    /// Tests that helicopter-specific scenarios work correctly.
    ///
    /// **Validates: Requirements 14.1**
    #[test]
    fn prop_helicopter_lifecycle(
        frame_count in 10usize..50,
    ) {
        // Test helicopter scenarios with appropriate aircraft
        let helo_aircraft = vec![
            AircraftId::new("Ka50"),
            AircraftId::new("UH1H"),
            AircraftId::new("Mi8"),
        ];

        for aircraft in helo_aircraft {
            let test = AdapterIntegrationTest::new(AdapterType::Dcs)
                .with_aircraft(aircraft.clone())
                .with_scenario(ScenarioType::HeloHover)
                .with_frame_count(frame_count);

            let result = test.run().expect("Test should not error");

            prop_assert!(
                result.passed,
                "Helicopter lifecycle failed for aircraft {}",
                aircraft.icao
            );

            prop_assert!(
                !result.nan_inf_detected,
                "NaN/Inf detected in helicopter data for {}",
                aircraft.icao
            );
        }
    }
}

/// Additional unit tests for edge cases
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// Test minimum frame count
    #[test]
    fn test_minimum_frame_count() {
        for adapter_type in [AdapterType::Msfs, AdapterType::XPlane, AdapterType::Dcs] {
            let test = AdapterIntegrationTest::new(adapter_type).with_frame_count(1);

            let result = test.run().unwrap();
            assert!(result.passed, "Should pass with minimum frame count");
            assert_eq!(result.frames_processed, 1);
        }
    }

    /// Test all adapter types with all scenarios
    #[test]
    fn test_all_combinations() {
        let adapters = [AdapterType::Msfs, AdapterType::XPlane, AdapterType::Dcs];
        let scenarios = [
            ScenarioType::ColdAndDark,
            ScenarioType::GroundIdle,
            ScenarioType::Takeoff,
            ScenarioType::Cruise,
            ScenarioType::Approach,
            ScenarioType::Emergency,
        ];

        for adapter in adapters {
            for scenario in scenarios {
                let test = AdapterIntegrationTest::new(adapter)
                    .with_scenario(scenario)
                    .with_frame_count(10);

                let result = test.run().unwrap();
                assert!(
                    result.passed,
                    "{:?} with {:?} should pass",
                    adapter, scenario
                );
            }
        }
    }

    /// Test that phase results are properly recorded
    #[test]
    fn test_phase_results_recorded() {
        let test = AdapterIntegrationTest::new(AdapterType::Msfs).with_frame_count(10);

        let result = test.run().unwrap();

        // Should have 4 phases: Connect, Stream, Disconnect, Reconnect
        assert_eq!(result.phase_results.len(), 4);
        assert_eq!(result.phase_results[0].name, "Connect");
        assert_eq!(result.phase_results[1].name, "Stream");
        assert_eq!(result.phase_results[2].name, "Disconnect");
        assert_eq!(result.phase_results[3].name, "Reconnect");

        // All phases should succeed
        for phase in &result.phase_results {
            assert!(phase.success, "Phase {} should succeed", phase.name);
            assert!(
                phase.error.is_none(),
                "Phase {} should have no error",
                phase.name
            );
        }
    }

    /// Test large frame count
    #[test]
    fn test_large_frame_count() {
        let test = AdapterIntegrationTest::new(AdapterType::Msfs).with_frame_count(500);

        let result = test.run().unwrap();
        assert!(result.passed, "Should pass with large frame count");
        assert_eq!(result.frames_processed, 500);
    }

    /// Test all helicopter scenarios
    #[test]
    fn test_helicopter_scenarios() {
        let helo_aircraft = ["Ka50", "UH1H"];

        for aircraft in helo_aircraft {
            let test = AdapterIntegrationTest::new(AdapterType::Dcs)
                .with_aircraft(AircraftId::new(aircraft))
                .with_scenario(ScenarioType::HeloHover)
                .with_frame_count(20);

            let result = test.run().unwrap();
            assert!(
                result.passed,
                "Helicopter {} with HeloHover should pass",
                aircraft
            );
            assert!(
                !result.nan_inf_detected,
                "No NaN/Inf should be detected for helicopter {}",
                aircraft
            );
        }
    }
}
