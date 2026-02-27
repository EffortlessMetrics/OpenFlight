Feature: War Thunder Ground Vehicle Axis Mapping
  As a flight simulation enthusiast
  I want the War Thunder adapter to support ground vehicle axis mapping
  So that I can use my hardware controls for tanks and other ground vehicles

  Background:
    Given the OpenFlight service is running
    And the War Thunder adapter is connected

  Scenario: Ground vehicle controls use separate axis mapping from aircraft
    Given a profile with both aircraft and ground vehicle axis mappings
    When a ground vehicle session is active in War Thunder
    Then the ground vehicle axis mapping is applied instead of the aircraft mapping

  Scenario: Vehicle type is detected from War Thunder telemetry
    When War Thunder sends telemetry indicating a ground vehicle session
    Then the adapter detects the vehicle type from the telemetry data

  Scenario: Profile auto-switch uses vehicle type for rule matching
    Given a profile with vehicle-type-based auto-switch rules
    When the vehicle type changes in War Thunder
    Then the profile auto-switches to the matching configuration

  Scenario: Ground vehicle mapping is configurable in profile
    When a ground vehicle axis mapping is defined in the profile
    Then the mapping is applied when a ground vehicle session is detected
