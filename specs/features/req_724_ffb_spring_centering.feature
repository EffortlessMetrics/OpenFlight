Feature: FFB Spring Centering
  As a flight simulation enthusiast
  I want FFB spring effects to support configurable centering
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configurable center position
    Given a FFB spring effect is active
    When the center position is configured in the profile
    Then the spring centers at the configured position

  Scenario: Spring strength adjustable
    Given a FFB spring effect is active
    When the strength is adjusted in the profile
    Then the spring force reflects the configured strength

  Scenario: Centering respects safety envelope
    Given a FFB spring centering force is calculated
    When the force exceeds safety envelope limits
    Then the force is clamped to the envelope maximum

  Scenario: Works with all FFB devices
    Given a supported FFB device is connected
    When spring centering is activated
    Then the effect works correctly on the device
