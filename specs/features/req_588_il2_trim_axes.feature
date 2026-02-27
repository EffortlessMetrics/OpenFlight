Feature: IL-2 Trim Axis Support
  As a flight simulation enthusiast
  I want the IL-2 adapter to report trim axis positions
  So that OpenFlight can use trim state in profiles and rules

  Background:
    Given the OpenFlight service is running
    And the IL-2 export adapter is configured and connected

  Scenario: IL-2 trim axis positions are included in telemetry frame
    When a telemetry frame is received from IL-2
    Then the frame data includes pitch trim, roll trim, and yaw trim positions

  Scenario: Trim positions are normalized to range -1.0 to 1.0
    Given IL-2 reports a raw pitch trim value of 50%
    When the adapter processes the telemetry frame
    Then the normalised pitch trim value exposed to the bus is 0.5

  Scenario: Trim axis bus snapshot includes pitch, roll, and yaw trim
    When the bus publishes an IL-2 axis snapshot
    Then the snapshot contains fields for pitch_trim, roll_trim, and yaw_trim

  Scenario: Trim axis data triggers profile rules matching on trim position
    Given a profile rule triggers when pitch_trim exceeds 0.8
    When the IL-2 adapter reports a pitch trim of 0.9
    Then the matching profile rule fires
