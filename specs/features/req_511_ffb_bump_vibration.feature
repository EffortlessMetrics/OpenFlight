@REQ-511 @product
Feature: FFB Bump and Vibration Effects

  @AC-511.1
  Scenario: Bump effect generates a short directional impulse force
    Given the FFB engine is active
    When a bump effect is triggered with direction and intensity parameters
    Then the output force SHALL be a short directional impulse of the specified intensity

  @AC-511.2
  Scenario: Vibration effect generates symmetric oscillating force
    Given the FFB engine is active
    When a vibration effect is triggered with intensity and frequency parameters
    Then the output force SHALL oscillate symmetrically at the specified frequency

  @AC-511.3 @AC-511.4
  Scenario: Bump and vibration can be triggered by turbulence sim event
    Given a sim event for turbulence is received
    When the FFB engine processes the event
    Then a vibration effect SHALL be triggered with intensity derived from turbulence severity
    And the effect SHALL be bounded by envelope safety limits
